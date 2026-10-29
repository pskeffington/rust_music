//! Own AIFF and uncompressed AIFF-C integer PCM decoder.
//! Strict big-endian IFF bounds, 80-bit sample-rate conversion, bounded metadata,
//! no libc/codec dependency, no float conversion, and no hidden device opens.
//! Compressed AIFF-C (including mu-law/ADPCM) is deliberately unsupported.

use rust_music_core::{AudioFormat, Decoder, MusicError, PcmBlock, Result};
use std::io::Read;

const MAX_CHUNKS: usize = 512;
const MAX_CHUNK_META: u64 = 16 * 1024 * 1024;
const MAX_TOTAL_META: u64 = 64 * 1024 * 1024;
const BLOCK_FRAMES: usize = 4096;
const AIFC_VERSION: [u8; 4] = [0xa2, 0x80, 0x51, 0x40];

fn be16(x: &[u8]) -> u16 {
    u16::from_be_bytes([x[0], x[1]])
}
fn be32(x: &[u8]) -> u32 {
    u32::from_be_bytes([x[0], x[1], x[2], x[3]])
}

fn skip<R: Read>(input: &mut R, mut bytes: u64) -> Result<()> {
    let mut scratch = [0u8; 4096];
    while bytes != 0 {
        let len = scratch.len().min(bytes as usize);
        input.read_exact(&mut scratch[..len])?;
        bytes -= len as u64;
    }
    Ok(())
}

/// AIFF uses the 80-bit extended format. Accept positive, finite rates only
/// when their representation is an exact integer hertz; never round a clock.
fn integer_rate(bytes: &[u8]) -> Result<u32> {
    let sign_exponent = be16(&bytes[..2]);
    let exponent = i32::from(sign_exponent & 0x7fff);
    let mantissa = u64::from_be_bytes(bytes[2..10].try_into().unwrap());
    if sign_exponent & 0x8000 != 0
        || exponent == 0
        || exponent == 0x7fff
        || mantissa & (1u64 << 63) == 0
    {
        return Err(MusicError::Invalid(
            "AIFF sample rate is not positive normalized extended PCM",
        ));
    }
    // Value = mantissa * 2^(exponent - 16383 - 63).
    // Frequencies supported by the internal PCM contract are small integers.
    let shift = 16383 + 63 - exponent;
    if !(0..=63).contains(&shift) {
        return Err(MusicError::Unsupported("AIFF sample rate is out of range"));
    }
    let low_mask = if shift == 0 { 0 } else { (1u64 << shift) - 1 };
    if mantissa & low_mask != 0 {
        return Err(MusicError::Unsupported(
            "fractional AIFF sample rates are unsupported",
        ));
    }
    let rate = mantissa >> shift;
    u32::try_from(rate).map_err(|_| MusicError::Unsupported("AIFF sample rate is out of range"))
}

#[derive(Debug, Clone, Copy)]
enum ByteOrder {
    Big,
    Little,
}

#[derive(Debug, Clone, Copy)]
struct Common {
    format: AudioFormat,
    total_frames: u64,
    frame_bytes: usize,
    order: ByteOrder,
}

fn parse_common(bytes: &[u8], aifc: bool) -> Result<Common> {
    if (!aifc && bytes.len() != 18) || (aifc && bytes.len() < 22) {
        return Err(MusicError::Invalid("invalid AIFF COMM size"));
    }
    let channels = be16(&bytes[..2]);
    let total_frames = u64::from(be32(&bytes[2..6]));
    let bits = be16(&bytes[6..8]);
    // Keep the first gate byte-exact, including signed 8-bit AIFF test vectors.
    if !matches!(bits, 8 | 16 | 24 | 32) {
        return Err(MusicError::Unsupported(
            "AIFF requires integer 8/16/24/32-bit PCM",
        ));
    }
    let rate = integer_rate(&bytes[8..18])?;
    let order = if aifc {
        let code = &bytes[18..22];
        if code == b"NONE" || code == b"twos" {
            ByteOrder::Big
        } else if code == b"sowt" {
            ByteOrder::Little
        } else {
            return Err(MusicError::Unsupported(
                "lossy or non-integer AIFF-C codec is prohibited",
            ));
        }
    } else {
        ByteOrder::Big
    };
    let format = AudioFormat::new(
        rate,
        u8::try_from(channels)
            .map_err(|_| MusicError::Unsupported("AIFF channel count out of range"))?,
        bits as u8,
    )?;
    let frame_bytes = usize::from(format.channels) * usize::from(bits / 8);
    Ok(Common {
        format,
        total_frames,
        frame_bytes,
        order,
    })
}

