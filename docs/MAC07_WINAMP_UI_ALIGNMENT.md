# MAC07 — Winamp-functional desktop alignment

Status: **ACTIVE TASKING; supplied-album GUI audibility confirmed; CLI parity and hardware acceptance open** (2026-11-01). The operator-supplied Winamp reference informs interaction semantics only. Do not reproduce copyrighted imagery or skins.

**Progress correction (2026-11-01):** the dated evidence ledger records the
operator's real-FLAC GUI and multi-file audible confirmation. MAC07-00/01 remain
in progress because the identical CLI/server route cannot be verified from the
engineering shell and device recovery remains open. RM-029 is in progress:
snapshot decoding tests, persisted title/artist/album ordering, accessible
multi-selection, bounded bulk queueing, Play Next, and queue reorder/clear are
implemented. The native app now defines separate Player, Playlist Editor,
Equalizer, and Media Library windows over one shared playback model. Initial
positions place Player, Playlist, and Equalizer down the left and Library on
the right; native frame autosave preserves each window's placement. EQ controls
stay disabled because no verified DSP is connected. Catalog parsing and
debounced filtering/sorting run away from the main actor. A synthetic
three-track UI exercise confirmed selection and queue actions without
playback; the current four-window layout has compiled but is not yet exercised
in a live UI session. Compact/expanded resizing, keyboard, and VoiceOver
acceptance remain open. The catalog, scanner, and v1 playback protocol remain
limited to 20,000 tracks. The named AUI02 reference file is absent from this
checkout; this pass follows the operator requirements supplied in the task.

## Evidence boundary and current state

- At source baseline `e9cacb1d8d1a3464fd1ba973449767f7152cffd1`, two same-file CLI playback attempts failed at `AudioQueueStart` with OSStatus `-66680`; the local SDK header maps this to `kAudioQueueErr_InvalidDevice`.
- Before explicit route binding, the GUI queue had an associated device UID that did not match CoreAudio's default output UID. Sound settings showed MacBook Air Speakers selected, but an independent CoreAudio query found no default output ID and a zero-length device list while idle. Reselecting the built-in speakers did not change that query result.
- On the active branch, the persistent-server `CountingSink` regression passed for all 11 supplied FLACs. This verifies decoder/frame delivery to a counting test sink; it does not verify AudioQueue output or audibility.
- Native AudioQueue telemetry now compares the queue UID with macOS's default output and explicitly sets `kAudioQueueProperty_CurrentDevice` to that default during open. If CoreAudio has no default, opening the sink fails before PCM submission. The offline gate and macOS app build/signature checks pass.
- After a fresh bundled-app launch, GUI playback reported route status 0, device-query status 0, an associated UID matching the default, 1,059 callbacks, completed drain, and successful stop/dispose after 271,104 frames were enqueued. A same-file direct CLI attempt then failed route setup with `-66680` before enqueueing frames. CoreAudio idle query returned default device ID 0 and no enumerated devices. Audible comparison remains unresolved; route/callback evidence is not audible acceptance.
- In a later live session, the operator again reported no sound. While the app UI showed active progress, the independent command-line CoreAudio probe returned default output ID 0 and no devices; `system_profiler SPAudioDataType` listed no devices, and the system volume query returned missing output volume/mute values. These command-line results did not characterize the GUI process's route.
- Correction: the operator confirms other Mac apps, including browser radio, produce audible sound. The prior empty list/missing-volume results came from command-line probes and do not establish that the Mac lacks output hardware. Rust Music's output route must be inspected from its actual GUI-launched child process.
- The operator also reported player flicker during playback. Source inspection traced it to the 500 ms snapshot poll using the interactive command path, which toggled transport busy state and blanked the visible status. Snapshot polling now shares the serialized package queue without changing visible status or transport lock, and ignores stale results around user commands. App build/signature/lifecycle checks and the offline gate passed; fresh-launch runtime flicker acceptance remains pending.
- The live output panel showed a completed 271,104-frame queue summary while the active snapshot had reached 5,263,872 frames at 1:59. The diagnostic reader was attaching a retained prior-session stderr record to active playback snapshots. Its previous `default yes` value was stale and could not establish that session's route. A per-session diagnostic reset and terminal-only summary presentation are now implemented; fresh-run evidence is recorded in [MAC07 pass results](evidence/MAC07_PASS_RESULTS.md).
- Exact commands and limitations are recorded in [MAC07 pass results](evidence/MAC07_PASS_RESULTS.md). Current pass states are in [mac07-pass-index.json](mac07-pass-index.json). RM-028 remains in progress for CLI route parity and device recovery. RM-029 is in progress; RM-030 remains planned pending broader acceptance.

## Functional layout contract

Four independent resizable native windows share one PlayerModel:

