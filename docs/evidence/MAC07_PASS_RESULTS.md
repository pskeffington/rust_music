# MAC07 pass evidence register

**State: PARTIAL REPRODUCTION; AUDIO/HARDWARE ACCEPTANCE BLOCKED.** Do not store private filenames, absolute paths, music, or media hashes here. Baseline source: `e9cacb1d8d1a3464fd1ba973449767f7152cffd1` on `build/native-codecs-g0`.

| Pass | Source / commands | Observation | Result / blocker |
| --- | --- | --- | --- |
| MAC07-00 | Same-file CLI attempts; `system_profiler SPAudioDataType` | CLI `AudioQueueStart` returned OSStatus `-66680` twice. Local SDK header identifies `kAudioQueueErr_InvalidDevice`. `system_profiler` listed no output devices. | **BLOCKED / PARTIAL.** No usable output route was available to classify CLI-versus-GUI audibility. |
| MAC07-01 | Persistent-server `CountingSink` regression, run for each of the 11 operator-supplied FLACs on the active branch: `RUST_MUSIC_TEST_FLAC=<operator-supplied local FLAC> cargo test --locked --offline -p rust-music-cli --features macos-output -- --ignored package_stream_external_flac_transport` | 11/11 runs passed. This proves FLAC decode/frame delivery to the counting test sink, not native device output. Separately, the operator reports the GUI is silent although the UI counted submitted frames. Instrumented server output recorded start status `-66680`, one enqueued buffer / 256 frames, one callback, and successful stop/drain/dispose. Typed bounded AudioQueue lifecycle counters and a bounded Swift stderr parser are implemented; native tests passed 22/22, the offline gate and signed app build passed, and a synthetic parser harness accepted the valid record and rejected malformed/path-bearing records. The live GUI diagnostic-summary check remains pending because the Mac is locked. | **IN PROGRESS / UNVERIFIED.** The lifecycle instrumentation is implemented, but no audible playback or hardware acceptance is established; GUI display of the summary remains unverified. |
| MAC07-02 | No acceptance run recorded | GUI frame count alone is not validated against audible device output; telemetry/parser work is underway. | **NOT VERIFIED.** Depends on MAC07-01 acceptance. |
| MAC07-03 | No acceptance run recorded | No compact/expanded layout or keyboard acceptance recorded for this pass. | **NOT RUN.** |
| MAC07-04 | No acceptance run recorded | No audible queue transition or operator transport acceptance recorded. | **NOT RUN.** |
| MAC07-05 | No acceptance run recorded | No playlist/persistence/path-safety acceptance recorded. | **NOT RUN.** |
| MAC07-06 | No acceptance run recorded | No A/B run, UX pilots, or acoustic evidence recorded. | **NOT RUN.** |
| MAC07-07 | Documentation synchronization only | Roadmap and task routing were updated; release gate, app/hardware acceptance and independent safety review are not implied. | **NOT COMPLETE.** |

## Interpretation

The current host's AudioQueue invalid-device result and absence of enumerated output devices prevent audible hardware acceptance. The operator's report of GUI silence is preserved as reported evidence; UI frame counters are not acoustic evidence. The 11/11 `CountingSink` result is decoder/frame-delivery evidence only. No MAC07 audio, hardware, or production acceptance pass is recorded.

## Remaining acceptance sequence

Restore a usable output route, rerun same-file CLI and GUI on that route, diagnose and repair a proven handoff defect, then validate progress/state, layout, transport, playlists, and A/B behavior. Capture sanitized commands, source SHA, and operator audibility confirmation. Run release checks at the accepted candidate; retain hardware/device and distribution blockers until separately observed.
