# G6 - tiny offline MUSIC panel (128 x 64)

Status: source implemented; Rust compilation and hardware display integration not yet verified.

## Design

`rust-music-panel` is a separately embeddable, dependency-free Rust crate. It is
not a GUI application and does not own the screen, audio device, event loop,
threads, storage, or network. It converts a borrowed `MusicSlot` snapshot into
one 128 x 64 monochrome bitmap using an independently authored 5 x 7 ASCII
font. Each frame is exactly **1024 bytes** (1 bit/pixel, P4 PBM row order).
The host draws set pixels in its chosen color, for example CRT phosphor green.
There is no shader, animation timer, GPU, toolkit, icon download, or font file.

```
[Rust_Boy host] -- paints 128x64 bitmap; maps inputs; owns Settings
       |
       +-- rust-music-panel       (pure 1 KiB renderer)
       |      \-- PanelView<'_>  (borrows read-only snapshot/title)
       +-- rust-music-host        (exclusive audio focus and commands)
       +-- rust-music-library     (explicit local directory scan)
       +-- rust-music             (FLAC/WAV decode and PCM output)
```

## Operator interface

Top: `MUSIC`, selected track index / total, `SET`.
Middle: filename-derived title; actual playback state and FLAC/WAV type;
original sample rate, bit depth and channels **only when available**.
Bottom: left/right selection arrows plus `PLAY` and `STOP`. Actual input mapping:

| Input | Action | Audio behavior |
| --- | --- | --- |
| Previous | `Command::Previous` | Stops and selects, never auto-plays |
| Next | `Command::Next` | Stops and selects, never auto-plays |
| Confirm | `Play` when stopped; `Stop` when playing | Explicit only |
| Stop | `Command::Stop` | Releases music's audio lease |
| Settings | `PanelAction::OpenSettings` | Host opens actual shared settings panel |

The host MUST handle `OpenSettings` as navigation to its own settings screen;
no screen is created inside Music. Buttons should be debounced by the host. The
panel never calls the host or the audio engine from `render()`.

There is **no fabricated progress bar**: total duration, sample-accurate seek,
and metadata tags are not yet available to the panel. No play/pause symbol is
shown until real pause/resume semantics are implemented. The terminal preview
path is read-only and uses `NullSink`, not a hardware or second audio backend.

## API example

```rust
use rust_music_panel::{render, Button, PanelAction, PanelView, translate_button};

// The host owns `slot`, `focus`, its event loop, and physical audio sink.
let frame = render(PanelView::from_slot(&slot));
assert_eq!(frame.pixels().len(), 1024);
// Host paints frame.pixels() 1-bits in the selected monochrome color.

match translate_button(Button::Settings, slot.status().playback.state) {
    PanelAction::OpenSettings => { /* open the host's existing settings */ }
    PanelAction::Music(command) => { slot.handle(command, &focus)?; }
}
```

## Reproducible preview

The offline CLI can write one valid PBM P4 bitmap without audio devices:

```sh
cargo run --locked --offline -p rust-music-cli -- preview /path/to/music /tmp/music.pbm
```

Inspect the resulting file with any PBM viewer or directly feed its 1024 bytes
after the header to the embedded screen. No image library or network is needed.

## Acceptance gates

- Verify `size_of::<MonoFrame>() == 1024` and deterministic snapshots.
- Verify non-ASCII/control-containing filenames render safely with fallback.
- Verify empty library, maximum sample-rate/track-count and edge pixels.
- Verify UI render does not change selection, output, or audio focus.
- Verify `Previous`/`Next` never become a play command.
- Complete `cargo fmt`, Clippy, tests and the zero-dependency audit on a local
  Rust toolchain (still unavailable in this execution environment).
- Integrate physical screen color/contrast and exclusive Radio/TV audio focus
  in Rust_Boy separately; do not claim interoperability before those tests.
