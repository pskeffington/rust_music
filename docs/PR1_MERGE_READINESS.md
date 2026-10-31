# PR #1 merge-readiness and evidence gate — 2026-10-31

**Current decision: scripted local checks pass; PR #1 is not release-merge eligible.** The private FLAC, WAVE, AIFF and offline Rust gates pass locally. Rust_Boy live MUSIC/RADIO/TV acceptance, physical panel/DAC acceptance, distribution signing and detached release approval remain open. Shure headphone unplug/reconnect now has active AudioQueue route-transition evidence, but no measured acoustic continuity or invalid-device failure/reopen was observed. A GitHub `mergeable=true` value is about branch conflict resolution, not quality assurance.

## Current local candidate and remote PR recheck — 2026-10-31

Local HEAD is `a1e15e056104902cdce53abdcbd92c79ab4a0d6f` on
`build/native-codecs-g0`, clean and 92 commits ahead of the local tracking ref
`2947e6737622c3c668003130006823df0bdde3e0` (`git rev-list --left-right --count
origin/build/native-codecs-g0...HEAD` returned `0 92`). Local `origin/main`
`c02ef6c9b86cff3a86ceaaf5e35900593f592fbf` is an ancestor (exit 0), and the
GitHub PR head is an ancestor of this checkout (exit 0). A live GitHub read
reports PR #1 open, draft, unmerged, `mergeable=true`, with remote head
`2947e6737622c3c668003130006823df0bdde3e0` and base
`c02ef6c9b86cff3a86ceaaf5e35900593f592fbf`. No push or merge was made.

The current product source candidate `a1e15e0` passed
`sh scripts/offline-gate.sh` (exit 0; 17 local crates, 32 path-only internal
edges, no registry/Git crates or third-party system links) and
`sh scripts/build-macos-app.sh` (exit 0; 18 symlink-path refusal cases, Swift
playback-state regression, strict ad-hoc signature verification). The
SwiftUI library panel now shares one filtered-track result between its count
and list; a generated 2,000-track UI search returned expected result counts.
During active AudioQueue playback, operator-assisted removal of Shure analog
headphones changed macOS's selected route from External Headphones to
MacBook Air Speakers, then returned to External Headphones when reconnected;
Rust Music stayed in PLAYING state throughout and showed no error; explicit
Stop then Quit succeeded. This confirms default-route transition and return,
not acoustic continuity or a device-error/reopen path. The feature-gated
`package_process` suite was rerun at source-equivalent HEAD `0e2bc78` and passed
7/7. A focused path-containment review found no further finding in its
reviewed scope. A host-level, low-amplitude synthetic WAV through the
app-bundled CLI exited 0 against the default output route; acoustic/DAC
fidelity remain untested.
Five synthetic catalog-core runs are recorded in the MAC v1.0 ledger; they do
not close app-wide performance acceptance. Existing private parity evidence
was not rerun. Keep PR #1 draft and unpushed while host focus, hardware and
remaining performance/release gates are open.

The bundle built from product code at `5da9393` (no app/Rust source changes
through current HEAD) was exercised with two generated 60-second WAV fixtures:
search returned one row without autoplay; explicit Play, Pause/Resume,
active Next/Previous, volume adjustment and Stop worked. During AudioQueue
playback, Activity Monitor showed app PID 28923 and CLI PID 33396. After
Cmd-Q, the old active process and child were gone; the fresh Ready / No Track
process was PID 33445 without a CLI child. Closing it left `cua.listApps()` at
`isRunning=false`, and Activity Monitor showed no Rust Music/CLI row. Fixtures
were removed and queue/playlist state was not changed. Device disconnect and
recovery remain untested.

## 2026-10-31 standalone text-library workflow

