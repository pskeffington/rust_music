//! Explicit, bounded, offline music catalog. No threads, database, network,
//! third-party crates, or metadata downloads. Never follows symbolic links.
//!
//! Scan only when the host asks. A scan checks file signatures and file names;
//! it does NOT decode tracks or start playback. Entries are sorted for stable
//! session-local selection regardless of filesystem iteration order.

use rust_music_core::{MusicError, Result};
use std::{
    cmp::Ordering,
    collections::HashSet,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering as AtomicOrdering},
};

/// Bounded, text-only M3U8 playlist import/export and edit model.
pub mod playlist;
/// Bounded, versioned local persistence for named playlists.
pub mod playlist_store;
/// Bounded, persistent text-only queue state and navigation model.
pub mod queue;
/// Canonical, bounded portable snapshot manifest serialization.
pub mod sync_manifest;
/// Read-only, bounded offline package delta planning; no writes or hashing.
pub mod sync_plan;

const MAX_TAG_BLOCKS: usize = 128;
const MAX_TAG_BLOCK_BYTES: u32 = 16 * 1024 * 1024;
const MAX_TAG_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TAG_VALUE_BYTES: usize = 512;
const MAX_TAG_ITEMS: u32 = 4096;
const MAX_TAG_CHARS: usize = 120;
const MAX_TAG_CHUNKS: usize = 512;
const MAX_QUERY_BYTES: usize = 512;
pub const MAX_PAGE_SIZE: usize = 200;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AudioContainer {
    Flac,
    Wave,
    Aiff,
    Caf,
    Mp4,
}

/// Deterministic identifier scoped to one canonical library root and relative
/// path. It remains stable across scans of that root. Moving/remounting a root
/// changes its IDs; a future persisted-root identity can replace this scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TrackId([u8; 16]);

impl TrackId {
    /// Stable opaque identifier bytes suitable for a local versioned index.
    /// These bytes contain no source path or media metadata.
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// Text-only query and page parameters for an already-scanned catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackQuery {
    /// Case-insensitive Unicode lowercase substring across title, artist and
    /// album. Empty text does not filter. This is not locale-aware collation.
    pub text: String,
    pub container: Option<AudioContainer>,
    pub sort_by: TrackSort,
    pub descending: bool,
    pub offset: usize,
    pub limit: usize,
}

