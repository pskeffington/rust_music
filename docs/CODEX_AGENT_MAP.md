# Codex agent routing — Rust Music

Codex reads [AGENTS.md](../AGENTS.md) as root project instructions. Actual **project-scoped subagent definitions** live in `.codex/agents/*.toml` and are recognized by Codex versions supporting custom agents. [Codex project config](../.codex/config.toml) allows up to **three** concurrent spawned agents and intentionally pins no model, cloud provider, server or dependencies. These are development assistants, not part of the offline player. Custom-agent instructions do not create separate sandbox permissions.

The single source of machine-routing facts is [roadmap-index.json](roadmap-index.json); human gate progress is in [ROADMAP.md](ROADMAP.md).

## Role / file ownership

| Role | Codex file | Mode | Owned scope | Purpose |
| --- | --- | --- | --- | --- |
| `gate_verifier` | `.codex/agents/gate-verifier.toml` | read only | read-only | Offline gate and sample-parity evidence; no edits |
| `codec_engineer` | `.codex/agents/codec-engineer.toml` | write scoped | `crates/rust-music-flac/`, `crates/rust-music-wave/`, `crates/rust-music-aiff/`, `crates/rust-music-alac/`, approved future codec crates | Own independent codec/container decoding and rigorous PCM tests |
| `audio_backend` | `.codex/agents/audio-backend.toml` | write scoped | `crates/rust-music-native/` | OS PCM adapters and hardware-format acceptance |
| `host_integrator` | `.codex/agents/host-integrator.toml` | write scoped | `crates/rust-music-host/`, `crates/rust-music-library/`, `crates/rust-music-panel/` | Rust_Boy audio lease, bounded library and 1KiB interface |
| `index_curator` | `.codex/agents/index-curator.toml` | write scoped | `docs/ROADMAP.md`, `docs/CODEX_AGENT_MAP.md`, `docs/REPOSITORY_INDEX.md`, `docs/roadmap-index.json`, `scripts/check_roadmap_index.py` | Stable roadmap IDs, evidence routing, docs and JSON index |
| `safety_reviewer` | `.codex/agents/safety-reviewer.toml` | read only | read-only | Lossless policy and adversarial code/security review; no edits |

**Shared write boundaries:** `Cargo.toml`, `Cargo.lock`, `crates/rust-music/src/lib.rs`, `crates/rust-music-core/src/lib.rs`, `crates/rust-music-engine/src/lib.rs`, `docs/lossless-formats.json` and roadmap/index files require **serial, coordinated edits**. Parallelize independent exploration/review; never let two writing agents edit an overlapping tree.

## Assignment order

1. Inspect current branch, worktree and highest-priority requested `RM-###` task. Check the task's `status`, `depends_on`, `owner_agent`, `review_agent` and `focus_paths`. Do not declare a blocked dependency complete based on older evidence.
2. Assign one implementer or verifier to the scope. Delegated read-only evidence review may occur in parallel. The main Codex session owns shared-file integration and final evidence synthesis.
3. Require the task's exact `acceptance` assertion and its source/compiled/PCM-parity/host/hardware evidence class. If the environment lacks Rust, the six private FLACs or the physical DAC, mark **blocked/unverified** and report the operator command.
4. Run `python3 scripts/check_roadmap_index.py` after changing routing, `python3 scripts/check_literature.py` after literature maintenance, and `sh scripts/offline-gate.sh` when the toolchain exists. Optional Python checks are not part of the product's Rust-only build gate.
5. Update the task status and documentary evidence only after observing the corresponding result. Never auto-merge draft PR #1, force-push, install external codec libraries, invent measurements, or modify rust_radio/rust_tv/pip_boy without specific authorization.

## Handoff policy

- `gate_verifier` and `safety_reviewer` are **read-only in their instructions**. They may report findings but not claim a separate enforced read-only sandbox; inherited Codex permission settings still apply.
- `codec_engineer` handles own FLAC/WAVE/AIFF and future ALAC/CAF/MP4/WavPack decoders, with `safety_reviewer` inspecting codec confusion and bounds.
- `audio_backend` owns native PCM output and the physical evidence ladder; `host_integrator` owns the music socket, library and panel. Each must separately verify source ABI vs actual hardware behavior.
- `index_curator` reconciles evidence and routing, but **must not** promote gate outcomes just because a source commit landed.
- Current evidence: `RM-001`, `RM-002`, and `RM-008` through `RM-010` have source/build/parity evidence recorded in the roadmap. `RM-005` physical output and `RM-006` host audio arbitration remain blocked on device/host integration; under this goal, sibling repositories stay read-only. Broad ALAC admission remains open pending real-world corpus coverage.

## Codex kickoff prompt

```text
Read AGENTS.md and docs/roadmap-index.json. Select the highest-priority
planned task whose prerequisites are complete; verify the current status
before starting. As of 2026-10-29, RM-001, RM-002 and RM-008 through RM-010 are
done; verify remaining host/hardware tasks before choosing work. Preserve local edits, use one writer per overlapping
scope, request a read-only safety review for product changes, record exact
commands/exit status, update the roadmap only for evidence actually observed,
and keep PR #1 draft and unmerged.
```

## Maintaining the index

Update `docs/roadmap-index.json` when introducing a gate, task, agent, path or dependency. Keep stable IDs; do not renumber past tasks. Synchronize the readable tables and role files, then run the read-only [index checker](../scripts/check_roadmap_index.py). Do not hand-edit generated [LITERATURE_INDEX.md](LITERATURE_INDEX.md) because `scripts/build_literature.py` owns it.
