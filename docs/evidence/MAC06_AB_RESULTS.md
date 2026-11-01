# MAC-06 baseline and paired A/B results

**Status:** A/B software and one operator-confirmed audible B app run recorded.
The candidate now reports RBPK frame-derived progress and preserves native
output errors. This is not a production acceptance pass: LaunchServices
`open -n`, repeated app-window audibility, live device loss/recovery, and the
library/queue/playlist usability pilot remain open.

## Protocol and constraints

- A is the unmodified source commit `93438c66eab685bc0298ef5a39fcef88d1908d9d`.
- B is source commit `2b7544677008bc3183be02a80477aee79ff360ad`.
- A was built from a clean `git archive` into a temporary checkout, then copied
  to a temporary app bundle with an isolated bundle ID. This avoided the
  already-open Rust Music instance and its private library.
- Generated fixture `MAC06-TONE-440-10S` is a 10-second, 440 Hz, mono,
  signed 16-bit / 44,100 Hz sine at peak amplitude 328 (about -40 dBFS).
  SHA-256: WAV `8890dc24c2b88aeba0ee11d677e182fa4e45155ce40aa2ed12a593e3c2078846`;
  FLAC `158aded0437ef79ad939e009fda61d27a6aaf817582137290c0508dd09abb63d`.
  Both decode to 882,000 identical PCM bytes, SHA-256
  `d7d1b95367e64319895ca6d54549aa25d2a758210530b1d42c6743d3407dceef`.
  `flac --test` exited 0. Generated material is outside Git-tracked files.
- Machine: macOS 26.5.2, arm64. System Settings showed MacBook Air Speakers
  selected, output volume 92%, mute off. The isolated app volume was 100%.
  No audio settings were changed. The restricted shell's `system_profiler`
  returned an empty device list, so its device inventory is not used.
- No user/private media was opened. Paths are represented with environment
  variables below and omitted from this report.

## A observations

| Condition | Command / observation | Result |
| --- | --- | --- |
| Baseline app build | `sh scripts/build-macos-app.sh` from the archived A tree | **PASS**, exit 0. Existing path guards, process shutdown, search, playback UI-state, bounds, and strict local ad-hoc signature checks completed. |
| Baseline offline Rust gate | `sh scripts/offline-gate.sh` from the archived A tree | **PASS**, exit 0: 17 local crates, 32 path-only edges, 0 registry/Git crates, 0 third-party system links, one OS framework link; formatting, warnings-denied Clippy, workspace tests, release build, macOS feature check, and 17/17 injected AudioQueue tests passed. |
| Restricted direct CLI WAV | `"$CLI" play "$MAC06_ROOT/MAC06-TONE-440-10S.wav"` in restricted execution | **FAIL / environment-limited**, exit 1 in 0.02 s: `AudioQueueStart OSStatus -66680`. The same binary succeeded in the user audio context below; this failure is not treated as a product reproduction. |
| Direct CLI WAV, user audio context | Same file and binary, `/usr/bin/time -p` | **PASS for API/process path**, exit 0, real 10.43 s. Operator later reported audio audible, but this run was not tied to that confirmation. |
| Direct CLI FLAC, user audio context | Same binary and decoded-PCM-equivalent FLAC | **PASS for API/process path**, exit 0, real 10.16 s. No format-specific audibility confirmation was captured. |
| Baseline app WAV | Explicitly load the generated root, select WAV, press Play | UI changed to `PLAYING … THROUGH RUST MUSIC AND MACOS AUDIOQUEUE`, then `PLAYBACK FINISHED`; app volume 100%. No elapsed time, total duration, frame count, output details, or error diagnostics appeared in the UI. |
| Baseline app FLAC | Explicitly select FLAC and press Play | UI showed `PLAYING … THROUGH RUST MUSIC AND MACOS AUDIOQUEUE`. No progress or detailed diagnostics appeared. |
| RBPK counter sample, WAV | `serve --desktop` with the same app-bundled CLI; immediate snapshot then snapshot 1.077 s later | State 2 / Playing, format 44,100 Hz mono 16-bit, selected index 1, total 441,000 frames, audio claim true. `frames_delivered` rose 0 → 57,344. This is queued/delivered-to-sink progress; it is not a hardware playback clock. |
| Pause / resume counters | Same RBPK process; pause, snapshot, wait 750 ms, snapshot, resume, wait 500 ms, snapshot | Paused state 6 had `audio_claimed=false`; counter stayed 57,344 for 750 ms. Resume returned state 2 / claim true; after about 535 ms, counter reached 81,920. Child shut down with exit 0. |

