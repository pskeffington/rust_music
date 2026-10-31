//! Bounded WavPack v4/v5 block and metadata parsing.
//!
//! A bounded packed-16-bit, packed-24-bit, and padded-24-bit mono/stereo block
//! decoder supports predictor terms -2, -1, 2–5, 17, and 18, but this crate is
//! intentionally not registered by the public facade. Trailing tags, broader
//! profile coverage, and host playback integration remain open RM-014 work.

use rust_music_core::{MusicError, Result};
use std::io::{self, Read};

pub const MAX_BLOCK_BYTES: usize = 1024 * 1024;
pub const MAX_BLOCK_SAMPLES: u32 = 131_072;
const HEADER_BYTES: usize = 32;
const MAX_BIT_READ: u8 = 32;
const MAX_DECORRELATION_PASSES: usize = 16;
const DECORRELATION_RING_SIZE: usize = 8;
const MAX_METADATA_BLOCKS_PER_GAP: usize = 64;
const EXP2_1_256_Q32: u64 = 4_306_612_134;

const FLAG_MONO: u32 = 1 << 2;
const FLAG_HYBRID: u32 = 1 << 3;
const FLAG_FLOAT: u32 = 1 << 7;
const FLAG_JOINT_STEREO: u32 = 1 << 4;
const FLAG_INT32_DATA: u32 = 1 << 8;
const FLAG_INITIAL_BLOCK: u32 = 1 << 11;
const FLAG_FINAL_BLOCK: u32 = 1 << 12;
const FLAG_DSD: u32 = 1 << 31;
const FLAG_HAS_CHECKSUM: u32 = 1 << 28;
const ID_BLOCK_CHECKSUM: u8 = 0x0f;
const ID_DECORRELATION_TERMS: u8 = 2;
const ID_DECORRELATION_WEIGHTS: u8 = 3;
const ID_DECORRELATION_SAMPLES: u8 = 4;
const ID_ENTROPY_VARIABLES: u8 = 5;
const ID_WV_BITSTREAM: u8 = 10;
const ID_INT32_INFO: u8 = 9;

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

