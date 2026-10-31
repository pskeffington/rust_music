# Rust Music — phased engineering roadmap

The **machine-readable task, gate and agent registry** is [roadmap-index.json](roadmap-index.json). Keep this file readable for operators; let the registry own stable RM task IDs, gate states, dependencies and Codex routing. Do not infer test results from a commit, source review or a filename.

## Non-negotiable release contract

- Playback accepts only verified **lossless** codecs and at least 16-bit / 44.1 kHz PCM. Lower-resolution decoder fixtures are permitted for tests, never playback.
- Own all runtime decoders and adapters; Cargo dependencies are workspace-local, zero remote registry/Git packages or bundled third-party codecs. Native OS/device APIs are explicit platform prerequisites.
- Verify exact decoded integer PCM independently of hardware. **Bit-perfect DAC** needs physical capture and exact device negotiation, not just FLAC-to-WAV parity.
- Keep Rust_Boy host-owned single audio focus, no unrequested autoplay or background scanning, and the 128×64, 1024-byte display budget. Optional autoplay remains future work: it may run only after explicit user library load, persistent opt-in (default off), a successful bounded scan, and host focus grant; never on startup, refresh, or reconnect.
- The offline Rust gate has no Python dependency; optional agent/index maintenance checks use Python stdlib only.

## Gate status and ownership

| Gate | Scope | Codex owner | Evidence state | Next acceptance |
| --- | --- | --- | --- | --- |
| **G0** | Ownership, standards and offline audit | `index_curator` | partial evidence | Offline dependency audit, fmt, Clippy, workspace tests, release build and macOS feature check pass locally on Rust 1.99.1; target hardware and host integration remain open |
| **G1** | Integer PCM core and explicit playback state | `codec_engineer` | source implemented | Workspace tests pass, including lossless playback floor and fail-closed output state; actual device behavior remains unverified |
| **G2** | Native FLAC with CRC, Rice/LPC, PCM MD5 | `codec_engineer` | partial evidence | Seven generated vectors and six private real-world tracks pass native PCM parity; wider codec/hardware claims remain separate |
| **G3** | Mac AudioQueue / Linux kernel PCM output | `audio_backend` | source implemented | Source present; DAC format, drain and underrun measurement pending |
| **G4** | Expanded lossless umbrella | `codec_engineer` | partial evidence | AIFF, generated M4A/ALAC, and bounded CAF profiles are implemented; broader ALAC admission remains |
| **G5** | Rust_Boy socket and shared audio-focus | `host_integrator` | partial evidence | Rust Music has a bounded RBPK v1 contract, spawned CLI lifecycle coverage, and a local release-binary hello/activate/shutdown smoke; Pip-Boy catalogs MUSIC as a distinct uninstalled package and has a supervised runner. Trusted release registration, live mount, user input/worker bridge, shared Radio/TV/Music contention, and field acceptance remain open |
| **G6** | 128x64 monochrome 1KiB panel | `host_integrator` | source implemented | Renderer exists; hardware screen/controls pending |
| **G7** | Strict RIFF/WAVE PCM and extensible WAV | `codec_engineer` | partial evidence | Six native RIFF PCM cases and synthetic RF64 >4 GiB streaming/sample parity pass; actual large-file corpus and hardware output remain unverified |
| **G8** | Six-file private FLAC differential corpus | `gate_verifier` | partial evidence | Reference FLAC/FFmpeg and native Rust PCM hashes match on all six private files; artwork and hardware stress remain separate |
| **G9** | Exact declared-frame timing and display progress | `host_integrator` | source implemented | Workspace tests pass; physical display and Rust_Boy button behavior remain unverified |
| **G10** | Audiophile-only admission and AIFF/AIFF-C | `codec_engineer` | partial evidence | Local offline gate passes on Rust 1.99.1/macOS and seven AIFF/AIFF-C native PCM vectors pass; physical DAC and host integration remain open |
| **G11** | ALAC decoder and M4A/CAF demultiplexers | `codec_engineer` | partial evidence | Raw packet parity, six Internet Archive M4A ALAC parity cases, AAC denial, and five generated CAF ALAC/LPCM vectors pass; broader encoders, real-world CAF, and host/output acceptance remain |
| **G12** | RF64 and WavPack lossless extensions | `codec_engineer` | partial evidence | Bounded RF64/ds64 and large-size synthetic streaming pass. WavPack's bounded decoder passes generated FFmpeg PCM vectors and independent Tiny Encoder 4.40 parity for selected packed-16/24-bit profiles, including a 1,063,147-frame deterministic synthetic 24-bit stress vector. Native real-world WavPack files, broader profile/encoder coverage, trailing tags, hybrid correction and public playback admission remain open |
| **G13** | Bounded metadata and sample-accurate seek | `codec_engineer` | partial evidence | Bounded FLAC/AIFF tags, synthetic seek tests, and PCM-exact seeks at every populated SEEKTABLE point in six private FLAC streams pass; other encoders and host/hardware evidence remain |
| **G14** | Windows WASAPI and Android AAudio ports | `audio_backend` | planned | Platform-local compile, focus and physical output acceptance needed |
| **G15** | Desktop micro-library browsing and playback controls | `host_integrator` | partial evidence | Explicit text-only library/search UI, persistent queue and playlist controls, explicit Play/Stop, selection-only Next/Previous, and local app bundle validation pass; host focus workflow, Pause/volume, autoplay setting, and EQ remain |

