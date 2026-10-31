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
| MAC-01 | Complete usable library-to-playback workflow | **PARTIAL / OUTPUT PATH VERIFIED** | Current rebuilt-app UI loaded two generated 60-second, 16-bit/44.1 kHz WAV tracks, showed search narrowing to one result without playback, then explicit Play, Pause/Resume, active Next/Previous, volume 99%→98% while active, and Stop. No queue or playlist records were changed. Earlier queue/playlist restart persistence and repeat behavior remain separately verified. The app-bundled CLI host tone exited 0; prior app UI output was operator-reported audible. No acoustic loopback or DAC bit-perfect evidence. |
| MAC-02 | Native macOS playback, shutdown and device recovery | **PARTIAL / ACTIVE CLOSE AND PHYSICAL ROUTE RECOVERY OBSERVED** | Prior active-playback close evidence shows Rust Music and its `rust-music-cli` child exiting after Cmd-Q, with no matching Activity Monitor rows after refresh. The 7/7 feature-gated subprocess cleanup suite passes. New operator-assisted unplug/reconnect of Shure headphones on the analog port during generated AudioQueue playback changed the macOS output selection from External Headphones to built-in speakers and back; Rust Music remained PLAYING throughout and surfaced no playback error. Explicit Stop reached Playback Stopped, then Quit resulted in `cua.listApps()` reporting Rust Music stopped. This verifies the physical route transition and return, not an AudioQueue invalid-device error, acoustic loopback, or DAC measurement. Developer ID/notarization remain open. |
| MAC-03 | Complete offline suite, performance and independent security/code review | **PARTIAL** | Product source candidate `a1e15e056104902cdce53abdcbd92c79ab4a0d6f` passes `sh scripts/offline-gate.sh` (exit 0; 17 local crates, 32 path-only edges, zero registry/Git crates and third-party system links) and `sh scripts/build-macos-app.sh` (exit 0; 18/18 path-guard cases, Swift UI-state check and strict ad-hoc signature validation). Focused subprocess cleanup remains 7/7 at unchanged CLI source. Five synthetic Rust catalog-core runs measured 20k build median 599 ms (344–672) and search p95 median 745.625 µs (743.125–748.292); this is not SwiftUI/full-app qualification. The current 2k UI search sample returned correct counts, but timings include CUA and accessibility overhead and have no accepted threshold. App-wide incremental memory, direct search/transition latency, device recovery, DAC/panel and distribution acceptance remain open. Existing FLAC 6/6, WAV 6/6, AIFF 7/7 and ALAC/WavPack evidence is retained; private corpus was not rerun. |
| MAC-04 | Reviewed mainline integration with preserved history | **BLOCKED BY RELEASE GATES** | Fresh GitHub read on 2026-10-31: PR #1 is open, draft and unmerged (`mergeable=true`), remote PR head `2947e6737622c3c668003130006823df0bdde3e0`, base `c02ef6c9b86cff3a86ceaaf5e35900593f592fbf`. Local HEAD `5d1a91218947a8ea11a5cb739f7759be81c4cf5b` contains both the PR head and local `origin/main` (ancestor checks exit 0); it is 97 commits ahead and 0 behind its tracking ref. The 97 commits are preserved local history; mergeability only confirms ancestry. No push or merge while release gates remain open. Preserve backup branches and stash through post-merge validation. |

### 2026-10-31 current-candidate offline and app-bundle refresh

At documentation candidate commit `0dd8f3104c0a62393632c75e9821a9f1b656cd15`
(product source unchanged from `a1e15e056104902cdce53abdcbd92c79ab4a0d6f`),
`sh scripts/offline-gate.sh` exited **0**. The audit reported 17 local crates,
32 path-only internal edges, zero registry/Git crates, zero third-party system
links, and one OS framework link. Workspace tests, warnings-denied Clippy,
release build, and macOS feature checks passed; the native macOS AudioQueue
suite passed **17/17**, including injected invalid-device enqueue recovery.
`sh scripts/build-macos-app.sh` also exited **0**, including 18/18 unsafe
symlink-path guards, playback UI-state regression, bundle construction, and
strict ad-hoc signature validation. These automated results do not establish
physical acoustic continuity or DAC fidelity; the separate headphone route
observation above only proves macOS default-route switching and return.

### 2026-10-31 candidate verification at `c1ce494`

At source commit `c1ce494fa08a1867c9441b2e15d1eb4d7d253a30`, the focused
package-process suite passed 7/7, including active-playback EOF cleanup and
the app's normal Stop-then-Shutdown protocol. The latter confirms Stop returns
`Stopped` with `audio_claimed=false`, then confirms Shutdown Ack, child exit
status 0 and stdout EOF. A release-mode filter exited 0 with zero debug-only
tests selected. `sh scripts/offline-gate.sh` exited 0 across the workspace.
`sh scripts/build-macos-app.sh`, Info.plist lint and strict ad-hoc bundle
signature verification exited 0; the Swift UI and bundled CLI are arm64
Mach-O executables.

The exact bundle was then launched by host LaunchServices:
`/usr/bin/open -n target/macos/RustMusic.app` exited 0. `lsappinfo` reported
the expected bundle ID and executable path; the
UI showed Ready / No Track with no scan or autoplay. After menu Quit, a second
`cua.listApps()` reported Rust Music stopped, and host `lsappinfo`/`ps` showed
no Rust Music or CLI process. A host `system_profiler SPAudioDataType
-detailLevel mini` query listed ASUS VG32V HDMI, External Headphones and
MacBook Air Speakers (default output), all at 48 kHz. No device was
disconnected during this pass, so physical loss/recovery remains open.

### 2026-10-31 earlier automated gate refresh

At checkout `c8e17f13557ad281813e92a52b2d5c908b39cab8`,
`cargo test --locked --offline -p rust-music-cli --features
macos-output,test-support --test package_process` exited **0** (6/6 passed,
including managed playback-error cleanup, repeated parent-pipe-close cleanup,
and the paced-null active-playback parent-close regression). The active case
observes `Playing`, `audio_claimed`, and nonzero delivered frames before closing
the IPC parent; the child exits with status 0 and stdout reaches EOF. Its
test-only sink does not prove AudioQueue output. The equivalent release-mode
test filter exited **0** with zero matching tests, keeping the debug-only hook
out of release builds. `sh scripts/offline-gate.sh` also exited **0**: all 17
local crates built and tested offline, dependency audit found 32 path-only
internal edges, zero registry/Git crates and zero third-party system links
(one OS framework link). The 15 injected AudioQueue failure/teardown tests
passed. Separately, the live GUI test above observed app and child exit during
active synthetic silent AudioQueue playback. No private corpus was rerun.

## Component matrix

