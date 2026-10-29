//! Bounded ALAC packet decoding primitives, authored independently.
//!
//! This crate handles validated ALAC cookies and bounded compressed or
//! uncompressed mono/stereo packet elements. There is no container demuxer, so
//! this crate must not be advertised as complete ALAC file playback.
#![forbid(unsafe_code)]

use rust_music_core::{AudioFormat, MusicError, PcmBlock, Result};

const ALAC_CONFIG_BYTES: usize = 24;
const MAX_PACKET_BYTES: usize = 8 * 1024 * 1024;
const MAX_PACKET_FRAMES: usize = 65_535;

/// Decoder parameters carried by the 24-byte ALACSpecificConfig cookie.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AlacConfig {
    pub frame_length: u32,
    pub bit_depth: u8,
    pub rice_history_multiplier: u8,
    pub rice_initial_history: u8,
    pub rice_limit: u8,
    pub channels: u8,
    pub max_run: u16,
    pub max_frame_bytes: u32,
    pub average_bit_rate: u32,
    pub format: AudioFormat,
}

impl AlacConfig {
    /// Parse the bare, big-endian 24-byte ALACSpecificConfig.
    /// Container atom headers and optional channel-layout extensions belong
    /// to a separately bounded demux layer.
    pub fn parse(cookie: &[u8]) -> Result<Self> {
        if cookie.len() != ALAC_CONFIG_BYTES {
            return Err(MusicError::Invalid("ALAC config must be exactly 24 bytes"));
        }
        let frame_length = u32::from_be_bytes(cookie[0..4].try_into().unwrap());
        let compatible_version = cookie[4];
        let bit_depth = cookie[5];
        let rice_history_multiplier = cookie[6];
        let rice_initial_history = cookie[7];
        let rice_limit = cookie[8];
        let channels = cookie[9];
        let max_run = u16::from_be_bytes(cookie[10..12].try_into().unwrap());
        let max_frame_bytes = u32::from_be_bytes(cookie[12..16].try_into().unwrap());
        let average_bit_rate = u32::from_be_bytes(cookie[16..20].try_into().unwrap());
        let sample_rate = u32::from_be_bytes(cookie[20..24].try_into().unwrap());

        if compatible_version != 0 {
            return Err(MusicError::Unsupported(
                "unsupported ALAC compatible version",
            ));
        }
        if frame_length == 0 || frame_length as usize > MAX_PACKET_FRAMES {
            return Err(MusicError::Limit(
                "ALAC frame length exceeds configured bound",
            ));
        }
        if !matches!(bit_depth, 16 | 20 | 24 | 32) {
            return Err(MusicError::Unsupported("unsupported ALAC source bit depth"));
        }
        if !(1..=2).contains(&channels) {
            return Err(MusicError::Unsupported(
                "only mono and stereo ALAC packets are currently supported",
            ));
        }
        if rice_limit == 0 || rice_limit > 31 {
            return Err(MusicError::Invalid("invalid ALAC predictor parameters"));
        }
        if rice_initial_history > 127 {
            return Err(MusicError::Invalid("invalid ALAC initial Rice history"));
        }
        if max_frame_bytes as usize > MAX_PACKET_BYTES {
            return Err(MusicError::Limit("ALAC maximum frame size exceeds bound"));
        }
        let format = AudioFormat::new(sample_rate, channels, bit_depth)?;
        Ok(Self {
            frame_length,
            bit_depth,
            rice_history_multiplier,
            rice_initial_history,
            rice_limit,
            channels,
            max_run,
            max_frame_bytes,
            average_bit_rate,
            format,
        })
    }
}

