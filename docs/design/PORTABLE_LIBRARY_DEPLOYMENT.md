# RM-LIBSYNC-01 — Desktop-managed library, offline Pip-Boy deployment

Status: **approved product direction / proposed schema, not implemented**.
Date: 2026-10-31.
Read alongside [PIPBOY_SOCKET_SCHEMA.md](PIPBOY_SOCKET_SCHEMA.md), [PRIMARY_UI_DIRECTION.md](PRIMARY_UI_DIRECTION.md), [../HOST_CONTRACT.md](../HOST_CONTRACT.md), and [../CODEX_MACOS_V1_DELIVERABLE.md](../CODEX_MACOS_V1_DELIVERABLE.md).

## Product principle

A user can manage their own lossless music library in standalone Rust Music desktop applications on **macOS, Linux or Windows**. A connected Rust Boy 2000 receives a portable, validated, preorganized offline subset or full library; its MUSIC module can browse, sort and play it without the desktop computer. Do not mistake this cross-platform goal for a claim that the Linux/Windows GUI or deployment pipeline already exists. macOS v1.0 remains the immediate release gate.

## User workflow

1. User explicitly imports a local directory into Rust Music Desktop. Scan only authorized roots and do not alter source media.
2. Desktop builds a durable local catalog of real lossless tracks, available bounded metadata, stable IDs, local relative locations, favorites, named playlists and deterministic sort keys. Unsupported media is marked rather than silently recoded.
3. User selects **Deploy to Rust Boy**, selects whole library or playlist/track subset, previews required capacity and missing/unsupported tracks.
4. User chooses a user-authorized target (initially a mounted removable filesystem or export folder). Copy only changes, use staging and atomic manifest publication; verify all copied bytes.
5. Rust Boy validates the manifest and local media before exposing the snapshot; it loads tracks and ordering without desktop services or re-import. On mismatch, fail safely and retain the last known-good catalog when possible.
6. Later desktop edits can incrementally sync: send new/changed tracks, small library/playlist metadata changes, and explicit deletions, while leaving user-owned destination files alone unless known to belong to this managed package.
7. During playback no network or desktop link is required. Host owns audio focus, mount lifecycle, and Pip-Boy settings. No sync operation may trigger audio playback.

## Package v1 — proposed on-disk shape

```text
RUST_MUSIC/
  library.manifest            # canonical, versioned, bounded, portable encoding
  tracks/
    <content-id>.<extension>   # lossless original bytes; sanitized paths
  playlists/
    <playlist-id>.list         # ordered stable track IDs, bounded
  .staging/                   # ignored by the reader; temporary only
```

This is a **proposed format**, not a deployed format. Before implementation, choose a deterministic bounded encoding that can be parsed by locally authored Rust code without third-party runtime dependencies. Avoid opaque serialization, absolute host paths, hidden OS metadata dependencies, network fetching or cache pointers to the desktop.

### Manifest fields

- `schema_version`, `library_id`, `generation`, `created_at`, optional `source_platform` (informational only)
- `tracks[]`: `track_id`, `relative_media_path`, `byte_size`, `content_hash`, `container`, `codec`, `display_title`, optional `artist`, `album`, `genre`, `track_no`, `disc_no`, `duration_frames`, `sample_rate`, `bit_depth`, `channels`, `sort_keys`; only verified or known metadata
- `playlists[]`: stable `playlist_id`, bounded name, ordered `track_id` references; no dangling references accepted silently
- `favorites[]` and optional user-visible manual track order
- `capabilities_required`: declared decoder/format profiles and schema minimum
- `package_integrity`: canonical manifest hash or independently verifiable generation/checkpoint

A track ID must be stable across sorting, renaming and multiple playlists. Define identity based on verified content plus collision-safe namespacing; do **not** assume filename is unique. Content hashing is mandatory for deploy verification; use a locally implemented, reviewed cryptographic digest, or a platform-verified safe primitive consistent with repository dependency policy. Do not invent a hash algorithm. Handle duplicate bytes without collapsing user-distinct playlist entries unless the data model explicitly supports that behavior.

## Sync and safety contracts

- v1 transport: explicit export to a selected folder or removable volume. Future wired/IP transport is optional and requires explicit pairing, authorization and separate security review.
- Use a prepared *plan* before writing: source availability, allowed formats, target filesystem capabilities, free space, delta list, conflict strategy and deletion preview.
- Write media to staging, verify byte counts and content hashes, fsync where supported, then publish a complete new manifest atomically using same-filesystem rename. If atomic rename cannot be guaranteed, use a versioned directory/pointer strategy and recovery tests. Never expose a partially published generation.
- On interruption, retain last-known-good generation. Clean only application-owned staging data; never delete arbitrary target paths or unknown music.
- Reject traversal, absolute paths, symlinks, duplicate normalized/case-folded destination names, excessive nesting, oversized manifest fields, unsupported schema, hash mismatch and malformed track entries. Account for case-insensitive Windows/macOS filesystems and removable FAT/exFAT behavior.
- Never rewrite original FLAC/other media just to sort or retag; keep user playlists and catalog separate. No transcoding, lossy downgrade or unaudited resampling in v1.
- Library sync must not play music, request audio focus, fetch from a network or scan unrelated files.
- Destination can retain the original file format only if decoder and host capabilities admit it; distinguish catalog import from playable media.
- Host reads a bounded, validated snapshot, can suspend/unmount without playing, and can still use the fixed 1024-byte micro panel.

