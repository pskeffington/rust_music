# Rust Music — repository and machine navigation

This is the **entrypoint for agents and operators**. Use the smallest relevant section instead of loading all documentation at once.

The crate table below indexes product source by ownership. The asset inventory
at the end indexes repository instructions, documents, scripts, tests and CI.
`git ls-files` remains the exact tracked-file listing; update this navigation
index whenever a crate, document, script, test or workflow is added or removed.

## Authority by question

- **What to build next, which gate is blocked, who owns it?** [Roadmap JSON](roadmap-index.json), [human roadmap](ROADMAP.md), [Codex agent map](CODEX_AGENT_MAP.md).
- **What may play, what is only planned?** [Lossless format JSON](lossless-formats.json), [audiophile format policy](G10_AUDIOPHILE_FORMATS.md).
- **Why these algorithms and standards?** [Literature registry](literature-index.json), [independent literature matrix](LITERATURE_MATRIX.md).
- **May this crate depend on a library or call network?** [Offline dependency policy](DEPENDENCY_POLICY.md), [offline gate](../scripts/offline-gate.sh).
- **How does MUSIC pop into Rust_Boy/Pip-Boy?** [Host contract](HOST_CONTRACT.md), [Pip-Boy adapter contract](PIP_BOY_ADAPTER.md), and [1KiB panel](G6_MICRO_PANEL.md).
- **What is actually verified?** Per-gate ledgers [G2](G2_VALIDATION.md), [G5](G5_VALIDATION.md), [G6](G6_VALIDATION.md), [G7](G7_RIFF_CONFORMANCE.md), [G8](G8_REALWORLD_FLAC_CORPUS.md), [G9](G9_TIMING_PROGRESS.md), [G10](G10_AUDIOPHILE_FORMATS.md).
- **Which Codex guidance loads?** Root [AGENTS.md](../AGENTS.md) and `.codex/agents/` definitions documented in [CODEX_AGENT_MAP.md](CODEX_AGENT_MAP.md).
- **What changed and what was verified?** [Work log](WORK_LOG.md); task status remains authoritative in [roadmap JSON](roadmap-index.json).

## Crate ownership

| Area | Repository entry | Owning agent | Gate / evidence |
| --- | --- | --- | --- |
| PCM types, sink contracts | `crates/rust-music-core/` | `codec_engineer` | G1; G10 floor |
| FLAC decoder and MD5 | `crates/rust-music-flac/` | `codec_engineer` | G2 / G8 |
| WAVE integer PCM and RIFF | `crates/rust-music-wave/` | `codec_engineer` | G7 |
| AIFF/AIFF-C signed PCM | `crates/rust-music-aiff/` | `codec_engineer` | G10 |
| ALAC raw-packet decoder and synthetic vectors | `crates/rust-music-alac/`, `crates/rust-music-alac/tests/fixtures/` | `codec_engineer` | G11 (16/24/32-bit plus stereo-matrix parity; not admitted; malformed coverage and demux remain) |
| M4A/MP4 ALAC-only demux and synthetic files | `crates/rust-music-mp4/`, `crates/rust-music-mp4/tests/fixtures/` | `codec_engineer` | G11 / RM-009 (bounded profile; generated 24-bit parity; AAC/DRM denial; broader corpus and CAF remain) |
| Playback state/progress | `crates/rust-music-engine/` | `codec_engineer` | G1 / G9 |
| OS DAC adapters | `crates/rust-music-native/` | `audio_backend` | G3 |
| Public facade, sniffing | `crates/rust-music/` | serial handoff | G1 / G10 |
| CLI / acceptance harness | `crates/rust-music-cli/` | serial handoff | G7 / G8 / G10 |
| Workspace dependency auditor | `crates/rust-music-audit/` | `gate_verifier` reviews | G0 |
| Directory scanner | `crates/rust-music-library/` | `host_integrator` | G5 |
| Music socket and audio lease | `crates/rust-music-host/` | `host_integrator` | G5 |
| Fixed 1 KiB screen | `crates/rust-music-panel/` | `host_integrator` | G6 / G9 |
| Roadmap and Codex role index | `docs/roadmap-index.json` | `index_curator` | G0 |
| Independent literature registry | `docs/literature-index.json` | `index_curator` | G0 |
| Supported/denied format registry | `docs/lossless-formats.json` | serial handoff | G10 / G11 |