/// Decode one raw ALAC packet. This is a packet API, not a file/container
/// `Decoder`; callers must provide packet boundaries from a validated demuxer.
pub fn decode_packet(config: AlacConfig, packet: &[u8]) -> Result<PcmBlock> {
    if packet.is_empty() || packet.len() > MAX_PACKET_BYTES {
        return Err(MusicError::Limit(
            "ALAC packet size is outside configured bounds",
        ));
    }
    if config.max_frame_bytes != 0 && packet.len() > config.max_frame_bytes as usize {
        return Err(MusicError::Limit(
            "ALAC packet exceeds configured frame size",
        ));
    }

    let mut bits = BitReader::new(packet);
    let element = bits.read(3)?;
    let expected_element = if config.channels == 1 { 0 } else { 1 };
    if element != expected_element {
        return Err(MusicError::Invalid(
            "ALAC packet element does not match channel count",
        ));
    }
    let _instance = bits.read(4)?;
    if bits.read(12)? != 0 {
        return Err(MusicError::Invalid("nonzero reserved ALAC element header"));
    }
    let flags = bits.read(4)?;
    let partial = flags & 0b1000 != 0;
    let bytes_shifted = ((flags >> 1) & 0b11) as u8;
    let escape = flags & 1 != 0;
    if bytes_shifted == 3 {
        return Err(MusicError::Invalid("invalid ALAC shifted-byte count"));
    }

    let frames = if partial {
        bits.read(32)? as usize
    } else {
        config.frame_length as usize
    };
    if frames == 0 || frames > config.frame_length as usize || frames > MAX_PACKET_FRAMES {
        return Err(MusicError::Limit(
            "ALAC packet frame count exceeds configured bound",
        ));
    }
    let sample_count = frames
        .checked_mul(usize::from(config.channels))
        .ok_or(MusicError::Limit("ALAC sample count overflow"))?;
    let mut samples = Vec::with_capacity(sample_count);
    if escape {
        if bytes_shifted != 0 {
            return Err(MusicError::Unsupported(
                "shifted-byte escape frames are not implemented",
            ));
        }
        for _ in 0..sample_count {
            samples.push(bits.read_signed(config.bit_depth)?);
        }
    } else {
        samples = decode_compressed(&mut bits, config, frames, bytes_shifted)?;
    }
    bits.finish_packet(!escape)?;
    PcmBlock::new(config.format, samples)
}

struct PredictorParameters {
    mode: u8,
    denominator_shift: u8,
    rice_factor: u8,
    coefficients: Vec<i16>,
}

fn read_predictor(bits: &mut BitReader<'_>) -> Result<PredictorParameters> {
    let mode = bits.read(4)? as u8;
    let denominator_shift = bits.read(4)? as u8;
    let rice_factor = bits.read(3)? as u8;
    let order = bits.read(5)? as usize;
    if !matches!(mode, 0 | 15) {
        return Err(MusicError::Unsupported("unsupported ALAC prediction mode"));
    }
    if order > 31 || (order > 0 && denominator_shift == 0) {
        return Err(MusicError::Invalid("invalid ALAC predictor order or scale"));
    }
    let mut coefficients = Vec::with_capacity(order);
    for _ in 0..order {
        coefficients.push(bits.read_signed(16)? as i16);
    }
    Ok(PredictorParameters {
        mode,
        denominator_shift,
        rice_factor,
        coefficients,
    })
}

