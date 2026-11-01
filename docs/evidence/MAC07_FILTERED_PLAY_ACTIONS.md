# MAC-07 filtered playback action check

Date: 2026-11-01  
Source: `76bbdd1` (the source file in this commit matches the locally built candidate)

## Build and scope

- `sh scripts/build-macos-app.sh` exited 0 after the Player and Playback-menu guard change. Strict local app signature validation passed.
- Catalog search/scaling, playlist selection, operator-window geometry, playback snapshot, catalog bounds, lifecycle, and process shutdown harnesses passed.
- The compact Media Library screenshot measured 736×462 pixels. This is screenshot-pixel size, not an AppKit point measurement.
- Current synthetic catalog harness results: 2,000 tracks, search p95 0.636 ms and sort p95 4.472 ms; 20,000 tracks, 6.158 ms and 44.609 ms; 100,000 tracks, 31.396 ms and 283.347 ms; 250,000 tracks, 91.106 ms and 1,535.265 ms. Peak RSS at 250,000 tracks was 240,467,968 bytes. These are catalog-harness measurements, not full-app memory or playback-under-load results.

## Live interaction

Using a user-provided local six-track library, search was narrowed to one visible row while a different row remained selected. The interface reported that the current selection was outside search. Play Selected and Play Next were disabled in both the view and Library menu; Enqueue Selected remained enabled. Selecting the visible result removed the notice and enabled Play Selected and Play Next. Clearing the query retained the visible selected row.

No track was played and no queue action was invoked during this check.

## Player transport and playback menu follow-up

The rebuilt app was explicitly loaded with a generated three-entry metadata-only FLAC fixture containing no audio frames. The first row remained selected while search was narrowed to a different row. The Media Library showed one result and the outside-search notice; Play Selected and Play Next were disabled, while Enqueue Selected remained enabled. The Main Player's Play button and Playback menu's Play / Pause command were also disabled. No transport, queue action, or playback was started. The temporary fixture was removed after the check, and the app was relaunched to Player / Ready / No Track.

## Remaining MAC-07 checks

Minimum-frame behavior, populated-list accessibility, VoiceOver speech and rotor navigation, drag docking, multiple-display and saved-frame restart behavior, scan cancellation/failure states, output-route parity, device recovery, and playback under library load remain open.