/// Validate the WavPack header checksum after a complete audio block has been
/// reconstructed into signed integer PCM in channel-interleaved order.
pub fn verify_decoded_pcm_checksum(header: &BlockHeader, samples: &[i32]) -> Result<()> {
    let expected_samples = usize::try_from(header.block_samples)
        .ok()
        .and_then(|frames| frames.checked_mul(usize::from(header.channels())))
        .ok_or(MusicError::Limit("WavPack decoded sample count overflows"))?;
    if samples.len() != expected_samples {
        return Err(MusicError::Invalid(
            "WavPack decoded sample count does not match block header",
        ));
    }

    let checksum = samples.iter().fold(u32::MAX, |checksum, sample| {
        checksum.wrapping_mul(3).wrapping_add(*sample as u32)
    });
    if checksum != header.crc {
        return Err(MusicError::Invalid("WavPack decoded PCM checksum mismatch"));
    }
    Ok(())
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

/// One reversible WavPack decorrelation predictor. The bounded decoder accepts
/// cross-channel predictors -2/-1, lag predictors 2–8, and special two-sample
/// predictors 17 and 18.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecorrelationPass {
    pub term: i8,
    pub delta: u8,
    pub weights: [i32; 2],
    /// Predictor history per channel. Lag terms use a ring indexed by the
    /// block-local sample phase; terms 17 and 18 use the first two entries.
    pub history: [[i32; DECORRELATION_RING_SIZE]; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecorrelationState {
    channels: u8,
    passes: Vec<DecorrelationPass>,
}

impl DecorrelationState {
    /// Parse terms, initial weights and log-coded sample history from the
    /// corresponding bounded metadata blocks (IDs 2, 3 and 4).
    pub fn parse(terms: &[u8], weights: &[u8], samples: &[u8], channels: u8) -> Result<Self> {
        if !(1..=2).contains(&channels) {
            return Err(MusicError::Unsupported(
                "WavPack decorrelation requires mono or stereo",
            ));
        }
        if terms.is_empty() || terms.len() > MAX_DECORRELATION_PASSES {
            return Err(MusicError::Limit(
                "WavPack decorrelation pass count is outside bounds",
            ));
        }
        let mut passes = Vec::with_capacity(terms.len());
        for &encoded in terms.iter().rev() {
            let term = (encoded & 0x1f) as i8 - 5;
            if term < 0 && channels != 2 {
                return Err(MusicError::Unsupported(
                    "WavPack cross-channel decorrelation requires stereo",
                ));
            }
            decorrelation_history_count(term)?;
            passes.push(DecorrelationPass {
                term,
                delta: encoded >> 5,
                weights: [0; 2],
                history: [[0; DECORRELATION_RING_SIZE]; 2],
            });
        }

        let expected_weights = passes
            .len()
            .checked_mul(usize::from(channels))
            .ok_or(MusicError::Limit("WavPack weight count overflows"))?;
        if weights.len() > expected_weights || weights.len() % usize::from(channels) != 0 {
            return Err(MusicError::Invalid(
                "WavPack decorrelation weights have the wrong length",
            ));
        }
        let pass_count = passes.len();
        for (pass_index, encoded) in weights.chunks_exact(usize::from(channels)).enumerate() {
            let pass = passes
                .get_mut(pass_count - 1 - pass_index)
                .expect("weight count is bounded by pass count");
            for (weight, &encoded) in pass.weights.iter_mut().zip(encoded) {
                *weight = restore_weight(encoded as i8);
            }
        }

        let mut offset = 0;
        for pass_index in (0..passes.len()).rev() {
            let pass = &passes[pass_index];
            let history_count = decorrelation_history_count(pass.term)?;
            let pass_bytes = history_count
                .checked_mul(2)
                .and_then(|bytes| bytes.checked_mul(usize::from(channels)))
                .ok_or(MusicError::Limit("WavPack sample history size overflows"))?;
            /* Each stored value is a signed 16-bit logarithmic sample. */
            if offset == samples.len() {
                break;
            }
            let end = offset
                .checked_add(pass_bytes)
                .ok_or(MusicError::Limit("WavPack sample history size overflows"))?;
            if end > samples.len() {
                return Err(MusicError::Invalid(
                    "WavPack decorrelation samples have the wrong length",
                ));
            }
            let pass = &mut passes[pass_index];
            for channel in 0..usize::from(channels) {
                for history in 0..history_count {
                    let encoded = i16::from_le_bytes([samples[offset], samples[offset + 1]]);
                    pass.history[channel][history] = expand_log_sample(encoded)?;
                    offset += 2;
                }
            }
        }
        if offset != samples.len() {
            return Err(MusicError::Invalid(
                "WavPack decorrelation samples have the wrong length",
            ));
        }
        Ok(Self { channels, passes })
    }

    /// Reverse the decoded residuals through each adaptive predictor in
    /// stream order. Samples are interleaved and must contain whole frames.
    pub fn reconstruct(&mut self, residuals: &mut [i32]) -> Result<()> {
        if residuals.len() % usize::from(self.channels) != 0 {
            return Err(MusicError::Invalid(
                "WavPack residual count is not a whole number of frames",
            ));
        }
        let original_state = self.clone();
        let original_residuals = residuals.to_vec();
        let result = self.reconstruct_inner(residuals);
        if result.is_err() {
            *self = original_state;
            residuals.copy_from_slice(&original_residuals);
        }
        result
    }

    fn reconstruct_inner(&mut self, residuals: &mut [i32]) -> Result<()> {
        let channels = usize::from(self.channels);
        for pass in &mut self.passes {
            let mut sample_phase = 0usize;
            for frame in residuals.chunks_exact_mut(channels) {
                if pass.term < 0 {
                    let encoded_left = frame[0];
                    let encoded_right = frame[1];
                    let (left, right) = if pass.term == -1 {
                        let predictor_left = pass.history[0][0];
                        let left = apply_weight(pass.weights[0], predictor_left)
                            .checked_add(encoded_left)
                            .ok_or(MusicError::Limit("WavPack reconstructed sample overflows"))?;
                        let predictor_right = left;
                        let right = apply_weight(pass.weights[1], predictor_right)
                            .checked_add(encoded_right)
                            .ok_or(MusicError::Limit("WavPack reconstructed sample overflows"))?;
                        update_weight_clipped(
                            &mut pass.weights[0],
                            pass.delta,
                            predictor_left,
                            encoded_left,
                        );
                        update_weight_clipped(
                            &mut pass.weights[1],
                            pass.delta,
                            predictor_right,
                            encoded_right,
                        );
                        (left, right)
                    } else {
                        let predictor_right = pass.history[1][0];
                        let right = apply_weight(pass.weights[1], predictor_right)
                            .checked_add(encoded_right)
                            .ok_or(MusicError::Limit("WavPack reconstructed sample overflows"))?;
                        let predictor_left = right;
                        let left = apply_weight(pass.weights[0], predictor_left)
                            .checked_add(encoded_left)
                            .ok_or(MusicError::Limit("WavPack reconstructed sample overflows"))?;
                        update_weight_clipped(
                            &mut pass.weights[0],
                            pass.delta,
                            predictor_left,
                            encoded_left,
                        );
                        update_weight_clipped(
                            &mut pass.weights[1],
                            pass.delta,
                            predictor_right,
                            encoded_right,
                        );
                        (left, right)
                    };
                    pass.history[0][0] = right;
                    pass.history[1][0] = left;
                    frame[0] = left;
                    frame[1] = right;
                } else {
                    for (channel, sample_out) in frame.iter_mut().enumerate() {
                        let history = &mut pass.history[channel];
                        let predictor = match pass.term {
                            17 => history[0].wrapping_mul(2).wrapping_sub(history[1]),
                            18 => history[0].wrapping_mul(3).wrapping_sub(history[1]) >> 1,
                            _ => history[sample_phase],
                        };
                        let residual = *sample_out;
                        let sample = apply_weight(pass.weights[channel], predictor)
                            .checked_add(residual)
                            .ok_or(MusicError::Limit("WavPack reconstructed sample overflows"))?;
                        update_weight(&mut pass.weights[channel], pass.delta, predictor, residual);
                        if matches!(pass.term, 17 | 18) {
                            history[1] = history[0];
                            history[0] = sample;
                        } else {
                            let write_index =
                                (sample_phase + pass.term as usize) & (DECORRELATION_RING_SIZE - 1);
                            history[write_index] = sample;
                        }
                        *sample_out = sample;
                    }
                }
                sample_phase = (sample_phase + 1) & (DECORRELATION_RING_SIZE - 1);
            }
        }
        Ok(())
    }

    pub fn passes(&self) -> &[DecorrelationPass] {
        &self.passes
    }
}

/// Restore left/right channels when the block used joint-stereo mid/side
/// coding. The input is an interleaved sequence of mid and side samples.
pub fn restore_joint_stereo(samples: &mut [i32]) -> Result<()> {
    if samples.len() % 2 != 0 {
        return Err(MusicError::Invalid(
            "WavPack joint-stereo samples are not paired",
        ));
    }
    for pair in samples.chunks_exact_mut(2) {
        let mid = pair[0];
        let side = pair[1]
            .checked_sub(mid >> 1)
            .ok_or(MusicError::Limit("WavPack side sample overflows"))?;
        pair[0] = mid
            .checked_add(side)
            .ok_or(MusicError::Limit("WavPack left sample overflows"))?;
        pair[1] = side;
    }
    Ok(())
}

fn validate_int32_info(metadata: Option<&[u8]>, padded_24_bit: bool) -> Result<()> {
    match (padded_24_bit, metadata) {
        (true, Some([0, 8, 0, 0])) => Ok(()),
        (true, Some(_)) => Err(MusicError::Unsupported(
            "WavPack extended integer layout is unsupported",
        )),
        (true, None) => Err(MusicError::Invalid(
            "WavPack extended integer metadata is missing",
        )),
        (false, Some(_)) => Err(MusicError::Invalid(
            "WavPack extended integer metadata is unexpected",
        )),
        (false, None) => Ok(()),
    }
}

fn expand_log_sample(encoded: i16) -> Result<i32> {
    let magnitude = i32::from(encoded).unsigned_abs();
    if magnitude > u16::MAX as u32 {
        return Err(MusicError::Limit("WavPack sample history exceeds range"));
    }
    let value = expand_log_median(magnitude as u16)?;
    let value = i32::try_from(value)
        .map_err(|_| MusicError::Limit("WavPack sample history exceeds i32"))?;
    Ok(if encoded < 0 { -value } else { value })
}

fn apply_weight(weight: i32, sample: i32) -> i32 {
    ((i64::from(sample) * i64::from(weight) + 512) >> 10) as i32
}

fn restore_weight(encoded: i8) -> i32 {
    let mut weight = i32::from(encoded) * 8;
    if weight > 0 {
        weight += (weight + 64) >> 7;
    }
    weight
}

fn update_weight(weight: &mut i32, delta: u8, source: i32, residual: i32) {
    if source > 0 && residual > 0 || source < 0 && residual < 0 {
        *weight += i32::from(delta);
    } else if source > 0 && residual < 0 || source < 0 && residual > 0 {
        *weight -= i32::from(delta);
    }
}

fn decorrelation_history_count(term: i8) -> Result<usize> {
    match term {
        -2 | -1 => Ok(1),
        2..=8 => Ok(term as usize),
        17 | 18 => Ok(2),
        _ => Err(MusicError::Unsupported(
            "WavPack decorrelation term is not yet supported",
        )),
    }
}

fn update_weight_clipped(weight: &mut i32, delta: u8, source: i32, residual: i32) {
    if source == 0 || residual == 0 {
        return;
    }
    if (source < 0) == (residual < 0) {
        *weight = (*weight + i32::from(delta)).min(1024);
    } else {
        *weight = (*weight - i32::from(delta)).max(-1024);
    }
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

/// Decode one complete, lossless WavPack block with packed 16-bit, packed
/// 24-bit, or the bounded 24-in-32 profile, mono or stereo. Returns
/// channel-interleaved signed PCM at its declared integer depth. Other
/// extended-integer, hybrid, float, DSD, multichannel, unknown mandatory
/// metadata, and unsupported decorrelation modes fail closed.
pub fn decode_lossless_block(block: &[u8]) -> Result<(BlockHeader, Vec<i32>)> {
    let header = BlockHeader::parse(block)?;
    if block.len() != header.block_bytes {
        return Err(MusicError::Invalid(
            "WavPack decoder requires exactly one complete block",
        ));
    }
    header.validate_integer_mono_stereo()?;
    let stored_bytes = header.flags & 0x3;
    let extended_integer = header.flags & FLAG_INT32_DATA != 0;
    let pcm_bits = match (extended_integer, stored_bytes) {
        (false, 1) => 16,
        (false, 2) | (true, 3) => 24,
        _ => {
            return Err(MusicError::Unsupported(
                "WavPack block is outside packed-16, packed-24, or padded-24-bit profiles",
            ))
        }
    };
    verify_encoded_block_checksum(&header, block)?;

    let mut required: [Option<&[u8]>; 11] = [None; 11];
    for entry in iter_metadata(block)? {
        let metadata = entry?;
        if matches!(metadata.id, 0 | ID_BLOCK_CHECKSUM) {
            continue;
        }
        let slot = match metadata.id {
            ID_DECORRELATION_TERMS
            | ID_DECORRELATION_WEIGHTS
            | ID_DECORRELATION_SAMPLES
            | ID_ENTROPY_VARIABLES
            | ID_INT32_INFO
            | ID_WV_BITSTREAM => &mut required[usize::from(metadata.id)],
            _ if metadata.optional => continue,
            _ => {
                return Err(MusicError::Unsupported(
                    "unknown mandatory WavPack metadata is unsupported",
                ))
            }
        };
        if metadata.optional {
            continue;
        }
        if slot.replace(metadata.data).is_some() {
            return Err(MusicError::Invalid(
                "WavPack decoder metadata is duplicated",
            ));
        }
    }

    let terms = required[usize::from(ID_DECORRELATION_TERMS)].ok_or(MusicError::Invalid(
        "WavPack decorrelation terms are missing",
    ))?;
    let weights = required[usize::from(ID_DECORRELATION_WEIGHTS)].ok_or(MusicError::Invalid(
        "WavPack decorrelation weights are missing",
    ))?;
    let history = required[usize::from(ID_DECORRELATION_SAMPLES)].ok_or(MusicError::Invalid(
        "WavPack decorrelation samples are missing",
    ))?;
    let entropy_data = required[usize::from(ID_ENTROPY_VARIABLES)]
        .ok_or(MusicError::Invalid("WavPack entropy variables are missing"))?;
    let bitstream = required[usize::from(ID_WV_BITSTREAM)]
        .ok_or(MusicError::Invalid("WavPack audio bitstream is missing"))?;
    validate_int32_info(required[usize::from(ID_INT32_INFO)], extended_integer)?;
    if bitstream.is_empty() || bitstream.len() % 2 != 0 {
        return Err(MusicError::Invalid(
            "WavPack audio bitstream has an invalid byte length",
        ));
    }

    let channels = header.channels();
    let entropy = parse_entropy_variables(entropy_data, channels)?;
    let mut words = WordDecoder::new(entropy)?;
    let mut bits = BitReader::new(bitstream);
    let mut samples = words.decode_frames(&mut bits, header.block_samples)?;
    let mut decorrelation = DecorrelationState::parse(terms, weights, history, channels)?;
    decorrelation.reconstruct(&mut samples)?;
    if header.flags & FLAG_JOINT_STEREO != 0 {
        if channels != 2 {
            return Err(MusicError::Invalid(
                "WavPack joint stereo is set for a mono block",
            ));
        }
        restore_joint_stereo(&mut samples)?;
    }
    verify_decoded_pcm_checksum(&header, &samples)?;
    if pcm_bits == 24 {
        for sample in &samples {
            if !(-8_388_608..=8_388_607).contains(sample) {
                return Err(MusicError::Invalid("WavPack sample exceeds 24-bit range"));
            }
        }
    }
    Ok((header, samples))
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
    /// Prefix bits arrive least-significant first; longer values add one
    /// suffix bit after the truncated prefix.
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
            if range.is_power_of_two() {
                let width = range.trailing_zeros() as u8;
                if width == 0 {
                    return Ok(0);
                }
                let prefix = self.read_bits(width - 1)?;
                let suffix = u32::from(self.read_bit()?);
                return Ok((prefix << 1) | suffix);
            }
            let width = (u32::BITS - 1 - range.leading_zeros()) as u8;
            let short_count = ((1u64 << (width + 1)) - u64::from(range)) as u32;
            let mut prefix = 0u32;
            for bit_index in 0..width {
                if self.read_bit()? {
                    prefix |= 1 << bit_index;
                }
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
    verify_encoded_block_checksum(&header, &block)?;
    Ok(Some((header, block)))
}

/// Verify the optional WavPack 5 encoded-block checksum before any metadata or
/// entropy decoder consumes the block. The checksum covers little-endian
/// 16-bit words from the block start up to (but excluding) checksum metadata.
fn verify_encoded_block_checksum(header: &BlockHeader, block: &[u8]) -> Result<()> {
    let payload = block
        .get(HEADER_BYTES..)
        .ok_or(MusicError::Invalid("WavPack block header is truncated"))?;
    let mut offset = 0usize;
    let mut checksum_seen = false;
    while offset < payload.len() {
        let start = offset;
        let (metadata, used) = parse_metadata(&payload[offset..])?;
        let raw_id = payload[start];
        if metadata.id == ID_BLOCK_CHECKSUM {
            if checksum_seen || raw_id & 0x40 != 0 || !metadata.optional {
                return Err(MusicError::Invalid(
                    "WavPack block checksum metadata flags are invalid",
                ));
            }
            if !matches!(metadata.data.len(), 2 | 4) {
                return Err(MusicError::Invalid(
                    "WavPack block checksum must be 16 or 32 bits",
                ));
            }
            let checksum_start = HEADER_BYTES
                .checked_add(start)
                .ok_or(MusicError::Limit("WavPack checksum offset overflows"))?;
            let mut checksum = u32::MAX;
            for word in block[..checksum_start].chunks_exact(2) {
                checksum = checksum
                    .wrapping_mul(3)
                    .wrapping_add(u32::from(u16::from_le_bytes([word[0], word[1]])));
            }
            let expected = if metadata.data.len() == 4 {
                u32::from_le_bytes(metadata.data.try_into().unwrap())
            } else {
                checksum ^= checksum >> 16;
                u32::from(u16::from_le_bytes(metadata.data.try_into().unwrap()))
            };
            let actual = if metadata.data.len() == 4 {
                checksum
            } else {
                checksum & 0xffff
            };
            if expected != actual {
                return Err(MusicError::Invalid(
                    "WavPack encoded-block checksum mismatch",
                ));
            }
            checksum_seen = true;
        }
        offset = offset
            .checked_add(used)
            .ok_or(MusicError::Limit("WavPack metadata offset overflows"))?;
    }
    if header.flags & FLAG_HAS_CHECKSUM != 0 && !checksum_seen {
        return Err(MusicError::Invalid(
            "WavPack block declares a checksum but none is present",
        ));
    }
    Ok(())
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

/// Incremental decoder for a bounded sequence of supported WavPack blocks.
/// Returns one decoded block at a time and retains no whole-file PCM buffer.
/// The currently verified profiles are lossless mono/stereo packed 16-bit,
/// packed 24-bit, and one padded-24-bit layout; external trailing tags fail
/// closed.
pub struct WavPackDecoder<R> {
    input: R,
    sequence: BlockSequence,
    metadata_blocks_in_gap: usize,
    finished: bool,
    failed: bool,
}

impl<R: Read> WavPackDecoder<R> {
    pub fn new(input: R) -> Self {
        Self {
            input,
            sequence: BlockSequence::default(),
            metadata_blocks_in_gap: 0,
            finished: false,
            failed: false,
        }
    }

    /// Decode the next audio block. Metadata-only blocks are skipped, with a
    /// fixed cap per gap. Any framing, sequence, profile, entropy or checksum
    /// error poisons the decoder; it cannot resume after malformed input.
    pub fn next_block(&mut self) -> Result<Option<(BlockHeader, Vec<i32>)>> {
        if self.failed {
            return Err(MusicError::Invalid("WavPack stream decoder has failed"));
        }
        if self.finished {
            return Ok(None);
        }

        loop {
            match read_block(&mut self.input) {
                Ok(Some((header, block))) => {
                    if let Err(error) = self.sequence.accept(&header) {
                        self.failed = true;
                        return Err(error);
                    }
                    if header.block_samples == 0 {
                        self.metadata_blocks_in_gap += 1;
                        if self.metadata_blocks_in_gap > MAX_METADATA_BLOCKS_PER_GAP {
                            self.failed = true;
                            return Err(MusicError::Limit(
                                "WavPack metadata-only block gap exceeds limit",
                            ));
                        }
                        continue;
                    }
                    self.metadata_blocks_in_gap = 0;
                    match decode_lossless_block(&block) {
                        Ok(decoded) => return Ok(Some(decoded)),
                        Err(error) => {
                            self.failed = true;
                            return Err(error);
                        }
                    }
                }
                Ok(None) => match self.sequence.finish() {
                    Ok(()) => {
                        self.finished = true;
                        return Ok(None);
                    }
                    Err(error) => {
                        self.failed = true;
                        return Err(error);
                    }
                },
                Err(error) => {
                    self.failed = true;
                    return Err(error);
                }
            }
        }
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

    fn checked_block(checksum_bytes: usize) -> Vec<u8> {
        assert!(matches!(checksum_bytes, 2 | 4));
        let mut bytes = block(1, FLAG_INITIAL_BLOCK | FLAG_FINAL_BLOCK | FLAG_HAS_CHECKSUM);
        bytes.extend_from_slice(&[0, 1, 0x34, 0x12]); // bounded dummy metadata
        let checksum_metadata_bytes = 2 + checksum_bytes;
        let complete_len = bytes.len() + checksum_metadata_bytes;
        bytes[4..8].copy_from_slice(&((complete_len as u32) - 8).to_le_bytes());
        let mut checksum = u32::MAX;
        for word in bytes.chunks_exact(2) {
            checksum = checksum
                .wrapping_mul(3)
                .wrapping_add(u32::from(u16::from_le_bytes([word[0], word[1]])));
        }
        bytes.extend_from_slice(&[0x2f, (checksum_bytes / 2) as u8]);
        if checksum_bytes == 2 {
            checksum ^= checksum >> 16;
            bytes.extend_from_slice(&(checksum as u16).to_le_bytes());
        } else {
            bytes.extend_from_slice(&checksum.to_le_bytes());
        }
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
    fn positive_term_weights_are_not_clamped_at_unit_gain() {
        let mut positive = 1024;
        update_weight(&mut positive, 1, 1, 1);
        assert_eq!(positive, 1025);

        let mut negative = -1024;
        update_weight(&mut negative, 1, 1, -1);
        assert_eq!(negative, -1025);

        let mut unchanged = 1024;
        update_weight(&mut unchanged, 7, 0, 1);
        assert_eq!(unchanged, 1024);
    }

    #[test]
    fn only_verified_terms_and_exact_history_lengths_are_admitted() {
        assert!(DecorrelationState::parse(&[7], &[], &[0; 4], 1).is_ok());
        assert!(DecorrelationState::parse(&[8], &[], &[0; 6], 1).is_ok());
        assert!(DecorrelationState::parse(&[9], &[], &[0; 8], 1).is_ok());
        assert!(DecorrelationState::parse(&[10], &[], &[0; 10], 1).is_ok());
        assert!(DecorrelationState::parse(&[11], &[], &[0; 12], 1).is_ok());
        assert!(DecorrelationState::parse(&[12], &[], &[0; 14], 1).is_ok());
        assert!(DecorrelationState::parse(&[13], &[], &[0; 16], 1).is_ok());
        assert!(DecorrelationState::parse(&[8], &[], &[0; 4], 1).is_err());
        assert!(DecorrelationState::parse(&[4], &[], &[0; 4], 2).is_ok());
        assert!(DecorrelationState::parse(&[3], &[], &[0; 4], 2).is_ok());
        assert!(DecorrelationState::parse(&[4], &[], &[0; 2], 2).is_err());
        assert!(DecorrelationState::parse(&[4], &[], &[0; 4], 1).is_err());
        assert!(DecorrelationState::parse(&[6], &[], &[], 1).is_err());
        assert!(DecorrelationState::parse(&[14], &[], &[], 1).is_err());
    }

    #[test]
    fn padded_24_bit_profile_requires_exact_int32_info_shape() {
        assert!(validate_int32_info(Some(&[0, 8, 0, 0]), true).is_ok());
        assert!(matches!(
            validate_int32_info(None, true),
            Err(MusicError::Invalid(
                "WavPack extended integer metadata is missing"
            ))
        ));
        assert!(matches!(
            validate_int32_info(Some(&[1, 7, 0, 0]), true),
            Err(MusicError::Unsupported(
                "WavPack extended integer layout is unsupported"
            ))
        ));
        assert!(matches!(
            validate_int32_info(Some(&[0, 8, 0, 0]), false),
            Err(MusicError::Invalid(
                "WavPack extended integer metadata is unexpected"
            ))
        ));
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

    #[test]
    fn decodes_ffmpeg_generated_mono_pcm_and_header_checksum() {
        let fixture = include_bytes!("../tests/fixtures/ffmpeg-synthetic-mono16.wv");
        let (header, block) = read_block(&mut Cursor::new(fixture.as_slice()))
            .unwrap()
            .unwrap();
        let (decoded_header, samples) = decode_lossless_block(&block).unwrap();
        assert_eq!(decoded_header, header);
        assert_eq!(samples.len(), header.block_samples as usize);

        let reference = include_bytes!("../tests/fixtures/ffmpeg-synthetic-mono16.pcm");
        let expected = reference
            .chunks_exact(2)
            .map(|sample| i32::from(i16::from_le_bytes([sample[0], sample[1]])))
            .collect::<Vec<_>>();
        assert_eq!(samples, expected);
    }

    #[test]
    fn decodes_contiguous_ffmpeg_multiblock_stream_incrementally() {
        let fixture = include_bytes!("../tests/fixtures/ffmpeg-synthetic-multiblock16.wv");
        let reference = include_bytes!("../tests/fixtures/ffmpeg-synthetic-multiblock16.pcm");
        let mut decoder = WavPackDecoder::new(Cursor::new(fixture.as_slice()));
        let mut decoded = Vec::new();
        let mut decoded_frames = 0u64;
        let mut blocks = 0usize;

        while let Some((header, samples)) = decoder.next_block().unwrap() {
            assert_eq!(header.block_index, decoded_frames);
            assert_eq!(samples.len(), header.block_samples as usize);
            decoded_frames += u64::from(header.block_samples);
            blocks += 1;
            decoded.extend(samples);
        }
        assert_eq!(blocks, 5);
        assert_eq!(decoded_frames, 4_410);
        assert!(decoder.next_block().unwrap().is_none());

        let expected = reference
            .chunks_exact(2)
            .map(|sample| i32::from(i16::from_le_bytes([sample[0], sample[1]])))
            .collect::<Vec<_>>();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn multiblock_stereo_decoder_matches_pcm_when_mode_changes_between_blocks() {
        let fixture = include_bytes!("../tests/fixtures/ffmpeg-synthetic-multiblock-stereo16.wv");
        let reference =
            include_bytes!("../tests/fixtures/ffmpeg-synthetic-multiblock-stereo16.pcm");
        let mut decoder = WavPackDecoder::new(Cursor::new(fixture.as_slice()));
        let mut decoded = Vec::new();
        let mut decoded_frames = 0usize;
        let mut blocks = 0;
        while let Some((header, samples)) = decoder.next_block().unwrap() {
            assert_eq!(header.block_index as usize, decoded_frames);
            decoded_frames += header.block_samples as usize;
            blocks += 1;
            decoded.extend(samples);
        }
        assert_eq!(blocks, 5);
        assert_eq!(decoded_frames, 4_410);
        let expected = reference
            .chunks_exact(2)
            .map(|sample| i32::from(i16::from_le_bytes([sample[0], sample[1]])))
            .collect::<Vec<_>>();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn decodes_ffmpeg_true_stereo_multiblock_pcm_exactly() {
        let fixture =
            include_bytes!("../tests/fixtures/ffmpeg-synthetic-multiblock-stereo-nojoint16.wv");
        let reference =
            include_bytes!("../tests/fixtures/ffmpeg-synthetic-multiblock-stereo16.pcm");
        let mut decoder = WavPackDecoder::new(Cursor::new(fixture.as_slice()));
        let mut decoded = Vec::new();
        while let Some((header, samples)) = decoder.next_block().unwrap() {
            assert_eq!(header.channels(), 2);
            decoded.extend(samples);
        }
        let expected = reference
            .chunks_exact(2)
            .map(|sample| i32::from(i16::from_le_bytes([sample[0], sample[1]])))
            .collect::<Vec<_>>();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn decodes_deterministic_noise_stereo_multiblock_pcm_exactly() {
        let fixture = include_bytes!("../tests/fixtures/ffmpeg-synthetic-random-stereo16.wv");
        let reference = include_bytes!("../tests/fixtures/ffmpeg-synthetic-random-stereo16.pcm");
        let mut decoder = WavPackDecoder::new(Cursor::new(fixture.as_slice()));
        let mut decoded = Vec::new();
        let mut frames = 0usize;
        while let Some((header, samples)) = decoder.next_block().unwrap() {
            assert_eq!(header.block_index as usize, frames);
            assert_eq!(header.channels(), 2);
            frames += header.block_samples as usize;
            decoded.extend(samples);
        }
        assert_eq!(frames, 4_410);
        let expected = reference
            .chunks_exact(2)
            .map(|sample| i32::from(i16::from_le_bytes([sample[0], sample[1]])))
            .collect::<Vec<_>>();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn decodes_padded_24_bit_stereo_blocks_to_pcm_exactly() {
        let fixture = include_bytes!("../tests/fixtures/ffmpeg-synthetic-stereo24.wv");
        let reference = include_bytes!("../tests/fixtures/ffmpeg-synthetic-stereo24.pcm");
        let mut decoder = WavPackDecoder::new(Cursor::new(fixture.as_slice()));
        let mut decoded = Vec::new();
        let mut frames = 0usize;
        while let Some((header, samples)) = decoder.next_block().unwrap() {
            assert_eq!(header.channels(), 2);
            frames += header.block_samples as usize;
            decoded.extend(samples);
        }
        assert_eq!(frames, 4_410);
        let expected = reference
            .chunks_exact(3)
            .map(|sample| {
                let sign_extension = if sample[2] & 0x80 == 0 { 0 } else { 0xff };
                i32::from_le_bytes([sample[0], sample[1], sample[2], sign_extension])
            })
            .collect::<Vec<_>>();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn decodes_padded_24_bit_mono_blocks_to_pcm_exactly() {
        let fixture = include_bytes!("../tests/fixtures/ffmpeg-synthetic-mono24.wv");
        let reference = include_bytes!("../tests/fixtures/ffmpeg-synthetic-mono24.pcm");
        let mut decoder = WavPackDecoder::new(Cursor::new(fixture.as_slice()));
        let mut decoded = Vec::new();
        let mut frames = 0usize;
        while let Some((header, samples)) = decoder.next_block().unwrap() {
            assert_eq!(header.channels(), 1);
            frames += header.block_samples as usize;
            decoded.extend(samples);
        }
        assert_eq!(frames, 4_410);
        let expected = reference
            .chunks_exact(3)
            .map(|sample| {
                let sign_extension = if sample[2] & 0x80 == 0 { 0 } else { 0xff };
                i32::from_le_bytes([sample[0], sample[1], sample[2], sign_extension])
            })
            .collect::<Vec<_>>();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn decodes_deterministic_noise_padded_24_bit_stereo_blocks_exactly() {
        let fixture = include_bytes!("../tests/fixtures/ffmpeg-synthetic-random-stereo24.wv");
        let reference = include_bytes!("../tests/fixtures/ffmpeg-synthetic-random-stereo24.pcm");
        let mut decoder = WavPackDecoder::new(Cursor::new(fixture.as_slice()));
        let mut decoded = Vec::new();
        let mut frames = 0usize;
        while let Some((header, samples)) = decoder.next_block().unwrap() {
            assert_eq!(header.channels(), 2);
            frames += header.block_samples as usize;
            decoded.extend(samples);
        }
        assert_eq!(frames, 4_410);
        let expected = reference
            .chunks_exact(3)
            .map(|sample| {
                let sign_extension = if sample[2] & 0x80 == 0 { 0 } else { 0xff };
                i32::from_le_bytes([sample[0], sample[1], sample[2], sign_extension])
            })
            .collect::<Vec<_>>();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn decodes_tiny_encoder_440_random_24_bit_stereo_pcm_exactly() {
        let fixture = include_bytes!("../tests/fixtures/tiny-encoder-4.40-random-stereo24.wv");
        let reference = include_bytes!("../tests/fixtures/ffmpeg-synthetic-random-stereo24.pcm");
        let mut decoder = WavPackDecoder::new(Cursor::new(fixture.as_slice()));
        let mut decoded = Vec::new();
        let mut frames = 0usize;
        while let Some((header, samples)) = decoder.next_block().unwrap() {
            assert_eq!(header.channels(), 2);
            frames += header.block_samples as usize;
            decoded.extend(samples);
        }
        assert_eq!(frames, 4_410);
        let expected = reference
            .chunks_exact(3)
            .map(|sample| {
                let sign_extension = if sample[2] & 0x80 == 0 { 0 } else { 0xff };
                i32::from_le_bytes([sample[0], sample[1], sample[2], sign_extension])
            })
            .collect::<Vec<_>>();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn decodes_tiny_encoder_440_random_16_bit_stereo_pcm_exactly() {
        let fixture = include_bytes!("../tests/fixtures/tiny-encoder-4.40-random-stereo16.wv");
        let reference = include_bytes!("../tests/fixtures/ffmpeg-synthetic-random-stereo16.pcm");
        let mut decoder = WavPackDecoder::new(Cursor::new(fixture.as_slice()));
        let mut decoded = Vec::new();
        let mut frames = 0usize;
        while let Some((header, samples)) = decoder.next_block().unwrap() {
            assert_eq!(header.channels(), 2);
            frames += header.block_samples as usize;
            decoded.extend(samples);
        }
        assert_eq!(frames, 4_410);
        let expected = reference
            .chunks_exact(2)
            .map(|sample| i32::from(i16::from_le_bytes([sample[0], sample[1]])))
            .collect::<Vec<_>>();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn decodes_tiny_encoder_440_default_mode_lag_terms_exactly() {
        let fixture =
            include_bytes!("../tests/fixtures/tiny-encoder-4.40-random-default-stereo16.wv");
        let reference =
            include_bytes!("../tests/fixtures/tiny-encoder-4.40-random-default-stereo16.pcm");

        let mut probe = Cursor::new(fixture.as_slice());
        let (_, first_block) = read_block(&mut probe).unwrap().unwrap();
        let terms = iter_metadata(&first_block)
            .unwrap()
            .find_map(|metadata| {
                let metadata = metadata.unwrap();
                (metadata.id == ID_DECORRELATION_TERMS).then_some(metadata.data.to_vec())
            })
            .unwrap();
        assert_eq!(
            terms
                .iter()
                .map(|term| (term & 0x1f) as i8 - 5)
                .collect::<Vec<_>>(),
            [18, 18, 2, 17, 3]
        );

        let mut decoder = WavPackDecoder::new(Cursor::new(fixture.as_slice()));
        let mut decoded = Vec::new();
        let mut frames = 0usize;
        let mut blocks = 0usize;
        while let Some((header, samples)) = decoder.next_block().unwrap() {
            assert_eq!(header.channels(), 2);
            frames += header.block_samples as usize;
            blocks += 1;
            decoded.extend(samples);
        }
        assert_eq!(frames, 20_000);
        assert_eq!(blocks, 2);
        let expected = reference
            .chunks_exact(2)
            .map(|sample| i32::from(i16::from_le_bytes([sample[0], sample[1]])))
            .collect::<Vec<_>>();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn decodes_tiny_encoder_440_high_mode_cross_channel_predictors_exactly() {
        let fixture = include_bytes!("../tests/fixtures/tiny-encoder-4.40-random-high-stereo16.wv");
        let reference =
            include_bytes!("../tests/fixtures/tiny-encoder-4.40-random-default-stereo16.pcm");

        let mut probe = Cursor::new(fixture.as_slice());
        let (_, first_block) = read_block(&mut probe).unwrap().unwrap();
        let terms = iter_metadata(&first_block)
            .unwrap()
            .find_map(|metadata| {
                let metadata = metadata.unwrap();
                (metadata.id == ID_DECORRELATION_TERMS).then_some(metadata.data.to_vec())
            })
            .unwrap();
        assert_eq!(
            terms
                .iter()
                .map(|term| (term & 0x1f) as i8 - 5)
                .collect::<Vec<_>>(),
            [18, 18, 18, -2, 2, 3, 5, -1, 17, 4]
        );

        let mut decoder = WavPackDecoder::new(Cursor::new(fixture.as_slice()));
        let mut decoded = Vec::new();
        let mut frames = 0usize;
        let mut blocks = 0usize;
        while let Some((header, samples)) = decoder.next_block().unwrap() {
            assert_eq!(header.channels(), 2);
            frames += header.block_samples as usize;
            blocks += 1;
            decoded.extend(samples);
        }
        assert_eq!(frames, 20_000);
        assert_eq!(blocks, 2);
        let expected = reference
            .chunks_exact(2)
            .map(|sample| i32::from(i16::from_le_bytes([sample[0], sample[1]])))
            .collect::<Vec<_>>();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn decodes_tiny_encoder_440_very_high_profile_exactly() {
        let fixture =
            include_bytes!("../tests/fixtures/tiny-encoder-4.40-random-veryhigh-stereo16.wv");
        let reference =
            include_bytes!("../tests/fixtures/tiny-encoder-4.40-random-default-stereo16.pcm");
        let mut decoder = WavPackDecoder::new(Cursor::new(fixture.as_slice()));
        let mut decoded = Vec::new();
        let mut frames = 0usize;
        while let Some((header, samples)) = decoder.next_block().unwrap() {
            assert_eq!(header.channels(), 2);
            frames += header.block_samples as usize;
            decoded.extend(samples);
        }
        assert_eq!(frames, 20_000);
        let expected = reference
            .chunks_exact(2)
            .map(|sample| i32::from(i16::from_le_bytes([sample[0], sample[1]])))
            .collect::<Vec<_>>();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn decodes_long_synthetic_24_bit_stereo_pcm_exactly() {
        let fixture = include_bytes!("../tests/fixtures/ffmpeg-synthetic-long-stereo24.wv");
        let reference = include_bytes!("../tests/fixtures/ffmpeg-synthetic-long-stereo24.pcm");
        let mut decoder = WavPackDecoder::new(Cursor::new(fixture.as_slice()));
        let mut frames = 0usize;
        let mut pcm_offset = 0usize;
        while let Some((header, samples)) = decoder.next_block().unwrap() {
            assert_eq!(header.channels(), 2);
            frames += header.block_samples as usize;
            for sample in samples {
                let encoded = reference
                    .get(pcm_offset..pcm_offset + 3)
                    .expect("PCM golden contains every decoded sample");
                let sign_extension = if encoded[2] & 0x80 == 0 { 0 } else { 0xff };
                let expected =
                    i32::from_le_bytes([encoded[0], encoded[1], encoded[2], sign_extension]);
                assert_eq!(sample, expected);
                pcm_offset += 3;
            }
        }
        assert_eq!(frames, 1_063_147);
        assert_eq!(pcm_offset, reference.len());
    }

    #[test]
    fn multiblock_decoder_fails_closed_on_truncated_tail_and_cannot_resume() {
        let fixture = include_bytes!("../tests/fixtures/ffmpeg-synthetic-multiblock16.wv");
        let truncated = &fixture[..fixture.len() - 1];
        let mut decoder = WavPackDecoder::new(Cursor::new(truncated));
        let mut audio_blocks = 0;
        loop {
            match decoder.next_block() {
                Ok(Some(_)) => audio_blocks += 1,
                Err(MusicError::Io(error)) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                    break
                }
                other => panic!("unexpected truncated stream result: {other:?}"),
            }
        }
        assert_eq!(audio_blocks, 4);
        assert!(matches!(
            decoder.next_block(),
            Err(MusicError::Invalid("WavPack stream decoder has failed"))
        ));
    }

    #[test]
    fn multiblock_decoder_rejects_a_gap_before_decoding_the_next_block() {
        let fixture = include_bytes!("../tests/fixtures/ffmpeg-synthetic-multiblock16.wv");
        let mut corrupted = fixture.to_vec();
        let first_block_len = u32::from_le_bytes(corrupted[4..8].try_into().unwrap()) as usize + 8;
        let second_index = first_block_len + 16;
        let original_index = u32::from_le_bytes(
            corrupted[second_index..second_index + 4]
                .try_into()
                .unwrap(),
        );
        corrupted[second_index..second_index + 4]
            .copy_from_slice(&(original_index + 1).to_le_bytes());

        let mut decoder = WavPackDecoder::new(Cursor::new(corrupted));
        assert!(decoder.next_block().unwrap().is_some());
        assert!(matches!(
            decoder.next_block(),
            Err(MusicError::Invalid(
                "WavPack audio blocks are not contiguous"
            ))
        ));
        assert!(matches!(
            decoder.next_block(),
            Err(MusicError::Invalid("WavPack stream decoder has failed"))
        ));
    }

    #[test]
    fn decodes_ffmpeg_generated_stereo_residual_words() {
        let fixture = include_bytes!("../tests/fixtures/ffmpeg-synthetic-stereo16.wv");
        let (header, block) = read_block(&mut Cursor::new(fixture.as_slice()))
            .unwrap()
            .unwrap();
        assert_eq!(header.total_samples, Some(882));
        assert_eq!(header.block_index, 0);
        assert_eq!(header.block_samples, 882);
        assert_eq!(header.channels(), 2);
        header.validate_integer_mono_stereo().unwrap();

        let metadata = iter_metadata(&block)
            .unwrap()
            .map(|entry| entry.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            metadata.iter().map(|entry| entry.id).collect::<Vec<_>>(),
            [2, 3, 4, 5, 10]
        );

        let entropy = parse_entropy_variables(metadata[3].data, 2).unwrap();
        assert_ne!(entropy.medians[0], entropy.medians[1]);
        assert_eq!(entropy.channels, 2);
        let mut decoder = WordDecoder::new(entropy).unwrap();
        let mut bits = BitReader::new(metadata[4].data);
        let mut residuals = decoder
            .decode_frames(&mut bits, header.block_samples)
            .unwrap();
        let decoded_residuals = residuals.clone();
        assert_eq!(residuals.len(), header.block_samples as usize * 2);
        assert_eq!(decoder.decoded_words(), header.block_samples * 2);
        assert!(residuals.chunks_exact(2).any(|frame| frame[0] != frame[1]));

        let mut decorrelation = DecorrelationState::parse(
            metadata[0].data,
            metadata[1].data,
            metadata[2].data,
            header.channels(),
        )
        .unwrap();
        assert_eq!(
            decorrelation
                .passes()
                .iter()
                .map(|pass| pass.term)
                .collect::<Vec<_>>(),
            [17, 18]
        );
        decorrelation.reconstruct(&mut residuals).unwrap();
        restore_joint_stereo(&mut residuals).unwrap();

        let reference = include_bytes!("../tests/fixtures/ffmpeg-synthetic-stereo16.pcm");
        let expected = reference
            .chunks_exact(2)
            .map(|sample| i32::from(i16::from_le_bytes([sample[0], sample[1]])))
            .collect::<Vec<_>>();
        let mut encoded_pcm = expected.clone();
        for frame in encoded_pcm.chunks_exact_mut(2) {
            let left = frame[0];
            let right = frame[1];
            let first = left - right;
            frame[0] = first;
            frame[1] = right + (first >> 1);
        }
        let mut encoder_state = DecorrelationState::parse(
            metadata[0].data,
            metadata[1].data,
            metadata[2].data,
            header.channels(),
        )
        .unwrap();
        for pass in encoder_state.passes.iter_mut().rev() {
            for frame in encoded_pcm.chunks_exact_mut(2) {
                for (channel, residual_out) in frame.iter_mut().enumerate() {
                    let history = &mut pass.history[channel];
                    let predictor = if pass.term == 17 {
                        history[0].wrapping_mul(2).wrapping_sub(history[1])
                    } else {
                        history[0].wrapping_mul(3).wrapping_sub(history[1]) >> 1
                    };
                    let sample = *residual_out;
                    let residual = sample - apply_weight(pass.weights[channel], predictor);
                    update_weight(&mut pass.weights[channel], pass.delta, predictor, residual);
                    history[1] = history[0];
                    history[0] = sample;
                    *residual_out = residual;
                }
            }
        }
        let residual_mismatch = decoded_residuals
            .iter()
            .zip(&encoded_pcm)
            .position(|(actual, reference)| actual != reference);
        assert!(
            residual_mismatch.is_none(),
            "first residual mismatch: {:?}",
            residual_mismatch.map(|index| (index, decoded_residuals[index], encoded_pcm[index]))
        );

        let mismatch = residuals
            .iter()
            .zip(&expected)
            .position(|(actual, reference)| actual != reference);
        assert!(
            mismatch.is_none(),
            "first PCM mismatch: {:?}",
            mismatch.map(|index| (index, residuals[index], expected[index]))
        );
        verify_decoded_pcm_checksum(&header, &residuals).unwrap();
        let mut corrupted_pcm = residuals.clone();
        corrupted_pcm[0] ^= 1;
        assert!(matches!(
            verify_decoded_pcm_checksum(&header, &corrupted_pcm),
            Err(MusicError::Invalid("WavPack decoded PCM checksum mismatch"))
        ));
        assert!(matches!(
            verify_decoded_pcm_checksum(&header, &residuals[..residuals.len() - 1]),
            Err(MusicError::Invalid(
                "WavPack decoded sample count does not match block header"
            ))
        ));
        let (decoded_header, decoded_pcm) = decode_lossless_block(&block).unwrap();
        assert_eq!(decoded_header, header);
        assert_eq!(decoded_pcm, residuals);
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
        // Wire-order equivalents of Table 7.32, packed LSB-first on the stream.
        let tables: &[(u32, &[&str])] = &[
            (6, &["00", "10", "010", "011", "110", "111"]),
            (7, &["00", "100", "101", "010", "011", "110", "111"]),
            (8, &["000", "001", "100", "101", "010", "011", "110", "111"]),
            (
                9,
                &[
                    "000", "100", "010", "110", "001", "101", "011", "1110", "1111",
                ],
            ),
            (
                10,
                &[
                    "000", "100", "010", "110", "001", "101", "0110", "0111", "1110", "1111",
                ],
            ),
            (
                11,
                &[
                    "000", "100", "010", "110", "001", "1010", "1011", "0110", "0111", "1110",
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
                    "range {range}, LSB-first code {code}"
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

    #[test]
    fn validates_wavpack_v5_16_and_32_bit_encoded_block_checksums() {
        for width in [2, 4] {
            let bytes = checked_block(width);
            assert!(read_block(&mut Cursor::new(bytes.clone())).is_ok());

            let mut corrupted = bytes;
            corrupted[34] ^= 1; // alter dummy metadata covered by the checksum
            assert!(matches!(
                read_block(&mut Cursor::new(corrupted)),
                Err(MusicError::Invalid(
                    "WavPack encoded-block checksum mismatch"
                ))
            ));
        }
    }

    #[test]
    fn checksum_flag_requires_checksum_metadata() {
        let mut bytes = block(1, FLAG_INITIAL_BLOCK | FLAG_FINAL_BLOCK | FLAG_HAS_CHECKSUM);
        bytes.extend_from_slice(&[0, 1, 0x34, 0x12]);
        let chunk_size = (bytes.len() as u32) - 8;
        bytes[4..8].copy_from_slice(&chunk_size.to_le_bytes());
        assert!(matches!(
            read_block(&mut Cursor::new(bytes)),
            Err(MusicError::Invalid(
                "WavPack block declares a checksum but none is present"
            ))
        ));
    }
}
