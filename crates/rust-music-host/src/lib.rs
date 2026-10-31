//! Rust_Boy-compatible, headless music slot. No GUI framework, background task,
//! playback thread, network calls, device discovery, or autoplay.
//!
//! The host supplies a short-lived exclusive audio-focus lease. Holding the
//! lease inside MusicSlot ensures it is relinquished on stop, EOF, errors, and
//! destruction, without assuming which other Rust_Boy module owns the device.

use rust_music::{open_file, MusicError, PcmSink, PlaybackSnapshot, PlaybackState, Player, Result};
use rust_music_library::{
    queue::{PlaybackQueue, RepeatMode},
    Library,
};
use std::path::Path;

/// Stable Pip-Boy package identity. It is deliberately distinct from Radio's
/// `org.rustboy.radio`; the host registers each module as its own package.
pub const MUSIC_PACKAGE_ID: &str = "org.rustboy.music";

/// Host-owned RAII lease. Its Drop implementation MUST release audio focus.
/// Music never manufactures a lease or assumes that RADIO/TV is stopped.
pub trait AudioLease: Send {}

/// Implement in Rust_Boy using its single arbitration mechanism for TV, RADIO,
/// and MUSIC. Return None when another module holds output ownership.
pub trait AudioFocus: Send + Sync {
    fn try_acquire_music(&self) -> Option<Box<dyn AudioLease>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Select(usize),
    Next,
    Previous,
    Play,
    Pause,
    ResumePlayback,
    Stop,
    /// Software attenuation, 0 (silent) through 100 (exact decoded PCM).
    SetVolumePercent(u8),
    /// Reposition a seek-capable decoder; successful seeks stop output and do
    /// not autoplay. Frames are sample frames per channel, not PCM samples.
    SeekFrame(u64),
    /// Add the current checked catalog track to the in-memory playback queue.
    /// This never starts output or acquires audio focus.
    EnqueueSelection,
    /// Select a queue entry after resolving it against the checked catalog.
    /// Selection stops existing playback but never starts the new candidate.
    SelectQueue(usize),
    /// Select the next or previous queue candidate without starting playback.
    QueueNext,
    QueuePrevious,
    SetRepeat(RepeatMode),
    /// Set a deterministic shuffle seed, or `None` to clear shuffle metadata.
    ShuffleQueue(Option<u64>),
    /// Process exactly one decoded block. The caller schedules a worker tick.
    Poll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotSnapshot {
    pub selection: Option<usize>,
    pub tracks: usize,
    pub playback: PlaybackSnapshot,
    pub has_audio_focus: bool,
    pub queue_entries: usize,
    pub queue_selection: Option<usize>,
    pub repeat_mode: RepeatMode,
    pub shuffle_seed: Option<u64>,
}

/// One Rust_Boy pop-in music module. There is exactly one player and sink.
pub struct MusicSlot<S: PcmSink> {
    player: Player<S>,
    library: Library,
    selected: Option<usize>,
    queue: PlaybackQueue,
    focus_lease: Option<Box<dyn AudioLease>>,
}

impl<S: PcmSink> MusicSlot<S> {
    pub fn new(sink: S, library: Library) -> Self {
        let selected = (!library.is_empty()).then_some(0);
        Self {
            player: Player::new(sink),
            library,
            selected,
            queue: PlaybackQueue::new(),
            focus_lease: None,
        }
    }
    pub fn library(&self) -> &Library {
        &self.library
    }
    pub fn sink(&self) -> &S {
        self.player.sink()
    }
    /// Read-only queue state. Persist through `PlaybackQueue::save_atomic` or
    /// serialize with `to_record`; queue selection itself never starts audio.
    pub fn queue(&self) -> &PlaybackQueue {
        &self.queue
    }
    /// Replace persisted queue state without selecting or starting its current
    /// item. Entries are resolved against this slot's current catalog only
    /// when a separate queue-selection command is issued.
    pub fn replace_queue(&mut self, queue: PlaybackQueue) -> SlotSnapshot {
        self.queue = queue;
        self.status()
    }
    pub fn status(&self) -> SlotSnapshot {
        SlotSnapshot {
            selection: self.selected,
            tracks: self.library.len(),
            playback: self.player.status(),
            has_audio_focus: self.focus_lease.is_some(),
            queue_entries: self.queue.len(),
            queue_selection: self.queue.current_index(),
            repeat_mode: self.queue.repeat_mode(),
            shuffle_seed: self.queue.shuffle_seed(),
        }
    }

    /// The caller owns scan timing and file permissions. Replacing the catalog
    /// always stops playback, preserves the previous relative path if present,
    /// and never starts a different song automatically.
    pub fn replace_library(&mut self, catalog: Library) -> Result<SlotSnapshot> {
        let same_root = self.library.root() == catalog.root();
        let old_path = self
            .selected
            .and_then(|i| self.library.track(i))
            .map(|t| t.relative_path.clone());
        self.stop()?;
        self.selected = old_path
            .and_then(|p| catalog.tracks().iter().position(|t| t.relative_path == p))
            .or_else(|| (!catalog.is_empty()).then_some(0));
        self.library = catalog;
        if !same_root {
            // Queue paths are relative to one selected root. Without a root
            // identity in the persisted queue record, retaining them across
            // roots could silently target unrelated files with equal names.
            self.queue = PlaybackQueue::new();
        }
        Ok(self.status())
    }

