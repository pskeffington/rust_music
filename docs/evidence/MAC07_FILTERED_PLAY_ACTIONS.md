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

## Local workspace repeat

The same catalog-index source was applied to the preserved workspace and
`sh scripts/build-macos-app.sh` exited **0** there as well. Strict app
signature verification, all included Swift harnesses, `python3 -m json.tool
docs/mac07-pass-index.json`, `python3 scripts/check_roadmap_index.py`, and
`git diff --check` passed. This was a repeat in the dirty local worktree, not
a separate commit checkout.

Repeat catalog measurements were:

| Tracks | Title-index build | Cached sort p95 | Narrow-search p95 | Broad-search p95 |
| ---: | ---: | ---: | ---: | ---: |
| 2,000 | 0.252 ms | 0.044 ms | 0.597 ms | 0.751 ms |
| 20,000 | 2.813 ms | 0.444 ms | 5.782 ms | 7.702 ms |
| 100,000 | 20.521 ms | 8.948 ms | 28.750 ms | 44.912 ms |
| 250,000 | 62.475 ms | 25.288 ms | 73.103 ms | 123.791 ms |

Catalog-harness peak RSS was **227,442,688 bytes** in this run. The difference
from the prior exact-candidate run (204,914,688 bytes) reflects separate
process runs and peak-RSS measurement; both are recorded rather than
presented as interchangeable catalog-only memory values.

## Bounded Media Library pages — PR #9 candidate

Based on PR head `e54b85e28cd0dffaaf04089f25160bc89c9bfa3e`, the Media Library
now publishes at most 200 matching rows at a time. Search counts the full result
set and keeps the chosen order; PREV PAGE / NEXT PAGE and an accessible
page/range value report navigation state. Search or sort changes reset to the
first page. Selection edits preserve selected IDs on other pages. Play Selected
and Play Next require the active track to be selected and visible on the
current page; bulk Enqueue still includes selected IDs from other pages. Search
and page construction use the existing serial background queue. IPC's
20,000-track limit and wire format are unchanged.

In this isolated PR-head checkout, `sh scripts/build-macos-app.sh` exited **0**.
Page tests cover title/artist order, filtered middle pages, out-of-range
offsets, cache revision invalidation, the 200-row maximum, and selection across
two pages. All bundled UI harnesses passed, including playlist selection,
window geometry, playback snapshots, catalog bounds, process lifecycle and
shutdown. Strict app signature validation reported valid on disk and a
satisfied designated requirement. The existing 17 unused AudioQueue
diagnostic warnings remain.

The synthetic broad-query page p95 at 2,000, 20,000, 100,000 and 250,000 rows
was **0.557 ms, 5.212 ms, 43.554 ms and 123.240 ms** respectively. At 250,000,
the catalog harness process peak RSS was **204,980,224 bytes**. Those larger
profiles exercise only the in-memory algorithm: scanner and IPC imports remain
limited to 20,000 tracks. These figures are not full-app memory, real-import
latency, or playback-under-load results.

The desktop availability check reports macOS locked and requires manual
unlock. The paging controls, compact-window fit, cross-page selection in the
live app, and VoiceOver page-status speech therefore remain unverified.

## Pagination status and boundary behavior

The page counter, visible item range, and previous/next enabled state now use
one `CatalogPageNavigation` value shared by the Library controls and the
underlying page-move action. Regression cases cover an empty catalog, a middle
page, a one-item final page, and an `Int.max` requested offset. This prevents
the on-screen range and page buttons from independently calculating different
boundaries.

The follow-up `sh scripts/build-macos-app.sh` exited **0** on the PR branch
after that change. All included macOS UI harnesses and strict signature checks
passed; the roadmap validator and `git diff --check` also passed. In this
repeat synthetic run the 250,000-row broad-page p95 was **107.707 ms** and
process peak RSS was **204,963,840 bytes**. Imported libraries remain capped
at 20,000 tracks. Live page control and VoiceOver behavior remain unverified
because macOS is locked.

## Persisted Library row density

The Library now has COMPACT and STANDARD row density choices, persisted through
`@AppStorage`. STANDARD is the default and retains the existing title/metadata
font sizes, text spacing, row padding, colors, and monospaced styles. COMPACT
reduces title and metadata sizes by one point and reduces text spacing and
vertical padding, without changing the color palette or typeface. The density
picker has an accessibility label and explains the compact option. Track IDs
and selection state do not depend on density.

The candidate `sh scripts/build-macos-app.sh` exited **0**. The catalog harness
verified STANDARD values remain 12/10-point labels with spacing/padding 2, and
that COMPACT reduces each measurement. Catalog, page navigation, playlist,
window geometry, playback-state, catalog-bound, and process lifecycle/shutdown
harnesses passed; strict signature verification passed. Roadmap/index
validation and `git diff --check` passed. The run measured synthetic broad-page
p95 **110.603 ms** and peak harness RSS **204,996,608 bytes** at 250,000 rows.
These are algorithm-harness numbers, not full-app memory or imported-catalog
results; the standalone import cap remains 20,000 tracks.

