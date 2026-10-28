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
    pub format: Option<AudioFormat>,
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
            state: PlaybackState::Empty, generation: 0, frames_delivered: 0, format: None,
        } }
    }
    pub fn status(&self) -> PlaybackSnapshot { self.snapshot }
    pub fn sink(&self) -> &S { &self.sink }

    pub fn load(&mut self, decoder: Box<dyn Decoder>) -> Result<()> {
        self.stop()?;
        self.decoder = Some(decoder);
        self.snapshot.generation = self.snapshot.generation.wrapping_add(1);
        self.snapshot.format = self.decoder.as_ref().map(|x| x.format());
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
                if let Err(err) = self.sink.write(&block) {
                    self.snapshot.state = PlaybackState::Failed;
                    let _ = self.sink.finish();
                    return Err(err);
                }
                self.snapshot.frames_delivered = self.snapshot.frames_delivered
                    .checked_add(block.frames() as u64).ok_or(MusicError::Limit("output frame counter overflow"))?;
            }
            Ok(None) => {
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
        if self.snapshot.state == PlaybackState::Playing {
            self.sink.finish()?;
        }
        self.decoder = None;
        self.snapshot.state = PlaybackState::Stopped;
        self.snapshot.format = None;
        Ok(())
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
