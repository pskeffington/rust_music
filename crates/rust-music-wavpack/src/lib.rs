//! Bounded WavPack v4/v5 block and metadata parsing.
//!
//! This crate is not yet a decoder and is intentionally not registered by the
//! public facade. It provides the checked framing layer for the RM-014 decoder.

use rust_music_core::{MusicError, Result};
use std::io::{self, Read};

pub const MAX_BLOCK_BYTES: usize = 1024 * 1024;
pub const MAX_BLOCK_SAMPLES: u32 = 131_072;
const HEADER_BYTES: usize = 32;

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
        // The upper index and length bytes are defined for the WavPack 5
        // stream format (0x407 and later). Earlier versions reserved them.
        let (total_samples, block_index) = if version >= 0x407 {
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
    fn v4_uses_32_bit_lengths_and_indices_only() {
        let mut bytes = block(128, FLAG_INITIAL_BLOCK | FLAG_FINAL_BLOCK);
        bytes[8..10].copy_from_slice(&0x406u16.to_le_bytes());
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
    }

    #[test]
    fn v5_uses_40_bit_fields_and_only_first_block_has_total_length() {
        let mut bytes = block(128, FLAG_INITIAL_BLOCK | FLAG_FINAL_BLOCK);
        bytes[8..10].copy_from_slice(&0x407u16.to_le_bytes());
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
    fn reads_complete_bounded_blocks_and_rejects_truncation() {
        let bytes = block(1, FLAG_INITIAL_BLOCK | FLAG_FINAL_BLOCK);
        let (header, complete) = read_block(&mut Cursor::new(bytes.clone()))
            .unwrap()
            .unwrap();
        assert_eq!(header.block_bytes, complete.len());
        assert!(read_block(&mut Cursor::new(bytes[..20].to_vec())).is_err());
    }
}