## Codex task queue

| Task | Priority | Gate | Owner | State | Prerequisite |
| --- | --- | --- | --- | --- | --- |
| `RM-001` — Rerun full offline Rust quality gate after formatter repair | P0 | G10 | `gate_verifier` | done | None |
| `RM-002` — Replay six private FLACs through native Rust decoder | P0 | G8 | `gate_verifier` | done | RM-001 |
| `RM-003` — Prove native RIFF/WAVE decoded PCM roundtrip | P0 | G7 | `gate_verifier` | done | RM-001 |
| `RM-004` — Verify lossless-only floor and focus denial through Rust_Boy | P0 | G10 | `host_integrator` | done | RM-001 |
| `RM-005` — Qualify macOS AudioQueue output with physical DAC | P1 | G3 | `audio_backend` | blocked | RM-001 (done); physical DAC required |
| `RM-006` — Integrate independent MUSIC socket with Radio and TV ownership | P1 | G5 | `host_integrator` | partial_evidence | RM-004 |
| `RM-007` — Validate fixed 1KiB panel on actual Rust_Boy display | P1 | G6 | `host_integrator` | blocked | RM-006 |
| `RM-008` — Develop independent ALAC bitstream and PCM decoder | P1 | G11 | `codec_engineer` | done | RM-001 |
| `RM-009` — Implement M4A/MP4 ALAC-only demux and AAC denial | P1 | G11 | `codec_engineer` | done | RM-008 |
| `RM-010` — Implement CAF ALAC and integer LPCM demux | P2 | G11 | `codec_engineer` | done | RM-008 |
| `RM-011` — Add bounded local metadata without cover-art memory spikes | P2 | G13 | `codec_engineer` | done | RM-001 |
| `RM-012` — Implement validated sample-accurate FLAC seek | P2 | G13 | `codec_engineer` | done | RM-002 |
| `RM-013` — Implement RF64 large PCM files with ds64 limits | P2 | G12 | `codec_engineer` | done | RM-003 |
| `RM-014` — Implement WavPack only in verified lossless modes | P2 | G12 | `codec_engineer` | in progress | RM-001 |
| `RM-015` — Port native output to Windows and Android without codec dependencies | P2 | G14 | `audio_backend` | blocked | RM-005, RM-006 |
| `RM-016` — Keep code, literature, agent roles and format registry indexed | P1 | G0 | `index_curator` | done | None |
| `RM-017` — Index M4A/MP4 in library and validate Archive.org ALAC corpus | P1 | G11 | `codec_engineer` | done | RM-009 |
| `RM-018` — Define bounded Pip-Boy package-process protocol | P1 | G5 | `host_integrator` | in_progress | RM-006 |
| `RM-019` — Qualify standalone macOS app lifecycle and package | P1 | G5 | `host_integrator` | done | RM-001 |
| `RM-020` — Build zero-artwork micro media library experience | P1 | G13 | `host_integrator` | in_progress | RM-019 |

**Priority order:** RM-008 raw ALAC parity, RM-009 bounded M4A demux/AAC denial, RM-010 CAF ALAC/LPCM demux, RM-017 M4A catalog ingestion plus six licensed upstream ALAC files, RM-011 bounded text tags, and RM-012 FLAC seek acceptance pass their recorded criteria. Six supplied real-world streams now pass exact range parity at every populated SEEKTABLE point and the following sample; broader encoder coverage remains open. Next decoder evidence is ALAC parity across independent sources/encoders and real-world CAF coverage. Physical output, Rust_Boy shared audio focus, and display acceptance remain separate production gates; do not infer those from host mocks or source review. Each Pip-Boy module retains its own module identity, package/registration record, and lifecycle while using the shared container.

## Acceptance ladder (no skipped proofs)

