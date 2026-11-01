# MAC-06 — Playback diagnosis, UI reconstruction, and A/B qualification

**Priority:** P0 playback truth + P1 UI usability. **Status:** SPECIFIED / NOT IMPLEMENTED. **Date:** 2026-10-31. **Depends on:** [MAC-05](MAC05_SILENT_PLAYBACK_PROGRESS.md). Product: offline, text-only, lossless Rust Music standalone macOS followed by host-mounted Pip-Boy MUSIC. No third-party codec/audio dependency.

## 1. Observations versus hypotheses

Observed by operator:
- Mac app compiles, local ad-hoc signature validates, and app opens.
- UI indicates PLAYING on selected Track 011, displays software volume 100%, but no audible output.
- No elapsed time, total duration, waveform, or progress indicator in Now Playing.
- Test CLI attempt used literal placeholder `/full/path/to/Blood, Milk and Sky.flac` and failed with OS error 2 **before decoding**. This is not evidence of native audio failure.
- Finder Get Info identifies another FLAC on SMB `smb://Archive._smb._tcp.local/Data/Downloads/` (More Human Than Human); that is **not established as the Track 011 file**. Test each track by its exact path; never silently equate them.
- App screenshot shows large unused mid-panel space, small transport controls and a horizontal volume slider.

Source confirmed on main inspection:
- RBPK `View` supplies `format`, `frames_delivered`, `total_frames`, `audio_claimed`; CLI maps player snapshot to these.
- The Swift PackageProcessClient discards the progress fields and sample rate when parsing view replies, retaining only state.
- The UI polls snapshots while Playing, but cannot display progress.
- The package child stderr is routed to null device; debugging may require bounded, explicit diagnostics.
- Reported Playing is **not** proof of actual hardware DAC delivery.

Unverified and not to assert as cause: SMB latency/access, sample-format conversion, muted/wrong audio route, enqueue/start failure, decoder returning silent PCM, incorrect selected file, stream EOF, stale UI state. Do not equate the Get Info filename with the track shown in the screenshot.

## 2. P0 deterministic diagnosis

Define a local, operator-controlled test corpus: known audible non-silent 16-bit/44.1k stereo PCM WAV, matching verified FLAC, one representative privately owned file. Compute SHA-256 of test bytes locally; never commit media or absolute private paths.

| A/B ID | A | B | What to measure |
| --- | --- | --- | --- |
| AB-01 | CLI native macos-output | SwiftUI app on same file | audible output, status, exit code, process lifecycle |
| AB-02 | local APFS copy | same bytes via mounted SMB path | SHA-256 equality, scan/first audible output time, errors |
| AB-03 | known 16/44.1 WAV | same decoded PCM in FLAC | exact PCM parity, frames, audible output |
| AB-04 | built-in speakers | chosen USB/headphone output | current route, volume, startup/transition status |
| AB-05 | 100% app volume | 25% app volume | measurable amplitude change, clipping, silence detection |
| AB-06 | stop/start same track | pause/resume same track | frame progress and focus/queue state transitions |
| AB-07 | app cold launch no library | explicit library load then Play | zero background scan/audio, correct state |
| AB-08 | baseline current UI | redesigned UI with same playback backend | task-completion errors/time, accessibility, progress accuracy |
| AB-09 | 2k local tagged catalog | 20k tagged catalog | scan/search latency, memory and scroll responsiveness |
| AB-10 | normal end of file | device disappearance/injected enqueue failure | terminal state, surfaced error, audio-focus release, explicit recovery |

Use matched files/devices, repeat at least 3 independent sessions for AB-01–07; 5 or more for performance samples; report raw measures, medians/ranges and operator environment. These are **engineering paired A/B comparisons**, not randomized-user efficacy claims. Test short and long tracks, including one with missing total_frames; choose a real audible fixture, never silent synthetic smoke data. Record both playback child PID and selected output endpoint. If CLI and GUI disagree, prioritize subprocess/IPC; if both silent with nonzero frames, prioritize sink/route and acoustic verification; if frames do not advance, prioritize stream scheduler/decode.

## 3. Instrumentation and proof hierarchy

Record for each attempt:
- SHA/test asset type, source kind (APFS/SMB), exact selected catalog identity (redact private paths), format/channels/sample rate, and file admission decision.
- Each transition: Empty, Ready, Playing, Paused, Stopped, Finished, Failed; monotonic timestamps, selected track index, `frames_delivered`, `total_frames`, audio-claim flag.
- Decoder nonzero PCM/peak signal, sink write/enqueue counts, AudioQueueNewOutput/Start/Enqueue/Stop OSStatus errors, end-of-stream, process exit code.
- Actual route as available from OS API and operator confirmation; local user-controlled acoustic confirmation required to state audible success.