| System | Requirement | State / next acceptance |
| --- | --- | --- |
| Native audio | macOS native output, safe teardown, device error/recovery | AudioQueue adapter and injected failure/cleanup tests exist (17/17 native macOS tests). During operator-assisted Shure headphone unplug/reconnect while the generated test tone was playing, macOS switched External Headphones → MacBook Air Speakers (46%) → External Headphones (93%); the app stayed in PLAYING and showed no error. Stop then quit cleanly. This verifies real route change and return through the active app path; an actual AudioQueue error callback/reopen path, measured output/acoustic capture and DAC fidelity remain unverified. |
| Decoders | Lossless-only admission and validated PCM | Native decoders and offline parity evidence are tracked by G2/G7/G8/G10/G11/G12. Retain existing evidence; run full codec gate only when release procedure requires it, and never include private media in Git. |
| Library | Explicit scan, indexing, metadata, search | Text-only explicit folder load, cancellation, catalog search and bounded metadata exist. The current library suite also rejects missing, non-directory and Unix symlink roots, and invalidates catalog paths after the selected root is deleted (38/38 library tests; full offline gate exit 0 at the current source change). Synthetic catalog RSS and app launch/search snapshots exist, but full-app incremental memory and system-boot cold startup are not qualified; physical removable-volume eject/recovery remains untested. |
| Playlists | Named persistence and M3U8 support | Bounded versioned named playlist storage and CLI/UI CRUD exist. Complete import/export and restart/error acceptance if required by v1 scope. |
| Queue | Persistent ordering, repeat and shuffle | Root-scoped persistent CLI/UI queue exists; selection does not play. The nonzero two-tone UI smoke verified active Next/Previous queue transitions. Non-Unix persistent queue behavior remains unsupported; broader repeat/shuffle playback semantics remain covered by unit tests rather than this device smoke. |
| Transport | Play, Pause, Resume, Stop, Next, Previous | Implemented over the persistent package process. Engine/host tests cover pause/resume position, lease release/reacquire, denied focus and Stop; package E2E and the two-tone UI smoke exercised transitions while native output was running. Physical device-loss recovery remains open. |
| Volume | User-visible functional native volume | Implemented as bounded software attenuation (0–100), with strict 100% integer-sample bypass. It is not hardware gain or bit-perfect processing. The app slider changed 100→95→100 during the two-tone output smoke. |
| Application | Valid bundle, LaunchServices and shutdown | Local bundle build/plist/ad-hoc code-sign validation and LaunchServices launch evidence are in PR readiness ledger. Revalidate after binary/UI changes; distribution signature/notarization remains open. |
| Security | Bounded parsers, safe paths, no network/artwork | Workspace-local Cargo edges and fail-closed bounded inputs are enforced by offline gate. Maintain no image/artwork/visualizer functionality and never modify source audio during library operations. |
| Performance | Startup, memory, search, transitions | Search/scan profiles at 0/1/100/2k/20k exist. Five earlier fresh-process launch-to-Ready samples took 753–891 ms; three latest runs took 794.6–1,120.5 ms (median 801.6 ms), all stopped with no scan/autoplay and quit observed. Activity Monitor showed 34.2 MB `anonymousMemory` in one idle snapshot while the host had high memory pressure; this is not a full-app memory pass. A 2k UI search returned correct counts 2,000/1,000/1/2/0. After one duplicate-filter removal, median CUA `setValue` time fell 451.2→165.4 ms across five queries; accessibility readback was variable and slower post-change (median 814.2→1,170.2 ms), so these are exploratory UI round-trips, not direct search latency or a threshold qualification. Four active synthetic silent-track Next/Previous transitions measured 729–1,093 ms (median 886 ms) including UI automation overhead. Synthetic catalog RSS profiles measure catalog process deltas, not full-app incremental RSS or exact heap allocation. System-boot cold start, app-wide incremental memory, direct search/transition latency and attenuation cost remain unqualified. |
| Release | Reproducible build and independent acceptance | Local offline/app build evidence exists at recorded commits. Require fresh build evidence at candidate SHA, independent code/security review, operator hardware record, clean mainline build and app launch before declaring v1.0 ready. |

### 2026-10-31 fresh-process startup and idle-memory sample

Five fresh app processes returned the empty Ready / No Track accessibility
tree in **891, 858, 753, 798 and 779 ms** (median **798 ms**, range **753–891
ms**). Every run displayed no loaded library and no autoplay, then exited
before the next run. Activity Monitor separately reported **28.2 MB** in its
`anonymousMemory` column for the idle app (PID 12236), with no `rust-music-cli`
child; after Quit, `listApps()` and the refreshed Activity Monitor filter
showed no Rust Music or CLI process. This is a five-run process-launch sample,
not a system-boot cold-start profile or performance threshold, and one idle
memory reading, not repeatable memory profiling or RSS.

### 2026-10-31 catalog memory reduction and repeated RSS delta

At code commit `7c1b1d0ba795a8af6cd0ff36bb55abf3fd087bcc`, common catalog
tracks no longer carry an inline three-String metadata struct. FLAC/AIFF
metadata is packed into a single bounded UTF-8 buffer with 16-bit field spans,
and allocated only when tags exist. Public `TrackTags` reads now use
`title()`, `artist()`, and `album()`; the previous public fields and
`Track.tags: Option<TrackTags>` layout were pre-v1 and source-incompatible with
the compact representation. All in-workspace consumers were updated and the
full workspace builds. The memory-layout tests guard current supported desktop
targets; they are not a stable ABI promise.

The new `profile_micro_library_memory` example and
`scripts/profile_catalog_memory.py` measure the same process before scanning
and after keeping the catalog alive. The driver samples `/bin/ps` three times
at each phase and reports the median; fixtures contain no user media, FLAC
frames, or playback. Three independent 20,000-track runs per scenario on the
rebuilt release profile produced:

| Metadata mix | RSS delta median | Range | 8 MiB target |
| --- | ---: | ---: | --- |
| No tags | 5,248 KiB | 5,248–5,264 KiB | Below |
| 10% tagged | 5,568 KiB | 5,536–5,584 KiB | Below |
| All tagged | 7,824 KiB | 7,792–7,872 KiB | Below |

The FLAC fixtures have bounded Vorbis comments and no audio frames. This
measurement includes any allocator-retained parser temporaries and OS page
behavior, so it is an RSS delta estimate, not exact live-catalog heap usage or
the full macOS app's incremental memory. The all-tagged case is 320 KiB under
8 MiB in these samples; confirm against real catalogs and app-level profiling
before calling the product memory target fully qualified. Cold startup,
transition latency, and app-wide RSS delta remain open.

### 2026-10-31 fresh catalog RSS sample at `01ac0e6`

