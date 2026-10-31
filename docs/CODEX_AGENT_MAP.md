# Codex agent routing — Rust Music

Codex reads [AGENTS.md](../AGENTS.md) as root project instructions. Actual **project-scoped subagent definitions** live in `.codex/agents/*.toml` and are recognized by Codex versions supporting custom agents. [Codex project config](../.codex/config.toml) allows up to **three** concurrent spawned agents and intentionally pins no model, cloud provider, server or dependencies. These are development assistants, not part of the offline player. Custom-agent instructions do not create separate sandbox permissions.

The separately pinned workflow reference and mapping from its orchestrator/reviewer roles to these Rust Music specialists is documented in [AGENT_SKILLS_INTEGRATION.md](AGENT_SKILLS_INTEGRATION.md). It is guidance only; repository instructions, task dependencies and actual Codex permissions remain controlling.

The single source of machine-routing facts is [roadmap-index.json](roadmap-index.json); human gate progress is in [ROADMAP.md](ROADMAP.md).

## Role / file ownership

| Role | Codex file | Mode | Owned scope | Purpose |
| --- | --- | --- | --- | --- |
| `gate_verifier` | `.codex/agents/gate-verifier.toml` | read only | read-only | Offline gate and sample-parity evidence; no edits |
| `codec_engineer` | `.codex/agents/codec-engineer.toml` | write scoped | `crates/rust-music-flac/`, `crates/rust-music-wave/`, `crates/rust-music-aiff/`, `crates/rust-music-alac/`, approved future codec crates | Own independent codec/container decoding and rigorous PCM tests |
| `audio_backend` | `.codex/agents/audio-backend.toml` | write scoped | `crates/rust-music-native/` | OS PCM adapters and hardware-format acceptance |
| `host_integrator` | `.codex/agents/host-integrator.toml` | write scoped | `crates/rust-music-host/`, `crates/rust-music-library/`, `crates/rust-music-panel/`, `apps/rust-music-macos/`, package smoke/build scripts | Rust_Boy audio lease, bounded library, 1KiB interface and standalone UI lifecycle |
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
- Current evidence: `RM-001`, `RM-002`, and `RM-008` through `RM-010`, plus `RM-017` M4A ingestion/upstream corpus parity, have source/build/parity evidence recorded in the roadmap. `RM-005` physical output and `RM-006` host audio arbitration remain blocked on device/host integration; under this goal, sibling repositories stay read-only. Broad ALAC admission remains open pending wider real-world source/encoder coverage.

## Current handoff

Use [engineering handoff — 2026-10-30](ENGINEERING_HANDOFF_2026-10-30.md) to verify branch selection, evidence provenance, and exact next-agent assignments before launching Codex. `main` was README/LICENSE-only at inspection; do not treat it as the engineering checkout. Preserve stable RM task IDs and explicit prerequisite blocks.

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

## 2026-10-31 portable library task routing

Route RM-021 through RM-027 using the canonical [roadmap index](roadmap-index.json) and [portable deployment contract](design/PORTABLE_LIBRARY_DEPLOYMENT.md). Initially retain the existing `host_integrator` as **the sole writer** for library/socket/IPC/desktop application paths; no additional agent writes those overlapping trees concurrently. `index_curator` owns roadmap/index updates serially. `gate_verifier` and `safety_reviewer` independently validate tests and security (read-only).

Task order: read-only RM-021 API inventory -> RM-022 manifest and corpus fixtures -> RM-023 pack/verify/import -> RM-024 macOS deploy UI + RM-025 host consumer on explicitly non-overlapping files -> RM-026 Linux/Windows adapters -> RM-027 physical field testing. Preserve RM-020/macOS v1.0 as the highest immediate production priority; no automatic cross-platform declaration. Only add specialized platform agents after actual code ownership is split and indexed.

Under agent_skills orchestration, assign software-delivery-orchestrator to integration scheduling, code-implementation to one scoped writing lane, build-integration to offline build evidence, test-verification to separately observed results, security-assurance to path traversal/hash/staging reviews, independent-code-review to compatibility, and documentation-maintenance to registry synchronization. Pin and verify the actual upstream agent_skills revision; never infer it was loaded.

## Current UX research and repair assignment (2026-10-31)

Read [Winamp usability research tasking](WINAMP_UI_AGENT_RESEARCH.md), [literature matrix](LITERATURE_MATRIX.md), and [MAC-06](MAC06_DIAGNOSIS_UI_AB_PLAN.md). `host_integrator` is the sole writer for `apps/rust-music-macos/`; `audio_backend` alone changes the native sink; `gate_verifier` performs independent read-only playback/UI regression evidence; `safety_reviewer` checks offline/no-artwork/audio-focus boundaries; `index_curator` writes literature/roadmap docs serially. Do not spawn overlapping writers or infer audible output from Playing state. Run literature/index validators after registry changes and do not claim tests run unless observed.
