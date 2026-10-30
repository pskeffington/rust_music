# RM-001 operator gate and failure-routing runbook — 2026-10-30

Status: **needs operator execution; not certified**. Source: [offline gate](../scripts/offline-gate.sh), [CI workflow](../.github/workflows/ci.yml), [roadmap](ROADMAP.md), [Codex agents](CODEX_AGENT_MAP.md).

## What CI actually does

The repository workflow is **manual** (`workflow_dispatch`) and selects a self-hosted runner labeled `rust-music-offline`. It assumes an operator has already provisioned a trustworthy local checkout of the exact workflow SHA and an offline Rust toolchain, and configured `RUST_MUSIC_SOURCE_ROOT`. It does **not** call `actions/checkout`, use a marketplace action, fetch packages, or schedule ordinary PR jobs. Historical `startup_failure` runs are not evidence of a Rust compiler failure. Check the job log / runner registration rather than weakening the no-third-party policy.

## Operator sequence (run from a clean working tree at PR #1 HEAD)

```sh
git status --short --branch
git rev-parse HEAD
rustc --version
cargo --version
python3 scripts/check_roadmap_index.py
python3 scripts/check_literature.py
sh scripts/capture_offline_gate.sh
```

The capture wrapper writes a private transcript and machine-readable summary to an explicit directory **outside Git**. It resolves repository/evidence roots physically and refuses to run if Git cannot verify the checkout or if the tree is dirty. It reports the actual exit code and does not mark any roadmap status automatically. It requires a local Rust toolchain, makes no installs, and uses the authoritative `scripts/offline-gate.sh` without modifying it. For a deliberate evidence location:

```sh
RUST_MUSIC_EVIDENCE_DIR="$HOME/rust-music-gate-evidence" sh scripts/capture_offline_gate.sh
```

If the script reports `BLOCKED` and exit 3, obtain a preinstalled offline toolchain through operator-approved channels and rerun; **do not** add a downloaded action, crate, codec, or shell installer. If another command fails, the exit status is preserved; fix the first concrete diagnostic and rerun the **whole** gate.

## Failure routing

| Failure | Responsible agent | Correct action |
| --- | --- | --- |
| Missing cargo/rustfmt/Clippy | `gate_verifier` | Block with toolchain details; operator provisions offline toolchain |
| Workspace/lock/dependency audit | `safety_reviewer` + coordinator | Review local-only path graph and FFI; no downloaded replacement |
| Rustfmt or Clippy diagnostic | Owner of changed crate | Minimal source repair, native tests and full gate rerun |
| FLAC/WAV/AIFF decode unit failure | `codec_engineer` | Reproduce failing vector, add bounded regression, review integrity |
| Native macOS/ALSA compile | `audio_backend` | Target-specific ABI check, preserve no external codec policy |
| Host lease/panel unit failure | `host_integrator` | Reproduce before fixes; test early denial and no sound callback work |
| GitHub workflow startup failure/no jobs | Coordinator / operator | Inspect runner labels, offline registration, exact SHA and jobs; do not conflate with code failure |

## Evidence acceptance rules

- Only a real exit-0 `sh scripts/offline-gate.sh` at the recorded commit and toolchain can satisfy **RM-001 build evidence**.
- The wrapper's own successful completion means a transcript was captured, **not** that Rust passed. Its exit status matches the gate command.
- `RM-002` requires native decoder comparison on the private six-track corpus; `RM-003` native WAVE differential tests; `RM-004` real focus-policy tests. Those remain distinct from RM-001.
- Every new status should be reconciled with `docs/roadmap-index.json` and `docs/ROADMAP.md` by `index_curator` only once the evidence is reviewed.
- Retain PR #1 as draft, keep user media/private paths out of public commits, and never describe integer PCM parity as measured physical DAC bit perfection.
