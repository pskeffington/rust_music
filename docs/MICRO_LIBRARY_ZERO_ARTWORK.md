# Micro library policy: zero artwork

## Active task directives

The [agent_skills-first production continuation](CODEX_AGENT_SKILLS_FIRST_PRODUCTION.md)
and [library-load/autoplay/EQ gates](CODEX_WINAMP_LIBRARY_AUTOPLAY_EQ.md)
define current sequencing and acceptance criteria. Their requirements are
plans, not evidence of implementation. Reconcile older blanket autoplay
prohibitions as prohibiting **unrequested** playback: optional autoplay is
allowed only after explicit user library load, persistent opt-in, a successful
bounded scan, and host audio-focus grant. It defaults off and never runs on
startup, silent refresh, cancellation, or device reconnect.

## Contract

The library catalog, package-process view, and 128×64 panel carry no album
art, embedded images, thumbnails, image URLs, or artwork cache. Catalog entries
expose only a filename fallback, bounded allowlisted text tags, container,
file size, and playback state. Artwork is never decoded for display or copied
into the 1024-byte monochrome framebuffer.

This is a deliberate memory and privacy boundary for small Rust_Boy targets;
it is not a promise that arbitrary audio files contain no artwork. A scan may
encounter image metadata in an input file, but the library must skip it and
retain no image bytes or reference to an artwork payload.

## Implementation behavior

- `Track` contains a path relative to the explicitly selected library root, a
  filename title, optional `TrackTags`, container, and size. `TrackTags`
  allowlists only `TITLE`, `ARTIST`, and `ALBUM` for FLAC, and `NAME` and
  `AUTH` for AIFF/AIFF-C.
- The FLAC metadata parser enforces at most 128 metadata blocks, 16 MiB per
  block, and 64 MiB aggregate metadata. It parses STREAMINFO and bounded seek
  points; other blocks, including PICTURE, are skipped without retaining
  their payload.
- The library's Vorbis-comment scanner examines at most 4096 entries per file.
  It reads only a 64-byte key prefix before deciding whether a field is an
  allowlisted short text value. Oversized and unknown values are skipped by
  seeking over the remainder. The retained text value is at most 512 bytes and
  its display form at most 120 Unicode scalar values.
- AIFF/AIFF-C text parsing accepts only `NAME` and `AUTH`, bounds chunk count
  and metadata size, and seeks past unknown chunks. CAF/M4A library entries do
  not expose artwork or container metadata in `TrackTags`.
- The IPC `View` contains bounded display fields and playback state only; it
  has no artwork field. The panel framebuffer remains exactly 1024 bytes.

## Text-first library roadmap

The standalone app and Rust Boy MUSIC slot are to become a local, user-directed
media manager with browsing by artist/album/track, text search, sorting,
playlists, a persistent queue, repeat/shuffle, transport controls, and compact
metadata. The existing scanner remains explicit and bounded: catalog discovery
is not decoder admission, and scanning never autoplays or acquires audio focus.

### Forbidden subsystems

- No artwork, image UI, placeholders, thumbnails, galleries, image widgets,
  visualizers, animated skins, image packages, image fetching, or image caches.
- Never extract, copy, decode, retain, export, or cache FLAC PICTURE, ID3 APIC,
  MP4 `covr`, or other image payloads. Skip bounded length-delimited metadata
  without allocating the payload.
- No network metadata, telemetry, downloads, third-party runtime Cargo
  dependencies, or general-purpose media framework.
- Never modify source audio. Playlist and queue operations only update their
  own bounded text records.

### Data and UI boundaries

| Boundary | Responsibility |
| --- | --- |
| Catalog | Explicit approved roots, bounded scan/refresh, stable root-scoped track IDs, missing-media state |
| Metadata | Locally parsed allowlisted text; filename fallback; bounded fields; image-bearing tags skipped |
| Search | Case-folded text-only filtering, stable sorting, bounded pagination |
| Playlist | Bounded versioned list, safe M3U/M3U8 import/export, audio files untouched |
| Queue | Enqueue/dequeue/reorder, next/previous, repeat-one/all, deterministic seeded shuffle; do not invent pause/seek support |
| Persistence | Compact versioned local state, atomic replacement, malformed-state recovery, no media payloads |
| Presentation | Text-first desktop views and existing fixed 1024-byte monochrome panel |

Revalidate catalog paths before playback, refuse symlinks and traversal, bound
roots/entries/fields/queues/serialization, and make explicit updates cancellable
and off UI/audio callbacks. Stable IDs must include the selected root identity
and must not depend on a filename alone. Keep the known local TOCTOU limitation
documented unless the host uses OS no-follow descriptor APIs.

### ML implementation passes

