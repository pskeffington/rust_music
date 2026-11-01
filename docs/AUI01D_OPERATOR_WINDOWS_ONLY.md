# AUI-01D — Operator Windows Only; Preserve Rust Boy Theme

Date: 2026-11-01
Status: AUTHORITATIVE UI SCOPE CORRECTION / IMPLEMENTATION NOT RUN
Supersedes styling instructions in AUI01B and AUI01C wherever they conflict.

## Operator requirement
Copy the OPERATIONAL WINDOW ORGANIZATION demonstrated by the supplied Winamp Classic screenshot, not its visual appearance. Keep the existing Rust Boy / Pip-Boy phosphor-green theme, typography, native vector accents and project theme policies unchanged. No style rework, new palette, Winamp artwork, skin import or bitmap development is part of this pass.

## Four operator windows
1. Main Player: independent compact window, persistent transport controls, selected/playing identity, factual elapsed and codec status, volume, repeat/shuffle, buttons to show Playlist and EQ.
2. Playlist Editor: its own scrollable, resizable operator window, track rows and queue add/remove/reorder/save/select controls; selecting does not autoplay.
3. Equalizer: independent dockable operator window; actual DSP only if verified, otherwise clearly unavailable/disabled. Do not implement faux filters.
4. Media Library: independent full-height window with local collection navigation, artist and album columns, searchable track list, explicit import, Play and Enqueue operations.

## Window manager acceptance
- Player top left, Playlist directly beneath, EQ beneath playlist; Library docked to the right spanning stack height by default.
- Every window can be separately moved, resized where sensible, hidden/shown, and redocked.
- Edge-snapping, group movement where possible, persistent layout and safe restore on multi-monitor changes.
- Closing Playlist/Library/EQ never stops audio. A single shared playback and queue state, not separate copies.
- Preserve keyboard, VoiceOver, window scaling, shortcut and focus behavior.
- Preserve embedded Rust Boy 128x64 MUSIC panel and RBPK protocol unchanged.
- Use existing visual components and Rust Boy colors. No separate dashboard, new art, visualizer or skin work.
- Maintain MAC-06 actual-playback and truthful status acceptance; UI-only work must not modify decoders/audio sinks.

## Codex immediate task
Inspect checkout/working tree, existing SwiftUI windows and source state; preserve all uncommitted work. Read AGENTS.md and MAC-06 plan. Assign host_integrator sole UI writer. Implement four separate SwiftUI/NSWindow presentation shells sharing the existing observable app model, migrate existing controls rather than restyling them. First achieve default Winamp-like arrangement and independent visibility; then docking, saved layouts and keyboard/accessibility. Gate_verifier checks screenshot-based window composition, independent window controls, shared queue/playback behavior, and actual playback regressions.

All runtime tests NOT RUN at authoring. Do not mark implemented until verified on macOS.
