# Rust Music — Primary UI Direction (2026-10-31)

**Status: APPROVED PRIMARY DESIGN DIRECTION.** This decision supersedes earlier visual proposals for the standalone macOS player but does **not** supersede lossless playback, local-only dependency, resource, audio-focus, or hardware-release gates.

## Source reference

Approved design: the user-supplied **Rust Music / Rust Boy 2000** composite reference image in the 2026-10-31 project conversation. The original binary image was supplied in chat; it is **not yet tracked in this repository**. A maintainer or Codex with access to the original attachment must copy the unmodified PNG into `docs/design/assets/rust-music-primary-ui.png`, check its hash and dimensions, and update this document to reference that tracked asset. **Do not substitute a generated approximation or claim that the asset is committed before verification.**

## Design contract

- **Identity:** distressed industrial bronze/olive outer frame, deep black panels, crisp phosphor-green type and accents, compact retro-terminal feel. Preserve readable contrast and real macOS hit targets. Decorative metalwork must not compromise performance or resizing.
- **Main player:** prominent now-playing title/artist, truthful container/codec/sample-rate/bit-depth metadata, timeline, previous, pause/play, next, stop, repeat/shuffle and volume. No fake audio state or visualizer-driven dependencies.
- **Playlist:** bounded scrolling track list, selection distinct from actual playback, add/remove/reorder, and persistent queue/playlist semantics; never autoplay by merely selecting.
- **Library:** explicit directory import/scan, tracks/artists/albums/genres/favorites/search derived from real indexed local metadata, empty states, large-library performance budgets.
- **Settings:** actual output-device selection, volume, playback mode, optional crossfade and EQ only when the underlying feature works. Disabled or marked unavailable until implemented and verified. User preferences persist.
- **Curie 2000:** optional bright phosphor-green ferret companion, with idle/playing/paused/error reactions; animations lightweight, bounded, and disabled by preference or reduced-motion setting. Never impact audio timing or become a playback prerequisite.
- **Mini player:** optional small window with actual title, transport state, and buttons bound to the same single audio session.
- **Micro/Rust Boy:** retain the existing 128x64 fixed-buffer panel as a separate resource-tier presentation, not a compressed copy of the macOS desktop artwork.

## Engineering priority for PR #1 (macOS production candidate)

1. Finish host-owned audio focus and single active stream, real pause/resume/stop, volume, selection semantics and correct state snapshots.
2. Implement UI shell around functional library, queue, playlist, and real transport; map approved visual hierarchy to native macOS controls with accessible labels, keyboard use, and adaptable layout.
3. Persist and recover library/queue/playlist/preferences reliably; validate startup, cancellation, corrupted media, missing files, and large-library scanning.
4. Add real output-device behavior and lossless output acceptance; performance and memory profiles on cold start and incremental scans.
5. Add optional Curie 2000 animation and mini player after reliable playback; optional EQ/crossfade only after measurement and verified behavior.
6. Release checklist: offline locked workspace gates, local authored codec tests, macOS bundle/signature checks, hardware playback acceptance, accessibility/reduced-motion checks, and no regressions in Rust Boy micro mode.

## Codex instructions

Treat this document as the **primary product-design authority**. Create an indexed task map in the repo roadmap, implement in small independently tested passes, never replace local decoder modules with third-party runtime dependencies, and record evidence for each acceptance gate. Do not claim features are implemented merely because they appear in the mockup. Keep PR #1 draft until all mandatory macOS gates pass. Do not merge into `main` without release acceptance.

## Visual acceptance

Match the supplied reference in color palette, screen layout, information hierarchy, button affordances, and recognizable Curie silhouette, while keeping decorative panels separately scalable and respecting platform accessibility. The mockup's example track names, number of tracks, timer readings, and music metadata are **illustrative, not fixtures or product claims**.
