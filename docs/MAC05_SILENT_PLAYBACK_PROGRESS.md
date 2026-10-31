# MAC-05 — Silent playback and observable track progress (2026-10-31)

**Status:** OPEN / observed by operator on macOS. The app presents Playing with no audible sound and no song progress UI. This is a regression/acceptance blocker, not proof of a codec or hardware fault.

## Verified source findings

- `crates/rust-music-ipc/src/lib.rs` View already exports `frames_delivered`, `total_frames`, and `format` (sample rate).
- `crates/rust-music-cli/src/main.rs` maps player status into these fields.
- `apps/rust-music-macos/RustMusicApp.swift`, `PackageProcessClient.request`, currently parses but discards those counters and the sample rate, returning only `PackageResponse.view(state)`.
- The SwiftUI Now Playing panel has no progress display; snapshots are polled while Playing.
- `PackageProcessClient` routes the supervised player's stderr to `FileHandle.nullDevice`, hiding potentially useful diagnostics. Capture stderr in a bounded, local, opt-in debug path, not a world-readable persistent media log.
- `Playing` means the player entered its state; it is not proof of audible DAC output. `frames_delivered` is decoder/output-delivery progress, not a physical DAC playback clock.

## Ordered investigation

1. Reproduce using a known non-silent 16-bit/44.1 kHz WAV and a verified FLAC; confirm the macOS Sound Output device and app volume. Do not conflate prior deliberately silent synthetic WAV test fixtures with audible acceptance.
2. Run the release CLI with `--features macos-output` on the same file and output device. Compare app vs CLI. Do not copy licensed media into Git.
3. Sample the existing RBPK Snapshot from the app: record state, format/sample rate, frames delivered, total frames and audio focus at two successive sample times. Compare whether frames advance, stay zero, or stop early. Preserve explicit no-autoplay.
4. Distinguish decoder progress from sink enqueue and macOS hardware actually playing; add bounded/error-only AudioQueue diagnostics (start/enqueue/stop OSStatus, selected route if available without a third-party runtime).
5. Inspect volume scaling, PCM silence vs nonzero samples, stream start timing and route changes. Fail closed on unsupported format or sink error; report actionable UI failure instead of remaining indefinitely Playing.
6. Preserve the locally authored, offline-only decoders/output and host-owned audio-focus contract.

## Required UI implementation

- Refactor `PackageResponse.view` to carry a typed status snapshot containing state, sample rate, `frames_delivered`, `total_frames`, and `audio_claimed`; strictly validate flags and bounds without changing RBPK wire format.
- Persist snapshot on the SwiftUI model, refresh from existing polls, clear on a different selected track and on load/stop/error as appropriate.
- Display read-only elapsed / total `mm:ss` (or `h:mm:ss`), a clamped accessible progress bar, and indeterminate/unknown duration when `total_frames` is unavailable. Avoid claiming physical output clock precision.
- No seeking control until a safe seek operation is explicitly implemented. Do not use a draggable Slider as fake seek.
- Add protocol decoding tests (known/unknown lengths, zero sample rate, malformed/truncated frames, counter overflow), pause/resume/track-switch UI regressions, and audio error-path tests.
- Keep text-only, no artwork/visualizer, 128x64 micro panel unchanged.

## Acceptance evidence

- macOS real, audible, non-silent WAV and FLAC through the standalone app, with route and output volume identified.
- A rising progress counter during actual playback, stable while paused, monotonic within a track, and reset at a track change.
- Honest playback errors and no erroneous Playing status after native output failure.
- `sh scripts/offline-gate.sh`, `sh scripts/build-macos-app.sh`, strict codesign verification, manual close/relaunch without surprise playback, and no regressions to queue/playlist operations.
- Record exact source SHA, OS/audio endpoint, command exits and observed behavior; never mark complete solely from simulated AudioQueue tests.