At implementation commit `2546cae`, the Rust CLI provides root-scoped
persistent queue and named-playlist operations. Its state tests passed 12/12
within the offline gate. The standalone macOS app now connects explicit
folder selection and catalog search to queue/playlist controls and explicit
Play/Stop. Queue selection and Previous/Next do not start playback or acquire
focus. `sh scripts/offline-gate.sh` exited 0 (17 local crates, 32 path-only
edges, zero registry/Git crates); `sh scripts/build-macos-app.sh` exited 0 and
the local ad-hoc bundle signature verified. Roadmap/literature checks exited
0 and no image or visualizer symbols were found in app source. This does not
complete standalone-to-host focus transport, Pause/volume, autoplay preference,
EQ, physical output, panel, or live Radio/TV acceptance. No private codec
parity suites were rerun for this UI/state change.

## Text library foundation and cancellable scans — 2026-10-31

At source commit `e5d6b34`, named playlists use bounded, versioned `.rmp`
storage with safe relative paths and atomic replacement, and explicit scans
can be cancelled without returning a partial catalog. The library suite passed
27/27; `python3 scripts/check_roadmap_index.py` and
`python3 scripts/check_literature.py` exited 0. A fresh
`sh scripts/offline-gate.sh` at `e5d6b34` exited 0 across 17 local crates,
32 path-only edges, zero registry/Git crates, workspace tests, release build,
and macOS feature checks. The first offline-gate attempt after adding the new
cancellation error exited 101 because the CLI's exhaustive error mapping lacked
a cancellation arm; that arm was added and the committed rerun passed. Private
codec parity evidence was not rerun because this change does not touch decoder
or PCM paths. These APIs are not yet wired to the standalone UI or host queue.
`sh scripts/build-macos-app.sh` also exited 0 at this source commit; the local
bundle's ad-hoc signature passed strict `codesign` verification. Developer ID,
notarization, and distribution signing remain unverified.

At app commit `8f0d64d`, the remaining SF Symbol widgets were removed from the
macOS shell to keep the current interface text-only. The artwork/image search
over the app, IPC and panel source returned no matches; the macOS bundle build
and strict local ad-hoc signature verification exited 0. The shell remains a
single-file Play/Stop inspector and does not yet implement the library UX.

## 2026-10-31 catalog and package hardening pass

At this recording, the latest implementation snapshot on
`build/native-codecs-g0` is `84f2096` (this subsequent commit only refreshes
this document). Local `origin/main`
(`c02ef6c9b86cff3a86ceaaf5e35900593f592fbf`) is an ancestor, and that
snapshot is 58 commits ahead of local
`origin/build/native-codecs-g0` tracking ref (`4e3d24a8fe6bea68b4cd40736ddd4b7db6368011`).
That tracking ref is known to be stale; this is not a fresh GitHub comparison
and is not authorization to push. Recent commits add bounded text search,
deterministic sort/pagination, root-scoped IDs, a bounded non-persistent M3U8
model, a versioned Unix-persistent queue/repeat/shuffle model, a synthetic
0/1/100/2k/20k catalog profile, and simulated AudioQueue failure/teardown
handling. The full `sh scripts/offline-gate.sh` exited 0 at the queue/native
implementation snapshot `845b32c`; the profile example was separately built,
run and checked with warnings-denied Clippy. The app bundle build, plist lint
and strict signature verification exited 0 at `845b32c`; private corpus parity
was not rerun. Audible Rust Music host contention, live TV, physical device
disconnect/recovery, panel/DAC, distribution signing and release approval
still block merge.

A separate Pip-Boy commit `bfbb9c76d147911feb6d359ba0f2342a8a0cd97e` adds
and passes a 1/1 real Rust Music child test for MUSIC/RADIO broker handoff:
MUSIC excludes RADIO, suspend releases focus, RADIO acquires, resume
reacquires MUSIC, and shutdown releases. This is host broker lifecycle
evidence, not audible playback or live TV contention. The change remains on
its separate clean branch and is unpushed; it is not in Rust Music history.

## Local post-commit verification — 2026-10-31

