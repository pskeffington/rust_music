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

### 2026-11-01 native library and queue interaction pass

The standalone app rebuilt from the current worktree with
`sh scripts/build-macos-app.sh` (exit 0). It passed catalog and snapshot
checks, local ad-hoc signature validation, graceful/protocol-error shutdown,
three repeated process lifecycle cycles, and forced-parent child reaping. The
deterministic synthetic core sweep covered 2,000, 20,000, 100,000, and
250,000 tracks. Last-item search p50/p95 was 573.792/576.500 µs,
5,685.709/5,832.500 µs, 28,165.667/29,352.792 µs, and
69,792.292/70,173.625 µs; full-catalog sorting p50/p95 was 4.284/4.337 ms,
39.132/39.729 ms, 233.342/241.475 ms, and 659.021/664.637 ms.
Search-index payload was 134,000 / 1,340,000 / 6,700,000 / 16,750,000 bytes.
The separate 101-query 20k sample reported 5,531.833 µs p50 / 5,636.666 µs
p95; model construction was 179.304 ms p50 / 185.310 ms p95. Catalog sort
keys are precomputed per track to avoid repeated Unicode case-folding during
comparisons. Darwin `getrusage` reported 225,263,616 bytes peak RSS for the
complete test process, including the 250k synthetic catalog and scrambled
sort fixture. These are core microbenchmarks; they do not include SwiftUI or
the full application process.
The current app/catalog protocol remains capped at 20,000 tracks, so the 100k
and 250k results measure core search and sorting only, not end-to-end library
support.

In a fresh app process, a generated three-file WAV-signature-only catalog was
explicitly loaded. The UI showed all three tracks and remained stopped. The
library sort menu exposed Title, Artist, and Album; the operator selected
Artist and Command-clicked two rows, with the accessible action updating to
`ENQUEUE SELECTED · 2`. Bulk enqueue produced two queue rows without enabling
Pause or Stop. Selecting a queue row enabled reorder controls; Move Up changed
the visible order while playback remained stopped. The operator then selected
a different library track and invoked Play Next. It appeared immediately
after the current queue entry, leaving the existing cursor unchanged. The
Player view reported Ready and explicitly stated that queue selection does not
start playback. This is synthetic UI and state-model acceptance, not audible
output or real-media decoding. That interaction predates the current four-
window composition and does not exercise its scene layout. Playlist multi-add
is implemented in the UI and CLI; the CLI's atomic add-many bound and
traversal rejection passed focused regressions. The current playlist-entry
multi-select, reorder, and remove controls have CLI regression coverage but
were not manually exercised in the locked UI session. Queue and playlist bulk
operations reject over-limit batches atomically.

The UI session ended at the OS lock screen before a final close/relaunch check;
the generated signature fixtures were removed from temporary storage. Sort
preference persistence across restart, compact/expanded layout acceptance,
VoiceOver pilots, end-to-end 100k/250k catalogs, and live real-FLAC Play Next
acceptance were **not run**. The UI now debounces and performs catalog
parse/filter/sort work off the main actor, but the scanner and protocol remain
bounded to 20,000 tracks. No source audio or private media was used.

Focused Rust regressions passed: the library `enqueue_next` preservation/no-
autoplay case passed 1/1, and CLI queue bulk atomicity, Play Next placement,
reorder/current-selection, playlist multi-add atomicity, selected-entry batch
reorder, and batch removal passed. Invalid traversal, duplicate/over-limit
selection, and out-of-range playlist moves/removals preserve stored state. The
final `sh scripts/offline-gate.sh` exited 0 across 17 local crates, 32
path-only edges, zero registry/Git crates, and all workspace tests/builds,
including 36 native AudioQueue simulations. An earlier final-pass attempt
stopped at rustfmt (exit 1); `cargo fmt --all` corrected formatting before
the passing gate. `python3 scripts/check_roadmap_index.py` and `git diff
--check` exited 0. None of these
checks closes same-file CLI route parity, physical device recovery, acoustic
acceptance beyond the separately recorded supplied-album GUI session, or
production release gates.

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

### 2026-11-01 AUI02 native operator-window and library pass

The standalone macOS source now defines independent Player, Playlist Editor,
Media Library, and Equalizer scenes. Shared playback state is retained while
operator windows close and reopen; the app delegate owns final process
shutdown. Window commands can show/focus each scene or all four, and saved
frames use distinct stable names. The initial arrangement places the Player,
Playlist Editor, and Equalizer in a vertical left stack beside the Media
Library. This arithmetic fits a visible display area at least 1,236 points
wide and 648 points high; narrower or shorter displays cannot satisfy all
four windows' current minimum sizes. Live placement and persistence remain
unverified because the Mac UI session ended at the lock screen.

The Player and Library surfaces now expose explicit Play, Play Next, enqueue,
and Add to Playlist actions. The Playlist Editor has named playlists, queue
visibility, selection, and batch add/remove/reorder operations. Rust CLI
playlist and queue batch operations are bounded to 64 entries and apply
atomically. Queue Play Next inserts after the current item without starting
playback. Playlist reorder selection mapping has a dedicated Swift regression
harness covering boundary and separated selections. Equalizer controls remain
disabled because no verified DSP engine is connected; no processing is
simulated. The app's catalog protocol and scanner are still bounded to 20,000
tracks.

The final sh scripts/build-macos-app.sh exited 0 after these UI changes. It
compiled and ran the Swift catalog tests, the playlist-selection regression,
snapshot/bounds checks, and shutdown/process lifecycle checks; the local app
signature was valid on disk and satisfied its designated requirement. The
selection harness reported
PASS: playlist multi-selection remains stable across edge and separated moves.
The current deterministic synthetic catalog sweep measured last-item search
p50/p95 at 677.333/691.417 µs (2,000 tracks), 6,836.417/7,057.500 µs (20,000),
33,375.416/34,504.583 µs (100,000), and 74,550.375/80,939.334 µs (250,000).
Full-sort p50/p95 was 5.001/5.021 ms, 46.749/47.170 ms, 272.580/277.524 ms,
and 676.635/718.912 ms respectively. Cached index payload was 134,000 /
1,340,000 / 6,700,000 / 16,750,000 bytes. A 101-query 20k run measured
6,453.375 µs p50 / 6,648.375 µs p95; model build measured 0.323/0.501 ms
unindexed and 202.488/251.838 ms with cached indexes. Darwin getrusage
reported 224,493,568 bytes peak RSS for the synthetic catalog test process,
including the 250k fixtures; this is not whole-app memory use.

