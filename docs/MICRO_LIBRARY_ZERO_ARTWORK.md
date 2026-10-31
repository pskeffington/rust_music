# Rust Music micro-library: zero-artwork media management

Status: engineering directive; **not** a claim of implementation or test success. Applies to standalone Rust Music and Rust Boy MUSIC slot. Codex must first finish any active local merge/stash reconciliation and preserve existing WavPack/IPC/packaging work before implementation.

## Product contract

Build a Winamp-like *functional* text-first music manager: local library, text search, artist/album/track browsing, playlists, queue, repeat/shuffle, transport controls and compact metadata. **No album artwork or image features whatsoever.** Existing 128x64 monochrome / 1,024-byte framebuffer for text and controls is allowed; it is not an artwork pipeline.

### Absolutely forbidden

- Album/artist/track images, artwork placeholders, thumbnails, galleries, image visualizers/animations, animated skins, image UI widgets, album-art persistence or cache.
- PNG/JPEG/WebP/GIF/BMP/etc media decoders or image-processing packages; no image fetch, generation or automatic scraping.
- Extracting/copying/decoding/caching embedded FLAC PICTURE, ID3 APIC, Vorbis METADATA_BLOCK_PICTURE, MP4 `covr`, or AIFF artwork payloads. Treat image-bearing metadata as untrusted length-delimited blocks and **skip without allocating payloads**, using checked offsets and strict size bounds. Do not store or export them.
- Any network metadata/artwork lookup, runtime telemetry, downloads, general-purpose media frameworks, third-party Cargo crates or optional external image features.
- Spectrograms/visualizers in the ultra-micro product profile. Basic numerical time/progress display in the existing framebuffer is permitted.

## Existing base and ownership

Extend `crates/rust-music-library/src/lib.rs` rather than replacing its explicit bounded `scan`, symlink refusal, signature-based preliminary detection, deterministic ordering and `checked_path` revalidation. Catalog detection is never proof a file can be played; full decoder and lossless floor remain authoritative. Keep project-owned crates, no remote Cargo dependencies. Preserve Rust Boy host-owned exclusive audio focus and no autoplay/background rescans by default.

Architectural separations (use modules or local path-only crates only when justified by measured value):

| Boundary | Responsibility |
| --- | --- |
| Library/catalog | Multiple operator-approved roots, bounded scanning, stable track IDs, refresh, missing-file state |
| Metadata | Own bounded parsers for artist/album/title/track/disc/year/genre/duration/sample rate/bit depth; filename fallback; skip all image tags |
| Search | Case-folded text-only title/artist/album/genre filtering, stable sorting and pagination |
| Playlist | Create/rename/delete/reorder, safe M3U/M3U8 import/export, no original audio modifications |
| Queue | Enqueue/dequeue/reorder, next/previous, stop/pause only when engine implements it, repeat-one/all, deterministic seeded shuffle |
| Persistence | Compact versioned offline index and user preferences; atomic temporary-file/rename where supported, corruption recovery, no media payloads |
| Host/CLI/UI | Shared queue and state API; tiny text-first desktop presentation and fixed Rust Boy panel, no cover-art area |

## Security, data integrity and footprint

- Read-only media scan. **Never delete, move, rename, tag-write or transcode user files without explicit operation/confirmation**. Playlist and database changes must not affect audio bytes.
- Never follow symlinks or permit traversal outside selected roots. Validate playlist relative/absolute paths and recheck before playback; document remaining TOCTOU limitations and use OS-safe descriptor opening where necessary.
- Bound recursion, examined entries, parsed metadata, field lengths, queue/playlist sizes, index allocation and serialization. Reject overlimit operations without publishing partial corrupt state.
- Do not load entire audio files or image blocks for indexing. Prefer header-only, streaming and lazily loaded metadata.
- Add stable, root-scoped IDs (not filename alone); safely account for duplicate files, case differences, moves/removable media and missing paths.
- No unrequested background threads, scans, watchers or network activity. Index updates are explicit and cancellable; GUI/audio callbacks do not scan or allocate.
- Maintain lossless-only admission for playable source >=16-bit PCM and >=44,100 Hz; never upscale or silently resample to pass.
- Optimize for low idle memory, small release binary and low cold-start latency; benchmark against synthetic libraries rather than claiming achieved targets. Initial engineering targets: <8 MiB additional idle catalog memory for a defined typical library; 0 idle scan CPU; retain 1024-byte framebuffer. Measure and revise targets with evidence.

## Engineering passes

1. **ML-00 / baseline:** Inspect current local branch, merge/stash, `apps/`, WavPack, IPC, host, packaging and existing library APIs. Do not stomp Codex's ongoing checkout. Inventory before editing. Record available test commands, dependencies, memory baseline and crate layout.
2. **ML-01 / data contract:** Specify `TrackId`, `TrackMetadata`, `LibraryRoot`, `CatalogSnapshot`, limits and read-only commands. Keep source compatibility via adapter where needed.
3. **ML-02 / metadata:** Bounded FLAC/Vorbis-comments, WAV INFO, AIFF text chunks, and supported format-specific metadata; skip all image-bearing chunks. Absent tags use sanitized filenames. Reject malformed lengths and invalid encodings safely.
4. **ML-03 / catalog:** Multiple roots, explicit incremental rescan/reconcile, missing/removable media, deterministic IDs, sorting/search and pagination without scan blocking UI.
5. **ML-04 / playlists:** Versioned durable playlist model, M3U/M3U8 path safety, bounded input, atomic saves; user files untouched.
6. **ML-05 / queue:** Deterministic state machine and transport integration (no fake pause/seek claims). Repeat/shuffle and failure recovery with host audio lease ownership.
7. **ML-06 / presentation:** Text-first library/playlist/queue panels and tiny 128x64 host view. Never introduce image UI, skins or art placeholders.
8. **ML-07 / profiling and release gate:** Synthetic 0/1/100/2k/20k-track fixtures, corruption/fuzz-like adversarial metadata/playlist cases, persistence/restart, removable media, CPU/RSS/binary size benchmarks. Run `sh scripts/offline-gate.sh`, local validators and relevant native parity tests; no hardware success claim without hardware.

## Required regression tests

- PICTURE/APIC/covr/metadata images are skipped without content allocations, decoded pixels, saved artwork or index entries; huge/truncated lengths fail safely.
- Spoofed extensions, unsupported lossy streams, invalid container structures and malformed tag lengths are refused by playback admission.
- Symlinks, dot-dot paths, concurrent replace/race scenarios and missing media fail closed.
- Search/sort stable under duplicates and non-ASCII text; catalog bounded under deep trees and excessive tags.
- No autoplay, no implicit network or background scan; no file mutation on playlist/queue edits.
- Queue replay and repeat/shuffle deterministic; malformed M3U and interrupted index saves recover.
- Fixed 1,024-byte display holds; no art assets, decoder dependencies or image code path enters builds.

## Codex coordination / acceptance

Use `agent_skills` software-delivery orchestrator, implementation, verification, security review and documentation skills at a **verified development-branch SHA**; never assume its minimal `main` holds these skills. Pair with this repository's `host_integrator`, `codec_engineer`, `index_curator` and read-only `safety_reviewer`. One writer per overlapping scope; preserve existing unmerged work. Extend the roadmap with stable `ML-##` subtask references without breaking the canonical RM registry or claiming implemented gates. Document actual command output, memory measurements, and acceptance evidence. Keep PR #1 and mainline merges under the existing verification gates.

**Success definition:** Fully local, user-controlled, searchable, playlist-capable, text-only lossless Rust player and library manager with no image-related code, artwork UI, downloads or third-party runtime dependencies.
