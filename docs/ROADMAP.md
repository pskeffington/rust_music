# Rust Music — phased engineering roadmap

The **machine-readable task, gate and agent registry** is [roadmap-index.json](roadmap-index.json). Keep this file readable for operators; let the registry own stable RM task IDs, gate states, dependencies and Codex routing. Do not infer test results from a commit, source review or a filename.

## Non-negotiable release contract

- Playback accepts only verified **lossless** codecs and at least 16-bit / 44.1 kHz PCM. Lower-resolution decoder fixtures are permitted for tests, never playback.
- Own all runtime decoders and adapters; Cargo dependencies are workspace-local, zero remote registry/Git packages or bundled third-party codecs. Native OS/device APIs are explicit platform prerequisites.
- Verify exact decoded integer PCM independently of hardware. **Bit-perfect DAC** needs physical capture and exact device negotiation, not just FLAC-to-WAV parity.
- Keep Rust_Boy host-owned single audio focus, no autoplay or background scanning, and the 128×64, 1024-byte display budget.
- The offline Rust gate has no Python dependency; optional agent/index maintenance checks use Python stdlib only.

## Gate status and ownership

| Gate | Scope | Codex owner | Evidence state | Next acceptance |
| --- | --- | --- | --- | --- |
| **G0** | Ownership, standards and offline audit | `index_curator` | partial evidence | Offline dependency audit, fmt, Clippy, workspace tests, release build and macOS feature check pass locally on Rust 1.99.1; target hardware and host integration remain open |
| **G1** | Integer PCM core and explicit playback state | `codec_engineer` | source implemented | Workspace tests pass, including lossless playback floor and fail-closed output state; actual device behavior remains unverified |
| **G2** | Native FLAC with CRC, Rice/LPC, PCM MD5 | `codec_engineer` | partial evidence | Seven generated vectors and six private real-world tracks pass native PCM parity; wider codec/hardware claims remain separate |
| **G3** | Mac AudioQueue / Linux kernel PCM output | `audio_backend` | source implemented | Source present; DAC format, drain and underrun measurement pending |
| **G4** | Expanded lossless umbrella | `codec_engineer` | partial evidence | AIFF, generated M4A/ALAC, and bounded CAF profiles are implemented; broader ALAC admission remains |
| **G5** | Rust_Boy socket and shared audio-focus | `host_integrator` | partial evidence | Rust Music has stable `org.rustboy.music` identity and an explicit suspend/pop-out API preserving the mounted slot and selection; Pip-Boy registration/protocol bridge, worker scheduling, shared Radio/TV/Music arbitration and field acceptance remain open |
| **G6** | 128x64 monochrome 1KiB panel | `host_integrator` | source implemented | Renderer exists; hardware screen/controls pending |
| **G7** | Strict RIFF/WAVE PCM and extensible WAV | `codec_engineer` | partial evidence | Six native RIFF PCM cases and synthetic RF64 >4 GiB streaming/sample parity pass; actual large-file corpus and hardware output remain unverified |
| **G8** | Six-file private FLAC differential corpus | `gate_verifier` | partial evidence | Reference FLAC/FFmpeg and native Rust PCM hashes match on all six private files; artwork and hardware stress remain separate |
| **G9** | Exact declared-frame timing and display progress | `host_integrator` | source implemented | Workspace tests pass; physical display and Rust_Boy button behavior remain unverified |
| **G10** | Audiophile-only admission and AIFF/AIFF-C | `codec_engineer` | partial evidence | Local offline gate passes on Rust 1.99.1/macOS and seven AIFF/AIFF-C native PCM vectors pass; physical DAC and host integration remain open |
| **G11** | ALAC decoder and M4A/CAF demultiplexers | `codec_engineer` | partial evidence | Raw packet parity, four Internet Archive M4A ALAC parity cases, AAC denial, and five generated CAF ALAC/LPCM vectors pass; broader encoders, real-world CAF, and host/output acceptance remain |
| **G12** | RF64 and WavPack lossless extensions | `codec_engineer` | partial evidence | Bounded RF64/ds64 and large-size synthetic streaming pass; independent WavPack implementation and parity remain |
| **G13** | Bounded metadata and sample-accurate seek | `codec_engineer` | partial evidence | Bounded FLAC/AIFF tags and synthetic fixed/variable-block FLAC seek tests pass; real-world seektable and host/hardware evidence remain |
| **G14** | Windows WASAPI and Android AAudio ports | `audio_backend` | planned | Platform-local compile, focus and physical output acceptance needed |

