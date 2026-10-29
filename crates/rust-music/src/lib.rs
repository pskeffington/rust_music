//! Public Rust Music module: small, headless, offline, host-controlled playback.
//! All decoders are locally authored. No third-party runtime crates.

use std::{fs::File, io::{BufRead, BufReader}, path::Path};

pub use rust_music_core::{AudioFormat, Decoder, MusicError, NullSink, PcmBlock, PcmSink, Result};
pub use rust_music_engine::{PlaybackSnapshot, PlaybackState, Player};
pub use rust_music_flac::{FlacDecoder, StreamInfo};
pub use rust_music_aiff::AiffDecoder;
pub use rust_music_wave::WaveDecoder;

/// Sniff content, not filename. No audio starts during file opening.
pub fn open_file(path: impl AsRef<Path>) -> Result<Box<dyn Decoder>> {
    let mut input = BufReader::new(File::open(path)?);
    let head = input.fill_buf()?;
    if head.starts_with(b"fLaC") {
        Ok(Box::new(FlacDecoder::new(input)?))
    } else if head.len() >= 12 && head.starts_with(b"RIFF") && &head[8..12] == b"WAVE" {
        Ok(Box::new(WaveDecoder::new(input)?))
    } else if head.len() >= 12 && head.starts_with(b"FORM")
        && (&head[8..12] == b"AIFF" || &head[8..12] == b"AIFC") {
        Ok(Box::new(AiffDecoder::new(input)?))
    } else if head.len() >= 8 && &head[4..8] == b"ftyp" {
        Err(MusicError::Unsupported("M4A/MP4 may contain lossy AAC; native ALAC verification and decoding is not implemented"))
    } else {
        Err(MusicError::Unsupported("audiophile-only input: native FLAC, integer PCM WAV or uncompressed AIFF/AIFF-C"))
    }
}

#[cfg(all(feature = "native-macos", target_os = "macos"))]
pub use rust_music_native::MacAudioQueueSink;
#[cfg(all(feature = "native-alsa", target_os = "linux"))]
pub use rust_music_native::AlsaSink;