The current release-profile probe was rebuilt with
`cargo build --locked --offline --release -p rust-music-library --example
profile_micro_library_memory` (exit 0), then measured using
`python3 scripts/profile_catalog_memory.py
target/release/examples/profile_micro_library_memory --counts 2000 20000
--repeats 3` (exit 0 with process-inspection access). It creates only
synthetic 12-byte WAV signatures, keeps the catalog alive between RSS samples,
and reads RSS three times at each phase. Three 2,000-entry runs measured
incremental RSS estimates **800 KiB median** (784–816 KiB); three 20,000-entry
runs measured **5,232 KiB median** (5,216–5,280 KiB). No user files or playback
were involved. These samples refresh the catalog-core measurement only; they
do not measure the SwiftUI app's retained catalog memory, real tagged-library
behavior, total application RSS, or cold launch. The full app memory and cold
startup gates remain open.

### 2026-10-31 fresh synthetic search profile at `01ac0e6`

The release example was built with `cargo build --locked --offline --release
-p rust-music-library --example profile_micro_library` (exit 0). Two serial
direct runs of `target/release/examples/profile_micro_library` exited **0**.
At 20,000 synthetic entries, catalog construction was **419 ms / 313 ms**,
search p50 was **717,000 / 688,750 ns**, and search p95 was **739,000 / 728,291
ns**. At 2,000 entries, catalog construction was **30 / 25 ms** and search
p95 was **142,458 / 77,292 ns**. The profiler's fixtures contain only 12-byte
WAV signatures; no user audio is read or played. One optional `/usr/bin/time
-l` wrapped run printed timings but exited **1** because a sandbox-denied
`kern.clockrate` query failed; the successful direct runs make no peak-RSS
claim. These are warm local synthetic process samples, not interactive SwiftUI
search latency or a performance threshold qualification.

### 2026-10-31 interactive track-transition sample at `4d11059`

The rebuilt standalone app explicitly loaded three generated 60-second silent
stereo WAV fixtures (16-bit/44.1 kHz), started Track 001 through the native
AudioQueue path, and completed four active-playback transitions: Next to Track
003, Previous to Track 002, Next to Track 003, and Previous to Track 002. Each
result showed the expected track and returned to “Playing through Rust Music
and macOS AudioQueue.” UI-click-to-settled-accessibility timings were **729.4,
1,093.1, 887.5 and 884.6 ms** (median **886.1 ms**). This includes CUA and
accessibility polling overhead and is a small sample without a selected
performance threshold. Playback was stopped, the fresh UI returned to Ready /
No Track, no CLI child remained in process inspection, and the generated
fixture directory was removed. The sample proves the interactive native path
changed tracks during silent playback; it is not audible-output, DAC, or
hardware-recovery evidence.

### 2026-10-31 repeatable synthetic library profile

At code source `a40758d53b27d63673697dc68595cb680050b5f5`, the unchanged
release example built with `cargo build --release --locked --offline
-p rust-music-library --example profile_micro_library` (exit 0). Two serial
runs of `/usr/bin/time -l target/release/examples/profile_micro_library`
exited 0. They create only 12-byte RIFF/WAVE-signature synthetic files; no
audio payload is generated, read, or played. At 20,000 entries the runs
measured catalog construction of **312 ms / 310 ms**, search p50 of
**702,792 / 691,541 ns**, search p95 of **756,250 / 750,875 ns**, and fixture
creation of **1,394 / 1,193 ms**. Whole-process maximum RSS was
**11,321,344 / 11,223,040 bytes**; `time -l` peak footprint was
**10,682,728 / 9,748,840 bytes**. These include the executable, fixture
creation and all profile sizes, so they are not incremental catalog-memory
measurements and are not an under-8-MiB catalog pass. Two deliberately parallel
runs took 2.17 / 2.33 s for catalog construction, demonstrating contention
sensitivity; one first serial run after the build took 964 ms. Treat the two
later serial samples as the repeatable warm-run observation, not a hard
performance guarantee. Cold system startup, incremental catalog RSS, and
transition latency remain unmeasured.

## Operating constraints

### 2026-10-31 repeat-boundary regression and final automated verification

At source commit `31d0059306ed85b7d1fcf185b4c5bc68a6c61bb2`, queue Next and
Previous report whether a candidate exists at Repeat Off boundaries. The
standalone app now keeps the current selection at a manual boundary and shows
Finished after natural completion of the final item rather than restarting it.
`cargo test --locked --offline -p rust-music-cli --features
macos-output,test-support` passed on two consecutive runs: each had 19/19 unit
tests and 7/7 subprocess tests. The test fixture path race exposed during the
first run was fixed by unique atomic sequence suffixes in
`crates/rust-music-cli/tests/package_process.rs`.

The rebuilt app passed `sh scripts/build-macos-app.sh`, Info.plist lint, and
`codesign --verify --deep --strict`; LaunchServices `open -n` exited 0. A
synthetic silent short-track UI smoke observed Repeat Off finishing, Repeat
One continuing, Repeat All looping, and Stop returning to stopped controls.
On quit, app and playback-child processes were absent from refreshed process
and LaunchServices checks. `sh scripts/offline-gate.sh` exited 0 at this same
source commit. No private codec corpus was rerun. These observations do not
cover forced termination, an injected live playback error, device disconnect
and recovery, physical DAC/panel output, or live Rust Boy/Radio/TV contention;
those remain open acceptance gates.

### 2026-10-31 subprocess force-close and playback-error verification

At checkout `7467753d503419e03991886b334044a645dc3783`, rebuilding the debug
CLI with `cargo build --locked --offline -p rust-music-cli --features
macos-output,test-support` exited 0. Then
`python3 scripts/test_macos_parent_exit.py --binary target/debug/rust-music-cli`
exited 0 and observed normal EOF exit 0, malformed-protocol exit 1, three
repeated launch/EOF/exit cycles, and exit of the real serve child after its
app-like parent was SIGKILLed. The package subprocess regression command
`cargo test --locked --offline -p rust-music-cli --features
macos-output,test-support --test package_process` also exited 0 (7/7), including
managed playback-error cleanup, active-playback parent-pipe close, and
Stop/Shutdown child exit. This establishes process-level cleanup behavior
with the real CLI child and test output sink; the SIGKILL harness is not an
end-to-end forced termination of the SwiftUI app while AudioQueue has an open
device. Physical device loss/recovery and OS-level active-playback force-close
remain unverified.

### 2026-10-31 actual app SIGKILL during silent playback

