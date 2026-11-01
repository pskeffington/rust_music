# MAC07 pass evidence register

**State: PARTIAL REPRODUCTION; AUDIO/HARDWARE ACCEPTANCE BLOCKED.** Do not store private filenames, absolute paths, music, or media hashes here. Baseline source: `e9cacb1d8d1a3464fd1ba973449767f7152cffd1` on `build/native-codecs-g0`.

| Pass | Source / commands | Observation | Result / blocker |
| --- | --- | --- | --- |
| MAC07-00 | Same-file CLI attempts; Sound settings; CoreAudio default-device probe | CLI failed twice at `AudioQueueStart` with `-66680` (`kAudioQueueErr_InvalidDevice`). Sound settings showed MacBook Air Speakers selected, but an independent idle CoreAudio query returned default device ID 0 and a zero-length device list. | **BLOCKED / PARTIAL.** No output device is exposed through CoreAudio while idle; GUI audibility remains unverified. |
| MAC07-01 | Persistent-server `CountingSink` regression for all 11 supplied FLACs; fresh bundled GUI and direct CLI after explicit default-route binding | 11/11 FLACs passed decode/frame delivery to the counting test sink; this does not test native output. Before the fix, GUI `CurrentDevice` did not match the default UID. After the fix, the GUI record showed route status 0, device status 0, associated device matching default, 1,059 buffers / 271,104 frames, 1,059 callbacks, completed drain, stop 0, dispose 0. A same-file CLI attempt failed route binding with `-66680` before allocating buffers or submitting frames. The idle CoreAudio query still showed default ID 0 and no devices. | **IN PROGRESS / UNVERIFIED.** GUI route binding succeeds during app playback; CLI route binding fails in this environment. The operator reports no sound from the player. Do not infer sound from route match, callbacks, or frames. |
| MAC07-02 | No acceptance run recorded | GUI frame count alone is not validated against audible device output; telemetry/parser work is underway. | **NOT VERIFIED.** Depends on MAC07-01 acceptance. |
| MAC07-03 | No acceptance run recorded | No compact/expanded layout or keyboard acceptance recorded for this pass. | **NOT RUN.** |
| MAC07-04 | No acceptance run recorded | No audible queue transition or operator transport acceptance recorded. | **NOT RUN.** |
| MAC07-05 | No acceptance run recorded | No playlist/persistence/path-safety acceptance recorded. | **NOT RUN.** |
| MAC07-06 | No acceptance run recorded | No A/B run, UX pilots, or acoustic evidence recorded. | **NOT RUN.** |
| MAC07-07 | Documentation synchronization only | Roadmap and task routing were updated; release gate, app/hardware acceptance and independent safety review are not implied. | **NOT COMPLETE.** |

### 2026-11-01 follow-up at `cd076f8` and current SwiftUI change

During the current live app session, the operator again reported no audible
output. While the app UI showed active playback progress, an independent Swift
CoreAudio probe returned status 0 with default output device ID 0 and a zero-
length device list. `/usr/sbin/system_profiler SPAudioDataType` exited 0 and
listed no audio devices. `osascript -e 'get volume settings'` returned
`output volume:missing value` and `output muted:missing value` (with an
unrelated HiServices XPC connection warning). This host therefore does not
currently expose a usable output route to the probe. Earlier GUI route-match
telemetry is not evidence that an audible hardware device is attached.

The operator also reported visible player flicker during playback. Source
inspection traced it to the 500 ms snapshot poll using the interactive command
path, which briefly cleared the visible status and toggled the transport busy
state. A SwiftUI change now polls on the existing serialized package queue
without changing those visible fields, and generation checks discard stale
polls around explicit commands. The macOS app build/signature/lifecycle script
exited 0; an independent read-only review found no concrete issue in the
change. Runtime flicker acceptance is pending a fresh app launch. The full
`sh scripts/offline-gate.sh` exited 0, including all 24 native macOS tests.

**Status:** the flicker source defect is repaired at source/build level; the
audio route and audible MAC07-01 acceptance remain **BLOCKED** until macOS
exposes an output device and the operator confirms sound from the rebuilt app.

## Interpretation

The direct CLI route-binding error and CoreAudio's absence of a default/enumerated device remain relevant. Earlier Sound settings showed the built-in speakers selected, while current independent probes expose no usable route. The GUI previously reported a queue UID matching default during playback, while a direct CLI attempt could not bind in the same environment. Neither a route match nor successful callbacks prove sound. The 11/11 `CountingSink` result is decoder/frame-delivery evidence only. No MAC07 audio, hardware, or production acceptance pass is recorded.

## Remaining acceptance sequence

Restore a CoreAudio-visible output device, rerun the direct CLI and GUI, and confirm both bind to the selected default and produce audible sound. Investigate why idle CoreAudio enumeration returns no devices while the GUI can bind during playback. Then validate progress/state, layout, transport, playlists, and A/B behavior. Capture sanitized commands, source SHA, and operator audibility confirmation. Run release checks at the accepted candidate; retain hardware/device and distribution blockers until separately observed.
