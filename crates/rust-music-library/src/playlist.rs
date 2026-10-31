//! Bounded, text-only M3U8 playlists.
//!
//! This module stores relative path strings only. It performs no filesystem
//! access, does not resolve or open tracks, and never reads audio metadata or
//! artwork. Callers must resolve every entry through the selected library's
//! checked-path API before playback.

use rust_music_core::{MusicError, Result};
use std::path::{Component, Path, PathBuf};

/// Maximum number of entries retained in one playlist.
pub const MAX_PLAYLIST_ENTRIES: usize = 10_000;
/// Maximum UTF-8 byte length of an imported M3U8 document.
pub const MAX_PLAYLIST_BYTES: usize = 1_048_576;
/// Maximum UTF-8 byte length of a single M3U8 line.
pub const MAX_PLAYLIST_LINE_BYTES: usize = 4096;
const MAX_PLAYLIST_LINES: usize = MAX_PLAYLIST_ENTRIES * 2 + 16;

/// A text playlist whose entries are root-relative supported audio paths.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Playlist {
    entries: Vec<PathBuf>,
    entry_bytes: usize,
}

impl Playlist {
    /// Creates an empty playlist.
    pub fn new() -> Self {
        Self::default()
    }

    /// Parses a bounded UTF-8 M3U8 document. Comments, including `EXTINF`,
    /// are ignored; only safe relative audio paths are retained.
    pub fn from_m3u8(input: &[u8]) -> Result<Self> {
        if input.len() > MAX_PLAYLIST_BYTES {
            return Err(MusicError::Limit("playlist document is too large"));
        }
        let text = std::str::from_utf8(input)
            .map_err(|_| MusicError::Invalid("playlist is not valid UTF-8"))?;
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut playlist = Self::new();
        let mut lines = 0usize;
        for raw_line in text.lines() {
            lines = lines.saturating_add(1);
            if lines > MAX_PLAYLIST_LINES {
                return Err(MusicError::Limit("playlist has too many lines"));
            }
            let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
            if line.len() > MAX_PLAYLIST_LINE_BYTES {
                return Err(MusicError::Limit("playlist line is too long"));
            }
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            playlist.push(PathBuf::from(line))?;
        }
        Ok(playlist)
    }