1. **Main Player:** title/artist/album, state, elapsed/duration from validated engine state, Play/Pause/Stop/Previous/Next, software volume, and visible failure feedback.
2. **Playlist Editor:** distinguish the live queue from named playlists; show current/next; support multi-track add, enqueue/dequeue, keyboard reorder, Play Next, clear, repeat, shuffle, and persistent playlist changes.
3. **Equalizer:** independent window with ten bands. Keep every control disabled and identify DSP as unavailable until a verified processor is connected; do not simulate processing.
4. **Media Library:** text-only track browser with search, title/artist/album sort, multi-selection, Play, Play Next, bulk enqueue, and Add to Playlist. Missing metadata falls back to filename.

The default arrangement tiles Player, Playlist, and Equalizer down the left and Library on the right when the visible display is at least 1,076×648 points. On smaller displays, Arrange Operator Windows uses a bounded cascade at each window's declared minimum size, with Player brought forward; the Window menu and WINDOWS menu provide access to obscured windows. The library minimum is 520×360 points so each window can fit on a 640×420-point display. On restart or display changes, saved frames are reduced and repositioned to fit the current visible area while preserving minimum sizes. Magnetic edge docking can be toggled in the Window menu and WINDOWS menu. Geometry tests cover the tiling threshold, default 1,440×900-point arrangement, bounded cascade at 900×600 and 640×420 points, restore-time fitting, and side/vertical snaps. Live drag-snap and multiple display-size UI acceptance remain open. The visual reference does not authorize artwork, visualizers, image assets, or new dependencies. Preserve text-only UX, the 128×64 / 1,024-byte Pip-Boy frame, local-only runtime, Rust Boy colors, and keyboard accessibility.

## Interaction contracts

- Folder selection triggers an explicit bounded scan and persistent catalog update. Never scan or play at startup, refresh or reconnect.
- Single click selects only; double click or Enter plays. Document whether Play Now replaces the queue or moves its cursor without destroying saved playlists.
- Context menu offers Play Now, Play Next, Add to Queue, Add to Playlist and File Info. Support Shift/Cmd multi-selection and stable batch ordering.
- Finder import must enforce path/symlink boundaries and never write source media.
- Queue reorder has keyboard Move Up/Down alternatives; queue persistence must not trigger playback on restart.
- Previous/Next starts the adjacent queued item. Natural EOF advances once, without double play or overlapping sinks.
- M3U/M3U8 import/export resolves relative paths safely and rejects traversal/out-of-root references. Playlist state remains distinct from queue state.
- Expose seeking only when sample-accurate seek is supported; otherwise progress is read-only.

## Sequential passes

| Pass | Owner | Scope | Required acceptance |
| --- | --- | --- | --- |
| MAC07-00 | `gate_verifier` + `host_integrator` | Pin source, same-file CLI/GUI reproduction, selected output route and sanitized diagnostics | Same-file CLI-versus-GUI outcome classified with a usable output route; commands and observations recorded |
| MAC07-01 | `host_integrator`; consult `audio_backend` | Diagnose and repair proven selection/queue/focus/IPC/native-output handoff defect | Same real FLAC is audible in GUI; regression fails before and passes after; no false Playing state |
| MAC07-02 | `host_integrator` | Typed validated playback snapshot and progress/error reset | Protocol edge cases tested; progress/state match real playback and failure |
| MAC07-03 | `host_integrator` | Compact player, navigator, sortable/searchable table, docked queue | Layout checked at compact and expanded sizes, including keyboard order and no dropped tracks |
| MAC07-04 | `host_integrator` | Double-click/Enter Play Now, batch queue, Play Next, transport and EOF | Deterministic non-silent tests; operator confirms audible transitions |
| MAC07-05 | `host_integrator` | Queue reorder, Finder import, named playlists, M3U8 and restart persistence | Path safety, persistence, Unicode and queue ordering tests; no autoplay |
| MAC07-06 | `gate_verifier` + `safety_reviewer` | A/B baseline/candidate, codecs, routes, queues, catalog sizes and accessibility | Raw observations, fixed SHAs, three functional repetitions and five UX pilots; distinguish acoustic from UI evidence |
| MAC07-07 | `index_curator` | Synchronize roadmap/evidence and run release checks | Exact evidence and remaining blockers recorded; no pass without observed acceptance |

Do not run a dependent implementation pass before its prerequisite is accepted. Read-only review can proceed independently. Preserve existing decoder behavior unless a regression is demonstrated. Before accepted Mac candidate, run the offline gate, macOS app build/signature checks, focused tests and roadmap validators; absent hardware remains blocked. MAC07-00 through MAC07-07 are not currently passed.

## User journey for final acceptance

Open app -> explicitly choose a FLAC folder -> browse tracks -> double-click a real FLAC and hear it -> multi-select five tracks -> reorder queue -> Next changes audible track -> Pause freezes progress -> Resume continues -> save playlist -> relaunch and restore library/playlist/queue without automatic playback.
