//! Independently implemented native FLAC frame decoder following IETF RFC 9639.
//!
//! No libFLAC, Symphonia, Claxon, or other codec is linked. Integer PCM is
//! preserved through the decoder. It is deliberately a bounded streaming reader;
//! tags and unknown metadata are skipped without loading the entire file.
//!
//! Scope: native FLAC, mono/independent multi-channel and all FLAC stereo modes,
//! constant/verbatim/fixed/LPC subframes, both Rice methods, wasted bits, and
//! header/frame CRC. Ogg FLAC and seeking are not yet supported.

use rust_music_core::{AudioFormat, Decoder, MusicError, PcmBlock, Result};
use std::io::{self, Read};

mod md5;
use md5::Md5;

const MAX_METADATA_BLOCKS: usize = 128;
const MAX_METADATA_BYTES: u32 = 16 * 1024 * 1024;
const MAX_TOTAL_METADATA_BYTES: u64 = 64 * 1024 * 1024;
const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
const MAX_RICE_UNARY: u64 = 1 << 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamInfo {
    pub min_block_size: u16,
    pub max_block_size: u16,
    pub min_frame_size: u32,
    pub max_frame_size: u32,
    pub format: AudioFormat,
    pub total_samples: u64,
    pub pcm_md5: [u8; 16],
}

fn be24(raw: &[u8]) -> u32 {
    (u32::from(raw[0]) << 16) | (u32::from(raw[1]) << 8) | u32::from(raw[2])
}

impl StreamInfo {
    fn parse(raw: &[u8; 34]) -> Result<Self> {
        let min_block_size = u16::from_be_bytes([raw[0], raw[1]]);
        let max_block_size = u16::from_be_bytes([raw[2], raw[3]]);
        if min_block_size < 16 || max_block_size < min_block_size {
            return Err(MusicError::Invalid("invalid STREAMINFO block size limits"));
        }
        let min_frame_size = be24(&raw[4..7]);
        let max_frame_size = be24(&raw[7..10]);
        if max_frame_size != 0 && min_frame_size > max_frame_size {
            return Err(MusicError::Invalid("invalid STREAMINFO frame size limits"));
        }
        let packed = u64::from_be_bytes(raw[10..18].try_into().unwrap());
        let sample_rate = ((packed >> 44) & 0xfffff) as u32;
        let channels = (((packed >> 41) & 0x7) + 1) as u8;
        let bits_per_sample = (((packed >> 36) & 0x1f) + 1) as u8;
        let total_samples = packed & 0x0fff_fffff;
        let format = AudioFormat::new(sample_rate, channels, bits_per_sample)?;
        let mut pcm_md5 = [0; 16];
        pcm_md5.copy_from_slice(&raw[18..34]);
        Ok(Self {
            min_block_size,
            max_block_size,
            min_frame_size,
            max_frame_size,
            format,
            total_samples,
            pcm_md5,
        })
    }
}

fn skip_bounded<R: Read>(reader: &mut R, bytes: u32) -> Result<()> {
    let mut left = bytes;
    let mut buf = [0u8; 4096];
    while left > 0 {
        let count = (left as usize).min(buf.len());
        reader.read_exact(&mut buf[..count])?;
        left -= count as u32;
    }
    Ok(())
}