fn decode_compressed(
    bits: &mut BitReader<'_>,
    config: AlacConfig,
    frames: usize,
    bytes_shifted: u8,
) -> Result<Vec<i32>> {
    let mix_bits = bits.read(8)? as u8;
    let mix_res = bits.read_signed(8)? as i8;
    if mix_bits > 31 || (config.channels == 1 && mix_res != 0) || (mix_res != 0 && mix_bits == 0) {
        return Err(MusicError::Invalid("invalid ALAC stereo matrix parameters"));
    }
    let predictor_u = read_predictor(bits)?;
    let predictor_v = if config.channels == 2 {
        Some(read_predictor(bits)?)
    } else {
        None
    };

    let shift_bits = bytes_shifted * 8;
    if shift_bits >= config.bit_depth {
        return Err(MusicError::Invalid(
            "ALAC shifted bytes exhaust sample width",
        ));
    }
    let coded_bits = config
        .bit_depth
        .checked_sub(shift_bits)
        .and_then(|width| width.checked_add(u8::from(config.channels == 2)))
        .ok_or(MusicError::Invalid("invalid ALAC coded sample width"))?;
    if coded_bits == 0 || coded_bits > 32 {
        return Err(MusicError::Unsupported("ALAC coded width exceeds 32 bits"));
    }

    let low_parts = read_shifted_samples(bits, frames, usize::from(config.channels), shift_bits)?;
    let residual_u =
        decode_entropy_channel(bits, config, frames, coded_bits, predictor_u.rice_factor)?;
    let high_u = decode_predictor(
        &residual_u,
        &predictor_u.coefficients,
        predictor_u.mode,
        predictor_u.denominator_shift,
        coded_bits,
    )?;
    let high_v = if let Some(predictor) = predictor_v {
        let residual_v =
            decode_entropy_channel(bits, config, frames, coded_bits, predictor.rice_factor)?;
        Some(decode_predictor(
            &residual_v,
            &predictor.coefficients,
            predictor.mode,
            predictor.denominator_shift,
            coded_bits,
        )?)
    } else {
        None
    };

    if let Some(high_v) = high_v {
        let high_width = config.bit_depth - shift_bits;
        let mut output = Vec::with_capacity(frames * 2);
        for frame in 0..frames {
            let (left, right) = if mix_res == 0 {
                (high_u[frame], high_v[frame])
            } else {
                let side = i64::from(high_v[frame]);
                let left =
                    i64::from(high_u[frame]) + side - ((i64::from(mix_res) * side) >> mix_bits);
                (
                    wrap_signed(left, high_width),
                    wrap_signed(left - side, high_width),
                )
            };
            output.push(join_sample(
                left,
                low_parts[frame * 2],
                shift_bits,
                config.bit_depth,
            ));
            output.push(join_sample(
                right,
                low_parts[frame * 2 + 1],
                shift_bits,
                config.bit_depth,
            ));
        }
        Ok(output)
    } else {
        Ok(high_u
            .into_iter()
            .zip(low_parts)
            .map(|(high, low)| join_sample(high, low, shift_bits, config.bit_depth))
            .collect())
    }
}

fn read_shifted_samples(
    bits: &mut BitReader<'_>,
    frames: usize,
    channels: usize,
    shift_bits: u8,
) -> Result<Vec<u32>> {
    let count = frames
        .checked_mul(channels)
        .ok_or(MusicError::Limit("ALAC shifted sample count overflow"))?;
    (0..count).map(|_| bits.read(shift_bits)).collect()
}

fn decode_entropy_channel(
    bits: &mut BitReader<'_>,
    config: AlacConfig,
    frames: usize,
    coded_bits: u8,
    rice_factor: u8,
) -> Result<Vec<i32>> {
    let mut residuals = Vec::with_capacity(frames);
    let mut history = u64::from(config.rice_initial_history);
    let rice_bias = u64::from(config.rice_history_multiplier) * u64::from(rice_factor) / 4;
    let mut zero_mode = false;
    while residuals.len() < frames {
        let rice_k = floor_log2((history >> 9) + 3).min(config.rice_limit);
        let rice_m = (1u32 << rice_k) - 1;
        let raw = bits.read_adaptive_scalar(rice_m, rice_k, coded_bits)?;
        let mapped = raw
            .checked_add(u32::from(zero_mode))
            .ok_or(MusicError::Limit("ALAC residual mapping overflow"))?;
        let signed = if mapped & 1 == 0 {
            i64::from(mapped / 2)
        } else {
            -i64::from(mapped.div_ceil(2))
        };
        residuals.push(
            i32::try_from(signed)
                .map_err(|_| MusicError::Invalid("ALAC residual exceeds signed bounds"))?,
        );
        if mapped > 0xffff {
            history = 0xffff;
        } else {
            history = history + rice_bias * u64::from(mapped) - ((rice_bias * history) >> 9);
        }
        zero_mode = false;
        if history < 128 && residuals.len() < frames {
            let leading_zeroes = (history as u32).leading_zeros() as i32;
            let run_k = (leading_zeroes - 24 + ((history + 16) >> 6) as i32)
                .clamp(0, i32::from(config.rice_limit)) as u8;
            let run_m = ((1u32 << run_k) - 1) & ((1u32 << config.rice_limit) - 1);
            let run = bits.read_adaptive_scalar(run_m, run_k, 16)? as usize;
            if run > frames - residuals.len() {
                return Err(MusicError::Invalid("ALAC zero run exceeds packet frames"));
            }
            residuals.resize(residuals.len() + run, 0);
            zero_mode = run < u16::MAX as usize;
            history = 0;
        }
    }
    Ok(residuals)
}

