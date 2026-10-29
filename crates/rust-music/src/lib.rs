//! Public Rust Music module: small, headless, offline, host-controlled playback.
//! All decoders are locally authored. No third-party runtime crates.

use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

pub use rust_music_aiff::AiffDecoder;
pub use rust_music_caf::CafDecoder;
pub use rust_music_core::{AudioFormat, Decoder, MusicError, NullSink, PcmBlock, PcmSink, Result};
pub use rust_music_engine::{PlaybackSnapshot, PlaybackState, Player};
pub use rust_music_flac::{FlacDecoder, StreamInfo};
pub use rust_music_mp4::Mp4AlacDecoder;
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
    } else if head.starts_with(b"caff") {
        Ok(Box::new(CafDecoder::new(input)?))
    } else if head.len() >= 8 && &head[4..8] == b"ftyp" {
        Ok(Box::new(Mp4AlacDecoder::new(input)?))
    } else {
        Err(MusicError::Unsupported(
            "audiophile-only input: native FLAC, integer PCM WAV, uncompressed AIFF/AIFF-C, supported CAF, or ALAC M4A",
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
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    struct Temp(PathBuf);

    impl Temp {
        fn new() -> Self {
            loop {
                let sequence = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!(
                    "rust-music-admission-{}-{sequence}",
                    std::process::id()
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("failed to create admission test directory: {error}"),
                }
            }
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

    fn synthetic_caf_pcm() -> Vec<u8> {
        let mut bytes = b"caff\x00\x01\x00\x00".to_vec();
        let mut description = 44_100f64.to_bits().to_be_bytes().to_vec();
        description.extend_from_slice(b"lpcm");
        description.extend_from_slice(&0u32.to_be_bytes());
        description.extend_from_slice(&2u32.to_be_bytes());
        description.extend_from_slice(&1u32.to_be_bytes());
        description.extend_from_slice(&1u32.to_be_bytes());
        description.extend_from_slice(&16u32.to_be_bytes());
        bytes.extend_from_slice(b"desc");
        bytes.extend_from_slice(&(description.len() as i64).to_be_bytes());
        bytes.extend_from_slice(&description);
        let data = [0, 0, 0, 0, 0x80, 0, 0x7f, 0xff];
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(data.len() as i64).to_be_bytes());
        bytes.extend_from_slice(&data);
        bytes
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
            assert!(open_file(path).is_err());
        }
    }

    #[test]
    fn admits_alac_m4a_by_content_even_with_flac_suffix() {
        let temp = Temp::new();
        let path = temp.write_named(include_bytes!(
            "../../rust-music-mp4/tests/fixtures/generated_alac24.m4a"
        ));
        let mut decoder = open_file(path).unwrap();
        assert_eq!(decoder.format(), AudioFormat::new(44_100, 2, 24).unwrap());
        assert_eq!(decoder.total_frames(), Some(64));
        let mut actual = Vec::new();
        while let Some(block) = decoder.next_block().unwrap() {
            actual.extend(block.samples);
        }
        let expected: Vec<i32> = (0..64)
            .flat_map(|frame| [-7_000_000 + frame * 17_111, 4_000_000 - frame * 9_999])
            .collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn rejects_aac_m4a_by_content_even_with_flac_suffix() {
        let temp = Temp::new();
        let path = temp.write_named(include_bytes!(
            "../../rust-music-mp4/tests/fixtures/generated_aac.m4a"
        ));
        assert!(matches!(open_file(path), Err(MusicError::Unsupported(_))));
    }

    #[test]
    fn opens_caf_by_content_even_with_a_different_extension() {
        let temp = Temp::new();
        let path = temp.write_named(&synthetic_caf_pcm());
        let mut decoder = open_file(path).unwrap();
        assert_eq!(decoder.format(), AudioFormat::new(44_100, 1, 16).unwrap());
        assert_eq!(
            decoder.next_block().unwrap().unwrap().samples,
            [i16::MIN as i32, i16::MAX as i32]
        );
        assert!(decoder.next_block().unwrap().is_none());
    }
}
