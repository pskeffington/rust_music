//! Explicit, bounded, offline music catalog. No threads, database, network,
//! third-party crates, or metadata downloads. Never follows symbolic links.
//!
//! Scan only when the host asks. A scan checks file signatures and file names;
//! it does NOT decode tracks or start playback. Entries are sorted for stable
//! session-local selection regardless of filesystem iteration order.

use rust_music_core::{MusicError, Result};
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Component, Path, PathBuf},
};

const MAX_TAG_BLOCKS: usize = 128;
const MAX_TAG_BLOCK_BYTES: u32 = 16 * 1024 * 1024;
const MAX_TAG_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TAG_VALUE_BYTES: usize = 512;
const MAX_TAG_ITEMS: u32 = 4096;
const MAX_TAG_CHARS: usize = 120;
const MAX_TAG_CHUNKS: usize = 512;

/// Limits are application policy, not a promise to index an unlimited disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanLimits {
    pub max_entries: usize,
    pub max_tracks: usize,
    pub max_depth: usize,
}
impl Default for ScanLimits {
    fn default() -> Self {
        Self {
            max_entries: 8192,
            max_tracks: 2048,
            max_depth: 6,
        }
    }
}
impl ScanLimits {
    fn validate(self) -> Result<Self> {
        if self.max_entries == 0
            || self.max_tracks == 0
            || self.max_entries > 100_000
            || self.max_tracks > 20_000
            || self.max_tracks > self.max_entries
            || self.max_depth > 24
        {
            return Err(MusicError::Limit("invalid offline library scan limits"));
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioContainer {
    Flac,
    Wave,
    Aiff,
    Caf,
    Mp4,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    /// Path relative to the selected library directory. Never use this as a URL.
    pub relative_path: PathBuf,
    /// Local filename only, not an untrusted embedded tag or network result.
    pub title: String,
    /// Small allowlisted text metadata. Cover art and unknown fields are never
    /// decoded or retained; malformed/oversized values fall back to filename.
    pub tags: Option<TrackTags>,
    pub container: AudioContainer,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrackTags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
}

impl Track {
    /// Prefer sanitized embedded title metadata while keeping `title` as the
    /// deterministic filename fallback for hosts that do not want tags.
    pub fn display_title(&self) -> &str {
        self.tags
            .as_ref()
            .and_then(|tags| tags.title.as_deref())
            .unwrap_or(&self.title)
    }
}

#[derive(Debug, Clone)]
pub struct Library {
    root: PathBuf,
    tracks: Vec<Track>,
    pub examined_entries: usize,
    pub skipped_entries: usize,
}
impl Library {
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn len(&self) -> usize {
        self.tracks.len()
    }
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }
    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }
    pub fn track(&self, index: usize) -> Option<&Track> {
        self.tracks.get(index)
    }

    /// Resolve only an item emitted by this Library. Recheck every component
    /// against symbolic-link substitution before using the path for playback.
    /// This does not claim atomic no-follow protection against a hostile local
    /// actor racing directory changes; such hosts need OS-specific openat logic.
    pub fn checked_path(&self, index: usize) -> Result<PathBuf> {
        let track = self
            .tracks
            .get(index)
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
            return Err(MusicError::Invalid(
                "selected library track is no longer a regular file",
            ));
        }
        Ok(absolute)
    }
}

fn supported_extension(path: &Path) -> bool {
    path.extension().and_then(|x| x.to_str()).is_some_and(|x| {
        [
            "flac", "wav", "aif", "aiff", "aifc", "caf", "m4a", "mp4", "mp4a",
        ]
        .iter()
        .any(|allowed| x.eq_ignore_ascii_case(allowed))
    })
}
fn probe_container(input: &mut File) -> Result<Option<AudioContainer>> {
    input.seek(SeekFrom::Start(0))?;
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
    if len >= 4 && head.starts_with(b"fLaC") {
        Ok(Some(AudioContainer::Flac))
    } else if len == 12 && head.starts_with(b"RIFF") && &head[8..12] == b"WAVE" {
        Ok(Some(AudioContainer::Wave))
    } else if len == 12
        && head.starts_with(b"FORM")
        && (&head[8..12] == b"AIFF" || &head[8..12] == b"AIFC")
    {
        Ok(Some(AudioContainer::Aiff))
    } else if len >= 4 && head.starts_with(b"caff") {
        Ok(Some(AudioContainer::Caf))
    } else if len == 12 && &head[4..8] == b"ftyp" {
        Ok(Some(AudioContainer::Mp4))
    } else {
        Ok(None)
    }
}

