//! Bounded WavPack v4/v5 block and metadata parsing.
//!
//! This crate is not yet a decoder and is intentionally not registered by the
//! public facade. It provides the checked framing layer for the RM-014 decoder.

use rust_music_core::{MusicError, Result};
use std::io::{self, Read};

pub const MAX_BLOCK_BYTES: usize = 1024 * 1024;
pub const MAX_BLOCK_SAMPLES: u32 = 131_072;
const HEADER_BYTES: usize = 32;
const MAX_BIT_READ: u8 = 32;
const EXP2_1_256_Q32: u64 = 4_306_612_134;

const FLAG_MONO: u32 = 1 << 2;
const FLAG_HYBRID: u32 = 1 << 3;
const FLAG_FLOAT: u32 = 1 << 7;
const FLAG_INITIAL_BLOCK: u32 = 1 << 11;
const FLAG_FINAL_BLOCK: u32 = 1 << 12;
const FLAG_DSD: u32 = 1 << 31;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockHeader {
    pub block_bytes: usize,
    pub version: u16,
    pub total_samples: Option<u64>,
    pub block_index: u64,
    pub block_samples: u32,
    pub flags: u32,
    pub crc: u32,
}

impl BlockHeader {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < HEADER_BYTES {
            return Err(MusicError::Invalid("WavPack block header is truncated"));
        }
        if &bytes[..4] != b"wvpk" {
            return Err(MusicError::Invalid("missing WavPack block marker"));
        }
        let chunk_size = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let block_bytes = chunk_size
            .checked_add(8)
            .ok_or(MusicError::Limit("WavPack block length overflows"))?;
        if !(HEADER_BYTES..=MAX_BLOCK_BYTES).contains(&block_bytes) {
            return Err(MusicError::Limit(
                "WavPack block length outside supported bounds",
            ));
        }
        let version = u16::from_le_bytes(bytes[8..10].try_into().unwrap());
        if !(0x402..=0x410).contains(&version) {
            return Err(MusicError::Unsupported("unsupported WavPack version"));
        }
        let total_low = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
        let block_index_low = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
        // Version 0x407 is the legacy-compatible stream. WavPack 5's newer
        // stream format starts at 0x410 and defines the upper 40-bit fields.
        let (total_samples, block_index) = if version >= 0x410 {
            (
                if block_index_low == 0 && total_low != u32::MAX {
                    Some((u64::from(bytes[11]) << 32) | u64::from(total_low))
                } else {
                    None
                },
                (u64::from(bytes[10]) << 32) | u64::from(block_index_low),
            )
        } else {
            (
                if block_index_low == 0 && total_low != u32::MAX {
                    Some(u64::from(total_low))
                } else {
                    None
                },
                u64::from(block_index_low),
            )
        };
        let block_samples = u32::from_le_bytes(bytes[20..24].try_into().unwrap());
        let flags = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
        let crc = u32::from_le_bytes(bytes[28..32].try_into().unwrap());
        if block_samples > MAX_BLOCK_SAMPLES {
            return Err(MusicError::Limit(
                "WavPack block sample count exceeds limit",
            ));
        }
        if block_index.checked_add(u64::from(block_samples)).is_none() {
            return Err(MusicError::Invalid("WavPack block sample range overflows"));
        }
        Ok(Self {
            block_bytes,
            version,
            total_samples,
            block_index,
            block_samples,
            flags,
            crc,
        })
    }

    /// Validate only the simple integer mono/stereo profile intended for the
    /// first independently verified decoder milestone.
    pub fn validate_integer_mono_stereo(&self) -> Result<()> {
        if self.block_samples == 0 {
            return Err(MusicError::Invalid(
                "WavPack metadata block has no audio samples",
            ));
        }
        if self.flags & FLAG_HYBRID != 0 {
            return Err(MusicError::Unsupported(
                "WavPack hybrid data requires a validated correction stream",
            ));
        }
        if self.flags & (FLAG_FLOAT | FLAG_DSD) != 0 {
            return Err(MusicError::Unsupported(
                "WavPack float and DSD modes are unsupported",
            ));
        }
        if self.flags & (FLAG_INITIAL_BLOCK | FLAG_FINAL_BLOCK)
            != (FLAG_INITIAL_BLOCK | FLAG_FINAL_BLOCK)
        {
            return Err(MusicError::Unsupported(
                "WavPack multichannel block sequences are unsupported",
            ));
        }
        Ok(())
    }

    pub fn channels(&self) -> u8 {
        if self.flags & FLAG_MONO != 0 {
            1
        } else {
            2
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetadataBlock<'a> {
    pub id: u8,
    pub optional: bool,
    pub data: &'a [u8],
}

/// Initial median values carried by WavPack's entropy-variable metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntropyVariables {
    /// Three adaptive median partitions for each encoded channel.
    pub medians: [[u32; 3]; 2],
    pub channels: u8,
}

