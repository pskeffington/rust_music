//! Bounded text-only playback queue state.
//!
//! Queue operations select a candidate but never start playback or acquire
//! audio focus. Callers must explicitly issue Play and resolve each relative
//! path through the selected library's checked-path API. Persistence stores
//! only queue control state and relative text paths; it never touches audio.

use rust_music_core::{MusicError, Result};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

/// Maximum queue entries and serialized bytes.
pub const MAX_QUEUE_ENTRIES: usize = 10_000;
pub const MAX_QUEUE_BYTES: usize = 1_048_576;
pub const MAX_QUEUE_PATH_BYTES: usize = 4096;
const MAX_QUEUE_LINES: usize = MAX_QUEUE_ENTRIES + 4;
const QUEUE_HEADER: &str = "RUSTMUSIC_QUEUE\t1\n";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// What candidate selection does at queue boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    Off,
    One,
    All,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackQueue {
    entries: Vec<PathBuf>,
    current: Option<usize>,
    repeat: RepeatMode,
    shuffle_seed: Option<u64>,
    entry_bytes: usize,
}

impl Default for PlaybackQueue {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            current: None,
            repeat: RepeatMode::Off,
            shuffle_seed: None,
            entry_bytes: 0,
        }
    }
}

impl PlaybackQueue {
    pub fn new() -> Self {
        Self::default()
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

    pub fn current_index(&self) -> Option<usize> {
        self.current
    }

    pub fn current(&self) -> Option<&Path> {
        self.current
            .and_then(|index| self.entries.get(index).map(PathBuf::as_path))
    }

    pub fn repeat_mode(&self) -> RepeatMode {
        self.repeat
    }

    pub fn set_repeat_mode(&mut self, repeat: RepeatMode) {
        self.repeat = repeat;
    }

    pub fn shuffle_seed(&self) -> Option<u64> {
        self.shuffle_seed
    }

    /// Append a root-relative supported-lossless path. This only changes queue
    /// state; it does not inspect the file or start playback.
    pub fn enqueue(&mut self, path: PathBuf) -> Result<()> {
        if self.entries.len() >= MAX_QUEUE_ENTRIES {
            return Err(MusicError::Limit("playback queue has too many entries"));
        }
        validate_audio_path(&path)?;
        let text = path
            .to_str()
            .ok_or(MusicError::Invalid("queue path is not valid UTF-8"))?;
        if text.len() > MAX_QUEUE_PATH_BYTES {
            return Err(MusicError::Limit("queue path is too long"));
        }
        let length = text
            .len()
            .checked_add(7) // "entry\t" plus newline
            .ok_or(MusicError::Limit("queue record is too large"))?;
        let bytes = QUEUE_HEADER
            .len()
            .checked_add(self.entry_bytes)
            .and_then(|n| n.checked_add(length))
            .and_then(|n| n.checked_add(96)) // header and bounded control records
            .ok_or(MusicError::Limit("queue record is too large"))?;
        if bytes > MAX_QUEUE_BYTES {
            return Err(MusicError::Limit("queue record is too large"));
        }
        self.entries.push(path);
        self.entry_bytes += length;
        Ok(())
    }

    /// Remove one item and adjust the selected index without selecting a new
    /// item. Removing the current item clears selection; it never autoplays.
    pub fn remove(&mut self, index: usize) -> Option<PathBuf> {
        if index >= self.entries.len() {
            return None;
        }
        let entry = self.entries.remove(index);
        self.entry_bytes -= entry.to_str().map_or(0, |text| text.len() + 7);
        self.current = match self.current {
            Some(current) if current == index => None,
            Some(current) if current > index => Some(current - 1),
            other => other,
        };
        Some(entry)
    }

    pub fn move_entry(&mut self, from: usize, to: usize) -> Result<()> {
        if from >= self.entries.len() || to >= self.entries.len() {
            return Err(MusicError::Invalid("queue index is out of bounds"));
        }
        if from == to {
            return Ok(());
        }
        let current = self.current;
        let entry = self.entries.remove(from);
        self.entries.insert(to, entry);
        self.current = current.map(|index| {
            if index == from {
                to
            } else if from < index && index <= to {
                index - 1
            } else if to <= index && index < from {
                index + 1
            } else {
                index
            }
        });
        Ok(())
    }

    /// Explicit selection primitive. Selection alone never starts playback.
    pub fn select(&mut self, index: usize) -> Result<&Path> {
        if index >= self.entries.len() {
            return Err(MusicError::Invalid("queue index is out of bounds"));
        }
        self.current = Some(index);
        Ok(self.entries[index].as_path())
    }

    /// Select the next candidate. The host must separately issue Play.
    pub fn next_candidate(&mut self) -> Option<&Path> {
        if self.entries.is_empty() {
            self.current = None;
            return None;
        }
        let next = match self.current {
            None => 0,
            Some(index) if self.repeat == RepeatMode::One => index,
            Some(index) if index + 1 < self.entries.len() => index + 1,
            Some(_) if self.repeat == RepeatMode::All => 0,
            Some(_) => return None,
        };
        self.current = Some(next);
        self.current()
    }

    /// Select the previous candidate. The host must separately issue Play.
    pub fn previous_candidate(&mut self) -> Option<&Path> {
        if self.entries.is_empty() {
            self.current = None;
            return None;
        }
        let previous = match self.current {
            None => self.entries.len() - 1,
            Some(index) if self.repeat == RepeatMode::One => index,
            Some(index) if index > 0 => index - 1,
            Some(_) if self.repeat == RepeatMode::All => self.entries.len() - 1,
            Some(_) => return None,
        };
        self.current = Some(previous);
        self.current()
    }

    /// Deterministic Fisher-Yates shuffle. If an item is selected, it remains
    /// selected after the order changes. The seed is persisted for replay.
    pub fn shuffle(&mut self, seed: u64) {
        self.shuffle_seed = Some(seed);
        let current_path = self.current().map(Path::to_path_buf);
        let mut state = seed;
        for index in (1..self.entries.len()).rev() {
            let random = splitmix64(&mut state);
            let other = (random % (index as u64 + 1)) as usize;
            self.entries.swap(index, other);
        }
        self.current =
            current_path.and_then(|path| self.entries.iter().position(|item| *item == path));
    }

    /// Disable shuffle metadata without trying to reconstruct a prior order.
    pub fn clear_shuffle(&mut self) {
        self.shuffle_seed = None;
    }

    pub fn to_record(&self) -> Result<String> {
        self.validate()?;
        let mut out = String::with_capacity(QUEUE_HEADER.len() + self.entry_bytes + 96);
        out.push_str(QUEUE_HEADER);
        out.push_str("repeat\t");
        out.push_str(match self.repeat {
            RepeatMode::Off => "off\n",
            RepeatMode::One => "one\n",
            RepeatMode::All => "all\n",
        });
        out.push_str("current\t");
        match self.current {
            Some(index) => out.push_str(&index.to_string()),
            None => out.push('-'),
        }
        out.push('\n');
        out.push_str("shuffle\t");
        match self.shuffle_seed {
            Some(seed) => out.push_str(&seed.to_string()),
            None => out.push('-'),
        }
        out.push('\n');
        for path in &self.entries {
            out.push_str("entry\t");
            out.push_str(path.to_str().expect("queue paths validated as UTF-8"));
            out.push('\n');
        }
        if out.len() > MAX_QUEUE_BYTES {
            return Err(MusicError::Limit("queue record is too large"));
        }
        Ok(out)
    }

    pub fn from_record(input: &[u8]) -> Result<Self> {
        if input.len() > MAX_QUEUE_BYTES {
            return Err(MusicError::Limit("queue record is too large"));
        }
        let text = std::str::from_utf8(input)
            .map_err(|_| MusicError::Invalid("queue record is not UTF-8"))?;
        let mut lines = text.lines();
        if lines.next() != Some("RUSTMUSIC_QUEUE\t1") {
            return Err(MusicError::Invalid("unsupported queue record version"));
        }
        let repeat = match parse_field(&mut lines, "repeat")? {
            "off" => RepeatMode::Off,
            "one" => RepeatMode::One,
            "all" => RepeatMode::All,
            _ => return Err(MusicError::Invalid("invalid queue repeat mode")),
        };
        let current = match parse_field(&mut lines, "current")? {
            "-" => None,
            value => Some(parse_usize(value)?),
        };
        let shuffle_seed = match parse_field(&mut lines, "shuffle")? {
            "-" => None,
            value => Some(
                value
                    .parse::<u64>()
                    .map_err(|_| MusicError::Invalid("invalid queue shuffle seed"))?,
            ),
        };

        let mut queue = Self {
            repeat,
            shuffle_seed,
            ..Self::new()
        };
        let mut line_count = 4usize;
        for line in lines {
            line_count += 1;
            if line_count > MAX_QUEUE_LINES {
                return Err(MusicError::Limit("queue record has too many lines"));
            }
            if line.len() > MAX_QUEUE_PATH_BYTES + 6 {
                return Err(MusicError::Limit("queue record line is too long"));
            }
            let Some(path) = line.strip_prefix("entry\t") else {
                return Err(MusicError::Invalid("invalid queue record line"));
            };
            queue.enqueue(PathBuf::from(path))?;
        }
        if current.is_some_and(|index| index >= queue.entries.len()) {
            return Err(MusicError::Invalid("queue current index is out of bounds"));
        }
        queue.current = current;
        queue.validate()?;
        Ok(queue)
    }

    /// Atomically replace a dedicated `.rmq` state file on Unix. This refuses
    /// a symlink or non-file destination and canonicalizes the parent. As with
    /// library path checking, a hostile local filesystem actor can still race
    /// path replacement; hosts needing that threat model should use openat.
    #[cfg(unix)]
    pub fn save_atomic(&self, destination: &Path) -> Result<()> {
        use std::os::unix::fs::OpenOptionsExt;

        validate_state_path(destination)?;
        let contents = self.to_record()?;
        let parent = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let canonical_parent = parent.canonicalize()?;
        if !canonical_parent.is_dir() {
            return Err(MusicError::Invalid("queue state parent is not a directory"));
        }
        let name = destination
            .file_name()
            .ok_or(MusicError::Invalid("queue state filename is missing"))?;
        let target = canonical_parent.join(name);
        reject_existing_symlink_or_nonfile(&target)?;

        let mut temporary = None;
        for _ in 0..16 {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let temp_name = format!(".rust-music-queue-{}-{sequence}.tmp", std::process::id());
            let temp_path = canonical_parent.join(temp_name);
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temp_path)
            {
                Ok(file) => {
                    temporary = Some((temp_path, file));
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        let (temp_path, mut file) = temporary.ok_or(MusicError::Output(
            "could not allocate a unique queue state temporary file".into(),
        ))?;
        let write_result = (|| -> Result<()> {
            file.write_all(contents.as_bytes())?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temp_path, &target)?;
            File::open(&canonical_parent)?.sync_all()?;
            Ok(())
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        write_result
    }

    #[cfg(not(unix))]
    pub fn save_atomic(&self, _destination: &Path) -> Result<()> {
        Err(MusicError::Unsupported(
            "atomic queue persistence is currently supported on Unix targets",
        ))
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        validate_state_path(path)?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let canonical_parent = parent.canonicalize()?;
        let target = canonical_parent.join(
            path.file_name()
                .ok_or(MusicError::Invalid("queue state filename is missing"))?,
        );
        reject_existing_symlink_or_nonfile(&target)?;
        let file = File::open(target)?;
        let mut bytes = Vec::new();
        file.take((MAX_QUEUE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        Self::from_record(&bytes)
    }

    fn validate(&self) -> Result<()> {
        if self.entries.len() > MAX_QUEUE_ENTRIES {
            return Err(MusicError::Limit("playback queue has too many entries"));
        }
        if self
            .current
            .is_some_and(|index| index >= self.entries.len())
        {
            return Err(MusicError::Invalid("queue current index is out of bounds"));
        }
        let mut bytes = 0usize;
        for path in &self.entries {
            validate_audio_path(path)?;
            let text = path
                .to_str()
                .ok_or(MusicError::Invalid("queue path is not valid UTF-8"))?;
            bytes = bytes
                .checked_add(text.len() + 7)
                .ok_or(MusicError::Limit("queue record is too large"))?;
        }
        if QUEUE_HEADER.len() + bytes + 96 > MAX_QUEUE_BYTES {
            return Err(MusicError::Limit("queue record is too large"));
        }
        Ok(())
    }
}

fn validate_state_path(path: &Path) -> Result<()> {
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(MusicError::Invalid("queue state filename is invalid"))?;
    if filename.is_empty()
        || filename.chars().any(char::is_control)
        || path.extension().and_then(|ext| ext.to_str()) != Some("rmq")
        || path.file_name().is_none()
    {
        return Err(MusicError::Invalid("queue state must use a .rmq filename"));
    }
    Ok(())
}

fn reject_existing_symlink_or_nonfile(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(MusicError::Invalid("queue state cannot be a symlink"))
        }
        Ok(metadata) if !metadata.is_file() => Err(MusicError::Invalid(
            "queue state destination is not a regular file",
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn parse_field<'a>(lines: &mut impl Iterator<Item = &'a str>, name: &str) -> Result<&'a str> {
    let line = lines
        .next()
        .ok_or(MusicError::Invalid("truncated queue record"))?;
    let (key, value) = line
        .split_once('\t')
        .ok_or(MusicError::Invalid("invalid queue control record"))?;
    if key != name || value.is_empty() {
        return Err(MusicError::Invalid("invalid queue control record"));
    }
    Ok(value)
}

fn parse_usize(value: &str) -> Result<usize> {
    value
        .parse::<usize>()
        .map_err(|_| MusicError::Invalid("invalid queue current index"))
}

fn validate_audio_path(path: &Path) -> Result<()> {
    let text = path
        .to_str()
        .ok_or(MusicError::Invalid("queue path is not valid UTF-8"))?;
    if text.is_empty()
        || text.chars().any(char::is_control)
        || text.contains('\\')
        || text.contains(':')
    {
        return Err(MusicError::Invalid("queue path contains unsafe text"));
    }
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(MusicError::Invalid("queue path is not root-relative"));
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
            "queue entry has an unsupported lossless container suffix",
        ));
    }
    Ok(())
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut value = *state;
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queue() -> PlaybackQueue {
        let mut queue = PlaybackQueue::new();
        queue.enqueue("Album/one.flac".into()).unwrap();
        queue.enqueue("Album/two.wav".into()).unwrap();
        queue.enqueue("Album/three.aiff".into()).unwrap();
        queue
    }

    #[test]
    fn navigation_repeat_and_selection_never_autoplay() {
        let mut queue = queue();
        assert_eq!(queue.current(), None);
        assert_eq!(queue.next_candidate(), Some(Path::new("Album/one.flac")));
        assert_eq!(queue.current_index(), Some(0));
        assert_eq!(queue.next_candidate(), Some(Path::new("Album/two.wav")));
        assert_eq!(
            queue.previous_candidate(),
            Some(Path::new("Album/one.flac"))
        );
        queue.select(2).unwrap();
        assert_eq!(queue.next_candidate(), None);
        assert_eq!(queue.current(), Some(Path::new("Album/three.aiff")));
        queue.set_repeat_mode(RepeatMode::One);
        assert_eq!(queue.next_candidate(), Some(Path::new("Album/three.aiff")));
        assert_eq!(
            queue.previous_candidate(),
            Some(Path::new("Album/three.aiff"))
        );
        queue.set_repeat_mode(RepeatMode::All);
        assert_eq!(queue.next_candidate(), Some(Path::new("Album/one.flac")));
        assert_eq!(
            queue.previous_candidate(),
            Some(Path::new("Album/three.aiff"))
        );
    }

    #[test]
    fn move_and_remove_preserve_selection_without_autoplay() {
        let mut queue = queue();
        queue.select(1).unwrap();
        queue.move_entry(1, 0).unwrap();
        assert_eq!(queue.current(), Some(Path::new("Album/two.wav")));
        assert_eq!(queue.current_index(), Some(0));
        queue.remove(0).unwrap();
        assert_eq!(queue.current(), None);
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn seeded_shuffle_is_replayable_and_preserves_current_item() {
        let mut first = queue();
        first.select(1).unwrap();
        first.shuffle(0x1234_5678);
        let shuffled = first.entries().to_vec();
        assert_eq!(first.current(), Some(Path::new("Album/two.wav")));
        let mut second = queue();
        second.shuffle(0x1234_5678);
        assert_eq!(second.entries(), shuffled);
        assert_eq!(first.shuffle_seed(), Some(0x1234_5678));
        first.clear_shuffle();
        assert_eq!(first.shuffle_seed(), None);
    }

    #[test]
    fn versioned_record_round_trip_and_malformed_inputs_fail_closed() {
        let mut queue = queue();
        queue.select(1).unwrap();
        queue.set_repeat_mode(RepeatMode::All);
        queue.shuffle(99);
        let record = queue.to_record().unwrap();
        assert_eq!(
            PlaybackQueue::from_record(record.as_bytes()).unwrap(),
            queue
        );
        for bad in [
            b"RUSTMUSIC_QUEUE\t2\n".as_slice(),
            b"RUSTMUSIC_QUEUE\t1\nrepeat\tbad\ncurrent\t-\nshuffle\t-\n",
            b"RUSTMUSIC_QUEUE\t1\nrepeat\toff\ncurrent\t9\nshuffle\t-\nentry\ta.flac\n",
            b"RUSTMUSIC_QUEUE\t1\nrepeat\toff\ncurrent\t-\nshuffle\t-\nentry\t../a.flac\n",
            b"RUSTMUSIC_QUEUE\t1\nrepeat\toff\ncurrent\t-\nshuffle\t-\nentry\ta.mp3\n",
        ] {
            assert!(PlaybackQueue::from_record(bad).is_err());
        }
    }

    #[test]
    fn bounds_entry_count_path_line_and_record_bytes() {
        let mut queue = PlaybackQueue::new();
        let too_long = format!("{}.flac", "x".repeat(MAX_QUEUE_PATH_BYTES));
        assert!(queue.enqueue(PathBuf::from(too_long)).is_err());
        for index in 0..MAX_QUEUE_ENTRIES {
            queue.enqueue(format!("{index}.flac").into()).unwrap();
        }
        assert!(queue.enqueue("overflow.flac".into()).is_err());
        assert!(queue.to_record().unwrap().len() <= MAX_QUEUE_BYTES);
    }

    #[cfg(unix)]
    #[test]
    fn atomic_save_load_refuses_symlinks_and_replaces_only_queue_file() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!("rmq-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir(&root).unwrap();
        let state = root.join("queue.rmq");
        let audio = root.join("never-touch.flac");
        fs::write(&audio, b"keep").unwrap();
        let mut queue = queue();
        queue.select(1).unwrap();
        queue.set_repeat_mode(RepeatMode::One);
        queue.save_atomic(&state).unwrap();
        assert_eq!(PlaybackQueue::load_from(&state).unwrap(), queue);
        fs::write(&state, "RUSTMUSIC_QUEUE\t1\n").unwrap();
        queue.save_atomic(&state).unwrap();
        assert_eq!(PlaybackQueue::load_from(&state).unwrap(), queue);

        let link = root.join("linked.rmq");
        symlink(&audio, &link).unwrap();
        assert!(queue.save_atomic(&link).is_err());
        assert!(PlaybackQueue::load_from(&link).is_err());
        assert_eq!(fs::read(&audio).unwrap(), b"keep");
        assert!(queue.save_atomic(&root.join("not-a-queue.txt")).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