## Codex task queue

| Task | Priority | Gate | Owner | State | Prerequisite |
| --- | --- | --- | --- | --- | --- |
| `RM-001` — Rerun full offline Rust quality gate after formatter repair | P0 | G10 | `gate_verifier` | done | None |
| `RM-002` — Replay six private FLACs through native Rust decoder | P0 | G8 | `gate_verifier` | done | RM-001 |
| `RM-003` — Prove native RIFF/WAVE decoded PCM roundtrip | P0 | G7 | `gate_verifier` | done | RM-001 |
| `RM-004` — Verify lossless-only floor and focus denial through Rust_Boy | P0 | G10 | `host_integrator` | done | RM-001 |
| `RM-005` — Qualify macOS AudioQueue output with physical DAC | P1 | G3 | `audio_backend` | blocked | RM-001 (done); physical DAC required |
| `RM-006` — Integrate independent MUSIC socket with Radio and TV ownership | P1 | G5 | `host_integrator` | blocked | RM-004 |
| `RM-007` — Validate fixed 1KiB panel on actual Rust_Boy display | P1 | G6 | `host_integrator` | blocked | RM-006 |
| `RM-008` — Develop independent ALAC bitstream and PCM decoder | P1 | G11 | `codec_engineer` | done | RM-001 |
| `RM-009` — Implement M4A/MP4 ALAC-only demux and AAC denial | P1 | G11 | `codec_engineer` | done | RM-008 |
| `RM-010` — Implement CAF ALAC and integer LPCM demux | P2 | G11 | `codec_engineer` | done | RM-008 |
| `RM-011` — Add bounded local metadata without cover-art memory spikes | P2 | G13 | `codec_engineer` | done | RM-001 |
| `RM-012` — Implement validated sample-accurate FLAC seek | P2 | G13 | `codec_engineer` | done | RM-002 |
| `RM-013` — Implement RF64 large PCM files with ds64 limits | P2 | G12 | `codec_engineer` | done | RM-003 |
| `RM-014` — Implement WavPack only in verified lossless modes | P2 | G12 | `codec_engineer` | planned | RM-001 |
| `RM-015` — Port native output to Windows and Android without codec dependencies | P2 | G14 | `audio_backend` | blocked | RM-005, RM-006 |
| `RM-016` — Keep code, literature, agent roles and format registry indexed | P1 | G0 | `index_curator` | done | None |
| `RM-017` — Index M4A/MP4 in library and validate Archive.org ALAC corpus | P1 | G11 | `codec_engineer` | done | RM-009 |

**Priority order:** RM-008 raw ALAC parity, RM-009 bounded M4A demux/AAC denial, RM-010 CAF ALAC/LPCM demux, RM-017 M4A catalog ingestion plus four licensed upstream ALAC files, RM-011 bounded text tags, and RM-012 synthetic FLAC seek acceptance pass their recorded criteria. The next decoder evidence is broader ALAC coverage across more independent sources/encoders and real-world FLAC seektable compatibility. Physical output, Rust_Boy shared audio focus, and display acceptance remain separate production gates; do not infer those from host mocks or source review. Each Pip-Boy module retains its own module identity, package/registration record, and lifecycle while using the shared container.

## Acceptance ladder (no skipped proofs)