1. **Source:** code, locally-authored dependencies, bounded parser and tests exist.
2. **Compiled:** Rust fmt, Clippy, workspace tests and release build actually exit successfully on a defined toolchain.
3. **PCM-parity:** own decoder reproduces independent lossless PCM for named fixtures, rejects corruption and records counts/checksums.
4. **Host:** Rust_Boy focus, module unloading and 1024-byte screen work together without Radio/TV conflict.
5. **Hardware:** exact format/rate and loopback/measurement show device behavior, including errors, underruns and stop/drain. Only this level can justify hardware bit-perfect wording.

Each task closes only when its `acceptance` line has observed evidence. Record status changes in the machine index and update the human ledger. A reported success on a reference oracle does not establish our decoder passed.

## Current evidence snapshot — 2026-10-31

- **Offline gate:** Local `sh scripts/offline-gate.sh` exited 0 on Rust/Cargo 1.99.1 and macOS: 17 local crates, 32 path-only edges, zero registry/Git crates, zero third-party system links and one Apple OS framework link. Rustfmt, warnings-denied Clippy, workspace tests, release build and macOS feature check passed on the recorded worktree. This is build/test evidence, not physical audio acceptance. [G10 evidence](G10_AUDIOPHILE_FORMATS.md).
- **AIFF/AIFF-C:** Local native CLI PCM differential tests passed 7/7, including signed 24-bit/192 kHz, 32-bit, little-endian AIFF-C and malformed-input rejection; this does not certify audio hardware. [G10 evidence](G10_AUDIOPHILE_FORMATS.md).
- **FLAC:** Six user-provided private FLAC files passed reference FLAC/FFmpeg parity and native Rust full-PCM hash/byte-count parity. Do not commit audio. [G8 ledger](G8_REALWORLD_FLAC_CORPUS.md).
- **WAVE:** Local native CLI PCM parity passed 6/6, with malformed RIFF boundary, missing pad and nonzero precision-padding rejection. Physical device output remains untested. [G7 ledger](G7_RIFF_CONFORMANCE.md).
- **CAF:** Local FFmpeg-generated CAF parity passed 5/5 exact PCM vectors: signed LPCM 16/24/32-bit and ALAC 16/24-bit. FFmpeg is test-vector tooling only; custom channel layouts and unsupported codecs fail closed. Real-world CAF corpus and hardware remain open. [G10 evidence](G10_AUDIOPHILE_FORMATS.md).
- **M4A/MP4:** Six Internet Archive ALAC M4A files from one CC0-labeled collection pass exact PCM parity: five 16-bit/44.1 kHz tracks and one 24-bit/48 kHz track. Three additional Archive.org music M4As (one CC0-labeled sample and two tracks from distinct CC BY/CC BY-SA albums) plus one generated M4A are verified AAC and rejected. The optional script checks hashes, codec identity, catalog ingestion and the `.mp4a` alias. Archive license labels are uploader-supplied. Independent ALAC source/encoder, 20/32-bit and mono M4A parity, real-world CAF, and broad ALAC admission remain open. [Corpus provenance and coverage needs](../crates/rust-music-mp4/tests/fixtures/upstream/README.md).
- **Host and hardware:** The bounded IPC crate and endpoint implement RBPK v1, with process lifecycle coverage; this Rust Music worktree does not mount the package in a live Pip-Boy host. NullSink and pure render paths are not actual shared focus, screen wiring or DAC tests. Rust Music exposes a distinct stable package identity and suspend lifecycle; Pip-Boy's reference registry includes MUSIC as a distinct, explicitly uninstalled package. A trusted runner, shared focus, host display integration and device acceptance remain unverified. [Package-process contract](PACKAGE_PROCESS.md). [Pip-Boy adapter contract](PIP_BOY_ADAPTER.md). [Host contract](HOST_CONTRACT.md).

## Next expansion

