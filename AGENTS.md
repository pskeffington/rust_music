# Rust Music — Codex repository instructions

Rust Music is an independent, ultra-lightweight **lossless-only** audio module for Rust_Boy and standalone offline systems. Follow the repository, not assumptions about other Rust_Boy modules.

## Non-negotiable constraints

- Every product decoder, output adapter and crate is authored here. Cargo dependencies must remain workspace-local `path` edges. No crates.io/Git packages, build.rs downloads, FFmpeg, libFLAC, external ALAC libraries, GStreamer, CPAL, Rodio, system decoder fallback, runtime network calls or telemetry. The OS's built-in audio/device ABI and locally installed Rust toolchain are explicit prerequisites.
- Source input must be genuinely **lossless** at the codec level. Playback floor: integer PCM **16 bits / 44,100 Hz**. Reject AAC/MP3/Opus/Vorbis/lossy WavPack and unsupported codecs; never upscale low-quality audio to claim better fidelity. Parser-only lower-quality test fixtures are allowed but not playback.
- Exact integer PCM decode parity is not hardware bit-perfect certification. Do not claim tested, complete, merged, lossless mastering provenance or DAC fidelity without observed evidence.
- No user FLAC/other copyrighted music, account credentials, private absolute paths, reference-code copies, or build artifacts in Git.
- Rust_Boy controls the single RAII audio-focus lease shared with Radio/TV; no autoplay, surprise file scans, independent audio focus, or audio-callback I/O. Keep 128×64 panel output at 1024 bytes.

## Routing and engineering passes

- For a roadmap pass read `docs/roadmap-index.json` and choose the highest-priority unblocked `RM-###` task requested. `docs/CODEX_AGENT_MAP.md` maps tasks to project-scoped `.codex/agents/*.toml` specialists. Assign **one writer per overlapping file scope**. Delegate read-only discovery/review in parallel where helpful; shared files require serial integration.
- Read only task-relevant docs: `docs/ROADMAP.md` for phases, `docs/DEPENDENCY_POLICY.md` for dependency changes, `docs/G10_AUDIOPHILE_FORMATS.md` for format admission, `docs/HOST_CONTRACT.md` for embedding, and G2/G7/G8/G9/G10 evidence ledgers for corresponding tests.
- Before edits, identify current branch, dirty changes, task ID, exact scope, test expectations and acceptance gate. Respect uncommitted user work; no blind reset/clean/force push or retargeting/merging the draft PR.
- Execute the smallest reviewable change, add adversarial regression fixtures, run available tests, and report observed commands/exit codes. `sh scripts/offline-gate.sh` is the Rust-only no-download quality gate. Optional `python3 scripts/check_roadmap_index.py` validates navigation (no product Python requirement).
- When a toolchain/hardware/private corpus is unavailable, mark the task **blocked or unverified**, with explicit required operator command. Source implemented ≠ compiled ≠ PCM parity ≠ measured DAC.
- Do not advance a dependent gate while its prerequisite evidence is still blocked. Do not automatically create schedules, install dependencies, change sibling repositories, push/merge branches, or configure hardware outside the user-authorized task.

## Historical checkpoint (2026-10-29; superseded below)

At the time of this checkpoint, the branch was `build/native-codecs-g0`, draft PR #1. Operator-reported AIFF/AIFF-C native CLI golden PCM parity was 7/7. The operator audit passed for 12 local crates/21 local edges, but the full offline gate stopped at rustfmt. The 14-file rustfmt diff and three audit warnings were subsequently repaired in commits ending `13f8e85975f7b4cae921593df2ab0e7f310b8d30`. The later complete offline-gate result is recorded below and supersedes the gate status in this historical checkpoint.

## Latest verified checkpoint (2026-10-29)

Branch `build/native-codecs-g0`; the latest verified checkpoint includes
RM-017 M4A ingestion/corpus work, RM-011 bounded local tags, RM-012
synthetic fixed/variable-block FLAC seek, RM-013 bounded RF64/ds64 parsing
with synthetic >4 GiB streaming coverage, and the RM-014 WavPack framing
foundation (not a decoder). Rust/Cargo
1.99.1/macOS:
`sh scripts/offline-gate.sh` exited 0 on 16 local crates and 29 path-only
edges, with zero registry/Git crates and zero third-party system links. Fmt,
warnings-denied Clippy, workspace tests, release build and macOS feature check
passed. The optional `python3 scripts/test_caf_reference.py` exited 0 on five
FFmpeg-generated exact-PCM vectors (CAF LPCM 16/24/32 and ALAC 16/24); this
does not make FFmpeg a runtime dependency. `scripts/test_m4a_corpus.py` also
passes six Archive.org ALAC M4As with exact five 16/44.1 and one 24/48 PCM
parity and confirms M4A catalog ingestion/AAC refusal. Bounded FLAC Vorbis and
AIFF text tags pass deterministic size/UTF-8 tests, and a 2 MiB artwork value
is skipped without being retained or decoded. Roadmap/literature validators and
`git diff --check` pass. Generated FLAC/WAVE/AIFF parity and six private FLAC
reference/native parity remain recorded in their ledgers; private audio stays
outside Git. Rust Music keeps a distinct MUSIC module boundary, but actual
Rust_Boy/Pip-Boy shared focus and input routing, physical panel/output and
device acceptance remain unverified. Keep PR #1 draft and do not modify sibling
repositories under the current goal. RM-012 host seek routing, real-world
seektable compatibility, Rust_Boy shared focus and physical panel/output remain
unverified pending external acceptance evidence.
