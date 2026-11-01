# MAC07 pass evidence register

**State: REAL-FLAC GUI PLAYBACK AUDIBLY CONFIRMED; REMAINING HARDWARE/RELEASE ACCEPTANCE OPEN.** Do not store private filenames, absolute paths, music, or media hashes here. Baseline source: `e9cacb1d8d1a3464fd1ba973449767f7152cffd1` on `build/native-codecs-g0`. The dated entries retain earlier observations; the latest audible acceptance is recorded at the end of this ledger.

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
audio route and audible MAC07-01 acceptance remain **UNVERIFIED** pending
current-session CoreAudio route inspection and operator listening confirmation.

## Interpretation

The direct CLI route-binding error and empty command-line CoreAudio probe results remain historical observations. The operator confirms other Mac apps are audible. A rebuilt GUI run later verified its queue UID was listed and matched the default output; this proves association, not sound. Neither a route match nor successful callbacks prove sound. The 11/11 `CountingSink` result is decoder/frame-delivery evidence only. No MAC07 audible, hardware, or production acceptance pass is recorded.

## Remaining acceptance sequence

Verify playback through an explicitly chosen CoreAudio output as well as System Default, then classify direct CLI/GUI audible behavior. A text-only in-app selector is implemented and lists outputs from the GUI process. Then validate device changes, progress/state, layout, transport, playlists, and A/B behavior. Capture sanitized commands, source SHA, and operator audibility confirmation. Run release checks at the accepted candidate; retain hardware/device and distribution blockers until separately observed.


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

### 2026-11-01 volume and post-callback meter follow-up

The operator clarified the current player volume should be blaring; the GUI
showed 100%, and the AudioQueue is explicitly set to unity (`1.0`). The first
3.6 seconds of the supplied track decode to 44.1 kHz stereo 16-bit PCM with
peak 22,494 and RMS 2,070.4; the full track has 12,525,568 frames, peak
32,768, and RMS 8,008.0. The temporary decode was removed. Source level and
volume control therefore do not explain silence.

In the latest short GUI playback, AudioQueue reported successful queue start,
running state, selected/default device association, and callback completion;
620 buffers / 158,720 frames with 310,952 nonzero samples (peak 22,494) were
submitted. Meter enable/query/dB statuses were all 0, but the maximum reported
level remained 0/1e6 and -120,000 mdB across 35 post-callback observations.
This is evidence that AudioQueue's meter observed no level in that run, not
proof that the signal reached the DAC. I added bounded teardown-time queries
for the queue stream ASBD, associated device sample rate/channel count, and
AudioQueue converter error to distinguish format conversion from downstream
rendering. They compile into the signed app, but a fresh GUI capture of these
fields is still pending; this host's LaunchServices launch of the rebuilt
bundle failed with `kLSNoExecutableErr` despite the bundle executable being
present, so no new hardware conclusion is claimed.

**Status:** user audibility remains negative; MAC07-01 remains **UNVERIFIED**.
The 100% control, unity queue volume and high PCM level make a simple low
volume/source-silence explanation unlikely. Next inspect the new live stream,
device-format and converter fields, then measure beyond AudioQueue (for
example with an authorized system loopback path) if those fields are valid.


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

On a subsequent instrumented GUI run after the explicit unity-gain setting,
the route remained listed/default and the native sink observed 276,224 enqueued
frames, 545,919 nonzero samples, and peak absolute sample 22,494. The player
was stopped after this trace. The operator has since confirmed the interface
jitter is cleaned up and reported that the player remains silent. The
nonzero-sample count locates the remaining failure after PCM reaches the
AudioQueue buffers; it does not prove render or acoustic output. A user-facing
output selector is implemented and was used to test the route handoff.


### 2026-11-01 output selector runtime check