/// Parse the mandatory 6-byte mono or 12-byte stereo entropy metadata.
/// The signed-log representation is expanded with fixed-point integer math.
pub fn parse_entropy_variables(data: &[u8], channels: u8) -> Result<EntropyVariables> {
    let expected = match channels {
        1 => 6,
        2 => 12,
        _ => {
            return Err(MusicError::Unsupported(
                "WavPack entropy requires mono or stereo",
            ))
        }
    };
    if data.len() != expected {
        return Err(MusicError::Invalid(
            "WavPack entropy-variable metadata has the wrong length",
        ));
    }
    let mut medians = [[0; 3]; 2];
    for (index, chunk) in data.chunks_exact(2).enumerate() {
        let encoded = u16::from_le_bytes([chunk[0], chunk[1]]);
        medians[index / 3][index % 3] = expand_log_median(encoded)?;
    }
    Ok(EntropyVariables { medians, channels })
}

fn expand_log_median(encoded: u16) -> Result<u32> {
    let exponent = u32::from(encoded >> 8);
    if exponent > 31 {
        return Err(MusicError::Limit(
            "WavPack entropy median exceeds 32-bit decoder range",
        ));
    }
    let mut fraction = 1u128 << 32;
    for _ in 0..u32::from(encoded as u8) {
        fraction = (fraction * u128::from(EXP2_1_256_Q32) + (1u128 << 31)) >> 32;
    }
    let base = ((fraction + (1u128 << 23)) >> 24) as u32;
    let value = if exponent <= 9 {
        base >> (9 - exponent)
    } else {
        base.wrapping_shl(exponent - 9)
    };
    Ok(value)
}

/// Iterate through every metadata sub-block in one complete WavPack block.
/// The iterator borrows the input and does not allocate or retain payloads.
pub struct MetadataIter<'a> {
    payload: &'a [u8],
    offset: usize,
    failed: bool,
}

impl<'a> Iterator for MetadataIter<'a> {
    type Item = Result<MetadataBlock<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.offset == self.payload.len() {
            return None;
        }
        match parse_metadata(&self.payload[self.offset..]) {
            Ok((metadata, used)) => {
                self.offset += used;
                Some(Ok(metadata))
            }
            Err(error) => {
                self.failed = true;
                Some(Err(error))
            }
        }
    }
}

/// Create a zero-copy metadata iterator for one complete, bounded WavPack block.
pub fn iter_metadata(block: &[u8]) -> Result<MetadataIter<'_>> {
    let header = BlockHeader::parse(block)?;
    if block.len() != header.block_bytes {
        return Err(MusicError::Invalid(
            "WavPack metadata iterator requires exactly one complete block",
        ));
    }
    Ok(MetadataIter {
        payload: &block[HEADER_BYTES..],
        offset: 0,
        failed: false,
    })
}

