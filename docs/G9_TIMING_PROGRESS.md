# G9 — Exact PCM timing and micro-panel progress

Implemented on the active draft PR branch (2026-10-29). This pass adds no Cargo registry/Git dependencies, network calls, file watchers or device code.

## Contract
- `Decoder::total_frames() -> Option<u64>` reports a **declared** PCM sample-frame count per channel. The default is `None` for unknown or future codecs.
- FLAC reports `STREAMINFO.total_samples` when nonzero; zero remains unknown. WAV calculates its frame count from its validated `data` chunk length and `block_align`. No nominal bitrate or byte-size estimate is ever displayed as truth.
- `PlaybackSnapshot` records declared total frames on load, resets on stop, and exposes elapsed seconds, declared duration seconds, and integer progress per mille; these are time-of-output accounting values, not measurements of DAC completion.
- The engine rejects a block that would exceed the declared count **before writing to the sink**, and rejects premature EOF. A format mismatch, I/O failure, or count mismatch enters `Failed`.
- The 128x64 / 1024-byte panel displays `TIME elapsed / duration` and a one-bit progress rule when known. For unknown totals it shows elapsed only and omits the progress rule. Clock text is capped to 99:59 for screen geometry; the underlying u64 timing remains exact in whole PCM frames.
- No seeking, pause/resume, replay-gain, artist tags, or percent-complete claims for unknown-duration FLAC.

## Tests and acceptance
Unit tests include declared FLAC total, zero/unknown FLAC total, WAV frame counts for 8/24-bit, progress at 50% and 100%, no duration for unknown, and clamping at extreme u64 values. Additional compiler/static/hardware checks remain a separate gate.

Run on the offline, operator-owned Rust toolchain:

```sh
sh scripts/offline-gate.sh
python3 scripts/test_flac_user_corpus.py --mode rust --require-complete /private/music
```

The second command requires the six original private FLACs; do not commit, redistribute or auto-download music. The reference-only corpus pass does **not** validate the native Rust decoder. Do not merge draft PR #1 before the full compiler, decoder parity, and device/audio-focus acceptance gates.

## Follow-up
1. Compile all crates offline, repair any clippy/rustfmt or platform ABI defects.
2. Replay corrupted and truncated streams to confirm frame bounds and sample counts always fail closed.
3. Feed a real Rust_Boy host display and verify 128x64 layout, focus arbitration and hardware stop/drain.
4. Implement bounded Vorbis-comment metadata and indexed sample-accurate seek as separate G10 gates, without loading embedded PICTURE blocks into memory.
