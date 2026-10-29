# Rust_Boy MUSIC slot contract (G5 foundation)

The `rust-music-host` crate is a **standalone, framework-free interface** for loading Rust Music as one removable Rust_Boy package. It is locally authored, uses only workspace crates, and has no filesystem scanning, decoder thread, UI, soundserver, or network activity during construction. The host owns the window, focus, task scheduling, storage, and platform audio device selection.

## Core interface

- `rust_music_library::scan(root, limits)` runs **only on explicit request**, on a worker thread. It bounds examined filesystem entries, track count and descent depth; checks FLAC/WAV signatures, ignores symlinks, and sorts the resulting relative paths. No tags are downloaded and no audio is played.
- `MusicSlot::new(sink, library)` creates one player and an initial selection if nonempty. It does not open audio output or load media.
- `MusicSlot::handle(Command::{Select,Next,Previous,Play,Stop,Poll}, focus)` is the single command entrypoint. Selection and navigation always stop the previous track and **do not autoplay** the next track. `Poll` processes at most one decoded PCM block and must execute off the UI/render thread.
- `MusicSlot::status()` is a small read-only snapshot: selection, number of tracks, playback generation/state/frames, and whether this slot currently holds exclusive audio focus.
- `MusicSlot::replace_library(catalog)` stops playback and retains the old relative-path selection when possible; scanning remains external to the slot.

## Audio focus: required host implementation

Implement `AudioFocus` in **Rust_Boy's shared, single-owner audio arbiter**. `try_acquire_music()` must return an exclusive `Box<dyn AudioLease>` only when TV, Radio and every other output module are not using the device. A lease's `Drop` must release the shared claim, even after errors, dropped modules, or interrupted playback. Do not implement a private arbiter in the music module: it would not protect Radio or TV.

The music slot retains the lease until `Stop`, track selection, EOF, output failure or module Drop. Repeated `Play` while already playing does nothing (it does not create a second stream). A denied focus request leaves the loaded decoder in `Ready` for a later explicit Play.

### Example host contract (schematic Rust)

```rust
use rust_music::{NullSink, PlaybackState};
use rust_music_host::{AudioFocus, AudioLease, Command, MusicSlot};
use rust_music_library::{scan, ScanLimits};

// Replace NullSink with the platform's explicit native PCM sink.
let catalog = scan("/operator/music", ScanLimits::default())?;
let mut slot = MusicSlot::new(NullSink::default(), catalog);
// `host_focus` must implement AudioFocus and share ownership with RADIO/TV.
slot.handle(Command::Play, &host_focus)?;
// Schedule this on a worker; never on the GUI / DAC callback:
while slot.status().playback.state == PlaybackState::Playing {
    slot.handle(Command::Poll, &host_focus)?;
}
```

## Micro-UI mapping

The host owns the phosphor-green, low-refresh CRT-like panel. It can display `MUSIC`, `track.title`, `track.container`, `playback.state`, and `frames_delivered/sample_rate` as integer-derived elapsed time. The primary controls are `PREV`, `PLAY`, `STOP`, `NEXT`, with a single slim settings affordance. Track titles are **filenames only**, not album/artist tags: those are later parser modules and must not be invented.

## Explicit limitations and acceptance

- No pause/resume, sample-accurate seek, playlist persistence, embedded album art, artist/tag parser or asynchronous library watcher yet. Do not expose fake controls.
- A path recheck rejects symlink replacement before file open, but cannot rule out a malicious local filesystem race without a platform-specific no-follow open; trust/permission boundaries must be set by the host.
- Host tests use `NullSink` and synthetic WAV files. Passing them proves neither CoreAudio nor ALSA kernel playback, nor exclusive focus interoperability until integrated with Rust_Radio/Rust_TV.
- `rust-music-library` and `rust-music-host` must build and test with `cargo --locked --offline` using only the local Rust toolchain. The current execution environment does not provide `cargo`; build status is unverified here.

## Integration acceptance checklist

1. Start Rust_Boy with MUSIC installed but inactive: no device opens and no filesystem scan occurs.
2. Scan an operator-selected directory on a worker with strict limits. Invalid/unsupported files are ignored, symlinks are not followed.
3. Let Radio hold the shared focus: Music Play is denied and creates no second audio sink.
4. Release Radio and press Play: Music acquires exactly one lease and one sink.
5. Press Next/Previous: output stops, focus releases, selection changes; no automatic resumption.
6. Reach EOF, inject a corrupted frame, or unload the module: the sink closes, focus releases, and the host screen remains responsive.
7. Pass mobile/desktop actual-device tests before claiming bit-perfect or production ready.
