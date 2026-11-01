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


### 2026-11-01 operator correction and signal check

The operator clarified that other Mac applications, including browser radio,
produce audible sound. Therefore the empty device list and missing volume
values observed from the command-line probes above do **not** establish that
the user's Mac lacks an output device; those probe results are specific to that
execution context and conflict with the operator's audible system output. The
operator still reports Rust Music itself produces no sound.

Local decodes of two supplied FLACs to temporary WAVs exited 0. The currently
displayed GUI track decoded as 44.1 kHz, 16-bit stereo, 9,572,352 frames,
peak absolute sample 32,768 and RMS 9,511.5; another supplied track measured
12,985,344 frames, peak 32,768 and RMS 8,592.8. The temporary WAVs were
removed. This confirms the source PCM is non-silent, but it does not prove the
package process submitted those same samples to a valid output route.

Next evidence must be collected from the actual GUI-launched AudioQueue: verify
that its `CurrentDevice` UID resolves to one of the current
`kAudioHardwarePropertyDevices` entries, in the same child-process/session
that writes PCM. Record only membership and default match, never device UID.
Until that identity and audible output are established, MAC07-01 remains
unverified.


The current GUI accessibility snapshot also showed an active track at 1:59 with
5,263,872 submitted frames, while its output panel displayed an already
completed queue summary for only 271,104 frames (`teardown=1`, drain complete).
That mismatch confirms the panel was carrying a stale prior-session record;
its earlier `default yes` value cannot be used as current route evidence. A
Swift fix is underway to clear/scope child diagnostics per playback and keep
completed summaries off active snapshots.


### 2026-11-01 rebuilt GUI route inspection

After rebuilding and relaunching the app, the operator-supplied album folder
was explicitly loaded and track playback was explicitly started. The GUI's
current post-stop summary reported: AudioQueue create/route status 0; default
device list status 0 and listed=yes; queue CurrentDevice query status 0,
associated=yes, default=yes, and current device list status 0/listed=yes;
three allocated buffers (49,152 bytes total); 802 buffers / 205,312 frames
enqueued; 802 callbacks; successful stop, drain and dispose. The playback
session was stopped after collecting this record. It establishes that the
GUI-launched queue's device UID resolved to a device in CoreAudio's current
device list and matched the selected default at observation time. It does not
prove that sound reached the speakers. Operator listening confirmation for
this exact rebuilt session is pending; MAC07-01 remains unverified.

Native routing tests passed 29/29, including unlisted/default-list failures,
current queue UID membership and per-open diagnostics reset. The complete
`sh scripts/offline-gate.sh` exited 0; the macOS app build/signature script,
warnings-denied native Clippy, formatting, roadmap-index checker,
literature checker and `git diff --check` also exited 0. Independent source
review found no high-confidence native-routing blocker and noted scalar
CoreAudio property result-size checks as optional hardening. No DAC,
device-switch recovery, direct CLI A/B, or acoustic acceptance was performed.
