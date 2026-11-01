# AUI-01B — Winamp Classic window alignment (2026-11-01)

Status: DESIGN AND CODEX DIRECTIVE; IMPLEMENTATION NOT RUN.

## User-directed adjustment
Prefer the Winamp 2.x/5 Classic *window organization*, not a contemporary dashboard. Use separate compact, individually showable windows: (1) Main Player, (2) Equalizer, (3) Playlist Editor, (4) Media Library; optional future Mini Player/windowshade. Each module docks or detaches, remembers visibility/position, and shares a single Rust playback/queue state. Use original Rust Music phosphor green text, original vector chrome, high contrast and native keyboard/VoiceOver support. No copying Winamp artwork/code or shipping unlicensed classic skin assets.

## Layout contract
- Main Player: small horizontal floating window with title, real elapsed/duration, title marquee only with reduced-motion fallback, real codec/sample rate/bit-depth, mono/stereo, transport row [previous/play/pause/stop/next], volume, shuffle/repeat, EQ and PL window toggle buttons. No fake waveform or visualizer.
- Equalizer: a separate height-matched narrow window docked under Main Player; currently display honest 'EQ unavailable' with disabled sliders until the native DSP is implemented/tested. Never present fictional working bands.
- Playlist Editor: vertically resizable, dense rows, numbered order, duration, selected vs playing distinction, ADD / REM / SEL / SORT / SAVE and move controls, direct double-click/Enter Play, no accidental autoplay from selection.
- Media Library: independently resizable larger window, tree navigation Tracks / Artists / Albums / Playlists, search + bounded import, dense columns with sorting, right-click context menu to enqueue or add to named playlist, no album artwork.
- Window docking: snap edges with scalable tolerance, drag grouped stacks, detachable independent panel bounds, snap/dock layout persistent and recoverable off-screen. At first launch stack player/EQ/playlist; library beside or beneath. Windowshade collapses player to title/time/transport.
- Mac ergonomics: 2x UI scale, keyboard shortcuts, VoiceOver actions, minimum touch/target area in accessibility mode, focus preservation and multi-monitor safe restoration.
- Preserve 128x64 Pip-Boy framebuffer and RBPK wire protocol; desktop window stack MUST NOT change embedded player behavior.

## Execution order
AUI-01B-01 inventory current SwiftUI views and actual desktop screenshots, pin baseline source SHA.
AUI-01B-02 introduce distinct window presentation shells tied to one observable transport and catalog model; no backend changes.
AUI-01B-03 implement player + playlist stack, keyboard navigation and correct transport state from MAC-06 typed snapshots.
AUI-01B-04 integrate media library and independent playlist editor, panel visibility, resize/dock/restore.
AUI-01B-05 EQ disabled placeholder only until audited native DSP and acceptance exists; windowshade and density presets.
AUI-01B-06 acceptance: tests for window docking, layout recovery, no duplicated state/queue, VoiceOver and keyboard, 2x scale, actual playback and failures, paired baseline-vs-new task timings.

## Policy gate
AGENTS.md / docs/MICRO_LIBRARY_ZERO_ARTWORK.md and docs/CODEX_MACOS_V1_DELIVERABLE.md currently forbid images, skins and visualizers. Implement first pass with original native vector/text controls only. A true pixel/bitmap WSZ import, reskinning and skin resources require a separately reviewed policy update; archive parsing must reject scripts, path traversal, zip bombs and excessive memory. No WAL execution.

## Validation
Run python3 scripts/check_literature.py; python3 scripts/check_roadmap_index.py; sh scripts/offline-gate.sh; sh scripts/build-macos-app.sh; codesign --verify --deep --strict --verbose=2 target/macos/RustMusic.app; operator real-audible WAV/FLAC A/B; keyboard/VoiceOver and resize/docking tests. All tests NOT RUN at authoring. Do not mark as implemented, release-ready or merged until recorded.
