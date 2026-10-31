# rust_music

**A lightweight, native, locally authored lossless music engine for Rust_Boy.** Designed to stay alive on minimal handheld/desktop systems, run offline, and ship independently of the host shell.

All runtime codecs are ours. All Cargo dependencies are local workspace paths. No external playback/decoder libraries, registry packages, network or downloaded assets are required. The OS and preinstalled Rust standard toolchain are the only platform prerequisites. MIT licensed.

## Product direction: desktop library manager + Pip-Boy MUSIC socket

**Primary design:** [Rust Music 2000 interface direction](docs/design/PRIMARY_UI_DIRECTION.md) and [portable MUSIC socket schema](docs/design/PIPBOY_SOCKET_SCHEMA.md). The approved reference specifies the phosphor-green visual hierarchy and a mountable MUSIC content surface. It does **not** replace the native lossless engine or require the Pip-Boy to render a desktop-sized ornamental chassis. The image attachment itself has not yet been committed to this repository; the design decision is documented.

**Intended operator workflow:** organize, search and sort a local lossless music library on the desktop; save favorites, queue and named playlists; explicitly export a whole library or selected tracks/playlists as a verified offline package; mount the preloaded catalog and music in the Rust Boy 2000 MUSIC module. Subsequent syncs should transmit deltas rather than copying unchanged FLAC files. Source media stays untouched. The Pip-Boy remains usable without an active desktop, cloud service or network.

| Target | Role | Current release status |
| --- | --- | --- |
| **macOS** | First standalone manager/player, native audio output and eventual export UI | Native app/library and text queue/playlist workflow exists; Play, Pause/Resume, Stop, Next, Previous and software volume were exercised, and the user confirms audible output. Device recovery, DAC measurement, live host audio-focus, and hardware release evidence remain open |
| **Linux** | Future standalone manager/player with same portable catalog | Core Rust and experimental kernel ALSA output exist; desktop GUI and portable export are planned |
| **Windows** | Future standalone manager/player with same portable catalog | GUI, native WASAPI output and package conformance remain planned |
| **Rust Boy / Pip-Boy** | Host-mounted offline MUSIC socket, preloaded catalog, micro panel, shared audio focus | Socket and 128x64 panel foundations exist; live trusted host integration, deployment and physical acceptance remain open |

**Architecture:** one Rust decoder/player and authoritative session; versioned, bounded catalog/command snapshots; macOS desktop and Pip-Boy presentation adapters. Rust Boy owns audio arbitration alongside Radio/TV, navigation, settings, storage permissions, and physical display. The existing micro frame is exactly 1,024 bytes. Package writing, incremental validation and mount-on-device are specified, **not yet implemented**.

**Implementation sequence:** finish [standalone macOS v1.0 acceptance](docs/CODEX_MACOS_V1_DELIVERABLE.md) and RM-020 while doing read-only socket inventory; then RM-021–027 cover manifest, packager, macOS export, Pip-Boy mounting, Linux/Windows adapters and on-device tests. See [phased roadmap](docs/ROADMAP.md), [machine-readable tasks](docs/roadmap-index.json), [agent map](docs/CODEX_AGENT_MAP.md), and [portable deployment protocol](docs/design/PORTABLE_LIBRARY_DEPLOYMENT.md). PR #1 remains gated; design documents are not build or hardware evidence.

**Release scope:** macOS v1.0 is intentionally text-only, with no album art, visualizers or decorative image assets. The approved design guides information hierarchy, phosphor styling and socket layout. Optional Curie animation and advanced desktop presentation require later policy review and separate resource/accessibility gates. All playback remains local, offline, lossless-only with locally authored Rust components.

## G6 micro display

The new `rust-music-panel` crate renders a 128x64 monochrome MUSIC view into a
fixed **1024-byte** buffer, without a GUI toolkit, network, OS display library,
or audio I/O. Rust_Boy can tint that bitmap phosphor-green. It uses the
existing host audio focus and command contract; `Settings` is routed to the
host's actual settings screen, not a decorative control. Standalone preview:

```sh
cargo run --locked --offline -p rust-music-cli -- preview /path/to/music /tmp/music.pbm
```

See [G6 micro panel](docs/G6_MICRO_PANEL.md). The Rust workspace now passes its
local offline build gate; physical screen integration and host controls remain
unverified.

## Implemented foundation

