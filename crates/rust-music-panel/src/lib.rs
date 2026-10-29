//! Minimal 128x64, one-bit music panel. Locally authored font and rasterizer.
//!
//! This is a pure renderer: it reads a borrowed snapshot and draws 1024 bytes.
//! It opens no files/devices, spawns no threads, and never dispatches commands.
//! The Rust_Boy host supplies the color (e.g. phosphor green), screen/window,
//! input events, audio worker, and settings screen. No GUI toolkit is linked.

use rust_music::{PcmSink, PlaybackState};
use rust_music_host::{Command, MusicSlot, SlotSnapshot};
use rust_music_library::AudioContainer;

pub const WIDTH: usize = 128;
pub const HEIGHT: usize = 64;
pub const BYTES: usize = WIDTH * HEIGHT / 8;

/// Row-major bitmap, most significant bit first within each eight-pixel row.
/// Compatible with PBM P4 and simple monochrome displays (without a renderer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonoFrame {
    pixels: [u8; BYTES],
}

impl Default for MonoFrame {
    fn default() -> Self {
        Self::new()
    }
}

impl MonoFrame {
    pub const fn new() -> Self {
        Self { pixels: [0; BYTES] }
    }
    pub const fn pixels(&self) -> &[u8; BYTES] {
        &self.pixels
    }

    pub fn pixel(&self, x: usize, y: usize) -> bool {
        if x >= WIDTH || y >= HEIGHT {
            return false;
        }
        self.pixels[y * (WIDTH / 8) + x / 8] & (0x80 >> (x % 8)) != 0
    }
    fn put(&mut self, x: usize, y: usize) {
        if x < WIDTH && y < HEIGHT {
            self.pixels[y * (WIDTH / 8) + x / 8] |= 0x80 >> (x % 8);
        }
    }
    fn line(&mut self, y: usize) {
        if y < HEIGHT {
            self.pixels[y * (WIDTH / 8)..(y + 1) * (WIDTH / 8)].fill(0xff);
        }
    }
    fn glyph(&mut self, x: usize, y: usize, ch: char) {
        let rows = glyph_rows(ch);
        for (row, bits) in rows.iter().enumerate() {
            for col in 0..5 {
                if *bits & (0b1_0000 >> col) != 0 {
                    self.put(x + col, y + row);
                }
            }
        }
    }
    /// Explicit maximum column count, with no intermediate String allocation.
    fn text(&mut self, x: usize, y: usize, value: &str, max_cols: usize) -> usize {
        let mut next_x = x;
        for ch in value.chars().take(max_cols) {
            if next_x.saturating_add(5) > WIDTH {
                break;
            }
            let safe = if ch.is_ascii_graphic() || ch == ' ' {
                ch.to_ascii_uppercase()
            } else {
                '?'
            };
            self.glyph(next_x, y, safe);
            next_x += 6;
        }
        next_x
    }
    fn number(&mut self, x: usize, y: usize, number: u64) -> usize {
        let mut digits = [0u8; 20];
        let mut count = 0;
        let mut n = number;
        loop {
            digits[count] = (n % 10) as u8 + b'0';
            count += 1;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        let mut x = x;
        for digit in digits[..count].iter().rev() {
            if x.saturating_add(5) > WIDTH {
                break;
            }
            self.glyph(x, y, char::from(*digit));
            x += 6;
        }
        x
    }
}

/// Borrowed data from the existing Rust_Boy MUSIC slot. Reading this does not
/// cause decoding, media scans, playback, or audio focus acquisition.
#[derive(Debug, Clone, Copy)]
pub struct PanelView<'a> {
    pub status: SlotSnapshot,
    pub title: Option<&'a str>,
    pub container: Option<AudioContainer>,
}

impl<'a> PanelView<'a> {
    pub fn from_slot<S: PcmSink>(slot: &'a MusicSlot<S>) -> Self {
        let status = slot.status();
        let track = status
            .selection
            .and_then(|index| slot.library().track(index));
        Self {
            status,
            title: track.map(|item| item.title.as_str()),
            container: track.map(|item| item.container),
        }
    }
}

fn state_text(state: PlaybackState) -> &'static str {
    match state {
        PlaybackState::Empty => "EMPTY",
        PlaybackState::Ready => "READY",
        PlaybackState::Playing => "PLAYING",
        PlaybackState::Stopped => "STOPPED",
        PlaybackState::Finished => "FINISHED",
        PlaybackState::Failed => "ERROR",
    }
}

