# MAC-07 — Winamp-functional desktop alignment: implementation passes

Status: **ACTIVE TASKING / NOT IMPLEMENTED** (2026-10-31). Source of requirements: operator-supplied Winamp Media Library screenshot and confirmed usability failures. Do not reproduce copyrighted imagery or Winamp skins; replicate interaction semantics only.

## Observed baseline and evidence boundary

- Operator reports native Rust CLI playing an actual FLAC audibly; the same music does **not** play via macOS graphical player. This is one operator-reported CLI acceptance, not full GUI/hardware certification.
- Current GUI library-to-queue operation is difficult. Do not represent code-level repairs, A/B testing, or physical app acceptance as complete.
- Use MAC-05 and MAC-06 for existing diagnosis and parity evidence. Commit no private audio or private absolute paths.

## Functional layout contract

One resizable window in four regions:
1. **Persistent compact player** (top): title/artist/album, state, elapsed and duration from validated engine snapshot, Play/Pause/Stop/Previous/Next, software volume, repeat and shuffle, visible failure feedback.
2. **Library navigation** (left): Now Playing, All Music, Artists, Albums, Folders, Playlists, Recently Added, Recently Played. Recently Played must not be shown as populated without durable history.
3. **Track browser** (center): Search, Artist, Album, Track #, Title, Length, Format and optional Genre. Resizable sortable columns, multi-selection, keyboard focus, non-blocking incremental filter; missing metadata falls back to filename.
4. **Queue / playlist editor** (right or docked): current/next indicated, enqueue/dequeue, reorder, play next, clear, repeat/shuffle; named playlists distinct from live queue. Layouts collapse logically for small windows, no unusable text clipping.

The supplied screenshot is a **functional reference**, not a requirement for album artwork, visualizer, image assets, new dependencies, or separate floating windows. Preserve no-artwork policy, 128x64 / 1024-byte Pip-Boy output, local-only code, no network, accessible text and keyboard controls.

## End-user interaction contracts

- Select folder -> explicit bounded recursive scan -> persistent catalog -> immediate browsing; no scan/play on app startup.
- Single-click: select only. Double-click or Enter on track: play it. Use predictable queue policy; document whether Play Now replaces or positions the active cursor without destroying saved playlists.
- Right-click: Play Now / Play Next / Add to Queue / Add to named Playlist / File Info.
- Shift/Cmd multi-select and batch enqueue; track/album-level queue addition with stable ordering.
- Finder drag-and-drop for approved directories/files; no symlink/traversal bypass; never write to source media.
- Drag reorder queue with keyboard accessible Move Up/Down alternatives; persistent queue changes survive restart without unsolicited playback.
- Previous/Next **starts** selected adjacent queued media (unlike selection-only behavior); natural EOF advances exactly once, no double-play and no overlapping sinks.
- M3U/M3U8 import/export safely resolves relative paths, refuses malicious traversal and out-of-root references; playlist files are distinct from queue state.
- Seek bar only when sample-accurate seek is truly supported; otherwise noninteractive read-only progress.

Machine-readable pass order: [mac07-pass-index.json](mac07-pass-index.json). Evidence ledger: [MAC07_PASS_RESULTS.md](evidence/MAC07_PASS_RESULTS.md). Registry task groups RM-028/029/030 must not be marked done until all mapped passes have independent evidence.

## Engineering passes (sequential release gates)