/// Parse one bounded metadata sub-block from a WavPack audio-block payload.
/// Metadata lengths are stored in 16-bit words and data is padded to even size.
pub fn parse_metadata(input: &[u8]) -> Result<(MetadataBlock<'_>, usize)> {
    if input.len() < 2 {
        return Err(MusicError::Invalid("WavPack metadata header is truncated"));
    }
    let id_flags = input[0];
    let mut cursor = 2usize;
    let words = if id_flags & 0x80 != 0 {
        if input.len() < 4 {
            return Err(MusicError::Invalid(
                "WavPack large metadata header is truncated",
            ));
        }
        let words = u32::from_le_bytes([input[1], input[2], input[3], 0]);
        cursor = 4;
        words
    } else {
        u32::from(input[1])
    };
    let padded_len = usize::try_from(words)
        .ok()
        .and_then(|words| words.checked_mul(2))
        .ok_or(MusicError::Limit("WavPack metadata length overflows"))?;
    let data_len = padded_len
        .checked_sub(usize::from(id_flags & 0x40 != 0))
        .ok_or(MusicError::Invalid(
            "WavPack odd-length metadata has no payload",
        ))?;
    let end = cursor
        .checked_add(padded_len)
        .ok_or(MusicError::Limit("WavPack metadata end overflows"))?;
    if end > input.len() {
        return Err(MusicError::Invalid("WavPack metadata exceeds its block"));
    }
    Ok((
        MetadataBlock {
            id: id_flags & 0x1f,
            optional: id_flags & 0x20 != 0,
            data: &input[cursor..cursor + data_len],
        },
        end,
    ))
}

/// Bounded least-significant-bit-first reader for a WavPack metadata bitstream.
/// Failed multi-bit reads leave the cursor unchanged.
pub struct BitReader<'a> {
    bytes: &'a [u8],
    bit_offset: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            bit_offset: 0,
        }
    }

    pub fn remaining_bits(&self) -> usize {
        self.bytes.len() * 8 - self.bit_offset
    }

    pub fn read_bit(&mut self) -> Result<bool> {
        if self.remaining_bits() == 0 {
            return Err(MusicError::Invalid("WavPack bitstream is truncated"));
        }
        let byte = self.bytes[self.bit_offset / 8];
        let bit = (byte >> (self.bit_offset % 8)) & 1 != 0;
        self.bit_offset += 1;
        Ok(bit)
    }

    /// Read up to 32 bits, with the first stream bit becoming bit zero.
    pub fn read_bits(&mut self, width: u8) -> Result<u32> {
        if width > MAX_BIT_READ {
            return Err(MusicError::Limit(
                "WavPack bit read exceeds 32-bit result width",
            ));
        }
        if usize::from(width) > self.remaining_bits() {
            return Err(MusicError::Invalid("WavPack bitstream is truncated"));
        }
        let mut value = 0u32;
        for bit_index in 0..width {
            if self.read_bit()? {
                value |= 1u32 << bit_index;
            }
        }
        Ok(value)
    }

    /// Read a unary run of one bits terminated by zero, bounded by both the
    /// caller's cap and the remaining block payload.
    pub fn read_unary_ones(&mut self, max_ones: u32) -> Result<u32> {
        let mut count = 0u32;
        while self.read_bit()? {
            if count == max_ones {
                return Err(MusicError::Limit(
                    "WavPack unary code exceeds configured bound",
                ));
            }
            count += 1;
        }
        Ok(count)
    }
}

/// Read one complete WavPack block without exceeding the 1 MiB format limit.
pub fn read_block<R: Read>(input: &mut R) -> Result<Option<(BlockHeader, Vec<u8>)>> {
    let mut header_bytes = [0u8; HEADER_BYTES];
    let mut read = 0;
    while read < HEADER_BYTES {
        match input.read(&mut header_bytes[read..]) {
            Ok(0) if read == 0 => return Ok(None),
            Ok(0) => return Err(MusicError::Invalid("WavPack block header is truncated")),
            Ok(count) => read += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        }
    }
    let header = BlockHeader::parse(&header_bytes)?;
    let mut block = Vec::with_capacity(header.block_bytes);
    block.extend_from_slice(&header_bytes);
    block.resize(header.block_bytes, 0);
    input.read_exact(&mut block[HEADER_BYTES..])?;
    Ok(Some((header, block)))
}

/// Validate ordering and stream-wide invariants for a simple mono/stereo file.
/// This does not decode samples or validate the block's audio CRC.
#[derive(Debug, Default)]
pub struct BlockSequence {
    next_sample: u64,
    declared_total: Option<u64>,
    format_flags: Option<u32>,
    saw_audio: bool,
}