The full sh scripts/offline-gate.sh exited 0 after the Rust queue and
playlist changes, covering 17 local crates, 32 path-only internal edges, zero
registry/Git crates, and 36 native AudioQueue tests. Final
cargo fmt --all -- --check, roadmap-index, literature, and git diff --check
validations passed. The 100k/250k timings exercise core search and sorting
only, not end-to-end catalogs. This pass did not include a fresh four-window
interactive session, VoiceOver/resizing acceptance, real-media playback while
scanning, or hardware tests. It does not change the separate audible playback
confirmation above or close the hardware/device, recovery, host-focus, or
distribution gates.

### 2026-11-01 magnetic operator-window docking update

The four native scenes now support magnetic edge snapping with a persisted
user toggle, plus an Arrange Operator Windows command that opens all four
scenes, restores the left-stack/right-library layout, and brings Player
forward. Per-window autosave names remain in place. Pure AppKit geometry
regressions passed for the 1,440×900-point arrangement, horizontal docking,
vertical docking, distant windows, non-overlapping windows, and a disabled
docking tolerance.

The macOS app build exited 0 after the update. The harness reported
PASS: operator window arrangement and magnetic docking geometry, and the
locally signed app satisfied its designated requirement; process shutdown and
reaping checks also passed. For a live smoke, the same built bundle was copied
to a temporary QA bundle with a distinct identifier and locally re-signed.
The Window menu visibly listed Player, Playlist Editor, Media Library, and
Equalizer. Invoking Arrange Operator Windows left Player focused, with no
library loaded and no playback started. The existing installed Rust Music
instance was not replaced.

Manual dragging close to a peer edge, two-display behavior, small-screen
overflow, VoiceOver, and persistence across an actual application restart
remain unverified. MAC07-03 therefore remains in progress; the geometry test
does not replace live snap and layout acceptance.

### 2026-11-01 compact operator-window layout correction

Correction to the preceding geometry update: its 900×600 and 640×420 frames
were smaller than SwiftUI's declared minimum sizes, so those assertions did not
prove the actual windows would remain on screen. The layout now centralizes
minimum sizes (Player 520×200, Playlist 520×200, Equalizer 480×200, Library
520×360 points). Displays at least 1,076×648 points tile the four minimum-size
windows without overlap; smaller displays use a bounded cascade, with Player
front and the Window menus available to reach the obscured windows. The
1,440×900 arrangement remains unchanged. Resizing uses the same magnetic snap
calculation as moving.

Focused AppKit tests check the tiling threshold, declared minimum sizes and
visible-frame bounds at 900×600 and 640×420 points. These are geometry checks,
not live interaction acceptance. A full app build previously exited 0 with
resize-notification wiring, signing and child lifecycle checks. A temporary QA
copy validated its signature, but LaunchServices refused to launch it with
`kLSNoExecutableErr` even though the declared executable was present. The
installed older Rust Music instance was left untouched. Live resize, compact
screen usability, drag-snap and restart-persistence acceptance remain open.

### 2026-11-01 exact-HEAD pre-merge gate refresh

At clean candidate `5563d7dcc410bd2f80c5e77f45db553429e719d6`,
`RUST_MUSIC_EVIDENCE_DIR=/private/tmp/rust-music-merge-evidence-5563d7d
PRIVATE_FLAC_DIRECTORY=/private/corpus sh scripts/premerge-check.sh` exited
**3**. Roadmap/literature validators and 8 roadmap unit tests passed, the
captured offline Rust gate exited **0**, WAVE parity passed **6/6**, and
AIFF/AIFF-C parity passed **7/7**. The script then stopped because
`/private/corpus` does not exist. No user music was decoded, played or copied.
Afterward, the operator authorized a local SHA-256 match check of the 25
listed FLACs; **0/6** manifest digests matched, so no FLAC parity run occurred
and no audio or track identifiers were copied into Git. The private six-track
FLAC parity gate remains required before merge; captured gate transcripts are
outside Git.

The documented pre-merge script was repeated at clean candidate
`cfb1b130edab07e159d42e5673cb9411f71e1e40` after the authorized path check.
It again passed the offline Rust gate, WAVE 6/6, and AIFF/AIFF-C 7/7, then
exited **3** at the absent `/private/corpus` directory. The six expected FLAC
hashes were not available among the specifically listed files.

At app source commit `91607a5`, `sh scripts/build-macos-app.sh` exited **0**.
Catalog, playlist, compact/tiled window geometry, snapshot bounds, strict local
signature, shutdown coordination, and child-process lifecycle checks passed.
The synthetic catalog harness measured search p50/p95 of 587.542/607.417 µs
(2k), 5,943.417/6,038.541 µs (20k), 31,099.583/31,780.833 µs (100k), and
75,323.916/78,595.791 µs (250k). Full-sort p50 ranged from 4.342 ms (2k) to
742.303 ms (250k); cached-index payload ranged from 134,000 to 16,750,000
bytes. Peak RSS for the synthetic catalog test process was 240,435,200 bytes.
These are core harness measurements, not end-to-end filesystem scans or
whole-app footprint. Roadmap-index, literature and `git diff --check`
validators also passed.

### 2026-11-01 restored-window fitting and app-build refresh

At app source commit `3f359f2`, the restored-frame path now fits saved window
size and position to the current display bounds when the declared minimum
size permits it. The regression starts with a saved 1,000×800-point library
frame outside a 640×420-point display, then verifies an on-screen result at or
above the library's 520×360-point minimum. `sh scripts/build-macos-app.sh`
exited **0**, including this geometry suite, the 2k/20k/100k/250k synthetic
catalog profiles, strict local signing, and process-lifecycle checks. The
latest synthetic test process reported 240,467,968 bytes peak RSS; search p95
was 626.583 µs / 6,344.875 µs / 31,607.666 µs / 74,538.625 µs respectively,
and full-sort p95 was 4.475 ms / 41.423 ms / 304.149 ms / 848.790 ms. These
core harness figures do not establish full-app library performance or
footprint. Compact-screen UI interaction and true restart acceptance remain
open.

At clean current candidate `0f31548faf22714a4107d17179bb80ba9c150f87`,
the required `scripts/premerge-check.sh` was rerun. Roadmap/literature checks,
8 roadmap unit tests, captured offline Rust gate, WAVE 6/6 and AIFF/AIFF-C 7/7
passed; the command exited **3** because `/private/corpus` remains absent. The
exact-head Rust/evidence result is kept outside Git at
`/private/tmp/rust-music-merge-evidence-0f31548/`.

### 2026-11-01 exact corpus supplied; pre-merge gate passed