The rebuilt GUI enumerated three output-capable devices in the GUI process,
including the selected system default. The operator explicitly selected the
built-in speakers, then explicitly loaded the supplied album and started
playback. Post-stop AudioQueue diagnostics reported route status 0, current
device listed=yes and target=yes, default=yes, 331,264 frames enqueued,
662,327 nonzero samples with peak absolute sample 7,961, 1,294 callbacks, and
successful stop/drain/dispose. This confirms the explicit UID was accepted as
the queue target and that nonzero PCM reached its buffers. The operator confirmed that this exact selected-route run was still silent;
MAC07-01 remains unverified. The output picker works, but selecting the
built-in speakers did not resolve the audible-output defect.

Integrated verification after adding the selector passed: `sh
scripts/offline-gate.sh`, native tests (31/31), native Clippy with warnings
denied, CLI macOS-output tests (21 passed, 1 ignored, plus 3 integration
tests), app build/signature checks, Swift compile/typecheck, formatting,
roadmap-index/literature validators, and `git diff --check`. These checks do
not substitute for listening acceptance.

### 2026-11-01 live meter and same-file CLI follow-up

The output meter was sampled from the non-real-time write thread after queue
start and after buffer submissions, retaining the maximum over the session.
In the rebuilt GUI on System Default (MacBook Air Speakers), operator
listening again reported silence. The post-stop record showed route status 0,
default and target listed=yes, queue device associated=yes/default=yes/target
yes, 999 buffers / 255,744 frames submitted, 504,970 nonzero PCM samples with
peak 22,494, 1,000 meter observations with a maximum of 122/1,000,000, 999
callbacks, and successful stop/drain/dispose. These meter reads were taken by the producer after enqueue, before confirming
that a callback had retired a buffer. They may therefore be stale/pre-render
zeros and are not valid evidence of a signal-level discrepancy.

The supplied-file persistent-server regression passed (1/1), confirming the
package server can decode the file and deliver frames to its test sink. A
direct native CLI playback attempt failed before buffer allocation with
AudioQueue route lookup OSStatus `-66680`, zero listed default output, and no
PCM submission; this matches earlier command-line-only route failures and is
not evidence of a system-wide output failure. The directive's
`scripts/mac07_flac_ab.sh` is absent from this checkout, so that harness could
not be run. The GUI session remains silent despite successful queue/device
identity checks. MAC07-00/01 therefore remain blocked/in progress; do not
advance dependent passes or mark audible acceptance.

The focused native macOS suite passed 31/31, warnings-denied native Clippy
passed, `cargo fmt --all -- --check` passed, and `sh
scripts/build-macos-app.sh` exited 0 with a valid signed bundle. These are
source/build checks, not acoustic acceptance. Meter sampling is diagnostic-
only, opt-in, and runs outside the real-time callback.

A second rebuilt-app run after throttling meter queries to approximately 10 Hz
still produced no sound per the operator. It submitted 155,648 frames with
304,809 nonzero samples (peak 22,494); route/device/default checks and
608 callbacks succeeded, teardown drained and disposed cleanly, while the
maximum output meter value was 0/1,000,000 across 34 observations. These values came from the same pre-render sampling path and cannot establish
that AudioQueue rendered silence. The operator still reported silence; acoustic
acceptance remains blocked.


### 2026-11-01 meter sampling correction

A read-only review found the earlier meter queries were made after enqueue, not
after a completed callback, and the `ppm` label was not an appropriate unit
for Apple's unitless 0–1 linear meter. Withdraw the earlier inference that
122 ppm or 0 ppm identified a signal-level failure. Instrumentation now keeps
the meter-enable status separate from linear and dB query statuses, samples
after callback retirement (throttled to about 10 Hz), and records AudioQueue
`IsRunning`. A fresh rebuilt-app measurement with these corrected checkpoints
is required before drawing a queue-render conclusion. The operator's report of
silence remains the acceptance result.

### 2026-11-01 real-FLAC multi-track audible playback confirmation

Following the preceding silent observations, the persistent playback worker's
request wait interval was reduced from 10 ms to 1 ms. The supplied first FLAC
contains 50,724 decoded blocks across 12,985,344 frames (about 256 frames per
block on average, or 5.8 ms at 44.1 kHz); waiting 10 ms between worker polls
could leave the native queue without the next small block. The AudioQueue
sink's existing bounded backpressure remains in place.

