//! Bounded, versioned persistence for named text-only playlists.
//!
//! The store contains only UTF-8 playlist names and safe relative audio paths.
//! It never opens or changes media files and performs no playback or focus work.

use crate::playlist::{Playlist, MAX_PLAYLIST_ENTRIES, MAX_PLAYLIST_LINE_BYTES};
use rust_music_core::{MusicError, Result};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub const MAX_NAMED_PLAYLISTS: usize = 128;
pub const MAX_PLAYLIST_STORE_BYTES: usize = 1_048_576;
pub const MAX_PLAYLIST_NAME_BYTES: usize = 128;
const STORE_HEADER: &str = "RUSTMUSIC_PLAYLISTS\t1\n";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedPlaylist {
    name: String,
    playlist: Playlist,
}

impl NamedPlaylist {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn playlist(&self) -> &Playlist {
        &self.playlist
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlaylistStore {
    playlists: Vec<NamedPlaylist>,
}

impl PlaylistStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn playlists(&self) -> &[NamedPlaylist] {
        &self.playlists
    }

    pub fn get(&self, name: &str) -> Option<&Playlist> {
        self.playlists
            .iter()
            .find(|item| item.name == name)
            .map(NamedPlaylist::playlist)
    }

    /// Append an entry while enforcing both per-playlist and aggregate limits.
    pub fn push_entry(&mut self, name: &str, path: PathBuf) -> Result<()> {
        let index = self
            .playlists
            .iter()
            .position(|item| item.name == name)
            .ok_or(MusicError::Invalid("playlist does not exist"))?;
        self.playlists[index].playlist.push(path)?;
        if let Err(error) = self.validate_size() {
            let last = self.playlists[index].playlist.len() - 1;
            self.playlists[index].playlist.remove(last);
            return Err(error);
        }
        Ok(())
    }

    pub fn remove_entry(&mut self, name: &str, index: usize) -> Result<PathBuf> {
        let item = self
            .playlists
            .iter_mut()
            .find(|item| item.name == name)
            .ok_or(MusicError::Invalid("playlist does not exist"))?;
        item.playlist
            .remove(index)
            .ok_or(MusicError::Invalid("playlist entry index is out of bounds"))
    }

    pub fn move_entry(&mut self, name: &str, from: usize, to: usize) -> Result<()> {
        self.playlists
            .iter_mut()
            .find(|item| item.name == name)
            .ok_or(MusicError::Invalid("playlist does not exist"))?
            .playlist
            .move_entry(from, to)
    }

    pub fn create(&mut self, name: &str) -> Result<()> {
        validate_name(name)?;
        if self.playlists.iter().any(|item| item.name == name) {
            return Err(MusicError::Invalid("playlist name already exists"));
        }
        if self.playlists.len() >= MAX_NAMED_PLAYLISTS {
            return Err(MusicError::Limit("too many named playlists"));
        }
        self.playlists.push(NamedPlaylist {
            name: name.to_owned(),
            playlist: Playlist::new(),
        });
        if let Err(error) = self.validate_size() {
            self.playlists.pop();
            return Err(error);
        }
        Ok(())
    }

    pub fn rename(&mut self, old_name: &str, new_name: &str) -> Result<()> {
        validate_name(new_name)?;
        if self.playlists.iter().any(|item| item.name == new_name) {
            return Err(MusicError::Invalid("playlist name already exists"));
        }
        let item = self
            .playlists
            .iter_mut()
            .find(|item| item.name == old_name)
            .ok_or(MusicError::Invalid("playlist does not exist"))?;
        let previous = std::mem::replace(&mut item.name, new_name.to_owned());
        if let Err(error) = self.validate_size() {
            self.playlists
                .iter_mut()
                .find(|item| item.name == new_name)
                .expect("renamed playlist remains present")
                .name = previous;
            return Err(error);
        }
        Ok(())
    }

    pub fn remove(&mut self, name: &str) -> Option<Playlist> {
        let index = self.playlists.iter().position(|item| item.name == name)?;
        Some(self.playlists.remove(index).playlist)
    }

