# Rust Music — Audio Agent Skills Refresh (2026-11-06)

Status: execution doctrine; not test or release evidence.

## Mandatory routing
1. Read AGENTS.md, docs/CODEX_AGENT_SKILLS_FIRST_PRODUCTION.md, docs/CODEX_AGENT_MAP.md, docs/roadmap-index.json, docs/mac07-pass-index.json, and the latest dated MAC07 evidence.
2. Inspect branch, HEAD, dirty files and existing PRs read-only before changes. Preserve all local work.
3. Pin a verified agent_skills revision and load applicable audio systems and audio UI skill contracts. If unavailable, use existing repository agents and mark central routing BLOCKED, never fabricate a pin.
4. Assign one writer per overlapping file scope. The host_integrator alone writes SwiftUI; audio_backend owns native sink changes; codec_engineer owns decoder changes; gate_verifier independently validates.
5. Implement on isolated task branches, test offline, submit reviewable PRs. No automatic merge, dependency installation, network playback, or private audio commits.

## Agent deployment matrix

| Agent | Scope | Acceptance |
| --- | --- | --- |
| audio_lead | Task graph, contracts, dependencies, scope conflict arbitration | Recorded plan, owners and blockers |
| audio_backend | AudioQueue route, session lifetime, underrun, drain, recovery, focus | Measured device route and actual audible operator gate |
| ui_engineer (host_integrator writer) | Winamp-like text-only player, keyboard, accessibility, queue visibility, scaling | Screenshot/input/accessibility A/B tests |
| library_queue_engineer | Explicit scans, indexing, playlist/queue persistence, deterministic transitions | Repeatable catalog and restart regressions |
| dsp_engineer | 10-band optional EQ, bypass exactness, headroom, clipping | PCM-exact bypass and bounded processed output |
| performance_engineer | Startup, memory, scan, playback jitter, incremental updates | Baseline/candidate measurements on identical corpus |
| platform_engineer | macOS packaging; later Linux/Windows adapters | Platform-local compile and physical output gates |
| gate_verifier | Independent adversarial review and evidence ledger | Exact commands, SHA, environment, exit codes and unrun gates |

## Execution order
- P0: reconcile current MAC07 evidence against stale status prose; diagnose CLI/GUI route parity, truthful per-session diagnostics, actual playback and device recovery.
- P0: complete UI transport/queue/playlist keyboard and accessibility A/B tests without copying Winamp assets or introducing images.
- P1: characterize large-library ingestion, search, incremental memory, startup latency, queue persistence and playback jitter; compare baseline/candidate.
- P1: validate opt-in EQ and explicit-library-load-only autoplay, default off and subject to host audio focus.
- P2: package and physically qualify macOS before advancing Linux ALSA, Windows WASAPI, portable library export or live Rust Boy integration.

## Immutable product boundaries
Lossless-only >=16-bit/44.1-kHz playback; workspace-local Rust dependencies; no third-party codec, FFmpeg, GStreamer, network or telemetry; no artwork, image cache, visualizer or skin assets; no private media in Git. Rust Boy owns shared audio focus. Embedded 128x64 panel stays 1024 bytes. UI skin work is declarative text/color/layout only until an explicit policy change.

## Evidence protocol
For each pass record: task ID, owner, branch and SHA, baseline, commands and exit codes, fixtures/corpus provenance, expected and actual outcomes, platform and device, independent review, and residual blockers. Source code, tests, decoded PCM, queue callbacks and UI status never alone prove audible DAC output. Mark missing physical tests NOT RUN or BLOCKED; do not claim production readiness.

## First Codex order
Read the required sources and pinned skills, generate a gap matrix of current agent contracts and MAC07 gates, then implement the smallest unblocked P0 fix with independent verification. Preserve historical ledgers; append dated evidence. Open a draft PR and stop at the review gate.
