//! Bounded, independently authored RIFF/WAVE integer PCM reader.
//!
//! Supports WAVE_FORMAT_PCM and PCM WAVE_FORMAT_EXTENSIBLE, including
//! left-aligned 24 valid bits in a 32-bit container. It never resamples,
//! truncates decoded precision, loads a whole file, or follows a network URL.
//! Unsupported compressed, IEEE-float, RF64 and RIFX variants fail closed.

use rust_music_core::{AudioFormat, Decoder, MusicError, PcmBlock, Result};
use std::io::Read;

const BLOCK_FRAMES: usize = 4096;
const MAX_CHUNKS: usize = 1024;
const MAX_SKIP: u64 = 32 * 1024 * 1024;
const PCM_SUBFORMAT_GUID: [u8; 16] = [
    1, 0, 0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xaa, 0, 0x38, 0x9b, 0x71,
];

fn u16le(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}
fn u32le(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn skip<R: Read>(reader: &mut R, mut len: u64) -> Result<()> {
    let mut scratch = [0u8; 4096];
    while len > 0 {
        let count = (len as usize).min(scratch.len());
        reader.read_exact(&mut scratch[..count])?;
        len -= count as u64;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct WaveFormat {
    pcm: AudioFormat,
    bytes_per_sample: usize,
    right_shift: u8,
}

fn parse_format(data: &[u8]) -> Result<WaveFormat> {
    if data.len() < 16 {
        return Err(MusicError::Invalid("WAVE fmt chunk too short"));
    }
    let code = u16le(&data[0..2]);
    let channels = u16le(&data[2..4]);
    let rate = u32le(&data[4..8]);
    let byte_rate = u32le(&data[8..12]);
    let block_align = u16le(&data[12..14]);
    let container_bits = u16le(&data[14..16]);
    if !matches!(container_bits, 8 | 16 | 24 | 32) {
        return Err(MusicError::Unsupported(
            "only packed 8/16/24/32-bit WAVE containers are supported",
        ));
    }
    if !(1..=8).contains(&channels) {
        return Err(MusicError::Unsupported("WAVE supports 1..8 channels"));
    }
    let valid_bits = match code {
        1 => {
            if data.len() != 16
                && (data.len() < 18 || usize::from(u16le(&data[16..18])) > data.len() - 18)
            {
                return Err(MusicError::Invalid(
                    "WAVE PCM extra format bytes are inconsistent",
                ));
            }
            container_bits
        }
        0xfffe => {
            if data.len() < 40
                || u16le(&data[16..18]) < 22
                || usize::from(u16le(&data[16..18])) > data.len() - 18
            {
                return Err(MusicError::Invalid(
                    "WAVE_FORMAT_EXTENSIBLE format size is invalid",
                ));
            }
            if &data[24..40] != PCM_SUBFORMAT_GUID.as_slice() {
                return Err(MusicError::Unsupported(
                    "WAVE_FORMAT_EXTENSIBLE subtype is not integer PCM",
                ));
            }
            let mask = u32le(&data[20..24]);
            if mask != 0 && mask.count_ones() != u32::from(channels) {
                return Err(MusicError::Invalid(
                    "WAVE speaker mask disagrees with channel count",
                ));
            }
            u16le(&data[18..20])
        }
        _ => {
            return Err(MusicError::Unsupported(
                "only integer PCM WAVE is supported",
            ))
        }
    };
    if !matches!(valid_bits, 8 | 16 | 24 | 32) || valid_bits > container_bits {
        return Err(MusicError::Unsupported(
            "valid PCM bits must be 8/16/24/32 and fit their container",
        ));
    }
    let actual_align = u32::from(channels) * u32::from(container_bits / 8);
    if u32::from(block_align) != actual_align || rate.checked_mul(actual_align) != Some(byte_rate) {
        return Err(MusicError::Invalid(
            "WAVE frame alignment or byte rate mismatch",
        ));
    }
    Ok(WaveFormat {
        pcm: AudioFormat::new(rate, channels as u8, valid_bits as u8)?,
        bytes_per_sample: usize::from(container_bits / 8),
        right_shift: (container_bits - valid_bits) as u8,
    })
}

/// Streaming native WAV reader, with strict declared RIFF/chunk boundaries.
/// A chunk claiming bytes beyond RIFF fails before any PCM is returned.
/// The final odd-data pad byte is consumed at EOF so truncated files fail.
pub struct WaveDecoder<R: Read + Send> {
    input: R,
    format: WaveFormat,
    remaining: u64,
    total_frames: u64,
    pad_after_data: bool,
}

impl<R: Read + Send> WaveDecoder<R> {
    pub fn new(mut input: R) -> Result<Self> {
        let mut riff = [0u8; 12];
        input.read_exact(&mut riff)?;
        if &riff[..4] != b"RIFF" || &riff[8..12] != b"WAVE" {
            return Err(MusicError::Invalid("expected RIFF WAVE file"));
        }
        let riff_size = u64::from(u32le(&riff[4..8]));
        if riff_size < 4 {
            return Err(MusicError::Invalid(
                "RIFF container smaller than WAVE marker",
            ));
        }
        // RIFF size includes WAVE marker, all chunk headers, data and padding.
        let mut budget = riff_size - 4;
        let mut fmt = None;
        for _ in 0..MAX_CHUNKS {
            if budget == 0 {
                break;
            }
            if budget < 8 {
                return Err(MusicError::Invalid("trailing partial WAVE chunk header"));
            }
            let mut header = [0u8; 8];
            input.read_exact(&mut header)?;
            budget -= 8;
            let length = u64::from(u32le(&header[4..8]));
            let padded = length + (length & 1);
            if padded > budget {
                return Err(MusicError::Invalid("WAVE chunk extends past RIFF boundary"));
            }
            if &header[..4] == b"fmt " {
                if fmt.is_some() {
                    return Err(MusicError::Invalid("duplicate WAVE fmt chunk"));
                }
                if !(16..=1024).contains(&length) {
                    return Err(MusicError::Invalid("unsupported WAVE fmt length"));
                }
                let mut bytes = vec![0u8; length as usize];
                input.read_exact(&mut bytes)?;
                fmt = Some(parse_format(&bytes)?);
            } else if &header[..4] == b"data" {
                let format =
                    fmt.ok_or(MusicError::Invalid("WAVE data before fmt is unsupported"))?;
                let frame_bytes = format.bytes_per_sample * usize::from(format.pcm.channels);
                if length % frame_bytes as u64 != 0 {
                    return Err(MusicError::Invalid("WAVE data size is not frame aligned"));
                }
                return Ok(Self {
                    input,
                    format,
                    remaining: length,
                    total_frames: length / frame_bytes as u64,
                    pad_after_data: length & 1 != 0,
                });
            } else {
                if length > MAX_SKIP {
                    return Err(MusicError::Limit("WAVE metadata chunk exceeds skip budget"));
                }
                skip(&mut input, length)?;
            }
            if length & 1 != 0 {
                skip(&mut input, 1)?;
            }
            budget -= padded;
        }
        Err(MusicError::Invalid(
            "WAVE fmt or data chunk missing within bounded RIFF",
        ))
    }

    pub fn next_pcm_block(&mut self) -> Result<Option<PcmBlock>> {
        if self.remaining == 0 {
            if self.pad_after_data {
                skip(&mut self.input, 1)?;
                self.pad_after_data = false;
            }
            return Ok(None);
        }
        let sample_bytes = self.format.bytes_per_sample;
        let frame_bytes = sample_bytes * usize::from(self.format.pcm.channels);
        let frames = (self.remaining / frame_bytes as u64).min(BLOCK_FRAMES as u64) as usize;
        let bytes = frames * frame_bytes;
        let mut raw = vec![0u8; bytes];
        self.input.read_exact(&mut raw)?;
        self.remaining -= bytes as u64;
        let mut samples = Vec::with_capacity(bytes / sample_bytes);
        for sample in raw.chunks_exact(sample_bytes) {
            let value = match sample_bytes {
                1 => i32::from(sample[0]) - 128,
                2 => i32::from(i16::from_le_bytes([sample[0], sample[1]])),
                3 => {
                    let packed = u32::from(sample[0])
                        | (u32::from(sample[1]) << 8)
                        | (u32::from(sample[2]) << 16);
                    ((packed << 8) as i32) >> 8
                }
                4 => i32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]),
                _ => return Err(MusicError::Unsupported("unsupported WAVE sample width")),
            };
            // WAVE extensible pads the *least* significant container bits.
            // Arithmetic right shift returns the original signed integer.
            if self.format.right_shift > 0 {
                let mask = (1i32 << self.format.right_shift) - 1;
                if value & mask != 0 {
                    return Err(MusicError::Invalid(
                        "nonzero WAVE low padding bits would lose PCM precision",
                    ));
                }
            }
            samples.push(value >> self.format.right_shift);
        }
        Ok(Some(PcmBlock::new(self.format.pcm, samples)?))
    }
}

impl<R: Read + Send> Decoder for WaveDecoder<R> {
    fn format(&self) -> AudioFormat {
        self.format.pcm
    }
    fn total_frames(&self) -> Option<u64> {
        Some(self.total_frames)
    }
    fn next_block(&mut self) -> Result<Option<PcmBlock>> {
        self.next_pcm_block()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn wave(
        bits: u16,
        channels: u16,
        samples: &[u8],
        extensible: bool,
        valid_bits: u16,
    ) -> Vec<u8> {
        let frame_bytes = channels * (bits / 8);
        let mut fmt = Vec::new();
        fmt.extend_from_slice(&(if extensible { 0xfffeu16 } else { 1u16 }).to_le_bytes());
        fmt.extend_from_slice(&channels.to_le_bytes());
        fmt.extend_from_slice(&44_100u32.to_le_bytes());
        fmt.extend_from_slice(&(44_100u32 * u32::from(frame_bytes)).to_le_bytes());
        fmt.extend_from_slice(&frame_bytes.to_le_bytes());
        fmt.extend_from_slice(&bits.to_le_bytes());
        if extensible {
            fmt.extend_from_slice(&22u16.to_le_bytes());
            fmt.extend_from_slice(&valid_bits.to_le_bytes());
            fmt.extend_from_slice(&0u32.to_le_bytes());
            fmt.extend_from_slice(&PCM_SUBFORMAT_GUID);
        }
        let mut body = b"WAVEfmt ".to_vec();
        body.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
        body.extend_from_slice(&fmt);
        body.extend_from_slice(b"data");
        body.extend_from_slice(&(samples.len() as u32).to_le_bytes());
        body.extend_from_slice(samples);
        if samples.len() & 1 != 0 {
            body.push(0);
        }
        let mut wav = b"RIFF".to_vec();
        wav.extend_from_slice(&(body.len() as u32).to_le_bytes());
        wav.extend_from_slice(&body);
        wav
    }

    #[test]
    fn signed_24_and_unsigned_8() {
        let source = wave(24, 2, &[0xff, 0xff, 0x7f, 0, 0, 0x80], false, 24);
        let mut decoder = WaveDecoder::new(Cursor::new(source)).unwrap();
        assert_eq!(decoder.total_frames(), Some(1));
        assert_eq!(
            decoder.next_pcm_block().unwrap().unwrap().samples,
            vec![8_388_607, -8_388_608]
        );
        assert_eq!(decoder.next_pcm_block().unwrap(), None);
        let source = wave(8, 1, &[0, 128, 255], false, 8);
        let mut decoder = WaveDecoder::new(Cursor::new(source)).unwrap();
        assert_eq!(decoder.total_frames(), Some(3));
        assert_eq!(
            decoder.next_pcm_block().unwrap().unwrap().samples,
            vec![-128, 0, 127]
        );
        assert_eq!(decoder.next_pcm_block().unwrap(), None);
    }

    #[test]
    fn extensible_24_in_32_preserves_signed_samples() {
        let raw = [0, 0xff, 0xff, 0x7f, 0, 0, 0, 0x80];
        let wav = wave(32, 2, &raw, true, 24);
        let mut decoder = WaveDecoder::new(Cursor::new(wav)).unwrap();
        assert_eq!(decoder.format(), AudioFormat::new(44_100, 2, 24).unwrap());
        assert_eq!(
            decoder.next_pcm_block().unwrap().unwrap().samples,
            vec![8_388_607, -8_388_608]
        );
        assert_eq!(decoder.next_pcm_block().unwrap(), None);
    }

    #[test]
    fn rejects_nonzero_padding_bits_in_extensible_pcm() {
        let raw = [1, 0xff, 0xff, 0x7f, 0, 0, 0, 0x80];
        let wav = wave(32, 2, &raw, true, 24);
        let mut decoder = WaveDecoder::new(Cursor::new(wav)).unwrap();
        assert!(decoder.next_pcm_block().is_err());
    }

    #[test]
    fn multichannel_extensible_preserves_speaker_masked_pcm() {
        let mut wav = wave(16, 6, &[1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0], true, 16);
        wav[40..44].copy_from_slice(&0x3fu32.to_le_bytes());
        let mut decoder = WaveDecoder::new(Cursor::new(wav)).unwrap();
        assert_eq!(
            decoder.next_pcm_block().unwrap().unwrap().samples,
            vec![1, 2, 3, 4, 5, 6]
        );
    }

    #[test]
    fn rejects_data_past_declared_riff_boundary() {
        let mut wav = wave(16, 2, &[0, 0, 0, 0], false, 16);
        wav[4..8].copy_from_slice(&37u32.to_le_bytes());
        assert!(matches!(
            WaveDecoder::new(Cursor::new(wav)),
            Err(MusicError::Invalid("WAVE chunk extends past RIFF boundary"))
        ));
    }

    #[test]
    fn rejects_non_pcm_extensible_guid_and_misreported_valid_bits() {
        let mut wav = wave(32, 2, &[0; 8], true, 24);
        wav[44] = 3; // WAVE_FORMAT_EXTENSIBLE subtype: IEEE float, not PCM.
        assert!(WaveDecoder::new(Cursor::new(wav)).is_err());
        let wav = wave(16, 2, &[0; 4], true, 24);
        assert!(WaveDecoder::new(Cursor::new(wav)).is_err());
    }

    #[test]
    fn odd_data_pad_missing_fails_at_eof() {
        let mut wav = wave(8, 1, &[128], false, 8);
        wav.pop(); // RIFF declares one pad byte, but it is missing on disk.
        let mut decoder = WaveDecoder::new(Cursor::new(wav)).unwrap();
        assert_eq!(decoder.next_pcm_block().unwrap().unwrap().samples, vec![0]);
        assert!(decoder.next_pcm_block().is_err());
    }

    #[test]
    fn ignores_bounded_odd_metadata_chunk_and_tracks_padding() {
        let original = wave(16, 1, &[1, 2], false, 16);
        let mut wav = original[..36].to_vec(); // RIFF + WAVE + fmt chunk.
        wav.extend_from_slice(b"JUNK");
        wav.extend_from_slice(&3u32.to_le_bytes());
        wav.extend_from_slice(&[1, 2, 3, 0]);
        wav.extend_from_slice(&original[36..]);
        let len = (wav.len() - 8) as u32;
        wav[4..8].copy_from_slice(&len.to_le_bytes());
        let mut decoder = WaveDecoder::new(Cursor::new(wav)).unwrap();
        assert_eq!(
            decoder.next_pcm_block().unwrap().unwrap().samples,
            vec![513]
        );
    }
}
