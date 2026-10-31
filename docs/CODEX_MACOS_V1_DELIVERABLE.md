# Codex release target: standalone macOS v1.0

**Status:** Required engineering plan and scoping decision; this document does not certify release readiness. The latest observed remote PR #1 head before this documentation pass was `820582c3602df2cc63a1b76fbda88b5b58bbc396`. Verify actual local HEAD and integration state; preserve active local work.

## FIRST — agent_skills and checkout safety

1. Read-only inspect `git status --short --branch`, `git ls-files -u`, `git stash list`, `git log --graph --oneline -30`. Do not merge/pull over uncommitted or conflicting local work.
2. Load a verified revision of `pskeffington/agent_skills` with software-delivery-orchestrator, build-integration, code-implementation, test-verification, independent-code-review, security-assurance and release-readiness; read actual SKILL.md role contracts. Record SHA. Retain Rust Music specialists. One code writer per overlapping scope.
3. Reconcile these remote docs non-destructively with any more recent local engineering; never force-push/reset/clean away local changes or private evidence.

## Explicit standalone Mac v1.0 scope

The release target for this pass is a **working, offline, ultra-lightweight native macOS app**, not simultaneous Rust Boy hardware certification. Support versions consistent with the bundle's minimum declared macOS version and actual test machine (currently macOS 13 floor); record tested architectures and versions.

Required user workflow: launch signed local .app; choose local library folder; bounded deterministic scan; text-only search/browse; build/save/reopen named playlists and queue; choose a supported lossless file; **Play, Pause/Resume, Stop, Next and Previous must operate on actual audio**; correctly handle repeat/shuffle; volume control; close/quit stops/releases child and native audio device; reopen without corruption. Never silently autostart after startup/recovery.

No artwork, embedded pictures, image decoding/caching, visualizers, skins, placeholders, remote metadata or network activity. Existing fixed Rust Boy monochrome text panel can remain separately implemented. Zero third-party Cargo runtime dependencies. Native lossless codecs only; retain source minimum >=16-bit integer PCM and >=44.1kHz admission and exact native decoder parity evidence.

**Explicitly deferred from standalone Mac v1.0 (track for v1.1/host release):** native 10-band EQ, optional autoplay-after-explicit-library-load, Rust Boy 128x64 physical panel, live Rust Boy MUSIC/RADIO/TV arbitration, Windows/Android, distribution via a public notarized installer. Do not misrepresent those as complete or silently remove existing code. If EQ/autoplay code already exists, it must not regress or weaken safety. A normally downloadable, Gatekeeper-friendly Mac release requires Developer ID signing/notarization before that *separate distribution claim*; ad-hoc local signing supports only local/test delivery.

## P0 — minimum usable application gate (MAC-01)

- Inspect existing current source and identify real vs selection-only Previous/Next; implement actual playback transitions and end-of-track advance under tested queue/repeat/shuffle semantics.
- Add native Pause/Resume and bounded volume handling without restarting or leaking playback. If platform/backend cannot support a feature, fix rather than claim completion.
- Connect named playlist persistence and library reload to UI, using atomic versioned local state and robust errors, without modifying user media.
- Validate invalid/missing/removable tracks, cancel/scan limits, repeated clicks, focus/resource cleanup and shutdown.
- Native app should expose compact text controls without images. Ensure app and CLI do not duplicate conflicting state machines.

**Acceptance:** reproducible end-to-end user workflow with real audible output on a Mac; UI and subprocess exit verified; restart restores catalog/queue/playlists without autoplay. Add automation for deterministic logic and record manual evidence where UI/device interaction cannot be automated.

## P1 — macOS output and failure gate (MAC-02)

- Rebuild .app, lint Info.plist, confirm binary architecture/version and contents, verify strict ad-hoc local signature; exercise unsandboxed `open -n`/LaunchServices and direct executable paths as appropriate.
- Test available CoreAudio/AudioQueue device negotiation, non-immediate drain, immediate stop, disposal and child exit with real hardware; exercise no-device, device disappearance, unavailable/permission errors, reconnect/recovery, rapid exit/restart. Verify no orphan child processes, no permanently held device/focus and no callback after disposal.
- Validate actual audible playback. A complete scripted offline gate is **not** physical audio acceptance; physical DAC bit-perfect wording requires measurement.
- Record machine OS, architecture, audio device type, tested formats/rates, commands, results, and known restrictions. Do not include sensitive machine/user paths in repo.
- Local ad-hoc signature is adequate for internal/test app acceptance only. Label distribution signing/notarization pending if no authorized Developer ID credentials.

## P2 — release regression and measurements (MAC-03)

- Run `cargo fmt --all -- --check`, `cargo clippy --locked --offline --workspace --all-targets -- -D warnings`, `cargo test --locked --offline --workspace`, `cargo build --locked --offline --release --workspace`, `sh scripts/offline-gate.sh`, roadmap/literature checks, package smoke and app-build script.
- Preserve prior reported FLAC real-world 6/6, WAV 6/6, AIFF 7/7, ALAC parity and WavPack test evidence. Re-run impacted regression fixtures as required; use private six-file FLAC corpus only locally and never commit media or private logs.
- Benchmark cold app launch, idle memory, catalog *incremental* RSS, scan/search, playback Next latency, device failure/restart and release binary/bundle size. State reproducible test conditions and thresholds selected from measured baseline, not arbitrary PASS claims.
- Independent security + code review inspect bounded metadata, playlists, symlinks, no image support, no network, native child lifecycle and privacy.

## P3 — scoped PR #1 mainline decision (MAC-04)

PR #1 historically includes Rust Boy integration work; **scoping the first Mac deliverable does not erase, weaken or waive repository gates for code claimed production-ready**. Determine whether all of PR #1 is safe to merge as-is with unverified Rust Boy functionality explicitly marked experimental/disabled, or whether to form a dedicated reviewed Mac-only PR/branch that excludes incomplete hardware-dependent changes. Record the release-scope decision; do not silently declare global production acceptance.

- Update canonical roadmap, G5/G3 and PR1 merge-readiness with Mac-only versus deferred acceptance; never convert untested gates to PASS.
- Ensure local/remote histories reconciled with non-force ancestry; never overwrite unpushed commits.
- When MAC-01/02/03 pass and independent review approves the scoped code, update the proper PR with test evidence, mark ready and merge **only** with authorized release decision. Otherwise leave draft and enumerate blockers.
- Verify `main` clean checkout/offline build and standalone Mac launch after any merge. Preserve rollback and backup refs until verified. No release tag without actual acceptance.

## Codex implementation behavior

Do not stop with a planning document. Continue focused code/test/commit passes for the first unblocked Mac v1.0 task. Work on supported local checkout; do not assume a remote API write can update Codex's live checkout. No third-party crates, artwork, or unrequested playback. Avoid unrelated codec/platform feature expansion until basic Mac player delivers.

## Required handoff

Report exact local/remote HEAD, agent_skills SHA, task IDs/owners, code commits, actual test outputs/exit codes, Mac app/manual evidence, device and signing status, what remains deferred for v1.1/Rust Boy, and PR/mainline recommendation. Never claim release completion from compilation alone.
