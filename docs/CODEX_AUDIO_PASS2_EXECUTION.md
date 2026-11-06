# Rust Music — Codex execution packet, pass 2 (2026-11-06)

This is an instruction packet, not an assertion that tests ran. Use alongside [agent refresh](AUDIO_AGENT_SKILLS_REFRESH_2026-11-06.md), [agent map](CODEX_AGENT_MAP.md), and [MAC07 ledger](evidence/MAC07_PASS_RESULTS.md).

## Preflight (read-only, before spawning agents)
1. Record `git status --short --branch`, `git rev-parse HEAD`, `git stash list`, current PRs, and unresolved merge entries. Never reset or discard local work.
2. Check live GitHub PR status. PR #1 merged historically; do NOT execute old instructions to keep it draft/unmerged. PR #15 is the agent-doctrine draft at authoring time; recheck current state.
3. Resolve contradictions by priority: observed HEAD and dated test evidence > current canonical task index > dated handoffs. Preserve history, append corrections, never rewrite observations.
4. Pin reachable `agent_skills` SHA; report the exact loaded roles and references. If unavailable, use the existing `.codex/agents/*.toml` roster, log degraded orchestration and do not pretend roles were loaded.
5. Read `docs/mac07-pass-index.json` and evidence ledger. Its recorded `branch` is historical and cannot dictate checkout. A pass with a blocked prerequisite cannot be marked accepted by downstream implementation alone.

## Three-lane deployment (max three simultaneous agents)
- **Lane A — host_integrator**, sole writer for `apps/rust-music-macos/`, library, queue and GUI protocol. Start with read-only MAC07 route/progress gap classification, then one scoped defect repair once reproducible.
- **Lane B — audio_backend**, read-only diagnostic investigation until an independent output-adapter defect is demonstrated. Own only native sink code when approved. Never write the SwiftUI tree.
- **Lane C — gate_verifier or safety_reviewer**, read-only independent evaluator, with the reviewer role rotated per task. Check path, codec, focus, state-truth and evidence claims.

The primary Codex session acts as orchestrator and serial documentation integrator. Do not exceed the existing 3-agent concurrency limit. Workstreams for DSP, performance and packaging are queued, not concurrent file writers.

## Current work orders

### RM-028 / MAC07-00: route truth (P0)
- Input: same operator-approved, locally held real FLAC; device inventory and timestamped CLI and GUI sessions; never commit media, paths or private filenames.
- Compare exact route/device UID, OSStatus, active session identity, stdout/stderr, delivered frames, callbacks, selected output and actual operator hearing.
- Explicitly separate a `CountingSink` decode pass from physical sound. Do not infer audibility from `Playing`, frames, callback counts, or remembered stderr.
- Acceptance: operator-confirmed result on a usable route, with sanitized repeatable command, revision, selected device, outcome and independent reviewer.
- If audio hardware inaccessible: `BLOCKED-HARDWARE`; continue only non-dependent read-only review and independent tests.

### RM-028 / MAC07-01: managed-process playback (P0)
- Only after diagnosis, repair the smallest verified GUI handoff/session defect.
- Test bounded stderr lifetime, fresh process/session attribution, disconnect/restart, control handoff, exactly one sink and truthful error transitions.
- Acceptance: audible real FLAC through current GUI on confirmed output route; review of no-regression offline tests and independent verification.
- Historical one-album audible observation is useful but not a broad acceptance gate.

### RM-029 / MAC07-02 and MAC07-03: state and UI (P1, dependency guarded)
- Check progress monotonicity except explicit seek/reset, paused state, EOF, failed route, stale events, queue selection and accessible labels.
- Verify native keyboard focus, queue and library selection, small-window layout and VoiceOver interaction. Use original text-only style, zero artwork/assets.
- Capture baseline-versus-candidate reproducible screenshots and interaction logs. Mark untested device/VoiceOver observations explicitly.

### RM-030 / MAC07-04 onward: queue and release (P1, dependent)
- Only advance after dependencies are supported by evidence.
- Validate Play Now/Next, EOF advancement, persistent queue/playlist restart, safety against path traversal and shared focus overlap.
- Finally test the same music on same routes with A/B comparison; separate reproducible software tests from human auditory acceptance.

## Required evidence record per order
```text
task_id:
branch_sha:
agent_skills_sha: verified | unavailable
owner_writer:
independent_reviewer:
environment_and_output_device:
fixture_provenance: synthetic | operator-private (never expose path)
commands_and_exit_codes:
baseline_observation:
candidate_observation:
actual_audibility: confirmed | denied | not_run
acceptance: pass | fail | blocked | not_run
residual_risks:
next_unblocked_task:
```

## Validation commands and stop rules
- `python3 scripts/check_roadmap_index.py` for registry/routing edits.
- `sh scripts/offline-gate.sh` when offline toolchain is present; report exact result.
- macOS app build and reproducible same-file A/B diagnostic where documented; never assume tools/hardware exist.
- Fail closed on missing prerequisites or unrelated dirty edits. Do not install dependencies, push to main, merge, change sibling repositories, or auto-certify release.
- Closeout: add dated evidence to MAC07 ledger, sync indexes only for observed changes, independent review, draft PR.