Correction to the earlier blocked result: the operator subsequently supplied
the exact six private files represented by the manifest. At clean PR head
`9ddbb1bee6de380def4103a42bed761cfbd5e185`,
`RUST_MUSIC_EVIDENCE_DIR=/private/tmp/rust-music-merge-evidence-9ddbb1b
PRIVATE_FLAC_DIRECTORY=<operator-supplied corpus directory> sh
scripts/premerge-check.sh` exited **0**. Roadmap/literature checks, eight
roadmap unit tests, the captured offline Rust gate, WAVE 6/6, AIFF/AIFF-C 7/7,
and native FLAC manifest metadata/hash/PCM parity for L1, L2, L3, G4, G5 and
G6 passed. The captured private log and evidence summary remain under
`/private/tmp/rust-music-merge-evidence-9ddbb1b/`; no audio, filenames or
absolute corpus path were added to Git. The script's NOT CERTIFIED notice
still applies to Rust_Boy Radio/TV focus, physical panel/DAC, and release
acceptance. UI accessibility, multi-display interaction, CLI route parity,
and device recovery remain separate open acceptance items.

### 2026-11-01 RBPK v1 compatibility repair and exact-head gate

At source commit `e662e99e50aa2ac3ee1eb8fe092bd42ae862d809`, removed the
unnegotiated seek request from the RBPK v1 codec, CLI dispatch, and macOS
transport. The player progress indicator is read-only; decoder-level seek
support remains internal. `cargo fmt --all -- --check`,
`cargo test --workspace --offline`, and `sh scripts/build-macos-app.sh` passed.
The macOS build reported a valid signed bundle, passed operator-window and
process-lifecycle checks, and completed 2k/20k/100k/250k synthetic catalog
checks. Search p95 was 0.669/6.002/32.939/111.750 ms and full-sort p95 was
5.108/44.828/269.278/750.076 ms; test-process peak RSS was 240,467,968 bytes.

`RUST_MUSIC_EVIDENCE_DIR=/private/tmp/rust-music-merge-evidence-e662e99
PRIVATE_FLAC_DIRECTORY=<operator-supplied corpus directory> sh
scripts/premerge-check.sh` exited **0** at that exact commit. Roadmap and
literature checks, eight roadmap tests, the captured offline gate (exit 0),
WAVE 6/6, AIFF/AIFF-C 7/7, and all six manifest FLAC metadata/hash/native PCM
checks passed. Private transcripts and audio remain outside Git. This does
not certify physical output hardware, VoiceOver, multi-display interaction,
or release readiness.

### 2026-11-01 deferred operator-window arrangement

A live arrangement check found that opening all four windows could raise a
later scene after the initial layout pass, leaving the Equalizer frontmost
instead of Player. The arrangement request now schedules a final settling pass
after SwiftUI's window presentation. On the rebuilt app, Window > Arrange
Operator Windows left Player focused after 800 ms; the Window menu listed all
four operator scenes. This verifies the Player-front result in that live
session, not multi-display positioning, drag-snap, or compact-size usability.

`sh scripts/build-macos-app.sh` exited **0** with a valid signed bundle;
geometry, protocol, catalog, and process harness checks passed. Synthetic
catalog search/sort passed at 2k, 20k, 100k and 250k entries. Search p95 was
1.071/9.222/51.390/120.151 ms; full-sort p95 was
6.471/58.375/347.719/935.947 ms; harness peak RSS was 224,493,568 bytes.
These harness measurements do not represent end-to-end UI latency.

### 2026-11-01 library track context actions

Added a row context menu in the native Media Library for Enqueue Track, Play
Next, and Add to Selected Playlist. Each command uses the clicked row's track
identity and existing bounded CLI state operation. These actions persist queue
or playlist changes without starting playback. The library's accessibility
hint now also advertises the track context menu.

`sh scripts/build-macos-app.sh` exited **0** after the UI change. The signed
bundle verified; operator-window geometry and process lifecycle checks passed.
Synthetic search and sort passed at 2k, 20k, 100k and 250k entries. On the
final app build in this pass, search p95 was 0.821/6.653/31.269/78.298 ms;
full-sort p95 was 4.937/49.222/295.171/846.936 ms; catalog-harness peak RSS
was 240,762,880 bytes. These are harness
measurements, not end-to-end UI latency.

A live accessibility-tree smoke used a temporary one-track library. Enqueue
changed the visible queue count to 1; Play Next changed it to 2; Add to
Selected Playlist produced one visible playlist entry. The test queue and
playlist were cleared, the temporary fixture was removed, and playback was
never started. This verifies UI-to-state routing only, not audible playback,
keyboard navigation, VoiceOver usability, or multi-window layout acceptance.

### 2026-11-01 library keyboard shortcuts and search focus

The rebuilt app's Command-O shortcut opened the explicit folder picker; the
picker was canceled without loading a library. With a temporary one-track
catalog loaded for UI inspection, Command-F from Player opened Media Library
and focused its search field. Typing a no-match query changed the visible
catalog count from one to zero. One Tab from the selected track list also
moved focus to Search. No playback was started, and the temporary fixture was
removed. These observations verify the folder-picker shortcut and one search
focus path only; they do not establish complete keyboard order or VoiceOver
usability.

`sh scripts/build-macos-app.sh` exited **0** for this candidate. The signed
bundle verified; geometry, protocol, catalog, and process lifecycle checks
passed. Synthetic search/sort passed at 2k, 20k, 100k and 250k entries. Search
p95 was 0.695/7.466/34.126/105.984 ms; full-sort p95 was
4.912/50.369/325.324/1,183.812 ms; catalog-harness peak RSS was
220,889,088 bytes. These synthetic harness results are not end-to-end UI
latency. Keyboard order across the operator windows, VoiceOver, compact-size
interaction, and live multi-display behavior remain open.

### 2026-11-01 disabled library action styling and launch observation

At app source commit `9a65c91f2a1dee17128cfa682a29faae16cb6a73`, the custom
Rust Boy button style now respects SwiftUI's enabled state. In a fresh empty
library, Play Selected, Enqueue Selected, and Play Next appeared muted and the
accessibility tree marked all three disabled. No library was loaded and no
playback was started. Enabled button colors and prominent styling are
unchanged. Independent gate and safety reviews found no blocker for this
narrow styling change; VoiceOver, contrast, and complete keyboard acceptance
were not part of those reviews.

`sh scripts/build-macos-app.sh` exited **0**. The signed bundle passed
validation; operator-window geometry, protocol, catalog, shutdown, and process
lifecycle harnesses passed. Synthetic search and full-sort harness p95 values
at 2k/20k/100k/250k entries were 1.036/10.724/53.999/141.769 ms and
7.497/66.559/463.292/1,326.506 ms, respectively. Catalog-harness peak RSS
was 242,237,440 bytes. These synthetic measurements are harness results, not
end-to-end UI latency or memory profiling of an imported library.

