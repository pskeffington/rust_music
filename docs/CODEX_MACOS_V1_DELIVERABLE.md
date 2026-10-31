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
| MAC-01 | Complete usable library-to-playback workflow | **PARTIAL / OUTPUT PATH VERIFIED** | Exact rebuilt-app UI loaded two generated 10-second, nonzero 16-bit/44.1 kHz WAV tones, queued both, played, paused/resumed, advanced Previous/Next between titles while Playing, changed volume 100→95→100, and stopped. The bundled CLI played a synthetic 440 Hz WAV through AudioQueue with exit 0; the operator reports audible output. This is native output-path evidence, not an acoustic loopback or DAC bit-perfect measurement. |
| MAC-02 | Native macOS playback, shutdown and device recovery | **PARTIAL / DEVICE RECOVERY OPEN** | Unsandboxed `open -n` exited 0; LaunchServices recorded bundle check-in in 0.065434 s. `system_profiler` listed MacBook Air Speakers as the 48 kHz default output and an ASUS VG32V HDMI output. Before Cmd-Q the app PID 93068 owned `rust-music-cli serve` PID 93107; after Cmd-Q neither PID remained and LaunchServices showed the app stopped. AudioQueue injected failure/teardown tests pass 15/15. Physical disconnect/recovery and DAC measurement remain open; Developer ID/notarization remains a distribution gate. |
| MAC-03 | Complete offline suite, performance and independent security/code review | **PARTIAL** | At source `9d1141975d5fa54c31fa2a7ce25c622c4218ebba`, `sh scripts/offline-gate.sh` exited 0 (17 local crates, 32 local path edges, zero registry/Git crates); macOS AudioQueue failure/teardown tests passed 15/15. A generated nonzero 440 Hz 16-bit/44.1 kHz WAV played through the bundled CLI with exit 0; `system_profiler` listed MacBook Air Speakers as the 48 kHz default output. In the UI, loading 2,000 generated catalog entries returned in <1 s and searching `track-01999` returned one result. Same-process RSS rose 103,312→104,880 KiB (+1,568 KiB); this is one fixture sample, not a threshold pass. Existing FLAC 6/6, WAV 6/6, AIFF 7/7 and ALAC/WavPack evidence is retained; private corpus was not rerun. Cold-launch distribution, broader incremental-memory profiles and latency measurements remain open. Independent Swift and security reviews found no actionable finding after the paused-navigation fix. |
| MAC-04 | Reviewed mainline integration with preserved history | **BLOCKED BY RELEASE GATES** | `origin/main` is an ancestor. Local transport/app and evidence commits remain preserved; the development branch is nine commits ahead of the refreshed remote. No push was made because other acceptance gates remain open. PR #1 remains draft. Preserve backup branch and stash through post-merge verification. |

## Component matrix

| System | Requirement | State / next acceptance |
| --- | --- | --- |
| Native audio | macOS native output, safe teardown, device error/recovery | AudioQueue adapter and injected failure/cleanup tests exist. Add transport-safe pause/volume support if API permits; operator must disconnect/reconnect a real output device during playback and verify cleanup plus explicit recovery. |
| Decoders | Lossless-only admission and validated PCM | Native decoders and offline parity evidence are tracked by G2/G7/G8/G10/G11/G12. Retain existing evidence; run full codec gate only when release procedure requires it, and never include private media in Git. |
| Library | Explicit scan, indexing, metadata, search | Text-only explicit folder load, cancellation, catalog search and bounded metadata exist. Measure cold startup and incremental catalog RSS; test empty/missing/removable roots. |
| Playlists | Named persistence and M3U8 support | Bounded versioned named playlist storage and CLI/UI CRUD exist. Complete import/export and restart/error acceptance if required by v1 scope. |
| Queue | Persistent ordering, repeat and shuffle | Root-scoped persistent CLI/UI queue exists; selection does not play. The nonzero two-tone UI smoke verified active Next/Previous queue transitions. Non-Unix persistent queue behavior remains unsupported; broader repeat/shuffle playback semantics remain covered by unit tests rather than this device smoke. |
| Transport | Play, Pause, Resume, Stop, Next, Previous | Implemented over the persistent package process. Engine/host tests cover pause/resume position, lease release/reacquire, denied focus and Stop; package E2E and the two-tone UI smoke exercised transitions while native output was running. Physical device-loss recovery remains open. |
| Volume | User-visible functional native volume | Implemented as bounded software attenuation (0–100), with strict 100% integer-sample bypass. It is not hardware gain or bit-perfect processing. The app slider changed 100→95→100 during the two-tone output smoke. |
| Application | Valid bundle, LaunchServices and shutdown | Local bundle build/plist/ad-hoc code-sign validation and LaunchServices launch evidence are in PR readiness ledger. Revalidate after binary/UI changes; distribution signature/notarization remains open. |
| Security | Bounded parsers, safe paths, no network/artwork | Workspace-local Cargo edges and fail-closed bounded inputs are enforced by offline gate. Maintain no image/artwork/visualizer functionality and never modify source audio during library operations. |
| Performance | Startup, memory, search, transitions | Search/scan profile at 0/1/100/2k/20k exists. One interactive 2k-entry UI sample completed in <1 s; same-process RSS increased 1,568 KiB from the pre-scan snapshot. Cold-start distribution, repeatable incremental catalog memory, transition latency, and attenuation processing cost remain unmeasured. |
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