fn add_tag(slot: &mut Option<String>, raw: &[u8]) {
    if slot.is_some() || raw.len() > MAX_TAG_VALUE_BYTES {
        return;
    }
    let Ok(text) = std::str::from_utf8(raw) else {
        return;
    };
    let text = text
        .trim()
        .chars()
        .filter(|ch| !ch.is_control())
        .take(MAX_TAG_CHARS)
        .collect::<String>();
    if !text.is_empty() {
        *slot = Some(text);
    }
}

fn tag_slot<'a>(tags: &'a mut TrackTags, key: &[u8]) -> Option<&'a mut Option<String>> {
    if key.eq_ignore_ascii_case(b"TITLE") {
        Some(&mut tags.title)
    } else if key.eq_ignore_ascii_case(b"ARTIST") {
        Some(&mut tags.artist)
    } else if key.eq_ignore_ascii_case(b"ALBUM") {
        Some(&mut tags.album)
    } else {
        None
    }
}

fn read_flac_tags(file: &mut File) -> Option<TrackTags> {
    file.seek(SeekFrom::Start(4)).ok()?;
    let mut tags = TrackTags::default();
    let mut total = 0u64;
    let mut saw_last = false;
    let mut items_left = MAX_TAG_ITEMS;
    for _ in 0..MAX_TAG_BLOCKS {
        let mut header = [0; 4];
        file.read_exact(&mut header).ok()?;
        let last = header[0] & 0x80 != 0;
        let kind = header[0] & 0x7f;
        let length =
            (u32::from(header[1]) << 16) | (u32::from(header[2]) << 8) | u32::from(header[3]);
        if length > MAX_TAG_BLOCK_BYTES {
            return None;
        }
        total = total.checked_add(u64::from(length) + 4)?;
        if total > MAX_TAG_TOTAL_BYTES {
            return None;
        }
        let end = file
            .stream_position()
            .ok()?
            .checked_add(u64::from(length))?;
        if kind == 4 && length >= 8 {
            let mut field = [0; 4];
            file.read_exact(&mut field).ok()?;
            let vendor = u32::from_le_bytes(field);
            if u64::from(vendor) > end.checked_sub(file.stream_position().ok()?)? {
                return None;
            }
            file.seek(SeekFrom::Current(i64::from(vendor))).ok()?;
            file.read_exact(&mut field).ok()?;
            let count = u32::from_le_bytes(field).min(items_left);
            items_left -= count;
            let mut remaining = end.checked_sub(file.stream_position().ok()?)?;
            for _ in 0..count {
                if remaining < 4 {
                    break;
                }
                file.read_exact(&mut field).ok()?;
                remaining -= 4;
                let size = u32::from_le_bytes(field);
                if u64::from(size) > remaining {
                    break;
                }
                let mut prefix = [0u8; 64];
                let prefix_len = (size as usize).min(prefix.len());
                file.read_exact(&mut prefix[..prefix_len]).ok()?;
                remaining -= prefix_len as u64;
                let value_start = prefix[..prefix_len].iter().position(|b| *b == b'=');
                let target = value_start.and_then(|sep| tag_slot(&mut tags, &prefix[..sep]));
                if let (Some(slot), Some(sep)) = (target, value_start) {
                    let value_len = size as usize - sep - 1;
                    if value_len <= MAX_TAG_VALUE_BYTES {
                        let already_read = prefix_len.saturating_sub(sep + 1);
                        let mut value = prefix[sep + 1..prefix_len].to_vec();
                        if already_read < value_len {
                            let mut suffix = vec![0; value_len - already_read];
                            file.read_exact(&mut suffix).ok()?;
                            value.extend_from_slice(&suffix);
                        }
                        add_tag(slot, &value);
                    } else {
                        file.seek(SeekFrom::Current(i64::from(size) - prefix_len as i64))
                            .ok()?;
                    }
                } else {
                    file.seek(SeekFrom::Current(i64::from(size) - prefix_len as i64))
                        .ok()?;
                }
                remaining = remaining.checked_sub(u64::from(size) - prefix_len as u64)?;
            }
        }
        file.seek(SeekFrom::Start(end)).ok()?;
        if last {
            saw_last = true;
            break;
        }
    }
    (saw_last && !tags.is_empty()).then_some(tags)
}