    pub fn to_record(&self) -> Result<String> {
        self.validate_size()?;
        let mut out = String::from(STORE_HEADER);
        for item in &self.playlists {
            validate_name(&item.name)?;
            out.push_str("playlist\t");
            for byte in item.name.as_bytes() {
                use std::fmt::Write as _;
                write!(out, "{byte:02x}").expect("writing to String cannot fail");
            }
            out.push('\n');
            for path in item.playlist.entries() {
                let path = path
                    .to_str()
                    .ok_or(MusicError::Invalid("playlist path is not valid UTF-8"))?;
                out.push_str("entry\t");
                out.push_str(path);
                out.push('\n');
            }
            if out.len() > MAX_PLAYLIST_STORE_BYTES {
                return Err(MusicError::Limit("playlist store is too large"));
            }
        }
        Ok(out)
    }

    pub fn from_record(input: &[u8]) -> Result<Self> {
        if input.len() > MAX_PLAYLIST_STORE_BYTES {
            return Err(MusicError::Limit("playlist store is too large"));
        }
        let text = std::str::from_utf8(input)
            .map_err(|_| MusicError::Invalid("playlist store is not UTF-8"))?;
        let mut lines = text.lines();
        if lines.next() != Some("RUSTMUSIC_PLAYLISTS\t1") {
            return Err(MusicError::Invalid("unsupported playlist store version"));
        }
        let mut store = Self::new();
        let mut current: Option<(String, Playlist)> = None;
        let mut line_count = 1usize;
        let mut total_entries = 0usize;
        for line in lines {
            line_count += 1;
            if line_count > MAX_NAMED_PLAYLISTS * (MAX_PLAYLIST_ENTRIES + 1) + 1 {
                return Err(MusicError::Limit("playlist store has too many records"));
            }
            if let Some(encoded_name) = line.strip_prefix("playlist\t") {
                if let Some((name, playlist)) = current.take() {
                    store.insert_parsed(name, playlist)?;
                }
                current = Some((decode_name(encoded_name)?, Playlist::new()));
                continue;
            }
            let Some(path) = line.strip_prefix("entry\t") else {
                return Err(MusicError::Invalid("invalid playlist store record"));
            };
            let (_, playlist) = current
                .as_mut()
                .ok_or(MusicError::Invalid("playlist entry precedes its name"))?;
            if path.len() > MAX_PLAYLIST_LINE_BYTES {
                return Err(MusicError::Limit("playlist path is too long"));
            }
            total_entries = total_entries.saturating_add(1);
            if total_entries > MAX_PLAYLIST_ENTRIES {
                return Err(MusicError::Limit("playlist store has too many entries"));
            }
            playlist.push(PathBuf::from(path))?;
        }
        if let Some((name, playlist)) = current {
            store.insert_parsed(name, playlist)?;
        }
        store.validate_size()?;
        Ok(store)
    }