/// An AIFF reader yields bounded PCM blocks. It validates the declared FORM
/// and SSND bounds before the first PCM sample. AIFF-C is permitted only with
/// a valid FVER and explicitly uncompressed signed PCM ("NONE", "twos", "sowt").
pub struct AiffDecoder<R: Read + Send> {
    input: R,
    common: Common,
    remaining: u64,
    pad_after_sound: bool,
}

impl<R: Read + Send> AiffDecoder<R> {
    pub fn new(mut input: R) -> Result<Self> {
        let mut form = [0u8; 12];
        input.read_exact(&mut form)?;
        if &form[..4] != b"FORM" || (&form[8..12] != b"AIFF" && &form[8..12] != b"AIFC") {
            return Err(MusicError::Invalid("expected AIFF or AIFF-C FORM"));
        }
        let aifc = &form[8..12] == b"AIFC";
        let form_bytes = u64::from(be32(&form[4..8]));
        if form_bytes < 4 {
            return Err(MusicError::Invalid("AIFF FORM smaller than type"));
        }
        let mut budget = form_bytes - 4;
        let mut metadata_total = 0u64;
        let mut common = None;
        let mut saw_fver = false;

        for _ in 0..MAX_CHUNKS {
            if budget == 0 {
                break;
            }
            if budget < 8 {
                return Err(MusicError::Invalid("partial AIFF chunk header within FORM"));
            }
            let mut header = [0u8; 8];
            input.read_exact(&mut header)?;
            budget -= 8;
            let length = u64::from(be32(&header[4..8]));
            let padded = length + (length & 1);
            if padded > budget {
                return Err(MusicError::Invalid(
                    "AIFF chunk exceeds declared FORM boundary",
                ));
            }
            let id: [u8; 4] = header[..4].try_into().unwrap();
            match &id {
                b"FVER" => {
                    if !aifc || saw_fver || length != 4 {
                        return Err(MusicError::Invalid("invalid or duplicate AIFF-C FVER"));
                    }
                    let mut version = [0u8; 4];
                    input.read_exact(&mut version)?;
                    if version != AIFC_VERSION {
                        return Err(MusicError::Unsupported("unknown AIFF-C version"));
                    }
                    saw_fver = true;
                }
                b"COMM" => {
                    if common.is_some() {
                        return Err(MusicError::Invalid("duplicate AIFF COMM"));
                    }
                    if length > 1024 || (aifc && length < 22) || (!aifc && length != 18) {
                        return Err(MusicError::Invalid("AIFF COMM length is invalid"));
                    }
                    let mut data = vec![0u8; length as usize];
                    input.read_exact(&mut data)?;
                    common = Some(parse_common(&data, aifc)?);
                }
                b"SSND" => {
                    let common = common.ok_or(MusicError::Invalid(
                        "AIFF SSND preceding COMM is unsupported",
                    ))?;
                    if aifc && !saw_fver {
                        return Err(MusicError::Invalid("AIFF-C FVER is required"));
                    }
                    if length < 8 {
                        return Err(MusicError::Invalid(
                            "AIFF SSND has no offset/block-size fields",
                        ));
                    }
                    let mut sound_header = [0u8; 8];
                    input.read_exact(&mut sound_header)?;
                    let offset = u64::from(be32(&sound_header[..4]));
                    let block_size = be32(&sound_header[4..8]);
                    if block_size != 0 {
                        return Err(MusicError::Unsupported(
                            "AIFF nonzero SSND block alignment not yet supported",
                        ));
                    }
                    if offset > length - 8 || offset > MAX_CHUNK_META {
                        return Err(MusicError::Limit("AIFF SSND offset exceeds bound"));
                    }
                    let pcm_bytes = length - 8 - offset;
                    let expected = common
                        .total_frames
                        .checked_mul(common.frame_bytes as u64)
                        .ok_or(MusicError::Limit("AIFF PCM length overflows"))?;
                    if pcm_bytes != expected {
                        return Err(MusicError::Invalid(
                            "AIFF SSND byte count disagrees with COMM frame count",
                        ));
                    }
                    skip(&mut input, offset)?;
                    return Ok(Self {
                        input,
                        common,
                        remaining: pcm_bytes,
                        pad_after_sound: length & 1 != 0,
                    });
                }
                _ => {
                    if length > MAX_CHUNK_META {
                        return Err(MusicError::Limit("AIFF metadata chunk exceeds skip bound"));
                    }
                    skip(&mut input, length)?;
                }
            }
            if length & 1 != 0 {
                skip(&mut input, 1)?;
            }
            metadata_total = metadata_total
                .checked_add(padded + 8)
                .ok_or(MusicError::Limit("AIFF metadata total overflows"))?;
            if metadata_total > MAX_TOTAL_META {
                return Err(MusicError::Limit("AIFF metadata total exceeds bound"));
            }
            budget -= padded;
        }
        Err(MusicError::Invalid(
            "AIFF COMM or SSND missing within bounded FORM",
        ))
    }

