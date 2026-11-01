# Winamp usability study — Rust Music agent engineering directive

**Date:** 2026-10-31. **State:** research indexed; implementation and operator A/B testing pending. **Scope:** native macOS player UI, portable Rust Boy MUSIC navigation patterns. Read [MAC-06](MAC06_DIAGNOSIS_UI_AB_PLAN.md), [literature matrix](LITERATURE_MATRIX.md), and [agent map](CODEX_AGENT_MAP.md).

## Research synthesis

Winamp's documented desktop design combines an immediately accessible player, library and playlist editor, direct playlist addition/reordering, multiple sorting choices and explicit selection/Enter behavior (LIT-022). Borrow its direct manipulation and low navigation cost, not its bitmap skins or window chrome. Nielsen's heuristics (LIT-023) emphasize visible status, consistency, recognition and control. Apple's macOS HIG (LIT-024–027) supports resizable layouts, familiar keyboard behavior, honest progress and assistive access.

This is a comparative heuristic study, **not** a controlled human-subject experiment and not evidence that Winamp performs better on a measured task.

## Rust Music gap analysis

| Current observation | Winamp/HIG comparator | Build requirement | Gate |
| --- | --- | --- | --- |
| Playback says Playing, but user reports silence | Visibility of actual system state | Distinguish requested playback, delivered frames, sink errors and verified route; do not claim audible output from player state | P0 |
| No elapsed/total progress | Persistent transport feedback | Parse RBPK counters, render accessible non-seekable timeline, handle unknown length | P0 |
| Library, queue and playlists split into separate sidebar destinations | Winamp integrated playlist workflow | Persistent Now Playing and transport with one-step add to queue/playlist and selected-item context actions | P1 |
| Large unused desktop center region | macOS information density and resizing | Resizable two/three-pane mode: library list, contextual queue, persistent player; compact window gracefully collapses panes | P1 |
| Track title truncates | Visible metadata | Full title on focus/selection, separate artist/album, scalable typography and tooltips | P1 |
| Volume displayed but silent output | User confidence and actionable errors | Label exact software volume, route and output error states; avoid misleading playing indicator | P0 |
| Manual mouse navigation predominates | Keyboard-first music management | Search focus shortcut, Enter-to-play, arrow list navigation, queue add, play/pause shortcut with text-field focus rules | P1 |
| No built-in verified usability metrics | A/B comparison | Log only local opt-in aggregate task metrics, no telemetry, no music paths | P1 |

## Agent assignments (no new unvalidated agent definitions)

- **host_integrator — writer:** prototype SwiftUI UI and typed playback snapshot; own `apps/rust-music-macos/`, UI fixtures and accessibility tests. Maintain existing wire protocol and metadata-only layout.
- **audio_backend — writer, separate scope:** native AudioQueue output analysis and focused error instrumentation under `crates/rust-music-native/`; no GUI edits.
- **gate_verifier — read-only:** compare baseline and candidate with the same tasks and same files/devices; capture raw exit codes, frame progression, focus, user observed audible output.
- **safety_reviewer — read-only:** audit no artwork, no third-party packages, privacy, bounded state handling and no surprise scanning/autoplay.
- **index_curator — serial documentation writer:** update roadmap task status and dated evidence only after independent validation; keep IDs consistent across literature index and matrix.

## Paired A/B usability protocol

**A** = baseline UI at pinned source SHA. **B** = changed UI at new SHA. Same Mac, same window size, same explicit local library and audio endpoint; randomized A/B order when practical. Repeat each task 5 times per condition for pilot engineering comparison, report individual timings/errors and medians, not statistical significance.

1. Load library, find an exact artist/title and start playback.
2. Add a track to queue, reorder/select and play requested next track without losing context.
3. Create named playlist, add several items, save, restart, verify persistence.
4. Pause and resume; identify elapsed and duration; inspect current source/output status.
5. Diagnose a deliberate failed/unsupported file without misunderstanding a Playing label.
6. Complete all above with keyboard navigation; rerun essential tasks with VoiceOver and enlarged text.
7. Repeat at narrow and wide window sizes and at 2k/20k synthetic tagged catalogs for memory/latency stress.

Report time to first correct action, task completion/error count, context switches, focus traps, discoverability of current track and output, CPU/memory, and perceived responsiveness. Compare paired observations; no arbitrary performance PASS without declared budget and reproducible measurements.

## Acceptance and dependency gates

P0 audio root cause from MAC-06 must be diagnosed before UX-only success can be called release ready. Typed progress UI must be snapshot-derived, robust to zero sample rate/unknown total/overflow and freeze on Pause. A seekable bar is forbidden until real seek exists. No album art, imagery, skin system, visualizers, bundled media, automatic library scans, or auto-play on launch.

Run `python3 scripts/check_literature.py`, `python3 scripts/check_roadmap_index.py`, `sh scripts/offline-gate.sh` and `sh scripts/build-macos-app.sh` locally, recording exact results. No status promotion on the basis of a remote documentation commit alone.

## Source IDs

LIT-022 Winamp Desktop Player; LIT-023 NN/g Ten Heuristics; LIT-024 Apple Designing for macOS; LIT-025 Apple Progress Indicators; LIT-026 Apple Keyboards; LIT-027 Apple Accessibility. All external resources are behavioral references only.