impl BlockSequence {
    /// Accept one header in physical file order. Non-audio metadata blocks are
    /// ignored; audio blocks must be contiguous and keep a stable PCM profile.
    pub fn accept(&mut self, header: &BlockHeader) -> Result<()> {
        if header.block_samples == 0 {
            return Ok(());
        }
        header.validate_integer_mono_stereo()?;
        if header.block_index != self.next_sample {
            return Err(MusicError::Invalid(
                "WavPack audio blocks are not contiguous",
            ));
        }
        let format_flags = header.flags
            & (0x3 | FLAG_MONO | FLAG_HYBRID | FLAG_FLOAT | (1 << 8) | (0xf << 23) | FLAG_DSD);
        if self
            .format_flags
            .is_some_and(|previous| previous != format_flags)
        {
            return Err(MusicError::Invalid(
                "WavPack PCM format changes between audio blocks",
            ));
        }
        if let Some(total) = header.total_samples {
            if self.declared_total.is_some() {
                return Err(MusicError::Invalid(
                    "WavPack total sample count is declared more than once",
                ));
            }
            let end = header
                .block_index
                .checked_add(u64::from(header.block_samples))
                .ok_or(MusicError::Limit("WavPack sample range overflows"))?;
            if total < end {
                return Err(MusicError::Invalid(
                    "WavPack total is shorter than its first audio block",
                ));
            }
            self.declared_total = Some(total);
        }
        self.next_sample = header
            .block_index
            .checked_add(u64::from(header.block_samples))
            .ok_or(MusicError::Limit("WavPack sample range overflows"))?;
        self.format_flags = Some(format_flags);
        self.saw_audio = true;
        Ok(())
    }

    /// Finish the file and compare its observed audio frames with the optional
    /// total advertised by its first audio block.
    pub fn finish(&self) -> Result<()> {
        if !self.saw_audio {
            return Err(MusicError::Invalid(
                "WavPack stream contains no audio blocks",
            ));
        }
        if self
            .declared_total
            .is_some_and(|total| total != self.next_sample)
        {
            return Err(MusicError::Invalid(
                "WavPack block samples do not match declared total",
            ));
        }
        Ok(())
    }

    pub fn decoded_frames(&self) -> u64 {
        self.next_sample
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn block(block_samples: u32, flags: u32) -> Vec<u8> {
        let mut bytes = b"wvpk".to_vec();
        bytes.extend_from_slice(&24u32.to_le_bytes()); // 32-byte header total.
        bytes.extend_from_slice(&0x410u16.to_le_bytes());
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&10_000u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&block_samples.to_le_bytes());
        bytes.extend_from_slice(&flags.to_le_bytes());
        bytes.extend_from_slice(&0x1234_5678u32.to_le_bytes());
        bytes
    }

    #[test]
    fn parses_bounded_header_and_channel_flags() {
        let bytes = block(128, FLAG_INITIAL_BLOCK | FLAG_FINAL_BLOCK | FLAG_MONO);
        let header = BlockHeader::parse(&bytes).unwrap();
        assert_eq!(header.block_bytes, HEADER_BYTES);
        assert_eq!(header.total_samples, Some(10_000));
        assert_eq!(header.block_index, 0);
        assert_eq!(header.block_samples, 128);
        assert_eq!(header.channels(), 1);
        header.validate_integer_mono_stereo().unwrap();
    }

    #[test]
    fn parses_ffmpeg_generated_lossless_block_and_all_metadata_lengths() {
        let fixture = include_bytes!("../tests/fixtures/ffmpeg-synthetic-mono16.wv");
        let (header, block) = read_block(&mut Cursor::new(fixture.as_slice()))
            .unwrap()
            .unwrap();
        assert_eq!(header.version, 0x410);
        assert_eq!(header.total_samples, Some(882));
        assert_eq!(header.block_index, 0);
        assert_eq!(header.block_samples, 882);
        assert_eq!(header.channels(), 1);
        header.validate_integer_mono_stereo().unwrap();

        let metadata = iter_metadata(&block)
            .unwrap()
            .map(|entry| entry.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            metadata.iter().map(|entry| entry.id).collect::<Vec<_>>(),
            [2, 3, 4, 5, 10]
        );
        assert!(metadata.iter().all(|entry| !entry.optional));

        let entropy = parse_entropy_variables(metadata[3].data, 1).unwrap();
        assert_eq!(entropy.channels, 1);
        assert_eq!(entropy.medians, [[79, 62, 229], [0; 3]]);
    }

