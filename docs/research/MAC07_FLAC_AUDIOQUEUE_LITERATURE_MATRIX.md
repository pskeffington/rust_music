# MAC07 — FLAC-to-AudioQueue literature matrix and Codex implementation notes

Status: research/engineering guidance only; no macOS audible-output claim. Date: 2026-11-01.
Issue: CLI plays real FLAC; desktop SwiftUI/serve --desktop output has been reported silent. Root cause remains unproven.

| ID | Reference | Finding | Project action | Priority |
|---|---|---|---|---|
| LIT-FLAC-001 | RFC 9639 https://www.rfc-editor.org/rfc/rfc9639.html | Signed interleaved PCM, variable FLAC frame block sizes, STREAMINFO/MD5 and integrity checks | Decode same-file corpus and hash exact PCM; reject unsupported format changes | P1 |
| LIT-AQ-001 | Apple AudioStreamBasicDescription https://developer.apple.com/documentation/coreaudiotypes/audiostreambasicdescription | PCM stream description must match actual memory layout | Unit test sample rate, channels, bit depth, packed/signed/interleaved flags and bytes per frame | P0 |
| LIT-AQ-002 | Apple Audio Stream Basic Description Flags https://developer.apple.com/documentation/coreaudiotypes/audio-stream-basic-description-flags | Signedness, packing, endianness and layout flags are independent | Compare 16/24/32-bit serialization against expected little-endian interleaved PCM | P0 |
| LIT-AQ-003 | Apple Audio Queue Services https://developer.apple.com/documentation/audiotoolbox/audio-queue-services | Explicit allocation, enqueue, start, stop, dispose; OSStatus error paths | Trace counts and return codes outside callback; test partial allocation and failure recovery | P0 |
| LIT-AQ-004 | Apple AudioQueueOutputCallback https://developer.apple.com/documentation/audiotoolbox/audioqueueoutputcallback | Callback means buffer available for reuse; does not guarantee audible output | Separate decoded/queued/recycled counters from hardware listening result | P0 |
| LIT-AQ-005 | Apple AudioQueuePrime https://developer.apple.com/documentation/audiotoolbox/audioqueueprime%28_%3A_%3A_%3A%29 | Optional preparation of enqueued buffers before start | A/B current first-buffer-start with measured prebuffering; do not assert prime fixes silence | P1 |

## Source-code observations (reviewed MAC07 branch)

- `crates/rust-music-native/src/macos.rs`: 3 preallocated buffers, 4096 frames per slot, `AudioQueueNewOutput`, `EnqueueBuffer`, `Start`, and completion callback. Output callback only clears atomic in-flight flags.
- `crates/rust-music-cli/src/main.rs`: CLI `play` and persistent `serve --desktop` both construct `MacAudioQueueSink`, but use different control/poll lifecycle.
- `apps/rust-music-macos/RustMusicApp.swift`: GUI uses RBPK select/volume/play, then polls snapshots. UI Playing state must not be conflated with hardware audibility.
- Existing opt-in real FLAC server test uses `CountingSink`, not real AudioQueue. CLI/GUI listening acceptance is separate.

## Codex pass plan

1. **AQ-01 / format test:** construct test table for mono/stereo 16/24/32 bit at 44.1/48/96/192k Hz, signed packed little-endian values including -min, -1, 0, 1, max. Assert ASBD matches emitted bytes.
2. **AQ-02 / telemetry:** add bounded atomic counters for blocks/frames submitted, buffers enqueued, queue starts, callbacks, OSStatus failures and timeout. Callback must be lock-free, allocation-free and log-free. Read snapshot from non-realtime thread.
3. **AQ-03 / replay:** same real FLAC in `play`, `serve --desktop`, and GUI, same output device, capture format/frame/callback deltas and OSStatus; operator verifies audibility.
4. **AQ-04 / startup:** only if evidence indicates starvation/startup problem, experiment with 1 vs 2+ queued buffers before AudioQueueStart, using explicit tests and timing evidence; consider AudioQueuePrime without assuming it is necessary for LPCM.
5. **AQ-05 / lifecycle:** test Stop/Next/Pause/Resume while queued callbacks are pending, slow/long files, scan/library switching and device failure.
6. **AQ-06 / release:** offline Cargo fmt/clippy/tests, opt-in real FLAC server test, signed local bundle, physical DAC/speaker listening checks. Keep PR #3 draft until all required evidence exists.

## Mandatory diagnostics

Sample-rate, channels, bit depth, ASBD flags, expected bytes-per-frame, PCM frames decoded, frames written, AudioQueue enqueue count, start OSStatus, completion callback count, time to first enqueue, time to first callback, hardware output device, and GUI/CLI audible yes/no.

Failing test precedes root-cause patch. Do not swap Rust decoders for external libraries, install runtime dependencies, claim bit-perfect DAC without capture, or report a callback as proof of audibility.