Using only the generated silent fixtures under
`/private/tmp/rust-music-repeat-regression`, the rebuilt app loaded two
16-bit/44.1 kHz WAV tracks and visibly entered
`PLAYING … THROUGH RUST MUSIC AND MACOS AUDIOQUEUE`. Host process inspection
identified app PID 16381 and its `rust-music-cli serve` child PID 16556. The
app was sent SIGKILL; both PIDs became non-existent within the 8-second
deadline, and the CUA app inventory reported `dev.rustmusic.standalone` as not
running. The app was launched from the current committed code tree
`a2614087e032f34c88d555e0f1045958b573b39a` (the intervening commits since the
tested app binary changed documentation only). This directly verifies the
SwiftUI parent/CLI child close path during AudioQueue playback on silent PCM;
it does not test a physical device disconnect or acoustic output.


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
showed bundle `target/macos/RustMusic.app`,
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

After commit `31cb4cb3d8a2855f5a1132085371ae22871ef151`,
`python3 scripts/test_macos_parent_exit.py --binary target/debug/rust-music-cli`
exited 0. It observed real serve-child PIDs reaped after normal EOF (exit 0),
malformed protocol (exit 1), three repeated launch/exit cycles, and forced
termination of an app-like parent. These tests do not launch the SwiftUI app or
prove its window-close callback ran; that app-level observation remains
pending.

At HEAD `258ecec8d15f392e5528ca098eda700cba9913bc`,
`sh scripts/offline-gate.sh` exited 0. The audit reported 17 local crates,
32 path-only internal edges, zero registry/Git crates, and zero third-party
system links. Workspace tests, release build, and the 15 macOS AudioQueue
fault/teardown tests passed. The gate does not compile the SwiftUI app or
certify a physical audio device; the separate temporary Swift compilation
above covers source compilation only.

At merge commit `f8052f28cf187143b54aa15e80779ff3b8db841f`, an initial run of
`python3 scripts/test_macos_parent_exit.py --binary target/debug/rust-music-cli`
exited 1 because the pre-existing debug executable exited before the RBPK hello
reply. After `cargo build --locked --offline -p rust-music-cli --features
macos-output` exited 0, the same subprocess-cleanup script exited 0: it observed
real child exits for normal EOF (0), malformed protocol (1), three repeated
launch/exit cycles, and forced termination of an app-like parent. This verifies
the CLI process lifecycle, not the SwiftUI window-close callback; app-level
close observation remains unverified.

At source HEAD `51216085b560d2bb5d2d87929d92c0398d074538`, a separate temporary
preview bundle assembled from the compiled current SwiftUI executable and the
offline release CLI passed `plutil -lint` and strict deep ad-hoc
`codesign --verify`; it was not launched and did not replace the normal app.
Both executables are arm64 Mach-O. `vtool` reports the app minimum macOS 13.0
and CLI minimum 11.0; the bundle identifier is `dev.rustmusic.standalone` and
`LSMinimumSystemVersion` is 13.0. The measured executable SHA-256 values were
`12351149b134390ef46d6f7c6dd2b2b2b9ca042115e281b3cdade25024a9cfb2` (app) and
`7be9853bd6eb9f6cc1706d3fcd3d6065fdfaec223be9f22d695e9e97ede242ce` (CLI).
This is a staged package-structure/signature check only; production build
script execution, LaunchServices launch, visible layout and app-close cleanup
remain pending.

### Fresh automated gates and packaged-child lifecycle — `b03c592ec67d102325c4b0af0adc93acc71e9651`

On the clean checkout at this exact HEAD, `sh scripts/offline-gate.sh`
exited **0** (17 local crates, 32 path-only edges, zero registry/Git crates,
zero third-party system links, all workspace tests/builds and macOS feature
checks; injected AudioQueue failure/teardown tests passed 15/15).
`sh scripts/build-macos-app.sh` exited **0**; the generated local bundle passed
strict ad-hoc signature validation. `python3 scripts/check_roadmap_index.py`,
`python3 scripts/check_literature.py`, and `git diff --check` each exited **0**.

The packaged `Contents/MacOS/rust-music-cli` was exercised directly.
`cargo test --locked --offline -p rust-music-cli --features
macos-output,test-support --test package_process` passed **5/5** on the default
parallel run (0.24 s); an immediately preceding parallel attempt timed out in
all five cases, while a serialized rerun passed 5/5 in 0.28 s. Preserve that
transient failure in the evidence rather than treating it as a clean-only
history. `python3 scripts/test_macos_parent_exit.py --binary
target/macos/RustMusic.app/Contents/MacOS/rust-music-cli` exited **0** and
observed normal EOF exit 0, malformed-protocol exit 1, three repeated
launch/exit cycles, and a real serve child exit after its app-like parent was
SIGKILLed. `python3 scripts/test_release_package_process.py --binary
target/macos/RustMusic.app/Contents/MacOS/rust-music-cli` exited **0**; the
bundled CLI SHA-256 was
`2f4adc33ef8a29c0b282c34307f6e0b33530d775d14dc73e2aa5c9ed417d655e`.

These are packaged-child cleanup checks, but they do **not** observe the
current SwiftUI app closing its own child after Cmd-Q/window close. The native
folder picker remained open during this pass and was left untouched. Device
disconnect/recovery, physical output measurement, live host audio focus, and
release authorization remain unverified.

### Current-candidate UI, idle-child shutdown and startup — 2026-10-31

After the user closed the folder picker, the already-running rebuilt app showed
the loaded library and `PLAYBACK FINISHED`; no track was started or changed.
Activity Monitor listed Rust Music PID 95299 and its `rust-music-cli` child PID
95500. Cmd-Q closed that instance. During verification, CUA's `getApp()`
automatically started a fresh instance (PID 9117) with an empty library; the
refreshed Activity Monitor list no longer contained the original app or CLI
child. A subsequent Quit produced `cua.listApps()` `isRunning=false` and an
Activity Monitor `rust` filter with no Rust Music or CLI rows. This is current
app idle-child cleanup evidence; playback was already finished, so close during
active output remains unverified.

Reopening the same bundle produced Rust Music PID 9188 in Activity Monitor,
with no `rust-music-cli` process. The UI showed Ready / No Track, an explicit
Load Library button, and disabled transport until a user loads a folder and
presses Play. Runtime navigation reached Player, Library, Queue and Playlists;
the screenshot showed the phosphor-green interior within macOS window chrome,
with transport and software volume in the bottom bar and no artwork or
visualizer. This runtime check did not scan media.

## Required handoff

Report exact local/remote HEAD, agent_skills SHA, task IDs/owners, code commits, actual test outputs/exit codes, Mac app/manual evidence, device and signing status, what remains deferred for v1.1/Rust Boy, and PR/mainline recommendation. Never claim release completion from compilation alone.

### 2026-10-31 native Pip-Boy chassis visual pass

`apps/rust-music-macos/RustMusicApp.swift` now frames the existing functional
player in a static native SwiftUI Pip-Boy style chassis: muted brass trim,
device label, and four small fasteners. The catalog, navigation and transport
controls remain the existing text UI. There are no image assets, album art,
audio-reactive bars, or visualizers.