impl Default for TrackQuery {
    fn default() -> Self {
        Self {
            text: String::new(),
            container: None,
            sort_by: TrackSort::Path,
            descending: false,
            offset: 0,
            limit: 50,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackSort {
    Path,
    Title,
    Artist,
    Album,
    Container,
}

/// A bounded page of references into the scanned library. `total_matches`
/// describes the full filtered result, before applying offset and limit.
#[derive(Debug)]
pub struct TrackPage<'a> {
    pub total_matches: usize,
    pub offset: usize,
    pub limit: usize,
    pub tracks: Vec<&'a Track>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub id: TrackId,
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
    /// Empty, no-I/O catalog for a package that has been mounted but has not
    /// yet received an explicit user library-scan request.
    pub fn empty() -> Self {
        Self {
            root: PathBuf::new(),
            tracks: Vec::new(),
            examined_entries: 0,
            skipped_entries: 0,
        }
    }

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

    /// Find a track by its stable root-scoped identifier.
    pub fn track_by_id(&self, id: TrackId) -> Option<&Track> {
        self.tracks.iter().find(|track| track.id == id)
    }

    /// Filter, sort and paginate only the in-memory catalog. This method does
    /// no filesystem access and is safe to call from a UI/search worker. Scan
    /// work remains an explicit caller action.
    pub fn query(&self, query: &TrackQuery) -> Result<TrackPage<'_>> {
        if query.text.len() > MAX_QUERY_BYTES {
            return Err(MusicError::Limit("music library query is too long"));
        }
        if query.text.chars().any(char::is_control) {
            return Err(MusicError::Invalid("music library query contains controls"));
        }
        if query.limit == 0 || query.limit > MAX_PAGE_SIZE {
            return Err(MusicError::Limit(
                "music library page size is out of bounds",
            ));
        }

        let needle = query.text.to_lowercase();
        let mut matching = self
            .tracks
            .iter()
            .filter(|track| {
                query
                    .container
                    .is_none_or(|container| track.container == container)
                    && (needle.is_empty() || text_matches(track, needle.as_str()))
            })
            .collect::<Vec<_>>();
        matching
            .sort_by(|left, right| compare_tracks(left, right, query.sort_by, query.descending));

        let total_matches = matching.len();
        let start = query.offset.min(total_matches);
        let end = start.saturating_add(query.limit).min(total_matches);
        let tracks = matching.drain(start..end).collect();
        Ok(TrackPage {
            total_matches,
            offset: query.offset,
            limit: query.limit,
            tracks,
        })
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

fn text_matches(track: &Track, needle: &str) -> bool {
    std::iter::once(Some(track.display_title()))
        .chain(
            track
                .tags
                .as_ref()
                .into_iter()
                .flat_map(|tags| [tags.artist.as_deref(), tags.album.as_deref()]),
        )
        .flatten()
        .any(|field| field.to_lowercase().contains(needle))
}

fn compare_optional_text(left: Option<&str>, right: Option<&str>) -> Ordering {
    match (left, right) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(left), Some(right)) => left.cmp(right),
    }
}

fn compare_tracks(left: &Track, right: &Track, sort_by: TrackSort, descending: bool) -> Ordering {
    let primary = match sort_by {
        TrackSort::Path => left.relative_path.cmp(&right.relative_path),
        TrackSort::Title => {
            compare_optional_text(Some(left.display_title()), Some(right.display_title()))
        }
        TrackSort::Artist => compare_optional_text(
            left.tags.as_ref().and_then(|tags| tags.artist.as_deref()),
            right.tags.as_ref().and_then(|tags| tags.artist.as_deref()),
        ),
        TrackSort::Album => compare_optional_text(
            left.tags.as_ref().and_then(|tags| tags.album.as_deref()),
            right.tags.as_ref().and_then(|tags| tags.album.as_deref()),
        ),
        TrackSort::Container => left.container.cmp(&right.container),
    };
    let primary = if descending {
        primary.reverse()
    } else {
        primary
    };
    primary
        .then_with(|| left.relative_path.cmp(&right.relative_path))
        .then_with(|| left.id.cmp(&right.id))
}

fn stable_track_id(root_bytes: &[u8], relative_path: &Path) -> TrackId {
    let relative_bytes = path_bytes(relative_path);
    let root_len = (root_bytes.len() as u64).to_le_bytes();
    let relative_len = (relative_bytes.len() as u64).to_le_bytes();
    let mut first = 0xcbf29ce484222325u64;
    let mut second = 0x84222325cbf29ce4u64;
    for byte in b"rust-music-track-id-v1"
        .iter()
        .chain(root_len.iter())
        .chain(root_bytes.iter())
        .chain(relative_len.iter())
        .chain(relative_bytes.iter())
    {
        first = (first ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
        second = (second ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
        second ^= second.rotate_left(23);
    }
    let mut id = [0u8; 16];
    id[..8].copy_from_slice(&first.to_be_bytes());
    id[8..].copy_from_slice(&second.to_be_bytes());
    TrackId(id)
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(windows)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(not(any(unix, windows)))]
fn path_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().as_bytes().to_vec()
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
    scan_cancellable(root, limits, &AtomicBool::new(false))
}

/// Explicit bounded scan that can be cancelled by a caller-owned flag.
/// Cancellation returns no partial catalog, allowing the caller to keep its
/// previously committed library snapshot intact. Polling occurs between each
/// filesystem entry, on the worker performing the scan.
pub fn scan_cancellable(
    root: impl AsRef<Path>,
    limits: ScanLimits,
    cancellation: &AtomicBool,
) -> Result<Library> {
    scan_with_cancel_check(root, limits, || cancellation.load(AtomicOrdering::Relaxed))
}

fn scan_with_cancel_check(
    root: impl AsRef<Path>,
    limits: ScanLimits,
    is_cancelled: impl Fn() -> bool,
) -> Result<Library> {
    if is_cancelled() {
        return Err(MusicError::Cancelled("library scan cancelled"));
    }
    let limits = limits.validate()?;
    let metadata = fs::symlink_metadata(root.as_ref())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(MusicError::Invalid(
            "music library root must be a real directory",
        ));
    }
    let root = root.as_ref().canonicalize()?;
    let root_bytes = path_bytes(&root);
    let mut pending = vec![(root.clone(), 0usize)];
    let mut tracks = Vec::new();
    let mut track_ids = HashSet::new();
    let mut examined_entries = 0usize;
    let mut skipped_entries = 0usize;
    while let Some((dir, depth)) = pending.pop() {
        for item in fs::read_dir(&dir)? {
            if is_cancelled() {
                return Err(MusicError::Cancelled("library scan cancelled"));
            }
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
                    let id = stable_track_id(&root_bytes, &relative_path);
                    if !track_ids.insert(id) {
                        return Err(MusicError::Invalid("stable music track ID collision"));
                    }
                    let title = path
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .unwrap_or("untitled")
                        .chars()
                        .take(120)
                        .collect::<String>();
                    let size_bytes = file.metadata()?.len();
                    tracks.push(Track {
                        id,
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
    if is_cancelled() {
        return Err(MusicError::Cancelled("library scan cancelled"));
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

    fn wave_file() -> &'static [u8] {
        b"RIFF1234WAVE"
    }

    #[test]
    fn track_ids_are_stable_unique_and_scoped_to_the_canonical_root() {
        let first_root = Temp::new();
        let second_root = Temp::new();
        fs::create_dir(first_root.0.join("one")).unwrap();
        fs::create_dir(first_root.0.join("two")).unwrap();
        fs::write(first_root.0.join("one/duplicate.flac"), b"fLaCdata").unwrap();
        fs::write(first_root.0.join("two/duplicate.flac"), b"fLaCdata").unwrap();
        fs::write(second_root.0.join("one.flac"), b"fLaCdata").unwrap();

        let first = scan(&first_root.0, ScanLimits::default()).unwrap();
        let again = scan(&first_root.0, ScanLimits::default()).unwrap();
        let second = scan(&second_root.0, ScanLimits::default()).unwrap();

        assert_eq!(first.len(), 2);
        assert_ne!(first.track(0).unwrap().id, first.track(1).unwrap().id);
        for track in first.tracks() {
            assert_eq!(
                again.track_by_id(track.id).unwrap().relative_path,
                track.relative_path
            );
        }
        assert!(second.track_by_id(first.track(0).unwrap().id).is_none());
    }

    #[test]
    fn scan_rejects_missing_and_non_directory_roots() {
        let t = Temp::new();
        let missing = t.0.join("removed-library");
        assert!(matches!(
            scan(&missing, ScanLimits::default()),
            Err(MusicError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound
        ));

        let file_root = t.0.join("not-a-directory");
        fs::write(&file_root, b"not a directory").unwrap();
        assert!(matches!(
            scan(&file_root, ScanLimits::default()),
            Err(MusicError::Invalid(
                "music library root must be a real directory"
            ))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn scan_rejects_a_symbolic_link_as_the_library_root() {
        use std::os::unix::fs::symlink;

        let t = Temp::new();
        let actual_root = t.0.join("actual-library");
        fs::create_dir(&actual_root).unwrap();
        let linked_root = t.0.join("linked-library");
        symlink(&actual_root, &linked_root).unwrap();

        assert!(matches!(
            scan(&linked_root, ScanLimits::default()),
            Err(MusicError::Invalid(
                "music library root must be a real directory"
            ))
        ));
    }

    #[test]
    fn removed_library_root_invalidates_previously_cataloged_paths() {
        let t = Temp::new();
        fs::write(t.0.join("track.wav"), wave_file()).unwrap();
        let library = scan(&t.0, ScanLimits::default()).unwrap();
        assert_eq!(library.len(), 1);
        assert!(library.checked_path(0).is_ok());

        fs::remove_dir_all(&t.0).unwrap();
        assert!(library.checked_path(0).is_err());
    }

    #[test]
    fn text_query_is_unicode_case_insensitive_and_paginates_deterministically() {
        let t = Temp::new();
        fs::write(t.0.join("Café.wav"), wave_file()).unwrap();
        fs::write(t.0.join("Alpha.wav"), wave_file()).unwrap();
        fs::write(t.0.join("Bravo.wav"), wave_file()).unwrap();
        let lib = scan(&t.0, ScanLimits::default()).unwrap();

        let unicode = lib
            .query(&TrackQuery {
                text: "CAFÉ".into(),
                ..TrackQuery::default()
            })
            .unwrap();
        assert_eq!(unicode.total_matches, 1);
        assert_eq!(unicode.tracks[0].title, "Café");

        let first_page = lib
            .query(&TrackQuery {
                sort_by: TrackSort::Title,
                limit: 2,
                ..TrackQuery::default()
            })
            .unwrap();
        let second_page = lib
            .query(&TrackQuery {
                sort_by: TrackSort::Title,
                offset: 2,
                limit: 2,
                ..TrackQuery::default()
            })
            .unwrap();
        assert_eq!(first_page.total_matches, 3);
        assert_eq!(first_page.tracks.len(), 2);
        assert_eq!(second_page.tracks.len(), 1);
        let all_titles = first_page
            .tracks
            .iter()
            .chain(second_page.tracks.iter())
            .map(|track| track.display_title())
            .collect::<Vec<_>>();
        assert_eq!(all_titles, vec!["Alpha", "Bravo", "Café"]);

        let beyond_end = lib
            .query(&TrackQuery {
                offset: usize::MAX,
                ..TrackQuery::default()
            })
            .unwrap();
        assert!(beyond_end.tracks.is_empty());
        assert_eq!(beyond_end.total_matches, 3);
    }

    #[test]
    fn query_bounds_and_container_filters_are_enforced() {
        let t = Temp::new();
        fs::write(t.0.join("a.wav"), wave_file()).unwrap();
        fs::write(t.0.join("b.flac"), b"fLaCdata").unwrap();
        let lib = scan(&t.0, ScanLimits::default()).unwrap();

        let waves = lib
            .query(&TrackQuery {
                container: Some(AudioContainer::Wave),
                ..TrackQuery::default()
            })
            .unwrap();
        assert_eq!(waves.total_matches, 1);
        assert_eq!(waves.tracks[0].container, AudioContainer::Wave);

        for invalid in [
            TrackQuery {
                limit: 0,
                ..TrackQuery::default()
            },
            TrackQuery {
                limit: MAX_PAGE_SIZE + 1,
                ..TrackQuery::default()
            },
            TrackQuery {
                text: "x".repeat(MAX_QUERY_BYTES + 1),
                ..TrackQuery::default()
            },
            TrackQuery {
                text: "bad\nquery".into(),
                ..TrackQuery::default()
            },
        ] {
            assert!(lib.query(&invalid).is_err());
        }
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
    fn cancelled_scan_returns_no_partial_catalog_and_preserves_committed_snapshot() {
        let t = Temp::new();
        fs::write(t.0.join("a.wav"), wave_file()).unwrap();
        let committed = scan(&t.0, ScanLimits::default()).unwrap();
        fs::write(t.0.join("b.wav"), wave_file()).unwrap();
        let checks = std::cell::Cell::new(0usize);
        let attempted = scan_with_cancel_check(&t.0, ScanLimits::default(), || {
            let next = checks.get() + 1;
            checks.set(next);
            next > 2
        });
        assert!(matches!(attempted, Err(MusicError::Cancelled(_))));
        assert_eq!(committed.len(), 1);
        assert_eq!(committed.track(0).unwrap().title, "a");
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