**G11:** Locally authored ALAC, bounded M4A/MP4 and CAF demultiplexers, AAC/DRM denial, and M4A catalog ingestion are implemented. Six Archive.org M4A ALAC files from one source and generated cases pass exact PCM parity. Four real/generated AAC cases are rejected. Independent ALAC encoder/source, supported 20/32-bit and mono M4A parity, and real-world CAF coverage are still required before broad ALAC admission.
**G12:** RF64/ds64 is implemented with bounded synthetic large-file evidence. WavPack has exact PCM parity and decoded-header checksum validation on generated packed-16-bit mono/joint-stereo one-block fixtures, five-block mono, mixed-mode and all-true-stereo fixtures, deterministic pseudorandom stereo fixtures with a joint-to-true-stereo transition, and padded-24-bit mono/stereo `INT32_INFO=[0,8,0,0]` profiles, including a high-entropy signed-24-bit stereo vector. An independent WavPack Tiny Encoder 4.40 implementation produces exact packed-16/24-bit parity for high-entropy stereo sources using supported fast-mode terms, plus exact packed-16-bit default-, high- and very-high-quality-mode parity on 20,000 stereo frames (the high-mode profile exercises terms `[18,18,18,-2,2,3,5,-1,17,4]`), plus exact 1,063,147-frame parity on deterministic synthetic packed-24-bit PCM. Positive-term adaptive weights are not clamped at ±1024; this correction closes the previously observed one-unit mismatch in the second mixed-mode block. It also has adjusted-binary vectors, adaptive median zones, and fail-closed WavPack 5 encoded-block checksum validation for 16/32-bit checksum metadata. Native WavPack real-world source files, additional encoder parity, other sample profiles, trailing tags, and hybrid correction support remain open; WavPack is still not admitted by the facade.
**G13:** Safe bounded metadata and accurate frame seek, without embedded artwork exhausting the micro UI.  
**G14:** Independent WASAPI and AAudio device adapters with real target qualification.
**RM-020 / ML-00–ML-07:** The catalog has stable canonical-root IDs, bounded text/container filtering, deterministic sorting/pagination, a cancel-aware scan API, bounded M3U8 editing, Unix-persistent queue state with repeat/seeded shuffle, and named-playlist CRUD with bounded versioned `.rmp` persistence. The standalone macOS UI connects explicit folder load/search, queue and named playlists, explicit Play/Stop, and selection-only Previous/Next. At `2546cae`, CLI persistence tests passed 12/12 in the full suite, the full `sh scripts/offline-gate.sh` exited 0, and `sh scripts/build-macos-app.sh` exited 0 with local ad-hoc signature verification. Roadmap and literature checks exited 0. The initial 0/1/100/2k/20k scan/search profile is recorded. Remaining work includes multiple roots/reconciliation, host playback/focus workflow, Pause/volume, optional explicit-load autoplay and its focus tests, native EQ, cold startup and incremental-memory profiles. All slices follow [the zero-artwork policy](MICRO_LIBRARY_ZERO_ARTWORK.md).

Expanded lossless **G4** remains the umbrella milestone for G10–G13, not a claim that ALAC or WavPack is currently supported.

## How Codex uses this map

Start with [AGENTS.md](../AGENTS.md), then [agent routing](CODEX_AGENT_MAP.md) and the selected `RM-###` entry in [roadmap-index.json](roadmap-index.json). Use one code-writing agent per overlapping scope; `gate_verifier` and `safety_reviewer` are read-only. The project-scoped `.codex/agents/*.toml` definitions are optional Codex tooling, not runtime player dependencies.

To validate links, agent names, IDs, statuses, dependency DAG and format-policy alignment without third-party Python packages:

```sh
python3 scripts/check_roadmap_index.py
python3 scripts/check_literature.py
sh scripts/offline-gate.sh
```

The first two are optional maintenance validators. The last is the authoritative Rust-only release gate and must not imply a PASS until it actually completes.

## Portable library and Pip-Boy socket — 2026-10-31 roadmap update

The approved visual direction is [PRIMARY_UI_DIRECTION](design/PRIMARY_UI_DIRECTION.md); the portable socket contract is [PIPBOY_SOCKET_SCHEMA](design/PIPBOY_SOCKET_SCHEMA.md), and the desktop-to-device export protocol is [PORTABLE_LIBRARY_DEPLOYMENT](design/PORTABLE_LIBRARY_DEPLOYMENT.md). These specifications do **not** certify source implementation.

**Delivery dependency:** finish the outstanding RM-020/macOS v1.0 playback and audio-focus work first. Parallel read-only socket/manifest inventory is acceptable; do not conflate standalone Mac release and live Pip-Boy hardware certification.

| Task | Priority | Route | Deliverable |
| --- | --- | --- | --- |
| RM-021 | P0 | host_integrator | Existing library, IPC and RBPK contract inventory |
| RM-022 | P1 | host_integrator | Versioned bounded portable manifest, stable ID and sort fixtures |
| RM-023 | P1 | host_integrator | Atomic portable export/import and validated incremental plan |
| RM-024 | P1 | host_integrator | macOS deploy-to-device flow |
| RM-025 | P1 | host_integrator | Pip-Boy socket consumption with host-owned audio focus |
| RM-026 | P2 | host_integrator initially; split platform ownership later | Linux and Windows desktop adapters |
| RM-027 | P2 | host_integrator + read-only gate_verifier | Physical Pi/Pip-Boy and removable-media acceptance |

All newly registered tasks remain **planned** pending code and observed tests. Do not ship ornamental image/skin assets from the approved reference while the current text-only/zero-artwork release policy remains in force. A future visual policy revision requires a separate gated decision. Update `docs/roadmap-index.json` together with this section; verify using the existing index checker before declaring the registry valid.