    /// Relinquish foreground playback while keeping this module mounted.
    /// Selection and catalog remain available; returning to Music does not
    /// autoplay. The operator must issue a fresh `Play` after pop-in.
    pub fn suspend(&mut self) -> Result<SlotSnapshot> {
        self.stop()?;
        Ok(self.status())
    }

    pub fn handle(
        &mut self,
        command: Command,
        audio_focus: &dyn AudioFocus,
    ) -> Result<SlotSnapshot> {
        match command {
            Command::Select(index) => self.select(index)?,
            Command::Next => self.move_selection(true)?,
            Command::Previous => self.move_selection(false)?,
            Command::Play => self.play(audio_focus)?,
            Command::Pause => self.pause()?,
            Command::ResumePlayback => self.resume_playback(audio_focus)?,
            Command::Stop => self.stop()?,
            Command::SetVolumePercent(percent) => self.player.set_volume_percent(percent)?,
            Command::SeekFrame(frame) => self.seek_frame(frame)?,
            Command::EnqueueSelection => self.enqueue_selection()?,
            Command::SelectQueue(index) => self.select_queue(index)?,
            Command::QueueNext => self.move_queue_candidate(true)?,
            Command::QueuePrevious => self.move_queue_candidate(false)?,
            Command::SetRepeat(mode) => self.queue.set_repeat_mode(mode),
            Command::ShuffleQueue(Some(seed)) => self.queue.shuffle(seed),
            Command::ShuffleQueue(None) => self.queue.clear_shuffle(),
            Command::Poll => self.poll()?,
        }
        Ok(self.status())
    }

