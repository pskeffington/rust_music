//! Explicit, bounded, offline music catalog. No threads, database, network,
//! third-party crates, or metadata downloads. Never follows symbolic links.
//!
//! Scan only when the host asks. A scan checks file signatures and file names;
//! it does NOT decode tracks or start playback. Entries are sorted for stable
//! session-local selection regardless of filesystem iteration order.

use rust_music_core::{MusicError, Result};
use std::{fs::{self, File}, io::Read, path::{Component, Path, PathBuf}};

/// Limits are application policy, not a promise to index an unlimited disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanLimits {
    pub max_entries: usize,
    pub max_tracks: usize,
    pub max_depth: usize,
}
impl Default for ScanLimits {
    fn default() -> Self {
        Self { max_entries: 8192, max_tracks: 2048, max_depth: 6 }
    }
}
impl ScanLimits {
    fn validate(self) -> Result<Self> {
        if self.max_entries == 0 || self.max_tracks == 0
            || self.max_entries > 100_000 || self.max_tracks > 20_000
            || self.max_tracks > self.max_entries || self.max_depth > 24 {
            return Err(MusicError::Limit("invalid offline library scan limits"));
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioContainer { Flac, Wave, Aiff }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    /// Path relative to the selected library directory. Never use this as a URL.
    pub relative_path: PathBuf,
    /// Local filename only, not an untrusted embedded tag or network result.
    pub title: String,
    pub container: AudioContainer,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct Library {
    root: PathBuf,
    tracks: Vec<Track>,
    pub examined_entries: usize,
    pub skipped_entries: usize,
}
impl Library {
    pub fn root(&self) -> &Path { &self.root }
    pub fn len(&self) -> usize { self.tracks.len() }
    pub fn is_empty(&self) -> bool { self.tracks.is_empty() }
    pub fn tracks(&self) -> &[Track] { &self.tracks }
    pub fn track(&self, index: usize) -> Option<&Track> { self.tracks.get(index) }

    /// Resolve only an item emitted by this Library. Recheck every component
    /// against symbolic-link substitution before using the path for playback.
    /// This does not claim atomic no-follow protection against a hostile local
    /// actor racing directory changes; such hosts need OS-specific openat logic.
    pub fn checked_path(&self, index: usize) -> Result<PathBuf> {
        let track = self.tracks.get(index)
            .ok_or(MusicError::Invalid("library track index out of bounds"))?;
        let mut absolute = self.root.clone();
        for component in track.relative_path.components() {
            match component {
                Component::Normal(name) => {
                    absolute.push(name);
                    let metadata = fs::symlink_metadata(&absolute)?;
                    if metadata.file_type().is_symlink() {
                        return Err(MusicError::Invalid("library path became a symbolic link"));
                    }
                }
                _ => return Err(MusicError::Invalid("unsafe relative library path")),
            }
        }
        if !fs::symlink_metadata(&absolute)?.is_file() {
            return Err(MusicError::Invalid("selected library track is no longer a regular file"));
        }
        Ok(absolute)
    }
}

fn supported_extension(path: &Path) -> bool {
    path.extension().and_then(|x| x.to_str())
        .is_some_and(|x| ["flac", "wav", "aif", "aiff", "aifc"].iter()
            .any(|allowed| x.eq_ignore_ascii_case(allowed)))
}
fn probe_container(path: &Path) -> Result<Option<AudioContainer>> {
    let mut input = File::open(path)?;
    let mut head = [0u8; 12];
    let mut len = 0;
    while len < head.len() {
        match input.read(&mut head[len..]) {
            Ok(0) => break,
            Ok(n) => len += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        }
    }
    if len >= 4 && head.starts_with(b"fLaC") { Ok(Some(AudioContainer::Flac)) }
    else if len == 12 && head.starts_with(b"RIFF") && &head[8..12] == b"WAVE" {
        Ok(Some(AudioContainer::Wave))
    } else if len == 12 && head.starts_with(b"FORM")
        && (&head[8..12] == b"AIFF" || &head[8..12] == b"AIFC") {
        Ok(Some(AudioContainer::Aiff))
    } else { Ok(None) }
}

/// Explicit library scan. A limit violation is an error rather than a silent
/// partial playlist. Call on a worker thread, never from the Rust_Boy UI tick.
/// The filesystem is read-only; no file contents are cached or sent anywhere.
pub fn scan(root: impl AsRef<Path>, limits: ScanLimits) -> Result<Library> {
    let limits = limits.validate()?;
    let metadata = fs::symlink_metadata(root.as_ref())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(MusicError::Invalid("music library root must be a real directory"));
    }
    let root = root.as_ref().canonicalize()?;
    let mut pending = vec![(root.clone(), 0usize)];
    let mut tracks = Vec::new();
    let mut examined_entries = 0usize;
    let mut skipped_entries = 0usize;
    while let Some((dir, depth)) = pending.pop() {
        for item in fs::read_dir(&dir)? {
            let item = item?;
            examined_entries = examined_entries.saturating_add(1);
            if examined_entries > limits.max_entries {
                return Err(MusicError::Limit("music library entry budget exceeded"));
            }
            let file_type = item.file_type()?;
            if file_type.is_symlink() {
                skipped_entries += 1;
                continue;
            }
            if file_type.is_dir() {
                if depth < limits.max_depth {
                    pending.push((item.path(), depth + 1));
                } else {
                    skipped_entries += 1;
                }
            } else if file_type.is_file() && supported_extension(&item.path()) {
                let path = item.path();
                if let Some(container) = probe_container(&path)? {
                    if tracks.len() >= limits.max_tracks {
                        return Err(MusicError::Limit("music library track budget exceeded"));
                    }
                    let relative_path = path.strip_prefix(&root)
                        .map_err(|_| MusicError::Invalid("library entry escaped selected root"))?
                        .to_path_buf();
                    let title = path.file_stem().and_then(|stem| stem.to_str()).unwrap_or("untitled")
                        .chars().take(120).collect::<String>();
                    let size_bytes = item.metadata()?.len();
                    tracks.push(Track { relative_path, title, container, size_bytes });
                } else {
                    skipped_entries += 1;
                }
            }
        }
    }
    tracks.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(Library { root, tracks, examined_entries, skipped_entries })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::atomic::{AtomicU64, Ordering}, time::{SystemTime, UNIX_EPOCH}};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let path = std::env::temp_dir().join(format!("rust-music-library-{}-{}-{}",
                std::process::id(), stamp, COUNTER.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }

    #[test]
    fn signature_checked_ordered_and_never_autoplays() {
        let t = Temp::new();
        fs::create_dir(t.0.join("album")).unwrap();
        fs::write(t.0.join("z.FLAC"), b"fLaCxxxx").unwrap();
        fs::write(t.0.join("album/a.wav"), b"RIFF1234WAVE").unwrap();
        fs::write(t.0.join("B.AIFF"), b"FORM1234AIFF").unwrap();
        fs::write(t.0.join("fake.flac"), b"NOTFLAC").unwrap();
        fs::write(t.0.join("ignore.mp3"), b"fLaCxxxx").unwrap();
        let lib = scan(&t.0, ScanLimits::default()).unwrap();
        assert_eq!(lib.len(), 3);
        assert_eq!(lib.track(0).unwrap().title, "a");
        assert_eq!(lib.track(1).unwrap().title, "B");
        assert_eq!(lib.track(2).unwrap().title, "z");
        assert_eq!(lib.track(0).unwrap().container, AudioContainer::Wave);
        assert_eq!(lib.track(1).unwrap().container, AudioContainer::Aiff);
        assert_eq!(lib.examined_entries, 5);
        assert!(lib.checked_path(0).unwrap().ends_with("album/a.wav"));
        assert!(lib.checked_path(10).is_err());
    }

    #[test]
    fn refuses_unbounded_scan() {
        let t = Temp::new();
        fs::write(t.0.join("a.flac"), b"fLaC").unwrap();
        fs::write(t.0.join("b.flac"), b"fLaC").unwrap();
        assert!(scan(&t.0, ScanLimits { max_entries: 1, max_tracks: 1, max_depth: 0 }).is_err());
        assert!(scan(&t.0, ScanLimits { max_entries: 0, ..ScanLimits::default() }).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn skips_symbolic_links_and_detects_replacement() {
        use std::os::unix::fs::symlink;
        let t = Temp::new();
        fs::write(t.0.join("a.flac"), b"fLaC").unwrap();
        fs::create_dir(t.0.join("outside")).unwrap();
        symlink(t.0.join("outside"), t.0.join("redirect")).unwrap();
        let lib = scan(&t.0, ScanLimits::default()).unwrap();
        assert_eq!(lib.len(), 1);
        fs::remove_file(t.0.join("a.flac")).unwrap();
        symlink(t.0.join("outside"), t.0.join("a.flac")).unwrap();
        assert!(lib.checked_path(0).is_err());
    }
}
