//! Public Rust Music module: small, headless, offline, host-controlled playback.
//! All decoders are locally authored. No third-party runtime crates.

use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

pub use rust_music_aiff::AiffDecoder;
pub use rust_music_core::{AudioFormat, Decoder, MusicError, NullSink, PcmBlock, PcmSink, Result};
pub use rust_music_engine::{PlaybackSnapshot, PlaybackState, Player};
pub use rust_music_flac::{FlacDecoder, StreamInfo};
pub use rust_music_wave::WaveDecoder;

/// Sniff content, not filename. No audio starts during file opening.
pub fn open_file(path: impl AsRef<Path>) -> Result<Box<dyn Decoder>> {
    let mut input = BufReader::new(File::open(path)?);
    let head = input.fill_buf()?;
    if head.starts_with(b"fLaC") {
        Ok(Box::new(FlacDecoder::new(input)?))
    } else if head.len() >= 12 && head.starts_with(b"RIFF") && &head[8..12] == b"WAVE" {
        Ok(Box::new(WaveDecoder::new(input)?))
    } else if head.len() >= 12
        && head.starts_with(b"FORM")
        && (&head[8..12] == b"AIFF" || &head[8..12] == b"AIFC")
    {
        Ok(Box::new(AiffDecoder::new(input)?))
    } else if head.len() >= 8 && &head[4..8] == b"ftyp" {
        Err(MusicError::Unsupported("M4A/MP4 may contain lossy AAC; native ALAC verification and decoding is not implemented"))
    } else {
        Err(MusicError::Unsupported(
            "audiophile-only input: native FLAC, integer PCM WAV or uncompressed AIFF/AIFF-C",
        ))
    }
}

#[cfg(all(feature = "native-alsa", target_os = "linux"))]
pub use rust_music_native::AlsaSink;
#[cfg(all(feature = "native-macos", target_os = "macos"))]
pub use rust_music_native::MacAudioQueueSink;

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct Temp(PathBuf);

    impl Temp {
        fn new() -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "rust-music-admission-{}-{stamp}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn write_named(&self, bytes: &[u8]) -> PathBuf {
            let path = self.0.join("lossy-content.flac");
            fs::write(&path, bytes).unwrap();
            path
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn rejects_unsupported_signatures_by_content_even_with_flac_suffix() {
        let temp = Temp::new();
        // These are signature-shaped inputs for content sniffing, not encoded
        // lossless or lossy audio conformance vectors.
        for bytes in [
            b"\x00\x00\x00\x18ftypM4A \x00\x00\x00\x00isom".as_slice(),
            b"ID3\x04\x00\x00\x00\x00\x00\x00".as_slice(),
            b"\xff\xf1\x50\x80\x00\x1f\xfc".as_slice(),
        ] {
            let path = temp.write_named(bytes);
            assert!(matches!(open_file(path), Err(MusicError::Unsupported(_))));
        }
    }
}