At this pass's Swift source, `sh scripts/build-macos-app.sh`
exited **0** and strict deep ad-hoc code-signature validation passed. A
temporary copy at `/private/tmp/RustMusic-theme-smoke.app` launched through CUA;
the accessibility tree displayed `RUST MUSIC PIP-BOY 2000 · AUDIO`, and the
screenshot showed the chassis around the Ready / No Track screen. This visual
smoke did not load a library or start playback.

At source commit `d65efc16f5e694ba40acdac6b4662ae94542b88d`,
`sh scripts/offline-gate.sh` exited **0**: 17 local crates, 32 path-only
internal edges, zero registry/Git crates, zero third-party system links, and
one OS framework link. Formatting, warnings-denied Clippy, workspace tests,
release build, and macOS feature checks passed. This gate does not resolve the
separate LaunchServices shell-launch result below.

The shell invocation `/usr/bin/open -n` for both the built bundle and the
temporary copy exited **1** with LaunchServices `kLSNoExecutableErr` (-10827),
despite `CFBundleExecutable=RustMusic`, the arm64 executable being present and
executable, `plutil -lint` passing, and strict deep `codesign --verify`
passing. A direct executable invocation exited **134**. Attempted
`lsregister -f` exited **1** with a Spotlight scan error (-10822). CUA launch of
the temporary app copy worked, but the shell `open -n` path remains
**UNVERIFIED** and must be resolved before accepting LaunchServices packaging.

The packaged child smoke
`python3 scripts/test_release_package_process.py --binary
target/macos/RustMusic.app/Contents/MacOS/rust-music-cli` exited **0**. It
completed RBPK v1 Hello, Activate with an empty view, Shutdown acknowledgment,
and child exit; binary SHA-256 was
`2082adacc6b1e94abe44c808cf23f1b28f46a5b1be817b0d1b4f11f2312fe401`.

### 2026-10-31 live PR ancestry check

The connected GitHub read of PR #1 reports it open, draft, unmerged and
`mergeable=true`, with remote head `2947e6737622c3c668003130006823df0bdde3e0`
and base `c02ef6c9b86cff3a86ceaaf5e35900593f592fbf`. At local source
`9ef316d502f04b916dfbea15b2f875f6dedc6d49`, the PR head object is present and
is an ancestor of the local branch. The command
`git rev-list --left-right --count
2947e6737622c3c668003130006823df0bdde3e0...HEAD` reported `0 44`. Thus all
commits on the fetched PR head are in local history, followed by 44 local
commits. `mergeable=true` only
describes GitHub's mergeability result; it does not satisfy release acceptance.
No push or merge was performed. Hardware acceptance and the LaunchServices
shell-launch result remain open.

### 2026-10-31 host LaunchServices revalidation

At SwiftUI source commit `d65efc16f5e694ba40acdac6b4662ae94542b88d`, the exact
bundle was launched with `/usr/bin/open -n` through the unsandboxed host
command path; it exited **0**. This supersedes the earlier restricted-shell
`kLSNoExecutableErr` observation, which did not establish host LaunchServices
behavior. CUA showed the Pip-Boy chassis UI at Ready / No Track, with no scan
or playback. Host `ps` inspection showed Rust Music process IDs **20761** and
**21395** running the expected bundle executable and no `rust-music-cli` child.
Thus exact-path LaunchServices launch and the idle no-child state pass; this
does not test playback output, device loss/recovery, or normal-close child
teardown.

`system_profiler SPAudioDataType -detailLevel mini` exited **0** and enumerated
the built-in speakers and headphones plus a two-channel HDMI output, all
reported at 48 kHz; the built-in speakers were the default output. This is a
current device-inventory observation only. It does not establish Music's
AudioQueue playback on each route or device-disconnect recovery.

An independent read-only `safety_reviewer` review of the SwiftUI chassis and
launch/inventory evidence found no blocker: the wrapper is static SwiftUI,
preserves the existing transport and shutdown callback, and adds no assets,
audio-reactive visualization, or dependency. The review did not run tests or
recheck runtime state; empty-state screenshot coverage does not qualify
active-playback close, physical output, device recovery, DAC, or panel
acceptance.

### 2026-10-31 synthetic app scan and RSS trace

The built app explicitly scanned a temporary catalog of 2,000 generated
12-byte RIFF/WAVE-signature files. CUA reported `LOADED 2000 TRACKS. PLAYBACK
IS STOPPED.` The Library view showed `2000 TRACKS`. In a preceding run with
the same fixture shape, searching `synthetic-01999` returned one row; a single
set-value/accessibility round trip took 1,334 ms, including CUA and
accessibility observation overhead. This is not an isolated search-latency
result or a p95 measurement. No fixture was played, and both generated
directories were removed afterward.

For process PID 22043, five idle `ps` RSS samples before loading were 96,368,
96,400, 96,400, 96,384 and 96,384 KiB (median 96,384 KiB). After the catalog
was held live, the immediate sample was 84,656 KiB; five samples over the next
five seconds were 69,472, 69,440, 69,440, 69,392 and 69,360 KiB (median
69,440 KiB). RSS fell by 26,944 KiB from the pre-scan median while the 2,000
tracks remained visible. The process reclaimed other resident pages during
the sample, so this is **not a valid incremental catalog-memory delta** and
does not pass the app-wide memory gate. Cold-start/app RSS qualification still
needs a measurement method that separates transient page reclamation from
catalog allocation. Playback was not started; Cmd-Q removed the tested app
process, and the synthetic fixture directory was deleted.

### 2026-10-31 debug subprocess cleanup regression

The first `python3 scripts/test_macos_parent_exit.py --binary
target/debug/rust-music-cli` attempt exited **1** because the existing debug
binary exited before its Hello reply. Rebuilding with
`cargo build --locked --offline -p rust-music-cli --features
macos-output,test-support` exited **0**; rerunning the harness exited **0**.
It observed normal EOF child PID 22405 exit 0, malformed-protocol child PID
22406 exit 1, three repeated launch/exit children 22407–22409, and child PID
22411 exit after its app-like parent was force-killed. A subsequent host
`ps -p 22405,22406,22407,22408,22409,22411 -o pid,ppid,args` returned exit **1**
with only the header, confirming none of those PIDs remained. This is
CLI-child lifecycle evidence with the injected test sink; it does not add
physical-device recovery or audible-output evidence.

At source commit `eb69a8c6ef0f8c91d5c68294c5cb2faaf0940adb`,
`cargo test --locked --offline -p rust-music-cli --features
macos-output,test-support --test package_process` exited **0** with **7/7**
tests passing: wrong package identity, inert handshake/shutdown, removed-track
rejection, managed playback-error cleanup, repeated parent-pipe close,
active-playback parent-pipe close, and active-playback Shutdown exit. These
use test support and do not establish physical device recovery.

