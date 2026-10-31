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