| Pass | Scope | State |
| --- | --- | --- |
| ML-00 | Baseline inventory of scanner, host slot, UI, persistence, tests and footprint | complete; current scanner is explicit and bounded, but has no stable IDs, search, playlists, or durable queue |
| ML-01 | Data contract for root-scoped IDs, metadata, catalog snapshots and limits | partial: stable root-scoped IDs and query API implemented |
| ML-02 | Bounded local metadata for supported containers; skip image payloads | planned |
| ML-03 | Multiple explicit roots, reconcile/missing media, search, stable sort and pagination | partial: single-root query/sort/pagination plus atomic cancel-aware scan API; multiple roots/reconciliation and UI worker wiring remain |
| ML-04 | Durable playlists and safe M3U/M3U8 import/export | partial: bounded text-only M3U8 model, versioned `.rmp` persistence, CLI CRUD, and standalone text UI for create/rename/delete/add/queue; host workflow and import/export remain |
| ML-05 | Persistent queue, transport state, repeat/shuffle and failure recovery under host-owned focus | partial: bounded versioned queue state, Unix atomic persistence, CLI/UI queue editing, queue navigation, repeat and seeded shuffle; explicit queue selection is non-autoplay; standalone process focus does not prove Rust Boy shared focus |
| ML-06 | Text-only desktop and 1 KiB panel presentation | partial: explicit folder selection, cancellable bounded catalog load, text search/list, queue and named-playlist controls, Play/Pause/Resume/Stop/Next/Previous and software volume; fixed panel is implemented, physical input/display and host workflow remain |
| ML-07 | Synthetic scale, restart/corruption and performance profiles | partial: catalog build/search profiled at 0/1/100/2k/20k; cold app startup, incremental memory, restart/corruption remain |

Profile synthetic catalogs at 0, 1, 100, 2,000 and 20,000 entries. Initial
engineering target: under 8 MiB additional idle catalog memory for a defined
typical library and zero idle scan CPU. Measure cold startup, peak RSS, catalog
build and search latency; record the fixture shape and tool rather than
presenting unmeasured targets as achieved.

Required regressions cover duplicate/non-ASCII search, stable pagination,
malformed or oversized tags/playlists, huge image lengths, symlink/traversal
and missing paths, restart after interrupted persistence, no autoplay/network,
read-only source files, deterministic queue replay, and the fixed 1024-byte
framebuffer.

## Evidence

`rust-music-library::tests::flac_tags_are_allowlisted_utf8_bounded_and_artwork_is_skipped`
creates a synthetic FLAC comment with a 2 MiB `METADATA_BLOCK_PICTURE` value,
then confirms that only the three allowlisted text fields are retained. The
unit test does not decode or retain the picture value. Bounded metadata and
seek evidence is tracked under RM-011/G13 in
[G13 metadata and seek](G13_METADATA_AND_SEEK.md). The panel's fixed storage is
covered by [G6 micro panel](G6_MICRO_PANEL.md); process view bounds are in
[PACKAGE_PROCESS.md](PACKAGE_PROCESS.md).

This evidence applies to the local scanner and catalog model. It does not
certify every third-party player or OS component, nor claim that source audio
files have been scrubbed of artwork.

The current catalog slice adds deterministic opaque IDs derived from the
canonical selected root and relative path, duplicate-ID rejection, bounded
Unicode lowercase text filtering, container filtering, deterministic sort,
and bounded pagination. The ID hash is for local identity only, not
authentication. `cargo test --locked --offline -p rust-music-library` passes
15 tests covering repeat-scan stability, root scoping, duplicate filenames,
Unicode queries, pagination, query bounds, safe M3U8 round trips, and playlist
limits. The new playlist model accepts only bounded UTF-8 relative paths with
supported lossless-container suffixes; it does no filesystem access. Playlist
durability, multiple roots, host queue transport/focus wiring, presentation,
and scale profiling remain unimplemented.

`rust-music-library::queue` adds a bounded versioned text record, Unix atomic
save/load restricted to `.rmq` state files, relative lossless paths, repeat
Off/One/All, explicit selection/next/previous and deterministic seeded
shuffle. Queue navigation never starts playback or acquires audio focus. The
six queue tests cover state round trip, malformed records, bounds, navigation,
repeat/shuffle, and queue-file persistence while refusing a symlink target;
the audio file fixture remains byte-identical. Non-Unix queue persistence
currently returns `Unsupported`. Local filesystem path replacement races
remain documented; resolve every queue entry through the selected library's
checked-path API before playback.

