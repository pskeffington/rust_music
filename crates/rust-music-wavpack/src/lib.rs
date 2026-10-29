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

/// Inclusive residual interval selected by one WavPack entropy partition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResidualRange {
    pub low: u32,
    pub high: u32,
}

/// Adaptive Q4 median breakpoints for one encoded channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MedianChannel {
    values: [u32; 3],
}

impl MedianChannel {
    pub fn new(values: [u32; 3]) -> Self {
        Self { values }
    }

    /// Select the recursive Golomb interval for an already decoded unary
    /// partition count, then update the three breakpoints for that partition.
    pub fn select_range(&mut self, ones_count: u32) -> Result<ResidualRange> {
        let original = *self;
        match self.select_range_inner(ones_count) {
            Ok(range) => Ok(range),
            Err(error) => {
                *self = original;
                Err(error)
            }
        }
    }

    /// Decode one signed residual after the unary partition count has been
    /// resolved by the caller. Median and bit cursors commit only on success.
    pub fn decode_residual(&mut self, bits: &mut BitReader<'_>, ones_count: u32) -> Result<i32> {
        let original_medians = *self;
        let original_bit_offset = bits.bit_offset;
        let decoded = (|| {
            let range = self.select_range(ones_count)?;
            let code_count = range.high - range.low + 1;
            let residual = range.low + bits.read_adjusted_binary(code_count)?;
            let sign = bits.read_bit()?;
            let magnitude = residual as i32;
            Ok(if sign { !magnitude } else { magnitude })
        })();
        if decoded.is_err() {
            *self = original_medians;
            bits.bit_offset = original_bit_offset;
        }
        decoded
    }

    fn select_range_inner(&mut self, ones_count: u32) -> Result<ResidualRange> {
        let mut low = 0u64;
        let high;
        if ones_count == 0 {
            high = u64::from(self.get(0)) - 1;
            self.decrease(0)?;
        } else {
            low = u64::from(self.get(0));
            self.increase(0)?;
            if ones_count == 1 {
                high = low + u64::from(self.get(1)) - 1;
                self.decrease(1)?;
            } else {
                low += u64::from(self.get(1));
                self.increase(1)?;
                if ones_count == 2 {
                    high = low + u64::from(self.get(2)) - 1;
                    self.decrease(2)?;
                } else {
                    low += u64::from(ones_count - 2) * u64::from(self.get(2));
                    high = low + u64::from(self.get(2)) - 1;
                    self.increase(2)?;
                }
            }
        }

        const SAMPLE_MAGNITUDE_MASK: u64 = 0x7fff_ffff;
        let low = low & SAMPLE_MAGNITUDE_MASK;
        let mut high = high & SAMPLE_MAGNITUDE_MASK;
        if low > high {
            high = low;
        }
        Ok(ResidualRange {
            low: low as u32,
            high: high as u32,
        })
    }

    pub fn fixed_point_medians(&self) -> [u32; 3] {
        self.values
    }

    fn get(&self, index: usize) -> u32 {
        (self.values[index] >> 4) + 1
    }

    fn increase(&mut self, index: usize) -> Result<()> {
        const DIVISORS: [u64; 3] = [128, 64, 32];
        let value = u64::from(self.values[index]);
        let divisor = DIVISORS[index];
        let step = ((value + divisor) / divisor) * 5;
        self.values[index] = u32::try_from(value + step)
            .map_err(|_| MusicError::Limit("WavPack entropy median update overflows"))?;
        Ok(())
    }

    fn decrease(&mut self, index: usize) -> Result<()> {
        const DIVISORS: [u64; 3] = [128, 64, 32];
        let value = u64::from(self.values[index]);
        let divisor = DIVISORS[index];
        let step = ((value + divisor - 2) / divisor) * 2;
        self.values[index] = u32::try_from(value.checked_sub(step).ok_or(MusicError::Invalid(
            "WavPack entropy median update underflows",
        ))?)
        .map_err(|_| MusicError::Limit("WavPack entropy median update overflows"))?;
        Ok(())
    }
}

/// Stateful entropy word decoder for a WavPack mono/stereo block.
/// De-correlation and block CRC validation are separate decoder stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WordDecoder {
    channels: u8,
    medians: [MedianChannel; 2],
    holding_one: bool,
    holding_zero: bool,
    zeros_remaining: u32,
    decoded_words: u32,
}

impl WordDecoder {
    pub fn new(entropy: EntropyVariables) -> Result<Self> {
        if !(1..=2).contains(&entropy.channels) {
            return Err(MusicError::Unsupported(
                "WavPack word decoder requires mono or stereo",
            ));
        }
        Ok(Self {
            channels: entropy.channels,
            medians: [
                MedianChannel::new(entropy.medians[0]),
                MedianChannel::new(entropy.medians[1]),
            ],
            holding_one: false,
            holding_zero: false,
            zeros_remaining: 0,
            decoded_words: 0,
        })
    }