fn read_metadata<R: Read>(reader: &mut R) -> Result<StreamInfo> {
    let mut magic = [0; 4];
    reader.read_exact(&mut magic)?;
    if &magic != b"fLaC" {
        return Err(MusicError::Invalid("missing native FLAC marker"));
    }
    let mut info = None;
    let mut last = false;
    let mut metadata_bytes = 0u64;
    for index in 0..MAX_METADATA_BLOCKS {
        let mut hdr = [0; 4];
        reader.read_exact(&mut hdr)?;
        last = hdr[0] & 0x80 != 0;
        let kind = hdr[0] & 0x7f;
        let length = be24(&hdr[1..]);
        if kind == 127 {
            return Err(MusicError::Invalid("reserved metadata type"));
        }
        if length > MAX_METADATA_BYTES {
            return Err(MusicError::Limit("metadata block exceeds configured bound"));
        }
        metadata_bytes += u64::from(length) + 4;
        if metadata_bytes > MAX_TOTAL_METADATA_BYTES {
            return Err(MusicError::Limit(
                "aggregate metadata exceeds configured bound",
            ));
        }
        if index == 0 {
            if kind != 0 || length != 34 {
                return Err(MusicError::Invalid(
                    "first FLAC block must be STREAMINFO(34)",
                ));
            }
            let mut raw = [0u8; 34];
            reader.read_exact(&mut raw)?;
            info = Some(StreamInfo::parse(&raw)?);
        } else {
            if kind == 0 {
                return Err(MusicError::Invalid("duplicate STREAMINFO"));
            }
            skip_bounded(reader, length)?;
        }
        if last {
            break;
        }
    }
    if !last {
        return Err(MusicError::Limit("too many FLAC metadata blocks"));
    }
    info.ok_or(MusicError::Invalid("FLAC STREAMINFO missing"))
}

fn crc8_update(mut crc: u8, byte: u8) -> u8 {
    crc ^= byte;
    for _ in 0..8 {
        crc = if crc & 0x80 != 0 {
            (crc << 1) ^ 0x07
        } else {
            crc << 1
        };
    }
    crc
}
fn crc16_update(mut crc: u16, byte: u8) -> u16 {
    crc ^= u16::from(byte) << 8;
    for _ in 0..8 {
        crc = if crc & 0x8000 != 0 {
            (crc << 1) ^ 0x8005
        } else {
            crc << 1
        };
    }
    crc
}

struct Bits<R> {
    input: R,
    current: u8,
    left: u8,
    frame_tracking: bool,
    crc8: u8,
    crc16: u16,
    frame_bytes: usize,
}
impl<R: Read> Bits<R> {
    fn new(input: R) -> Self {
        Self {
            input,
            current: 0,
            left: 0,
            frame_tracking: false,
            crc8: 0,
            crc16: 0,
            frame_bytes: 0,
        }
    }
    fn start_frame(&mut self) -> Result<()> {
        if self.left != 0 {
            return Err(MusicError::Invalid("frame is not byte aligned"));
        }
        self.frame_tracking = true;
        self.frame_bytes = 0;
        self.crc8 = 0;
        self.crc16 = 0;
        Ok(())
    }
    fn raw_opt(&mut self) -> Result<Option<u8>> {
        let mut b = [0u8; 1];
        loop {
            match self.input.read(&mut b) {
                Ok(0) => return Ok(None),
                Ok(_) => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
            }
        }
        if self.frame_tracking {
            self.frame_bytes += 1;
            if self.frame_bytes > MAX_FRAME_BYTES {
                return Err(MusicError::Limit("FLAC frame exceeds byte budget"));
            }
            self.crc8 = crc8_update(self.crc8, b[0]);
            self.crc16 = crc16_update(self.crc16, b[0]);
        }
        Ok(Some(b[0]))
    }
    fn byte_opt(&mut self) -> Result<Option<u8>> {
        if self.left != 0 {
            return Err(MusicError::Invalid("byte operation not aligned"));
        }
        self.raw_opt()
    }
    fn byte(&mut self) -> Result<u8> {
        self.byte_opt()?
            .ok_or(MusicError::Invalid("truncated FLAC frame"))
    }
    fn bits(&mut self, mut count: u8) -> Result<u64> {
        if count > 64 {
            return Err(MusicError::Invalid("bit width exceeds 64"));
        }
        let mut out = 0u64;
        while count > 0 {
            if self.left == 0 {
                self.current = self.byte()?;
                self.left = 8;
            }
            let take = count.min(self.left);
            let shift = self.left - take;
            let mask = (1u16 << take) - 1;
            let val = (u16::from(self.current >> shift) & mask) as u64;
            out = (out << take) | val;
            self.left -= take;
            count -= take;
        }
        Ok(out)
    }
    fn signed(&mut self, width: u8) -> Result<i64> {
        if !(1..=33).contains(&width) {
            return Err(MusicError::Invalid("invalid signed sample width"));
        }
        let raw = self.bits(width)?;
        let shift = 64 - width;
        Ok(((raw << shift) as i64) >> shift)
    }
    fn zeros_until_one(&mut self, limit: u64) -> Result<u64> {
        let mut count = 0;
        while self.bits(1)? == 0 {
            count += 1;
            if count > limit {
                return Err(MusicError::Limit("unary code exceeds configured bound"));
            }
        }
        Ok(count)
    }
    fn align_zero(&mut self) -> Result<()> {
        if self.left > 0 {
            let rest = self.left;
            if self.bits(rest)? != 0 {
                return Err(MusicError::Invalid("nonzero FLAC frame pad bits"));
            }
        }
        Ok(())
    }
}

