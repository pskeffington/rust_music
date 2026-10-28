# rust_music

**A lightweight, native, locally authored lossless music engine for Rust_Boy.** Designed to stay alive on minimal handheld/desktop systems, run offline, and ship independently of the host shell.

All runtime codecs are ours: we do not embed Symphonia, Claxon, FFmpeg, libFLAC, CPAL, or Rodio. OS audio devices are reached through thin, feature-gated native API shims. MIT licensed.

## Implemented foundation

- **FLAC decoder**: native FLAC streams, 4-32-bit PCM, 1-8 channels, frame CRC8/CRC16, constant/verbatim/fixed/LPC subframes, Rice residuals, stereo decorrelation, frame-number continuity, full-file decoded PCM MD5 when provided. Corpus parity is **not yet certified**.
- **WAVE PCM decoder**: 8/16/24/32-bit RIFF PCM with bounded streaming.
- **Headless player**: explicit `load()` then `play()`, single active decoder/sink, no automatic sound or network activity.
- **Audio outputs (experimental)**: own macOS AudioQueue (`macos-output`) and Linux ALSA (`alsa-output`) adapters. Neither is yet hardware validated as bit-perfect.
- **Development CLI**: inspect full-file decode, FLAC/WAV to WAV, and optional native output.
- **Research**: 17 cited entries plus machine-readable/validated engineering rules.

## Get started

```sh
cargo test --workspace
python3 scripts/check_literature.py
python3 scripts/test_flac_reference.py --fixtures-only  # reference fixtures; cargo not needed
python3 scripts/test_flac_reference.py                  # Rust decoder parity; needs cargo
cargo run -p rust-music-cli -- inspect ~/Music/track.flac
cargo run -p rust-music-cli -- decode ~/Music/track.flac /tmp/track.wav
# macOS native output (experimental):
cargo run -p rust-music-cli --features macos-output -- play ~/Music/track.flac
# Linux ALSA direct device (experimental):
cargo run -p rust-music-cli --features alsa-output -- play ~/Music/track.flac hw:0,0
```

The no-native-features default has **no third-party Rust dependencies** and builds as a pure headless codec/module. Start from `rust_music::open_file(path)` and `Player::new(host_owned_sink)` to embed in Rust_Boy. Playback `step()` must run on a decoder worker, not the GUI or native audio callback.

## Module and research guide

- [Architecture and micro-interface](docs/ARCHITECTURE.md)
- [Phased roadmap and evidence gates](docs/ROADMAP.md)
- [Cited comparative literature matrix](docs/LITERATURE_MATRIX.md)
- [Navigation index](docs/LITERATURE_INDEX.md)
- [Machine registry](docs/literature-index.json)

**Important:** Losslessly decoded integer PCM is not the same as verified bit-perfect DAC output. `FLAC -> WAV` parity and physical sample-path testing are separate gates. ALAC/M4A, AIFF, Ogg FLAC, Windows WASAPI, and Android AAudio are not yet supported.

**Validation truth:** Native MD5 verification occurs after complete decoding, not before playback starts. An all-zero FLAC STREAMINFO digest cannot establish end-to-end integrity. GitHub Actions PR #1 initially reported `startup_failure` with no jobs scheduled; Rust compilation and the full differential suite remain unverified until a runner successfully executes them.