The RBPK observations show the counters are suitable for a monotonic elapsed-time
estimate (`frames_delivered / sample_rate`) with pause stability. They may run
ahead of physical sound because frames are counted after successful
AudioQueue enqueue/start, not after a DAC completion callback. Do not label
this value a hardware clock or use it to prove sound.

The UI's `Playing` label is not an audibility assertion. The later B run below
has a time-correlated operator confirmation for a generated fixture and
observed route; frame progress remains evidence of frames submitted to the
AudioQueue, not a physical playback clock.

### Reproduction follow-up

- A second direct CLI attempt against the generated 16-bit/44.1 kHz WAV exited
  1 with `AudioQueueStart OSStatus -66680`. That status is Apple's
  `kAudioQueueErr_InvalidDevice`; the OS did not start this queue.
- Repeated direct playback through the exact isolated A app's bundled CLI
  then succeeded: the PCM-equivalent generated FLAC completed with exit 0 and
  real time 11.6 s. Three further repeat attempts of that exact FLAC fixture
  all exited 0; the operator did not listen during the first, then confirmed
  "audio heard" during each of the next two. These are direct CLI executions
  of the binary bundled inside the isolated app, not app-window-triggered
  playback. The matched RBPK WAV exercise also exited 0. The earlier general
  "hear audio now" response was not time-correlated to a fixture or route.
- The matching RBPK v1 run returned Playing immediately with 0 delivered
  frames and `audio_claimed=true`, then Failed at the next sample with 0
  frames and `audio_claimed=false`. This confirms that the user-facing Playing
  interval can precede successful output. It does not establish whether the
  later "hear audio now" report was the same app or fixture.
- On the subsequent successful RBPK WAV run, the 1.0 s snapshot reported
  53,248 frames of 441,000 with Playing/claim=true. Pause held at 53,248 for
  750 ms with claim=false. Resume increased to 77,824 after about 0.53 s with
  claim=true. The child exited 0. These are queue-submitted frame counts, not
  physical playback-clock measurements.
- The package-process harness initially failed when it launched a nested CLI
  from the unsigned, in-progress app bundle. The harness now runs after final
  bundle signing; current `sh scripts/build-macos-app.sh` result is recorded
  under B below.

## B candidate and paired runs

The B candidate is the isolated, ad-hoc signed arm64/macOS 13 app bundle
(`dev.rustmusic.mac06candidate2`). Its bundled CLI SHA-256 was
`d18a3c0489729bde837a8be786bc1dd845489fd52d1048dbcc81ff67bf33b877`; the
main executable in the final build output was
`8c89b28929011b8afc1fccb6f79ebeaf38862052f484645e2247e7fb5dbff747`.
The isolated B2 app main executable was re-signed for the test bundle, SHA-256
`6fbd3e37b916652003ff7f35aca4febbf69cba414f2c5984c7d1c1e895867917`.
The MacBook Air Speakers remained selected at system volume 92%, unmuted; app
volume was 100%. Fixture hashes and PCM are the same as A. Only the generated
fixture folder was explicitly loaded; scanning left playback stopped.