Local commit `0bc3f1673a635e688b928406c63baa46bbbb22da` is clean, includes the
RM-019 child-lifecycle changes, and has local `origin/main` as an ancestor. The
operator-provisioned Rust/Cargo 1.99.1 toolchain completed
`PRIVATE_FLAC_DIRECTORY=<private six-file directory> RUST_MUSIC_EVIDENCE_DIR=<private-output-directory> sh scripts/premerge-check.sh`:
roadmap/literature checks and eight roadmap unit tests passed, the complete
offline Rust gate exited 0, WAVE passed 6/6, AIFF/AIFF-C passed 7/7, and the
six exact manifest FLACs (L1/L2/L3/G4/G5/G6) passed native PCM/metadata/hash
parity. The checker printed its final automated-check PASS. Private files and
captured logs remain outside Git.

This completes the scripted local premerge checks at that checkpoint, but does
not satisfy the remaining host shared-focus, physical panel/DAC, native device
failure and recovery, or release approval requirements below. PR #1 remained
draft and unpushed.

## Standalone package and LaunchServices — 2026-10-31

At clean local commit `bbd5dc3c52d3e187a1a332010f07d2a72906766c`,
`sh scripts/build-macos-app.sh` exited 0. `plutil -lint` and
`codesign --verify --deep --strict` passed; the app's `LC_BUILD_VERSION`
minimum is macOS 13.0, matching `LSMinimumSystemVersion`. After unregistering
the generated app (`lsregister -u`, exit 0), unsandboxed `open -n` by path
auto-registered and launched it (exit 0); unified logs contain
LaunchServices CHECKIN for `dev.rustmusic.standalone`. Rebuilt-app UI checks
inspected and played a generated 16-bit/44.1 kHz WAV, stopped playback, then
closed the window during playback; Activity Monitor showed no
`rust-music-cli` process afterward. The sandboxed `open` error and `lsregister`
`-10822` were sandbox service-access failures. The local ad-hoc signature is
not accepted by `spctl`; Developer ID signing/notarization and distribution
acceptance remain unverified. This closes local RM-019 launch acceptance, not
the independent hardware gates.

## GitHub evidence verified on 2026-10-31

