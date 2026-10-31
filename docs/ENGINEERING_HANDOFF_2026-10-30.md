# Engineering handoff and Codex continuation — 2026-10-30

> Historical handoff, superseded for branch/PR state by the 2026-10-31 integration record in [PR #1 merge readiness](PR1_MERGE_READINESS.md). PR #1 has merged and `main` contains the compileable macOS app update. This document remains useful for task history, but is not the current checkout or release certificate. Canonical task states and dependencies remain in [roadmap-index.json](roadmap-index.json).

## Historical repository inspection (2026-10-30)

- At the time of inspection, `main` contained only `README.md` and `LICENSE`. This has been superseded: PR #1 merged at `f8052f2`, and `main` was later verified at `e8e5ff8` with the product and app sources.
- The branch indexes 12 local Rust crates: `rust-music`, `rust-music-core`, `rust-music-flac`, `rust-music-wave`, `rust-music-aiff`, `rust-music-engine`, `rust-music-native`, `rust-music-host`, `rust-music-library`, `rust-music-panel`, `rust-music-cli`, `rust-music-audit`.
- Project instructions: [AGENTS.md](../AGENTS.md), [agent roles](CODEX_AGENT_MAP.md), [production playbook](CODEX_PRODUCTION_PLAYBOOK.md), [roadmap](ROADMAP.md), and [navigation index](REPOSITORY_INDEX.md).
- Operational note: PR #1 was still draft at inspection. Its metadata reported `mergeable=false`, which alone is not enough to diagnose merge conflicts; inspect GitHub's actual mergeability/CI diagnostics before attributing a cause.

## Evidence register — prevent false promotion

| Area | Evidence already reported | Still required |
| --- | --- | --- |
| Ownership | Operator audit reported 12 workspace crates and 21 local path edges, zero registry/Git packages | Fresh full offline gate, lockfile and FFI audit after any new commits |
| AIFF / AIFF-C | Operator-provided native CLI golden PCM output: 7/7 generated vectors PASS | Full workspace gate, more adversarial cross-platform coverage, physical output |
| WAVE | Six deterministic reference fixtures previously passed | Fresh native parity and workspace checks |
| FLAC | Independent reference checks, including six private tracks, reported | Run **our** decoder on all six privately; record exact PCM and digest comparisons; never commit tracks |
| Rust_Boy | Host socket, exclusive focus contract and 1 KiB panel have source implementations | Real integrated Radio/TV focus refusal/transfer, Settings and display acceptance |
| Native output | macOS/ALSA adapters implemented | Exact negotiated PCM/device evidence, underruns, drain/stop, loopback for bit-perfect claims |
| Formatting | Operator supplied rustfmt failure; repair series ending `13f8e85975f7b4cae921593df2ab0e7f310b8d30` was committed | Fresh `cargo fmt --all -- --check` and complete offline gate outcome |

**Do not turn a past operator report or static code review into a newly executed test.** Record command, environment, timestamp, exit code, fixture identifier and evidence source separately.

## Next Codex assignments

1. **RM-001 — gate_verifier (P0).** Read-only baseline: `git status --short --branch`, toolchain versions, `python3 scripts/check_roadmap_index.py`, `python3 scripts/check_literature.py`, then `sh scripts/offline-gate.sh`. Preserve the full log outside Git. If no Rust toolchain is present, mark blocked and provide commands for the operator; do not install packages. If a concrete failure occurs, route a minimal fix to the agent owning that file, rerun the entire gate and leave dependent tasks blocked.
2. **RM-002 — gate_verifier (P0, after RM-001).** Use `python3 scripts/test_flac_user_corpus.py --mode rust --require-complete /private/music` locally with authorized private corpus. Compare decoded samples, frames and independent oracle digests. Do not upload, log filenames unnecessarily, or commit the FLAC contents.
3. **RM-003 — gate_verifier (P0, after RM-001).** Execute `python3 scripts/test_wave_reference.py` with native CLI enabled and capture negative-fixture results.
4. **RM-004 — host_integrator with safety_reviewer (P0, after RM-001).** Verify quality-floor rejection before DAC and host audio lease, including spoofed extensions, unsupported codecs and repeated focus errors.
5. **RM-005 through RM-007 — audio_backend / host_integrator (P1).** Qualify native DAC then Rust_Boy/Radio/TV focus and the physical 128x64 panel. Explicitly record device details.
6. **RM-008 onward — codec_engineer (only after prerequisites).** ALAC decoder first; M4A/CAF admission only after independent PCM parity. RF64, WavPack, metadata, accurate seek and additional OS backends remain later work.

RM-016 index maintenance can proceed separately, but **must not** mutate task/gate status on source review alone. Update the canonical JSON and readable docs together for any observed status change.

## Agent delegation boundary

- **Coordinator:** select one indexed task, check preconditions, control shared Cargo/core/engine/facade and index writes, and own the final handoff.
- **gate_verifier:** evidence-only inspection; no source writes.
- **codec_engineer:** native decoder and format-specific corpus/negative-case work.
- **audio_backend:** native PCM output and measurements.
- **host_integrator:** Rust_Boy socket, library, panel and audio-focus boundaries.
- **index_curator:** docs, stable ID registry, cross-links and maintenance scripts.
- **safety_reviewer:** read-only adversarial review of limits, overflows, license/dependency and fail-closed gates.

Parallel read-only review is allowed. Limit concurrent agents to existing Codex configuration and **one writer per overlapping file scope**. Only the coordinator integrates shared files; do not cross-edit sibling repositories.

## Required completion report

```text
TASK / GATE:
BRANCH / HEAD / DIRTY FILES:
IMPLEMENTATION SCOPE:
COMMANDS / ENVIRONMENT / ACTUAL EXIT CODES:
FIXTURE IDS / EVIDENCE SOURCE:
SOURCE / COMPILED / PCM / HOST / HARDWARE STATUS:
NEW NEGATIVE TESTS:
SECURITY AND DEPENDENCY REVIEW:
BLOCKERS AND OPERATOR COMMAND:
REGISTRY AND DOCUMENTATION DIFF:
NEXT UNBLOCKED TASK:
PR STATUS (keep draft unless explicitly authorized):
```

## Release rule

No production support, bit-perfect hardware claim or automatic rollout follows from a formatter fix, reference fixture success, app build or source merge. Follow [production playbook](CODEX_PRODUCTION_PLAYBOOK.md) and the evidence ladder in [ROADMAP.md](ROADMAP.md).

## 2026-10-31 branch and issue audit addendum

See [branch and issue triage](BRANCH_AND_ISSUE_TRIAGE_2026-10-31.md) and [RM-001 tracker #2](https://github.com/pskeffington/rust_music/issues/2). At audit, feature was 70 commits ahead / 1 behind `main`, with a README-only main divergence and a GitHub `dirty` mergeability state. Reconcile ancestry without changing gate status. Historical Actions `startup_failure` is not successful validation. Keep PR #1 draft until the complete offline Rust gate, private native FLAC PCM parity, host focus and device acceptance are observed. Do not delete the implementation branch during this cleanup.
