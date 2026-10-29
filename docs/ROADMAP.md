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
| **G0** | Ownership, standards and offline audit | `index_curator` | partial evidence | Dependency audit 12 local crates/21 edges passed; full Rust gate recheck pending |
| **G1** | Integer PCM core and explicit playback state | `codec_engineer` | source implemented | Unit tests and output behavior require full offline gate |
| **G2** | Native FLAC with CRC, Rice/LPC, PCM MD5 | `codec_engineer` | partial evidence | Source present; own-decoder real-corpus parity not reported |
| **G3** | Mac AudioQueue / Linux kernel PCM output | `audio_backend` | source implemented | Source present; DAC format, drain and underrun measurement pending |
| **G4** | Expanded lossless umbrella | `codec_engineer` | planned | Implemented AIFF is G10; ALAC and later formats are G11/G12 work |
| **G5** | Rust_Boy socket and shared audio-focus | `host_integrator` | source implemented | Unit slot exists; real Radio/TV integration and field gate pending |
| **G6** | 128x64 monochrome 1KiB panel | `host_integrator` | source implemented | Renderer exists; hardware screen/controls pending |
| **G7** | Strict RIFF/WAVE PCM and extensible WAV | `codec_engineer` | partial evidence | Six reference fixtures; native full workspace gate/parity pending |
| **G8** | Six-file private FLAC differential corpus | `gate_verifier` | partial evidence | Reference decoder 6/6; Rust decoder parity on those six files pending |
| **G9** | Exact declared-frame timing and display progress | `host_integrator` | source implemented | Source present; full gate and physical display behavior pending |
| **G10** | Audiophile-only admission and AIFF/AIFF-C | `codec_engineer` | partial evidence | Operator-reported 7/7 native CLI cases PASS; full gate stopped at rustfmt; repair needs retest |
| **G11** | ALAC decoder and M4A/CAF demultiplexers | `codec_engineer` | planned | Independent ALAC decode golden PCM and strict AAC rejection needed |
| **G12** | RF64 and WavPack lossless extensions | `codec_engineer` | planned | Per-format independent parity and malicious-input rejection needed |
| **G13** | Bounded metadata and sample-accurate seek | `codec_engineer` | planned | Never claim seek or album tags without indexed code and tests |
| **G14** | Windows WASAPI and Android AAudio ports | `audio_backend` | planned | Platform-local compile, focus and physical output acceptance needed |

## Codex task queue

| Task | Priority | Gate | Owner | State | Prerequisite |
| --- | --- | --- | --- | --- | --- |
| `RM-001` — Rerun full offline Rust quality gate after formatter repair | P0 | G10 | `gate_verifier` | needs operator | None |
| `RM-002` — Replay six private FLACs through native Rust decoder | P0 | G8 | `gate_verifier` | blocked | RM-001 |
| `RM-003` — Prove native RIFF/WAVE decoded PCM roundtrip | P0 | G7 | `gate_verifier` | blocked | RM-001 |
| `RM-004` — Verify lossless-only floor and focus denial through Rust_Boy | P0 | G10 | `host_integrator` | blocked | RM-001 |
| `RM-005` — Qualify macOS AudioQueue output with physical DAC | P1 | G3 | `audio_backend` | blocked | RM-001 |
| `RM-006` — Integrate independent MUSIC socket with Radio and TV ownership | P1 | G5 | `host_integrator` | blocked | RM-004 |
| `RM-007` — Validate fixed 1KiB panel on actual Rust_Boy display | P1 | G6 | `host_integrator` | blocked | RM-006 |
| `RM-008` — Develop independent ALAC bitstream and PCM decoder | P1 | G11 | `codec_engineer` | blocked | RM-001 |
| `RM-009` — Implement M4A/MP4 ALAC-only demux and AAC denial | P1 | G11 | `codec_engineer` | blocked | RM-008 |
| `RM-010` — Implement CAF ALAC and integer LPCM demux | P2 | G11 | `codec_engineer` | blocked | RM-008 |
| `RM-011` — Add bounded local metadata without cover-art memory spikes | P2 | G13 | `codec_engineer` | blocked | RM-001 |
| `RM-012` — Implement validated sample-accurate FLAC seek | P2 | G13 | `codec_engineer` | blocked | RM-002 |
| `RM-013` — Implement RF64 large PCM files with ds64 limits | P2 | G12 | `codec_engineer` | blocked | RM-003 |
| `RM-014` — Implement WavPack only in verified lossless modes | P2 | G12 | `codec_engineer` | blocked | RM-001 |
| `RM-015` — Port native output to Windows and Android without codec dependencies | P2 | G14 | `audio_backend` | blocked | RM-005, RM-006 |
| `RM-016` — Keep code, literature, agent roles and format registry indexed | P1 | G0 | `index_curator` | planned | None |

**Priority order:** Complete the fresh G10 offline gate first (`RM-001`). While it is blocked, read-only review can proceed, but dependent implementation and certification tasks must not be promoted to verified. After it passes, qualify the private real-world FLACs (`RM-002`), WAVE parity (`RM-003`) and fail-closed playback/focus (`RM-004`) before calling the baseline complete.

## Acceptance ladder (no skipped proofs)

1. **Source:** code, locally-authored dependencies, bounded parser and tests exist.
2. **Compiled:** Rust fmt, Clippy, workspace tests and release build actually exit successfully on a defined toolchain.
3. **PCM-parity:** own decoder reproduces independent lossless PCM for named fixtures, rejects corruption and records counts/checksums.
4. **Host:** Rust_Boy focus, module unloading and 1024-byte screen work together without Radio/TV conflict.
5. **Hardware:** exact format/rate and loopback/measurement show device behavior, including errors, underruns and stop/drain. Only this level can justify hardware bit-perfect wording.

Each task closes only when its `acceptance` line has observed evidence. Record status changes in the machine index and update the human ledger. A reported success on a reference oracle does not establish our decoder passed.

## Current evidence snapshot — 2026-10-29

- **Audit:** Operator-run `rust-music-audit` passed **12 local crates, 21 internal edges, no registry/Git crates, no third-party system libraries and one approved OS framework**. The subsequent full gate stopped at rustfmt. Exact formatter diffs were repaired in four commits through `13f8e85975f7b4cae921593df2ab0e7f310b8d30`; no fresh full-gate PASS has been supplied.
- **AIFF/AIFF-C:** Seven native CLI PCM differential tests passed on the operator's Mac (including signed 24-bit/192 kHz, 32-bit, little-endian AIFF-C and malformed inputs); this does not certify audio hardware. [G10 evidence](G10_AUDIOPHILE_FORMATS.md).
- **FLAC:** Six user-provided private FLAC files passed reference-decoder parity; native Rust replay on those files remains open. Do not commit audio. [G8 ledger](G8_REALWORLD_FLAC_CORPUS.md).
- **WAVE:** Six deterministic reference cases previously passed; compiled full workspace parity remains open. [G7 ledger](G7_RIFF_CONFORMANCE.md).
- **Host and hardware:** NullSink and pure render paths are not actual shared focus, screen wiring or DAC tests. [Host contract](HOST_CONTRACT.md).

## Next expansion

**G11:** Locally authored ALAC, strict ALAC-only M4A/MP4 codec inspection, and CAF ALAC/lpcm. Reject AAC in a .m4a shell.  
**G12:** WavPack pure-lossless/hybrid-with-correction, RF64/ds64 and verified large PCM.  
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
