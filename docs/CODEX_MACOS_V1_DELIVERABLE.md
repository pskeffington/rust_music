# Rust Music macOS v1.0 production matrix

**Baseline:** shared source `820582c3602df2cc63a1b76fbda88b5b58bbc396`; Rust
transport commits `fc47a19` and `7ae312b`; Swift/CLI integration commit
`0b1263c`; launch evidence commit `a4d6fc9`; local non-force history
reconciliation commit `6d17cce` (2026-10-31). The remote development history
is now an ancestor locally; no push was made. This is an evidence ledger, not
release approval.

## Acceptance gates

| Gate | Production requirement | Current status | Evidence / remaining acceptance |
| --- | --- | --- | --- |
| MAC-01 | Complete usable library-to-playback workflow | **IMPLEMENTED / UI ACCEPTANCE OPEN** | The app connects explicit bounded/cancellable library load, text search, named playlists, persistent queue/repeat/shuffle, Play/Pause/Resume/Stop/Next/Previous, and volume through the RBPK child process. Queue selection does not autoplay. The offline Rust gate and macOS app bundle build pass on this integration; simulated package-stream E2E is covered. Window interaction with this exact rebuilt app remains to be observed. |
| MAC-02 | Native macOS playback, shutdown and device recovery | **PARTIAL / OPERATOR** | Prior LaunchServices startup, ad-hoc bundle signing, and generated-WAV Play/Stop/window-close child-exit evidence are recorded in `docs/PR1_MERGE_READINESS.md`. AudioQueue fault/teardown tests pass 15/15, including pause-before-first-buffer. This pass did not test audible output or real device disconnect/recovery. Developer ID/notarization remains a distribution gate. |
| MAC-03 | Complete offline suite, performance and independent security/code review | **PARTIAL** | `sh scripts/offline-gate.sh` exited 0 on source through `0b1263c` (17 local crates, 32 local path edges, no registry/Git crates); app build/signature verification and roadmap/literature validators exited 0. Existing FLAC 6/6, WAV 6/6, AIFF 7/7 and ALAC/WavPack evidence is retained; private corpus was not rerun. One LaunchServices sample and 94,432 KiB RSS sample were recorded, but cold-launch distribution and incremental library memory remain unmeasured. Independent Swift and security reviews found no actionable finding after the paused-navigation fix. |
| MAC-04 | Reviewed mainline integration with preserved history | **BLOCKED BY RELEASE GATES** | `origin/main` is an ancestor. All fetched remote development commits and local transport/app commits are preserved in local non-force merge `6d17cce`. Five commits remain local relative to refreshed origin; no push was made because acceptance gates remain open. PR #1 remains draft. Preserve backup branch and stash through post-merge verification. |

## Component matrix

| System | Requirement | State / next acceptance |
| --- | --- | --- |
| Native audio | macOS native output, safe teardown, device error/recovery | AudioQueue adapter and injected failure/cleanup tests exist. Add transport-safe pause/volume support if API permits; operator must disconnect/reconnect a real output device during playback and verify cleanup plus explicit recovery. |
| Decoders | Lossless-only admission and validated PCM | Native decoders and offline parity evidence are tracked by G2/G7/G8/G10/G11/G12. Retain existing evidence; run full codec gate only when release procedure requires it, and never include private media in Git. |
| Library | Explicit scan, indexing, metadata, search | Text-only explicit folder load, cancellation, catalog search and bounded metadata exist. Measure cold startup and incremental catalog RSS; test empty/missing/removable roots. |
| Playlists | Named persistence and M3U8 support | Bounded versioned named playlist storage and CLI/UI CRUD exist. Complete import/export and restart/error acceptance if required by v1 scope. |
| Queue | Persistent ordering, repeat and shuffle | Root-scoped persistent CLI/UI queue exists; selection does not play. Verify queue transitions against active playback after transport API is integrated. Non-Unix persistent queue behavior remains unsupported. |
| Transport | Play, Pause, Resume, Stop, Next, Previous | Implemented over the persistent package process. Engine and host tests cover pause/resume position, lease release/reacquire, denied focus and Stop; CLI package-stream E2E covers Play/Pause/Resume/volume/Stop. Next/Previous transition queued or catalog selection during active or paused playback. Actual output remains unverified. |
| Volume | User-visible functional native volume | Implemented as bounded software attenuation (0–100), with strict 100% integer-sample bypass. It is not hardware gain or bit-perfect processing. Engine, IPC and CLI tests cover validation/routing; user interaction and physical output remain unverified. |
| Application | Valid bundle, LaunchServices and shutdown | Local bundle build/plist/ad-hoc code-sign validation and LaunchServices launch evidence are in PR readiness ledger. Revalidate after binary/UI changes; distribution signature/notarization remains open. |
| Security | Bounded parsers, safe paths, no network/artwork | Workspace-local Cargo edges and fail-closed bounded inputs are enforced by offline gate. Maintain no image/artwork/visualizer functionality and never modify source audio during library operations. |
| Performance | Startup, memory, search, transitions | Search/scan profile at 0/1/100/2k/20k exists. Cold startup, incremental catalog memory, transition latency, and attenuation processing cost are not measured. |
| Release | Reproducible build and independent acceptance | Local offline/app build evidence exists at recorded commits. Require fresh build evidence at candidate SHA, independent code/security review, operator hardware record, clean mainline build and app launch before declaring v1.0 ready. |