1. **Source:** code, locally-authored dependencies, bounded parser and tests exist.
2. **Compiled:** Rust fmt, Clippy, workspace tests and release build actually exit successfully on a defined toolchain.
3. **PCM-parity:** own decoder reproduces independent lossless PCM for named fixtures, rejects corruption and records counts/checksums.
4. **Host:** Rust_Boy focus, module unloading and 1024-byte screen work together without Radio/TV conflict.
5. **Hardware:** exact format/rate and loopback/measurement show device behavior, including errors, underruns and stop/drain. Only this level can justify hardware bit-perfect wording.

Each task closes only when its `acceptance` line has observed evidence. Record status changes in the machine index and update the human ledger. A reported success on a reference oracle does not establish our decoder passed.

## Current evidence snapshot — 2026-10-29

- **Offline gate:** Local `sh scripts/offline-gate.sh` exited 0 on Rust/Cargo 1.99.1 and macOS: 15 local crates, 28 path-only edges, zero registry/Git crates and zero third-party system links. Rustfmt, warnings-denied Clippy, workspace tests, release build and macOS feature check passed. This is build/test evidence, not physical audio acceptance. [G10 evidence](G10_AUDIOPHILE_FORMATS.md).
- **AIFF/AIFF-C:** Local native CLI PCM differential tests passed 7/7, including signed 24-bit/192 kHz, 32-bit, little-endian AIFF-C and malformed-input rejection; this does not certify audio hardware. [G10 evidence](G10_AUDIOPHILE_FORMATS.md).
- **FLAC:** Six user-provided private FLAC files passed reference FLAC/FFmpeg parity and native Rust full-PCM hash/byte-count parity. Do not commit audio. [G8 ledger](G8_REALWORLD_FLAC_CORPUS.md).
- **WAVE:** Local native CLI PCM parity passed 6/6, with malformed RIFF boundary, missing pad and nonzero precision-padding rejection. Physical device output remains untested. [G7 ledger](G7_RIFF_CONFORMANCE.md).
- **CAF:** Local FFmpeg-generated CAF parity passed 5/5 exact PCM vectors: signed LPCM 16/24/32-bit and ALAC 16/24-bit. FFmpeg is test-vector tooling only; custom channel layouts and unsupported codecs fail closed. Real-world CAF corpus and hardware remain open. [G10 evidence](G10_AUDIOPHILE_FORMATS.md).
- **M4A/MP4:** Four Internet Archive ALAC M4A files from one CC0 collection passed exact PCM parity: three 16-bit/44.1 kHz tracks and one 24-bit/48 kHz track. The library indexes `.m4a` and `.mp4` `ftyp` containers; decoder-level AAC refusal remains. This is limited source/encoder coverage, not broad ALAC admission. [Corpus provenance](../crates/rust-music-mp4/tests/fixtures/upstream/README.md).
- **Host and hardware:** NullSink and pure render paths are not actual shared focus, screen wiring or DAC tests. Rust Music now exposes a distinct stable package identity and explicit suspend lifecycle; Pip-Boy registration, shared focus, and host input/display integration remain unverified. The current goal keeps Pip-Boy read-only; the [Pip-Boy adapter contract](PIP_BOY_ADAPTER.md) records the remaining integration work. [Host contract](HOST_CONTRACT.md).

## Next expansion

**G11:** Locally authored ALAC, bounded M4A/MP4 and CAF demultiplexers, AAC/DRM denial, and M4A catalog ingestion are implemented. Four licensed M4A ALAC files and generated cases pass exact PCM parity; broader independent encoder/source and real-world CAF coverage are still required before broad ALAC admission.
**G12:** RF64/ds64 is implemented with bounded synthetic large-file evidence; WavPack pure-lossless/hybrid-with-correction remains planned.
**G13:** Safe bounded metadata and accurate frame seek, without embedded artwork exhausting the micro UI.  
**G14:** Independent WASAPI and AAudio device adapters with real target qualification.

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