fn utf8_number<R: Read>(reader: &mut Bits<R>) -> Result<u64> {
    let first = reader.byte()?;
    if first & 0x80 == 0 {
        return Ok(u64::from(first));
    }
    let follow = first.leading_ones() as usize - 1;
    if !(1..=6).contains(&follow) || (first & (0x40 >> follow)) != 0 {
        return Err(MusicError::Invalid(
            "invalid FLAC UTF-8 frame/sample number",
        ));
    }
    let first_value_bits = 6usize.saturating_sub(follow);
    let mut value = u64::from(first & ((1u8 << first_value_bits) - 1));
    for _ in 0..follow {
        let b = reader.byte()?;
        if b & 0xc0 != 0x80 {
            return Err(MusicError::Invalid("bad FLAC number continuation"));
        }
        value = (value << 6) | u64::from(b & 0x3f);
    }
    // Two-byte UTF-8 starts at 0x80; higher widths start at 2^(5*n+1).
    // Reject overlong number encodings even when the header CRC is valid.
    let minimum = 1u64 << (if follow == 1 { 7 } else { 5 * follow + 1 });
    if value < minimum || value >= (1u64 << 36) {
        return Err(MusicError::Invalid("noncanonical or oversized FLAC number"));
    }
    Ok(value)
}

fn sample_width(code: u8, source: u8) -> Result<u8> {
    match code {
        0 => Ok(source),
        1 => Ok(8),
        2 => Ok(12),
        4 => Ok(16),
        5 => Ok(20),
        6 => Ok(24),
        7 => Ok(32),
        _ => Err(MusicError::Invalid("reserved frame bit-depth code")),
    }
}
fn block_size<R: Read>(code: u8, b: &mut Bits<R>) -> Result<usize> {
    let n = match code {
        1 => 192,
        2..=5 => 576usize << (code - 2),
        6 => b.byte()? as usize + 1,
        7 => ((usize::from(b.byte()?) << 8) | usize::from(b.byte()?)) + 1,
        8..=15 => 256usize << (code - 8),
        _ => return Err(MusicError::Invalid("reserved FLAC block size")),
    };
    Ok(n)
}
fn sample_rate<R: Read>(code: u8, b: &mut Bits<R>, source: u32) -> Result<u32> {
    match code {
        0 => Ok(source),
        1 => Ok(88_200),
        2 => Ok(176_400),
        3 => Ok(192_000),
        4 => Ok(8_000),
        5 => Ok(16_000),
        6 => Ok(22_050),
        7 => Ok(24_000),
        8 => Ok(32_000),
        9 => Ok(44_100),
        10 => Ok(48_000),
        11 => Ok(96_000),
        12 => Ok(u32::from(b.byte()?) * 1000),
        13 => Ok((u32::from(b.byte()?) << 8) | u32::from(b.byte()?)),
        14 => Ok(((u32::from(b.byte()?) << 8) | u32::from(b.byte()?)) * 10),
        _ => Err(MusicError::Invalid("reserved sample rate code")),
    }
}