fn decode_predictor(
    residuals: &[i32],
    initial_coefficients: &[i16],
    mode: u8,
    denominator_shift: u8,
    width: u8,
) -> Result<Vec<i32>> {
    if residuals.is_empty() {
        return Err(MusicError::Invalid("empty ALAC predictor input"));
    }
    let errors = if mode == 0 {
        residuals.to_vec()
    } else {
        cumulative_predictor(residuals, width)
    };
    let order = initial_coefficients.len();
    if order >= errors.len() && order != 0 {
        return Err(MusicError::Invalid(
            "ALAC predictor order exceeds packet frames",
        ));
    }
    if order == 31 {
        return Ok(cumulative_predictor(&errors, width));
    }
    if order == 0 {
        return Ok(errors
            .iter()
            .map(|&value| wrap_signed(i64::from(value), width))
            .collect());
    }
    let mut output = vec![0i32; errors.len()];
    output[0] = wrap_signed(i64::from(errors[0]), width);
    for index in 1..=order {
        output[index] = wrap_signed(
            i64::from(output[index - 1]) + i64::from(errors[index]),
            width,
        );
    }
    let rounding = 1i64 << (denominator_shift - 1);
    let mut coefficients = initial_coefficients.to_vec();
    for index in order + 1..errors.len() {
        let top = output[index - order - 1];
        let prediction_sum = coefficients
            .iter()
            .enumerate()
            .map(|(coefficient_index, coefficient)| {
                i64::from(*coefficient)
                    * (i64::from(output[index - 1 - coefficient_index]) - i64::from(top))
            })
            .sum::<i64>();
        let error = errors[index];
        output[index] = wrap_signed(
            i64::from(error) + i64::from(top) + ((prediction_sum + rounding) >> denominator_shift),
            width,
        );
        let mut remaining_error = i64::from(error);
        if error > 0 {
            for coefficient_index in (0..order).rev() {
                let difference = i64::from(top) - i64::from(output[index - 1 - coefficient_index]);
                let direction = difference.signum();
                adjust_coefficient(&mut coefficients[coefficient_index], -direction)?;
                remaining_error -= (order - coefficient_index) as i64
                    * ((direction * difference) >> denominator_shift);
                if remaining_error <= 0 {
                    break;
                }
            }
        } else if error < 0 {
            for coefficient_index in (0..order).rev() {
                let difference = i64::from(top) - i64::from(output[index - 1 - coefficient_index]);
                let direction = -difference.signum();
                adjust_coefficient(&mut coefficients[coefficient_index], -direction)?;
                remaining_error -= (order - coefficient_index) as i64
                    * ((direction * difference) >> denominator_shift);
                if remaining_error >= 0 {
                    break;
                }
            }
        }
    }
    Ok(output)
}

fn adjust_coefficient(coefficient: &mut i16, change: i64) -> Result<()> {
    let next = i32::from(*coefficient)
        .checked_add(
            i32::try_from(change)
                .map_err(|_| MusicError::Invalid("ALAC coefficient adjustment overflow"))?,
        )
        .ok_or(MusicError::Invalid("ALAC predictor coefficient overflow"))?;
    *coefficient = i16::try_from(next)
        .map_err(|_| MusicError::Invalid("ALAC predictor coefficient overflow"))?;
    Ok(())
}

