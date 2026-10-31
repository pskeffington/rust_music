//! Small, dependency-free audio types shared by independent Rust Music modules.
//! Sample values are signed, interleaved and *not* converted to float by default.

use std::{error::Error, fmt, io};

/// Errors are deliberate and actionable; malformed input never triggers a panic.
#[derive(Debug)]
pub enum MusicError {
    Io(io::Error),
    Cancelled(&'static str),
    Invalid(&'static str),
    Unsupported(&'static str),
    Limit(&'static str),
    Output(String),
}

impl fmt::Display for MusicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O: {e}"),
            Self::Cancelled(s) => write!(f, "cancelled: {s}"),
            Self::Invalid(s) => write!(f, "invalid audio: {s}"),
            Self::Unsupported(s) => write!(f, "unsupported audio: {s}"),
            Self::Limit(s) => write!(f, "audio resource limit: {s}"),
            Self::Output(s) => write!(f, "audio output: {s}"),
        }
    }
}
impl Error for MusicError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}
impl From<io::Error> for MusicError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

pub type Result<T> = std::result::Result<T, MusicError>;

/// Native integer PCM description. Validated before allocation or output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u8,
    pub bits_per_sample: u8,
}

impl AudioFormat {
    pub fn new(sample_rate: u32, channels: u8, bits_per_sample: u8) -> Result<Self> {
        let format = Self {
            sample_rate,
            channels,
            bits_per_sample,
        };
        format.validate()?;
        Ok(format)
    }
    /// Validate even formats built with a struct literal through this public API.
    pub fn validate(self) -> Result<()> {
        if !(1..=8).contains(&self.channels) {
            return Err(MusicError::Unsupported("1..8 channels required"));
        }
        if !(4..=32).contains(&self.bits_per_sample) {
            return Err(MusicError::Unsupported("4..32 bit PCM required"));
        }
        if !(1..=1_048_575).contains(&self.sample_rate) {
            return Err(MusicError::Unsupported("invalid sample rate"));
        }
        Ok(())
    }
    /// Listening policy only, separate from decoder test-vector support.
    /// Lossless CD-quality (16-bit / 44.1 kHz) is the playback floor.
    pub fn meets_audiophile_floor(self) -> bool {
        self.bits_per_sample >= 16 && self.sample_rate >= 44_100
    }
}

/// Exact integer samples in interleaved frame order: L, R, L, R, ...
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcmBlock {
    pub format: AudioFormat,
    pub samples: Vec<i32>,
}
impl PcmBlock {
    pub fn new(format: AudioFormat, samples: Vec<i32>) -> Result<Self> {
        let block = Self { format, samples };
        block.validate()?;
        Ok(block)
    }
    /// Check public fields before frame arithmetic or output conversion.
    pub fn validate(&self) -> Result<()> {
        self.format.validate()?;
        let channels = usize::from(self.format.channels);
        if self.samples.len() % channels != 0 {
            return Err(MusicError::Invalid("incomplete interleaved PCM frame"));
        }
        if self.samples.len() / channels > 65_535 {
            return Err(MusicError::Limit(
                "PCM block exceeds maximum decoded frame size",
            ));
        }
        let bits = self.format.bits_per_sample;
        if bits < 32 {
            let magnitude = 1i64 << (bits - 1);
            let minimum = -magnitude;
            let maximum = magnitude - 1;
            if self
                .samples
                .iter()
                .any(|&sample| i64::from(sample) < minimum || i64::from(sample) > maximum)
            {
                return Err(MusicError::Invalid(
                    "PCM sample exceeds declared integer width",
                ));
            }
        }
        Ok(())
    }
    pub fn frames(&self) -> usize {
        self.samples
            .len()
            .checked_div(usize::from(self.format.channels))
            .unwrap_or_default()
    }
}

