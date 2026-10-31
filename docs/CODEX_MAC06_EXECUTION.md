# CODEX KICKOFF — MAC-06 audio A/B testing + desktop UI repair

**Priority:** P0 blocking audible playback, P0 progress truth, P1 Winamp-class library/queue usability. **State:** authorized tasking; not an assertion of completion. Review [MAC-05](MAC05_SILENT_PLAYBACK_PROGRESS.md), [MAC-06 investigation](MAC06_DIAGNOSIS_UI_AB_PLAN.md), [Winamp research](WINAMP_UI_AGENT_RESEARCH.md), [literature matrix](LITERATURE_MATRIX.md), [macOS acceptance](CODEX_MACOS_V1_DELIVERABLE.md), and [AGENTS](../AGENTS.md).

## Codex operating instruction

Work in `pskeffington/rust_music`. First run `git status --short --branch`, `git log -5 --oneline`, inspect current source and unpushed local work. Do not clean/reset, overwrite user files, force-push or merge without evidence. Follow pinned agent_skills instructions when available. Only one writer per scope: host_integrator for SwiftUI and package response parsing, audio_backend for native sink, gate_verifier read-only measurements, safety_reviewer read-only audit, index_curator serial documents. The same engineer may orchestrate but must serialize overlapping edits.

**Ship a working, audible Mac player with an honest progress display, not a cosmetic mock-up.** Find and repair the cause of the observed UI Playing-with-no-sound symptom. Neither a passing build, a UI Playing label, nor frame counts alone prove audible output. The earlier CLI error 2 used a non-existent placeholder path and does not test audio. The screenshot's Track 011 is not established as the separate SMB FLAC in Finder Get Info.

## Pass 0 — freeze baseline and produce reproducible evidence

- Record branch/source SHA, OS version/architecture, local audio output endpoint, app volume, system volume, app's selected track name and exact on-disk file identity (redact paths from committed reports).
- Operator provides or approves a **non-silent** small lossless fixture. Create a matching WAV/FLAC pair from known authored/CC0 PCM; generated silent WAV files cannot validate audible output. Avoid embedding licensed tracks or data in repo.
- Run baseline CLI and app with the **same actual file bytes**. If testing SMB, copy to local only with permission, prove file identity by SHA-256, and compare mounted-volume vs APFS access.
- Observe process exit codes; capture two snapshots with monotonic times, playback state, source format, delivered/total frames, focus state, and audio enqueue/start outcomes.
- Commit a concise report template with observed/pass/fail/blocked rows. Hardware audio cannot be marked PASS from automated simulated tests.

## Pass 1 — output regression diagnosis (P0)

Implement opt-in, locally bounded diagnostics: child exit/errors, decoder admission and nonzero PCM, AudioQueue create/start/enqueue/stop OSStatus, output route if OS can reliably report it, volume state, and monotonically increasing delivered frames. Never log raw audio, private paths or credentials; keep I/O away from audio callback. Fix the actual root cause only after a failing reproduction test. Ensure native errors transition visibly to Failed rather than staying Playing. Preserve host-owned audio-focus semantics and local-only dependencies.

## Pass 2 — typed progress snapshot (P0)

- Preserve RBPK v1 binary wire compatibility. Refactor Swift `PackageResponse.view` to hold typed playback state, sample rate, format, selected track ID, delivered frames, total frames (optional), audio-claim flag.
- Decode with strictly bounded reads and reject malformed/truncated frames, invalid flags, zero or nonsensical sample rates, and invalid totals. No overflow or divide-by-zero.
- Retain state on `PlayerModel`; update from real snapshots only. Calculate `seconds = frames_delivered / sample_rate` and total duration if known; clamp progress and display **position estimate from delivered frames** (not a physical device clock).
- Reset on explicit new selection/library change/Stop; freeze during Pause; resume from last truthful counter. EOF uses actual final state. Unknown total displays elapsed with 'duration unknown'. A progress indicator is read-only; no fake seek slider or timer-inflated playback.
- Add Swift protocol and model tests for Play/Paused/Stop/Finished/Failed, long duration, unknown duration, zero sample rate, truncated payloads, selection changes and counters > total.

