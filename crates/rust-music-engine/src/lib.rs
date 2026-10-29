//! App-owned playback state machine. No automatic playback, I/O on construction,
//! background thread, GUI, networking, or hidden second audio stream.

use rust_music_core::{AudioFormat, Decoder, MusicError, PcmSink, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState { Empty, Ready, Playing, Stopped, Finished, Failed }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaybackSnapshot {
    pub state: PlaybackState,
    pub generation: u64,
    pub frames_delivered: u64,
    /// Declared sample frames; an untrusted header until EOF checks complete.
    pub total_frames: Option<u64>,
    pub format: Option<AudioFormat>,
}

impl PlaybackSnapshot {
    pub fn elapsed_seconds(self) -> Option<u64> {
        let rate = u64::from(self.format?.sample_rate);
        Some(self.frames_delivered / rate)
    }
    pub fn duration_seconds(self) -> Option<u64> {
        let rate = u64::from(self.format?.sample_rate);
        Some(self.total_frames? / rate)
    }
    /// Integer ratio avoids overflow at u64::MAX and never guesses duration.
    pub fn progress_per_mille(self) -> Option<u16> {
        let total = self.total_frames?;
        if total == 0 { return None; }
        let frames = self.frames_delivered.min(total);
        let whole = frames / total;
        let remainder = frames % total;
        Some((whole * 1000 + ((u128::from(remainder) * 1000) / u128::from(total)) as u64) as u16)
    }
}

/// Exactly one decoder and sink; the host must arbitrate device focus with RADIO/TV.
pub struct Player<S: PcmSink> {
    decoder: Option<Box<dyn Decoder>>,
    sink: S,
    snapshot: PlaybackSnapshot,
}

impl<S: PcmSink> Player<S> {
    pub fn new(sink: S) -> Self {
        Self { decoder: None, sink, snapshot: PlaybackSnapshot {
            state: PlaybackState::Empty, generation: 0, frames_delivered: 0, total_frames: None, format: None,
        } }
    }
    pub fn status(&self) -> PlaybackSnapshot { self.snapshot }
    pub fn sink(&self) -> &S { &self.sink }

    pub fn load(&mut self, decoder: Box<dyn Decoder>) -> Result<()> {
        self.stop()?;
        self.decoder = Some(decoder);
        self.snapshot.generation = self.snapshot.generation.wrapping_add(1);
        self.snapshot.format = self.decoder.as_ref().map(|x| x.format());
        self.snapshot.total_frames = self.decoder.as_ref().and_then(|x| x.total_frames());
        self.snapshot.frames_delivered = 0;
        self.snapshot.state = PlaybackState::Ready;
        Ok(())
    }
    /// Explicit action after the caller acquired output focus.
    pub fn play(&mut self) -> Result<()> {
        if self.snapshot.state != PlaybackState::Ready {
            return Err(MusicError::Invalid("play requires a loaded track in Ready state"));
        }
        let format = self.snapshot.format.ok_or(MusicError::Invalid("track format missing"))?;
        if let Err(err) = self.sink.open(format) {
            self.snapshot.state = PlaybackState::Failed;
            // A native open may have acquired some resources before failing.
            let _ = self.sink.finish();
            return Err(err);
        }
        self.snapshot.state = PlaybackState::Playing;
        Ok(())
    }
    /// One bounded decoding/output step. Call from a worker, not the UI or audio callback.
    pub fn step(&mut self) -> Result<PlaybackSnapshot> {
        if self.snapshot.state != PlaybackState::Playing {
            return Err(MusicError::Invalid("step requires Playing state"));
        }
        let decoder = self.decoder.as_mut().ok_or(MusicError::Invalid("decoder missing"))?;
        let next = decoder.next_block();
        match next {
            Ok(Some(block)) => {
                if Some(block.format) != self.snapshot.format {
                    self.snapshot.state = PlaybackState::Failed;
                    let _ = self.sink.finish();
                    return Err(MusicError::Invalid("decoder changed PCM format during playback"));
                }
                // Check the entire counter BEFORE a sink can output any bytes.
                let frames = self.snapshot.frames_delivered
                    .checked_add(block.frames() as u64)
                    .ok_or(MusicError::Limit("output frame counter overflow"));
                let frames = match frames {
                    Ok(frames) => frames,
                    Err(err) => {
                        self.snapshot.state = PlaybackState::Failed;
                        let _ = self.sink.finish();
                        return Err(err);
                    }
                };
                if self.snapshot.total_frames.is_some_and(|total| frames > total) {
                    self.snapshot.state = PlaybackState::Failed;
                    let _ = self.sink.finish();
                    return Err(MusicError::Invalid("decoded audio exceeds declared total frames"));
                }
                if let Err(err) = self.sink.write(&block) {
                    self.snapshot.state = PlaybackState::Failed;
                    let _ = self.sink.finish();
                    return Err(err);
                }
                self.snapshot.frames_delivered = frames;
            }
            Ok(None) => {
                if self.snapshot.total_frames.is_some_and(|total| self.snapshot.frames_delivered != total) {
                    self.snapshot.state = PlaybackState::Failed;
                    let _ = self.sink.finish();
                    return Err(MusicError::Invalid("decoded audio is shorter than declared total frames"));
                }
                if let Err(err) = self.sink.finish() {
                    self.snapshot.state = PlaybackState::Failed;
                    return Err(err);
                }
                self.snapshot.state = PlaybackState::Finished;
            }
            Err(err) => {
                self.snapshot.state = PlaybackState::Failed;
                let _ = self.sink.finish();
                return Err(err);
            }
        }
        Ok(self.snapshot)
    }
    /// Stop releases the output before any future module can acquire focus.
    pub fn stop(&mut self) -> Result<()> {
        // Clear ownership even when the physical device refuses a clean drain.
        // The host's focus lease must not be stranded by a native I/O error.
        let result = if self.snapshot.state == PlaybackState::Playing {
            self.sink.finish()
        } else { Ok(()) };
        self.decoder = None;
        self.snapshot.state = PlaybackState::Stopped;
        self.snapshot.format = None;
        self.snapshot.total_frames = None;
        result
    }
}

impl<S: PcmSink> Drop for Player<S> {
    fn drop(&mut self) { let _ = self.stop(); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_music_core::{NullSink, PcmBlock};
    struct OneBlock { format: AudioFormat, step: usize }
    impl Decoder for OneBlock {
        fn format(&self) -> AudioFormat { self.format }
        fn next_block(&mut self) -> Result<Option<PcmBlock>> {
            self.step += 1;
            if self.step == 1 { PcmBlock::new(self.format, vec![10, -10]).map(Some) }
            else { Ok(None) }
        }
    }
    struct Mismatch { expected: AudioFormat, actual: AudioFormat }
    impl Decoder for Mismatch {
        fn format(&self) -> AudioFormat { self.expected }
        fn next_block(&mut self) -> Result<Option<PcmBlock>> {
            Ok(Some(PcmBlock::new(self.actual, vec![1, 2])?))
        }
    }
    #[test]
    fn progress_is_bounded_and_unknown_is_not_estimated() {
        let format = AudioFormat::new(44_100, 2, 16).unwrap();
        let mut snap = PlaybackSnapshot {
            state: PlaybackState::Playing, generation: 1, frames_delivered: 22_050,
            total_frames: Some(44_100), format: Some(format),
        };
        assert_eq!(snap.progress_per_mille(), Some(500));
        assert_eq!(snap.elapsed_seconds(), Some(0));
        assert_eq!(snap.duration_seconds(), Some(1));
        snap.frames_delivered = u64::MAX;
        assert_eq!(snap.progress_per_mille(), Some(1000));
        snap.total_frames = None;
        assert_eq!(snap.progress_per_mille(), None);
        assert_eq!(snap.duration_seconds(), None);
        snap.total_frames = Some(0);
        assert_eq!(snap.progress_per_mille(), None);
    }
    #[test]
    fn changing_format_fails_before_output() {
        let expected = AudioFormat::new(44_100, 2, 16).unwrap();
        let actual = AudioFormat::new(48_000, 2, 16).unwrap();
        let mut player = Player::new(NullSink::default());
        player.load(Box::new(Mismatch { expected, actual })).unwrap();
        player.play().unwrap();
        assert!(player.step().is_err());
        assert_eq!(player.status().state, PlaybackState::Failed);
        assert_eq!(player.sink().frames_written, 0);
        assert!(player.sink().opened.is_none());
    }
    struct FailClose { opened: bool }
    impl PcmSink for FailClose {
        fn open(&mut self, _: AudioFormat) -> Result<()> { self.opened = true; Ok(()) }
        fn write(&mut self, _: &PcmBlock) -> Result<()> { Ok(()) }
        fn finish(&mut self) -> Result<()> {
            self.opened = false;
            Err(MusicError::Output("simulated drain failure".into()))
        }
    }
    #[test]
    fn stop_releases_state_even_if_device_drain_fails() {
        let format = AudioFormat::new(44_100, 2, 16).unwrap();
        let mut player = Player::new(FailClose { opened: false });
        player.load(Box::new(OneBlock { format, step: 0 })).unwrap();
        player.play().unwrap();
        assert!(player.stop().is_err());
        assert_eq!(player.status().state, PlaybackState::Stopped);
        assert!(!player.sink().opened);
    }
    #[test]
    fn no_autoplay_and_one_track_at_a_time() {
        let format = AudioFormat::new(44_100, 2, 16).unwrap();
        let mut player = Player::new(NullSink::default());
        player.load(Box::new(OneBlock { format, step: 0 })).unwrap();
        assert_eq!(player.status().state, PlaybackState::Ready);
        assert_eq!(player.sink().frames_written, 0);
        player.play().unwrap();
        assert_eq!(player.step().unwrap().frames_delivered, 1);
        assert_eq!(player.step().unwrap().state, PlaybackState::Finished);
        assert!(player.play().is_err());
    }
}
