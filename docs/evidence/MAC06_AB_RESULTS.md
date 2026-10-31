# MAC-06 baseline and paired A/B results

**Status:** A baseline recorded; B candidate pending implementation. The
baseline reproduced a silent-playback failure. The operator later reported
"hear audio now"; the exact fixture, route and timing for that confirmation
were not captured, so treat it as positive operator feedback, not a paired
acceptance pass.

## Protocol and constraints

- A is the unmodified source commit `93438c66eab685bc0298ef5a39fcef88d1908d9d`.
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

The UI's `Playing` label is not an audibility assertion. This run establishes
that a non-silent fixture traversed the native API successfully and that the
app state advanced to Playing/Finished. The user initially reported that the
low-level tone was not audible. A later response said "hear audio now," but
there is no time-correlated fixture or route record for that response.

### Reproduction follow-up

- A second direct CLI attempt against the generated 16-bit/44.1 kHz WAV exited
  1 with `AudioQueueStart OSStatus -66680`. That status is Apple's
  `kAudioQueueErr_InvalidDevice`; the OS did not start this queue.
- The matching RBPK v1 run returned Playing immediately with 0 delivered
  frames and `audio_claimed=true`, then Failed at the next sample with 0
  frames and `audio_claimed=false`. This confirms that the user-facing Playing
  interval can precede successful output. It does not establish whether the
  later "hear audio now" report was the same app or fixture.
- `sh scripts/build-macos-app.sh` on the current checkout exited 1 at
  `FAIL: Package process closed a reply early.` The individual package-process
  integration suite passed 9/9 in the independent audit; the discrepancy
  needs investigation before claiming the app-build gate passes.

## B candidate and paired runs

Pending. Repeat the exact fixture hashes, Mac, selected endpoint, app/system
volume, and steps on the candidate. Alternate A/B order where practical. Run at
least three repeats for functional/hardware checks and five for UX/performance
pilots; report each observation, range, and median. Do not claim statistical
significance from these engineering samples.

| Pair / repeat | A result | B result | Difference / notes |
| --- | --- | --- | --- |
| Non-silent WAV playback | API run exit 0; separate retry exit 1 (`-66680`); first operator response not audible | Pending | Later operator feedback says audio is audible but is not tied to a fixture/route; repeat controlled run |
| Non-silent FLAC playback | API run exit 0 | Pending | Human audibility confirmation required for this exact file |
| Progress / pause / resume | Immediate Playing/0 frames followed by Failed/0 frames on failing retry | Pending | Compare snapshots, not UI timers |
| Library / queue / playlist UX | Baseline architecture inventory pending | Pending | Use five-repeat usability pilot after layout implementation |

## Next steps

1. Add an automated nonzero PCM → sink conversion/enqueue regression and
   injected start/enqueue failure assertions before changing behavior.
2. Preserve RBPK v1; decode and retain its existing frame/format/focus fields
   in a typed Swift snapshot, with strict bounds and malformed-frame tests.
3. Add read-only accessible elapsed/total progress, honest Playing/Paused/
   Finished/Failed status, and an opt-in bounded diagnostics view.
4. Reconstruct library, queue, playlist and transport presentation without
   artwork, visualizers, new runtime dependencies, or host-focus changes.
5. Repeat paired A/B checks; record human audibility separately from software
   success. Keep device/DAC validation blocked unless observable output is
   confirmed on the named route.