/// Synchronous pure snapshot-to-bitmap transformation, fixed one-kilobyte
/// raster. The host may tint white pixels phosphor green in its own renderer.
/// No estimated duration/progress is displayed: frames are factual only.
pub fn render(view: PanelView<'_>) -> MonoFrame {
    let mut frame = MonoFrame::new();
    frame.text(2, 2, "MUSIC", 5);
    frame.text(49, 2, "T", 1);
    let after = frame.number(
        57,
        2,
        view.status.selection.map_or(0, |i| i.saturating_add(1)) as u64,
    );
    let after = frame.text(after, 2, "/", 1);
    frame.number(after, 2, view.status.tracks as u64);
    frame.text(107, 2, "SET", 3);
    frame.line(11);

    frame.text(2, 16, view.title.unwrap_or("NO MUSIC"), 20);
    frame.text(2, 28, state_text(view.status.playback.state), 9);
    if view.status.has_audio_focus {
        frame.text(58, 28, "AUDIO", 5);
    }
    match view.container {
        Some(AudioContainer::Flac) => {
            frame.text(99, 28, "FLAC", 4);
        }
        Some(AudioContainer::Wave) => {
            frame.text(99, 28, "WAV", 3);
        }
        Some(AudioContainer::Aiff) => {
            frame.text(99, 28, "AIFF", 4);
        }
        Some(AudioContainer::Caf) => {
            frame.text(99, 28, "CAF", 3);
        }
        Some(AudioContainer::Mp4) => {
            frame.text(99, 28, "M4A", 3);
        }
        None => {}
    }
    // Show exact sample-derived elapsed/duration clocks only when declared.
    // No duration guesses when FLAC STREAMINFO has an unknown sample count.
    if let Some(elapsed) = view.status.playback.elapsed_seconds() {
        frame.text(2, 40, "TIME", 4);
        clock(&mut frame, 32, 40, elapsed);
        if let Some(total) = view.status.playback.duration_seconds() {
            frame.text(74, 40, "/", 1);
            clock(&mut frame, 83, 40, total);
        }
    }
    if let Some(progress) = view.status.playback.progress_per_mille() {
        for x in 2..126 {
            if x <= 2 + (u32::from(progress) * 123 / 1000) as usize {
                frame.put(x, 50);
            }
        }
    }
    if let Some(format) = view.status.playback.format {
        // Duration occupies this row during playback; otherwise show PCM format.
        if view.status.playback.elapsed_seconds().is_none() {
            let x = frame.number(2, 40, u64::from(format.sample_rate));
            let x = frame.text(x, 40, "HZ", 2).saturating_add(6);
            let x = frame.number(x, 40, u64::from(format.bits_per_sample));
            let x = frame.text(x, 40, "B", 1).saturating_add(6);
            let x = frame.number(x, 40, u64::from(format.channels));
            frame.text(x, 40, "CH", 2);
        }
    } else {
        frame.text(2, 40, "OFFLINE / LOSSLESS", 18);
    }
    frame.text(2, 55, "<   PLAY  STOP   >", 18);
    frame
}

// Compact mm:ss clock. Saturate to 99:59 to preserve fixed display geometry.
fn clock(frame: &mut MonoFrame, x: usize, y: usize, seconds: u64) {
    let capped = seconds.min(99 * 60 + 59);
    let mins = capped / 60;
    let secs = capped % 60;
    let next = frame.number(x, y, mins);
    let next = frame.text(next, y, ":", 1);
    if secs < 10 {
        frame.text(next, y, "0", 1);
    }
    frame.number(next + if secs < 10 { 6 } else { 0 }, y, secs);
}

/// Buttons are translated into host actions, not executed by the renderer.
/// Settings is handled by Rust_Boy; playback selection remains explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Previous,
    Next,
    Confirm,
    Stop,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelAction {
    Music(Command),
    OpenSettings,
}

pub fn translate_button(button: Button, playback: PlaybackState) -> PanelAction {
    match button {
        Button::Previous => PanelAction::Music(Command::Previous),
        Button::Next => PanelAction::Music(Command::Next),
        Button::Stop => PanelAction::Music(Command::Stop),
        Button::Settings => PanelAction::OpenSettings,
        Button::Confirm => {
            let command = if playback == PlaybackState::Playing {
                Command::Stop
            } else {
                Command::Play
            };
            PanelAction::Music(command)
        }
    }
}

