# Codex production continuation — agent_skills first

**ORDER OF OPERATIONS: agent_skills discovery and task routing is the FIRST engineering step after a non-mutating Git safety inspection.** Before starting Rust Music implementation, load the verified agent framework and delegate according to its skill contracts. This document is an instruction, not a report of completed checks.

## 0. Safety inspection (read only)

Inspect `git status --short --branch`, `git ls-files -u`, `git stash list`, `git log --graph --oneline --all -30`. Do not pull, reset, rebase, switch branches or overwrite an unfinished merge. The operator previously reported local-vs-remote divergence, conflicts in AGENTS.md/README.md and a saved stash containing WavPack, IPC and app work. Those reports may be stale: diagnose actual state.

## 1. Load agent_skills FIRST

Upstream: `https://github.com/pskeffington/agent_skills` (sibling repo, **not** a Rust Music runtime dependency).

- Find an already existing local checkout first. If missing, obtain it through approved Git access without overwriting local data.
- Inspect the current branches and commit trees. The software-agent hierarchy was verified on `feat/software-agent-hierarchy-20261030` (observed SHA `42cf3196122feffa8658239fe7947c033bd39913`), and an additional priority branch exists at `docs/codex-priorities-20261031` (observed SHA `6989d835c62f9e0858db56252f1ac7ff6ce65414`). These are reference observations, **not** assumptions about latest branch state; pin and report the exact revision actually loaded.
- IMPORTANT: `agent_skills/main` was observed as minimal (README/LICENSE); do not silently treat it as the full hierarchy.
- Read its `AGENTS.md`, `catalog/roles.json`, `catalog/resource-chain.json`, `docs/RESOURCE_CHAIN.md`, `docs/RELEASE_GATES.md` and only the applicable `.agents/skills/*/SKILL.md` entrypoints.
- Assign `software-delivery-orchestrator` to planning/serial integration; `build-integration` to checkout and compilation; `code-implementation` to scoped Rust changes; `test-verification` to observed gates; `security-assurance` and `independent-code-review` to independent checks; `documentation-maintenance` to index updates; `release-readiness` to final readiness determination.
- Respect the contracts actually present in the pinned revision. Skills are development instructions; they cannot replace executed tests or enforce isolation merely by role naming.
- If the framework is not accessible, record the blocker and use existing Rust Music Codex agents only. Do not invent missing skills or stall safe checkout repair.

## 2. Recover the rust_music checkout

Have the orchestration/build-integration agents complete the pending merge **before** restoring the saved stash. Resolve conflicted README/AGENTS content semantically, preserve local code and remote policy, finish the merge, then `git stash apply` and reconcile any additional conflicts. Keep backups and stash until all content is verified. Do not force-push or lose WavPack, IPC, fixtures, app or packaging work. Integrate remote docs without overwriting local changes.

## 3. Deliver the ultra-micro player and library

Read `docs/MICRO_LIBRARY_ZERO_ARTWORK.md` before library/metadata/UI code. Implement a standalone and Rust Boy text-first, offline lossless music app with browsing, bounded metadata, sorting/search, playlists, persistent queue, repeat/shuffle, CLI controls, native playback and host audio-focus integration. **ZERO artwork/images**, including extracted PICTURE/APIC/covr blocks, decoding, caching, placeholders, network metadata, visualizers or skins. Keep the 128x64 fixed 1024-byte monochrome text/control framebuffer. Preserve native own decoders and zero third-party runtime Cargo packages.

Suggested order: reconcile workspace -> full clean offline build gate -> native codec PCM parity (FLAC/WAVE/AIFF/WavPack where supported) -> player lifecycle/focus -> compact metadata/search -> playlists and queue -> standalone packaging and Rust Boy interface -> memory/CPU benchmarks -> independent release review. Avoid claiming WavPack is production supported until parity and malformed-input tests pass.

## 4. Verification and mainline

Use `docs/PR1_MERGE_READINESS.md`, `docs/ROADMAP.md` and `docs/roadmap-index.json` as existing acceptance authorities. Run the real `sh scripts/offline-gate.sh`, documented reference suites, host and hardware tests where available. The private six-track corpus remains private. Status is based on recorded command/exit/platform/HEAD evidence, never static source review. One writer per overlapping scope. Record blockers; continue other independent safe tasks.

PR #1 must remain draft and unmerged while release-critical gates lack proof. When all applicable acceptance conditions are observed, synchronize the branch, make PR ready, review independently and merge non-force using a verified HEAD; verify a clean `main` build, and retain backups until acceptance. Do not disable checks for expediency.

## Required continuation report

Report: loaded `agent_skills` branch and SHA, specialist assignments, local merge/stash reconciliation, Rust Music HEAD and dirty state, code and fixtures preserved, exact commands/exits, PCM/host/hardware status, docs/index changes, blockers, and whether PR #1 is actually eligible for `main`.

**First task:** read the pinned `agent_skills` framework and produce its ownership/dependency routing, before taking any mutable step in rust_music.