    fn sequence_header(index: u32, samples: u32, total: u32, flags: u32) -> BlockHeader {
        let mut bytes = block(samples, flags);
        bytes[12..16].copy_from_slice(&total.to_le_bytes());
        bytes[16..20].copy_from_slice(&index.to_le_bytes());
        BlockHeader::parse(&bytes).unwrap()
    }

    #[test]
    fn block_sequence_accepts_contiguous_audio_and_skips_metadata_blocks() {
        let flags = FLAG_INITIAL_BLOCK | FLAG_FINAL_BLOCK | FLAG_MONO;
        let mut sequence = BlockSequence::default();
        sequence.accept(&sequence_header(0, 2, 4, flags)).unwrap();
        sequence
            .accept(&sequence_header(2, 0, u32::MAX, flags))
            .unwrap(); // Metadata-only block does not advance PCM position.
        sequence
            .accept(&sequence_header(2, 2, u32::MAX, flags))
            .unwrap();
        assert_eq!(sequence.decoded_frames(), 4);
        sequence.finish().unwrap();
    }

    #[test]
    fn block_sequence_rejects_gaps_overlaps_and_format_changes() {
        let flags = FLAG_INITIAL_BLOCK | FLAG_FINAL_BLOCK | FLAG_MONO;
        let first = sequence_header(0, 2, 4, flags);
        for bad_index in [1, 3] {
            let mut sequence = BlockSequence::default();
            sequence.accept(&first).unwrap();
            assert!(matches!(
                sequence.accept(&sequence_header(bad_index, 2, u32::MAX, flags)),
                Err(MusicError::Invalid(
                    "WavPack audio blocks are not contiguous"
                ))
            ));
        }
        let mut sequence = BlockSequence::default();
        sequence.accept(&first).unwrap();
        assert!(matches!(
            sequence.accept(&sequence_header(2, 2, u32::MAX, flags ^ 1)),
            Err(MusicError::Invalid(
                "WavPack PCM format changes between audio blocks"
            ))
        ));
    }

    #[test]
    fn block_sequence_checks_declared_total_and_audio_presence() {
        let flags = FLAG_INITIAL_BLOCK | FLAG_FINAL_BLOCK | FLAG_MONO;
        assert!(matches!(
            BlockSequence::default().finish(),
            Err(MusicError::Invalid(
                "WavPack stream contains no audio blocks"
            ))
        ));
        let mut sequence = BlockSequence::default();
        sequence.accept(&sequence_header(0, 2, 4, flags)).unwrap();
        assert!(matches!(
            sequence.finish(),
            Err(MusicError::Invalid(
                "WavPack block samples do not match declared total"
            ))
        ));
    }

    #[test]
    fn metadata_iterator_rejects_trailing_and_truncated_subblocks() {
        let mut complete = block(1, FLAG_INITIAL_BLOCK | FLAG_FINAL_BLOCK);
        complete[4..8].copy_from_slice(&28u32.to_le_bytes());
        complete.extend_from_slice(&[0x02, 1, 0x55, 0x00]);
        assert_eq!(iter_metadata(&complete).unwrap().count(), 1);

        let mut truncated = complete.clone();
        truncated[4..8].copy_from_slice(&27u32.to_le_bytes());
        truncated.pop();
        let mut metadata = iter_metadata(&truncated).unwrap();
        assert!(matches!(metadata.next(), Some(Err(MusicError::Invalid(_)))));
        assert!(metadata.next().is_none());

        complete.push(0xff);
        assert!(matches!(
            iter_metadata(&complete),
            Err(MusicError::Invalid(_))
        ));
    }

