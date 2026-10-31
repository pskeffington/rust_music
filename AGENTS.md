# Rust Music — Codex repository instructions

Rust Music is an independent, ultra-lightweight **lossless-only** audio module for Rust_Boy and standalone offline systems. Follow the repository, not assumptions about other Rust_Boy modules.

## Non-negotiable constraints

- Every product decoder, output adapter and crate is authored here. Cargo dependencies must remain workspace-local `path` edges. No crates.io/Git packages, build.rs downloads, FFmpeg, libFLAC, external ALAC libraries, GStreamer, CPAL, Rodio, system decoder fallback, runtime network calls or telemetry. The OS's built-in audio/device ABI and locally installed Rust toolchain are explicit prerequisites.
- Source input must be genuinely **lossless** at the codec level. Playback floor: integer PCM **16 bits / 44,100 Hz**. Reject AAC/MP3/Opus/Vorbis/lossy WavPack and unsupported codecs; never upscale low-quality audio to claim better fidelity. Parser-only lower-quality test fixtures are allowed but not playback.
- Exact integer PCM decode parity is not hardware bit-perfect certification. Do not claim tested, complete, merged, lossless mastering provenance or DAC fidelity without observed evidence.
- No user FLAC/other copyrighted music, account credentials, private absolute paths, reference-code copies, or build artifacts in Git.
- Rust_Boy controls the single RAII audio-focus lease shared with Radio/TV; no autoplay, surprise file scans, independent audio focus, or audio-callback I/O. Keep 128×64 panel output at 1024 bytes.

## Text-only micro library (new directive)

- For user-facing library, metadata, playlist and queue development, read [docs/MICRO_LIBRARY_ZERO_ARTWORK.md](docs/MICRO_LIBRARY_ZERO_ARTWORK.md). **Absolutely no cover art, embedded picture extraction, image decoders/caches/assets, visualizers, or artwork placeholders.** Maintain text-only UX and the current fixed 1,024-byte monochrome panel.
- Preserve any unmerged local WavPack, IPC, packaging and app work before editing shared files. The media-library directive is a plan, not proof of implementation.

## Routing and engineering passes

- For a roadmap pass read `docs/roadmap-index.json` and choose the highest-priority unblocked `RM-###` task requested. `docs/CODEX_AGENT_MAP.md` maps tasks to project-scoped `.codex/agents/*.toml` specialists. Assign **one writer per overlapping file scope**. Delegate read-only discovery/review in parallel where helpful; shared files require serial integration.
- Read only task-relevant docs: `docs/ROADMAP.md` for phases, `docs/DEPENDENCY_POLICY.md` for dependency changes, `docs/G10_AUDIOPHILE_FORMATS.md` for format admission, `docs/HOST_CONTRACT.md` for embedding, and G2/G7/G8/G9/G10 evidence ledgers for corresponding tests.
- Before edits, identify current branch, dirty changes, task ID, exact scope, test expectations and acceptance gate. Respect uncommitted user work; no blind reset/clean/force push or retargeting/merging the draft PR.
- Execute the smallest reviewable change, add adversarial regression fixtures, run available tests, and report observed commands/exit codes. `sh scripts/offline-gate.sh` is the Rust-only no-download quality gate. Optional `python3 scripts/check_roadmap_index.py` validates navigation (no product Python requirement).
- When a toolchain/hardware/private corpus is unavailable, mark the task **blocked or unverified**, with explicit required operator command. Source implemented ≠ compiled ≠ PCM parity ≠ measured DAC.
- Do not advance a dependent gate while its prerequisite evidence is still blocked. Do not automatically create schedules, install dependencies, change sibling repositories, push/merge branches, or configure hardware outside the user-authorized task.

## Merge-preparation and verification

- Before any merge into `main`, read [PR #1 merge-readiness](docs/PR1_MERGE_READINESS.md). On the current clean PR branch run `PRIVATE_FLAC_DIRECTORY=/private/corpus sh scripts/premerge-check.sh` with an operator-provisioned toolchain; do not upload private logs/media. The checker is fail-closed, performs no network/downloads, and does **not** merge.
- GitHub `mergeable=true` confirms ancestry/conflict resolution only. Do not mark RM-001 complete, close issue #2, clear downstream blocks, or certify hardware from a static audit. Keep draft PR #1 unmerged until required acceptance evidence is recorded; never move `main` by force.

## RM-001 evidence capture

- For a local operator gate, follow [docs/RM001_OPERATOR_GATE.md](docs/RM001_OPERATOR_GATE.md) and run `sh scripts/capture_offline_gate.sh` from the clean active PR branch. The wrapper records the real gate exit status and private transcript outside the repository; it does not claim success or change roadmap states. Do not upload private transcripts without review.

## Current handoff and branch sanity

- Before any engineering pass, read [docs/ENGINEERING_HANDOFF_2026-10-30.md](docs/ENGINEERING_HANDOFF_2026-10-30.md). The live implementation is on draft PR #1 branch `build/native-codecs-g0`; `main` was README/LICENSE-only at the October 30 inspection. Never infer missing implementation from an accidental `main` checkout.
- Keep `RM-001` first until an actual complete offline gate pass is observed. Review-only/index work can continue independently; never mark dependent native PCM, host, or hardware tasks complete from source or operator reference tests.

## Recorded checkpoint (2026-10-29)

Branch `build/native-codecs-g0`, draft PR #1. Operator reported 7/7 AIFF/AIFF-C native CLI golden PCM cases PASS. Operator audit passed for 12 local crates/21 local edges; full offline gate stopped at rustfmt. The 14-file rustfmt diff and three audit warnings were repaired in commits ending `13f8e85975f7b4cae921593df2ab0e7f310b8d30`, but a **fresh complete offline gate** has not been reported. Native private FLAC corpus and physical DAC/host-focus acceptance remain open. Validate live status before repeating these claims.