    /// Atomically writes a dedicated `.rmp` state file via same-directory
    /// temporary file and rename. On Unix, the file is mode 0600 and the
    /// parent directory is synced. Other platforms use their native rename
    /// semantics and fail without deleting the old state if replacement fails.
    pub fn save_atomic(&self, destination: &Path) -> Result<()> {
        validate_state_path(destination)?;
        let contents = self.to_record()?;
        let parent = destination
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let canonical_parent = parent.canonicalize()?;
        if !canonical_parent.is_dir() {
            return Err(MusicError::Invalid(
                "playlist store parent is not a directory",
            ));
        }
        let target = canonical_parent.join(
            destination
                .file_name()
                .ok_or(MusicError::Invalid("playlist store filename is missing"))?,
        );
        reject_existing_symlink_or_nonfile(&target)?;

        let mut temporary = None;
        for _ in 0..16 {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let temp_path = canonical_parent.join(format!(
                ".rust-music-playlists-{}-{sequence}.tmp",
                std::process::id()
            ));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&temp_path) {
                Ok(file) => {
                    temporary = Some((temp_path, file));
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        let (temp_path, mut file) = temporary.ok_or(MusicError::Output(
            "could not allocate playlist state temporary file".into(),
        ))?;
        let write_result = (|| -> Result<()> {
            file.write_all(contents.as_bytes())?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temp_path, &target)?;
            #[cfg(unix)]
            File::open(&canonical_parent)?.sync_all()?;
            Ok(())
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        write_result
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        validate_state_path(path)?;
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let canonical_parent = parent.canonicalize()?;
        let target = canonical_parent.join(
            path.file_name()
                .ok_or(MusicError::Invalid("playlist store filename is missing"))?,
        );
        reject_existing_symlink_or_nonfile(&target)?;
        let mut bytes = Vec::new();
        File::open(target)?
            .take((MAX_PLAYLIST_STORE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        Self::from_record(&bytes)
    }

    fn insert_parsed(&mut self, name: String, playlist: Playlist) -> Result<()> {
        validate_name(&name)?;
        if self.playlists.iter().any(|item| item.name == name) {
            return Err(MusicError::Invalid("duplicate playlist name"));
        }
        if self.playlists.len() >= MAX_NAMED_PLAYLISTS {
            return Err(MusicError::Limit("too many named playlists"));
        }
        self.playlists.push(NamedPlaylist { name, playlist });
        Ok(())
    }

    fn validate_size(&self) -> Result<()> {
        if self.playlists.len() > MAX_NAMED_PLAYLISTS {
            return Err(MusicError::Limit("too many named playlists"));
        }
        // Check serialized size without allocating a second record.
        let mut bytes = STORE_HEADER.len();
        let mut total_entries = 0usize;
        for item in &self.playlists {
            validate_name(&item.name)?;
            bytes = bytes
                .checked_add("playlist\t\n".len() + item.name.len() * 2)
                .ok_or(MusicError::Limit("playlist store is too large"))?;
            for path in item.playlist.entries() {
                total_entries = total_entries.saturating_add(1);
                if total_entries > MAX_PLAYLIST_ENTRIES {
                    return Err(MusicError::Limit("playlist store has too many entries"));
                }
                let text = path
                    .to_str()
                    .ok_or(MusicError::Invalid("playlist path is not valid UTF-8"))?;
                bytes = bytes
                    .checked_add("entry\t\n".len() + text.len())
                    .ok_or(MusicError::Limit("playlist store is too large"))?;
            }
        }
        if bytes > MAX_PLAYLIST_STORE_BYTES {
            return Err(MusicError::Limit("playlist store is too large"));
        }
        Ok(())
    }
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > MAX_PLAYLIST_NAME_BYTES
        || name.trim().is_empty()
        || name.chars().any(char::is_control)
    {
        return Err(MusicError::Invalid("playlist name is invalid"));
    }
    Ok(())
}

fn decode_name(encoded: &str) -> Result<String> {
    if encoded.is_empty() || encoded.len() > MAX_PLAYLIST_NAME_BYTES * 2 || encoded.len() % 2 != 0 {
        return Err(MusicError::Invalid("encoded playlist name is invalid"));
    }
    let mut bytes = Vec::with_capacity(encoded.len() / 2);
    for pair in encoded.as_bytes().chunks_exact(2) {
        let digit = |byte: u8| match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        };
        let high = digit(pair[0]).ok_or(MusicError::Invalid("encoded playlist name is invalid"))?;
        let low = digit(pair[1]).ok_or(MusicError::Invalid("encoded playlist name is invalid"))?;
        bytes.push(high * 16 + low);
    }
    let name = String::from_utf8(bytes)
        .map_err(|_| MusicError::Invalid("playlist name is not valid UTF-8"))?;
    validate_name(&name)?;
    Ok(name)
}

fn validate_state_path(path: &Path) -> Result<()> {
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(MusicError::Invalid("playlist store filename is invalid"))?;
    if filename.is_empty()
        || filename.chars().any(char::is_control)
        || path.extension().and_then(|extension| extension.to_str()) != Some("rmp")
    {
        return Err(MusicError::Invalid(
            "playlist store must use a .rmp filename",
        ));
    }
    Ok(())
}

fn reject_existing_symlink_or_nonfile(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(MusicError::Invalid("playlist store cannot be a symlink"))
        }
        Ok(metadata) if !metadata.is_file() => Err(MusicError::Invalid(
            "playlist store destination is not a regular file",
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playlist() -> Playlist {
        let mut playlist = Playlist::new();
        playlist.push("Album/Café.flac".into()).unwrap();
        playlist.push("Album/live.wav".into()).unwrap();
        playlist
    }

    #[test]
    fn named_playlists_support_crud_and_versioned_round_trip() {
        let mut store = PlaylistStore::new();
        store.create("Café favorites").unwrap();
        store
            .push_entry("Café favorites", "Jazz/one.flac".into())
            .unwrap();
        store.rename("Café favorites", "Morning set").unwrap();
        assert!(store.create("Morning set").is_err());
        let encoded = store.to_record().unwrap();
        assert_eq!(
            PlaylistStore::from_record(encoded.as_bytes()).unwrap(),
            store
        );
        assert_eq!(store.remove("Morning set").unwrap().len(), 1);
        assert!(store.playlists().is_empty());
    }

    #[test]
    fn malformed_oversized_and_unsafe_records_fail_closed() {
        for record in [
            b"RUSTMUSIC_PLAYLISTS\t2\n".as_slice(),
            b"RUSTMUSIC_PLAYLISTS\t1\nentry\ta.flac\n",
            b"RUSTMUSIC_PLAYLISTS\t1\nplaylist\t0g\n",
            b"RUSTMUSIC_PLAYLISTS\t1\nplaylist\t61\nplaylist\t61\n",
            b"RUSTMUSIC_PLAYLISTS\t1\nplaylist\t61\nentry\t../x.flac\n",
        ] {
            assert!(PlaylistStore::from_record(record).is_err());
        }
        assert!(PlaylistStore::from_record(&vec![b'x'; MAX_PLAYLIST_STORE_BYTES + 1]).is_err());
        let mut store = PlaylistStore::new();
        assert!(store.create("\t").is_err());
        assert!(store
            .create(&"x".repeat(MAX_PLAYLIST_NAME_BYTES + 1))
            .is_err());
    }

    #[test]
    fn aggregate_limit_rejects_append_without_corrupting_store() {
        let mut store = PlaylistStore::new();
        store.create("large").unwrap();
        let path = PathBuf::from(format!("{}.flac", "x".repeat(MAX_PLAYLIST_LINE_BYTES - 5)));
        let mut rejected = false;
        for _ in 0..MAX_PLAYLIST_ENTRIES {
            if store.push_entry("large", path.clone()).is_err() {
                rejected = true;
                break;
            }
        }
        assert!(rejected);
        let record = store.to_record().unwrap();
        assert!(record.len() <= MAX_PLAYLIST_STORE_BYTES);
        assert_eq!(
            PlaylistStore::from_record(record.as_bytes()).unwrap(),
            store
        );
    }

    #[cfg(unix)]
    #[test]
    fn atomic_save_load_replaces_state_and_refuses_symlink_targets() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!("rmp-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir(&root).unwrap();
        let state_path = root.join("playlists.rmp");
        let mut first = PlaylistStore::new();
        first.create("first").unwrap();
        first.push_entry("first", "a.flac".into()).unwrap();
        first.save_atomic(&state_path).unwrap();
        assert_eq!(PlaylistStore::load_from(&state_path).unwrap(), first);

        let mut second = PlaylistStore::new();
        second.create("second").unwrap();
        second.save_atomic(&state_path).unwrap();
        assert_eq!(PlaylistStore::load_from(&state_path).unwrap(), second);

        let victim = root.join("victim.rmp");
        fs::write(&victim, b"untouched").unwrap();
        let link = root.join("linked.rmp");
        symlink(&victim, &link).unwrap();
        assert!(second.save_atomic(&link).is_err());
        assert_eq!(fs::read(&victim).unwrap(), b"untouched");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn model_never_changes_or_opens_audio_paths() {
        let before = playlist();
        let mut store = PlaylistStore::new();
        store.create("favorites").unwrap();
        for path in before.entries() {
            store.push_entry("favorites", path.clone()).unwrap();
        }
        let restored = PlaylistStore::from_record(store.to_record().unwrap().as_bytes()).unwrap();
        assert_eq!(restored.get("favorites"), Some(&before));
    }
}
