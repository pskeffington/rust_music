# Rust Music — Codex repository instructions

Rust Music is an independent, ultra-lightweight **lossless-only** audio module for Rust_Boy and standalone offline systems. Follow the repository, not assumptions about other Rust_Boy modules.

## FIRST: agent_skills orchestration

Before any mutable development work, load the verified sibling [`agent_skills`](https://github.com/pskeffington/agent_skills) hierarchy, following [docs/CODEX_AGENT_SKILLS_FIRST_PRODUCTION.md](docs/CODEX_AGENT_SKILLS_FIRST_PRODUCTION.md). First perform a read-only Git safety inspection; then pin the actual agent_skills development SHA and read applicable SKILL.md role contracts. Do not assume agent_skills/main contains the hierarchy. Only after routing the agents should Codex resolve the checkout, implement, test and pursue an evidence-backed mainline merge. If unavailable, report the limitation and preserve this repository's existing agents. Never treat a skill as evidence of a passed test.

## Current Codex product directive — library import, autoplay and EQ

Read [docs/CODEX_WINAMP_LIBRARY_AUTOPLAY_EQ.md](docs/CODEX_WINAMP_LIBRARY_AUTOPLAY_EQ.md) after the pinned `agent_skills` role contracts, and before modifying the library, player or UI. Product policy update: optional autoplay **only after a user explicitly loads a library**, with persistent opt-in and default off; never on app startup or background rescan, and never without host audio-focus grant. Implement an internally authored bypassable 10-band EQ with proof of unchanged PCM in bypass; EQ-active audio is processed, not bit-perfect. **No image/artwork/visualizer implementation.** Existing blanket no-autoplay provisions mean no *unrequested* autoplay; reconcile those texts on the current local branch. This remote documentation may lag local unpushed code; preserve that code and integrate non-destructively.

## Non-negotiable constraints

- Every product decoder, output adapter and crate is authored here. Cargo dependencies must remain workspace-local `path` edges. No crates.io/Git packages, build.rs downloads, FFmpeg, libFLAC, external ALAC libraries, GStreamer, CPAL, Rodio, system decoder fallback, runtime network calls or telemetry. The OS's built-in audio/device ABI and locally installed Rust toolchain are explicit prerequisites.
- Source input must be genuinely **lossless** at the codec level. Playback floor: integer PCM **16 bits / 44,100 Hz**. Reject AAC/MP3/Opus/Vorbis/lossy WavPack and unsupported codecs; never upscale low-quality audio to claim better fidelity. Parser-only lower-quality test fixtures are allowed but not playback.
- Exact integer PCM decode parity is not hardware bit-perfect certification. Do not claim tested, complete, merged, lossless mastering provenance or DAC fidelity without observed evidence.
- No user FLAC/other copyrighted music, account credentials, private absolute paths, reference-code copies, or build artifacts in Git.
- Rust_Boy controls the single RAII audio-focus lease shared with Radio/TV; no unrequested autoplay, surprise file scans, independent audio focus, or audio-callback I/O. Optional autoplay is allowed only after an explicit user library load, persistent opt-in, and successful host focus grant; it defaults off and never runs on startup, refresh, or reconnect. Keep 128×64 panel output at 1024 bytes.

## Production tasking

- Load the verified pinned `agent_skills` roles before mutable work, then follow [the production continuation](docs/CODEX_AGENT_SKILLS_FIRST_PRODUCTION.md) and [library-load/autoplay/EQ gates](docs/CODEX_WINAMP_LIBRARY_AUTOPLAY_EQ.md).
- Preserve local code, branches, backups, and stashes while reconciling remote docs. These directives are plans, not implementation or release evidence.
- Keep the zero-artwork policy absolute: no artwork extraction, images, image caches, placeholders, visualizers, or image dependencies.

## Text-only micro library (new directive)

- For user-facing library, metadata, playlist and queue development, read [docs/MICRO_LIBRARY_ZERO_ARTWORK.md](docs/MICRO_LIBRARY_ZERO_ARTWORK.md). **Absolutely no cover art, embedded picture extraction, image decoders/caches/assets, visualizers, or artwork placeholders.** Maintain text-only UX and the current fixed 1,024-byte monochrome panel.
- Preserve any unmerged local WavPack, IPC, packaging and app work before editing shared files. The media-library directive is a plan, not proof of implementation.

## Routing and engineering passes

- Follow the pinned framework reference and Rust Music role mapping in [docs/AGENT_SKILLS_INTEGRATION.md](docs/AGENT_SKILLS_INTEGRATION.md); its skills are guidance, not a permission boundary or release approval.
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

## Historical checkpoint (2026-10-29; superseded below)

At the time of this checkpoint, the branch was `build/native-codecs-g0`, draft PR #1. Operator-reported AIFF/AIFF-C native CLI golden PCM parity was 7/7. The operator audit passed for 12 local crates/21 local edges, but the full offline gate stopped at rustfmt. The 14-file rustfmt diff and three audit warnings were subsequently repaired in commits ending `13f8e85975f7b4cae921593df2ab0e7f310b8d30`. The later complete offline-gate result is recorded below and supersedes the gate status in this historical checkpoint.

## Latest verified checkpoint (2026-10-30)

Branch `build/native-codecs-g0`; preserve the extensive pre-existing dirty
worktree. Rust/Cargo 1.99.1/macOS:
`sh scripts/offline-gate.sh` exited 0 on 17 local crates and 32 path-only
internal edges, with zero registry/Git crates and zero third-party system
links. Fmt, warnings-denied Clippy, workspace tests, release build, and macOS
feature checks passed. Roadmap and literature validators plus `git diff
--check` passed.

RM-014 WavPack remains unregistered and unadmitted. Its crate suite passes
48/48. Exact PCM parity includes FFmpeg-generated profiles, Tiny Encoder 4.40
fast-mode packed-16/24 vectors, and two-block default/high-quality 20,000-frame
packed-16 stereo vectors. The high-mode vector covers terms
`[18,18,18,-2,2,3,5,-1,17,4]`; only terms -2, -1, 2–5, 17 and 18 are currently
admitted. Tiny Encoder 4.40 also matches a 1,063,147-frame packed-24 stream
derived from an uploader-labeled CC0 32-bit recording; parity is against the
derived 24-bit PCM, not the source FLAC. Real-world native WavPack files, other
terms/profiles, trailing tags, hybrid correction, and playback admission
remain open.

RM-018 has a bounded RBPK v1 unit contract and endpoint, spawned-process tests,
and a local release-binary Hello/Activate/Shutdown smoke. The tested digest is
not allowlisted by Pip-Boy; live mounting, host-supervised release
interoperability, shared audio contention, and platform acceptance remain
unverified. The Pip-Boy reference MUSIC identity is distinct, while MUSIC/TV
integration, user input, and physical panel/audio tests remain open. Keep PR #1
draft; preserve sibling work and do not push or merge. RM-005/007/015 require
operator hardware acceptance; RM-006/018 still need host evidence. Private FLAC
corpus audio remains outside Git.