    pub fn next_pcm_block(&mut self) -> Result<Option<PcmBlock>> {
        if self.remaining == 0 {
            if self.pad_after_sound {
                skip(&mut self.input, 1)?;
                self.pad_after_sound = false;
            }
            return Ok(None);
        }
        let count =
            (self.remaining / self.common.frame_bytes as u64).min(BLOCK_FRAMES as u64) as usize;
        let length = count * self.common.frame_bytes;
        let mut raw = vec![0u8; length];
        self.input.read_exact(&mut raw)?;
        self.remaining -= length as u64;
        let width = usize::from(self.common.format.bits_per_sample) / 8;
        let mut samples = Vec::with_capacity(length / width);
        for sample in raw.chunks_exact(width) {
            let value = match (width, self.common.order) {
                (1, _) => i32::from(sample[0] as i8),
                (2, ByteOrder::Big) => i32::from(i16::from_be_bytes([sample[0], sample[1]])),
                (2, ByteOrder::Little) => i32::from(i16::from_le_bytes([sample[0], sample[1]])),
                (3, ByteOrder::Big) => {
                    ((u32::from(sample[0]) << 24
                        | u32::from(sample[1]) << 16
                        | u32::from(sample[2]) << 8) as i32)
                        >> 8
                }
                (3, ByteOrder::Little) => {
                    ((u32::from(sample[2]) << 24
                        | u32::from(sample[1]) << 16
                        | u32::from(sample[0]) << 8) as i32)
                        >> 8
                }
                (4, ByteOrder::Big) => i32::from_be_bytes(sample.try_into().unwrap()),
                (4, ByteOrder::Little) => i32::from_le_bytes(sample.try_into().unwrap()),
                _ => return Err(MusicError::Unsupported("invalid integer AIFF sample width")),
            };
            samples.push(value);
        }
        Ok(Some(PcmBlock::new(self.common.format, samples)?))
    }
}

