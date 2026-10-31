# Micro library policy: zero artwork

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
| ML-03 | Multiple explicit roots, reconcile/missing media, search, stable sort and pagination | partial: single-root search, sort and pagination implemented; multiple roots/reconciliation remain |
| ML-04 | Durable playlists and safe M3U/M3U8 import/export | planned |
| ML-05 | Persistent queue, transport state, repeat/shuffle and failure recovery under host-owned focus | planned |
| ML-06 | Text-only desktop and 1 KiB panel presentation | planned |
| ML-07 | Synthetic scale, restart/corruption and performance profiles | planned |

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
9 tests covering repeat-scan stability, root scoping, duplicate filenames,
Unicode queries, pagination, and query bounds. Multiple roots, persisted
playlists/queue, presentation, and scale profiling remain unimplemented.
