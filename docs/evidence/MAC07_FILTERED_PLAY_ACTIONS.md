# MAC-07 filtered playback action check

Date: 2026-11-01  
Source: `4deeab1` (the source files in this commit match the locally built candidate)

## Build and scope

- `sh scripts/build-macos-app.sh` exited 0.
- Strict local app signature validation and catalog search/scaling, playlist selection, operator-window geometry, playback snapshot, catalog bounds, lifecycle, and process shutdown harnesses passed.
- The compact Media Library screenshot measured 736×462 pixels. This is screenshot-pixel size, not an AppKit point measurement.
- Synthetic catalog harness results: 2,000 tracks, search p95 0.935 ms and sort p95 5.875 ms; 20,000 tracks, 8.294 ms and 58.009 ms; 100,000 tracks, 38.763 ms and 387.834 ms; 250,000 tracks, 96.104 ms and 1,066.429 ms. Peak RSS at 250,000 tracks was 240,500,736 bytes. These are catalog-harness measurements, not full-app memory or playback-under-load results.

## Live interaction

Using a user-provided local six-track library, search was narrowed to one visible row while a different row remained selected. The interface reported that the current selection was outside search. Play Selected and Play Next were disabled in both the view and Library menu; Enqueue Selected remained enabled. Selecting the visible result removed the notice and enabled Play Selected and Play Next. Clearing the query retained the visible selected row.

No track was played and no queue action was invoked during this check.

## Remaining MAC-07 checks

Minimum-frame behavior, populated-list accessibility, VoiceOver speech and rotor navigation, drag docking, multiple-display and saved-frame restart behavior, scan cancellation/failure states, output-route parity, device recovery, and playback under library load remain open.