Explicit distinction: frames delivered to sink are NOT a device sample clock, DAC output measurement or guaranteed audible sound. No unbounded logs and no automatic network or telemetry. Provide an opt-in diagnostic mode that redacts paths and never captures raw licensed audio. Never block the audio callback with I/O.

## 4. UI reconstruction: desktop 1.1

Keep phosphor text-only visual language and socket inspiration. Priority is a functional player, not decoration.

Desktop layout:
- Top: title, explicit Load Library, compact audio output/state indicator (only when verified).
- Left: Player, Library, Queue, Playlists with clear active view and keyboard focus.
- Center: persistent Now Playing content with full title, artist/album, format/channels/rate where known, elapsed / total time, accessible **read-only** progress bar, queued-next text, visible error details.
- Bottom sticky: transport controls, labelled software volume with numeric value and mute state, shortcut hints. Play/Pause should be one state-appropriate primary action or clearly distinguish their actions. Show button disabled reasons.
- Expandable diagnostics drawer: decoded frames, source type, focus lease, sink status, last OSStatus, current device and child process state, without private filesystem path exposure.
- Small screens: adapt columns; prioritize title, current state, progress, transport, and library controls. Do not introduce artwork or visualizers.

Progress requirements:
- Change Swift `PackageResponse.view(state)` to typed `PlaybackSnapshot` while preserving RBPK v1 bytes; parse/validate sample rate, delivered/total frames and focus.
- Display `elapsed = delivered_frames/sample_rate` and `duration = total_frames/sample_rate` where valid. Clamp progress to [0,1]. Unknown totals show elapsed and unknown-duration state. Protect against zero sample rate, invalid flags, overflow and truncated reply.
- Prefix the progress label/diagnostics with `Decoded/delivered` where needed: avoid implying sample-accurate acoustic progress; test latency to avoid overstating real-time position.
- Reset position on new track, stop and library reload; pause freezes progress; resume continues. Track finished reaches end only when reported accurately.
- No draggable/seekable progress until true bounded seek support is implemented. No fake time animation.
- VoiceOver labels, keyboard navigation, predictable focus and contrast; text sizes usable without losing track details.

## 5. Agent tasks / implementation order

- **MAC06-A (P0):** Evidence harness and non-silent WAV/FLAC matched AB-01–04. No modifications until operator/source observations captured.
- **MAC06-B (P0):** Typed RBPK snapshot Swift parser + unit/adversarial tests (invalid tags, counters and partial frames).
- **MAC06-C (P0):** Now Playing elapsed/total/progress and reset/pause/end transitions; snapshot-driven not timer-faked.
- **MAC06-D (P0):** Bounded local child/AudioQueue diagnostics, surfaced errors, watchdog for no advancing frames while Playing (avoid falsely failing legitimately buffered or paused playback).
- **MAC06-E (P0):** Repair actual root cause identified from AB-01–07; add failing reproduction test before fix and then green regression.
- **MAC06-F (P1):** Desktop navigation and layout/accessibility refactor; no codec/sink modifications in this pass.
- **MAC06-G (P1):** AB-08–10 performance, device lifecycle and usability review, release evidence ledger update.

One writer per shared file, preserve user work and current main history. Do not merge PRs or claim production readiness without evidence. Route agents via [AGENTS.md](../AGENTS.md) and [CODEX_AGENT_MAP](CODEX_AGENT_MAP.md); task IDs in this document are a scoped acceptance work package, not automatically registered roadmap-index IDs.

## 6. Gates to close

MAC06 audio PASS: app audible for same non-silent WAV/FLAC that plays via native CLI on verified device, three repeat sessions, normal pause/resume/stop and correct nonzero frame advancement. No false Playing after sink error. SMB compare results reported separately, not assumed.
MAC06 UI PASS: correct displayed elapsed/duration/progress for known-length WAV/FLAC with observed bounded error versus frame counters; no frozen/spurious advancement while paused; unknown-duration and malformed protocol regression pass; keyboard/VoiceOver testing completed.
MAC06 release PASS: `cargo test --locked --offline --workspace`, `sh scripts/offline-gate.sh`, `sh scripts/build-macos-app.sh`, `codesign --verify --deep --strict --verbose=2 target/macos/RustMusic.app`, actual `open -n "$PWD/target/macos/RustMusic.app"` and manual audio verification on operator Mac. Record command exit codes. Existing physical DAC/output-device-recovery, Pip-Boy host-focus, notarization and distribution gates remain independent.

## 7. Operator repro commands

```sh
cd ~/GitHub/rust_music
find /Volumes -type f -iname '*.flac' 2>/dev/null | head -n 20
# Use the EXACT real track path from Finder; do not use a placeholder.
cargo run --locked --offline -p rust-music-cli --features macos-output -- play "/mounted/real/path/track.flac"
sh scripts/build-macos-app.sh
open -n "$PWD/target/macos/RustMusic.app"
```

Do not automatically copy private user media from SMB. Prefer a test fixture with explicit consent and report its provenance.