### 2026-10-31 current offline gate, package build and graceful active-playback quit

At source HEAD `be6615e802b24c42da3d2b5d89906da52938c110`,
`sh scripts/offline-gate.sh` exited **0** across 17 local crates and 32
path-only internal edges, with zero registry/Git crates and zero third-party
system links. The focused command
`cargo test --locked --offline -p rust-music-cli --features
macos-output,test-support --test package_process` exited **0**, with **7/7**
tests passing, including playback-error and active-playback parent cleanup.
`sh scripts/build-macos-app.sh` exited **0**; Info.plist validation and strict
ad-hoc signature validation passed. Roadmap/literature validators and
`git diff --check` also exited **0**. No private corpus was rerun.

The rebuilt app explicitly loaded only the two generated silent test WAVs
`01.wav` and `02.wav`, displayed `PLAYING THROUGH RUST MUSIC AND MACOS
AUDIOQUEUE`, then was quit from the Rust Music menu. Activity Monitor first
showed child PID 22787; its process inspector then reported
`rust-music-cli (22787) (Terminated)` and zero real/private memory. The
refreshed process list filtered by `Rust` showed no Rust Music app or CLI
process (the remaining matches were Apple `trustd` processes). This is
observable graceful app-quit cleanup during synthetic silent playback. No
private media was selected. Device disconnect/recovery, live Rust Boy focus,
physical DAC/panel, app-wide incremental-memory qualification, and
Developer ID/notarization remain open.

### 2026-10-31 Activity Monitor sample after a generated catalog scan

The signed app bundle built from code at
`be6615e802b24c42da3d2b5d89906da52938c110` (carried unchanged through
documentation-only commit `96fae343145cce4171934ad3fbed9faffc68fe9b`) was
launched as PID 23091. Activity Monitor's `anonymousMemory` column showed
84.1 MB before scanning. The app explicitly scanned 2,000 generated 12-byte
RIFF/WAVE-signature files and displayed `LOADED 2000 TRACKS. PLAYBACK IS
STOPPED.`; the same PID then showed 87.3 MB, a single observed increase of
3.2 MB. No generated file was played. This is one before/after
`anonymousMemory` pair, not RSS, exact catalog heap, repeated profiling, or
acceptance against a performance threshold. After choosing Quit Rust Music,
Activity Monitor's refreshed `Rust Music` filter showed no app or CLI rows; the
generated fixture directory was removed. This does not close the app-wide
memory gate.

### 2026-10-31 footprint sampler attempt during explicit catalog load

To improve on the Activity Monitor sample, `/usr/bin/footprint -p 23269
--sample 1 --sample-duration 12 --noCategories` exited **0** while the same
app process was observed before and after an explicit 2,000-entry generated
catalog scan. The 12-second output ranged from **42 MB** to **96 MB**; it
reported a `phys_footprint_peak` of 96 MB and later settled at 43 MB. The
folder picker and scan occurred during the series, so the samples do not
separate picker, scan, transient UI allocation, and retained catalog memory.
This is not a usable incremental-memory delta or threshold result. A separate
five-second idle series before opening the picker held at 37 MB, further
showing that this short method was not repeatable enough to qualify the app.
After selecting Quit Rust Music, Activity Monitor's refreshed process filter
showed no app or CLI row; generated fixtures were removed.

### 2026-10-31 repeated app-level catalog footprint at byte resolution

The app's `ScanLimits::default()` admits at most 2,048 tracks; the 20,000-file
UI probe returned `RUST_MUSIC: AUDIO RESOURCE LIMIT: MUSIC LIBRARY TRACK
BUDGET EXCEEDED` and did not begin playback. Thus the UI cannot load the 20k
profile size through its default scan path.

For a 2,000-entry generated catalog of 12-byte RIFF/WAVE-signature files, a
fresh app process (PID 23814) was sampled for five seconds before and after
the picker closed and the scan completed. The last two settled idle
`phys_footprint` observations were 28,296,080 bytes; five post-scan observations
were 34,046,864 bytes each. The paired difference is **5,750,784 bytes
(5.48 MiB)**. The tool's `Footprint` column changed from 28,132,216 to
34,014,096 bytes (**5,881,880 bytes; 5.61 MiB**). Two additional fresh-process
paired scans showed rounded settled increases of 7 MB and 6 MB. These runs
loaded the generated tracks in the UI and kept playback stopped; no audio was
played. Fixtures had no PCM payload or real metadata. This provides repeatable
2k synthetic app-footprint evidence below 8 MiB, but does not qualify a 20k
app catalog, real tagged libraries, cold startup, exact live heap, or DAC and
device behavior; app-scale memory acceptance remains partial.

The same 20k folder is above the UI's configured 2,048-track default, so the
standalone app's 20k memory/search behavior remains unmeasured. The separate
Rust library example still profiles 20k entries; that is not a substitute for
an app-level scan.

### 2026-10-31 repeated 2k UI search responses

On the same rebuilt app code, the explicit 2,000-entry generated catalog
returned the expected result counts for `synthetic-01999` (1),
`synthetic-00000` (1), `synthetic-0199` (10), `synthetic-01` (1,000), and
`no-such-track` (0). Five measured field-set plus accessibility-state
round-trips took **854.0, 861.4, 1,099.2, 1,165.0 and 1,442.8 ms** (median
**1,099.2 ms**, range **854.0–1,442.8 ms**). These include CUA and accessibility
overhead; they are not direct search-function latency or a p95 qualification.
The empty result was visible at the final query. No track was played; the app
was quit and the generated directory removed.

### 2026-10-31 native transport and sandboxed LaunchServices recheck

At source HEAD `713c7e90d28d12f4cc1c002c02c4355847f6bcb1`,
`sh scripts/build-macos-app.sh` exited **0** and strict ad-hoc bundle
signature verification passed. The rebuilt bundle has a valid plist whose
`CFBundleExecutable` is `RustMusic`; the arm64 Mach-O exists at that path and
`codesign --verify --deep --strict` exited **0**. In this sandboxed shell,
`/usr/bin/open -n target/macos/RustMusic.app` still exited **1** with
LaunchServices `kLSNoExecutableErr` (-10827), and
`lsregister -f` failed to scan the bundle with -10822. Directly invoking the
executable exited **134**; the matching crash report records SIGABRT inside
AppKit application registration. The desktop CUA launcher opened that same
rebuilt bundle successfully to Ready / No Track. This isolates the observed
failure to the shell/sandbox LaunchServices path but does not prove its root
cause or close distribution launch acceptance. An earlier unsandboxed local
`open -n` success is recorded above and remains valid for that environment.