fn valid_signed(value: i128, width: u8) -> Result<i64> {
    let max = (1i128 << (width - 1)) - 1;
    let min = -(1i128 << (width - 1));
    if value < min || value > max {
        return Err(MusicError::Invalid(
            "FLAC prediction outside coded sample width",
        ));
    }
    Ok(value as i64)
}

fn residual<R: Read>(b: &mut Bits<R>, block: usize, order: usize) -> Result<Vec<i64>> {
    let method = b.bits(2)?;
    let parameter_width = match method {
        0 => 4,
        1 => 5,
        _ => return Err(MusicError::Invalid("reserved FLAC residual method")),
    };
    let partition_order = b.bits(4)? as usize;
    let partitions = 1usize << partition_order;
    if block % partitions != 0 {
        return Err(MusicError::Invalid(
            "residual partition does not divide block",
        ));
    }
    let each = block / partitions;
    if each < order {
        return Err(MusicError::Invalid(
            "residual partition smaller than predictor",
        ));
    }
    let mut out = Vec::with_capacity(block - order);
    for index in 0..partitions {
        let count = if index == 0 { each - order } else { each };
        let param = b.bits(parameter_width)? as u8;
        let escape = (1 << parameter_width) - 1;
        if param == escape {
            let width = b.bits(5)? as u8;
            for _ in 0..count {
                let value = if width == 0 { 0 } else { b.signed(width)? };
                out.push(value);
            }
        } else {
            for _ in 0..count {
                let quotient = b.zeros_until_one(MAX_RICE_UNARY)?;
                let rem = b.bits(param)?;
                let folded = (quotient << param) | rem;
                // RFC 9639 9.2.7 excludes i32::MIN (-2^31) even though it
                // can be represented as a folded u32::MAX value.
                if folded >= u64::from(u32::MAX) {
                    return Err(MusicError::Invalid(
                        "FLAC residual outside allowed signed-32 range",
                    ));
                }
                let signed = ((folded >> 1) as i64) ^ (-((folded & 1) as i64));
                out.push(signed);
            }
        }
    }
    Ok(out)
}

