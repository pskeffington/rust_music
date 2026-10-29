# rust_music

**A lightweight, native, locally authored lossless music engine for Rust_Boy.** Designed to stay alive on minimal handheld/desktop systems, run offline, and ship independently of the host shell.

All runtime codecs are ours. All Cargo dependencies are local workspace paths. No external playback/decoder libraries, registry packages, network or downloaded assets are required. The OS and preinstalled Rust standard toolchain are the only platform prerequisites. MIT licensed.

## G6 micro display

The new `rust-music-panel` crate renders a 128x64 monochrome MUSIC view into a
fixed **1024-byte** buffer, without a GUI toolkit, network, OS display library,
or audio I/O. Rust_Boy can tint that bitmap phosphor-green. It uses the
existing host audio focus and command contract; `Settings` is routed to the
host's actual settings screen, not a decorative control. Standalone preview:

```sh
cargo run --locked --offline -p rust-music-cli -- preview /path/to/music /tmp/music.pbm
```

See [G6 micro panel](docs/G6_MICRO_PANEL.md). Compilation and physical screen
integration remain unverified until a Rust toolchain and target hardware run
those checks.

## Implemented foundation

- **FLAC decoder**: native FLAC streams, 4-32-bit PCM, 1-8 channels, frame CRC8/CRC16, constant/verbatim/fixed/LPC subframes, Rice residuals, stereo decorrelation, frame-number continuity, full-file decoded PCM MD5 when provided. Corpus parity is **not yet certified**.
- **WAVE PCM decoder**: strict RIFF boundaries, odd-chunk padding, 8/16/24/32-bit integer PCM, and PCM WAVE_FORMAT_EXTENSIBLE (including 24-bit samples stored left-aligned in 32-bit containers).
- **AIFF / AIFF-C integer PCM decoder (G10)**: newly written local crate with strict FORM/COMM/SSND bounds, 80-bit sample rate and exact 8/16/24/32-bit PCM, plus AIFF-C signed NONE/twos/sowt; Operator-run native CLI PCM parity 7/7 on generated cases; full offline gate and hardware validation still pending.
- **Audiophile-only playback**: lossless native decoders; minimum playable 16-bit / 44.1 kHz. Lower-resolution inputs remain usable only in decoder conformance tests and are rejected before native DAC activation. MP3, AAC, Opus, Vorbis and other lossy codecs are excluded; M4A/ALAC is a planned, not implemented, addition.
- **Headless player**: explicit `load()` then `play()`, single active decoder/sink, no automatic sound or network activity.
- **Audio outputs (experimental)**: own macOS AudioQueue (`macos-output`) and *direct Linux ALSA kernel ioctl* (`alsa-output`) adapters. No libasound, mixer plugin, or audio server. Neither is yet hardware validated as bit-perfect.
- **Local music library**: bounded, explicit directory scan; FLAC/WAV/AIFF signature detection, stable filename ordering, symlink rejection and no background scanner.
- **Rust_Boy host slot**: typed select/next/previous/play/stop/poll commands, read-only status snapshots, and host-owned RAII audio-focus leasing; no GUI requirement.
- **Playback safety**: refuse mid-stream PCM format changes before output; clear focus state even after native close errors.
- **Development CLI**: bounded library scan, inspect full-file decode, FLAC/WAV to WAV, 1-bit PBM screen preview, and optional native output. Decode and preview never overwrite existing output files.
- **Local build audit**: dedicated `rust-music-audit` crate checks manifests, Cargo.lock, Rust FFI and forbidden network/third-party code.
- **Research**: 19 indexed references plus machine-readable engineering rules.

## Get started