An approved outside-sandbox retry of the same absolute-path `/usr/bin/open -n`
exited **0**. The app showed Ready / No Track, with transport controls disabled;
Activity Monitor showed the Rust Music app and no `rust-music-cli` child. This
confirms the bundle's LaunchServices launch in the unsandboxed host. The
sandbox-specific `open` and `lsregister` failures are not package launch
failures; Developer ID signing and notarized distribution remain unverified.

Using only two generated sparse, all-zero 16-bit/44.1 kHz, 10-minute WAVs, the
UI explicitly loaded two tracks and showed active macOS AudioQueue playback.
Settled accessibility states confirmed Next to track 002 and Previous to
track 001, including a repeated Next/Previous pair, while playback remained
active. Pause showed the paused state and released-focus message; Resume
returned to active AudioQueue playback; Stop settled to Playback Stopped.
After Cmd-Q, the CUA app inventory reported `isRunning=false`; Activity
Monitor's refreshed `Rust` filter showed no Rust Music app or CLI child (the
initial Activity Monitor list was stale). Temporary fixtures and copied app
bundle were removed. These silent fixtures verify transport and process
cleanup, not audible output, device disconnect/recovery, or physical DAC
behavior. The volume slider was not qualified in this run.

### 2026-10-31 macOS build-path containment follow-up

Building on source parent `c2dff01015a06189317da01fd844786aa11af8eb`, the final
candidate pins `CARGO_TARGET_DIR` to the repository's guarded `target`
directory even when its caller supplies an external override. Its preflight
refuses symlinked Cargo metadata outputs (`.rustc_info.json`, `CACHEDIR.TAG`, and
`.future-incompat-report.json`) and checks for nested symlinks below release,
app-output, and Swift module-cache trees. The adversarial path test passed
18/18 cases with an external sentinel unchanged. The independent read-only
security follow-up found no additional path-containment finding in this scope;
it does not cover concurrent path replacement or broader app security.

`CARGO_TARGET_DIR=/private/tmp/rust-music-external-target sh
scripts/build-macos-app.sh` exited **0**, including the Swift playback-state
regression and strict ad-hoc signature verification. `sh
scripts/offline-gate.sh` exited **0** across 17 local crates and 32 path-only
edges, with zero registry/Git crates and zero third-party system links.
Roadmap-index and literature validators and `git diff --check` also exited
**0**. These checks do not change the open physical-device, host-focus,
distribution-signing, or app-wide performance acceptance states.

### 2026-10-31 current catalog-core scan/search profile

At HEAD `4afaf1baab544212c43eb5433298c1f82801c7b6`,
`cargo build --locked --offline --release -p rust-music-library --example
profile_micro_library` exited **0**. Two direct runs of
`target/release/examples/profile_micro_library` also exited **0**; three
additional runs of that same binary exited **0**. The helper
creates temporary directories containing only synthetic 12-byte WAV
signatures; it does not read user media or play audio.

| Tracks | Catalog build median ms (range) | Search p50 median µs (range) | Search p95 median µs (range) |
| ---: | ---: | ---: | ---: |
| 0 | 0 (0–0) | 0.083 (0.083–0.083) | 0.084 (0.084–0.125) |
| 1 | 0 (0–0) | 0.166 (0.125–0.167) | 0.709 (0.292–7.583) |
| 100 | 1 (1–3) | 3.917 (3.833–4.375) | 5.875 (5.083–5.917) |
| 2,000 | 29 (25–174) | 71.625 (68.916–72.042) | 78.125 (74.250–87.833) |
| 20,000 | 599 (344–672) | 683.834 (676.167–695.125) | 745.625 (743.125–748.292) |

The 20k build-time and one-track p95 spreads are substantial. These measure
the Rust catalog core, not SwiftUI filtering/rendering, app startup, full-app
memory, or real tagged-library behavior; those MAC-03 measurements remain open.

### 2026-10-31 host AudioQueue output check

The host-level `system_profiler SPAudioDataType -detailLevel mini` query exited
**0** and listed MacBook Air Speakers as the default output at 48 kHz, plus
ASUS VG32V HDMI and External Headphones. A temporary generated 0.25-second,
stereo, 16-bit, 44.1 kHz, 440 Hz low-amplitude WAV was played with the
app-bundled `rust-music-cli play` command outside the restricted shell; it
exited **0** and the fixture was removed. No user media was accessed. This
observes native queue playback on the available default route, not measured
acoustic output, DAC sample fidelity, other-device negotiation, or device
disconnect/recovery.

At source HEAD `9a86a5a1de79fcb40cb97f3628f3de8bf735b1be`, the focused
feature-gated subprocess command
`cargo test --locked --offline -p rust-music-cli --features
macos-output,test-support --test package_process` exited **0** (7/7). It
covers wrong package identity, inert activation/shutdown, removed-track
rejection, playback-error cleanup, repeated parent-pipe closure, active
playback parent closure, and shutdown during active playback. These use the
test support sink and complement, but do not replace, the native host output
check above.