Live confirmation of the density picker, persistence across relaunch, compact
row count, minimum-window fit, and VoiceOver navigation is pending because the
Mac is locked. No visual or runtime acceptance is claimed for this change.

## Responsive Library toolbar

The Library toolbar now uses `ViewThatFits`: Search, Sort, row density, and
the result count stay on the original single row when that layout fits; at
narrower widths Search gets a full row and Sort, row density, and count move to
a second row. Both arrangements reuse the same fields, bindings, accessibility
labels, and identifiers. The existing control styles and colors are unchanged.

`sh scripts/build-macos-app.sh` exited **0** on the PR candidate. All bundled
macOS harnesses passed and strict signature verification succeeded. The JSON
and roadmap validators plus `git diff --check` passed. The catalog harness's
250,000-row broad-page p95 was **108.290 ms**, and its process peak RSS was
**204,980,224 bytes**; these are synthetic catalog measurements and do not
measure SwiftUI layout or full-app memory. Real imports remain capped at 20,000
tracks.

The Mac remains locked. The breakpoint, focus behavior across the two toolbar
arrangements, compact-window fit, and VoiceOver traversal have not been
verified live.

## Field-scoped Library search

On PR base `77d755c129e8f2e287db9e698bfd2e3d0c34f1e2`, the Library search now
supports ALL, TITLE, ARTIST, and ALBUM scopes. ALL remains the default; the
choice is persisted. Scoped queries reuse the existing folded metadata keys,
remain on the catalog-view queue, retain bounded page materialization, and
apply to the selected-track visibility guard. The adaptive toolbar places the
scope control with the search field when the full row fits and in the second
row at narrower widths. Rust Boy colors, control styles, and row typography
are unchanged. No Cargo manifest or dependency changed.

Regression coverage includes case and canonical Unicode matching, excluding
matches found only in another field, result ordering and pagination, cache
scope forwarding, blank queries, and selected-track actionability under the
active scope. `sh scripts/build-macos-app.sh` exited **0** on this candidate
working tree; the catalog, page navigation, playlist selection, window
geometry, playback snapshot, catalog-bound, process lifecycle, and shutdown
harnesses passed. Strict app signature verification passed. The synthetic
250,000-row all-fields broad-page p95 was **124.757 ms** and catalog-harness
peak RSS was **204,996,608 bytes**. This run did not measure scoped-query
latency separately, full-app memory, SwiftUI layout, or real import performance;
the import protocol remains capped at 20,000 tracks.

`sh scripts/offline-gate.sh` then exited **0**. Its dependency audit reported
17 workspace crates, 32 path-only internal edges, zero registry/Git crates,
zero third-party system links, and three approved OS framework links. The
workspace offline checks and tests completed successfully.

The active app session was left untouched. The scope menu's live filtering and
persistence, the adaptive toolbar breakpoint and focus, and VoiceOver remain
unverified. This is build and harness evidence only, not UI acceptance.

## Artist and Album browsing — isolated PR #9 candidate

The Media Library now offers TRACKS, ARTISTS, and ALBUMS browse modes. Artist
and Album views deduplicate names using the existing Unicode-folded metadata
normalizer, sort groups by normalized name, count tracks, and place blank tags
under Unknown Artist / Unknown Album. Search filters group names; results use
the same bounded 200-row pages and background catalog-view queue. Selecting a
group and choosing BROWSE SELECTED ARTIST/ALBUM opens the track view scoped to
that name. Browse mode persists with TRACKS as the default. Track sort commands
are disabled in group views. Rust Boy styling and the no-artwork policy are
unchanged.

`sh scripts/build-macos-app.sh` exited **0** for this candidate. The UI
harnesses passed, including browse grouping, case/Unicode matching, unknown-tag
fallback, group counts, page bounds, and catalog-revision cache replacement.
Strict app signature validation passed. `sh scripts/offline-gate.sh` exited
**0**: 17 local crates, 32 path-only internal edges, no registry/Git crates,
and no third-party system links. The gate reports 17 existing unused native
AudioQueue diagnostic warnings.

The synthetic catalog harness created one unique artist per track to exercise
high-cardinality browse indexes. Group-index build plus first bounded page
took 4.148 ms / 42.414 ms / 245.381 ms / 630.646 ms at 2,000 / 20,000 /
100,000 / 250,000 tracks. Narrow artist-name query p95 was 1.787 ms / 19.062
ms / 93.378 ms / 225.323 ms. Catalog-harness process peak RSS at 250,000 was
273,285,120 bytes; searchable metadata payload was 18.5 MB. These measurements
are synthetic algorithm-harness results, not full-app memory, interactive UI
latency, real import timing, or playback-under-load evidence. The current
scanner/IPC catalog limit remains 20,000 tracks.

