//! Rust_Boy-compatible, headless music slot. No GUI framework, background task,
//! playback thread, network calls, device discovery, or autoplay.
//!
//! The host supplies a short-lived exclusive audio-focus lease. Holding the
//! lease inside MusicSlot ensures it is relinquished on stop, EOF, errors, and
//! destruction, without assuming which other Rust_Boy module owns the device.

use rust_music::{open_file, MusicError, PcmSink, PlaybackSnapshot, PlaybackState, Player, Result};
use rust_music_library::Library;

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
    Stop,
    /// Process exactly one decoded block. The caller schedules a worker tick.
    Poll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotSnapshot {
    pub selection: Option<usize>,
    pub tracks: usize,
    pub playback: PlaybackSnapshot,
    pub has_audio_focus: bool,
}

/// One Rust_Boy pop-in music module. There is exactly one player and sink.
pub struct MusicSlot<S: PcmSink> {
    player: Player<S>,
    library: Library,
    selected: Option<usize>,
    focus_lease: Option<Box<dyn AudioLease>>,
}

impl<S: PcmSink> MusicSlot<S> {
    pub fn new(sink: S, library: Library) -> Self {
        let selected = (!library.is_empty()).then_some(0);
        Self { player: Player::new(sink), library, selected, focus_lease: None }
    }
    pub fn library(&self) -> &Library { &self.library }
    pub fn sink(&self) -> &S { self.player.sink() }
    pub fn status(&self) -> SlotSnapshot {
        SlotSnapshot {
            selection: self.selected,
            tracks: self.library.len(),
            playback: self.player.status(),
            has_audio_focus: self.focus_lease.is_some(),
        }
    }

    /// The caller owns scan timing and file permissions. Replacing the catalog
    /// always stops playback, preserves the previous relative path if present,
    /// and never starts a different song automatically.
    pub fn replace_library(&mut self, catalog: Library) -> Result<SlotSnapshot> {
        let old_path = self.selected.and_then(|i| self.library.track(i))
            .map(|t| t.relative_path.clone());
        self.stop()?;
        self.selected = old_path
            .and_then(|p| catalog.tracks().iter().position(|t| t.relative_path == p))
            .or_else(|| (!catalog.is_empty()).then_some(0));
        self.library = catalog;
        Ok(self.status())
    }

    pub fn handle(&mut self, command: Command, audio_focus: &dyn AudioFocus) -> Result<SlotSnapshot> {
        match command {
            Command::Select(index) => self.select(index)?,
            Command::Next => self.move_selection(true)?,
            Command::Previous => self.move_selection(false)?,
            Command::Play => self.play(audio_focus)?,
            Command::Stop => self.stop()?,
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
    fn move_selection(&mut self, forwards: bool) -> Result<()> {
        let count = self.library.len();
        if count == 0 { return Ok(()); }
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
        let selection = self.selected.ok_or(MusicError::Invalid("no track selected"))?;
        if self.player.status().state != PlaybackState::Ready {
            let path = self.library.checked_path(selection)?;
            self.player.load(open_file(path)?)?;
        }
        let lease = audio_focus.try_acquire_music()
            .ok_or_else(|| MusicError::Output("audio focus belongs to another Rust_Boy module".into()))?;
        self.focus_lease = Some(lease);
        if let Err(error) = self.player.play() {
            self.focus_lease = None;
            return Err(error);
        }
        Ok(())
    }
    fn poll(&mut self) -> Result<()> {
        if self.player.status().state != PlaybackState::Playing { return Ok(()); }
        match self.player.step() {
            Ok(snapshot) => {
                if snapshot.state != PlaybackState::Playing { self.focus_lease = None; }
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
    use std::{fs, path::PathBuf, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::{SystemTime, UNIX_EPOCH}};

    #[derive(Clone, Default)]
    struct TestFocus { claimed: Arc<AtomicBool> }
    struct TestLease(Arc<AtomicBool>);
    impl AudioLease for TestLease {}
    impl Drop for TestLease {
        fn drop(&mut self) { self.0.store(false, Ordering::Release); }
    }
    impl AudioFocus for TestFocus {
        fn try_acquire_music(&self) -> Option<Box<dyn AudioLease>> {
            self.claimed.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .ok().map(|_| Box::new(TestLease(self.claimed.clone())) as Box<dyn AudioLease>)
        }
    }
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let path = std::env::temp_dir().join(format!("rust-music-slot-{}-{stamp}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
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
        fn scan(&self) -> Library { scan(&self.0, ScanLimits::default()).unwrap() }
    }
    impl Drop for Temp { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }

    #[test]
    fn below_cd_quality_source_never_claims_radio_focus_or_opens_sink() {
        let temp = Temp::new();
        temp.low_quality_wav("low.wav");
        let focus = TestFocus::default();
        let mut slot = MusicSlot::new(NullSink::default(), temp.scan());
        assert_eq!(slot.status().tracks, 1);
        assert!(matches!(slot.handle(Command::Play, &focus), Err(MusicError::Unsupported(_))));
        assert!(!focus.claimed.load(Ordering::Acquire));
        assert!(!slot.status().has_audio_focus);
        assert_eq!(slot.status().playback.state, PlaybackState::Empty);
        assert!(slot.sink().opened.is_none());
    }

    #[test]
    fn no_autoplay_and_focus_released_at_end_of_file() {
        let temp = Temp::new(); temp.wav("b.wav"); temp.wav("a.wav");
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
        let temp = Temp::new(); temp.wav("a.wav");
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
    fn next_previous_never_autoplay_and_drop_releases_lease() {
        let temp = Temp::new(); temp.wav("a.wav"); temp.wav("b.wav");
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
}