fn cumulative_predictor(values: &[i32], width: u8) -> Vec<i32> {
    let mut output = Vec::with_capacity(values.len());
    if let Some(&first) = values.first() {
        output.push(wrap_signed(i64::from(first), width));
    }
    for &value in values.iter().skip(1) {
        output.push(wrap_signed(
            i64::from(value) + i64::from(*output.last().unwrap()),
            width,
        ));
    }
    output
}

fn floor_log2(value: u64) -> u8 {
    if value <= 1 {
        0
    } else {
        (u64::BITS - 1 - value.leading_zeros()) as u8
    }
}

fn wrap_signed(value: i64, width: u8) -> i32 {
    let mask = (1i64 << width) - 1;
    let raw = value & mask;
    let sign = 1i64 << (width - 1);
    if raw & sign == 0 {
        raw as i32
    } else {
        (raw - (1i64 << width)) as i32
    }
}

fn join_sample(high: i32, low: u32, shift_bits: u8, total_bits: u8) -> i32 {
    wrap_signed((i64::from(high) << shift_bits) | i64::from(low), total_bits)
}

struct BitReader<'a> {
    bytes: &'a [u8],
    bit_offset: usize,
}

impl<'a> BitReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            bit_offset: 0,
        }
    }

    fn read(&mut self, count: u8) -> Result<u32> {
        if count > 32 {
            return Err(MusicError::Invalid("ALAC bit read exceeds 32 bits"));
        }
        let end = self
            .bit_offset
            .checked_add(usize::from(count))
            .ok_or(MusicError::Limit("ALAC bit offset overflow"))?;
        if end > self.bytes.len().saturating_mul(8) {
            return Err(MusicError::Invalid("truncated ALAC packet"));
        }
        let mut value = 0u32;
        for offset in self.bit_offset..end {
            let byte = self.bytes[offset / 8];
            let bit = (byte >> (7 - (offset % 8))) & 1;
            value = (value << 1) | u32::from(bit);
        }
        self.bit_offset = end;
        Ok(value)
    }

    fn read_signed(&mut self, count: u8) -> Result<i32> {
        let raw = self.read(count)?;
        if count == 0 {
            return Ok(0);
        }
        if count == 32 {
            return Ok(raw as i32);
        }
        let sign = 1u32 << (count - 1);
        if raw & sign == 0 {
            Ok(raw as i32)
        } else {
            Ok((i64::from(raw) - (1i64 << count)) as i32)
        }
    }

    fn require_zero_padding(&mut self) -> Result<()> {
        let remaining = self.bytes.len() * 8 - self.bit_offset;
        if remaining > 7 || self.read(remaining as u8)? != 0 {
            return Err(MusicError::Invalid("unexpected trailing ALAC packet data"));
        }
        Ok(())
    }

    fn read_adaptive_scalar(&mut self, multiplier: u32, rice_k: u8, max_bits: u8) -> Result<u32> {
        let mut unary = 0u8;
        while unary < 9 {
            if self.read(1)? == 0 {
                break;
            }
            unary += 1;
        }
        if unary == 9 {
            return self.read(max_bits);
        }
        if rice_k == 1 {
            return Ok(u32::from(unary));
        }
        let high = self.read(rice_k - 1)?;
        if high == 0 {
            return Ok(u32::from(unary) * multiplier);
        }
        let suffix = (high << 1) | self.read(1)?;
        u32::from(unary)
            .checked_mul(multiplier)
            .and_then(|base| base.checked_add(suffix - 1))
            .ok_or(MusicError::Limit("ALAC Golomb value overflow"))
    }

    fn finish_packet(&mut self, require_end_element: bool) -> Result<()> {
        let remaining = self.bytes.len() * 8 - self.bit_offset;
        if remaining <= 7 && !require_end_element {
            return self.require_zero_padding();
        }
        for _ in 0..=31 {
            if self.bit_offset + 3 > self.bytes.len() * 8 {
                break;
            }
            if self.read(3)? == 7 {
                return self.require_zero_padding();
            }
            self.bit_offset -= 3;
            if self.read(1)? != 0 {
                return Err(MusicError::Invalid("unexpected trailing ALAC packet data"));
            }
        }
        Err(MusicError::Invalid("missing ALAC packet end element"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct BitWriter(Vec<bool>);

    impl BitWriter {
        fn new() -> Self {
            Self(Vec::new())
        }
        fn write(&mut self, value: u32, count: u8) {
            for shift in (0..count).rev() {
                self.0.push((value >> shift) & 1 != 0);
            }
        }
        fn finish(self) -> Vec<u8> {
            let mut bytes = vec![0; self.0.len().div_ceil(8)];
            for (index, bit) in self.0.into_iter().enumerate() {
                if bit {
                    bytes[index / 8] |= 1 << (7 - index % 8);
                }
            }
            bytes
        }
    }

    fn config(channels: u8, bits: u8, frames: u32, rate: u32) -> AlacConfig {
        let mut raw = [0u8; ALAC_CONFIG_BYTES];
        raw[0..4].copy_from_slice(&frames.to_be_bytes());
        raw[5] = bits;
        raw[6] = 40;
        raw[7] = 10;
        raw[8] = 14;
        raw[9] = channels;
        raw[10..12].copy_from_slice(&255u16.to_be_bytes());
        raw[20..24].copy_from_slice(&rate.to_be_bytes());
        AlacConfig::parse(&raw).unwrap()
    }

    fn escape_packet(
        channels: u8,
        bits: u8,
        samples: &[i32],
        frames: u32,
        partial: bool,
    ) -> Vec<u8> {
        let mut writer = BitWriter::new();
        writer.write(if channels == 1 { 0 } else { 1 }, 3);
        writer.write(0, 4);
        writer.write(0, 12);
        writer.write((u32::from(partial) << 3) | 1, 4);
        if partial {
            writer.write(frames, 32);
        }
        for sample in samples {
            writer.write(*sample as u32, bits);
        }
        writer.finish()
    }

    #[test]
    fn parses_bounded_config_and_rejects_malformed_fields() {
        let good = config(2, 24, 4096, 96_000);
        assert_eq!(good.format, AudioFormat::new(96_000, 2, 24).unwrap());
        let mut raw = [0u8; ALAC_CONFIG_BYTES];
        assert!(AlacConfig::parse(&raw).is_err());
        raw[0..4].copy_from_slice(&4096u32.to_be_bytes());
        raw[4] = 1;
        assert!(AlacConfig::parse(&raw).is_err());
        raw[4] = 0;
        raw[5] = 18;
        assert!(AlacConfig::parse(&raw).is_err());
        raw[5] = 16;
        raw[9] = 9;
        assert!(AlacConfig::parse(&raw).is_err());
        assert!(AlacConfig::parse(&[0; 23]).is_err());
        assert!(AlacConfig::parse(&[0; 25]).is_err());
    }

    #[test]
    fn decodes_mono_16_bit_signed_uncompressed_packet() {
        let cfg = config(1, 16, 3, 44_100);
        let packet = escape_packet(1, 16, &[-32768, -1, 12345], 3, false);
        let block = decode_packet(cfg, &packet).unwrap();
        assert_eq!(block.samples, [-32768, -1, 12345]);
        assert_eq!(block.frames(), 3);
    }

    #[test]
    fn decodes_partial_stereo_24_bit_packet_in_interleaved_order() {
        let cfg = config(2, 24, 64, 48_000);
        let packet = escape_packet(2, 24, &[-8_388_608, 8_388_607, -1, 42], 2, true);
        let block = decode_packet(cfg, &packet).unwrap();
        assert_eq!(block.samples, [-8_388_608, 8_388_607, -1, 42]);
        assert_eq!(block.frames(), 2);
    }

    #[test]
    fn decodes_32_bit_escape_packet_at_full_signed_range() {
        let cfg = config(1, 32, 3, 96_000);
        let packet = escape_packet(1, 32, &[i32::MIN, -1, i32::MAX], 3, false);
        let block = decode_packet(cfg, &packet).unwrap();
        assert_eq!(block.samples, [i32::MIN, -1, i32::MAX]);
    }

    #[test]
    fn rejects_truncation_wrong_element_and_overlong_frame() {
        let cfg = config(1, 16, 2, 44_100);
        let packet = escape_packet(1, 16, &[1, 2], 2, false);
        assert!(decode_packet(cfg, &packet[..packet.len() - 1]).is_err());

        let stereo_for_mono = escape_packet(2, 16, &[1, 2], 1, false);
        assert!(decode_packet(cfg, &stereo_for_mono).is_err());

        let mut too_many = BitWriter::new();
        too_many.write(0, 3);
        too_many.write(0, 4);
        too_many.write(0, 12);
        too_many.write(0b1001, 4);
        too_many.write(3, 32);
        assert!(decode_packet(cfg, &too_many.finish()).is_err());

        let mut compressed_writer = BitWriter::new();
        compressed_writer.write(0, 3);
        compressed_writer.write(0, 4);
        compressed_writer.write(0, 12);
        compressed_writer.write(0, 4);
        assert!(decode_packet(cfg, &compressed_writer.finish()).is_err());
    }

    #[test]
    fn compressed_stereo_packet_matches_independent_synthetic_pcm() {
        let cookie = [
            0x00, 0x00, 0x10, 0x00, 0x00, 16, 40, 10, 14, 2, 0, 0, 0, 0, 0x40, 4, 0x00, 0x15, 0x88,
            0x80, 0x00, 0x00, 0xac, 0x44,
        ];
        let cfg = AlacConfig::parse(&cookie).unwrap();
        let packet = include_bytes!("../tests/fixtures/stereo16_packet.bin");
        let block = decode_packet(cfg, packet).unwrap();
        let expected: Vec<i32> = (0..64)
            .flat_map(|frame| [-30_000 + frame * 173, -25_000 + frame * 251])
            .collect();
        assert_eq!(block.samples, expected);
    }

    #[test]
    fn compressed_stereo_24_bit_shifted_packet_matches_synthetic_pcm() {
        let cookie = [
            0x00, 0x00, 0x10, 0x00, 0x00, 24, 40, 10, 14, 2, 0, 0, 0, 0, 0x60, 4, 0x00, 0x20, 0x4c,
            0xc0, 0x00, 0x00, 0xac, 0x44,
        ];
        let cfg = AlacConfig::parse(&cookie).unwrap();
        let packet = include_bytes!("../tests/fixtures/alac24_packet.bin");
        let block = decode_packet(cfg, packet).unwrap();
        let expected: Vec<i32> = (0..64)
            .flat_map(|frame| [-7_000_000 + frame * 17_111, 4_000_000 - frame * 9_999])
            .collect();
        assert_eq!(block.samples, expected);
    }

    #[test]
    fn compressed_stereo_32_bit_shifted_packet_matches_reference_pcm() {
        let cookie = [
            0x00, 0x00, 0x10, 0x00, 0x00, 32, 40, 10, 14, 2, 0, 0, 0, 0, 0x60, 4, 0x00, 0x20, 0x4c,
            0xc0, 0x00, 0x00, 0xac, 0x44,
        ];
        let cfg = AlacConfig::parse(&cookie).unwrap();
        let packet = include_bytes!("../tests/fixtures/alac32_packet.bin");
        let block = decode_packet(cfg, packet).unwrap();
        let expected: Vec<i32> = (0..64)
            .flat_map(|frame| {
                [
                    (-7_000_000 + frame * 17_111) * 256,
                    (4_000_000 - frame * 9_999) * 256,
                ]
            })
            .collect();
        assert_eq!(block.samples, expected);
    }

    #[test]
    fn compressed_stereo_matrix_packet_matches_reference_pcm() {
        let cookie = [
            0x00, 0x00, 0x10, 0x00, 0x00, 16, 40, 10, 14, 2, 0, 0, 0, 0, 0x40, 0x04, 0x00, 0x15,
            0x88, 0x80, 0x00, 0x00, 0xac, 0x44,
        ];
        let cfg = AlacConfig::parse(&cookie).unwrap();
        let packet = include_bytes!("../tests/fixtures/matrix_stereo16_packet.bin");
        let block = decode_packet(cfg, packet).unwrap();
        let expected: Vec<i32> = (0..256)
            .flat_map(|frame| {
                let left = (frame * 997) % 20_000 + 10_000;
                [left, left * 3 / 4 + 3_000]
            })
            .collect();
        assert_eq!(block.samples, expected);
    }

    #[test]
    fn rejects_matrix_shift_wider_than_sample_domain() {
        let cfg = config(2, 16, 1, 44_100);
        let mut writer = BitWriter::new();
        writer.write(1, 3);
        writer.write(0, 4);
        writer.write(0, 12);
        writer.write(0, 4);
        writer.write(255, 8);
        writer.write(1, 8);
        assert!(decode_packet(cfg, &writer.finish()).is_err());
    }

    #[test]
    fn packet_bit_flips_never_panic_or_exceed_decoded_frame_bounds() {
        let cookie = [
            0x00, 0x00, 0x10, 0x00, 0x00, 16, 40, 10, 14, 2, 0, 0, 0, 0, 0x40, 0x04, 0x00, 0x15,
            0x88, 0x80, 0x00, 0x00, 0xac, 0x44,
        ];
        let cfg = AlacConfig::parse(&cookie).unwrap();
        let packet = include_bytes!("../tests/fixtures/matrix_stereo16_packet.bin");
        for bit_index in 0..packet.len() * 8 {
            let mut damaged = packet.to_vec();
            damaged[bit_index / 8] ^= 1 << (7 - bit_index % 8);
            let result = std::panic::catch_unwind(|| decode_packet(cfg, &damaged));
            let decoded = result.expect("single packet bit flip must not panic");
            if let Ok(block) = decoded {
                assert!(block.frames() <= cfg.frame_length as usize);
                assert_eq!(
                    block.samples.len(),
                    block.frames() * usize::from(cfg.channels)
                );
            }
        }
    }

    #[test]
    fn rejects_every_truncated_prefix_of_compressed_stereo_packet() {
        let cookie = [
            0x00, 0x00, 0x10, 0x00, 0x00, 16, 40, 10, 14, 2, 0, 0, 0, 0, 0x40, 4, 0x00, 0x15, 0x88,
            0x80, 0x00, 0x00, 0xac, 0x44,
        ];
        let cfg = AlacConfig::parse(&cookie).unwrap();
        let packet = include_bytes!("../tests/fixtures/stereo16_packet.bin");
        for end in 0..packet.len() {
            assert!(
                decode_packet(cfg, &packet[..end]).is_err(),
                "accepted prefix {end}"
            );
        }

        let mut cookie_32 = cookie;
        cookie_32[5] = 32;
        let cfg_32 = AlacConfig::parse(&cookie_32).unwrap();
        let packet_32 = include_bytes!("../tests/fixtures/alac32_packet.bin");
        for end in 0..packet_32.len() {
            assert!(
                decode_packet(cfg_32, &packet_32[..end]).is_err(),
                "accepted 32-bit packet prefix {end}"
            );
        }
    }

    #[test]
    fn packet_and_padding_limits_fail_closed() {
        let cfg = config(1, 16, 1, 44_100);
        assert!(decode_packet(cfg, &[0; MAX_PACKET_BYTES + 1]).is_err());
        let mut packet = escape_packet(1, 16, &[1], 1, false);
        packet.push(0);
        assert!(decode_packet(cfg, &packet).is_err());
    }
}
