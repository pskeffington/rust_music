# AUI-01E — Large Library Operator Management

Date: 2026-11-01
Status: IMPLEMENTATION DIRECTIVE / NOT IMPLEMENTED
Parent: docs/AUI01D_OPERATOR_WINDOWS_ONLY.md
Priority: P1 for macOS desktop manager, portable design for Linux/Windows later.

## Unchanged constraints
Retain the four independent operator windows (Player, Playlist, EQ, Media Library), existing Rust Boy phosphor theme, existing audio backend, offline-only operation, zero artwork policy, and exact Rust Boy protocol. Large-library operations must never block the audio callback, UI event loop or shared focus lease. Do not change theme or introduce a dashboard.

## Media Library operations
- Explicit Add Folder / Rescan / Remove Location operations with permissions, cancellation and progress; never scan automatically on launch.
- Multiple library roots; distinguish unavailable volume or SMB location from deleted media; do not silently purge disconnected entries.
- Hierarchical sidebar (All Tracks, Artists, Albums, Genres when metadata exists, Playlists, Favorites, Recently Added, Locations, Missing/Unavailable). No invented tags.
- Dense sortable track table with track title, artist, album, track number, duration, codec, sample rate, bit depth, source availability. Column visibility, widths and sort state persisted locally.
- Search by artist, album, title and file name; filter tokens and selection retained while scrolling; keyboard shortcuts for search and enqueue.
- Multi-select, range-select, select all results, add to queue or named playlist, move/remove queue entries, rename playlists, delete playlist without deleting media. Destructive file operations are OUT OF SCOPE unless separately designed with confirmations.
- Track context menu: Play, Play Next, Enqueue, Add to Playlist, Show Local File, Inspect Format/Metadata; disable unsupported actions honestly.
- Duplicate candidates via canonicalized paths and optionally content hashes in deferred background operation; never deduplicate merely on title. Missing/broken links display actionable status.
- Reconcile catalog on explicit rescan using stable track IDs and deterministic sort keys; playlists and queue must preserve references or expose missing tracks with a repair action.
- Provide optional library statistics based on real indexed data (track count, albums, artists, total known duration, unavailable count).

## Scale/performance contract
- Target datasets: 0, 100, 2,000, 20,000, 100,000 and 250,000 synthetic entries, including Unicode long tags and missing metadata; tests should declare machine configuration and report raw values.
- Virtualize table rows, never construct one view per catalog item or synchronously render every result.
- Use bounded incremental paging and indexed query/filter results, stable sorting and cancellation/debounce with generation IDs to discard outdated query results.
- Scan on worker/background queue, report bounded progress and partial errors, provide pause/cancel/retry; throttle progress updates to avoid excessive SwiftUI recomposition.
- Use a memory-budgeted catalog cache with measurements; avoid full duplicate Swift-side and Rust-side catalogs if IPC can supply paged snapshots.
- 250k track performance is an acceptance stress target, not a claimed current supported capacity.
- Track filesystem failures, huge metadata, symlink loops, SMB disconnection, and directory permission errors without crash or silent data loss.
- Persist scan checkpoint and index atomically; recover from interruption or version upgrade without corrupting playlist/queue state.

## Playlist and queue at scale
- Support thousands of rows with virtualized rendering, persistent stable IDs, pagination and keyboard bulk actions.
- Playlist export/import is bounded; validate M3U8 paths/encoding against existing security policies and preserve missing entries explicitly.
- Sort library presentation without silently changing explicit playlist/queue playback order.
- Never autoplay on selection, scan, search, reconnect or app startup.
- Playback remains functional while library search/scan is active.

## A/B and acceptance
1. Full text search and sort responsiveness with 2k/20k/100k/250k catalogs; report measured p50/p95 and peak memory; no invented thresholds.
2. Scroll + keyboard navigation of large tables without visible UI hangs; select last item in an ordered catalog.
3. Bulk enqueue from filtered selection, cancel mid-scan, persist and recover playlists/queue after restart.
4. Disconnect mounted SMB/external volume, inspect Unavailable without deletion, reconnect and explicit reconciliation.
5. Validate duplicates, Unicode, corrupt tags, errors and stale IPC query responses.
6. Play/Pause/Stop real WAV/FLAC on operator Mac while background work executes; no audio underflow attributable to UI; preserve host focus and child lifecycle.
7. Verify VoiceOver labels/ordering and scalable density at large catalog sizes.
8. Run existing repository offline, macOS build/signature, literature and roadmap index validators. Record commands/results; ALL NOT RUN when this directive was written.

## Agent ownership
host_integrator: sole macOS UI writer, virtualized list + window state.
library_engineer (only if existing validated role): Rust catalog indexing, scanning, stable IDs, paging; otherwise assign existing qualified library owner.
audio_backend: read-only unless a measured audio issue requires scoped repair.
gate_verifier: read-only independent performance/usability/audio acceptance.
safety_reviewer: read-only local file/path, crash recovery, metadata and privacy review.
index_curator: serial updates to roadmap and acceptance ledger after observed evidence.

## Engineering pass order
1. Inventory current catalog, search, queue stores and IPC serialization; measure baseline.
2. Virtualized track grid and paged data source; preserve keyboard multi-selection.
3. Search/filter/sort/index + stale-query cancellation.
4. Explicit multi-root scanning with bounded progress, cancellation, missing-source lifecycle.
5. Bulk queue/playlist actions, stable IDs, recovery and duplicate reporting.
6. Large-scale benchmarks, accessibility, hardware playback, Mac A/B. Do not claim implementation from this document.