fn read_aiff_tags(file: &mut File) -> Option<TrackTags> {
    file.seek(SeekFrom::Start(0)).ok()?;
    let mut form = [0u8; 12];
    file.read_exact(&mut form).ok()?;
    if &form[..4] != b"FORM" || (&form[8..12] != b"AIFF" && &form[8..12] != b"AIFC") {
        return None;
    }
    let mut remaining =
        u64::from(u32::from_be_bytes(form[4..8].try_into().ok()?)).checked_sub(4)?;
    let mut tags = TrackTags::default();
    let mut total = 0u64;
    for _ in 0..MAX_TAG_CHUNKS {
        if remaining < 8 {
            break;
        }
        let mut header = [0u8; 8];
        file.read_exact(&mut header).ok()?;
        remaining -= 8;
        let length = u64::from(u32::from_be_bytes(header[4..8].try_into().ok()?));
        let padded = length.checked_add(length & 1)?;
        if padded > remaining {
            return None;
        }
        total = total.checked_add(padded + 8)?;
        if total > MAX_TAG_TOTAL_BYTES {
            return None;
        }
        let slot = match &header[..4] {
            b"NAME" => Some(&mut tags.title),
            b"AUTH" => Some(&mut tags.artist),
            _ => None,
        };
        if let Some(slot) = slot {
            if length <= MAX_TAG_VALUE_BYTES as u64 {
                let mut value = vec![0; length as usize];
                file.read_exact(&mut value).ok()?;
                add_tag(slot, &value);
            } else {
                file.seek(SeekFrom::Current(length as i64)).ok()?;
            }
        } else {
            file.seek(SeekFrom::Current(length as i64)).ok()?;
        }
        if padded != length {
            file.seek(SeekFrom::Current(1)).ok()?;
        }
        remaining -= padded;
        if &header[..4] == b"SSND" {
            break;
        }
    }
    (!tags.is_empty()).then_some(tags)
}

impl TrackTags {
    fn is_empty(&self) -> bool {
        self.title.is_none() && self.artist.is_none() && self.album.is_none()
    }
}

fn read_tags(file: &mut File, container: AudioContainer) -> Option<TrackTags> {
    match container {
        AudioContainer::Flac => read_flac_tags(file),
        AudioContainer::Aiff => read_aiff_tags(file),
        _ => None,
    }
}