## Pass 3 — Winamp-inspired usability repair (P1)

Keep text-only phosphor theme, zero artwork, and no visualizers. Layout: persistent Now Playing + transport, usable library/search/sort in main area, contextual queue and playlists, condensed diagnostics drawer, clear output/focus/error state. Eliminate oversized blank space; make main panels resize/reflow gracefully. Accessible elapsed/duration and VoiceOver labels. Provide keyboard-first flow: focus search, arrow navigation, enter-to-play, add to queue, explicit playlist creation, transport. Ensure shortcuts do not interfere with text input. Present precise software volume and failure/retry affordances. Do not add autoplay at startup, implicit scans, image assets, third-party UI or codec dependencies. Preserve Pip-Boy 1024-byte micro panel contract.

## Pass 4 — paired A/B engineering acceptance

Use A = original baseline pinned SHA and B = candidate pinned SHA, same file hashes/device, same screen size and OS, order alternated where practicable; at least three repeats for functional/hardware cases and five for UX/perf pilot. Report raw observations, ranges and medians. Do **not** label small engineering samples as statistically significant.

| Test | A and B | Success evidence |
| --- | --- | --- |
| AB-01 | Rust CLI vs Swift app | Both audible for identical non-silent WAV/FLAC and native playback logs |
| AB-02 | Mounted SMB vs APFS copy | Matching SHA, equivalent decode/output, report latency/errors |
| AB-03 | PCM WAV vs matched FLAC | Equal decoded PCM and audible result |
| AB-04 | Internal speakers vs selected headphones | Actual route and audible confirmation |
| AB-05 | 100% vs 25% software volume | Audible/loopback level difference and no unexpected silence |
| AB-06 | Stop/Play vs Pause/Resume | Correct state/focus and counters stable while paused |
| AB-07 | Launch without library vs explicit load/play | No spontaneous scans or output |
| AB-08 | Old UI vs new UI on same tasks | Correct task completion, fewer actions/errors, accessible navigation |
| AB-09 | 2k vs 20k tagged catalogs | Scan, search, memory and scrolling measurements |
| AB-10 | Natural EOF vs injected and real device failure | Accurate terminal state; explicit recoverable retry |

**Usability tasks:** find track by artist/title, start/stop, inspect progress, create/edit named playlist, add/reorder queued songs, resume after relaunch, keyboard-only and VoiceOver path. Track task time, errors, panel switches, and focus traps. Never report output audible unless human operator/hearing observer or documented loopback verifies it.

## Pass 5 — fail-closed production gate

At each pass run focused regressions and update evidence. Before declaring candidate ready:
```sh
python3 scripts/check_literature.py
python3 scripts/check_roadmap_index.py
cargo test --locked --offline --workspace
sh scripts/offline-gate.sh
sh scripts/build-macos-app.sh
codesign --verify --deep --strict --verbose=2 target/macos/RustMusic.app
open -n "$PWD/target/macos/RustMusic.app"
```
Document exact SHA, executed commands and exit codes. Hardware not available => mark hardware validation BLOCKED, never PASS. If source fails, fix and rerun tests before any merge. Keep all historical audit and private corpus records intact. Do not expand work into sibling repositories, change release policy or package third-party libraries.

## Deliverables

1. Source changes (audio fix, typed IPC snapshot, progress, UI layout) in reviewable commits.
2. New regression tests and a privacy-safe diagnostic capability.
3. `docs/evidence/MAC06_AB_RESULTS.md` with rows for each condition, test asset IDs/hashes (no paths), candidate SHA, commands and observed pass/fail/blocked.
4. Updated macOS acceptance ledger, roadmap and handoff **only with demonstrated evidence**.
5. Operator handoff: exact launch command and any manual hardware steps still required.

**Stop conditions:** private fixture unavailable, unverified physical output, unexpected new external dependency, unsafe path, changed RBPK protocol without compatibility tests, failed offline gate, or inaccessible macOS toolchain. If blocked, finish safe independent work and report the next precise operator action. Do not fabricate completion.