## Instructions, documents and supporting files

| Area | Indexed paths | Purpose |
| --- | --- | --- |
| Repository instructions and Codex roles | `AGENTS.md`, `.codex/config.toml`, `.codex/agents/*.toml`, `docs/CODEX_AGENT_MAP.md`, `docs/CODEX_PRODUCTION_PLAYBOOK.md` | Build constraints, task routing, ownership and release procedure |
| Architecture and host integration | `docs/ARCHITECTURE.md`, `docs/HOST_CONTRACT.md`, `docs/PIP_BOY_ADAPTER.md`, `docs/G6_MICRO_PANEL.md` | Crate boundaries, host lease, Pip-Boy adapter gaps and panel contract |
| Roadmap, progress and this index | `docs/ROADMAP.md`, `docs/roadmap-index.json`, `docs/REPOSITORY_INDEX.md`, `docs/WORK_LOG.md` | Stable task state, gate order, dated evidence and file navigation |
| Format and dependency policy | `docs/G10_AUDIOPHILE_FORMATS.md`, `docs/DEPENDENCY_POLICY.md`, `docs/lossless-formats.json` | Playback admission, format status and offline dependency rules |
| Validation ledgers | `docs/G2_VALIDATION.md`, `docs/G5_VALIDATION.md`, `docs/G6_VALIDATION.md`, `docs/G7_RIFF_CONFORMANCE.md`, `docs/G8_EXTERNAL_FLAC_CORPUS.md`, `docs/G8_REALWORLD_FLAC_CORPUS.md`, `docs/G9_TIMING_PROGRESS.md` | Per-gate commands, results and outstanding acceptance |
| Literature | `docs/literature-index.json`, `docs/LITERATURE_INDEX.md`, `docs/LITERATURE_MATRIX.md` | Source registry, generated index and human rationale |
| Root files and CI configuration | `README.md`, `Cargo.toml`, `Cargo.lock`, `.cargo/config.toml`, `.github/workflows/ci.yml`, `.gitignore`, `LICENSE` | User entrypoint, workspace, offline mode, operator-owned CI, exclusions and license |
| Scripts | `scripts/offline-gate.sh`, `scripts/build_literature.py`, `scripts/check_literature.py`, `scripts/check_roadmap_index.py`, `scripts/test_*` | Required offline gate and optional developer validation/reference harnesses |
| Tests and corpus metadata | `tests/test_roadmap_index.py`, `tests/corpus/guiezio-flac-sha256.json` | Index regression tests and hashes-only private-audio manifest; source recordings stay outside Git |

## Offline checks

```sh
python3 scripts/check_roadmap_index.py
python3 scripts/check_literature.py
sh scripts/offline-gate.sh
python3 scripts/test_aiff_reference.py
python3 scripts/test_wave_reference.py
python3 scripts/test_flac_user_corpus.py --mode rust --require-complete "$PRIVATE_FLAC_DIRECTORY"
```

The Python scripts are developer-only test/reference tools using the stdlib (or existing local reference binaries when explicitly stated), not runtime product dependencies. The mandatory Rust release gate requires only the preinstalled Rust toolchain and OS APIs.

## Truth and privacy

The six user-supplied private FLACs must remain private; only pinned hashes, corpus descriptions and test results belong in Git. No performance/bit-perfect claim without the matching host/hardware evidence. No unstated installation, scheduled background run or automatic merge.
