# CODEX AUI-02 — Operator Windows and Large Library Delivery

Date: 2026-11-01
Status: EXECUTION TASKING; NOT IMPLEMENTED / TESTS NOT RUN
Authoritative UI scope: docs/AUI01D_OPERATOR_WINDOWS_ONLY.md and docs/AUI01E_LARGE_LIBRARY_MANAGEMENT.md.
Related: docs/CODEX_MAC06_EXECUTION.md and docs/CODEX_AGENT_MAP.md.

## Codex launch instruction
Work in pskeffington/rust_music. Read AGENTS.md, inspect git status/branches and local unpushed work, pin baseline SHA. Load verified agent_skills hierarchy and project .codex/agents/*.toml definitions before task assignment. Preserve dirty files; do not force-push, auto-merge or reset. Review current SwiftUI window creation, library/catalog IPC, queue, playlist, RBPK playback snapshots and tests. Prefer small reviewable commits with observed evidence. Existing Mac audible playback and progress MAC-06 P0 gates remain prerequisites to release; do not regress them.

## Non-negotiable product instruction
Winamp reference describes OPERATOR WINDOW ORGANIZATION ONLY. Do not copy its palette, textures, bitmaps or branding, and do not redesign colors. Keep existing Rust Boy / Pip-Boy phosphor theme exactly. Do not implement skin imports, visualizers, album artwork, or unrelated styling. Keep own Rust decoders, zero third-party Cargo runtime packages, offline behavior, fixed Rust Boy 128x64 panel and exact RBPK v1 wire protocol.

## Agent dispatch / ownership
1. software-delivery-orchestrator (agent_skills if verified): scope, baseline, critical path, handoffs; no overlapping edits.
2. host_integrator (.codex/agents/host-integrator.toml): ONLY writer for apps/rust-music-macos/ UI. Deliver four independent window shells sharing ONE app state model, track table virtualization, keyboard/VoiceOver.
3. host_integrator in a SEPARATE SERIAL subpass: ONLY writer for crates/rust-music-library/ catalog/index/paging/scan, because this agent already owns that path. Split out a specialized library agent only after verified registered role exists and ownership moves explicitly.
4. audio_backend (.codex/agents/audio-backend.toml): owns crates/rust-music-native/ only if evidence reveals a sink issue. Otherwise review/read-only for this UI pass. Coordinate MAC-06 progress truth; UI should not infer audible output from Playing.
5. gate_verifier (.codex/agents/gate-verifier.toml): read-only independent screenshot geometry/interaction, large catalog latency-memory and actual playback A/B evidence.
6. safety_reviewer (.codex/agents/safety-reviewer.toml): read-only local paths, malformed tags, SMB disconnect, scan cancellation, private metadata, symlink handling, UI source-of-truth and audio-focus audit.
7. index_curator (.codex/agents/index-curator.toml): serial roadmap, index, evidence and handoff edits only after test outcomes.

Codex supports up to three concurrent spawned agents per current .codex/config.toml; do NOT spawn seven agents simultaneously. Parallelize read-only inspection only, serialize writing lanes and integrate with main orchestrator.

## Engineering passes
AUI02-00: inventory current app structure and baseline source SHA; screenshot each existing window; establish failing tests for missing operator behavior.
AUI02-01: implement separate Player, Playlist Editor, Equalizer and Media Library windows using existing Rust Boy styling and ONE shared state; default left stack and right library geometry; independent show/hide.
AUI02-02: implement snapping/docking, detach, safe multi-display layout restore, keyboard window visibility shortcuts, VoiceOver names and retained transport on window close.
AUI02-03: Media Library split artist/album/track navigation, explicit folder import, dense virtualized multi-select track table and honest unsupported/missing-file states.
AUI02-04: Rust catalog bounded paging, indexed sort/filter, stable IDs, generation-cancelled search, interruption-safe incremental scan and persistent locations; avoid duplicating 250k records into Swift UI.
AUI02-05: bulk enqueue/play-next/playlist assignment/reorder, playlist persistence and reconnect/missing-track handling; selection never triggers playback.
AUI02-06: actual FLAC/WAVE audio under scan/search/load, no UI or audio callback stalls, keyboard and VoiceOver, window/docking, offline gates and measured 0/100/2k/20k/100k/250k catalogs.

## Evaluation criteria and stop conditions
Record baseline/candidate SHA, exact Mac/software, catalog size, search p50/p95, scan throughput, peak RSS, scroll responsiveness, UI event-loop stalls, queue mutation correctness and operator audible output separately. State which samples were synthetic. A 250k target is NOT a claim of existing support. Simulated AudioQueue or frame counts are NOT proof of audible DAC. If hardware/toolchain absent, mark test NOT RUN/BLOCKED.

Commands:
python3 scripts/check_literature.py
python3 scripts/check_roadmap_index.py
cargo test --locked --offline --workspace
sh scripts/offline-gate.sh
sh scripts/build-macos-app.sh
codesign --verify --deep --strict --verbose=2 target/macos/RustMusic.app
open -n "$PWD/target/macos/RustMusic.app"

Commit code, fixtures and docs only after local verification, never private licensed audio. Release gate remains blocked until MAC-06 playback and device acceptance are independently demonstrated.

## Expected deliverables
Functional four-window application; virtualized searchable library; queue/playlist bulk workflow; dedicated tests; docs/evidence/AUI02_RESULTS.md with PASS/FAIL/BLOCKED per command; updated index/roadmap only for observed status; operator handoff listing unfinished items.
