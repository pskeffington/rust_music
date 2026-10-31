# Rust Music macOS v1.0 production matrix

**Baseline:** `820582c3602df2cc63a1b76fbda88b5b58bbc396`; this pass validates
source through `7ae312b` plus the current uncommitted Swift/CLI integration on
`build/native-codecs-g0` (2026-10-31). The branch is ahead of its local
tracking ref. This is an evidence ledger, not release approval.

## Acceptance gates

| Gate | Production requirement | Status at baseline | Evidence / remaining acceptance |
| --- | --- | --- | --- |
| MAC-01 | Complete usable library-to-playback workflow | **IMPLEMENTED / UI ACCEPTANCE OPEN** | The app connects explicit bounded/cancellable library load, text search, named playlists, persistent queue/repeat/shuffle, Play/Pause/Resume/Stop/Next/Previous, and volume through the RBPK child process. Queue selection does not autoplay. The offline Rust gate and macOS app bundle build pass on this integration; simulated package-stream E2E is covered. Window interaction with this exact rebuilt app remains to be observed. |
| MAC-02 | Native macOS playback, shutdown and device recovery | **PARTIAL / OPERATOR** | Prior LaunchServices startup, ad-hoc bundle signing, and generated-WAV Play/Stop/window-close child-exit evidence are recorded in `docs/PR1_MERGE_READINESS.md`. AudioQueue fault/teardown tests pass 15/15, including pause-before-first-buffer. This pass did not test audible output or real device disconnect/recovery. Developer ID/notarization remains a distribution gate. |
| MAC-03 | Complete offline suite, performance and independent security/code review | **PARTIAL** | `sh scripts/offline-gate.sh` exited 0 at this source snapshot (17 local crates, 32 local path edges, no registry/Git crates); app build/signature verification and roadmap/literature validators exited 0. Existing FLAC 6/6, WAV 6/6, AIFF 7/7 and ALAC/WavPack evidence is retained; private corpus was not rerun. Synthetic catalog scan/search profile exists; cold launch and incremental library memory are unmeasured. Independent review of the final integrated diff is pending. |
| MAC-04 | Reviewed mainline integration with preserved history | **BLOCKED BY RELEASE GATES** | Development branch is synced non-force to origin and contains `origin/main`; draft PR #1 remains under existing release policy. Do not merge until applicable MAC gates, independent review and release evidence are satisfied. Preserve backup branch and stash through post-merge verification. |

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

At source commit `7ae312b` plus the current Swift/CLI working diff, the full
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