## Sort and presentation semantics

Canonical independent sort modes: title, artist, album/disc/track, filename and manual playlist order; genre if actually known. Sorting is deterministic with stable track ID as final tie-breaker, explicit case/Unicode normalization rule and no dependence on locale or desktop OS. Playlists preserve user order even if library default sort changes. The Pip-Boy can filter and sort the preloaded index without reparsing all music files at boot. Unknown tags remain unknown instead of fabricated.

User edits **on the Pip-Boy** should initially be treated as device-local preferences/queue changes unless a verified bidirectional conflict-resolution policy is implemented. v1 is desktop-authoritative, with explicit export/import of device changes considered a later feature. Avoid overwriting local device changes without a preview.

## Ownership and deployment boundary

- Shared core: Rust-authored catalog model, deterministic sort, playlist IDs, schema validation, packer/importer, atomic generation handling.
- macOS: first production desktop shell, native audio output and explicit file/folder selection.
- Linux: future platform-specific native output + GUI adapter, same on-disk package and Rust core.
- Windows: future platform-specific native output + GUI adapter, same on-disk package and Rust core.
- Pip-Boy: RBPK MUSIC slot, shared audio-focus lease, bounded snapshot/renderer, read-only package loader; host grants storage access and owns target UI chrome.
- No cross-platform GUI toolkit or external dependency is authorized by this schema alone. Keep the zero-third-party-Cargo policy; platform adapters require separate reviews.

## Indexed Codex tasks

- **LIBSYNC-01 P0:** inventory actual Rust Music catalog, persistence, RBPK v1 messages, host storage path and current macOS UI. Record current implementation vs plan; no speculative API changes.
- **LIBSYNC-02 P0:** define canonical versioned portable schema and golden test vectors (macOS/Linux/Windows safe paths, duplicate naming, manifest corruption, deterministic ordering, missing fields, unsupported codec).
- **LIBSYNC-03 P1:** implement reusable bounded, zero-third-party dependency Rust packager, validator and safe importer; tests for interrupted writes, hash mismatch, staging recovery and no source modifications.
- **LIBSYNC-04 P1:** desktop UI for destination selection, capacity preview, subset deployment, progress, cancellation and incremental delta plans. Make it first available on macOS; do not block its v1 release on Pip-Boy integration.
- **LIBSYNC-05 P1:** consume validated catalog through existing host/RBPK contract; no second audio engine or scan; test browse/sort/playlist and actual host audio-focus contention with RADIO/TV.
- **LIBSYNC-06 P2:** Linux and Windows native GUI/output adapters and cross-platform portable-package conformance tests (not merely compile claims).
- **LIBSYNC-07 P2:** physical removable-volume and Raspberry Pi/Pip-Boy tests for reconnect, power interruption, latency/memory and usable playback.

Prioritize MAC-01/02/03 and current PR #1 acceptance. Task IDs are a proposed supplemental index until reconciled with `docs/roadmap-index.json`. Preserve local work, one writer per scope, add verification evidence; do not mark tasks complete based on this document.

## Engineering slice — 2026-10-31 sync planner (source committed, not gate-verified)

The new `crates/rust-music-library/src/sync_plan.rs` module exports `MediaEntry`, `Snapshot`, `SyncPlan`, `validate_snapshot` and `plan_sync` through the library crate. It implements bounded, deterministic **pure delta planning** with a caller-supplied 32-byte SHA-256 value, byte-size comparison, generation rollback rejection, transfer-byte overflow protection, duplicate IDs, ASCII-casefold path collisions and a list of stale managed paths. Unit tests are present for empty/incremental cases, rename, traversal/collisions, rollback and overflow. The planner never scans, hashes, opens, deletes or copies media and never autoplays.

**Important limitations:** digest bytes are trusted inputs (not independently calculated), stale path list is **not an authorized delete command**, full Unicode normalization and casefolding are not yet implemented, filesystem-safe no-follow operations and atomic publication are missing, and no compiled/test/hardware result was observed in this connector-only pass. Do not deploy packages or claim RM-023 complete until verification and a safe transfer/publisher implementation exist. Also note that current root-scoped `TrackId` values are not portable identities; consumers must use separate verified portable IDs. Next implement canonical manifest serialization/golden vectors and digest-checked staging, followed by integration tests.

## Engineering slice — canonical manifest v1 (2026-10-31)

The `rust-music-library::sync_manifest` source now defines an offline binary `RMSYNC01` snapshot encoding with little-endian generation/count fields, entries canonicalized by portable 16-byte ID, UTF-8 bounded relative paths, u64 sizes, and caller-provided 32-byte SHA-256 digests. Decoder rejects oversized input, incorrect magic, truncated records, duplicate/noncanonical IDs, invalid/duplicate paths and trailing bytes. Unit fixtures cover empty golden encoding, deterministic reordering, truncation, malicious paths and corruption. This is **pure catalog serialization**, not a cryptographically authenticated package or a working transfer pipeline.

Limitations: the manifest currently stores only basic track identity/location/length/digest, **not** playlists, favorites or full media metadata. Caller must compute trustworthy SHA-256 from actual media, validate decoded files on destination and perform safe transactional publication. Full Unicode filename normalization, symlink-safe opening, replay/rollback persistence, power-loss recovery and on-device acceptance remain required. Source and tests have been committed but **Cargo tests were not run** in this pass because a Rust toolchain was unavailable in the execution environment. RM-022/023 remain open and PR #1 remains draft.