The rebuilt, locally signed app loaded the supplied 11-track album. With
System Default selected and volume at 100%, the operator selected track 1 and
confirmed **WORKING** while it played. The app showed 231,168 frames submitted
at about five seconds and 394,496 at about eight seconds. It then advanced
through the queue; the operator explicitly confirmed **MULTI-FILE WORKING**.
After Stop, the sanitized AudioQueue summary showed route status 0, default
and target device listed, the current device associated/default/target all
yes, device format 48 kHz stereo, converter status 0/error 0, unity queue
volume, 1,289 buffers / 329,984 frames, 653,422 nonzero samples (peak
22,494), and successful stop/drain/dispose. The user audibility confirmation
is the acceptance evidence; these diagnostics alone do not prove acoustic
output. No media path, device UID, or copyrighted audio was added to the
repository.

This confirms audible real-FLAC playback and queue transitions on the current
Mac session and resolves the immediate silence defect for this supplied
album. The 1 ms polling change is the candidate cause of the earlier short-
block starvation, but the session does not isolate that cause from all other
concurrent native diagnostic changes. MAC07 hardware/device recovery, DAC
measurement, broader corpus, host-focus, and distribution gates remain
separate and open.

### 2026-11-01 AQ-01/AQ-02 format and AudioQueue lifecycle follow-up

The native ASBD regression now checks packed signed integer LPCM layout for
mono/stereo 16/24/32-bit samples at 44.1/48/96/192 kHz, including byte-level
little-endian signed extrema. The table passes against the requested stream
descriptor contract. New opt-in atomic diagnostics report PCM blocks, accepted
frames/buffers, first enqueue time, producer-observed callback release time,
enqueue status, start failures, callback count, write timeouts, and teardown
status. The callback itself still performs only bounded atomic updates and
buffer-slot release; timing is measured by the producer after callback release.

The test-first evidence includes an expected compile failure for the new
diagnostic fields before they were implemented. After wiring and schema changes,
the native macOS feature suite passed 33/33 tests. The macOS bundle build and
signature/lifecycle script passed. The GUI parser accepts the new fixed schema,
and the app build compiles it; the existing Swift test target does not cover the
stderr record parser directly.

AQ-03 same-file route comparison found that the direct CLI and actual desktop
persistent server, run from the engineering shell, both decoded/opened the
supplied FLAC but AudioQueue failed route validation with OSStatus -66680 before
allocating buffers or submitting PCM. The standalone CLI device listing was
empty in that process context, even when the saved output UID was supplied. The
GUI route successfully started AudioQueue and submitted PCM with zero buffer
timeouts; one 36-second GUI run submitted 1,613,312 frames at 44.1 kHz to the
48 kHz device. It observed 3,655 callback-progress changes on the producer
thread, with a maximum 29,554 µs between producer observations and no write
timeouts. This is not a callback-to-callback hardware interval and does not
prove underrun, but it provides a measurable refill gap consistent with the
operator's report of clock-related, periodically oscillating jitter. A complete
4:54 run also submitted all 12,985,344 frames with no write timeout. Neither
run establishes audibility. The environment difference between shell-launched
CLI/server and the GUI-launched child remains unexplained, and the operator's
audibility result for the latest fresh GUI run is pending. Do not use route
identity, frame counts, callbacks, or `Playing` state as acoustic acceptance.

AQ-04 one-versus-three-buffer startup A/B has not been run. The producer-side
gap measurement justifies controlled testing, but does not establish that
startup prebuffering will address sustained jitter. New instrumentation now
also samples in-flight queue depth before each write; a fresh GUI run with this
field is pending because the current app session is playing another track and
was left undisturbed. AQ-05 lifecycle coverage now includes a Prime failure
followed by teardown/reopen; the existing callback starvation, pause/resume,
drain, stop, failed-dispose and recovery tests remain synthetic. AQ-06 full
offline gate passes locally, while operator audibility acceptance remains open.