    #[test]
    fn legacy_stream_uses_32_bit_lengths_and_indices_only() {
        let mut bytes = block(128, FLAG_INITIAL_BLOCK | FLAG_FINAL_BLOCK);
        bytes[8..10].copy_from_slice(&0x40fu16.to_le_bytes());
        bytes[10] = 7;
        bytes[11] = 9;
        bytes[12..16].copy_from_slice(&10_000u32.to_le_bytes());
        let header = BlockHeader::parse(&bytes).unwrap();
        assert_eq!(header.total_samples, Some(10_000));
        assert_eq!(header.block_index, 0);

        bytes[16..20].copy_from_slice(&25u32.to_le_bytes());
        let header = BlockHeader::parse(&bytes).unwrap();
        assert_eq!(header.total_samples, None);
        assert_eq!(header.block_index, 25);

        bytes[8..10].copy_from_slice(&0x407u16.to_le_bytes());
        bytes[16..20].copy_from_slice(&0u32.to_le_bytes());
        let header = BlockHeader::parse(&bytes).unwrap();
        assert_eq!(header.total_samples, Some(10_000));
        assert_eq!(header.block_index, 0);

        bytes[16..20].copy_from_slice(&25u32.to_le_bytes());
        let header = BlockHeader::parse(&bytes).unwrap();
        assert_eq!(header.total_samples, None);
        assert_eq!(header.block_index, 25);
    }

    #[test]
    fn v5_uses_40_bit_fields_and_only_first_block_has_total_length() {
        let mut bytes = block(128, FLAG_INITIAL_BLOCK | FLAG_FINAL_BLOCK);
        bytes[8..10].copy_from_slice(&0x410u16.to_le_bytes());
        bytes[10] = 2;
        bytes[11] = 3;
        bytes[12..16].copy_from_slice(&10_000u32.to_le_bytes());
        let header = BlockHeader::parse(&bytes).unwrap();
        assert_eq!(header.total_samples, Some((3u64 << 32) | 10_000));
        assert_eq!(header.block_index, 2u64 << 32);

        bytes[16..20].copy_from_slice(&25u32.to_le_bytes());
        let header = BlockHeader::parse(&bytes).unwrap();
        assert_eq!(header.total_samples, None);
        assert_eq!(header.block_index, (2u64 << 32) | 25);
    }

    #[test]
    fn refuses_hybrid_float_dsd_and_multichannel_without_fallback() {
        for flags in [
            FLAG_HYBRID | FLAG_INITIAL_BLOCK | FLAG_FINAL_BLOCK,
            FLAG_FLOAT | FLAG_INITIAL_BLOCK | FLAG_FINAL_BLOCK,
            FLAG_DSD | FLAG_INITIAL_BLOCK | FLAG_FINAL_BLOCK,
            FLAG_INITIAL_BLOCK,
        ] {
            let header = BlockHeader::parse(&block(8, flags)).unwrap();
            assert!(header.validate_integer_mono_stereo().is_err());
        }
    }

    #[test]
    fn rejects_invalid_header_and_oversized_blocks() {
        let mut bytes = block(1, FLAG_INITIAL_BLOCK | FLAG_FINAL_BLOCK);
        bytes[8..10].copy_from_slice(&0x401u16.to_le_bytes());
        assert!(matches!(
            BlockHeader::parse(&bytes),
            Err(MusicError::Unsupported("unsupported WavPack version"))
        ));
        bytes[8..10].copy_from_slice(&0x410u16.to_le_bytes());
        bytes[4..8].copy_from_slice(&((MAX_BLOCK_BYTES as u32).to_le_bytes()));
        assert!(matches!(
            BlockHeader::parse(&bytes),
            Err(MusicError::Limit(_))
        ));
    }