/// Self-authored five-column glyphs represented as seven five-bit rows. The
/// fallback is an explicit question mark, never arbitrary font-file access.
fn glyph_rows(ch: char) -> [u8; 7] {
    match ch {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [31, 4, 4, 4, 4, 4, 31],
        'J' => [7, 2, 2, 2, 18, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        ' ' => [0; 7],
        '?' => [14, 17, 1, 2, 4, 0, 4],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        '_' => [0, 0, 0, 0, 0, 0, 31],
        '.' => [0, 0, 0, 0, 0, 12, 12],
        '/' => [1, 2, 2, 4, 8, 8, 16],
        ':' => [0, 12, 12, 0, 12, 12, 0],
        '<' => [2, 4, 8, 16, 8, 4, 2],
        '>' => [8, 4, 2, 1, 2, 4, 8],
        _ => [14, 17, 1, 2, 4, 0, 4],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_music::{AudioFormat, NullSink, PlaybackSnapshot};
    use rust_music_host::MusicSlot;
    use rust_music_library::{scan, ScanLimits};
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn framebuffer_one_kib_and_bounds() {
        assert_eq!(core::mem::size_of::<MonoFrame>(), 1024);
        let mut frame = MonoFrame::new();
        frame.put(0, 0);
        frame.put(127, 63);
        frame.put(128, 63);
        frame.put(0, 64);
        assert_eq!(frame.pixels()[0], 0x80);
        assert_eq!(frame.pixels()[1023], 1);
        assert!(frame.pixel(0, 0));
        assert!(frame.pixel(127, 63));
        assert!(!frame.pixel(128, 63));
    }

    fn empty_snapshot() -> SlotSnapshot {
        SlotSnapshot {
            selection: None,
            tracks: 0,
            has_audio_focus: false,
            playback: PlaybackSnapshot {
                state: PlaybackState::Empty,
                generation: 0,
                frames_delivered: 0,
                total_frames: None,
                format: None,
            },
        }
    }

    #[test]
    fn rendering_is_deterministic_and_handles_unicode() {
        let view = PanelView {
            status: empty_snapshot(),
            title: Some("\u{1f3b5}\ntrack \u{00e9}"),
            container: None,
        };
        let a = render(view);
        let b = render(view);
        assert_eq!(a, b);
        assert!(a.pixels().iter().any(|byte| *byte != 0));
        assert!(a.pixel(0, 11));
        assert!(a.pixel(127, 11));
        assert!(!a.pixel(127, 63));
    }

    #[test]
    fn format_render_does_not_overflow_framebuffer() {
        let mut status = empty_snapshot();
        status.playback.format = Some(AudioFormat::new(1_048_575, 8, 32).unwrap());
        status.playback.state = PlaybackState::Playing;
        status.has_audio_focus = true;
        status.selection = Some(usize::MAX);
        status.tracks = usize::MAX;
        let view = PanelView {
            status,
            title: Some(&"A".repeat(200)),
            container: Some(AudioContainer::Mp4),
        };
        let frame = render(view);
        assert_ne!(
            frame,
            render(PanelView {
                container: None,
                ..view
            })
        );
        assert_eq!(frame.pixels().len(), BYTES);
    }

    #[test]
    fn control_translation_never_autoplays_on_navigation() {
        assert_eq!(
            translate_button(Button::Previous, PlaybackState::Playing),
            PanelAction::Music(Command::Previous)
        );
        assert_eq!(
            translate_button(Button::Next, PlaybackState::Stopped),
            PanelAction::Music(Command::Next)
        );
        assert_eq!(
            translate_button(Button::Confirm, PlaybackState::Playing),
            PanelAction::Music(Command::Stop)
        );
        assert_eq!(
            translate_button(Button::Confirm, PlaybackState::Finished),
            PanelAction::Music(Command::Play)
        );
        assert_eq!(
            translate_button(Button::Settings, PlaybackState::Playing),
            PanelAction::OpenSettings
        );
    }

    #[test]
    fn renders_read_only_from_real_slot() {
        let path = std::env::temp_dir().join(format!(
            "rust-music-panel-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
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
        fs::write(path.join("sample.wav"), bytes).unwrap();
        let catalog = scan(&path, ScanLimits::default()).unwrap();
        let slot = MusicSlot::new(NullSink::default(), catalog);
        let before = slot.status();
        let frame = render(PanelView::from_slot(&slot));
        assert_eq!(frame.pixels().len(), BYTES);
        assert_eq!(slot.status(), before);
        assert!(frame.pixels().iter().any(|byte| *byte != 0));
        fs::remove_dir_all(&path).unwrap();
    }
}
