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

`sh scripts/build-macos-app.sh` exited **0** for source `5dceb78`; strict local
bundle signature validation and the catalog, playlist, geometry, playback
snapshot, catalog-bounds, and process-lifecycle harnesses passed. Synthetic
catalog results were: 2,000 tracks, search p95 0.617 ms and sort p95 4.280 ms;
20,000, 5.852 ms and 40.332 ms; 100,000, 29.280 ms and 244.908 ms; 250,000,
72.440 ms and 706.366 ms. Catalog-harness peak RSS was 240,451,584 bytes; these
measurements do not represent full-app memory or interactive UI latency.

The rebuilt candidate loaded two generated metadata-only FLAC entries with no
audio frames. A canonically decomposed accent query returned the matching
composed-accent row while the prior selection remained hidden. The outside-
search notice appeared; Play Selected and Play Next were disabled while
Enqueue Selected remained available. Selecting the visible result removed the
notice and re-enabled Play actions. Playback and queue operations were not
invoked. This is a focused live accessibility-tree check, not VoiceOver speech
or audio-output acceptance.

## Four-window focus routing

In the rebuilt app session at Player / Ready / No Track with no library loaded,
the native Window menu's Show Player, Show Playlist Editor, Show Media Library,
and Show Equalizer actions each brought the named window to the front according
to the resulting macOS accessibility-tree root. The in-window WINDOWS menu
also raised Equalizer from Player. Equalizer continued to expose all ten
disabled bands and the unavailable-DSP explanation. No library scan or
playback occurred. After explicitly returning to Player, a follow-up AX check
2.5 seconds later still showed Player / Ready / No Track as the active window.

This verifies window target and focus routing for both menu surfaces. No frame
dimensions were captured, and no claim is made here about Arrange geometry,
minimum-frame behavior, or simultaneous window visibility.

## Library selection accessibility actions

On 2026-11-01, a rebuilt app explicitly loaded one generated metadata-only FLAC fixture with no audio frames. The Library list exposed `Enqueue Selected Tracks` and `Play Next Selected Track` as macOS accessibility secondary actions. Invoking Enqueue changed the queue from zero to one entry; invoking Play Next changed it from one to two. Clearing the temporary queue returned it to zero. Player remained stopped with no output snapshot throughout. This verifies action exposure and queue effects on that candidate; it does not verify VoiceOver speech or rotor navigation.

A follow-up change makes Enqueue available only with a nonempty selection while commands are idle. Play Next additionally requires the transport target to be part of the Library selection and visible under the current search. The latest `sh scripts/build-macos-app.sh` exited 0. The final live no-selection check was interrupted when the desktop session reported that the app bundle had changed during user interaction; dynamic action removal for the latest build remains unverified. The fixture remains loaded in the stopped app at the last observation and is outside Git.


## Dated correction and selected-row control guard

On 2026-11-01, the desktop app was still running after the preceding entry;
this was confirmed in the live app inventory and accessibility tree. It was
Player / Ready with the generated metadata-only fixture selected, playback
stopped, and no output snapshot. Opening Media Library showed the selected
row and both named list actions. Command-clicking that row removed its
selection and both named list actions, and disabled Enqueue Selected. The
separate Play Selected and Play Next buttons remained enabled, exposing a
selection-state mismatch. Player was returned to the front without starting
playback or changing the queue.

The candidate now uses one bounded selected-row predicate for the named
Library actions, Play Selected, Play Next, and their Library-menu commands. It
requires the active track to be selected in the Library and visible under the
current search. Regression cases cover a matching selected row, no row
selection, and a selected row hidden by search. `sh scripts/build-macos-app.sh`
exited 0 after these changes; catalog, playlist, geometry, playback-state,
catalog-bounds, and lifecycle harnesses passed and the app signature verified.
At 250,000 synthetic tracks the final catalog harness reported search p95
102.611 ms, full-sort p95 1,086.662 ms, and peak process RSS 220,856,320 bytes;
these are harness results, not full-app memory or interactive UI latency.

The user-visible button and menu disabled states from this final build have
not been live-verified because the app that was already running still has the
prior binary loaded. That process remains at Player / Ready with the test
fixture, playback stopped, and no output snapshot. No restart was forced, and
VoiceOver speech and rotor navigation remain open. This correction supersedes
the preceding sentence that said the app was stopped; it does not change the
historical result for the earlier build.


## Relaunch verification attempt

A fresh-process check of the final selected-row button/menu state was attempted
on 2026-11-01. CUA reported that macOS was locked and automatic unlock failed,
then instructed the operator to unlock the Mac manually. I stopped UI
interaction; no unlock bypass was attempted. The rebuilt app's final disabled
states remain unverified, and the app's post-lock process/window state is
unknown. Before the attempt, the last observed UI was Player / Ready with one
metadata-only fixture, stopped playback, and no output snapshot.


## Retained-catalog scan status clarity

The source keeps the previous `libraryRoot` and catalog active while a new
folder scan runs; queue and playlist state commands continue to target that
retained catalog until the scan completes or is canceled. The Library showed
that prior results remained active, but did not say which catalog queue and
playlist edits affected. The scan banner now states that directly and exposes
the named accessibility value `Library scan status` with identifier
`library-scan-status`.