fn subframe<R: Read>(b: &mut Bits<R>, block: usize, bits: u8) -> Result<Vec<i64>> {
    if b.bits(1)? != 0 {
        return Err(MusicError::Invalid("nonzero FLAC subframe padding"));
    }
    let kind = b.bits(6)? as u8;
    let wasted_flag = b.bits(1)? != 0;
    let wasted = if wasted_flag {
        b.zeros_until_one(32)? as u8 + 1
    } else {
        0
    };
    if wasted >= bits {
        return Err(MusicError::Invalid("wasted bits exceed sample width"));
    }
    let effective = bits - wasted;
    let mut samples = match kind {
        0 => vec![b.signed(effective)?; block],
        1 => {
            let mut out = Vec::with_capacity(block);
            for _ in 0..block {
                out.push(b.signed(effective)?);
            }
            out
        }
        8..=12 => {
            let order = (kind - 8) as usize;
            if order > block {
                return Err(MusicError::Invalid("fixed predictor order exceeds block"));
            }
            let mut out = Vec::with_capacity(block);
            for _ in 0..order {
                out.push(b.signed(effective)?);
            }
            let remaining = residual(b, block, order)?;
            for r in remaining {
                let n = out.len();
                let predict = match order {
                    0 => 0,
                    1 => i128::from(out[n - 1]),
                    2 => 2 * i128::from(out[n - 1]) - i128::from(out[n - 2]),
                    3 => {
                        3 * i128::from(out[n - 1]) - 3 * i128::from(out[n - 2])
                            + i128::from(out[n - 3])
                    }
                    4 => {
                        4 * i128::from(out[n - 1]) - 6 * i128::from(out[n - 2])
                            + 4 * i128::from(out[n - 3])
                            - i128::from(out[n - 4])
                    }
                    _ => unreachable!(),
                };
                out.push(valid_signed(predict + i128::from(r), effective)?);
            }
            out
        }
        32..=63 => {
            let order = (kind - 31) as usize;
            if order > block {
                return Err(MusicError::Invalid("LPC order exceeds block"));
            }
            let mut out = Vec::with_capacity(block);
            for _ in 0..order {
                out.push(b.signed(effective)?);
            }
            let precision_code = b.bits(4)? as u8;
            if precision_code == 15 {
                return Err(MusicError::Invalid("reserved LPC precision"));
            }
            let precision = precision_code + 1;
            let shift = b.signed(5)?;
            if shift < 0 {
                return Err(MusicError::Invalid("negative LPC shift forbidden"));
            }
            let mut coeff = Vec::with_capacity(order);
            for _ in 0..order {
                coeff.push(b.signed(precision)?);
            }
            for r in residual(b, block, order)? {
                let n = out.len();
                let mut predicted = 0i128;
                for (back, coef) in coeff.iter().enumerate() {
                    predicted += i128::from(*coef) * i128::from(out[n - 1 - back]);
                }
                out.push(valid_signed(
                    (predicted >> (shift as u32)) + i128::from(r),
                    effective,
                )?);
            }
            out
        }
        _ => return Err(MusicError::Invalid("reserved FLAC subframe type")),
    };
    if wasted > 0 {
        for value in &mut samples {
            *value = valid_signed(i128::from(*value) << wasted, bits)?;
        }
    }
    Ok(samples)
}

/// Pull-based FLAC reader. Constructing it only parses metadata: never opens audio.
pub struct FlacDecoder<R: Read + Send> {
    reader: Bits<R>,
    info: StreamInfo,
    decoded_frames: u64,
    frame_index: u64,
    blocking_strategy: Option<bool>,
    pcm_md5: Option<Md5>,
    finished: bool,
}
impl<R: Read + Send> FlacDecoder<R> {
    pub fn new(mut input: R) -> Result<Self> {
        let info = read_metadata(&mut input)?;
        let pcm_md5 = (info.pcm_md5 != [0; 16]).then(Md5::new);
        Ok(Self {
            reader: Bits::new(input),
            info,
            decoded_frames: 0,
            frame_index: 0,
            blocking_strategy: None,
            pcm_md5,
            finished: false,
        })
    }
    pub fn stream_info(&self) -> &StreamInfo {
        &self.info
    }

    fn update_pcm_md5(&mut self, interleaved: &[i32]) {
        if let Some(digest) = self.pcm_md5.as_mut() {
            // RFC 9639: signed, sign-extended to whole bytes, little-endian,
            // interleaved sample by sample. This is independent of WAVE's
            // unsigned 8-bit representation.
            let bytes_per_sample = usize::from(self.info.format.bits_per_sample).div_ceil(8);
            let mut buffer = [0u8; 4096];
            let mut used = 0;
            for sample in interleaved {
                if used + bytes_per_sample > buffer.len() {
                    digest.update(&buffer[..used]);
                    used = 0;
                }
                buffer[used..used + bytes_per_sample]
                    .copy_from_slice(&sample.to_le_bytes()[..bytes_per_sample]);
                used += bytes_per_sample;
            }
            digest.update(&buffer[..used]);
        }
    }