The operator reported that launch starts with Player and then shows the other
windows. In this pass, a fresh launch showed only Player after about 2.2
seconds, and the app source contained no startup call that opens the other
scenes. With Player as the only visible window, invoking Window > Arrange
Operator Windows returned focus to Player after about 900 ms. Playlist and
Equalizer were subsequently reachable from the native Window menu. This
pass does not resolve whether macOS scene restoration explains the operator's
startup observation, nor independently establish the complete desktop
composition after arranging. MAC07-03 remains in progress; this evidence does
not make the change merge-ready or establish release readiness.

### 2026-11-01 operator-window startup restoration continuation

At source commit `f6f5e11`, startup restoration was added following the
disabled-control-state pass above. After Player appears, a delayed check at
250 ms opens missing operator scenes only if Player remains visible. The
restoration path waits for all four registered scenes to become visible,
checking every 100 ms for up to 3 seconds before it raises Player. It does not
request a new global arrangement, so saved named window frames remain in
effect. Closing or hiding Player before the delayed check suppresses peer
opening; a timeout does not focus Player. Reviewers passed the source-level
behavior, but did not independently verify it at runtime.

`sh scripts/build-macos-app.sh` exited **0** on this exact code. In the live
rebuilt bundle, the immediate accessibility tree and screenshot showed
Player / Ready / No Track; after 900 ms Player was still the active window.
No library was loaded and playback did not start. This smoke does not establish
all four windows simultaneously visible, the 250 ms/100 ms/3 second timing,
saved-frame round-trip, or closing/hiding Player during the delayed check.
These require dedicated live acceptance. MAC07-03 remains in progress; this
evidence does not establish release readiness.

### 2026-11-01 startup-restoration branch offline gate follow-up

At source commit `f6f5e11`, evidence commit `659b5cf`, on the clean PR branch,
`sh scripts/offline-gate.sh` exited **0**. It reported 17 local crates, 32
path-only edges, zero registry/Git crates, and passing workspace tests.
`python3 scripts/check_roadmap_index.py` exited **0** (16 gates, 30 tasks),
and `python3 scripts/check_literature.py` exited **0** (27 references, 16
standards). This follow-up does not cover private FLAC corpus parity or GUI,
physical hardware, and release acceptance.

### 2026-11-01 MAC07-03 Library vertical sizing correction

Correction for the UI worktree at branch `codex/mac07-disabled-control-state`,
pre-change HEAD `29c32cf` plus the uncommitted startup-restoration tweak: the
current source change in `apps/rust-music-macos/RustMusicApp.swift` keeps the
Library header and action row fixed while the track list takes the remaining
vertical height. `sh scripts/build-macos-app.sh` exited **0**; the local signed
bundle verified. Its harness passed synthetic catalog search at 2k/20k/100k/
250k tracks, playlist selection, layout, protocol, catalog, and process
lifecycle checks. Harness RSS exceeded 220 MiB; this is not UI-process memory
measurement.

A fresh CUA-launched rebuilt bundle showed Player first and all four named
operator windows in the Window menu. At 1,000×900 screenshot pixels, the
Library screenshot showed the list filling the area between its search row and
action row. No library was loaded and no playback occurred. Manual compact
resizing, scrolling through populated track rows, keyboard traversal, and
VoiceOver were not tested.
This is a bounded visual check; MAC07-03 remains in progress and is not fully
accepted.

Correction to the Library sizing check above: after independent review flagged
crowding at compact heights, the list minimum was lowered to 96 points. The
final build and a fresh CUA Library screenshot were rerun at 1,000×900
screenshot pixels and passed. No live resize to its declared 520×360-point
window minimum was performed, so compact reachability remains unverified.

At exact source HEAD `ddd18547d663ca45a0fb20d920435526c5489c9f`,
`sh scripts/offline-gate.sh` exited **0**: 17 local crates, 32 path-only
edges, zero registry/Git crates, and Rust workspace tests passed. The
macOS app build and signature verification at this same source commit also
exited **0**. These checks do not establish private FLAC parity or GUI/hardware
acceptance.

### 2026-11-01 MAC07-03 empty-library first-run guidance

On branch `codex/mac07-disabled-control-state`, source base `dc5d0a4` with an
uncommitted `RustMusicApp.swift` UI diff, `sh scripts/build-macos-app.sh`
exited **0**. The local signature verified; catalog profiles at
2k/20k/100k/250k, playlist selection, layout, protocol, catalog, and process
lifecycle harnesses passed.

A fresh CUA launch showed Player first, then Library. The Library accessibility
tree and screenshot showed `NO LIBRARY LOADED` and `Choose LOAD LIBRARY to
select a local music folder.` No library was loaded, scanned, or played. Source
also defines states for scanning an initially empty catalog, a successfully
loaded empty catalog, and no search matches, but only the first-run no-library
state was exercised live. This is bounded evidence; the other no-rows states,
compact sizing, keyboard navigation, and VoiceOver remain unverified.

### 2026-11-01 scan-notice source/build evidence addendum

Correction for the uncommitted `RustMusicApp.swift` change on source base
`dc5d0a4`: a fresh `sh scripts/build-macos-app.sh` exited **0**, and the local
app signature verified. The build harness passed synthetic catalog search
profiles through 250k tracks along with playlist selection, layout, protocol,
catalog, and process lifecycle checks. These are bounded harness results;
previously recorded catalog timings and RSS describe synthetic harness runs,
not live UI scanning or imported-library memory.

The scan-in-progress, scanner-start failure, folder-scan failure, and canceled
scan notices are source/build verified only. No live scan, failure, or cancel
state was exercised. The prior live check covered only the first-run
`NO LIBRARY LOADED` state. This does not establish scan-state UI acceptance.

### 2026-11-01 Library minimum-size enforcement correction

Correction to the initial source review: `playerWindow(_:)` already applied a
SwiftUI minimum frame from `PlayerWindowKind.minimumWidth` and
`minimumHeight`, including the Library's 520×360-point minimum. The additional
scene-level frame in commit `ed078de` duplicated that constraint; it did not
prove native resize enforcement. A fresh review moved the enforcement into
`WindowFrameObserver`, which now assigns the matching value to
`NSWindow.minSize` for each operator window while retaining the SwiftUI content
minimum and restore-time frame fitting.

`git diff --check` passed and the rebuilt candidate's
`sh scripts/build-macos-app.sh` exited **0**, including strict local bundle
signature validation, catalog/playlist/window/protocol checks, and repeated
process lifecycle tests. This synthetic run measured 250k last-item search
p95 at 78.330 ms, full-sort p95 at 809.712 ms, and catalog harness peak RSS at
240,402,432 bytes; these are core harness measurements, not live UI scan or
playback-under-load results.

