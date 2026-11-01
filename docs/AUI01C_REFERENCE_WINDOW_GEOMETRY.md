# AUI-01C — screenshot-anchored classic multiwindow geometry

Date: 2026-11-01. Status: DESIGN SPECIFICATION / NOT IMPLEMENTED.
Reference: operator-supplied screenshot of Winamp Classic: Player at top left, Playlist middle left, Equalizer bottom left, Library spanning right column. This file translates only layout and interaction into an ORIGINAL Rust Music implementation; do not copy proprietary art, glyph maps, skins, branding or bundled fonts.

## Exact composition / proportional baseline

Reference canvas approximately 600 x 450 px. The dark panel assembly begins around x=2, y=17 and ends at x=599, y=426. Use normalized composition, not these pixels as hard-coded sizes:
- LEFT column approximately 50% of total width.
- RIGHT Media Library approximately 50%.
- Left PLAYER top occupies approximately 30% of assembled height.
- Left PLAYLIST mid occupies approximately 38%.
- Left EQUALIZER bottom occupies approximately 32%.
- RIGHT LIBRARY spans entire three-window height.
- Window headers are narrow draggable titlebars; thin beveled/separator borders create clear docking edges.
- Small gap between independent windows; adjacent edges snap and group as a single stack on demand.

At first launch use a sensible device-scaled size, ideally ~2x logical reference for modern Retina readability, and allow user-defined scaling. Never assume screenshot-era hit target sizes are accessible.

## Main Player top-left

Compact horizontal status panel. Header: RUST MUSIC, minimize/windowshade/close. Left: elapsed time, truthful status visualization expressed as TEXT/meters only if real backend readings exist; no invented frequency bars. Right: full title with horizontally bounded presentation; codec/profile, sample rate and bitrate only when known, stereo/mono from actual metadata. Below: position read-only until real seek is implemented, previous, play, pause, stop, next, eject/load folder, software volume, repeat/shuffle, explicit PL and EQ panel toggles. Differentiated playing vs selected state; error/status accessible and non-animated fallback.

## Playlist Editor middle-left

Dense scrollable row list with track ordinal, display title and duration when available; selected row visually distinct from currently playing row. Right scrollbar. Bottom actions ADD, REMOVE, SELECT, MISC/SORT, LIST OPTIONS and aggregate duration if known. Allow double-click/Enter to invoke explicit Play; arrow keys select only. Direct reorder through keyboard and drag where feasible. Persist queue independently of window visibility.

## Equalizer bottom-left

Separate compact window, similar width to player and playlist. Titlebar and ON/AUTO/PRESETS controls only when a real 10-band DSP exists. Preset/scale markings and slider bands can appear only as clearly DISABLED prototype if EQ is not validated. Do not fake active filters, audio output, numerical frequencies or frequency response. User can hide/detach the panel.

## Media Library right-side

Full-height separate resizable window. Titlebar RUST MUSIC LIBRARY. Split internal panes: narrow left navigation tree with Local Media, Audio, Video (unsupported and disabled), Playlists, Devices (unsupported), Internet Radio (unsupported); hide unsupported categories in production unless explicitly presented as unavailable. Main area: top ARTIST and ALBUM lists as adjacent columns, bottom search and large sortable tracks table. Bottom bar: LIBRARY, PLAY, ENQUEUE, PLAY ALL with honest enabled states. Import folder explicitly; no silent scan or startup autoplay. The user's screenshot has small rows; provide adjustable density and accessibility scaling.

## Docking and persistence

Four independent NSWindow/SwiftUI window shells (or platform-supported equivalents), one shared source-of-truth playback/library/queue model. Snap player's bottom edge to playlist top; playlist bottom to EQ top; media library left edge to left stack right edge. Drag whole group with modifier or grouped drag handle, detach individual windows, restore saved geometry per monitor safely. Detect screen changes and recenter any offscreen window. Closing library/EQ/playlist does not stop music; closing/quitting app does clean up actual audio subprocess and focus. Preserve the Rust Boy 128x64 hosted micro panel and exact RBPK wire schema.

## Style

Use an ORIGINAL dark graphite/olive with phosphor-green text, small bright accent indicators, compact titlebars and crisp separators. Avoid the prior broad dashboard, large cards, ornamental bezels or integrated sidebar. Existing zero-artwork/no-skins macOS v1.0 policy controls: use SwiftUI native vector shapes and text only. Save independently replaceable panel themes as a FUTURE policy-gated architecture. Imported Winamp bitmap WSZ files remain out of v1.0.

## Implementation instruction

Read AGENTS.md, docs/AUI01B_WINAMP_WINDOWS.md, docs/CODEX_MAC06_EXECUTION.md and current SwiftUI sources first. Pin a baseline source SHA. host_integrator exclusively modifies SwiftUI; audio_backend exclusively modifies native audio; gate_verifier independently checks input, window positioning, accessibility, actual WAV/FLAC playback, correct frame-derived progress and cleanup. Begin by replacing dashboard navigation with separately controlled window shells and a shared presentation model, then the player/playlist layout, then library panes, then EQ stub. Add screenshot regression fixtures authored from original app output, NOT reference art.

## Acceptance gates

Compare intended proportions and window relationships with supplied reference, not exact copyrighted pixels. Test low/high DPI, resize, docking, keyboard, VoiceOver, reopening, no phantom Playing state, actual audible local file and non-autoplay launch. Run existing repository offline gate and Mac build/signing. All tests NOT RUN at specification authoring; no implementation or release PASS claim.