The same command in the restricted shell exited **1** with AudioQueueStart
OSStatus `-66680`, which Apple's Audio Toolbox documentation identifies as
`kAudioQueueErr_InvalidDevice` (see [Apple's result code](https://developer.apple.com/documentation/audiotoolbox/kaudioqueueerr_invaliddevice)).
The host-level retry passed. This indicates an environment-specific output
restriction; the error is preserved rather than counted as a product pass or
ignored. Physical device loss/recovery remains untested.

### 2026-10-31 exact-history offline and app-build refresh

At local pre-recording HEAD `5da93936b87486d8eca057f6e06c11ae5fd8281c`,
`sh scripts/offline-gate.sh` exited **0** across 17 local crates and 32
path-only edges, with zero registry/Git crates and zero third-party system
links. The same command sequence rebuilt the macOS bundle, reran the 18-case
build-path test and Swift playback-state regression, and passed strict ad-hoc
signature verification (all exit **0**). Roadmap-index, literature, and
`git diff --check` validation passed. This history adds only release evidence
documents after code candidate `ab829b3`; no product source changed. The
focused feature-gated subprocess suite remains 7/7 at unchanged CLI code.

### 2026-10-31 rebuilt-app library/transport and active-close smoke

Using the rebuilt bundle from the code tree verified at `5da9393` (no app or
Rust source changed through the current local head), CUA loaded two generated
60-second, stereo, 16-bit/44.1 kHz WAV tones. The library showed both tracks;
search for the second title returned one row and the Player remained Ready / No
Track. Explicit Play showed active macOS AudioQueue output, Pause and Resume
showed their expected states, Next and Previous changed tracks while playback
remained active, a volume decrement changed 99% to 98% during playback, and
Stop reached Playback Stopped. Queue and playlist data were not changed; their
list operations were read-only. The fixtures were removed after the test.

Activity Monitor showed Rust Music PID 28923 and its `rust-music-cli` child PID
33396 during active playback. After Cmd-Q, a fresh Ready / No Track process
appeared as PID 33445 with no CLI child. After closing that idle instance,
`cua.listApps()` reported Rust Music `isRunning=false` and Activity Monitor's
refreshed Rust filter contained neither Rust Music nor `rust-music-cli`. This
verifies current GUI child cleanup during generated playback; it is not device
disconnect/recovery, acoustic capture, or DAC fidelity evidence.

### 2026-10-31 current-HEAD offline and bundle gate refresh

At clean local HEAD `d46599ccea84ddb5739d7098a750d809b67bc624`,
`sh scripts/offline-gate.sh` exited **0**. It audited 17 workspace crates,
32 path-only internal dependency edges, zero registry/Git crates and zero
third-party system links; all workspace tests, Clippy, formatting and release
build stages completed. `sh scripts/build-macos-app.sh` exited **0**, including
18/18 symlink-path refusal cases, the Swift playback-state regression and
strict ad-hoc bundle signature verification. `python3
scripts/check_roadmap_index.py`, `python3 scripts/check_literature.py`, and
`git diff --check` passed. The rebuilt app opened to Ready / No Track without
scanning or playback. This refresh adds no native device-disconnect/recovery,
physical DAC, live Rust Boy focus, Developer ID signing or notarization
evidence. A shell process query was denied by the execution sandbox, so this
pass makes no new process-exit claim; retain the separate Activity Monitor/CUA
active-playback close evidence above.

After the evidence-only commit at source-equivalent HEAD
`0e2bc7837b63a07e88eaa080c16381034e9630e2`, `cargo test --locked --offline -p rust-music-cli --features
macos-output,test-support --test package_process` exited **0** (7/7). This
includes playback-error cleanup, repeated parent-pipe closure during active
playback, active-playback parent closure, and acknowledged shutdown/child
exit. These subprocess regressions complement, but do not replace, the GUI
close observation above.

### 2026-10-31 repeated rebuilt-app launch and idle-memory sample

The bundle built from source candidate `d46599c` was opened and quit three
times through the native UI. Time from `cua.getApp("RustMusic")` to the
accessible Ready / No Track state was 1,120.5 ms, 801.6 ms and 794.6 ms
(median 801.6 ms, range 794.6–1,120.5 ms). Every run remained stopped with no
library scan or autoplay, and the app inventory reported `isRunning=false`
after Quit. Activity Monitor initially showed PID 34617 at 34.2 MB in its
`anonymousMemory` column and the related ThemeWidgetControlViewService at
10.1 MB. After Quit and process-list refresh, neither row remained; no
`rust-music-cli` row was present. The host was under memory pressure (16 GB
physical memory, 8.52 GB swap), so the single anonymous-memory value is
context only, not a full-app memory qualification. The launch timings include
CUA/Accessibility overhead and are not system-boot cold-start measurements.

### 2026-10-31 SwiftUI search render and headphone-route check

Commit `a1e15e056104902cdce53abdcbd92c79ab4a0d6f` changes the library panel
to compute `visibleTracks` once per render and share that result between the
count and list. This removes a duplicate full-catalog filter without a
persistent cache or per-track memory overhead. `sh scripts/build-macos-app.sh`
and `sh scripts/offline-gate.sh` both exited **0** after the change. A native
UI scan loaded 2,000 generated 46-byte WAV fixtures and did not start
playback. Five searches returned the expected counts 2,000, 1,000, 1, 2 and
0. Across the same query set, CUA `setValue` call medians changed from 451.2
ms before the change to 165.4 ms after it; accessibility readback medians
were 814.2 ms before and 1,170.2 ms after. Both timings include automation,
UI and accessibility work, varied substantially, and do not isolate search
computation. Treat the lower action time as directional evidence only, not a
qualified search-performance pass.

For the operator-offered device check, System Settings showed `External
Headphones / Headphone port` selected; no output settings were changed. The
app explicitly played a generated 60-second stereo, 16-bit/44.1 kHz, 330 Hz
tone at 983/32767 peak amplitude (about -30.5 dBFS) with app software volume
at 79%, and showed “PLAYING THROUGH RUST MUSIC AND MACOS AUDIOQUEUE.” Playback
later reached `PLAYBACK FINISHED` before a physical unplug/reconnect was
observed, so this does **not** count as device-loss or recovery evidence. The
synthetic fixture directories were removed and Rust Music was quit. Physical
disconnect/reconnect was later tested during active playback; see the next
section. That later route test did not trigger an AudioQueue error state.

### 2026-10-31 active headphone-route loss and return

With Shure headphones connected to the analog port, System Settings showed
External Headphones selected at 93%. Rust Music explicitly played a generated
120-second stereo, 16-bit/44.1 kHz 330 Hz tone (peak sample 983/32767, about
-30.5 dBFS). While the app reported PLAYING through AudioQueue, the operator
unplugged the headphones. System Settings changed the selected route to
MacBook Air Speakers at 46%; Rust Music continued to report PLAYING and showed
no error. After the operator reconnected the headphones, System Settings
showed External Headphones selected again at 93%, and Rust Music still
reported PLAYING. The app was explicitly stopped (UI showed Playback Stopped)
and quit (`cua.listApps()` reported `isRunning=false`). No system output
settings were changed by the test; the observed per-route selection/volume
changes followed the physical jack state. The synthetic file was removed.

This demonstrates an active macOS route change and return without a visible
transport error or process restart. It does not prove uninterrupted acoustic
output, negotiated PCM at either route, a physical DAC response, or the
AudioQueue invalid-device error/reopen path: the OS followed the default route
automatically and no output capture was made. Keep those acceptance items
open.

### 2026-10-31 current-bundle LaunchServices retry

The bundle rebuilt at `0dd8f3104c0a62393632c75e9821a9f1b656cd15` has
`CFBundleExecutable=RustMusic`, an executable arm64 Mach-O at the declared
path, and a valid strict ad-hoc signature. Sandboxed
`/usr/bin/open -n target/macos/RustMusic.app` exited **1** with
`kLSNoExecutableErr` (-10827). Retrying `/usr/bin/open -n` against the same
bundle through the host context exited **0**. The resulting
UI showed Ready / No Track, no scan or autoplay, and disabled transport
controls. Activity Monitor's Rust filter showed no playback child. This
reproduces the known shell-sandbox LaunchServices limitation and confirms a
fresh host-context launch of the current bundle; it does not establish
Developer ID signing, notarization, or distribution acceptance.