A fresh UI launch briefly reported Playlist Editor as the current window;
after 1.8 seconds, `getApp` reported Player with Ready/No Track and no output
snapshot. This matches the existing delayed Player focus behavior, but the
initial transient remains visible to an immediate observer. The Library was
observed at its large default size. A live resize below 520×360 points has
not been measured; UI automation failed to target the corner consistently.
MAC07-03 compact-size acceptance remains open. The candidate remains on PR #9,
not merged-mainline evidence.

After that build, macOS Window → Move & Resize → Top Left reduced the live
Media Library to a 736×454-pixel CUA screenshot. Search, sort, the no-library
message, and the action row remained visible; the accessibility tree retained
the search, sort, and action controls. No library was loaded and no playback
started. This is a compact live layout observation in screenshot pixels; it
does not establish AppKit point dimensions or minimum-size enforcement. The
direct corner resize still could not be targeted by UI automation.

### 2026-11-01 populated Library keyboard focus correction

On candidate source `c595ed1` plus an uncommitted accessibility-hint change, a
temporary one-track metadata-only FLAC catalog was explicitly loaded. The
selected track appeared in the Library list. Pressing Tab from that row moved
keyboard focus to the Library Search field. No playback was started; the
temporary fixture was removed after quitting the app. This observation covers
one focus transition only and does not establish the complete Library or
cross-window keyboard order, or VoiceOver usability.

The track-list accessibility hint was corrected to describe the observed Tab
destination and directs queue/playlist actions to the row context menu. A
fresh `sh scripts/build-macos-app.sh` exited **0** after the change, including
strict app signature verification and the catalog, playlist, geometry,
protocol, and process lifecycle harnesses. The 250k synthetic catalog run
reported last-item search p95 72.746 ms, full-sort p95 703.141 ms, and peak
RSS 240,451,584 bytes; these do not measure GUI scan or playback-under-load
performance. The candidate remains on PR #9 and is not merged-mainline
evidence.

### 2026-11-01 Playlist keyboard hint correction and launch focus

On candidate source `4246f03` plus uncommitted SwiftUI changes, a temporary
one-track metadata-only catalog and one-entry playlist were used for a
keyboard smoke. Tab from the selected Playlist Editor entry focused the Queue
list, not the Move Up/Down or Remove buttons. The playlist-entry hint was
changed to name the actions that operate on the selection without promising a
Tab path. The temporary catalog and playlist state were removed by quitting
the app and deleting the temporary root. No playback was started.

The same UI pass reproduced startup focus delay: a fresh launch initially
showed Playlist Editor, then Player about 1.8 seconds later. A one-shot
Player-focus attempt did not change that result and has been removed. Startup
focus therefore remains unresolved. At source `4246f03` plus the then-current
uncommitted UI changes, `sh scripts/build-macos-app.sh` exited **0**, including
strict bundle signature validation and catalog, playlist, geometry, protocol,
and lifecycle harnesses. The 250k synthetic catalog harness reported search
p95 73.439 ms, full-sort p95 705.136 ms, and peak RSS 240,369,664 bytes; these
are not GUI scan or playback-under-load measurements. The candidate remains
on PR #9, not merged-mainline evidence.

### 2026-11-01 Playlist VoiceOver actions