    #[test]
    fn parses_small_large_and_odd_metadata_subblocks() {
        let small = [0x61, 1, 0x41, 0x00]; // optional, one actual byte, even-padded.
        let (metadata, used) = parse_metadata(&small).unwrap();
        assert_eq!(metadata.id, 1);
        assert!(metadata.optional);
        assert_eq!(metadata.data, &[0x41]);
        assert_eq!(used, 4);

        let large = [0x82, 1, 0, 0, 0xaa, 0xbb];
        let (metadata, used) = parse_metadata(&large).unwrap();
        assert_eq!(metadata.id, 2);
        assert_eq!(metadata.data, &[0xaa, 0xbb]);
        assert_eq!(used, 6);

        assert!(parse_metadata(&[0x80, 0xff, 0xff, 0xff]).is_err());
        assert!(parse_metadata(&[0x40, 0]).is_err());
    }

    #[test]
    fn entropy_variables_require_exact_mono_or_stereo_layouts() {
        assert!(matches!(
            parse_entropy_variables(&[0; 5], 1),
            Err(MusicError::Invalid(
                "WavPack entropy-variable metadata has the wrong length"
            ))
        ));
        assert!(matches!(
            parse_entropy_variables(&[0; 6], 3),
            Err(MusicError::Unsupported(
                "WavPack entropy requires mono or stereo"
            ))
        ));
        let stereo = parse_entropy_variables(
            &[
                0x4e, 0x07, 0xf4, 0x06, 0xd8, 0x08, 0x4e, 0x07, 0xf4, 0x06, 0xd8, 0x08,
            ],
            2,
        )
        .unwrap();
        assert_eq!(stereo.medians, [[79, 62, 229], [79, 62, 229]]);
    }

    #[test]
    fn entropy_medians_outside_integer_range_are_rejected() {
        assert!(matches!(
            parse_entropy_variables(&[0, 0x20, 0, 0, 0, 0], 1),
            Err(MusicError::Limit(
                "WavPack entropy median exceeds 32-bit decoder range"
            ))
        ));
    }

    #[test]
    fn bit_reader_is_lsb_first_and_handles_width_boundaries() {
        let mut bits = BitReader::new(&[0b1001_0110, 0b0000_0001]);
        assert_eq!(bits.read_bits(4).unwrap(), 0b0110);
        assert_eq!(bits.read_bits(4).unwrap(), 0b1001);
        assert_eq!(bits.read_bits(1).unwrap(), 1);
        assert_eq!(bits.read_bits(7).unwrap(), 0);
        assert_eq!(bits.remaining_bits(), 0);
        assert!(bits.read_bit().is_err());
        assert_eq!(BitReader::new(&[0xff; 4]).read_bits(32).unwrap(), u32::MAX);
    }

    #[test]
    fn bit_reader_rejects_oversized_and_truncated_reads_without_consuming() {
        let mut bits = BitReader::new(&[0b1010_0101]);
        assert!(matches!(
            bits.read_bits(33),
            Err(MusicError::Limit(
                "WavPack bit read exceeds 32-bit result width"
            ))
        ));
        assert_eq!(bits.remaining_bits(), 8);
        assert!(bits.read_bits(9).is_err());
        assert_eq!(bits.remaining_bits(), 8);
        assert_eq!(bits.read_bits(8).unwrap(), 0b1010_0101);
    }

    #[test]
    fn unary_codes_are_terminated_and_bounded() {
        let mut valid = BitReader::new(&[0b0000_0101]);
        assert_eq!(valid.read_unary_ones(3).unwrap(), 1);
        let mut overlong = BitReader::new(&[0b0000_0111]);
        assert!(matches!(
            overlong.read_unary_ones(2),
            Err(MusicError::Limit(
                "WavPack unary code exceeds configured bound"
            ))
        ));
        let mut unterminated = BitReader::new(&[0xff]);
        assert!(unterminated.read_unary_ones(8).is_err());
    }

    #[test]
    fn reads_complete_bounded_blocks_and_rejects_truncation() {
        let bytes = block(1, FLAG_INITIAL_BLOCK | FLAG_FINAL_BLOCK);
        let (header, complete) = read_block(&mut Cursor::new(bytes.clone()))
            .unwrap()
            .unwrap();
        assert_eq!(header.block_bytes, complete.len());
        assert!(read_block(&mut Cursor::new(bytes[..20].to_vec())).is_err());
    }
}