    /// Decode exactly `frame_count` interleaved residual frames, bounded by
    /// the block-header sample limit. This stage does not reconstruct PCM.
    pub fn decode_frames(
        &mut self,
        bits: &mut BitReader<'_>,
        frame_count: u32,
    ) -> Result<Vec<i32>> {
        if frame_count > MAX_BLOCK_SAMPLES {
            return Err(MusicError::Limit(
                "WavPack word frame count exceeds block limit",
            ));
        }
        let word_count = usize::try_from(frame_count)
            .ok()
            .and_then(|frames| frames.checked_mul(usize::from(self.channels)))
            .ok_or(MusicError::Limit("WavPack word count overflows"))?;
        let original = *self;
        let original_bit_offset = bits.bit_offset;
        let mut words = Vec::with_capacity(word_count);
        for _ in 0..word_count {
            match self.decode_word(bits) {
                Ok(word) => words.push(word),
                Err(error) => {
                    *self = original;
                    bits.bit_offset = original_bit_offset;
                    return Err(error);
                }
            }
        }
        Ok(words)
    }

    pub fn decoded_words(&self) -> u32 {
        self.decoded_words
    }

    fn decode_word(&mut self, bits: &mut BitReader<'_>) -> Result<i32> {
        let original = *self;
        let original_bit_offset = bits.bit_offset;
        let decoded = self.decode_word_inner(bits);
        match decoded {
            Ok(word) => {
                if let Some(next) = self.decoded_words.checked_add(1) {
                    self.decoded_words = next;
                    Ok(word)
                } else {
                    *self = original;
                    bits.bit_offset = original_bit_offset;
                    Err(MusicError::Limit("WavPack decoded word count overflows"))
                }
            }
            Err(error) => {
                *self = original;
                bits.bit_offset = original_bit_offset;
                Err(error)
            }
        }
    }

    fn decode_word_inner(&mut self, bits: &mut BitReader<'_>) -> Result<i32> {
        let channel = (self.decoded_words % u32::from(self.channels)) as usize;
        if !self.holding_zero
            && !self.holding_one
            && self
                .medians
                .iter()
                .take(usize::from(self.channels))
                .all(|median| median.values[0] & !1 == 0)
        {
            if self.zeros_remaining != 0 {
                self.zeros_remaining -= 1;
                if self.zeros_remaining != 0 {
                    return Ok(0);
                }
            } else {
                let run_length = read_zero_run(bits)?;
                if run_length != 0 {
                    self.zeros_remaining = run_length;
                    self.medians = [MedianChannel::new([0; 3]); 2];
                    return Ok(0);
                }
            }
        }

        let ones_count = if self.holding_zero {
            self.holding_zero = false;
            0
        } else {
            let encoded_count = read_encoded_ones_count(bits)?;
            let ones_count = if self.holding_one {
                self.holding_one = encoded_count & 1 != 0;
                (encoded_count >> 1) + 1
            } else {
                self.holding_one = encoded_count & 1 != 0;
                encoded_count >> 1
            };
            self.holding_zero = !self.holding_one;
            ones_count
        };

        self.medians[channel].decode_residual(bits, ones_count)
    }
}

fn read_zero_run(bits: &mut BitReader<'_>) -> Result<u32> {
    let prefix = bits.read_unary_ones(32)?;
    if prefix < 2 {
        Ok(prefix)
    } else {
        let width = (prefix - 1) as u8;
        let suffix = bits.read_bits(width)?;
        Ok((1u32 << width) | suffix)
    }
}