The Playlist Editor's selected-entry list now exposes named accessibility
actions to move the selected entries up or down and remove them. These invoke
the existing bounded playlist operations and refuse to run while state or
transport work is busy. The visible Move Up, Move Down, and Remove buttons
remain available. The accessibility hint now explains that VoiceOver can
invoke these actions. Apple documents named SwiftUI accessibility actions as
invocable by assistive technologies including VoiceOver
([API reference](https://developer.apple.com/documentation/swiftui/view/accessibilityaction%28named%3A_%3A%29)).

`sh scripts/build-macos-app.sh` exited **0** at candidate source `4246f03`
plus uncommitted changes. It compiled the macOS 13-targeted SwiftUI app,
validated the signed bundle, and passed catalog search at 2k/20k/100k/250k,
playlist-selection, operator-window geometry, playback snapshot, bounded
catalog, and process lifecycle harnesses. The 250k synthetic profile reported
search p95 72.169 ms, full-sort p95 708.649 ms, and process peak RSS
240,418,816 bytes for the catalog harness. The CUA session currently shows
the Player window Ready / No Track. VoiceOver execution of the new actions,
selected-entry persistence, and complete keyboard traversal have not been
exercised. This candidate is not merged-mainline evidence.

An independent read-only gate review of source `7648cea` found no blocking
source issue: the actions use the existing selection-validating playlist
operations (bounded to 64 entries) and include busy-state guards. The review
did not invoke VoiceOver. Next acceptance requires a populated temporary
playlist: confirm all three actions are discoverable, reorder/remove selected
rows, check boundary and busy states, and verify VoiceOver announces the
updated selection. Test keyboard-only operation and focus traversal
separately; custom VoiceOver actions do not establish keyboard operation.

### 2026-11-01 playlist batch argument UI regression

A live CUA run on candidate source `58afb7b` used a temporary three-track
metadata-only FLAC catalog. Multi-selection in the Library worked, but
`ADD SELECTED` failed and Player reported `too many or oversized state
arguments`. Inspection found the CLI entry point accepted only two trailing
state arguments, while `playlist add-many` supports a playlist name plus up
to 64 track paths and `move-many` supports a name, direction, and 64 indexes.
The parser now collects at most 66 arguments, retaining the 4096-byte per
argument bound. No playback started during this run.

After rebuilding the app with the fix, retrying `ADD SELECTED` created the
three requested playlist entries. Moving the selected second and third
entries up preserved their order and selection; removing the selection left
the two unselected entries. The temporary playlist and library were deleted,
the app was quit, and both the synthetic catalog and its root-scoped playlist
state file were removed. The rebuilt app was reopened and showed Player /
Ready / No Track, no output snapshot, and disabled transport controls. No
playback was started. VoiceOver was observed off, temporarily enabled for the
accessibility check, then restored off; CUA did not expose or invoke the named
custom actions, so live VoiceOver action acceptance remains unverified.

`cargo fmt --all -- --check` exited **0**. `cargo test --locked --offline -p
rust-music-cli` exited **0** with 17/17 unit tests and 1/1 subprocess integration test passing. The unit tests include the state-argument bound regression; the
subprocess test creates a named playlist and adds three paths through separate
CLI invocations. `sh scripts/build-macos-app.sh` exited **0**
on the formatted candidate source plus the uncommitted parser fix; the macOS
13 app compiled, signature validation passed, and the catalog, playlist,
window-geometry, playback-snapshot, bounded-catalog, and process-lifecycle
harnesses passed. The synthetic 250k catalog run measured 108.448 ms search
p95, 1,138.401 ms full-sort p95, and 240,467,968 bytes catalog-harness peak
RSS. These are synthetic catalog-harness figures, not UI scan or
playback-under-load measurements.

### 2026-11-01 Library keyboard selection and batch actions

On candidate branch `codex/mac07-disabled-control-state`, source `9780bf4`
plus uncommitted SwiftUI changes, a rebuilt app was exercised with three
generated metadata-only FLAC signatures (no audio frames). The folder was
explicitly loaded. Command-F opened the Library and focused Search after the
catalog was available; typing `gamma` returned one track. Tab moved between
Search and the track list, and Shift-Down extended the selection to all three
rows. In the observed macOS focus mode, repeated Tab cycled Search and the
list, skipping the visible sort and batch-action controls. This motivated
adding Library menu commands for Play Selected, Enqueue Selected, and Play
Next, with Command-Return, Command-Shift-Return, and Command-Option-Return
shortcuts. The track-list accessibility hint now points users to those menu
actions.

In the rebuilt app, the first Command-Shift-Return probe ran while the Library
menu remained open and added an earlier `01 Alpha Probe` row. After dismissing
the menu, selecting all three rows, and invoking the shortcut, the Playlist
Editor showed four queue rows total: the earlier `01 Alpha Probe` plus the
three requested selected tracks. The queue was cleared to 0 before the
separate Play Next test. Command-Option-Return then inserted the selected
`03 Gamma Probe` track as the only queue row. The Play Selected shortcut was
listed but not invoked. No playback was started. The app was quit before
removing the generated catalog and its exact root-scoped queue state file;
both were verified absent. On a subsequent launch, the first accessibility
snapshot showed Playlist Editor; after about 2.5 seconds Player was frontmost
with Ready / No Track, no output snapshot, and disabled transport controls.
The startup focus delay remains open. This does not establish VoiceOver
operation, complete Tab traversal, or playback acceptance.

`sh scripts/build-macos-app.sh` exited **0** on source `9780bf4` plus the
uncommitted menu commands and accessibility-hint change. Strict local bundle
signature validation passed; the catalog-search, playlist-selection,
operator-window geometry, playback-snapshot, catalog-bounds, and process
lifecycle harnesses passed. The synthetic 250k catalog harness reported
search p95 **74.091 ms**, full-sort p95 **708.465 ms**, and peak RSS
**240,418,816 bytes**. These are catalog-harness measurements, not full-app
memory, imported-library UI latency, or playback-under-load acceptance. The
candidate remains on open PR #9 and is not merged-mainline or production
readiness evidence.

### 2026-11-01 Playlist creation and batch-add keyboard workflow

On candidate branch `codex/mac07-disabled-control-state`, source `0abffa0`
plus the uncommitted SwiftUI change, the rebuilt app was exercised with an
explicitly loaded three-file metadata-only FLAC fixture (137 bytes per file,
no audio frames). Command-N opened Playlist Editor and focused the new-playlist
name field; typing `Keyboard Set` and pressing Return created and selected the
playlist. Command-F opened Library search. Shift-click selected all three
catalog rows, and Command-Option-P added them to the selected playlist. The
Playlist Editor showed `Keyboard Set, 3` and the three filename paths. The queue
remained empty. No transport action or playback occurred.

The temporary playlist was deleted through the UI, the app was quit, the
generated fixture was removed, and the rebuilt app was relaunched. Its fresh
accessibility state showed Player / Ready / No Track, an empty queue, and no
output snapshot. This confirms the exercised keyboard path for playlist
creation and batch add; it does not establish complete keyboard traversal,
VoiceOver operation, audio playback, or the full MAC07 acceptance gate.

`sh scripts/build-macos-app.sh` exited **0** on source `0abffa0` plus the
uncommitted SwiftUI change. Bundle signature validation and the catalog-search,
playlist-selection, operator-window geometry, playback-snapshot,
catalog-bounds, and process-lifecycle harnesses passed. The 250k synthetic
catalog harness reported search p95 **73.554 ms**, full-sort p95 **821.209 ms**,
and peak RSS **240,484,352 bytes**. These are catalog-harness measurements,
not full-app UI or playback-under-load measurements. The compiler emitted
unused diagnostic-code warnings in the native macOS audio module. A shell
`open` attempt returned Launch Services `kLSNoExecutableErr`; the CUA desktop
controller subsequently launched the exact rebuilt bundle and the live UI
workflow completed. The candidate remains on open PR #9 and is not
merged-mainline evidence. A read-only safety review found no actionable issue
in the keyboard-command diff.

### 2026-11-01 MAC07-03 compact Player transport layout

On candidate branch `codex/mac07-disabled-control-state` at source `e3343fd`,
with no library loaded, a live resize reached approximately 520×229 screenshot
pixels. At that size the Player transport buttons wrapped labels such as
`PREVIOUS` and `PAUSE`, and the volume label also wrapped. These are screenshot
pixel dimensions, not AppKit points; the observation exposed a compact-layout
usability defect.

The working-tree SwiftUI change uses `ViewThatFits` to retain the full labels
when they fit and select compact `PREV` / `VOL` labels with a narrower volume
slider when they do not. Full accessibility labels and the existing Rust Boy
colors and button styling are retained. The rebuilt app was observed at a
wider approximately 620-pixel screenshot width, where the full labels remain
on one line. Attempts to resize the rebuilt app back to the compact width
failed with CUA `windowNotFoundAtPosition` errors, so the compact fallback has
not yet been live-verified at its trigger width. No playback occurred; Player
remained Ready / No Track.

`sh scripts/build-macos-app.sh` exited **0** on the working-tree change;
bundle signature validation and the included source harnesses passed. The
synthetic 250k catalog harness reported search p95 **78.259 ms**, full-sort
p95 **824.418 ms**, and peak RSS **240,386,048 bytes**. These are catalog
harness measurements, not compact-window or playback-under-load results.
MAC07-03 remains in progress; compact fallback live verification is pending.

### 2026-11-01 MAC07-03 library sort keyboard commands

On candidate branch `codex/mac07-disabled-control-state`, source `69a7606`
plus the working-tree SwiftUI change, the Library menu now exposes Sort Library
by Title, Artist, and Album with Option-Command-1/2/3 shortcuts. The sort picker
hint points to those commands. `sh scripts/build-macos-app.sh` exited **0**;
the local bundle signature and included catalog, playlist-selection, window
geometry, playback-snapshot, catalog-bounds, and process-lifecycle harnesses
passed. The 250k catalog harness reported search p95 **79.580 ms**, full-sort
p95 **816.622 ms**, and peak RSS **242,221,056 bytes**; these are catalog
harness measurements, not full-app or playback measurements.

Live CUA explicitly loaded three generated metadata-only FLAC test entries
(118, 118, and 119 bytes, with no audio frames). Starting with ARTIST sort,
the visible order was Mike / Alpha, Zulu / Echo, Alpha / Zulu. Option-Command-1
changed the picker to TITLE and the rows to Alpha / Zulu, Mike / Alpha,
Zulu / Echo. Option-Command-2 restored ARTIST ordering; Option-Command-3
changed the picker to ALBUM and rows to Mike / Bravo, Zulu / Delta,
Alpha / Echo. The Library menu accessibility tree exposed all three named
commands. No playback or queue action occurred; the Player remained Ready / No
Track at fresh launch. During the loaded-catalog interaction, playback stayed
stopped and the output panel had no playback snapshot.

After quitting, removing the generated fixture, and relaunching the rebuilt
app, Command-F opened Media Library with sort still set to ALBUM, 0 tracks,
and NO LIBRARY LOADED. This verifies sort-preference persistence without
catalog restoration or startup scanning. The generated fixture was removed
and the app was left open idle. This is a focused keyboard and preference
check only: MAC07-03 remains in progress, the compact Player transport
fallback still lacks live verification, and full keyboard traversal,
VoiceOver, layout, playback, and hardware gates remain open.

### 2026-11-01 MAC07-03 operator-window controls smoke

On candidate `0a4c343bda129cb7034dc55f38d8de2c5ed5e26c`,
`sh scripts/build-macos-app.sh` exited **0** and the rebuilt bundle signature
verified. The desktop CUA launched the bundle with Player in front; it showed
Ready / No Track and no output snapshot. After quitting and relaunching the
unloaded app via CUA, without invoking Show All or Arrange, the active AX window
remained Player / Ready / No Track after a 2.5-second pause. The Window menu
exposed Player, Playlist Editor, Media Library, Equalizer, individual Show
actions, Show All, and Arrange Operator Windows. Raising Playlist Editor
showed an empty queue and empty named-playlist list. Raising Equalizer showed
all ten band sliders disabled with the message that no verified DSP processor
is connected. Raising Media Library showed 0 tracks, `NO LIBRARY LOADED`, and
disabled search and track actions. Show All and Arrange were then explicitly
invoked; Player remained the active AX window afterward. This confirms those
exercised window controls and empty states; AX observations do not establish
that all windows were simultaneously visible or that their arranged geometry
was visually accepted.

No folder was loaded, no scan was performed, and no playback occurred. Compact
and expanded geometry and full keyboard traversal were not verified in this
smoke. A Full Keyboard Access toggle attempt did not produce a reliable
comparison, so that traversal check is inconclusive. A shell `open` attempt
returned Launch Services `kLSNoExecutableErr` even though the executable was
present and the signature valid; the CUA controller subsequently launched the
same bundle successfully. MAC07-03 remains in progress; compact sizing,
keyboard, VoiceOver, playback, and hardware acceptance remain open.

### 2026-11-01 Rust-only offline gate recovery

At initial source `0b1f6cb`, `sh scripts/offline-gate.sh` exited **1** because
the strict dependency audit rejected the exact Cargo-built CLI process spawn in
`crates/rust-music-cli/tests/state_cli_batch.rs`. This was a narrow audit
allowlist mismatch. The scoped allowlist/test correction was committed as
`246baceca04ae6aefe7eaa76c83aeafaa4c12eb5` on the remote candidate branch.
The owner reran `sh scripts/offline-gate.sh` successfully immediately before
that source commit with identical tracked code content: exit **0**, 17 local
crates, 32 path-only internal edges, zero registry/Git crates, and zero
third-party system links. Fmt, Clippy, workspace tests, release build, and the
macOS feature check passed.

An independent reviewer at the same code content ran
`cargo test --locked --offline -p rust-music-audit` (exit **0**, 2/2 tests),
the dependency audit command (exit **0**), and `git diff --check` (exit **0**).
This records a passing Rust-only offline gate on the exact tracked code at
candidate head `246baceca04ae6aefe7eaa76c83aeafaa4c12eb5`; the gate ran
immediately before the source commit, whose tracked code is identical. It does
not establish a macOS app build, real FLAC playback, private-corpus parity,
DAC/hardware acceptance, or completion of any MAC07 pass.

### 2026-11-01 MAC07-03 compact Player transport verification

On the live candidate app, Player was resized by dragging its bottom-right
corner to a captured **539×339 screenshot pixels**. The compact transport
displayed `PREV / PLAY / PAUS / STOP / NEXT` and `VOL 100%` on one row without
wrapping. Its accessibility tree retained the full descriptions Previous,
Play, Pause, Stop, Next, and Volume. After Arrange, a **620×292 screenshot
pixel** capture showed the expanded labels `PREVIOUS / PLAY / PAUSE / STOP /
NEXT / VOLUME 100%` on one row. Both captures retained Rust Boy styling and
showed Ready / No Track with no output snapshot. Screenshot pixels are not
AppKit point dimensions. No folder scan or playback occurred.

This verifies the Player transport's compact-label fallback and expanded
labels only. Compact Library row retention, dropped-track behavior, complete
keyboard traversal, VoiceOver, and overall MAC07-03 acceptance remain
unverified; no pass status is changed.

### 2026-11-01 MAC07-03 keyboard navigation guidance

With macOS `Use keyboard navigation to move focus between controls` enabled
temporarily, Tab traversed Show/focus operator window, Load Library, Search,
Sort, Library list, Play Selected, Enqueue Selected, and Play Next. Shift+Down
selected two adjacent rows and updated the Enqueue Selected count to 2; Up
restored a single row. No action button was activated and the metadata-only
fixture was not played. The macOS setting was restored to its original off
state. The normal setting's Search/List path was separately observed. This is
an operator check of keyboard traversal and multi-selection, not a VoiceOver
acceptance result.

The rebuilt app's Sort control now exposes an accessibility hint explaining
the macOS setting required to Tab through every control. The Library list
hint was updated experimentally, but a loaded-catalog AX inspection with the
list focused exposed only its label and rows, with no Help attribute. A native
`.help` tag experiment also did not expose Help in that tree, so the list hint
was restored to its prior text rather than claiming the new guidance is
announced. A three-track generated metadata-only fixture (141, 141, and 143
bytes; no audio frames) was loaded for this check and then removed after
quitting. Playback remained stopped with no output snapshot. The Sort hint is
visible in AX; runtime exposure of the list hint remains unverified. Compact
Library sizing, VoiceOver, and overall MAC07-03 acceptance remain open.

### 2026-11-01 MAC07-03 Library rows and restart state

An explicitly loaded generated metadata-only FLAC fixture contained three
files (117, 119, and 119 bytes) with no audio frames. Media Library
accessibility state showed 3 tagged tracks, Alpha, Bravo, and Charlie. At a
captured **736×462 screenshot pixels**, after macOS Top Left tiling, the
visible Library table showed all three rows. A drag attempt to shrink the
Library failed with CUA `windowNotFoundAtPosition((-724.0, 1943.0))`; therefore
compact Library row retention and no-dropped-row behavior remain unverified.
Screenshot pixels are not AppKit point dimensions.

After quitting, deleting the generated fixture, and relaunching, Media Library
showed 0 tracks, `NO LIBRARY LOADED`, and disabled actions. No scan or playback
occurred. On this relaunch, initial CUA state was Playlist Editor and after a
2.5-second pause it was Player; an earlier fresh launch remained at Player
after the same delay. This timing variation is recorded without inferring
which other windows were visible. The app was left idle in Player. These
observations do not establish compact Library acceptance, complete keyboard
traversal, VoiceOver, or overall MAC07-03 completion.

### 2026-11-01 MAC07-03 compact Library tile check

On the rebuilt app at source `0c5c095`, macOS `Window > Move & Resize > Top
Left` tiled Media Library to a captured **736×462 screenshot pixels**. A
generated three-track metadata-only FLAC catalog (147, 147, and 149 bytes; no
audio frames) showed Alpha, Bravo, and Charlie together with artist, album,
and format details. The count was 3 TRACKS; no search or queue action was
performed. Playback remained stopped with no output snapshot. This confirms
those three rows at this tiled size; screenshot pixels are not AppKit points
and this is not a test at the declared minimum window frame or of larger
catalog row scrolling.

The app's Arrange Operator Windows command returned focus to Player. After
quitting, removing the generated fixture, and relaunching, the initial CUA
snapshot focused Playlist Editor; after 2.5 seconds Player was front with
Ready / No Track, an empty queue and playlist list, and no output snapshot.
This records a delayed startup-focus transition without inferring which
windows were simultaneously visible. No playback or unrequested scan occurred.

### 2026-11-01 MAC07-03 Player-first startup focus

An earlier rebuilt-app launch produced an initial Playlist Editor AX window and
then Player after 2.5 seconds. The launch restoration now explicitly opens the
existing Player scene after requesting the other operator scenes, retaining the
existing Player-focus settling check. This keeps Player as the final requested
scene without changing frame geometry or styling.

`sh scripts/build-macos-app.sh` exited **0** on the updated SwiftUI source.
Strict local signature validation passed, as did the app's catalog, playlist,
window geometry, playback-snapshot, catalog-bound, and process-lifecycle
harnesses. The 250k synthetic catalog run reported search p95 **73.805 ms**,
full-sort p95 **712.662 ms**, and catalog-harness peak RSS **240,484,352 bytes**.
These are machine-load-sensitive harness measurements, not interactive UI or
playback-under-load results.

After confirming the app process had exited, two fresh CUA launches initially
showed Player / Ready / No Track / no output snapshot. After 2.5 seconds each,
the accessibility tree still identified Player as the active window. The app
was left open idle in Player. No library was loaded and no playback occurred.
This verifies Player focus at the observed launch and settled snapshots; it
does not establish simultaneous visibility or the focus of every opening
frame during the secondary-window restoration sequence.

### 2026-11-01 Playlist accessibility-action invocation

The rebuilt app explicitly loaded three generated metadata-only FLAC files
(152, 152, and 154 bytes; no audio frames). A temporary named playlist,
`VoiceOver Action Check`, was created and populated with all three entries.
The selected-entry list's AX element exposed the named secondary actions
`Move selected entries up`, `Move selected entries down`, and `Remove selected
playlist entries`. Invoking them through the CUA accessibility-action API
reordered Alpha below Bravo, removed the selected Alpha entry, then moved
Charlie above Bravo. The action names remained exposed after each update.

This verifies action exposure through macOS accessibility APIs and the
resulting playlist edits; it does **not** verify VoiceOver speech, rotor
navigation, or spoken selection announcements. The generated playlist was
deleted through the UI, the app was quit, the fixture directory was removed,
and a fresh launch showed Player / Ready / No Track / no output snapshot. No
playback occurred. MAC07-03 VoiceOver acceptance remains open.

### 2026-11-01 MAC07-03 loaded Library hint inspection

To inspect the Library list's accessibility help on a loaded catalog, the
rebuilt app explicitly loaded three generated metadata-only FLAC fixtures
(141, 141, and 143 bytes, no audio frames). The focused list's accessibility
tree exposed its label and rows but no `Help` value. A temporary native `.help`
tag did not change that observation, so the ineffective experimental list
guidance was removed and its prior hint text restored. The Sort control does
expose its updated `Help` value explaining how to enable full Tab traversal.
The app was quit before the generated fixture was removed, then relaunched to
Player / Ready / No Track with no output snapshot. No playback occurred.

After restoring the list hint text, `sh scripts/build-macos-app.sh` exited
**0** on the final source state. Strict bundle signature validation and the
included catalog, playlist-selection, window-geometry, playback-snapshot,
catalog-bound, and process-lifecycle harnesses passed. The 250k synthetic
catalog run measured search p95 **117.599 ms**, full-sort p95 **1,051.471 ms**,
and catalog-harness peak RSS **225,263,616 bytes**. These harness measurements
are machine-load-sensitive synthetic catalog results, not full-app memory,
interactive scan latency, or playback-under-load evidence. The app was left
open idle in Player. MAC07-03 remains in progress.

### 2026-11-01 MAC07-03 Library sort control visibility

In the rebuilt workspace app, the Media Library sort picker clipped the
selected value to `AL...` at its 108-point frame. The picker width is now 132
points with higher HStack layout priority so the active Title, Artist, or Album
sort remains readable as the window narrows. After rebuilding, the live
Library showed the full `ALBUM`, `TITLE`, and `ARTIST` values in turn; the
existing Album preference was restored. Rust Boy colors and the rest of the
library layout were unchanged.

`sh scripts/build-macos-app.sh` exited **0**, including the focused catalog,
playlist-selection, window-geometry, playback-snapshot, catalog-bound and
process-lifecycle harnesses; strict app signature verification passed. The
rebuilt app was quit and relaunched from `target/macos/RustMusic.app`; after
2.5 seconds Player was focused with Ready / No Track and no output snapshot.
The Library check used an empty catalog: no folder was loaded, scanned, or
played. This verifies sort-label visibility and the observed startup focus
only. Compact Library sizing with rows, scrolling, VoiceOver, and overall
MAC07-03 acceptance remain open.