/// One decoded audio block at a time; no output device or GUI is implied.
pub trait Decoder: Send {
    fn format(&self) -> AudioFormat;
    /// Container-declared PCM sample frames per channel, not a proof that the
    /// entire file is present or valid. None means unknown, never an estimate.
    fn total_frames(&self) -> Option<u64> {
        None
    }
    /// Whether this decoder can reposition to an exact PCM sample frame.
    fn supports_seek(&self) -> bool {
        false
    }
    /// Position the next decoded block at `frame`. Unsupported by default.
    fn seek_frame(&mut self, _frame: u64) -> Result<()> {
        Err(MusicError::Unsupported(
            "sample-accurate seek is unsupported",
        ))
    }
    fn next_block(&mut self) -> Result<Option<PcmBlock>>;
}

/// Host-specific output must explicitly accept the exact source PCM format.
/// A sink must never silently resample or apply DSP in bit-perfect mode.
pub trait PcmSink {
    fn open(&mut self, format: AudioFormat) -> Result<()>;
    fn write(&mut self, block: &PcmBlock) -> Result<()>;
    fn finish(&mut self) -> Result<()>;
}

/// Deterministic sink for decoding tests; it never asserts that a physical device played audio.
#[derive(Debug, Default)]
pub struct NullSink {
    pub frames_written: u64,
    pub opened: Option<AudioFormat>,
}
impl PcmSink for NullSink {
    fn open(&mut self, format: AudioFormat) -> Result<()> {
        format.validate()?;
        self.opened = Some(format);
        Ok(())
    }
    fn write(&mut self, block: &PcmBlock) -> Result<()> {
        block.validate()?;
        if self.opened != Some(block.format) {
            return Err(MusicError::Invalid("sink format mismatch"));
        }
        self.frames_written = self
            .frames_written
            .checked_add(block.frames() as u64)
            .ok_or(MusicError::Limit("frame count overflow"))?;
        Ok(())
    }
    fn finish(&mut self) -> Result<()> {
        self.opened = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audio_format_rejects_bogus_inputs() {
        assert!(AudioFormat::new(44_100, 0, 16).is_err());
        assert!(AudioFormat::new(0, 2, 16).is_err());
        assert!(AudioFormat::new(44_100, 2, 33).is_err());
        assert!(AudioFormat::new(192_000, 2, 32).is_ok());
    }
    #[test]
    fn audiophile_floor_accepts_cd_and_hires_but_not_low_rate_pcm() {
        assert!(AudioFormat::new(44_100, 2, 16)
            .unwrap()
            .meets_audiophile_floor());
        assert!(AudioFormat::new(192_000, 2, 24)
            .unwrap()
            .meets_audiophile_floor());
        assert!(!AudioFormat::new(48_000, 1, 8)
            .unwrap()
            .meets_audiophile_floor());
        assert!(!AudioFormat::new(32_000, 2, 24)
            .unwrap()
            .meets_audiophile_floor());
    }
    #[test]
    fn public_pcm_blocks_reject_invalid_formats_and_out_of_range_samples() {
        let zero_channels = AudioFormat {
            sample_rate: 44_100,
            channels: 0,
            bits_per_sample: 16,
        };
        assert!(PcmBlock::new(zero_channels, vec![]).is_err());
        let malformed = PcmBlock {
            format: zero_channels,
            samples: vec![1],
        };
        assert_eq!(malformed.frames(), 0);
        assert!(malformed.validate().is_err());

        let sixteen = AudioFormat::new(44_100, 1, 16).unwrap();
        assert!(PcmBlock::new(sixteen, vec![i16::MIN.into(), i16::MAX.into()]).is_ok());
        assert!(PcmBlock::new(sixteen, vec![i32::from(i16::MAX) + 1]).is_err());

        let twenty_four = AudioFormat::new(48_000, 1, 24).unwrap();
        assert!(PcmBlock::new(twenty_four, vec![-8_388_608, 8_388_607]).is_ok());
        assert!(PcmBlock::new(twenty_four, vec![8_388_608]).is_err());
    }

    #[test]
    fn null_sink_checks_format_and_frame_count() {
        let f = AudioFormat::new(48_000, 2, 24).unwrap();
        let mut s = NullSink::default();
        assert!(s.write(&PcmBlock::new(f, vec![1, 2]).unwrap()).is_err());
        s.open(f).unwrap();
        s.write(&PcmBlock::new(f, vec![1, 2, 3, 4]).unwrap())
            .unwrap();
        assert_eq!(s.frames_written, 2);
    }
}