fn read_encoded_ones_count(bits: &mut BitReader<'_>) -> Result<u32> {
    const MAX_UNARY_PREFIX: u32 = 16;
    let prefix = bits.read_unary_ones(MAX_UNARY_PREFIX)?;
    if prefix < MAX_UNARY_PREFIX {
        Ok(prefix)
    } else {
        let escape = bits.read_unary_ones(32)?;
        let count = if escape < 2 {
            escape
        } else {
            let width = (escape - 1) as u8;
            (1u32 << width) | bits.read_bits(width)?
        };
        count
            .checked_add(MAX_UNARY_PREFIX)
            .ok_or(MusicError::Limit("WavPack escaped unary count overflows"))
    }
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

    /// Read one adjusted-binary value in `0..range` from the next stream bits.
    /// The WavPack codewords use a truncated binary prefix followed by one
    /// extra bit for values above the short-code cutoff.
    pub fn read_adjusted_binary(&mut self, range: u32) -> Result<u32> {
        if range == 0 {
            return Err(MusicError::Invalid(
                "WavPack adjusted-binary range must be nonzero",
            ));
        }
        if range == 1 {
            return Ok(0);
        }

        let checkpoint = self.bit_offset;
        let decoded = (|| {
            let width = (u32::BITS - 1 - range.leading_zeros()) as u8;
            let short_count = ((1u64 << (width + 1)) - u64::from(range)) as u32;
            let mut prefix = 0u32;
            for _ in 0..width {
                prefix = (prefix << 1) | u32::from(self.read_bit()?);
            }
            if prefix < short_count {
                Ok(prefix)
            } else {
                let suffix = u32::from(self.read_bit()?);
                Ok((prefix - short_count) * 2 + short_count + suffix)
            }
        })();
        if decoded.is_err() {
            self.bit_offset = checkpoint;
        }
        decoded
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

    fn stream_bits(bits: &str) -> Vec<u8> {
        let mut bytes = vec![0; bits.len().div_ceil(8)];
        for (index, bit) in bits.bytes().enumerate() {
            assert!(matches!(bit, b'0' | b'1'));
            if bit == b'1' {
                bytes[index / 8] |= 1 << (index % 8);
            }
        }
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
        let mut decoder = WordDecoder::new(entropy).unwrap();
        let mut bits = BitReader::new(metadata[4].data);
        let residuals = decoder
            .decode_frames(&mut bits, header.block_samples)
            .unwrap();
        assert_eq!(residuals.len(), header.block_samples as usize);
        assert_eq!(decoder.decoded_words(), header.block_samples);
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
    fn median_channel_selects_and_adapts_recursive_zones() {
        let mut medians = MedianChannel::new([79, 62, 229]);
        assert_eq!(
            medians.select_range(0).unwrap(),
            ResidualRange { low: 0, high: 4 }
        );
        assert_eq!(medians.fixed_point_medians(), [77, 62, 229]);
        assert_eq!(
            medians.select_range(1).unwrap(),
            ResidualRange { low: 5, high: 8 }
        );
        assert_eq!(medians.fixed_point_medians(), [82, 60, 229]);
        assert_eq!(
            medians.select_range(2).unwrap(),
            ResidualRange { low: 10, high: 24 }
        );
        assert_eq!(medians.fixed_point_medians(), [87, 65, 213]);
        assert_eq!(
            medians.select_range(3).unwrap(),
            ResidualRange { low: 25, high: 38 }
        );
        assert_eq!(medians.fixed_point_medians(), [92, 75, 248]);
    }

    #[test]
    fn median_channel_decodes_signed_residual_and_rolls_back_on_truncation() {
        let mut positive = MedianChannel::new([79, 62, 229]);
        let positive_bits = stream_bits("000");
        let mut bits = BitReader::new(&positive_bits);
        assert_eq!(positive.decode_residual(&mut bits, 0).unwrap(), 0);
        assert_eq!(positive.fixed_point_medians(), [77, 62, 229]);

        let mut negative = MedianChannel::new([79, 62, 229]);
        let negative_bits = stream_bits("001");
        let mut bits = BitReader::new(&negative_bits);
        assert_eq!(negative.decode_residual(&mut bits, 0).unwrap(), -1);

        let mut state = MedianChannel::new([79, 62, 229]);
        let original = state;
        let bytes = stream_bits("00000011");
        let mut bits = BitReader::new(&bytes);
        bits.read_bits(6).unwrap();
        assert!(state.decode_residual(&mut bits, 0).is_err());
        assert_eq!(state, original);
        assert_eq!(bits.remaining_bits(), 2);
    }

    #[test]
    fn median_update_overflow_fails_without_mutating_state() {
        let mut medians = MedianChannel::new([u32::MAX, 62, 229]);
        let original = medians;
        assert!(matches!(
            medians.select_range(1),
            Err(MusicError::Limit("WavPack entropy median update overflows"))
        ));
        assert_eq!(medians, original);
    }

    #[test]
    fn word_decoder_translates_paired_unary_counts_and_decodes_mono_words() {
        let entropy = parse_entropy_variables(&[0x4e, 0x07, 0xf4, 0x06, 0xd8, 0x08], 1).unwrap();
        let mut decoder = WordDecoder::new(entropy).unwrap();
        let encoded = stream_bits("0000000");
        let mut bits = BitReader::new(&encoded);
        assert_eq!(decoder.decode_frames(&mut bits, 2).unwrap(), [0, 0]);
        assert_eq!(decoder.decoded_words(), 2);
        assert!(decoder
            .decode_frames(&mut bits, MAX_BLOCK_SAMPLES + 1)
            .is_err());
    }

    #[test]
    fn word_decoder_interleaves_stereo_channels_with_separate_medians() {
        let entropy = EntropyVariables {
            medians: [[79, 62, 229], [160, 96, 512]],
            channels: 2,
        };
        let mut decoder = WordDecoder::new(entropy).unwrap();
        // Channel 1's first word is 5; subsequent words exercise the shared
        // paired-unary state while the channels adapt different medians.
        let encoded = stream_bits("0000101001100000000");
        let mut bits = BitReader::new(&encoded);
        assert_eq!(decoder.decode_frames(&mut bits, 2).unwrap(), [0, 5, 5, 0]);
    }

    #[test]
    fn word_decoder_frame_failure_rolls_back_prior_words_in_the_call() {
        let entropy = parse_entropy_variables(&[0x4e, 0x07, 0xf4, 0x06, 0xd8, 0x08], 1).unwrap();
        let mut decoder = WordDecoder::new(entropy).unwrap();
        let original = decoder;
        let encoded = stream_bits("11100000");
        let mut bits = BitReader::new(&encoded);
        bits.read_bits(3).unwrap();
        assert!(decoder.decode_frames(&mut bits, 2).is_err());
        assert_eq!(decoder, original);
        assert_eq!(bits.remaining_bits(), 5);
    }

    #[test]
    fn word_decoder_handles_zero_runs_and_resumes_word_stream() {
        let entropy = EntropyVariables {
            medians: [[0; 3]; 2],
            channels: 1,
        };
        let mut decoder = WordDecoder::new(entropy).unwrap();
        let encoded = stream_bits("110100");
        let mut bits = BitReader::new(&encoded);
        assert_eq!(decoder.decode_frames(&mut bits, 3).unwrap(), [0, 0, 0]);
        assert_eq!(decoder.decode_frames(&mut bits, 1).unwrap(), [0]);
    }

    #[test]
    fn word_decoder_escape_prefix_and_truncation_are_bounded_and_atomic() {
        let escaped = format!("{}01101", "1".repeat(16));
        let escaped = stream_bits(&escaped);
        let mut bits = BitReader::new(&escaped);
        assert_eq!(read_encoded_ones_count(&mut bits).unwrap(), 19);

        let entropy = parse_entropy_variables(&[0x4e, 0x07, 0xf4, 0x06, 0xd8, 0x08], 1).unwrap();
        let mut decoder = WordDecoder::new(entropy).unwrap();
        let original = decoder;
        let encoded = stream_bits("00000011");
        let mut bits = BitReader::new(&encoded);
        bits.read_bits(6).unwrap();
        assert!(decoder.decode_frames(&mut bits, 1).is_err());
        assert_eq!(decoder, original);
        assert_eq!(bits.remaining_bits(), 2);
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
    fn adjusted_binary_matches_official_golomb_code_vectors() {
        // Table 7.32 in the WavPack compression-algorithm description.
        let tables: &[(u32, &[&str])] = &[
            (6, &["00", "01", "100", "101", "110", "111"]),
            (7, &["00", "010", "011", "100", "101", "110", "111"]),
            (8, &["000", "001", "010", "011", "100", "101", "110", "111"]),
            (
                9,
                &[
                    "000", "001", "010", "011", "100", "101", "110", "1110", "1111",
                ],
            ),
            (
                10,
                &[
                    "000", "001", "010", "011", "100", "101", "1100", "1101", "1110", "1111",
                ],
            ),
            (
                11,
                &[
                    "000", "001", "010", "011", "100", "1010", "1011", "1100", "1101", "1110",
                    "1111",
                ],
            ),
        ];

        for &(range, codes) in tables {
            for (expected, code) in codes.iter().enumerate() {
                let bytes = stream_bits(code);
                let mut reader = BitReader::new(&bytes);
                assert_eq!(
                    reader.read_adjusted_binary(range).unwrap(),
                    expected as u32,
                    "range {range}, code {code}"
                );
                assert_eq!(reader.remaining_bits(), bytes.len() * 8 - code.len());
            }
        }
        assert_eq!(BitReader::new(&[]).read_adjusted_binary(1).unwrap(), 0);
        assert!(BitReader::new(&[]).read_adjusted_binary(0).is_err());

        let mut widest = BitReader::new(&[0xff; 4]);
        assert_eq!(widest.read_adjusted_binary(u32::MAX).unwrap(), u32::MAX - 1);
    }

    #[test]
    fn adjusted_binary_truncation_does_not_consume_prefix_bits() {
        let bytes = stream_bits("00000011");
        let mut reader = BitReader::new(&bytes);
        reader.read_bits(6).unwrap();
        assert!(reader.read_adjusted_binary(6).is_err());
        assert_eq!(reader.remaining_bits(), 2);
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