`rust-music-library::playlist_store` adds bounded named playlist create/rename/
remove/edit operations and a versioned `.rmp` record. Names and relative audio
paths only are persisted; store limits cover 128 playlists, 10,000 aggregate
entries and 1 MiB serialized state. Atomic save uses a same-directory temporary
file and rename, mode 0600 plus parent-directory sync on Unix, and platform
native rename behavior elsewhere. The store refuses symlink/non-file targets,
duplicate names, malformed versions, traversal and over-limit state. Four new
tests cover CRUD/restart round trip, malformed and unsafe records, Unix atomic
replacement/symlink refusal, and that playlist edits never open or change
audio files. This is a data-model/persistence slice; no standalone or host
workflow is implied by it. Like queue and catalog state, the symlink metadata
check and subsequent file open can race a hostile local filesystem actor;
hosts requiring that threat model need an OS no-follow/openat-style API.

`scan_cancellable` accepts a caller-owned `AtomicBool`, checks it between
filesystem entries, and returns a cancellation error without exposing a
partial catalog. Callers can replace the committed catalog only after a full
successful result, so cancel preserves the prior snapshot. A deterministic
mid-scan cancellation test verifies that path; UI worker ownership and progress
presentation remain unimplemented.

### Standalone catalog/queue/playlist workflow slice (2026-10-31)

The macOS app now offers explicit folder selection, a bounded Rust CLI catalog
scan with cancellation, text search and catalog rows, explicit Play and Stop,
and selection-only Previous/Next. Persistent queue and named-playlist CLI
state is loaded only after a successful explicit library selection. The UI
supports queue add/remove/select, repeat, shuffle, playlist create/rename/
delete/add, and queueing a playlist. State records remain in app-support
storage scoped by canonical root; catalog paths are revalidated before managed
playback. Selecting queue rows or using Next/Previous does not start playback
or acquire audio focus. `sh scripts/offline-gate.sh` and
`sh scripts/build-macos-app.sh` both exited 0 at the recorded implementation
snapshot; local ad-hoc app bundle signature verification passed. The image and
visualizer symbol search over the app source returned no matches. Pause,
volume, autoplay preference, EQ, host transport/focus bridge, physical output,
and live Radio/TV contention are not implemented or verified by this slice.

### Initial synthetic catalog profile (2026-10-31)

Built with `cargo build --release --locked --offline -p rust-music-library
--example profile_micro_library` (exit 0), then ran
`/usr/bin/time -l target/release/examples/profile_micro_library` (exit 0).
Fixtures are empty/small WAV signatures only; no audio payload is generated.
The machine was idle apart from the profile and reported no audio devices.

| Tracks | Catalog scan | Search p50 | Search p95 |
| ---: | ---: | ---: | ---: |
| 0 | <1 ms | 83 ns | 84 ns |
| 1 | <1 ms | 166 ns | 292 ns |
| 100 | 1 ms | 3.959 µs | 5.083 µs |
| 2,000 | 26 ms | 68.125 µs | 73.458 µs |
| 20,000 | 320 ms | 671.625 µs | 717 µs |

The finalized run took 2.93 s including fixture creation, scanning and 51
searches at each size. Peak process RSS was 11,239,424 bytes (10.72 MiB), which includes
the executable and profiling harness; it is not incremental catalog memory.
Cold application startup and incremental idle memory were not isolated, so the
under-8-MiB additional-memory target remains unverified. The exact fixture
generator and query are in
[`examples/profile_micro_library.rs`](../crates/rust-music-library/examples/profile_micro_library.rs).

### Integrated standalone transport continuation (2026-10-31)

The standalone UI now keeps one persistent RBPK child process for transport
and wires Play, Pause, Resume, Stop, Next, Previous, queue transitions, and
bounded 0–100 software volume. Pausing releases the Rust host lease; Resume
must reacquire it. Focus-denied Resume remains paused. Catalog browsing,
search, queue selection, and playlist selection do not start playback. State
operations and transport serialize at the UI boundary, and window shutdown
sends Stop/Shutdown before bounded termination escalation and child reaping.

The package-stream regression `package_stream_runs_play_pause_resume_volume_stop_end_to_end`
passes. It initially exposed and fixed `LazySink` not forwarding pause/resume.
The macOS AudioQueue fault suite passes 15/15, including pause/resume before
the first write. `sh scripts/offline-gate.sh` and `sh scripts/build-macos-app.sh`
exited 0; the app bundle passed local strict ad-hoc signature verification.
Roadmap/literature checks and the no-image-API search passed. Independent
review caught and fixed a Next/Previous while-paused selection mismatch; the
follow-up review confirmed the fix. The exact app UI also passed a synthetic
silent-WAV smoke for explicit load/search, queue and playlist persistence
across restart, Play/Pause/Resume/active Next/Previous/Stop, and volume changes.
This does not prove audible output: `system_profiler SPAudioDataType` listed no
devices. Physical device recovery, Rust Boy shared focus, cold startup, and
incremental catalog memory remain unverified. Autoplay and native EQ remain
follow-on features for the standalone v1.0 scope. This work adds no artwork or
image functionality and does not rerun private codec parity.