## Operating constraints

- No artwork, image extraction/decoding/caching, visualizers, remote metadata,
  telemetry, third-party runtime Cargo dependencies, or runtime network use.
- No startup/background scans or autoplay. Any future opt-in autoplay remains
  default-off and requires an explicit successful library load plus host focus
  grant.
- Rust_Boy retains exclusive shared audio focus with Radio/TV; standalone Mac
  acceptance and Rust_Boy hardware acceptance are tracked separately.
- Keep `main`, backup branches, and the saved stash untouched until release
  policy permits integration. Never force-push or bypass required checks.

## Required update per engineering pass

Record the exact source SHA, changed files, commands and exit codes, independent
review result, device/operator evidence, and the next unblocked task. A missing
hardware/tool prerequisite is **BLOCKED** or **UNVERIFIED**, never PASS.

## 2026-10-31 integrated transport verification

At Swift/CLI source commit `0b1263c` (Rust source through `7ae312b`), the full
`sh scripts/offline-gate.sh` exited 0 (17 local crates, 32 path-only edges,
zero registry/Git crates). AudioQueue injected failure and teardown tests pass
15/15, including immediate pause/resume before the first device write. The
package-stream Play/Pause/Resume/volume/Stop E2E regression passes. The Linux
ALSA target check passed for `aarch64-unknown-linux-gnu`; ALSA device tests
cannot run on this macOS machine. `sh scripts/build-macos-app.sh` exited 0,
and the local ad-hoc app signature passed strict verification. Roadmap and
literature validators and `git diff --check` exited 0. Source review found no
image APIs or visualizer subsystem. No private parity corpus was rerun.

Still unverified: interaction with this exact rebuilt app window, audible
output and device loss/recovery, physical DAC and panel, Rust Boy shared
Radio/TV focus, cold startup/incremental RSS, and Developer ID/notarization.
Independent Swift and security reviews found no actionable finding after the
paused-navigation fix. These operator and performance gaps block release
integration under current policy.

### Launch and memory sample for the rebuilt bundle

The rebuilt bundle was launched with `open -n` (exit 0). `lsappinfo list`
showed bundle `/Users/null/GitHub/rust_music/target/macos/RustMusic.app`,
executable `Contents/MacOS/RustMusic`, PID 89533, and LaunchServices check-in;
reported launch-to-check-in was 0.218 s. A single `ps` snapshot about 20 s
after launch reported RSS 94,432 KiB. This is not a cold-launch distribution,
catalog incremental RSS, nor memory target pass; it includes SwiftUI/AppKit and
the process baseline. Sending SIGTERM to validation PIDs 89533 and 88443 was
followed by `ps -p` exit status 1 for both, confirming neither app process
remained. The unrelated pre-existing app PID was not signaled. No playback
child was started during this LaunchServices sample.

## Reconciled standalone scope and acceptance checklist

The following scope and criteria are retained from the fetched remote
documentation commits `6012f53`, `e23989e`, and `9d58593`. They remain
requirements; the evidence matrix above is the current observed status.

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