Still unverified: audible output (no audio device was listed by
`system_profiler SPAudioDataType`), device loss/recovery, physical DAC and
panel, Rust Boy shared Radio/TV focus, cold startup/incremental RSS, and
Developer ID/notarization. Independent Swift and security reviews found no
actionable finding after the paused-navigation fix. These operator and
performance gaps block release integration under current policy.

### Rebuilt-app UI transport and persistence smoke

In the exact rebuilt app, an 8-second, 16-bit/44.1 kHz two-track silent WAV
fixture under `/private/tmp` was explicitly loaded. Search, queue insertion and
named-playlist creation succeeded. After Cmd-Q and relaunch, reselecting the
same folder restored both queue entries and the playlist while playback
remained stopped. Explicit Play produced Playing; Pause, Resume, queue Next,
queue Previous and Stop returned the expected visible states. The volume
slider's accessibility decrement changed the displayed value 100%→99% while
stopped and 99%→98% during playback. The process list after Cmd-Q contained no
Rust Music app or `rust-music-cli` process. This validates UI and process
control paths on silent PCM, not audible output or physical-device behavior.

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

## Fresh offline, output and catalog responsiveness check — 2026-10-31

At checkout `9d1141975d5fa54c31fa2a7ce25c622c4218ebba`,
`sh scripts/offline-gate.sh` exited 0: 17 local crates, 32 path-only internal
edges, zero registry/Git crates; all workspace tests, warnings-denied Clippy,
release build, macOS feature checks and the 15/15 injected AudioQueue
failure/teardown tests passed. The built app's bundled `rust-music-cli play`
played a generated nonzero 440 Hz, stereo, 16-bit/44.1 kHz WAV and exited 0.
An unsandboxed `system_profiler SPAudioDataType -detailLevel mini` listed
MacBook Air Speakers as the 48 kHz default output, plus HDMI and headphone
outputs. This confirms the native output path; audible confirmation was
previously supplied by the operator, while no acoustic loopback or DAC capture
was made in this check.

The already-open app loaded 2,000 generated 12-byte WAV-named catalog entries;
the accessibility tree returned the 2,000-row result in under one second.
Searching `track-01999` produced exactly one visible result in 3.3 seconds
including UI automation round-trip. App PID 93393 RSS was 103,312 KiB before
scan and 104,880 KiB after scan (+1,568 KiB). This is one incremental sample
on one process, not a repeatable profile or performance threshold. Cmd-Q
closed PID 93393; the follow-up process query found it absent and no
`rust-music-cli serve` child. Fixtures remained under `/private/tmp` and were
not added to Git. Device disconnect/recovery, physical DAC measurement and
Rust Boy shared focus remain open.

### Removable-track process regression

`cargo test --locked --offline -p rust-music-cli --features
macos-output,test-support --test package_process` exited 0 (5/5). The new
spawned-process case scans a generated track, removes it before Play, observes
the bounded `local music file could not be read` error, verifies the process
still answers a snapshot as Stopped with no audio claim, then requests
Shutdown and observes a clean child exit. This covers stale media before
output starts; it does not simulate a device disappearing during active audio.

### Compact phosphor UI theme — `fb5e70dd62a348053088a8bc03e9ea5e85e42a3e`

The app now renders a clean, text-only phosphor-green player surface intended
to sit inside the outer Pip-Boy wrapper. It does not draw a second chassis and
contains no artwork, mascot, waveform, EQ bar, or image subsystem. Player
status and queue sit above library/search and playlists; transport and volume
remain in a bottom bar. `sh scripts/build-macos-app.sh`, `plutil -lint`, and
strict deep `codesign --verify` passed. Unsandboxed `open -n` exited 0 and
LaunchServices checked in the app 0.0901999 seconds after launch. Visual review
at 974×620 showed all panels and transport in the window. In the actual UI, a
generated two-track WAV folder loaded; the queue remained stopped, and playlist
create/delete both worked. The user independently confirmed audio output and
library loading. This is a UI/build check, not DAC or device-recovery evidence.

### Four-section wrapper layout — 2026-10-31

The SwiftUI surface now uses a compact left navigation rail for Player, Library,
Queue, and Playlists, with the transport and volume controls remaining fixed
below the selected section. The root `PlayerModel` remains a single state object
across navigation, so switching sections does not recreate the playback client.
The interior minimum width is 600 points to match the app scene and fit within
the outer Pip-Boy wrapper. `shutdownPackage()` now strongly retains the model
until its serialized Stop/Shutdown/reap operation completes, avoiding a weak
capture that could drop cleanup if SwiftUI released the view model immediately
after window close.

In a working-tree pass based on `8116a077`, Swift compilation to a temporary
arm64 macOS 13 executable exited 0, `git diff --check` passed, and a source scan
found no image/artwork/visualizer APIs. The generated `.app` was not rebuilt or
launched in this pass; runtime layout at 600×390 and 974×620, navigation
interaction, and the updated close-path observation remain pending. The
compile and source checks do not replace the full offline gate or app bundle
acceptance.

## Required handoff

Report exact local/remote HEAD, agent_skills SHA, task IDs/owners, code commits, actual test outputs/exit codes, Mac app/manual evidence, device and signing status, what remains deferred for v1.1/Rust Boy, and PR/mainline recommendation. Never claim release completion from compilation alone.