impl<R: Read + Send> Decoder for AiffDecoder<R> {
    fn format(&self) -> AudioFormat {
        self.common.format
    }
    fn total_frames(&self) -> Option<u64> {
        Some(self.common.total_frames)
    }
    fn next_block(&mut self) -> Result<Option<PcmBlock>> {
        self.next_pcm_block()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn common(frames: u32, channels: u16, bits: u16, aifc: bool, codec: &[u8; 4]) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&channels.to_be_bytes());
        data.extend_from_slice(&frames.to_be_bytes());
        data.extend_from_slice(&bits.to_be_bytes());
        data.extend_from_slice(&[0x40, 0x0e, 0xac, 0x44, 0, 0, 0, 0, 0, 0]); // 44.1 kHz exact
        if aifc {
            data.extend_from_slice(codec);
            data.extend_from_slice(&[0, 0]); // Empty Pascal compression name + padding.
        }
        data
    }
    fn sound(data: &[u8], offset: &[u8]) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&(offset.len() as u32).to_be_bytes());
        payload.extend_from_slice(&0u32.to_be_bytes());
        payload.extend_from_slice(offset);
        payload.extend_from_slice(data);
        payload
    }
    fn form(aifc: bool, parts: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
        let mut body = if aifc {
            b"AIFC".to_vec()
        } else {
            b"AIFF".to_vec()
        };
        for (id, payload) in parts {
            body.extend_from_slice(&id[..]);
            body.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            body.extend_from_slice(payload);
            if payload.len() & 1 != 0 {
                body.push(0);
            }
        }
        let mut out = b"FORM".to_vec();
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        out.extend_from_slice(&body);
        out
    }
    fn version() -> Vec<u8> {
        AIFC_VERSION.to_vec()
    }

    #[test]
    fn aiff_stereo_24_bit_is_bit_exact() {
        let stream = form(
            false,
            &[
                (b"COMM", common(1, 2, 24, false, b"NONE")),
                (b"SSND", sound(&[0x7f, 0xff, 0xff, 0x80, 0, 0], &[])),
            ],
        );
        let mut decoder = AiffDecoder::new(Cursor::new(stream)).unwrap();
        assert_eq!(decoder.format(), AudioFormat::new(44_100, 2, 24).unwrap());
        assert_eq!(decoder.total_frames(), Some(1));
        assert_eq!(
            decoder.next_block().unwrap().unwrap().samples,
            vec![8_388_607, -8_388_608]
        );
        assert_eq!(decoder.next_block().unwrap(), None);
    }
    #[test]
    fn aifc_sowt_is_uncompressed_not_lossy() {
        let stream = form(
            true,
            &[
                (b"FVER", version()),
                (b"COMM", common(2, 1, 16, true, b"sowt")),
                (b"SSND", sound(&[0x34, 0x12, 0xfe, 0xff], &[])),
            ],
        );
        let mut decoder = AiffDecoder::new(Cursor::new(stream)).unwrap();
        assert_eq!(
            decoder.next_block().unwrap().unwrap().samples,
            vec![0x1234, -2]
        );
        assert_eq!(decoder.next_block().unwrap(), None);
    }
    #[test]
    fn sound_offset_and_odd_pad_are_consumed() {
        let stream = form(
            false,
            &[
                (b"COMM", common(1, 1, 8, false, b"NONE")),
                (b"JUNK", vec![1, 2, 3]),
                (b"SSND", sound(&[0x80], &[0xff, 0xff])),
            ],
        );
        let mut decoder = AiffDecoder::new(Cursor::new(stream)).unwrap();
        assert_eq!(decoder.next_block().unwrap().unwrap().samples, vec![-128]);
        assert_eq!(decoder.next_block().unwrap(), None);
    }
    #[test]
    fn rejects_lossy_aifc_and_missing_version() {
        let sample = sound(&[0, 0], &[]);
        let source = form(
            true,
            &[
                (b"FVER", version()),
                (b"COMM", common(1, 1, 16, true, b"ulaw")),
                (b"SSND", sample.clone()),
            ],
        );
        assert!(AiffDecoder::new(Cursor::new(source)).is_err());
        let source = form(
            true,
            &[
                (b"COMM", common(1, 1, 16, true, b"NONE")),
                (b"SSND", sample),
            ],
        );
        assert!(AiffDecoder::new(Cursor::new(source)).is_err());
    }
    #[test]
    fn rejects_fractional_rate_and_inconsistent_frame_count() {
        let mut comm = common(1, 1, 16, false, b"NONE");
        comm[17] = 1; // Fractional part of an 80-bit extended rate.
        let source = form(false, &[(b"COMM", comm), (b"SSND", sound(&[0, 0], &[]))]);
        assert!(AiffDecoder::new(Cursor::new(source)).is_err());
        let source = form(
            false,
            &[
                (b"COMM", common(2, 1, 16, false, b"NONE")),
                (b"SSND", sound(&[0, 0], &[])),
            ],
        );
        assert!(AiffDecoder::new(Cursor::new(source)).is_err());
    }
    #[test]
    fn rejects_form_overrun_and_short_pcm_read() {
        let mut source = form(
            false,
            &[
                (b"COMM", common(1, 1, 16, false, b"NONE")),
                (b"SSND", sound(&[0, 0], &[])),
            ],
        );
        source[4..8].copy_from_slice(&20u32.to_be_bytes());
        assert!(AiffDecoder::new(Cursor::new(source)).is_err());
        let mut source = form(
            false,
            &[
                (b"COMM", common(1, 1, 16, false, b"NONE")),
                (b"SSND", sound(&[0, 0], &[])),
            ],
        );
        source.pop(); // PCM bytes declared by COMM/SSND, absent physically.
        let mut decoder = AiffDecoder::new(Cursor::new(source)).unwrap();
        assert!(decoder.next_block().is_err());
    }
}