- **FLAC decoder**: native FLAC streams, 4-32-bit PCM, 1-8 channels, frame CRC8/CRC16, constant/verbatim/fixed/LPC subframes, Rice residuals, stereo decorrelation, frame-number continuity, full-file decoded PCM MD5 when provided. Seven generated CLI vectors and six private real-world tracks pass native PCM/integrity checks; the private corpus remains outside Git.
- **WAVE PCM decoder**: strict RIFF boundaries, odd-chunk padding, 8/16/24/32-bit integer PCM, and PCM WAVE_FORMAT_EXTENSIBLE (including 24-bit samples stored left-aligned in 32-bit containers); six generated native CLI vectors pass exact PCM parity.
- **AIFF / AIFF-C integer PCM decoder (G10)**: locally authored crate with strict FORM/COMM/SSND bounds, exact 80-bit sample rate and 8/16/24/32-bit PCM, plus signed NONE/twos/sowt; local native CLI PCM parity 7/7 and offline workspace gate pass; hardware validation remains pending.
- **Audiophile-only playback**: lossless native decoders; minimum playable 16-bit / 44.1 kHz. Lower-resolution inputs remain usable only in decoder conformance tests and are rejected before native DAC activation. MP3, AAC, Opus, Vorbis and other lossy codecs are excluded. Bounded ALAC M4A and CAF profiles pass generated exact PCM parity; AAC/DRM, unsupported codecs and custom CAF channel layouts fail closed. Broader real-world ALAC corpus coverage and production admission remain open.
- **Headless player**: explicit `load()` then `play()`, single active decoder/sink, no automatic sound or network activity.
- **Audio outputs (experimental)**: own macOS AudioQueue (`macos-output`) and *direct Linux ALSA kernel ioctl* (`alsa-output`) adapters. No libasound, mixer plugin, or audio server. Neither is yet hardware validated as bit-perfect.
- **Local music library**: bounded, explicit directory scan; FLAC/WAV/AIFF/CAF/M4A/MP4 signature detection, stable filename ordering, symlink rejection and no background scanner. MP4-family container detection does not imply codec admission; AAC remains rejected by the decoder.
- **Rust_Boy host slot**: typed select/next/previous/play/stop/poll commands, read-only status snapshots, and host-owned RAII audio-focus leasing; no GUI requirement.
- **Playback safety**: refuse mid-stream PCM format changes before output; clear focus state even after native close errors.
- **Development CLI**: bounded library scan, inspect/full decode, lossless source to WAV, sample-bounded seek-range export for seek-capable decoders, 1-bit PBM screen preview, and optional native output. Decode and preview never overwrite existing output files.
- **Standalone macOS app**: native SwiftUI shell around the local Rust CLI with explicit file selection, decoder inspection, Play and Stop. Build with `sh scripts/build-macos-app.sh`; the app bundle is local-only and does not package music.
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
python3 scripts/test_caf_reference.py                   # OPTIONAL, FFmpeg-generated CAF PCM parity; needs ffmpeg + cargo
python3 scripts/test_m4a_corpus.py                      # OPTIONAL, CC0 ALAC M4A corpus parity/ingestion; needs ffmpeg + ffprobe + cargo
cargo run --locked --offline -p rust-music-cli -- scan ~/Music
cargo run --locked --offline -p rust-music-cli -- inspect ~/Music/track.flac
cargo run --locked --offline -p rust-music-cli -- decode ~/Music/track.flac /tmp/track.wav
cargo run --locked --offline -p rust-music-cli -- decode-range ~/Music/track.flac 44100 4096 /tmp/track-range.wav
# macOS native output (experimental):
cargo run --locked --offline -p rust-music-cli --features macos-output -- play ~/Music/track.flac
# Linux ALSA direct device (experimental):
cargo run --locked --offline -p rust-music-cli --features alsa-output -- play ~/Music/track.flac hw:0,0
```

All **17 workspace crates** are locally authored and have zero third-party Cargo dependencies. The local Rust 1.99.1/macOS offline gate passes; generated FLAC, WAVE, AIFF, CAF and bounded WavPack vectors and six private real-world FLAC tracks pass recorded native PCM parity. Use `rust_music_library::scan` for explicit discovery and `rust_music_host::MusicSlot` to integrate with Rust_Boy. The host must provide an exclusive `AudioLease` shared with Radio/TV. Playback `Poll` must run on a worker, never on the GUI or native audio callback. Pip-Boy modules remain distinct package identities mounted into the common container; the Music adapter must not mutate or uninstall Radio. Physical playback and actual shared host arbitration remain unverified; see the [Pip-Boy adapter contract](docs/PIP_BOY_ADAPTER.md).

## Codex production continuation

Start with the pinned [agent_skills-first production directive](docs/CODEX_AGENT_SKILLS_FIRST_PRODUCTION.md) and preserve all unfinished local checkout work before integrating remotely. Then follow the [zero-artwork micro-library specification](docs/MICRO_LIBRARY_ZERO_ARTWORK.md). This is a tasking order, not a completed release.

## Standalone macOS v1.0 deliverable

The current Codex release objective and Mac-only acceptance checklist are defined in [docs/CODEX_MACOS_V1_DELIVERABLE.md](docs/CODEX_MACOS_V1_DELIVERABLE.md). First deliver a working text-only Mac library/playlist player with real transport and audible native output; EQ, autoplay, Rust Boy hardware and public notarized distribution remain separately gated. This document specifies a target, not a completed release.

## Winamp-like library import and EQ engineering

The active product tasking is [Codex library-load/autoplay/EQ gates](docs/CODEX_WINAMP_LIBRARY_AUTOPLAY_EQ.md). Autoplay is **optional, default off**, and may start only after a user explicitly loads a library and audio focus is granted. The native EQ is bypassable; enabled EQ intentionally processes PCM. The zero-artwork policy remains absolute. This is a specification, not a verified implementation.

## Media library design: no artwork

The text-first library, search, playlist and queue roadmap is specified in [the zero-artwork micro-library directive](docs/MICRO_LIBRARY_ZERO_ARTWORK.md). No cover art or image decoding, extraction, caching, or display is permitted; the fixed monochrome 128x64 screen remains a text/control surface only. These planned features are not a claim of implementation.

## Module and research guide

- [Architecture and micro-interface](docs/ARCHITECTURE.md)
- [Rust_Boy host and audio-focus contract](docs/HOST_CONTRACT.md)
- [Package-process protocol contract and boundaries](docs/PACKAGE_PROCESS.md)
- [Pip-Boy universal adapter integration contract](docs/PIP_BOY_ADAPTER.md)
- [Repository and machine navigation](docs/REPOSITORY_INDEX.md)
- [Phased roadmap and evidence gates](docs/ROADMAP.md)
- [Machine-readable gate/task/owner index](docs/roadmap-index.json)
- [Codex agent routing and handoff instructions](docs/CODEX_AGENT_MAP.md)
- [Pinned software-agent workflow and Rust Music role mapping](docs/AGENT_SKILLS_INTEGRATION.md)
- [Codex production and release playbook](docs/CODEX_PRODUCTION_PLAYBOOK.md)
- [Dated work and validation log](docs/WORK_LOG.md)
- [Codex root instructions](AGENTS.md)
- [Current engineering handoff and agent assignments (2026-10-30)](docs/ENGINEERING_HANDOFF_2026-10-30.md)
- [RM-001 operator gate and private evidence capture](docs/RM001_OPERATOR_GATE.md)
- [PR #1 merge-readiness ledger and offline premerge check](docs/PR1_MERGE_READINESS.md)
- [2026-10-31 branch and issue triage](docs/BRANCH_AND_ISSUE_TRIAGE_2026-10-31.md)
- [Zero third-party dependency policy and offline gate](docs/DEPENDENCY_POLICY.md)
- [G2 FLAC validation ledger](docs/G2_VALIDATION.md)
- [G5 host/source validation ledger](docs/G5_VALIDATION.md)
- [G6 independent micro panel, controls, and validation](docs/G6_MICRO_PANEL.md)
- [G7 strict RIFF/WAVE PCM implementation and validation ledger](docs/G7_RIFF_CONFORMANCE.md)
- [G8 real-world FLAC corpus (checksums only; private audio not committed)](docs/G8_REALWORLD_FLAC_CORPUS.md)
- [Pip-Boy adapter contract and open integration gaps](docs/PIP_BOY_ADAPTER.md)
- [G9 exact playback timing](docs/G9_TIMING_PROGRESS.md)
- [Micro library zero-artwork policy](docs/MICRO_LIBRARY_ZERO_ARTWORK.md)
- [G10 audiophile-only format policy and native AIFF implementation](docs/G10_AUDIOPHILE_FORMATS.md)
- [Machine-readable lossless format admission matrix](docs/lossless-formats.json)
- [Cited comparative literature matrix](docs/LITERATURE_MATRIX.md)
- [Navigation index](docs/LITERATURE_INDEX.md)
- [Machine registry](docs/literature-index.json)

**Important:** Losslessly decoded integer PCM is not the same as verified bit-perfect DAC output. Generated `FLAC -> WAV`, WAVE, AIFF and bounded M4A/CAF PCM parity and physical sample-path testing are separate gates. Broad ALAC admission, Ogg FLAC, WavPack, native DSD, Windows WASAPI, and Android AAudio remain unsupported. RF64/ds64 integer PCM parsing is implemented with synthetic large-size coverage; a real large-file corpus and hardware acceptance remain open. The local offline gate and six private FLAC decoder parity checks pass; host arbitration and physical DAC acceptance remain open. A filename extension is never proof of losslessness; the scanner only makes a provisional signature-based catalog and the player enforces codec/quality checks before playback.

**Validation truth:** Native MD5 verification occurs after complete decoding, not before playback starts. An all-zero FLAC STREAMINFO digest cannot establish end-to-end integrity. GitHub Actions PR #1 previously reported `startup_failure` with no jobs scheduled. The local offline build and generated-vector PCM parity pass; all six private tracks passed the combined reference/native PCM corpus run. No current CI result, host integration, or physical bit-perfect output is inferred.

**Local ownership boundary:** The implementation is independent; OS system APIs are permitted but not separately installed codec or sound libraries. macOS AudioQueue and Linux kernel ALSA hardware testing remain outstanding. The offline gate was locally executed with Rust/Cargo 1.99.1 on macOS; the native Linux feature and hardware path remain unverified here.

## Branch and release status

The implementation currently lives in [draft PR #1](https://github.com/pskeffington/rust_music/pull/1), not the released default branch. The project is designed to carry only the music you want, locally or in disjointed/offline deployments. This describes the deployment goal; playback remains offline and network-free. Do not merge until the evidence gates recorded in the roadmap have passed. See the [branch and issue triage](docs/BRANCH_AND_ISSUE_TRIAGE_2026-10-31.md) and [RM-001 gate issue #2](https://github.com/pskeffington/rust_music/issues/2).