    fn select(&mut self, index: usize) -> Result<()> {
        if self.library.track(index).is_none() {
            return Err(MusicError::Invalid("selected music track does not exist"));
        }
        if self.selected != Some(index) {
            self.stop()?;
            self.selected = Some(index);
        }
        Ok(())
    }
    fn enqueue_selection(&mut self) -> Result<()> {
        let index = self
            .selected
            .ok_or(MusicError::Invalid("no music track is selected"))?;
        self.library.checked_path(index)?;
        let relative_path = self
            .library
            .track(index)
            .ok_or(MusicError::Invalid("selected music track does not exist"))?
            .relative_path
            .clone();
        // The check above deliberately revalidates presence/symlink state but
        // queue insertion stores only the catalog-relative text path.
        self.queue.enqueue(relative_path)
    }
    fn select_queue(&mut self, queue_index: usize) -> Result<()> {
        let path = self
            .queue
            .entries()
            .get(queue_index)
            .ok_or(MusicError::Invalid("queue selection is out of bounds"))?
            .clone();
        let track_index = match self.resolve_queue_path(&path) {
            Ok(index) => index,
            Err(error) => {
                // A stale queue target must not leave the previously playing
                // track and its host-owned focus lease active after the user
                // explicitly requested a different queue item.
                self.stop()?;
                return Err(error);
            }
        };
        self.queue.select(queue_index)?;
        self.select(track_index)
    }
    fn move_queue_candidate(&mut self, forwards: bool) -> Result<()> {
        let path = if forwards {
            self.queue.next_candidate()
        } else {
            self.queue.previous_candidate()
        }
        .ok_or(MusicError::Invalid("queue has no next candidate"))?
        .to_path_buf();
        let track_index = match self.resolve_queue_path(&path) {
            Ok(index) => index,
            Err(error) => {
                self.stop()?;
                return Err(error);
            }
        };
        self.select(track_index)
    }
    fn resolve_queue_path(&self, path: &Path) -> Result<usize> {
        let index = self
            .library
            .tracks()
            .iter()
            .position(|track| track.relative_path == path)
            .ok_or(MusicError::Invalid(
                "queue candidate is missing from the current library",
            ))?;
        self.library.checked_path(index)?;
        Ok(index)
    }
    fn move_selection(&mut self, forwards: bool) -> Result<()> {
        let count = self.library.len();
        if count == 0 {
            return Ok(());
        }
        let next = match self.selected {
            Some(i) if forwards => (i + 1) % count,
            Some(0) if !forwards => count - 1,
            Some(i) => i - 1,
            None => 0,
        };
        self.select(next)
    }
    fn play(&mut self, audio_focus: &dyn AudioFocus) -> Result<()> {
        if self.player.status().state == PlaybackState::Playing {
            return Ok(()); // A repeated Play must not open a second sink.
        }
        if self.player.status().state == PlaybackState::Paused {
            return self.resume_playback(audio_focus);
        }
        let selection = self
            .selected
            .ok_or(MusicError::Invalid("no track selected"))?;
        if self.player.status().state != PlaybackState::Ready {
            let path = self.library.checked_path(selection)?;
            self.player.load(open_file(path)?)?;
        }
        let lease = audio_focus.try_acquire_music().ok_or_else(|| {
            MusicError::Output("audio focus belongs to another Rust_Boy module".into())
        })?;
        self.focus_lease = Some(lease);
        if let Err(error) = self.player.play() {
            self.focus_lease = None;
            return Err(error);
        }
        Ok(())
    }
    fn pause(&mut self) -> Result<()> {
        // A failed backend pause is failed closed by Player::pause. Drop the
        // host lease in either case so RADIO/TV are not blocked by a dead
        // stream.
        let result = self.player.pause();
        self.focus_lease = None;
        result
    }
    fn resume_playback(&mut self, audio_focus: &dyn AudioFocus) -> Result<()> {
        let lease = audio_focus.try_acquire_music().ok_or_else(|| {
            MusicError::Output("audio focus belongs to another Rust_Boy module".into())
        })?;
        self.focus_lease = Some(lease);
        if let Err(error) = self.player.resume() {
            self.focus_lease = None;
            return Err(error);
        }
        Ok(())
    }
    fn poll(&mut self) -> Result<()> {
        if self.player.status().state != PlaybackState::Playing {
            return Ok(());
        }
        match self.player.step() {
            Ok(snapshot) => {
                if snapshot.state != PlaybackState::Playing {
                    self.focus_lease = None;
                }
                Ok(())
            }
            Err(error) => {
                self.focus_lease = None;
                Err(error)
            }
        }
    }
    fn stop(&mut self) -> Result<()> {
        let result = self.player.stop();
        self.focus_lease = None; // Release even when native close fails.
        result
    }
    fn seek_frame(&mut self, frame: u64) -> Result<()> {
        let result = self.player.seek_frame(frame);
        if self.player.status().state != PlaybackState::Playing {
            self.focus_lease = None;
        }
        result
    }
}

impl<S: PcmSink> Drop for MusicSlot<S> {
    fn drop(&mut self) {
        let _ = self.player.stop();
        self.focus_lease = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_music::NullSink;
    use rust_music_library::{scan, ScanLimits};
    use std::{
        fs,
        path::PathBuf,
        sync::{
            atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
            Arc,
        },
        time::{SystemTime, UNIX_EPOCH},
    };

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    fn crc8(bytes: &[u8]) -> u8 {
        bytes.iter().fold(0u8, |mut crc, byte| {
            crc ^= byte;
            for _ in 0..8 {
                crc = if crc & 0x80 != 0 {
                    (crc << 1) ^ 0x07
                } else {
                    crc << 1
                };
            }
            crc
        })
    }

    fn crc16(bytes: &[u8]) -> u16 {
        bytes.iter().fold(0u16, |mut crc, byte| {
            crc ^= u16::from(*byte) << 8;
            for _ in 0..8 {
                crc = if crc & 0x8000 != 0 {
                    (crc << 1) ^ 0x8005
                } else {
                    crc << 1
                };
            }
            crc
        })
    }

    #[derive(Clone, Default)]
    struct TestFocus {
        claimed: Arc<AtomicBool>,
        attempts: Arc<AtomicUsize>,
    }
    struct TestLease(Arc<AtomicBool>);
    impl AudioLease for TestLease {}
    impl Drop for TestLease {
        fn drop(&mut self) {
            self.0.store(false, Ordering::Release);
        }
    }
    impl AudioFocus for TestFocus {
        fn try_acquire_music(&self) -> Option<Box<dyn AudioLease>> {
            self.attempts.fetch_add(1, Ordering::Relaxed);
            self.claimed
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .ok()
                .map(|_| Box::new(TestLease(self.claimed.clone())) as Box<dyn AudioLease>)
        }
    }

    #[derive(Default)]
    struct FailOpenSink {
        open_attempts: usize,
        finish_attempts: usize,
    }
    impl PcmSink for FailOpenSink {
        fn open(&mut self, _format: rust_music::AudioFormat) -> Result<()> {
            self.open_attempts += 1;
            Err(MusicError::Output("injected device-open failure".into()))
        }
        fn write(&mut self, _block: &rust_music::PcmBlock) -> Result<()> {
            Err(MusicError::Invalid("write must not follow a failed open"))
        }
        fn finish(&mut self) -> Result<()> {
            self.finish_attempts += 1;
            Ok(())
        }
    }

    #[derive(Default)]
    struct FailWriteSink {
        finish_attempts: usize,
        opened: Option<rust_music::AudioFormat>,
    }
    impl PcmSink for FailWriteSink {
        fn open(&mut self, format: rust_music::AudioFormat) -> Result<()> {
            self.opened = Some(format);
            Ok(())
        }
        fn write(&mut self, _block: &rust_music::PcmBlock) -> Result<()> {
            Err(MusicError::Output("injected device-write failure".into()))
        }
        fn finish(&mut self) -> Result<()> {
            self.finish_attempts += 1;
            self.opened = None;
            Ok(())
        }
    }

    struct RecoveringSink {
        available: Arc<AtomicBool>,
        open_attempts: usize,
        successful_writes: usize,
        finish_attempts: usize,
        opened: bool,
    }
    impl PcmSink for RecoveringSink {
        fn open(&mut self, _format: rust_music::AudioFormat) -> Result<()> {
            self.open_attempts += 1;
            if !self.available.load(Ordering::Acquire) {
                return Err(MusicError::Output("injected unavailable device".into()));
            }
            self.opened = true;
            Ok(())
        }
        fn write(&mut self, _block: &rust_music::PcmBlock) -> Result<()> {
            if !self.available.load(Ordering::Acquire) {
                self.opened = false;
                return Err(MusicError::Output("injected device disconnect".into()));
            }
            self.successful_writes += 1;
            Ok(())
        }
        fn finish(&mut self) -> Result<()> {
            self.finish_attempts += 1;
            self.opened = false;
            Ok(())
        }
    }

    #[derive(Default)]
    struct FailFinishSink {
        finish_attempts: usize,
        opened: bool,
    }
    impl PcmSink for FailFinishSink {
        fn open(&mut self, _format: rust_music::AudioFormat) -> Result<()> {
            self.opened = true;
            Ok(())
        }
        fn write(&mut self, _block: &rust_music::PcmBlock) -> Result<()> {
            if self.opened {
                Ok(())
            } else {
                Err(MusicError::Output("injected write while closed".into()))
            }
        }
        fn finish(&mut self) -> Result<()> {
            self.finish_attempts += 1;
            self.opened = false;
            Err(MusicError::Output("injected device-drain failure".into()))
        }
    }

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            loop {
                let sequence = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!(
                    "rust-music-slot-{}-{stamp}-{sequence}",
                    std::process::id()
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("failed to create test temp directory: {error}"),
                }
            }
        }
        fn wav(&self, name: &str) {
            let mut bytes = b"RIFF".to_vec();
            bytes.extend_from_slice(&38u32.to_le_bytes());
            bytes.extend_from_slice(b"WAVEfmt ");
            bytes.extend_from_slice(&16u32.to_le_bytes());
            bytes.extend_from_slice(&1u16.to_le_bytes());
            bytes.extend_from_slice(&1u16.to_le_bytes());
            bytes.extend_from_slice(&44_100u32.to_le_bytes());
            bytes.extend_from_slice(&88_200u32.to_le_bytes());
            bytes.extend_from_slice(&2u16.to_le_bytes());
            bytes.extend_from_slice(&16u16.to_le_bytes());
            bytes.extend_from_slice(b"data");
            bytes.extend_from_slice(&2u32.to_le_bytes());
            bytes.extend_from_slice(&123i16.to_le_bytes());
            fs::write(self.0.join(name), bytes).unwrap();
        }
        fn flac(&self, name: &str) {
            let mut bytes = b"fLaC".to_vec();
            bytes.extend_from_slice(&[0, 0, 0, 34]);
            bytes.extend_from_slice(&16u16.to_be_bytes());
            bytes.extend_from_slice(&16u16.to_be_bytes());
            bytes.extend_from_slice(&[0; 6]);
            let packed = (44_100u64 << 44) | (15u64 << 36) | 16;
            bytes.extend_from_slice(&packed.to_be_bytes());
            bytes.extend_from_slice(&[0; 16]);
            bytes.extend_from_slice(&[0x83, 0, 0, 18]);
            bytes.extend_from_slice(&0u64.to_be_bytes());
            bytes.extend_from_slice(&0u64.to_be_bytes());
            bytes.extend_from_slice(&16u16.to_be_bytes());

            let mut frame = vec![0xff, 0xf8, 0x60, 0, 0, 15];
            frame.push(crc8(&frame));
            frame.extend_from_slice(&[0, 0x01, 0x00]);
            frame.extend_from_slice(&crc16(&frame).to_be_bytes());
            bytes.extend_from_slice(&frame);
            fs::write(self.0.join(name), bytes).unwrap();
        }
        fn low_quality_wav(&self, name: &str) {
            // Valid 8-bit mono 44.1k PCM: decodable for tests, not playable.
            let mut bytes = b"RIFF".to_vec();
            bytes.extend_from_slice(&38u32.to_le_bytes());
            bytes.extend_from_slice(b"WAVEfmt ");
            bytes.extend_from_slice(&16u32.to_le_bytes());
            bytes.extend_from_slice(&1u16.to_le_bytes());
            bytes.extend_from_slice(&1u16.to_le_bytes());
            bytes.extend_from_slice(&44_100u32.to_le_bytes());
            bytes.extend_from_slice(&44_100u32.to_le_bytes());
            bytes.extend_from_slice(&1u16.to_le_bytes());
            bytes.extend_from_slice(&8u16.to_le_bytes());
            bytes.extend_from_slice(b"data");
            bytes.extend_from_slice(&1u32.to_le_bytes());
            bytes.extend_from_slice(&[128, 0]);
            fs::write(self.0.join(name), bytes).unwrap();
        }
        fn scan(&self) -> Library {
            scan(&self.0, ScanLimits::default()).unwrap()
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn below_cd_quality_source_never_claims_radio_focus_or_opens_sink() {
        let temp = Temp::new();
        temp.low_quality_wav("low.wav");
        let focus = TestFocus::default();
        let mut slot = MusicSlot::new(NullSink::default(), temp.scan());
        assert_eq!(slot.status().tracks, 1);
        assert!(matches!(
            slot.handle(Command::Play, &focus),
            Err(MusicError::Unsupported(_))
        ));
        assert!(!focus.claimed.load(Ordering::Acquire));
        assert!(!slot.status().has_audio_focus);
        assert_eq!(slot.status().playback.state, PlaybackState::Empty);
        assert!(slot.sink().opened.is_none());
    }

    #[test]
    fn no_autoplay_and_focus_released_at_end_of_file() {
        let temp = Temp::new();
        temp.wav("b.wav");
        temp.wav("a.wav");
        let focus = TestFocus::default();
        let mut slot = MusicSlot::new(NullSink::default(), temp.scan());
        assert_eq!(slot.status().tracks, 2);
        assert_eq!(slot.status().playback.state, PlaybackState::Empty);
        assert_eq!(slot.sink().frames_written, 0);
        slot.handle(Command::Play, &focus).unwrap();
        assert!(focus.claimed.load(Ordering::Acquire));
        slot.handle(Command::Play, &focus).unwrap(); // idempotent
        slot.handle(Command::Poll, &focus).unwrap();
        assert_eq!(slot.sink().frames_written, 1);
        slot.handle(Command::Poll, &focus).unwrap();
        assert_eq!(slot.status().playback.state, PlaybackState::Finished);
        assert!(!focus.claimed.load(Ordering::Acquire));
    }

    #[test]
    fn focus_denial_does_not_start_output_or_displace_radio() {
        let temp = Temp::new();
        temp.wav("a.wav");
        let focus = TestFocus::default();
        focus.claimed.store(true, Ordering::Release); // RADIO already owns audio.
        let mut slot = MusicSlot::new(NullSink::default(), temp.scan());
        assert!(slot.handle(Command::Play, &focus).is_err());
        assert_eq!(slot.status().playback.state, PlaybackState::Ready);
        assert_eq!(slot.sink().frames_written, 0);
        assert!(!slot.status().has_audio_focus);
        assert!(focus.claimed.load(Ordering::Acquire));
        focus.claimed.store(false, Ordering::Release);
        slot.handle(Command::Play, &focus).unwrap();
        slot.handle(Command::Stop, &focus).unwrap();
        assert!(!focus.claimed.load(Ordering::Acquire));
    }

    #[test]
    fn pause_releases_focus_and_resume_requires_a_fresh_grant() {
        let temp = Temp::new();
        temp.wav("a.wav");
        let mut slot = MusicSlot::new(NullSink::default(), temp.scan());
        let focus = TestFocus::default();
        slot.handle(Command::Play, &focus).unwrap();
        assert!(slot.status().has_audio_focus);

        let paused = slot.handle(Command::Pause, &focus).unwrap();
        assert_eq!(paused.playback.state, PlaybackState::Paused);
        assert!(!paused.has_audio_focus);
        assert!(!focus.claimed.load(Ordering::Acquire));

        focus.claimed.store(true, Ordering::Release); // RADIO took the lease.
        assert!(slot.handle(Command::ResumePlayback, &focus).is_err());
        assert_eq!(slot.status().playback.state, PlaybackState::Paused);
        assert!(!slot.status().has_audio_focus);
        assert!(focus.claimed.load(Ordering::Acquire));

        focus.claimed.store(false, Ordering::Release);
        // The explicit Play action is also a resume when the decoder is
        // paused; it must not reload from frame zero.
        let resumed = slot.handle(Command::Play, &focus).unwrap();
        assert_eq!(resumed.playback.state, PlaybackState::Playing);
        assert!(resumed.has_audio_focus);
        slot.handle(Command::Stop, &focus).unwrap();
        assert!(!focus.claimed.load(Ordering::Acquire));
    }

    #[test]
    fn bounded_volume_control_preserves_default_and_rejects_out_of_range() {
        let temp = Temp::new();
        temp.wav("a.wav");
        let mut slot = MusicSlot::new(NullSink::default(), temp.scan());
        let focus = TestFocus::default();
        assert!(slot.handle(Command::SetVolumePercent(100), &focus).is_ok());
        assert!(slot.handle(Command::SetVolumePercent(0), &focus).is_ok());
        assert!(slot.handle(Command::SetVolumePercent(101), &focus).is_err());
        assert!(!slot.status().has_audio_focus);
    }

    #[test]
    fn unsupported_seek_keeps_current_audio_lease_and_playback() {
        let temp = Temp::new();
        temp.wav("a.wav");
        let focus = TestFocus::default();
        let mut slot = MusicSlot::new(NullSink::default(), temp.scan());
        slot.handle(Command::Play, &focus).unwrap();
        assert!(focus.claimed.load(Ordering::Acquire));

        assert!(matches!(
            slot.handle(Command::SeekFrame(0), &focus),
            Err(MusicError::Unsupported(_))
        ));
        assert_eq!(slot.status().playback.state, PlaybackState::Playing);
        assert!(slot.status().has_audio_focus);
        assert!(focus.claimed.load(Ordering::Acquire));
        slot.handle(Command::Stop, &focus).unwrap();
        assert!(!focus.claimed.load(Ordering::Acquire));
    }

    #[test]
    fn successful_flac_seek_releases_focus_without_autoplay() {
        let temp = Temp::new();
        temp.flac("a.flac");
        let focus = TestFocus::default();
        let mut slot = MusicSlot::new(NullSink::default(), temp.scan());
        slot.handle(Command::Play, &focus).unwrap();
        assert!(focus.claimed.load(Ordering::Acquire));

        let seeked = slot.handle(Command::SeekFrame(8), &focus).unwrap();
        assert_eq!(seeked.playback.state, PlaybackState::Ready);
        assert_eq!(seeked.playback.frames_delivered, 8);
        assert!(!seeked.has_audio_focus);
        assert!(!focus.claimed.load(Ordering::Acquire));
        assert_eq!(slot.sink().frames_written, 0);

        slot.handle(Command::Play, &focus).unwrap();
        slot.handle(Command::Poll, &focus).unwrap();
        assert_eq!(slot.sink().frames_written, 8);
        slot.handle(Command::Poll, &focus).unwrap();
        assert_eq!(slot.status().playback.state, PlaybackState::Finished);
        assert!(!focus.claimed.load(Ordering::Acquire));
    }

    #[test]
    fn device_open_failure_releases_the_host_focus_lease() {
        let temp = Temp::new();
        temp.wav("a.wav");
        let focus = TestFocus::default();
        let mut slot = MusicSlot::new(FailOpenSink::default(), temp.scan());

        assert!(matches!(
            slot.handle(Command::Play, &focus),
            Err(MusicError::Output(_))
        ));
        assert_eq!(slot.sink().open_attempts, 1);
        assert_eq!(slot.sink().finish_attempts, 1);
        assert_eq!(slot.status().playback.state, PlaybackState::Failed);
        assert!(!slot.status().has_audio_focus);
        assert!(!focus.claimed.load(Ordering::Acquire));
    }

    #[test]
    fn device_write_failure_releases_the_host_focus_lease() {
        let temp = Temp::new();
        temp.wav("a.wav");
        let focus = TestFocus::default();
        let mut slot = MusicSlot::new(FailWriteSink::default(), temp.scan());

        slot.handle(Command::Play, &focus).unwrap();
        assert!(focus.claimed.load(Ordering::Acquire));
        assert!(matches!(
            slot.handle(Command::Poll, &focus),
            Err(MusicError::Output(_))
        ));
        assert_eq!(slot.status().playback.state, PlaybackState::Failed);
        assert!(!slot.status().has_audio_focus);
        assert!(!focus.claimed.load(Ordering::Acquire));
        assert_eq!(slot.sink().finish_attempts, 1);
        assert_eq!(slot.sink().opened, None);
    }

    #[test]
    fn device_write_failure_recovers_after_device_return_and_explicit_play() {
        let temp = Temp::new();
        temp.wav("a.wav");
        let focus = TestFocus::default();
        let available = Arc::new(AtomicBool::new(true));
        let sink = RecoveringSink {
            available: available.clone(),
            open_attempts: 0,
            successful_writes: 0,
            finish_attempts: 0,
            opened: false,
        };
        let mut slot = MusicSlot::new(sink, temp.scan());

        slot.handle(Command::Play, &focus).unwrap();
        assert!(focus.claimed.load(Ordering::Acquire));
        available.store(false, Ordering::Release);
        assert!(matches!(
            slot.handle(Command::Poll, &focus),
            Err(MusicError::Output(_))
        ));
        assert_eq!(slot.status().playback.state, PlaybackState::Failed);
        assert!(!slot.status().has_audio_focus);
        assert!(!focus.claimed.load(Ordering::Acquire));
        assert_eq!(slot.sink().open_attempts, 1);
        assert_eq!(slot.sink().finish_attempts, 1);

        available.store(true, Ordering::Release);
        slot.handle(Command::Play, &focus).unwrap();
        assert_eq!(slot.sink().open_attempts, 2);
        assert!(focus.claimed.load(Ordering::Acquire));
        slot.handle(Command::Poll, &focus).unwrap();
        assert_eq!(slot.sink().successful_writes, 1);
        assert_eq!(slot.status().playback.state, PlaybackState::Playing);
        assert!(slot.status().has_audio_focus);
        assert!(focus.claimed.load(Ordering::Acquire));

        slot.handle(Command::Poll, &focus).unwrap();
        assert_eq!(slot.status().playback.state, PlaybackState::Finished);
        assert!(!slot.status().has_audio_focus);
        assert!(!focus.claimed.load(Ordering::Acquire));
        assert_eq!(slot.sink().finish_attempts, 2);
    }

    #[test]
    fn explicit_stop_releases_focus_even_when_device_drain_fails() {
        let temp = Temp::new();
        temp.wav("a.wav");
        let focus = TestFocus::default();
        let mut slot = MusicSlot::new(FailFinishSink::default(), temp.scan());

        slot.handle(Command::Play, &focus).unwrap();
        assert!(focus.claimed.load(Ordering::Acquire));
        assert!(matches!(
            slot.handle(Command::Stop, &focus),
            Err(MusicError::Output(_))
        ));
        assert_eq!(slot.sink().finish_attempts, 1);
        assert_eq!(slot.status().playback.state, PlaybackState::Stopped);
        assert!(!slot.status().has_audio_focus);
        assert!(!focus.claimed.load(Ordering::Acquire));
    }

    #[test]
    fn eof_releases_focus_when_device_drain_fails() {
        let temp = Temp::new();
        temp.wav("a.wav");
        let focus = TestFocus::default();
        let mut slot = MusicSlot::new(FailFinishSink::default(), temp.scan());

        slot.handle(Command::Play, &focus).unwrap();
        slot.handle(Command::Poll, &focus).unwrap();
        assert!(focus.claimed.load(Ordering::Acquire));
        assert!(matches!(
            slot.handle(Command::Poll, &focus),
            Err(MusicError::Output(_))
        ));
        assert_eq!(slot.sink().finish_attempts, 1);
        assert_eq!(slot.status().playback.state, PlaybackState::Failed);
        assert!(!slot.status().has_audio_focus);
        assert!(!focus.claimed.load(Ordering::Acquire));
    }

    #[test]
    fn next_previous_never_autoplay_and_drop_releases_lease() {
        let temp = Temp::new();
        temp.wav("a.wav");
        temp.wav("b.wav");
        let focus = TestFocus::default();
        let mut slot = MusicSlot::new(NullSink::default(), temp.scan());
        slot.handle(Command::Previous, &focus).unwrap();
        assert_eq!(slot.status().selection, Some(1));
        slot.handle(Command::Play, &focus).unwrap();
        assert!(focus.claimed.load(Ordering::Acquire));
        slot.handle(Command::Next, &focus).unwrap();
        assert_eq!(slot.status().selection, Some(0));
        assert!(!slot.status().has_audio_focus);
        assert!(!focus.claimed.load(Ordering::Acquire));
        slot.handle(Command::Play, &focus).unwrap();
        drop(slot);
        assert!(!focus.claimed.load(Ordering::Acquire));
    }

    #[test]
    fn queue_selection_and_navigation_never_open_sink_or_acquire_focus() {
        let temp = Temp::new();
        temp.wav("a.wav");
        temp.wav("b.wav");
        let focus = TestFocus::default();
        let mut slot = MusicSlot::new(NullSink::default(), temp.scan());

        let initial = slot.status();
        assert_eq!(initial.queue_entries, 0);
        assert_eq!(initial.queue_selection, None);
        slot.handle(Command::EnqueueSelection, &focus).unwrap();
        slot.handle(Command::Next, &focus).unwrap();
        slot.handle(Command::EnqueueSelection, &focus).unwrap();
        assert_eq!(
            slot.queue().entries(),
            &[PathBuf::from("a.wav"), PathBuf::from("b.wav")]
        );
        let restored =
            PlaybackQueue::from_record(slot.queue().to_record().unwrap().as_bytes()).unwrap();
        let restored_status = slot.replace_queue(restored);
        assert_eq!(restored_status.queue_entries, 2);
        assert_eq!(restored_status.playback.state, PlaybackState::Stopped);
        assert_eq!(focus.attempts.load(Ordering::Relaxed), 0);
        assert!(slot.sink().opened.is_none());

        let selected = slot.handle(Command::SelectQueue(0), &focus).unwrap();
        assert_eq!(selected.selection, Some(0));
        assert_eq!(selected.queue_selection, Some(0));
        let next = slot.handle(Command::QueueNext, &focus).unwrap();
        assert_eq!(next.selection, Some(1));
        assert_eq!(next.queue_selection, Some(1));
        let previous = slot.handle(Command::QueuePrevious, &focus).unwrap();
        assert_eq!(previous.selection, Some(0));
        assert_eq!(previous.queue_selection, Some(0));

        let repeated = slot
            .handle(Command::SetRepeat(RepeatMode::All), &focus)
            .unwrap();
        assert_eq!(repeated.repeat_mode, RepeatMode::All);
        let shuffled = slot
            .handle(Command::ShuffleQueue(Some(42)), &focus)
            .unwrap();
        assert_eq!(shuffled.shuffle_seed, Some(42));
        assert_eq!(focus.attempts.load(Ordering::Relaxed), 0);
        assert!(!focus.claimed.load(Ordering::Acquire));
        assert!(slot.sink().opened.is_none());
        assert_eq!(slot.sink().frames_written, 0);

        slot.handle(Command::Play, &focus).unwrap();
        assert_eq!(focus.attempts.load(Ordering::Relaxed), 1);
        assert!(focus.claimed.load(Ordering::Acquire));
        // Selecting a new queue item stops and releases the active session;
        // it does not request another lease or restart output.
        let moved = slot.handle(Command::QueuePrevious, &focus).unwrap();
        assert_eq!(moved.selection, Some(1));
        assert_eq!(focus.attempts.load(Ordering::Relaxed), 1);
        assert!(!focus.claimed.load(Ordering::Acquire));
        assert!(slot.sink().opened.is_none());
        assert_eq!(slot.sink().frames_written, 0);
    }

    #[test]
    fn stale_queue_candidates_fail_closed_without_focus_or_output() {
        let temp = Temp::new();
        temp.wav("a.wav");
        let focus = TestFocus::default();
        let mut slot = MusicSlot::new(NullSink::default(), temp.scan());
        slot.handle(Command::EnqueueSelection, &focus).unwrap();
        fs::remove_file(temp.0.join("a.wav")).unwrap();

        assert!(matches!(
            slot.handle(Command::SelectQueue(0), &focus),
            Err(MusicError::Io(_))
        ));
        assert_eq!(focus.attempts.load(Ordering::Relaxed), 0);
        assert!(!focus.claimed.load(Ordering::Acquire));
        assert!(slot.sink().opened.is_none());
        assert_eq!(slot.sink().frames_written, 0);
    }

    #[test]
    fn stale_queue_navigation_stops_active_playback_and_releases_focus() {
        let temp = Temp::new();
        temp.wav("a.wav");
        temp.wav("b.wav");
        let b_bytes = fs::read(temp.0.join("b.wav")).unwrap();
        let focus = TestFocus::default();
        let mut slot = MusicSlot::new(NullSink::default(), temp.scan());
        slot.handle(Command::EnqueueSelection, &focus).unwrap(); // a.wav
        slot.handle(Command::Next, &focus).unwrap();
        slot.handle(Command::EnqueueSelection, &focus).unwrap(); // b.wav
        slot.handle(Command::SelectQueue(0), &focus).unwrap();
        slot.handle(Command::Play, &focus).unwrap();
        assert_eq!(slot.status().playback.state, PlaybackState::Playing);
        assert!(focus.claimed.load(Ordering::Acquire));
        assert_eq!(focus.attempts.load(Ordering::Relaxed), 1);

        fs::remove_file(temp.0.join("b.wav")).unwrap();
        assert!(matches!(
            slot.handle(Command::QueueNext, &focus),
            Err(MusicError::Io(_))
        ));
        assert_eq!(slot.status().playback.state, PlaybackState::Stopped);
        assert!(!slot.status().has_audio_focus);
        assert!(!focus.claimed.load(Ordering::Acquire));
        assert_eq!(focus.attempts.load(Ordering::Relaxed), 1);
        assert!(slot.sink().opened.is_none());
        assert_eq!(slot.sink().frames_written, 0);

        // A stale explicit queue selection must also stop the active song.
        fs::write(temp.0.join("b.wav"), b_bytes).unwrap();
        slot.handle(Command::Play, &focus).unwrap();
        assert_eq!(slot.status().playback.state, PlaybackState::Playing);
        fs::remove_file(temp.0.join("a.wav")).unwrap();
        assert!(matches!(
            slot.handle(Command::SelectQueue(0), &focus),
            Err(MusicError::Io(_))
        ));
        assert_eq!(slot.status().playback.state, PlaybackState::Stopped);
        assert!(!focus.claimed.load(Ordering::Acquire));
        assert_eq!(focus.attempts.load(Ordering::Relaxed), 2);
        assert!(slot.sink().opened.is_none());
        assert_eq!(slot.sink().frames_written, 0);
    }

    #[test]
    fn package_identity_is_distinct_and_suspend_preserves_mounted_module() {
        assert_eq!(MUSIC_PACKAGE_ID, "org.rustboy.music");
        assert_ne!(MUSIC_PACKAGE_ID, "org.rustboy.radio");

        let temp = Temp::new();
        temp.wav("a.wav");
        temp.wav("b.wav");
        let focus = TestFocus::default();
        let mut slot = MusicSlot::new(NullSink::default(), temp.scan());
        slot.handle(Command::Next, &focus).unwrap();
        assert_eq!(slot.status().selection, Some(1));
        slot.handle(Command::Play, &focus).unwrap();
        assert!(focus.claimed.load(Ordering::Acquire));

        let suspended = slot.suspend().unwrap();
        assert_eq!(suspended.tracks, 2);
        assert_eq!(suspended.selection, Some(1));
        assert_eq!(suspended.playback.state, PlaybackState::Stopped);
        assert!(!suspended.has_audio_focus);
        assert!(!focus.claimed.load(Ordering::Acquire));

        // Pop-in resumes the same mounted/catalogued package but never autoplay.
        assert_eq!(slot.status().tracks, 2);
        assert_eq!(slot.status().selection, Some(1));
        assert_eq!(slot.status().playback.state, PlaybackState::Stopped);
        assert_eq!(slot.sink().opened, None);
    }

    #[test]
    fn replacing_catalog_stops_audio_preserves_matching_selection_and_never_autoplays() {
        let initial = Temp::new();
        initial.wav("a.wav");
        initial.wav("b.wav");
        let reordered = Temp::new();
        reordered.wav("b.wav");

        let focus = TestFocus::default();
        let mut slot = MusicSlot::new(NullSink::default(), initial.scan());
        slot.handle(Command::Next, &focus).unwrap(); // Select b.wav.
        slot.handle(Command::EnqueueSelection, &focus).unwrap();
        slot.handle(Command::Play, &focus).unwrap();
        assert!(focus.claimed.load(Ordering::Acquire));

        let replaced = slot.replace_library(reordered.scan()).unwrap();
        assert_eq!(replaced.tracks, 1);
        assert_eq!(replaced.selection, Some(0)); // Same relative path.
        assert_eq!(replaced.queue_entries, 0); // Queue was scoped to the old root.
        assert_eq!(replaced.playback.state, PlaybackState::Stopped);
        assert!(!replaced.has_audio_focus);
        assert!(!focus.claimed.load(Ordering::Acquire));
        assert_eq!(slot.sink().opened, None);
        assert_eq!(slot.sink().frames_written, 0);
    }
}