| B check | Command / observation | Result |
| --- | --- | --- |
| Rust offline gate | `sh scripts/offline-gate.sh` | **PASS**, exit 0: 17 local crates, 32 path-only edges, no registry/Git crates or third-party system links; fmt, warnings-denied Clippy, workspace tests, release build, macOS feature checks and native AudioQueue tests 18/18. |
| Native output tests | `cargo test --locked --offline -p rust-music-native --features macos -- --test-threads=1` | **PASS**, exit 0, 18/18. Includes exact nonzero 16-bit little-endian PCM bytes and enqueue-before-start assertions plus failure cleanup. |
| CLI process tests | `cargo test --locked --offline -p rust-music-cli --features macos-output,test-support --test package_process` | **PASS**, exit 0, 9/9. Error details persist until explicit retry; injected device failure can recover. |
| macOS app bundle | `sh scripts/build-macos-app.sh` | **PASS**, exit 0. Swift snapshot tests, bounded catalog checks, shutdown/escalation, protocol errors, three child launch/exit cycles, force-killed parent cleanup, full arm64/macOS 13 compile, and strict signing passed. |
| Bundle signature | `codesign --verify --deep --strict --verbose=2 target/macos/RustMusic.app` | **PASS**, exit 0. |
| UI playback / audibility | Launch isolated B2 app, explicitly load generated folder, press Play | UI showed 44.1 kHz/16-bit mono, submitted frame count and progress. The operator answered “Yes, audible” while the generated FLAC ran. |
| Track progression | B2 snapshots during the 10-second generated FLAC | Frames progressed through 102,400 (~2 s), 176,128 (~3 s), then 441,000/441,000 at 00:10/00:10; UI reached Finished and 100%. Counters are enqueued frames, not DAC completion. |
| Startup / no autoplay | Quit B2, relaunch | Returned Ready / No Track with no library and no playback. The explicit load itself also did not start playback. |
| Close lifecycle | Quit B2 and inspect app inventory | B2 absent after quit. Build harness separately reported repeated child exit and force-killed parent cleanup gates passing. |
| Format check | `flac --test` on generated FLAC | **PASS**, exit 0. Decoded PCM is identical to generated WAV by hash. |
| LaunchServices | `open -n` on isolated test bundles | **FAIL**, exit 1 with `kLSNoExecutableErr -10827`; `lsregister -f` returned `-10822` from Spotlight. The valid bundle executable and strict signature pass, and direct CUA app launch works. Treat LaunchServices registration as unresolved/environment-limited, not repaired. |

| Pair / repeat | A result | B result | Difference / notes |
| --- | --- | --- | --- |
| Non-silent WAV playback | API run exit 0; separate retry exit 1 (`-66680`); later matched RBPK run exit 0 | Not repeated in B app | A demonstrates intermittent AudioQueue device-start error; do not infer recovery from candidate WAV. |
| Non-silent FLAC playback | Bundled A CLI: 3 repeats, exit 0 each; operator confirmed audible on repeats 2 and 3 | Isolated B app: operator confirmed audible during generated FLAC playback | Audibility confirmed in both generations, though not a balanced repeated A/B sample. |
| Progress / pause / resume | Successful trace: 0 → 53,248 at ~1s; paused 750ms stable; resumed to 77,824 after ~0.53s | 10-second UI run advanced to 441,000/441,000 and Finished | B adds displayed frame-based progression and terminal 100% state. Separate B pause/resume UI pairing was not run. |
| Library / queue / playlist UX | Baseline app did not expose progress or detailed output diagnostics | Explicit test-folder load and playback verified; no-autoplay on load/relaunch | Full five-repeat library/queue/playlist usability pilot remains open. |

These are engineering observations, not statistically significant samples.
No real hardware disconnect/reconnect was performed in this pass. No startup
RSS benchmark or repeated UX/performance pilot was run; the current build's
catalog profile measured 20k last-item search p50 8.23 ms / p95 8.86 ms,
unindexed model-build p50 0.425 ms / p95 0.794 ms, and indexed model-build
p50 170.6 ms / p95 189.6 ms with a 1.3 MB cached index payload (65 B/track).

## Next steps

1. Repeat paired app-window playback (at least three runs per format) with a
   time-correlated operator observation on a named output; include WAV B and
   pause/resume in the paired run.
2. Diagnose the unresolved LaunchServices `open -n` failure on a clean Mac
   registration state. The app binary, `CFBundleExecutable`, architecture and
   strict code signature were verified; current `kLSNoExecutableErr` is not
   evidence of an executable defect by itself.
3. Run an actual removable-device loss/recovery test and record output route,
   app state, process exit codes and recovery behavior. Do not substitute
   analog headphone unplug/reroute for device disappearance.
4. Complete the text-only library/queue/playlist workflow and five-repeat UX
   pilot, then capture startup RSS and responsiveness measurements.
5. Keep production readiness blocked pending these checks and the remaining
   Mac v1 acceptance evidence. Never describe frame submission as DAC progress.