    /// Serializes as UTF-8 M3U8 with no metadata or external references.
    pub fn to_m3u8(&self) -> Result<String> {
        if self.entries.len() > MAX_PLAYLIST_ENTRIES {
            return Err(MusicError::Limit("playlist has too many entries"));
        }
        let mut out = String::from("#EXTM3U\n");
        for entry in &self.entries {
            validate_relative_audio_path(entry)?;
            let text = entry
                .to_str()
                .ok_or(MusicError::Invalid("playlist path is not valid UTF-8"))?;
            if text.len() > MAX_PLAYLIST_LINE_BYTES {
                return Err(MusicError::Limit("playlist path is too long"));
            }
            out.push_str(text);
            out.push('\n');
            if out.len() > MAX_PLAYLIST_BYTES {
                return Err(MusicError::Limit("playlist document is too large"));
            }
        }
        Ok(out)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[PathBuf] {
        &self.entries
    }

    /// Appends a root-relative, supported-audio path without touching it.
    pub fn push(&mut self, path: PathBuf) -> Result<()> {
        if self.entries.len() >= MAX_PLAYLIST_ENTRIES {
            return Err(MusicError::Limit("playlist has too many entries"));
        }
        validate_relative_audio_path(&path)?;
        let text = path
            .to_str()
            .ok_or(MusicError::Invalid("playlist path is not valid UTF-8"))?;
        if text.len() > MAX_PLAYLIST_LINE_BYTES {
            return Err(MusicError::Limit("playlist path is too long"));
        }
        let length = text
            .len()
            .checked_add(1)
            .ok_or(MusicError::Limit("playlist document is too large"))?;
        let serialized_bytes = "#EXTM3U\n"
            .len()
            .checked_add(self.entry_bytes)
            .and_then(|size| size.checked_add(length))
            .ok_or(MusicError::Limit("playlist document is too large"))?;
        if serialized_bytes > MAX_PLAYLIST_BYTES {
            return Err(MusicError::Limit("playlist document is too large"));
        }
        self.entries.push(path);
        self.entry_bytes += length;
        Ok(())
    }

    pub fn remove(&mut self, index: usize) -> Option<PathBuf> {
        if index >= self.entries.len() {
            return None;
        }
        let entry = self.entries.remove(index);
        self.entry_bytes -= entry.to_str().map_or(0, |text| text.len() + 1);
        Some(entry)
    }

    pub fn move_entry(&mut self, from: usize, to: usize) -> Result<()> {
        if from >= self.entries.len() || to >= self.entries.len() {
            return Err(MusicError::Invalid("playlist index is out of bounds"));
        }
        if from != to {
            let entry = self.entries.remove(from);
            self.entries.insert(to, entry);
        }
        Ok(())
    }
}

fn validate_relative_audio_path(path: &Path) -> Result<()> {
    let text = path
        .to_str()
        .ok_or(MusicError::Invalid("playlist path is not valid UTF-8"))?;
    if text.is_empty()
        || text.chars().any(char::is_control)
        || text.contains('\\')
        || text.contains(':')
    {
        return Err(MusicError::Invalid("playlist path contains unsafe text"));
    }
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            _ => return Err(MusicError::Invalid("playlist path is not root-relative")),
        }
    }
    if !matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("flac" | "wav" | "wave" | "aif" | "aiff" | "caf" | "m4a" | "mp4a" | "mp4")
    ) {
        return Err(MusicError::Unsupported(
            "playlist entry does not have a supported lossless container suffix",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m3u8_round_trip_keeps_only_relative_audio_paths() {
        let input = b"\xef\xbb\xbf#EXTM3U\r\n#EXTINF:123,Ignored title\r\nAlbum/Caf\xc3\xa9.flac\r\ntrack.WAV\r\n";
        let playlist = Playlist::from_m3u8(input).unwrap();
        assert_eq!(playlist.len(), 2);
        assert_eq!(playlist.entries()[0], PathBuf::from("Album/Caf\u{e9}.flac"));
        assert_eq!(
            playlist.to_m3u8().unwrap(),
            "#EXTM3U\nAlbum/Caf\u{e9}.flac\ntrack.WAV\n"
        );
    }

    #[test]
    fn rejects_traversal_absolute_uri_and_lossy_suffixes() {
        for entry in [
            "../outside.flac",
            "/absolute/track.flac",
            "C:/music/track.flac",
            "https://example.invalid/track.flac",
            "track.mp3",
            "folder\\..\\track.flac",
        ] {
            assert!(Playlist::from_m3u8(entry.as_bytes()).is_err(), "{entry}");
        }
    }

    #[test]
    fn rejects_invalid_utf8_controls_and_oversized_lines() {
        assert!(Playlist::from_m3u8(b"track-\xff.flac").is_err());
        assert!(Playlist::from_m3u8(b"track\x01.flac").is_err());
        let long = format!("{}.flac", "x".repeat(MAX_PLAYLIST_LINE_BYTES));
        assert!(Playlist::from_m3u8(long.as_bytes()).is_err());
        assert!(Playlist::from_m3u8(&vec![b'x'; MAX_PLAYLIST_BYTES + 1]).is_err());
    }

    #[test]
    fn entry_bound_and_order_edits_are_checked() {
        let mut playlist = Playlist::new();
        playlist.push(PathBuf::from("a.flac")).unwrap();
        playlist.push(PathBuf::from("b.wav")).unwrap();
        playlist.move_entry(1, 0).unwrap();
        assert_eq!(playlist.entries()[0], PathBuf::from("b.wav"));
        assert!(playlist.move_entry(2, 0).is_err());
        for i in playlist.len()..MAX_PLAYLIST_ENTRIES {
            playlist
                .push(PathBuf::from(format!("track{i}.flac")))
                .unwrap();
        }
        assert!(playlist.push(PathBuf::from("overflow.flac")).is_err());
        assert_eq!(playlist.remove(0), Some(PathBuf::from("b.wav")));
    }

    #[test]
    fn aggregate_serialized_size_is_bounded_incrementally() {
        let path = PathBuf::from(format!("{}.flac", "x".repeat(MAX_PLAYLIST_LINE_BYTES - 5)));
        let mut playlist = Playlist::new();
        while playlist.push(path.clone()).is_ok() {}
        let serialized = playlist.to_m3u8().unwrap();
        assert!(serialized.len() <= MAX_PLAYLIST_BYTES);
        assert!(playlist.push(path).is_err());
    }

    #[test]
    fn an_empty_playlist_exports_a_valid_header() {
        assert_eq!(Playlist::new().to_m3u8().unwrap(), "#EXTM3U\n");
    }
}