```sh
sh scripts/offline-gate.sh      # no downloads, no third-party crates
cargo test --locked --offline --workspace
python3 scripts/check_literature.py  # optional research maintenance
python3 scripts/check_roadmap_index.py  # optional Codex task/agent/roadmap validator
python3 -m unittest discover -s tests -p 'test_roadmap_index.py'  # optional stdlib index tests
python3 scripts/test_flac_reference.py --fixtures-only  # OPTIONAL external reference validation
python3 scripts/test_flac_reference.py                  # OPTIONAL differential oracle, needs flac + cargo
python3 scripts/test_flac_user_corpus.py --mode oracle --require-complete /private/music  # OPTIONAL private real-audio oracle
python3 scripts/test_flac_user_corpus.py --mode rust --require-complete /private/music    # OFFLINE own decoder parity; needs cargo
python3 scripts/test_wave_reference.py --fixtures-only  # OPTIONAL, stdlib-only WAVE fixtures
python3 scripts/test_wave_reference.py                  # OPTIONAL, full WAVE PCM round-trip; needs cargo
python3 scripts/test_aiff_reference.py --fixtures-only  # OPTIONAL, stdlib-only AIFF/AIFF-C vectors
python3 scripts/test_aiff_reference.py                  # OPTIONAL, Rust native AIFF PCM parity; needs cargo
cargo run --locked --offline -p rust-music-cli -- scan ~/Music
cargo run --locked --offline -p rust-music-cli -- inspect ~/Music/track.flac
cargo run --locked --offline -p rust-music-cli -- decode ~/Music/track.flac /tmp/track.wav
# macOS native output (experimental):
cargo run --locked --offline -p rust-music-cli --features macos-output -- play ~/Music/track.flac
# Linux ALSA direct device (experimental):
cargo run --locked --offline -p rust-music-cli --features alsa-output -- play ~/Music/track.flac hw:0,0
```

All **12 workspace crates** are locally authored and have zero third-party Cargo dependencies. Use `rust_music_library::scan` for explicit discovery and `rust_music_host::MusicSlot` to integrate with Rust_Boy. The host provides an exclusive `AudioLease` shared with Radio/TV. Playback `Poll` must run on a worker, never on the GUI or native audio callback. Source is implemented; Rust compilation and physical playback are not yet verified here.

## Module and research guide

- [Architecture and micro-interface](docs/ARCHITECTURE.md)
- [Rust_Boy host and audio-focus contract](docs/HOST_CONTRACT.md)
- [Repository and machine navigation](docs/REPOSITORY_INDEX.md)
- [Phased roadmap and evidence gates](docs/ROADMAP.md)
- [Machine-readable gate/task/owner index](docs/roadmap-index.json)
- [Codex agent routing and handoff instructions](docs/CODEX_AGENT_MAP.md)
- [Codex root instructions](AGENTS.md)
- [Zero third-party dependency policy and offline gate](docs/DEPENDENCY_POLICY.md)
- [G2 FLAC validation ledger](docs/G2_VALIDATION.md)
- [G5 host/source validation ledger](docs/G5_VALIDATION.md)
- [G6 independent micro panel, controls, and validation](docs/G6_MICRO_PANEL.md)
- [G7 strict RIFF/WAVE PCM implementation and validation ledger](docs/G7_RIFF_CONFORMANCE.md)
- [G8 real-world FLAC corpus (checksums only; private audio not committed)](docs/G8_REALWORLD_FLAC_CORPUS.md)
- [G9 exact playback timing](docs/G9_TIMING_PROGRESS.md)
- [G10 audiophile-only format policy and native AIFF implementation](docs/G10_AUDIOPHILE_FORMATS.md)
- [Machine-readable lossless format admission matrix](docs/lossless-formats.json)
- [Cited comparative literature matrix](docs/LITERATURE_MATRIX.md)
- [Navigation index](docs/LITERATURE_INDEX.md)
- [Machine registry](docs/literature-index.json)

**Important:** Losslessly decoded integer PCM is not the same as verified bit-perfect DAC output. `FLAC -> WAV` parity and physical sample-path testing are separate gates. ALAC/M4A, CAF, RF64, Ogg FLAC, WavPack, native DSD, Windows WASAPI, and Android AAudio are not yet supported. AIFF/AIFF-C integer PCM passed seven operator-run native CLI parity vectors; complete offline gate and physical DAC acceptance remain pending. A filename extension is never proof of losslessness; the scanner only makes a provisional signature-based catalog and the player enforces codec/quality checks before playback.

**Validation truth:** Native MD5 verification occurs after complete decoding, not before playback starts. An all-zero FLAC STREAMINFO digest cannot establish end-to-end integrity. GitHub Actions PR #1 initially reported `startup_failure` with no jobs scheduled; Rust compilation and the full differential suite remain unverified until a runner successfully executes them.

**Local ownership boundary:** The implementation is independent; OS system APIs are permitted but not separately installed codec or sound libraries. AudioQueue and Linux kernel ALSA hardware testing and full decoder conformance are still outstanding. `Cargo.lock` is bootstrap-pinned; Rust toolchain validation has not yet run in this environment.