| Pass | Owner | Changes | Required evidence |
| --- | --- | --- | --- |
| MAC07-00 | gate_verifier + host_integrator read-only | Pin SHA, branch, working CLI FLAC file, broken GUI same-file path, output route, logs, baseline workflow friction | Repro report with commands/exit states and no private path or music |
| MAC07-01 | host_integrator; audio_backend consult | Fix SwiftUI package-process select/Play/queue/focus and error visibility. Trace IPC request/reply and worker -> decoder -> native sink. Preserve RBPK v1. | GUI plays same actual FLAC audibly, diagnostic frame/state checks, regression reproduces pre-fix |
| MAC07-02 | host_integrator | Implement typed validated state/progress; clear on change, correct pause/end/failure, accessible elapsed/duration | Swift protocol edge-case + UI state tests; no false Playing |
| MAC07-03 | host_integrator | Build persistent compact player + left navigator + sortable/searchable central table + docked queue skeleton | Screenshot/layout inspection at compact and expanded sizes, keyboard focus, no dropped tracks |
| MAC07-04 | host_integrator | Double-click/Enter to Play Now, batch queue/Play Next, right-click menus, functional active-track transport and EOF advance | Deterministic integration tests with non-silent fixtures plus operator audible confirmation |
| MAC07-05 | host_integrator | Drag reorder, Finder import, named playlists, M3U8 roundtrip; preserve user queue on restart | Path-safety, persistence, Unicode, queue ordering and audio-fidelity tests |
| MAC07-06 | gate_verifier + safety_reviewer read-only | A/B baseline vs candidate for CLI/app, WAV/FLAC, local/approved SMB, queue, 2k/20k catalog, keyboard/accessibility, device errors | Raw observations, fixed source SHAs, 3 functional repetitions and 5 UX pilots; distinguish acoustic vs state evidence |
| MAC07-07 | index_curator | Update roadmap, AGENTS, agent map, evidence and release blockers after verified work | Index checks, full offline gate, macOS build/signature, no fabricated results |

## Agent assignment and lock discipline

- **host_integrator** is sole SwiftUI / desktop library / queue UI writer; own `apps/rust-music-macos/` and approved `crates/rust-music-library/` changes.
- **audio_backend** writes `crates/rust-music-native/` only if an actual sink defect is proven. Do not replace working CLI decoder/audio code speculatively.
- **codec_engineer** remains read-only for MAC07 unless FLAC PCM parity regresses.
- **gate_verifier** independently runs existing fixtures and validates baseline/candidate metrics; **safety_reviewer** audits file import, output focus, no-network / no-artwork policy.
- **index_curator** alone updates roadmap/registry docs; shared core/engine or manifests require serialized ownership.
- Follow AGENTS.md agent_skills-first pin and permissions. Read-only review agents cannot assert that the system enforced their read-only role.

## First implementation kickoff — execute MAC07-00 then MAC07-01

1. Inspect `git status --short --branch`, `git log -5 --oneline`, and existing local edits; do not reset, clean, force-push or overwrite.
2. Read `AGENTS.md`, `docs/CODEX_MAC06_EXECUTION.md`, `docs/MAC05_SILENT_PLAYBACK_PROGRESS.md` and this file.
3. Reproduce direct `rust-music-cli play` vs SwiftUI `serve --desktop` with **the same existing FLAC bytes**. Diagnose selection, path identity, queue cursor, protocol responses, AudioQueue startup, output lease, and swallowed stderr.
4. Implement a focused failing regression, then minimal fix. Add bounded opt-in logs without private paths/media.
5. After each pass run focused tests; prior to accepted macOS candidate run `cargo test --locked --offline --workspace`, `sh scripts/offline-gate.sh`, `sh scripts/build-macos-app.sh`, `codesign --verify --deep --strict --verbose=2 target/macos/RustMusic.app`, and roadmap/literature checks. Report unavailable macOS hardware as BLOCKED, not PASS.
6. Commit small reversible changes, preserve private corpus, do not merge or claim production acceptance without operator audible GUI FLAC.

## Functional acceptance script

Open app -> choose FLAC album folder -> see searchable tracks/artist/album -> double-click a real FLAC -> **hear** music -> add 5 tracks with multi-select -> rearrange queue -> Next changes actual music -> Pause freezes progress -> Resume continues -> save playlist -> relaunch -> library, playlist and queue restore, **no automatic playback**.
