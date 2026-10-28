//! Minimal RIFF/WAVE native integer-PCM decoder with bounded chunk scanning.
//! The module does not use system multimedia codecs or third-party crates.

use rust_music_core::{AudioFormat, Decoder, MusicError, PcmBlock, Result};
use std::io::Read;

const BLOCK_FRAMES: usize = 4096;
const MAX_CHUNKS: usize = 1024;
const MAX_SKIP: u32 = 32 * 1024 * 1024;

fn skip<R: Read>(reader: &mut R, mut len: u32) -> Result<()> {
    let mut scratch = [0u8; 4096];
    while len > 0 {
        let take = (len as usize).min(scratch.len());
        reader.read_exact(&mut scratch[..take])?;
        len -= take as u32;
    }
    Ok(())
}
fn u16le(bytes: &[u8]) -> u16 { u16::from_le_bytes([bytes[0], bytes[1]]) }
fn u32le(bytes: &[u8]) -> u32 { u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) }

/// WAV decoder reads in small PCM blocks instead of holding the file in memory.
pub struct WaveDecoder<R: Read + Send> {
    input: R,
    format: AudioFormat,
    remaining: u64,
}

impl<R: Read + Send> WaveDecoder<R> {
    pub fn new(mut input: R) -> Result<Self> {
        let mut riff = [0; 12];
        input.read_exact(&mut riff)?;
        if &riff[..4] != b"RIFF" || &riff[8..12] != b"WAVE" {
            return Err(MusicError::Invalid("expected RIFF WAVE file"));
        }
        let mut fmt = None;
        let mut remaining = None;
        for _ in 0..MAX_CHUNKS {
            let mut head = [0u8; 8];
            input.read_exact(&mut head)?;
            let chunk_size = u32le(&head[4..8]);
            if &head[..4] == b"fmt " {
                    if fmt.is_some() { return Err(MusicError::Invalid("duplicate WAVE fmt chunk")); }
                    if !(16..=1024).contains(&chunk_size) { return Err(MusicError::Invalid("unsupported WAVE fmt length")); }
                    let mut bytes = vec![0u8; chunk_size as usize];
                    input.read_exact(&mut bytes)?;
                    if u16le(&bytes[0..2]) != 1 { return Err(MusicError::Unsupported("only uncompressed PCM WAVE is supported")); }
                    let channels = u16le(&bytes[2..4]);
                    let rate = u32le(&bytes[4..8]);
                    let byte_rate = u32le(&bytes[8..12]);
                    let block_align = u16le(&bytes[12..14]);
                    let bits = u16le(&bytes[14..16]);
                    if !matches!(bits, 8 | 16 | 24 | 32) || channels > 8 {
                        return Err(MusicError::Unsupported("WAVE must be 8/16/24/32-bit PCM with at most 8 channels"));
                    }
                    let format = AudioFormat::new(rate, channels as u8, bits as u8)?;
                    let actual_align = u32::from(channels) * u32::from(bits / 8);
                    if u32::from(block_align) != actual_align || byte_rate != rate.saturating_mul(actual_align) {
                        return Err(MusicError::Invalid("WAVE byte rate or frame alignment mismatch"));
                    }
                    fmt = Some(format);
            } else if &head[..4] == b"data" {
                    if fmt.is_none() { return Err(MusicError::Invalid("WAVE data before fmt is unsupported")); }
                    remaining = Some(u64::from(chunk_size));
                    break;
            } else {
                    if chunk_size > MAX_SKIP { return Err(MusicError::Limit("WAVE metadata chunk too large")); }
                    skip(&mut input, chunk_size)?;
            }
            if chunk_size % 2 == 1 { skip(&mut input, 1)?; }
        }
        let format = fmt.ok_or(MusicError::Invalid("WAVE fmt missing"))?;
        let remaining = remaining.ok_or(MusicError::Invalid("WAVE data missing"))?;
        let bytes_per_frame = u64::from(format.channels) * u64::from(format.bits_per_sample / 8);
        if remaining % bytes_per_frame != 0 { return Err(MusicError::Invalid("WAVE data size not frame aligned")); }
        Ok(Self { input, format, remaining })
    }
    pub fn next_pcm_block(&mut self) -> Result<Option<PcmBlock>> {
        if self.remaining == 0 { return Ok(None); }
        let sample_bytes = usize::from(self.format.bits_per_sample / 8);
        let frame_bytes = sample_bytes * usize::from(self.format.channels);
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
                    let raw = (u32::from(sample[0]))
                        | (u32::from(sample[1]) << 8) | (u32::from(sample[2]) << 16);
                    ((raw << 8) as i32) >> 8
                }
                4 => i32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]),
                _ => return Err(MusicError::Unsupported("unsupported WAVE sample byte width")),
            };
            samples.push(value);
        }
        Ok(Some(PcmBlock::new(self.format, samples)?))
    }
}
impl<R: Read + Send> Decoder for WaveDecoder<R> {
    fn format(&self) -> AudioFormat { self.format }
    fn next_block(&mut self) -> Result<Option<PcmBlock>> { self.next_pcm_block() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    fn wav(bits: u16, data: &[u8]) -> Vec<u8> {
        let mut b = b"RIFF".to_vec();
        b.extend_from_slice(&(36u32 + data.len() as u32).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&44_100u32.to_le_bytes());
        b.extend_from_slice(&(44_100u32 * 2 * u32::from(bits / 8)).to_le_bytes());
        b.extend_from_slice(&(2u16 * (bits / 8)).to_le_bytes());
        b.extend_from_slice(&bits.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(data.len() as u32).to_le_bytes());
        b.extend_from_slice(data);
        b
    }
    #[test]
    fn decodes_signed_24_bit() {
        let bytes = wav(24, &[0xff, 0xff, 0x7f, 0, 0, 0x80]);
        let mut d = WaveDecoder::new(Cursor::new(bytes)).unwrap();
        assert_eq!(d.next_pcm_block().unwrap().unwrap().samples, vec![8_388_607, -8_388_608]);
        assert_eq!(d.next_pcm_block().unwrap(), None);
    }
    #[test]
    fn rejects_invalid_frame_alignment() {
        assert!(WaveDecoder::new(Cursor::new(wav(16, &[1, 2, 3]))).is_err());
    }
    #[test]
    fn decodes_unsigned_8_bit_to_centered_pcm() {
        let mut d = WaveDecoder::new(Cursor::new(wav(8, &[0, 128, 255, 128]))).unwrap();
        assert_eq!(d.next_pcm_block().unwrap().unwrap().samples, vec![-128, 0, 127, 0]);
    }
}
