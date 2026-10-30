# Rust Music — repository and machine navigation

This is the **entrypoint for agents and operators**. Use the smallest relevant section instead of loading all documentation at once.

## Authority by question

- **What to build next, which gate is blocked, who owns it?** [Roadmap JSON](roadmap-index.json), [human roadmap](ROADMAP.md), [Codex agent map](CODEX_AGENT_MAP.md).
- **What may play, what is only planned?** [Lossless format JSON](lossless-formats.json), [audiophile format policy](G10_AUDIOPHILE_FORMATS.md).
- **Why these algorithms and standards?** [Literature registry](literature-index.json), [independent literature matrix](LITERATURE_MATRIX.md).
- **May this crate depend on a library or call network?** [Offline dependency policy](DEPENDENCY_POLICY.md), [offline gate](../scripts/offline-gate.sh).
- **How does MUSIC pop into Rust_Boy?** [Host contract](HOST_CONTRACT.md) and [1KiB panel](G6_MICRO_PANEL.md).
- **What is actually verified?** Per-gate ledgers [G2](G2_VALIDATION.md), [G5](G5_VALIDATION.md), [G6](G6_VALIDATION.md), [G7](G7_RIFF_CONFORMANCE.md), [G8](G8_REALWORLD_FLAC_CORPUS.md), [G9](G9_TIMING_PROGRESS.md), [G10](G10_AUDIOPHILE_FORMATS.md).
- **Which Codex guidance loads?** Root [AGENTS.md](../AGENTS.md) and `.codex/agents/` definitions documented in [CODEX_AGENT_MAP.md](CODEX_AGENT_MAP.md).

- **How does the operator capture a complete offline gate?** [RM-001 gate runbook](RM001_OPERATOR_GATE.md) and [evidence wrapper](../scripts/capture_offline_gate.sh).

## Crate ownership

| Area | Repository entry | Owning agent | Gate / evidence |
| --- | --- | --- | --- |
| PCM types, sink contracts | `crates/rust-music-core/` | `codec_engineer` | G1; G10 floor |
| FLAC decoder and MD5 | `crates/rust-music-flac/` | `codec_engineer` | G2 / G8 |
| WAVE integer PCM and RIFF | `crates/rust-music-wave/` | `codec_engineer` | G7 |
| AIFF/AIFF-C signed PCM | `crates/rust-music-aiff/` | `codec_engineer` | G10 |
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
