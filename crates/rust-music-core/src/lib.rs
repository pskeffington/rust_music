//! Small, dependency-free audio types shared by independent Rust Music modules.
//! Sample values are signed, interleaved and *not* converted to float by default.

use std::{error::Error, fmt, io};

/// Errors are deliberate and actionable; malformed input never triggers a panic.
#[derive(Debug)]
pub enum MusicError {
    Io(io::Error),
    Invalid(&'static str),
    Unsupported(&'static str),
    Limit(&'static str),
    Output(String),
}

impl fmt::Display for MusicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O: {e}"),
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
    fn from(value: io::Error) -> Self { Self::Io(value) }
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
        if !(1..=8).contains(&channels) { return Err(MusicError::Unsupported("1..8 channels required")); }
        if !(4..=32).contains(&bits_per_sample) { return Err(MusicError::Unsupported("4..32 bit PCM required")); }
        if !(1..=1_048_575).contains(&sample_rate) { return Err(MusicError::Unsupported("invalid sample rate")); }
        Ok(Self { sample_rate, channels, bits_per_sample })
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
        if samples.len() % usize::from(format.channels) != 0 {
            return Err(MusicError::Invalid("incomplete interleaved PCM frame"));
        }
        if samples.len() / usize::from(format.channels) > 65_535 {
            return Err(MusicError::Limit("PCM block exceeds maximum decoded frame size"));
        }
        Ok(Self { format, samples })
    }
    pub fn frames(&self) -> usize { self.samples.len() / usize::from(self.format.channels) }
}

/// One decoded audio block at a time; no output device or GUI is implied.
pub trait Decoder: Send {
    fn format(&self) -> AudioFormat;
    /// Container-declared PCM sample frames per channel, not a proof that the
    /// entire file is present or valid. None means unknown, never an estimate.
    fn total_frames(&self) -> Option<u64> { None }
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
pub struct NullSink { pub frames_written: u64, pub opened: Option<AudioFormat> }
impl PcmSink for NullSink {
    fn open(&mut self, format: AudioFormat) -> Result<()> { self.opened = Some(format); Ok(()) }
    fn write(&mut self, block: &PcmBlock) -> Result<()> {
        if self.opened != Some(block.format) { return Err(MusicError::Invalid("sink format mismatch")); }
        self.frames_written = self.frames_written.checked_add(block.frames() as u64)
            .ok_or(MusicError::Limit("frame count overflow"))?;
        Ok(())
    }
    fn finish(&mut self) -> Result<()> { self.opened = None; Ok(()) }
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
    fn null_sink_checks_format_and_frame_count() {
        let f = AudioFormat::new(48_000, 2, 24).unwrap();
        let mut s = NullSink::default();
        assert!(s.write(&PcmBlock::new(f, vec![1, 2]).unwrap()).is_err());
        s.open(f).unwrap();
        s.write(&PcmBlock::new(f, vec![1, 2, 3, 4]).unwrap()).unwrap();
        assert_eq!(s.frames_written, 2);
    }
}
