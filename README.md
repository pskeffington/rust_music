# rust_music

**A lightweight, native, locally authored lossless music engine for Rust_Boy.** Designed to stay alive on minimal handheld/desktop systems, run offline, and ship independently of the host shell.

All runtime codecs are ours. All Cargo dependencies are local workspace paths. No external playback/decoder libraries, registry packages, network or downloaded assets are required. The OS and preinstalled Rust standard toolchain are the only platform prerequisites. MIT licensed.

## Implemented foundation

- **FLAC decoder**: native FLAC streams, 4-32-bit PCM, 1-8 channels, frame CRC8/CRC16, constant/verbatim/fixed/LPC subframes, Rice residuals, stereo decorrelation, frame-number continuity, full-file decoded PCM MD5 when provided. Corpus parity is **not yet certified**.
- **WAVE PCM decoder**: 8/16/24/32-bit RIFF PCM with bounded streaming.
- **Headless player**: explicit `load()` then `play()`, single active decoder/sink, no automatic sound or network activity.
- **Audio outputs (experimental)**: own macOS AudioQueue (`macos-output`) and *direct Linux ALSA kernel ioctl* (`alsa-output`) adapters. No libasound, mixer plugin, or audio server. Neither is yet hardware validated as bit-perfect.
- **Development CLI**: inspect full-file decode, FLAC/WAV to WAV, and optional native output.
- **Local build audit**: dedicated `rust-music-audit` crate checks manifests, Cargo.lock, Rust FFI and forbidden network/third-party code.
- **Research**: 19 indexed references plus machine-readable engineering rules.

## Get started

```sh
sh scripts/offline-gate.sh      # no downloads, no third-party crates
cargo test --locked --offline --workspace
python3 scripts/check_literature.py  # optional research maintenance
python3 scripts/test_flac_reference.py --fixtures-only  # OPTIONAL external reference validation
python3 scripts/test_flac_reference.py                  # OPTIONAL differential oracle, needs flac + cargo
cargo run --locked --offline -p rust-music-cli -- inspect ~/Music/track.flac
cargo run --locked --offline -p rust-music-cli -- decode ~/Music/track.flac /tmp/track.wav
# macOS native output (experimental):
cargo run --locked --offline -p rust-music-cli --features macos-output -- play ~/Music/track.flac
# Linux ALSA direct device (experimental):
cargo run --locked --offline -p rust-music-cli --features alsa-output -- play ~/Music/track.flac hw:0,0
```

Every workspace configuration has **zero third-party Rust dependencies** and builds as a pure headless codec/module. Start from `rust_music::open_file(path)` and `Player::new(host_owned_sink)` to embed in Rust_Boy. Playback `step()` must run on a decoder worker, not the GUI or native audio callback.

## Module and research guide

- [Architecture and micro-interface](docs/ARCHITECTURE.md)
- [Phased roadmap and evidence gates](docs/ROADMAP.md)
- [Zero third-party dependency policy and offline gate](docs/DEPENDENCY_POLICY.md)
- [G2 FLAC validation ledger](docs/G2_VALIDATION.md)
- [Cited comparative literature matrix](docs/LITERATURE_MATRIX.md)
- [Navigation index](docs/LITERATURE_INDEX.md)
- [Machine registry](docs/literature-index.json)

**Important:** Losslessly decoded integer PCM is not the same as verified bit-perfect DAC output. `FLAC -> WAV` parity and physical sample-path testing are separate gates. ALAC/M4A, AIFF, Ogg FLAC, Windows WASAPI, and Android AAudio are not yet supported.

**Validation truth:** Native MD5 verification occurs after complete decoding, not before playback starts. An all-zero FLAC STREAMINFO digest cannot establish end-to-end integrity. GitHub Actions PR #1 initially reported `startup_failure` with no jobs scheduled; Rust compilation and the full differential suite remain unverified until a runner successfully executes them.

**Local ownership boundary:** The implementation is independent; OS system APIs are permitted but not separately installed codec or sound libraries. AudioQueue and Linux kernel ALSA hardware testing and full decoder conformance are still outstanding. `Cargo.lock` is bootstrap-pinned; Rust toolchain validation has not yet run in this environment.