/// Explicit library scan. A limit violation is an error rather than a silent
/// partial playlist. Call on a worker thread, never from the Rust_Boy UI tick.
/// The filesystem is read-only; no file contents are cached or sent anywhere.
pub fn scan(root: impl AsRef<Path>, limits: ScanLimits) -> Result<Library> {
    let limits = limits.validate()?;
    let metadata = fs::symlink_metadata(root.as_ref())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(MusicError::Invalid(
            "music library root must be a real directory",
        ));
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
                let mut file = File::open(&path)?;
                if let Some(container) = probe_container(&mut file)? {
                    if tracks.len() >= limits.max_tracks {
                        return Err(MusicError::Limit("music library track budget exceeded"));
                    }
                    let relative_path = path
                        .strip_prefix(&root)
                        .map_err(|_| MusicError::Invalid("library entry escaped selected root"))?
                        .to_path_buf();
                    let title = path
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .unwrap_or("untitled")
                        .chars()
                        .take(120)
                        .collect::<String>();
                    let size_bytes = file.metadata()?.len();
                    tracks.push(Track {
                        relative_path,
                        title,
                        tags: read_tags(&mut file, container),
                        container,
                        size_bytes,
                    });
                } else {
                    skipped_entries += 1;
                }
            }
        }
    }
    tracks.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(Library {
        root,
        tracks,
        examined_entries,
        skipped_entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "rust-music-library-{}-{}-{}",
                std::process::id(),
                stamp,
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn flac_with_comments(comments: &[Vec<u8>]) -> Vec<u8> {
        let mut block = Vec::new();
        block.extend_from_slice(&4u32.to_le_bytes());
        block.extend_from_slice(b"test");
        block.extend_from_slice(&(comments.len() as u32).to_le_bytes());
        for comment in comments {
            block.extend_from_slice(&(comment.len() as u32).to_le_bytes());
            block.extend_from_slice(comment);
        }
        let mut bytes = b"fLaC".to_vec();
        bytes.extend_from_slice(&[0, 0, 0, 34]); // STREAMINFO, non-final
        bytes.extend_from_slice(&[0; 34]);
        bytes.extend_from_slice(&[
            0x84,
            ((block.len() >> 16) & 0xff) as u8,
            ((block.len() >> 8) & 0xff) as u8,
            (block.len() & 0xff) as u8,
        ]);
        bytes.extend_from_slice(&block);
        bytes
    }

    fn aiff_with_text_chunks(chunks: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
        let mut body = b"AIFF".to_vec();
        for (id, value) in chunks {
            body.extend_from_slice(*id);
            body.extend_from_slice(&(value.len() as u32).to_be_bytes());
            body.extend_from_slice(value);
            if value.len() % 2 != 0 {
                body.push(0);
            }
        }
        let mut bytes = b"FORM".to_vec();
        bytes.extend_from_slice(&(body.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&body);
        bytes
    }

    #[test]
    fn signature_checked_ordered_and_never_autoplays() {
        let t = Temp::new();
        fs::create_dir(t.0.join("album")).unwrap();
        fs::write(t.0.join("z.FLAC"), b"fLaCxxxx").unwrap();
        fs::write(t.0.join("album/a.wav"), b"RIFF1234WAVE").unwrap();
        fs::write(t.0.join("B.AIFF"), b"FORM1234AIFF").unwrap();
        fs::write(t.0.join("C.CAF"), b"caff\x00\x01\x00\x00").unwrap();
        fs::write(
            t.0.join("D.M4A"),
            b"\x00\x00\x00\x18ftypM4A \x00\x00\x00\x00",
        )
        .unwrap();
        fs::write(
            t.0.join("E.mp4"),
            b"\x00\x00\x00\x18ftypisom\x00\x00\x00\x00",
        )
        .unwrap();
        fs::write(
            t.0.join("F.mp4a"),
            b"\x00\x00\x00\x18ftypM4A \x00\x00\x00\x00",
        )
        .unwrap();
        fs::write(t.0.join("fake.flac"), b"NOTFLAC").unwrap();
        fs::write(t.0.join("ignore.mp3"), b"fLaCxxxx").unwrap();
        let lib = scan(&t.0, ScanLimits::default()).unwrap();
        assert_eq!(lib.len(), 7);
        assert_eq!(lib.track(0).unwrap().title, "B");
        assert_eq!(lib.track(1).unwrap().title, "C");
        assert_eq!(lib.track(2).unwrap().title, "D");
        assert_eq!(lib.track(3).unwrap().title, "E");
        assert_eq!(lib.track(4).unwrap().title, "F");
        assert_eq!(lib.track(5).unwrap().title, "a");
        assert_eq!(lib.track(6).unwrap().title, "z");
        assert_eq!(lib.track(0).unwrap().container, AudioContainer::Aiff);
        assert_eq!(lib.track(1).unwrap().container, AudioContainer::Caf);
        assert_eq!(lib.track(2).unwrap().container, AudioContainer::Mp4);
        assert_eq!(lib.track(4).unwrap().container, AudioContainer::Mp4);
        assert_eq!(lib.track(5).unwrap().container, AudioContainer::Wave);
        assert_eq!(lib.examined_entries, 10);
        assert!(lib.checked_path(5).unwrap().ends_with("album/a.wav"));
        assert!(lib.checked_path(10).is_err());
    }

    #[test]
    fn refuses_unbounded_scan() {
        let t = Temp::new();
        fs::write(t.0.join("a.flac"), b"fLaC").unwrap();
        fs::write(t.0.join("b.flac"), b"fLaC").unwrap();
        assert!(scan(
            &t.0,
            ScanLimits {
                max_entries: 1,
                max_tracks: 1,
                max_depth: 0
            }
        )
        .is_err());
        assert!(scan(
            &t.0,
            ScanLimits {
                max_entries: 0,
                ..ScanLimits::default()
            }
        )
        .is_err());
    }

    #[test]
    fn flac_tags_are_allowlisted_utf8_bounded_and_artwork_is_skipped() {
        let t = Temp::new();
        let mut picture = b"METADATA_BLOCK_PICTURE=".to_vec();
        picture.extend(std::iter::repeat_n(b'A', 2 * 1024 * 1024));
        fs::write(
            t.0.join("fallback.flac"),
            flac_with_comments(&[
                picture,
                "TITLE=Bach – aria".as_bytes().to_vec(),
                b"ARTIST=Kimiko\nIshizaka".to_vec(),
                b"ALBUM=Open Goldberg Variations".to_vec(),
                b"COMMENT=unexposed".to_vec(),
            ]),
        )
        .unwrap();
        let lib = scan(&t.0, ScanLimits::default()).unwrap();
        let track = lib.track(0).unwrap();
        assert_eq!(track.title, "fallback");
        assert_eq!(track.display_title(), "Bach – aria");
        assert_eq!(
            track.tags,
            Some(TrackTags {
                title: Some("Bach – aria".into()),
                artist: Some("KimikoIshizaka".into()),
                album: Some("Open Goldberg Variations".into()),
            })
        );
    }

    #[test]
    fn invalid_or_oversized_flac_values_do_not_poison_other_tags() {
        let t = Temp::new();
        let oversized = [b"TITLE=".as_slice(), &vec![b'x'; MAX_TAG_VALUE_BYTES + 1]].concat();
        fs::write(
            t.0.join("fallback.flac"),
            flac_with_comments(&[
                b"TITLE=\xff".to_vec(),
                oversized,
                b"ARTIST=Valid artist".to_vec(),
            ]),
        )
        .unwrap();
        let lib = scan(&t.0, ScanLimits::default()).unwrap();
        let track = lib.track(0).unwrap();
        assert_eq!(track.display_title(), "fallback");
        assert_eq!(
            track.tags.as_ref().unwrap().artist.as_deref(),
            Some("Valid artist")
        );
    }

    #[test]
    fn aiff_name_and_author_tags_are_utf8_bounded_and_ignore_unknown_chunks() {
        let t = Temp::new();
        fs::write(
            t.0.join("fallback.aiff"),
            aiff_with_text_chunks(&[
                (b"NAME", &vec![b'x'; MAX_TAG_VALUE_BYTES + 1]),
                (b"NAME", "Piano – nocturne".as_bytes()),
                (b"AUTH", b"Artist\xff"),
                (b"ANNO", b"not displayed"),
            ]),
        )
        .unwrap();
        let lib = scan(&t.0, ScanLimits::default()).unwrap();
        let track = lib.track(0).unwrap();
        assert_eq!(track.display_title(), "Piano – nocturne");
        assert_eq!(track.tags.as_ref().unwrap().artist, None);
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