Focused regression checks confirm that the message appears only while scanning
with a retained catalog. The subsequent `sh scripts/build-macos-app.sh` exited
0; all macOS UI harnesses passed and strict bundle signature verification
succeeded. Live text/AX verification and scan cancel/failure interaction remain
unverified because macOS is locked.

## In-flight state command across a successful scan

A source review found that a queue or playlist command started while the
retained catalog was active could finish after a successful scan switched the
library root. Its response could overwrite the new catalog's visible
queue/playlist state, while the attempted refresh could be dropped because
another state command was still busy.

The model now advances a library-state revision on every successful catalog
switch, discards state-command responses from older revisions, and defers the
active-root refresh until state and transport operations are idle. Unit
coverage checks revision invalidation and deferred-refresh readiness while
state or transport is busy, then confirms the one-shot idle drain cannot be
consumed twice. `sh scripts/build-macos-app.sh` exited 0 with all
macOS UI harnesses and strict bundle signature validation passing. At 250,000
synthetic tracks the catalog harness reported search p95 72.548 ms, full-sort
p95 690.596 ms, and peak process RSS 225,230,848 bytes.

Independent gate review passed the revision/deferred-refresh source flow and
focused playback/UI-state harness on Darwin arm64 / Apple Swift 6.3.3 (exit 0).
The harness does not simulate a real two-root scan with an in-flight queue or
playlist command. Live verification of successful root switch, cancellation,
failure, and the new VoiceOver value remains pending because macOS is locked.


## Exact PR-head app and offline-gate confirmation

On 2026-11-01, a clean isolated checkout of PR #9 at exact source
`4a8aa619de462f78c8817c34169c3289ba773744` ran
`sh scripts/build-macos-app.sh` with exit code **0**. The native Rust release
build, Swift catalog search/scaling, playlist selection, operator geometry,
playback snapshot, catalog bounds, and process lifecycle/shutdown harnesses
passed. Strict local app signature validation reported valid on disk and a
satisfied designated requirement. The synthetic 250,000-track catalog
harness measured search p95 **72.084 ms**, full-sort p95 **673.876 ms**, and
peak process RSS **240,517,120 bytes**. These are harness measurements, not
full-app memory, interactive UI latency, or playback-under-load evidence.

The same clean checkout ran `sh scripts/offline-gate.sh` with exit code **0**:
17 local crates, 32 path-only internal edges, zero registry/Git crates, and
zero third-party system links. Formatting, warnings-denied Clippy, workspace
tests, release build, and macOS feature tests completed; the native macOS
audio crate emitted 17 unused diagnostic-field warnings. The Rust gate and UI
build do not establish real playback or hardware acceptance.

This confirms the committed PR source builds independently of the dirty local
worktree. Live verification remains pending because the desktop is locked:
the final selected-row disabled states, scan-status accessibility value, a
real two-root scan with an in-flight state command, VoiceOver, and playback
remain unverified in this exact checkout.

## Cached catalog order indexes

The Library view now keeps a catalog-revision-scoped search index on its
serial background queue. Title, artist, and album order indexes are built
lazily and reused across searches. Selective search results (up to 1,024 rows)
are sorted directly; broader results use a bounded bitmap when stable track
IDs map to catalog offsets, preserving the cached order without a full sort.
The index is discarded when the active catalog revision changes. No runtime
dependency, IPC change, or artwork path was added.

`sh scripts/build-macos-app.sh` exited **0** on the candidate based on
`d2a87766aa695a6f2637a111abfb3ae24198e658`; catalog, playlist, window
geometry, playback snapshot, catalog-bound, and process lifecycle harnesses
passed. The app signature verified on disk and met its designated requirement.
`sh scripts/offline-gate.sh` also exited **0**: 17 local crates, 32 path-only
edges, no registry/Git crates or third-party system links; format, Clippy,
workspace tests, release build, and macOS feature checks passed. Existing
unused AudioQueue diagnostic warnings remain.

The catalog harness used 21 warm samples for cached-sort and narrow-search
p50/p95, and five samples for broad-search p50/p95. Results by catalog size:

| Tracks | Title-index build | Cached sort p95 | Narrow-search p95 | Broad-search p95 |
| ---: | ---: | ---: | ---: | ---: |
| 2,000 | 0.265 ms | 0.052 ms | 0.593 ms | 0.736 ms |
| 20,000 | 2.865 ms | 0.442 ms | 5.865 ms | 7.850 ms |
| 100,000 | 22.650 ms | 9.983 ms | 29.110 ms | 44.401 ms |
| 250,000 | 66.656 ms | 25.769 ms | 72.424 ms | 116.084 ms |

The catalog harness process peak RSS was **204,914,688 bytes**. In the
250,000-track run, the same harness's linear-filter reference measured narrow
search p95 **90.884 ms**; this is one synthetic run, not an interactive UI
latency guarantee. The previous exact-head full-sort p95 was 673.876 ms; the
new cached sort is a different operation (warm full-row materialization from
cached order), so these timings should not be treated as a strict A/B trial.

Live search responsiveness, real 250,000-track import, memory on a tagged
library, and concurrent playback under scanning/search remain unverified.