    pub fn next_frame(&mut self) -> Result<Option<PcmBlock>> {
        if self.finished {
            return Ok(None);
        }
        let b = &mut self.reader;
        b.start_frame()?;
        let first = match b.byte_opt()? {
            Some(x) => x,
            None => {
                if self.info.total_samples != 0 && self.info.total_samples != self.decoded_frames {
                    return Err(MusicError::Invalid(
                        "FLAC decoded sample count mismatches STREAMINFO",
                    ));
                }
                if let Some(digest) = &self.pcm_md5 {
                    if digest.clone().finish() != self.info.pcm_md5 {
                        return Err(MusicError::Invalid(
                            "FLAC decoded PCM MD5 mismatches STREAMINFO",
                        ));
                    }
                }
                self.finished = true;
                return Ok(None);
            }
        };
        let second = b.byte()?;
        if first != 0xff || (second & 0xfe) != 0xf8 {
            return Err(MusicError::Invalid(
                "invalid FLAC frame sync or reserved bit",
            ));
        }
        let block_rate = b.byte()?;
        let channel_depth = b.byte()?;
        if channel_depth & 1 != 0 {
            return Err(MusicError::Invalid("FLAC reserved frame header bit"));
        }
        let channel_mode = channel_depth >> 4;
        let channels = if channel_mode < 8 {
            channel_mode + 1
        } else if channel_mode <= 10 {
            2
        } else {
            return Err(MusicError::Invalid("reserved FLAC channel assignment"));
        };
        let bit_depth = sample_width((channel_depth >> 1) & 7, self.info.format.bits_per_sample)?;
        let variable_blocking = second & 1 == 1;
        if self
            .blocking_strategy
            .is_some_and(|old| old != variable_blocking)
        {
            return Err(MusicError::Invalid(
                "FLAC blocking strategy changed mid-stream",
            ));
        }
        let number = utf8_number(b)?;
        let expected_number = if variable_blocking {
            self.decoded_frames
        } else {
            self.frame_index
        };
        if number != expected_number {
            return Err(MusicError::Invalid(
                "non-sequential FLAC frame/sample number",
            ));
        }
        let size = block_size(block_rate >> 4, b)?;
        let rate = sample_rate(block_rate & 15, b, self.info.format.sample_rate)?;
        if size == 0 || size > usize::from(self.info.max_block_size) {
            return Err(MusicError::Invalid("FLAC block exceeds STREAMINFO maximum"));
        }
        if rate != self.info.format.sample_rate
            || channels != self.info.format.channels
            || bit_depth != self.info.format.bits_per_sample
        {
            return Err(MusicError::Invalid(
                "FLAC frame format differs from STREAMINFO",
            ));
        }
        let header_crc = b.crc8;
        if b.byte()? != header_crc {
            return Err(MusicError::Invalid("FLAC frame header CRC8 mismatch"));
        }
        let mut pcm = Vec::with_capacity(usize::from(channels));
        for channel in 0..channels {
            let depth = if (channel_mode == 8 && channel == 1)
                || (channel_mode == 9 && channel == 0)
                || (channel_mode == 10 && channel == 1)
            {
                bit_depth + 1
            } else {
                bit_depth
            };
            pcm.push(subframe(b, size, depth)?);
        }
        b.align_zero()?;
        let frame_crc = b.crc16;
        let stored = (u16::from(b.byte()?) << 8) | u16::from(b.byte()?);
        if frame_crc != stored {
            return Err(MusicError::Invalid("FLAC frame CRC16 mismatch"));
        }
        let mut interleaved = Vec::with_capacity(size * usize::from(channels));
        for index in 0..size {
            if channel_mode < 8 {
                for channel in &pcm {
                    interleaved.push(valid_signed(i128::from(channel[index]), bit_depth)? as i32);
                }
            } else {
                let (left, right) = match channel_mode {
                    8 => (
                        i128::from(pcm[0][index]),
                        i128::from(pcm[0][index]) - i128::from(pcm[1][index]),
                    ),
                    9 => (
                        i128::from(pcm[0][index]) + i128::from(pcm[1][index]),
                        i128::from(pcm[1][index]),
                    ),
                    10 => {
                        let side = i128::from(pcm[1][index]);
                        let mid = (i128::from(pcm[0][index]) << 1) | (side & 1);
                        ((mid + side) >> 1, (mid - side) >> 1)
                    }
                    _ => unreachable!(),
                };
                interleaved.push(valid_signed(left, bit_depth)? as i32);
                interleaved.push(valid_signed(right, bit_depth)? as i32);
            }
        }
        self.decoded_frames = self
            .decoded_frames
            .checked_add(size as u64)
            .ok_or(MusicError::Limit("FLAC decoded sample counter overflow"))?;
        if self.info.total_samples != 0 && self.decoded_frames > self.info.total_samples {
            return Err(MusicError::Invalid(
                "FLAC decoded samples exceed STREAMINFO total",
            ));
        }
        self.frame_index = self
            .frame_index
            .checked_add(1)
            .ok_or(MusicError::Limit("FLAC frame index overflow"))?;
        self.blocking_strategy = Some(variable_blocking);
        self.update_pcm_md5(&interleaved);
        Ok(Some(PcmBlock::new(self.info.format, interleaved)?))
    }
}
impl<R: Read + Send> Decoder for FlacDecoder<R> {
    fn format(&self) -> AudioFormat {
        self.info.format
    }
    fn total_frames(&self) -> Option<u64> {
        (self.info.total_samples != 0).then_some(self.info.total_samples)
    }
    fn next_block(&mut self) -> Result<Option<PcmBlock>> {
        self.next_frame()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn fixture_header(channels: u8, sample_count: u64) -> Vec<u8> {
        let mut bytes = b"fLaC".to_vec();
        bytes.extend_from_slice(&[0x80, 0, 0, 34]);
        bytes.extend_from_slice(&16u16.to_be_bytes());
        bytes.extend_from_slice(&16u16.to_be_bytes());
        bytes.extend_from_slice(&[0; 6]);
        let packed =
            (44_100u64 << 44) | (u64::from(channels - 1) << 41) | (15u64 << 36) | sample_count;
        bytes.extend_from_slice(&packed.to_be_bytes());
        bytes.extend_from_slice(&[0; 16]);
        bytes
    }
    fn add_frame(file: &mut Vec<u8>, channel_code: u8, subframes: &[u8]) {
        let mut frame = vec![0xff, 0xf8, 0x60, channel_code << 4, 0, 15];
        let crc = frame
            .iter()
            .fold(0u8, |value, &byte| crc8_update(value, byte));
        frame.push(crc);
        frame.extend_from_slice(subframes);
        let crc = frame
            .iter()
            .fold(0u16, |value, &byte| crc16_update(value, byte));
        frame.extend_from_slice(&crc.to_be_bytes());
        file.extend(frame);
    }
    #[test]
    fn constant_mono_is_lossless() {
        let mut f = fixture_header(1, 16);
        add_frame(&mut f, 0, &[0, 0x12, 0x34]);
        let mut d = FlacDecoder::new(Cursor::new(f)).unwrap();
        assert_eq!(d.total_frames(), Some(16));
        let b = d.next_frame().unwrap().unwrap();
        assert_eq!(b.samples, vec![0x1234; 16]);
        assert_eq!(d.next_frame().unwrap(), None);
    }
    #[test]
    fn unknown_streaminfo_total_remains_unknown() {
        let mut file = fixture_header(1, 0);
        add_frame(&mut file, 0, &[0, 0, 1]);
        let mut decoder = FlacDecoder::new(Cursor::new(file)).unwrap();
        assert_eq!(decoder.total_frames(), None);
        assert_eq!(decoder.next_frame().unwrap().unwrap().frames(), 16);
        assert_eq!(decoder.next_frame().unwrap(), None);
    }

    #[test]
    fn left_side_restores_stereo() {
        let mut f = fixture_header(2, 16);
        // L=+1000, S=L-R=+500 => R=+500. Side width 17 bits.
        add_frame(&mut f, 8, &[0, 0x03, 0xe8, 0, 0x00, 0xfa, 0x00]);
        let mut d = FlacDecoder::new(Cursor::new(f)).unwrap();
        let b = d.next_frame().unwrap().unwrap();
        assert_eq!(b.samples, [1000, 500].repeat(16));
    }
    #[test]
    fn corrupt_crc_is_rejected() {
        let mut f = fixture_header(1, 16);
        add_frame(&mut f, 0, &[0, 0, 1]);
        *f.last_mut().unwrap() ^= 1;
        let mut d = FlacDecoder::new(Cursor::new(f)).unwrap();
        assert!(d.next_frame().is_err());
    }
    #[test]
    fn metadata_is_validated() {
        assert!(FlacDecoder::new(Cursor::new(b"nope".to_vec())).is_err());
        let mut f = fixture_header(1, 16);
        f[4] = 0;
        assert!(FlacDecoder::new(Cursor::new(f)).is_err());
    }
    #[test]
    fn rice_zeroes_and_fixed_predictor() {
        // fixed order 0, residual method 0/order 0/parameter 0;
        // 16 zero residuals encoded as 16 one bits, padded with 6 zeroes.
        let mut f = fixture_header(1, 16);
        add_frame(&mut f, 0, &[0x10, 0, 0x3f, 0xff, 0xc0]);
        let mut d = FlacDecoder::new(Cursor::new(f)).unwrap();
        assert_eq!(d.next_frame().unwrap().unwrap().samples, vec![0; 16]);
    }

    #[test]
    fn pcm_md5_matches_reference_for_interleaved_stereo() {
        let mut file = fixture_header(2, 16);
        file[26..42].copy_from_slice(&[
            0x0a, 0x89, 0x86, 0xce, 0xdc, 0x70, 0x17, 0x84, 0x51, 0x9a, 0xc2, 0x58, 0x34, 0xc9,
            0x23, 0x5f,
        ]);
        add_frame(&mut file, 8, &[0, 0x03, 0xe8, 0, 0x00, 0xfa, 0x00]);
        let mut decoder = FlacDecoder::new(Cursor::new(file.clone())).unwrap();
        assert_eq!(
            decoder.next_frame().unwrap().unwrap().samples,
            [1000, 500].repeat(16)
        );
        assert_eq!(decoder.next_frame().unwrap(), None);
        // STREAMINFO is outside the frame CRC domain: corruption is only
        // detectable with a full-stream decoded-PCM MD5 comparison.
        file[26] ^= 1;
        let mut corrupt = FlacDecoder::new(Cursor::new(file)).unwrap();
        assert!(corrupt.next_frame().unwrap().is_some());
        assert!(matches!(
            corrupt.next_frame(),
            Err(MusicError::Invalid(
                "FLAC decoded PCM MD5 mismatches STREAMINFO"
            ))
        ));
    }

    #[test]
    fn repeated_frame_number_is_not_accepted() {
        let mut file = fixture_header(1, 32);
        add_frame(&mut file, 0, &[0, 0, 1]);
        add_frame(&mut file, 0, &[0, 0, 2]);
        let mut decoder = FlacDecoder::new(Cursor::new(file)).unwrap();
        assert_eq!(decoder.next_frame().unwrap().unwrap().samples, vec![1; 16]);
        assert!(matches!(
            decoder.next_frame(),
            Err(MusicError::Invalid(
                "non-sequential FLAC frame/sample number"
            ))
        ));
    }

    #[test]
    fn rejects_overlong_frame_number_encoding() {
        let mut decoder = Bits::new(Cursor::new(vec![0xc1, 0x80]));
        assert!(utf8_number(&mut decoder).is_err());
        let mut canonical = Bits::new(Cursor::new(vec![0xc2, 0x80]));
        assert_eq!(utf8_number(&mut canonical).unwrap(), 128);
    }
}