- `build/native-codecs-g0` incorporated `main` in a real, non-force two-parent merge `ed241ac60ddd9b3a2f22ebc71bc557597d505d5a`. GitHub compare reported **74 ahead, 0 behind** before the subsequent premerge-script commit.
- PR #1 is open and draft. No required status checks or review submissions are recorded on the audited head.
- The remote PR head remains `7dc75e13ab242d6ac7353139f0955ccce96b10cf`; the local development branch is at `bbd5dc3c52d3e187a1a332010f07d2a72906766c`. Its PR-hosted status checks are empty; local test evidence is not GitHub CI evidence.
- Local Rust/Cargo 1.99.1 is available, and the complete offline and corpus premerge gate passed at local commit `8a057792586ba050521e6989137db4492f1ffb27`. Later local edits are limited to the macOS app build/signing script and documentation; the macOS app build itself passed on `bbd5dc3`.
- The local remote-tracking ref is stale. GitHub PR #1 has six remote-only commits since common base `4e3d24a`; local has 47 commits from that base. Integrate both histories without force before any eventual authorized push.
- [Issue #2](https://github.com/pskeffington/rust_music/issues/2) remains open for `RM-001`. Do not close it from static inspection.

## Read-only static audit in this pass

The repository's workspace manifests and machine-readable registries were fetched and checked: **12 workspace members; 12 matching lockfile packages; 21 dependency edges, all local `path`; 15 roadmap gates; 16 task IDs; 6 Codex agents; and matching 16-bit/44.1 kHz playback-floor records**. No violations were observed in this specific static comparison. Fifteen Rust source files were inspected (51 `#[test]` markers); no executable network client reference was observed. This is **not** a rustfmt/Clippy, Rust compilation, native PCM parity or hardware verification.

## One-command operator acceptance

Run from a clean local checkout on the feature branch, with the exact current PR head and an offline Rust toolchain already installed:

```sh
git fetch origin
git switch build/native-codecs-g0
git pull --ff-only
PRIVATE_FLAC_DIRECTORY="/path/to/private/six-flac-tracks" sh scripts/premerge-check.sh
```

The new [premerge checker](../scripts/premerge-check.sh) is entirely offline once the checkout is prepared. It refuses a dirty or out-of-sync checkout, validates the roadmap and literature indexes, runs the real `capture_offline_gate.sh` Rust build/test/Clippy/fmt/release gate, then invokes native WAVE/AIFF and private six-track Rust FLAC differential tests. No private audio or logs are committed or uploaded, and it never executes `git fetch`, installs anything, edits branches or merges. Its evidence wrapper writes private transcript and summary **outside Git**. Do not paste the full private transcript into a public issue.

**Interpretation:** An exit-zero result qualifies only the scripted gates. It does not establish real shared Music/Radio/TV focus, 128×64 physical screen input/output, negotiated DAC samples or bit-perfect hardware. For those, follow `RM-004` through `RM-007` and record named hardware evidence without sensitive metadata.

## Merge/cleanup instructions after evidence

1. Review the generated private evidence, record the exact HEAD/commands/exit codes and reconcile task statuses in `docs/roadmap-index.json`, `docs/ROADMAP.md`, and Issue #2 without guessing.
2. Verify native private FLAC PCM parity and WAVE/AIFF tests **independently** of the static manifest audit and `cargo test`.
3. Complete Rust_Boy shared-focus, physical DAC, panel and error-path acceptance for the release target or explicitly narrow any supported platform scope; never claim bit-perfect without measurement.
4. Reconfirm `main` is an ancestor of the PR head and GitHub's head SHA has not moved. Then mark PR ready, obtain the authorized merge, use a non-force GitHub merge with expected head SHA, and verify `main` contains the result.
5. Only after verifying that the branch's work is present on `main`, assess whether the old branch may be deleted. Never force-delete the only implementation branch or close an incomplete issue.
6. Update root `AGENTS.md`, README, roadmap metadata and PR references after merge so Codex does not check out an obsolete branch; no production tags/releases without independent release acceptance.

**Policy:** The user's request to work toward merge authorizes the reconciliation and preparation work. It does not convert an unexecuted gate into a PASS. See [production playbook](CODEX_PRODUCTION_PLAYBOOK.md), [RM-001 operator gate](RM001_OPERATOR_GATE.md), and [roadmap](ROADMAP.md).

## 2026-10-31 standalone transport integration — not merge approval

Rust transport controls are committed locally as `fc47a19` and the native
pause-before-first-buffer handling as `7ae312b`. The working integration
connects persistent RBPK transport to the macOS app's Play/Pause/Resume/Stop/
Next/Previous/software-volume controls. At this source snapshot,
`sh scripts/offline-gate.sh`, `sh scripts/build-macos-app.sh`, roadmap and
literature checks, and `git diff --check` all exited 0. AudioQueue injected
failure/teardown tests passed 15/15, and the package-stream transport E2E
regression passed. Independent Swift review identified and confirmed the fix
for Next/Previous while paused with an empty queue. Independent security
review found no actionable finding in bounded IPC, process cleanup, paths,
privacy, dependencies or image/network constraints.

This does not authorize merge. At the time of this silent-fixture smoke,
audible output was not tested. Existing private FLAC 6/6, WAVE 6/6 and
AIFF/AIFF-C 7/7 evidence is retained and was not rerun. See the later native
output smoke below. Device-loss/recovery, Rust Boy shared focus with Radio/TV,
panel/DAC, cold startup/incremental memory, and Developer ID/notarization remain
open. Keep PR #1 draft and `main` unchanged until policy gates and independent
approval are satisfied.

## 2026-10-31 rebuilt-app UI smoke

On the exact rebuilt app, the explicit folder picker loaded a two-track
16-bit/44.1 kHz synthetic silent WAV library. Text search, queue insertion,
and named-playlist creation worked. After Cmd-Q/relaunch and explicit folder
reload, the queue and playlist were restored and playback remained stopped.
Play, Pause, Resume, active queue Next/Previous, and Stop reached their
expected UI states. The volume slider changed 100%→99% while stopped and
99%→98% during playback. After Stop and Cmd-Q, `pgrep` showed no Rust Music
application or CLI child. This proves the app control and persistence paths on
silent PCM. It does **not** establish audible output: no OS audio device was
listed by the sandboxed `system_profiler` invocation, and no hardware loopback
was captured in that test.

## 2026-10-31 unsandboxed native-output and lifecycle smoke

The local bundle passed `open -n` with exit 0 when launched outside the
restricted shell. LaunchServices recorded bundle `dev.rustmusic.standalone`,
arm64 executable `/Users/null/GitHub/rust_music/target/macos/RustMusic.app`,
PID 92963, and 0.065434 s launch-to-check-in. The unsandboxed full audio-device
inventory listed MacBook Air Speakers as the 48 kHz default output, an ASUS
VG32V HDMI output, and built-in headphones. The bundled CLI played a generated
nonzero 440 Hz, 16-bit/44.1 kHz WAV through AudioQueue and exited 0. The user
reports the tone was audible; this is not acoustic loopback or DAC
bit-perfect evidence.

The exact app UI explicitly loaded a separate two-track library of generated
10-second, nonzero 440 Hz and 660 Hz WAVs. Play entered Playing; Previous and
Next changed the selected queue title while playback remained active; Pause
and Resume returned to their expected states; volume changed 100%→95%→100%;
Stop returned to Stopped. Before Cmd-Q, the app was PID 93068 and its real
`rust-music-cli serve` child PID 93107. After Cmd-Q, `ps` returned no matching
app or CLI processes, and LaunchServices reported the app stopped. Device
disconnect/recovery, DAC capture, Rust Boy Radio/TV focus, and notarized
distribution remain unverified.

## 2026-10-31 current-candidate offline and child-process checks

At source HEAD `b03c592ec67d102325c4b0af0adc93acc71e9651`, the offline gate
exited **0** (17 local crates, 32 path-only edges, zero registry/Git crates,
zero third-party system links; 15/15 injected AudioQueue failure/teardown
tests passed). `sh scripts/build-macos-app.sh` exited **0** and the bundle
passed strict ad-hoc signature validation. Roadmap-index, literature and
`git diff --check` validators exited **0**.

The current bundle's CLI child passed the default
`cargo test --locked --offline -p rust-music-cli --features
macos-output,test-support --test package_process` run (5/5, 0.24 s), after
one initial parallel run timed out in all five tests; a serialized rerun also
passed 5/5 (0.28 s). `scripts/test_macos_parent_exit.py` observed normal EOF,
malformed-protocol exit, three repeated launch/exit cycles, and child exit
after an app-like parent was force-killed. `scripts/test_release_package_process.py`
passed the bundled CLI RBPK lifecycle check (SHA-256
`2f4adc33ef8a29c0b282c34307f6e0b33530d775d14dc73e2aa5c9ed417d655e`). These
are child-process tests; they do not launch the SwiftUI app or stop active
AudioQueue playback.

The app-level Cmd-Q observation in the earlier native-output smoke belongs to
the older app source before the navigation/shutdown change at `31cb4cb`. The
current app's close-path child exit must be re-observed; the native folder
picker remained open during this pass and was left untouched. RM-019 is
reopened as `in_progress` for that current-candidate evidence. Device
disconnect/recovery, Rust Boy shared focus, physical panel/DAC, and
distribution approval remain open. Local HEAD remains 24 commits ahead of its
local development-branch tracking ref as of code checkpoint `b03c592` (not
refreshed during this pass); no push or merge was performed. At documentation
checkpoint `b16201d`, the local branch was 26 commits ahead. PR #1 remains
draft and ineligible for merge.

## 2026-10-31 follow-up current-app close and startup observation

After the operator closed the library picker, the current rebuilt app showed
its loaded catalog and `PLAYBACK FINISHED`. Without starting or selecting a
track, Activity Monitor showed Rust Music PID 95299 and a `rust-music-cli`
child PID 95500. Cmd-Q closed that app. CUA's subsequent `getApp()` call
automatically launched a fresh instance (PID 9117) with an empty library;
the refreshed Activity Monitor list had no old app PID or CLI process. A later
Quit was followed by `cua.listApps()` reporting `isRunning=false`, and the
Activity Monitor `rust` filter showed neither Rust Music nor its CLI child.
This verifies current-candidate idle-child cleanup; there was no active audio
at close.

The app was reopened once more for startup verification. Activity Monitor
listed Rust Music PID 9188 and no playback child; the UI displayed Ready / No
Track, disabled transport, and explicit Load Library. Player, Library, Queue
and Playlists navigation was exercised; no folder was loaded in this startup
check. This supports no startup scan/autoplay on the current bundle.

The local catalog example was rebuilt with
`cargo build --release --locked --offline -p rust-music-library --example
profile_micro_library` (exit **0**). Two direct profile runs exited **0** at
0/1/100/2k/20k generated signature-only entries. For 20,000 entries, catalog
builds took 519/636 ms and search p95 was 734/802 µs. An attempted
`/usr/bin/time -l` wrapper exited **1** because the sandbox denied its RSS
`sysctl kern.clockrate` call after all profile rows printed; peak RSS remains
unmeasured. The exact two-run table is recorded in
[`MICRO_LIBRARY_ZERO_ARTWORK.md`](MICRO_LIBRARY_ZERO_ARTWORK.md).

## 2026-10-31 active-playback shutdown evidence

The current macOS app was loaded with only the generated `silent-60s.wav`
fixture and displayed `PLAYING … THROUGH RUST MUSIC AND MACOS AUDIOQUEUE`.
Activity Monitor showed Rust Music PID 11612 and `rust-music-cli` PID 11664.
Cmd-Q was sent with Rust Music foreground; after refreshing the `rust` process
filter, neither process remained, and `cua.listApps()` reported the app
`isRunning=false`. This is observed app/child exit during active silent
playback; it is not audible-content or DAC acceptance.

The test-only paced-null subprocess regressions independently observed a
`Playing` view with `audio_claimed=true` and nonzero delivered frames. One
closed the IPC parent; another matched the Swift client's normal Stop-then-
Shutdown sequence, requiring a Stopped view with `audio_claimed=false`, a
Shutdown Ack, child exit status 0, and stdout EOF. The package-process suite
passed 7/7 on 2026-10-31. The release-mode targeted filter exited 0 with zero
debug-only tests selected, and the full offline gate was rerun after
integration and exited 0 across all workspace crates.

RM-019 is now **done for local bundle idle and active synthetic-playback
shutdown acceptance**. Native-device disconnect/recovery, live Rust Boy focus,
physical panel/DAC and distribution approval remain open. The overall release
decision is unchanged: local branch is unpushed, PR #1 stays draft, and mainline
integration remains blocked.

After that active-playback check, five fresh process launches returned the
empty Ready / No Track accessibility tree in **891, 858, 753, 798 and 779 ms**
(median **798 ms**, range **753–891 ms**). Every run showed no loaded library
or autoplay and exited before the next run. Activity Monitor reported
**28.2 MB** in its `anonymousMemory` column for the idle app (PID 12236), with no
CLI child. After Quit, both `listApps()` and the refreshed Activity Monitor
filter showed no Rust Music or CLI process. This is a five-run process-launch
sample, not a system-boot cold-start profile or performance threshold, and one
idle-memory reading, not repeatable memory profiling or RSS.
