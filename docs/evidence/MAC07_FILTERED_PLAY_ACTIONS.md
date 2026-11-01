# MAC-07 filtered playback action check

Date: 2026-11-01  
Source: `5dceb78` (the committed query-matching helper and regression test match the locally built candidate)

## Build and scope

- `sh scripts/build-macos-app.sh` exited 0 after the Player and Playback-menu guard and named-playlist context action changes. Strict local app signature validation passed.
- Catalog search/scaling, playlist selection, operator-window geometry, playback snapshot, catalog bounds, lifecycle, and process shutdown harnesses passed.
- The compact Media Library screenshot measured 736×462 pixels. This is screenshot-pixel size, not an AppKit point measurement.
- Current synthetic catalog harness results: 2,000 tracks, search p95 1.540 ms and sort p95 10.970 ms; 20,000 tracks, 13.325 ms and 93.048 ms; 100,000 tracks, 77.562 ms and 560.366 ms; 250,000 tracks, 187.415 ms and 1,427.183 ms. Peak RSS at 250,000 tracks was 240,467,968 bytes. These are catalog-harness measurements, not full-app memory or playback-under-load results.

## Live interaction

Using a user-provided local six-track library, search was narrowed to one visible row while a different row remained selected. The interface reported that the current selection was outside search. Play Selected and Play Next were disabled in both the view and Library menu; Enqueue Selected remained enabled. Selecting the visible result removed the notice and enabled Play Selected and Play Next. Clearing the query retained the visible selected row.

No track was played and no queue action was invoked during this check.

## Player transport and playback menu follow-up

The rebuilt app was explicitly loaded with a generated three-entry metadata-only FLAC fixture containing no audio frames. The first row remained selected while search was narrowed to a different row. The Media Library showed one result and the outside-search notice; Play Selected and Play Next were disabled, while Enqueue Selected remained enabled. The Main Player's Play button and Playback menu's Play / Pause command were also disabled. No transport, queue action, or playback was started. The temporary fixture was removed after the check, and the app was relaunched to Player / Ready / No Track.

## Compact library, keyboard, and scrolling check

On 2026-11-01, the rebuilt app loaded 40 generated metadata-only FLAC entries with no audio frames. Native Window > Move & Resize > Top Left yielded a 736×462-pixel screenshot. The search field, sort control, track count, library list, and Play Selected / Enqueue Selected / Play Next actions remained visible. This is compact-window evidence; screenshot pixels are not AppKit points and do not prove the declared 520×360-point minimum.

With default macOS keyboard navigation, Tab moved focus from the window to Search, then to the track list. Shift+Down selected two adjacent rows and the enqueue count changed to two. Scrolling the library to the bottom set the list scrollbar to 1.0 and reached the final entry. No playback or queue operation occurred. The temporary fixture was deleted and the app was relaunched to Player / Ready / No Track.

## Direct add to named playlist

The rebuilt app loaded three generated metadata-only FLAC entries with no audio frames. Command-N opened Playlist Editor; a temporary named playlist was created. In Media Library, the first row remained selected while the context menu was opened on the second row. The menu exposed Enqueue Track, Play Next, and Add to Playlist with the named destination. Choosing it produced a one-entry playlist containing the clicked second row. The queue remained empty; no playback occurred. The generated catalog and playlist were removed with the temporary folder, and the app was relaunched to Player / Ready / No Track.

## Remaining MAC-07 checks

The declared 520×360-point minimum-frame behavior, VoiceOver speech and rotor navigation, drag docking, multiple-display and saved-frame restart behavior, scan cancellation/failure states, output-route parity, device recovery, and playback under library load remain open.

## Rebuilt candidate confirmation

On 2026-11-01, `sh scripts/build-macos-app.sh` rebuilt the current worktree and
its local macOS UI harnesses passed. A generated 40-entry metadata-only FLAC
catalog (no audio frames) was explicitly loaded. With the first row selected,
searching for the last row left the first row selected but hidden and showed
`CURRENT SELECTION IS OUTSIDE SEARCH`. Play Selected and Play Next were
disabled; Enqueue Selected remained enabled. The Library menu likewise
disabled Play Selected Track and Play Next Selected Track while leaving
Enqueue Selected Tracks enabled. Selecting the visible result removed the
notice and re-enabled Play actions. Clearing search restored all 40 rows and
retained the selected last row. No playback or queue action occurred.

This confirms the filtered-selection guard and selection retention through the
macOS accessibility tree for this rebuilt worktree and synthetic catalog. It
does not verify VoiceOver speech, audio output, or behavior at the declared
minimum window frame.

## Canonical Unicode query and filtered Play guard

`CatalogSearch.visibleTracks` and the selected-track Play guard now share one
bounded, whitespace-trimmed, Unicode-folded query representation. The catalog
search harness adds a regression case with a decomposed accent in the query.

The rebuilt candidate loaded two generated metadata-only FLAC entries with no
audio frames. A canonically decomposed accent query returned the matching
composed-accent row while the prior selection remained hidden. The outside-
search notice appeared; Play Selected and Play Next were disabled while
Enqueue Selected remained available. Selecting the visible result removed the
notice and re-enabled Play actions. Playback and queue operations were not
invoked. This is a focused live accessibility-tree check, not VoiceOver speech
or audio-output acceptance.