The active desktop session was not changed. Live artist/album browsing,
selection-to-track behavior, preference persistence, toolbar/layout fit,
VoiceOver, 250,000-track import, and playback during scanning/search remain
unverified.

## Same-title album disambiguation — PR #9 candidate

Album groups are now keyed by both normalized artist and album metadata. Rows
show the artist under the album name, and searching album groups matches both
fields. Choosing an Artist or Album opens Tracks with explicit folded artist
and album identity filters; search then narrows within that selection. The
Library displays the active filters and offers CLEAR BROWSE FILTERS. Blank
artist or album tags retain Unknown Artist / Unknown Album labels and map back
to tracks with empty metadata. This prevents same-titled releases by different
artists from merging or opening as one combined track set.

Regression coverage verifies two same-titled albums remain separate, case
variants for the same artist/title merge, album-group search matches artist
details, opening the selected album filters its track page to the matching
artist and album, and blank artist/album groups map to blank-tag tracks.
`sh scripts/build-macos-app.sh`
exited **0**; the catalog, playlist, operator-window geometry, playback
snapshot, catalog-bound, process lifecycle, and shutdown harnesses passed, and
strict bundle signature validation succeeded. `sh scripts/offline-gate.sh`
exited **0** with 17 local crates, 32 path-only internal edges, zero
registry/Git crates, zero third-party system links, and three approved OS
framework links. The build emitted 17 existing unused AudioQueue diagnostic
warnings.

Artist and album group indexes cache normalized search keys; the album track
constraint reuses per-track folded artist keys. In the final 250,000-track
synthetic unique-artist run, artist-group index build plus first page measured
**624.376 ms**, artist-query p95 **230.627 ms**, and process peak RSS
**334,462,976 bytes**. The previous uncached artist query p95 was **624.586
ms** at **263,077,888 bytes** peak RSS. These separate harness runs suggest
faster repeated group matching at the cost of cached-key memory; they are not
a controlled A/B or full-app memory/interactive UI measurements.
Real imports remain limited to 20,000 tracks. The app currently running on the
desktop was left untouched, so live same-title album navigation, filter
clearing, persistence, VoiceOver, layout fit, and playback under search remain
unverified.

## Live Artist and Album browsing — PR #9 candidate

The macOS bundle built from PR source `9b018eb` was opened from the isolated
checkout while the pre-existing Rust Music process remained open. The
candidate started at Player / Ready / No Track with no library and no playback
snapshot. An explicitly selected temporary catalog contained three generated
metadata-only FLACs: two tracks named under the same album title by Artist One
and Artist Two, and one with blank artist/album tags. No audio frames were
included.

In Media Library, the BROWSE menu exposed TRACKS, ARTISTS, and ALBUMS. ARTISTS
showed Artist One, Artist Two, and Unknown Artist; choosing Unknown Artist
opened exactly the blank-tag track. CLEAR BROWSE FILTERS restored all three
tracks. ALBUMS showed separate `Twin Album` rows for Artist One and Artist Two,
plus Unknown Album / Unknown Artist. Choosing Artist Two's album opened only
`Second Twin`. Searching `First` within that group showed zero tracks;
searching `Second` showed the one matching track and retained the active artist
and album filters. The Player remained stopped with no output snapshot through
the check. The candidate was quit; CUA inventory confirmed the original
Rust Music process was still running. The generated fixture directory was
removed.

CUA reported that the candidate bundle changed during one search interaction.
I re-read the full accessibility tree before continuing; the same candidate
window still exposed the browse controls and retained the expected fixture
state. This is live accessibility-tree interaction evidence from the launched
candidate, not VoiceOver speech, playback, or a bundle-digest-bound UI test.
VoiceOver, persistence across relaunch, minimum-size layout, and playback under
search remain unverified.

## Artist and Album row accessibility label — PR #9 candidate

After adding an explicit single-element accessibility boundary to each artist
or album group row, the rebuilt macOS candidate was launched from the isolated
PR checkout. An explicitly loaded temporary catalog held two generated
metadata-only FLAC entries with the same album title and distinct artists; no
audio frames were included. In ALBUMS mode, the live accessibility tree exposed
exactly one row per group (`Twin, Artist A, 1 tracks` and
`Twin, Artist B, 1 tracks`) without repeating child text. Playback remained
stopped. The candidate was quit, TRACKS mode was restored before exit, and the
temporary fixture directory was removed. The original Rust Music process was
left running.

This is live accessibility-tree evidence only. VoiceOver speech and rotor
behavior, persistence after relaunch, and playback remain unverified.
