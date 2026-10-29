# Codex production execution contract — Rust Music

This document supplements [../AGENTS.md](../AGENTS.md), [ROADMAP.md](ROADMAP.md), [roadmap-index.json](roadmap-index.json), and [CODEX_AGENT_MAP.md](CODEX_AGENT_MAP.md). The machine-readable registry owns task IDs, gate dependencies, and current evidence status; this contract defines the repeatable operating procedure. Do not silently change those statuses.

## Mission and non-negotiables

Deliver a small, offline, locally authored, lossless-only Rust audio player usable standalone and as a Rust_Boy pop-in module. Enforce the current >=16-bit integer PCM / >=44,100 Hz playback policy before opening the sink or acquiring host audio focus. Parser-only lower-fidelity fixtures are not playable. No downloaded libraries, external Cargo registry/Git edges, codec frameworks, system decoder shortcuts, silent audio resampling, network/telemetry, or sibling-repo modifications. OS audio device ABIs and the locally installed Rust toolchain are permitted platform prerequisites. No private user media, credentials, or raw absolute file paths in commits or logs.

## Starting any Codex session

1. Inspect the actual branch, HEAD, dirty files, draft PR and recent commits. Do not reset, clean, force-push or overwrite operator changes.
2. Read root AGENTS.md, docs/roadmap-index.json, docs/ROADMAP.md, docs/CODEX_AGENT_MAP.md, and relevant target contracts. Select the highest-priority **unblocked** task in scope; if explicitly asked for another task, check its prerequisites before touching code.
3. Record the task ID, target gate, precise files, acceptance command, expected failure modes, local environment and permitted write scope.
4. Assign one writing agent per file scope; safety_reviewer and gate_verifier are advisory/read-only by instruction (not an enforced sandbox). Main agent serializes changes to Cargo manifests/lock, facade/core/engine, admission policy, README, and the roadmap index.
5. If blocked by missing cargo, private tracks, CI admission or hardware, do not invent green gates or work around the blocker by adding remote dependencies. Complete safe read-only inspection and produce an exact operator handoff. Do not advance dependent tasks.

## Minimal engineering loop for each task

- **Observe:** reproduce one defect or establish a baseline with an exact command, toolchain version and exit code. Distinguish locally executed evidence from prior operator reports.
- **Specify:** add explicit invariants, bounds and negative tests before (or alongside) the change. Favor limited buffers, checked arithmetic, explicit endianness/channel mapping, deterministic state transitions and fail-closed codec recognition. A file extension or catalog signature is not codec proof.
- **Implement:** make the smallest isolated patch; only own code. Use no unsafe unless unavoidable at the OS ABI, with local safety comments and tests. Do not block the real-time audio callback with I/O, allocation, scans, locking or decoder work.
- **Verify:** run the local offline gate when a Rust toolchain exists. Run task-specific native differential checks for FLAC/WAVE/AIFF and independent reference PCM, corruption/truncation, oversized declarations, channel layouts, end-of-stream and stop/drain. Never treat oracle-only fixture checks as own-decoder parity.
- **Review:** have safety_reviewer inspect changed parsers, focus/error paths, dependency graph and overflow/DoS risks; address findings before claiming task completion.
- **Index:** update the exact task and gate status only with observed evidence, linking the relevant ledger. Maintain cross-links and machine-checked index without inventing timestamps, test counts or devices.
- **Handoff:** provide branch/commit, changed files, commands and actual exit codes, verified properties, unverified properties, blockers, and the next available task. Keep PR #1 draft until all applicable release gates have evidence.

## Command sequence (run from repository root)

```sh
git status --short --branch
git rev-parse HEAD
rustc --version
cargo --version
python3 scripts/check_roadmap_index.py
python3 scripts/check_literature.py
sh scripts/offline-gate.sh
```

The Python index validators are optional developer maintenance helpers (stdlib-only); they are **not** a runtime or Rust offline-gate dependency. The offline gate is authoritative for its own checks only. If cargo is absent, log blocked and exit; never replace compiler/test results with static inspection. Check the exact gate script for the implemented subcommands and never skip its failures. Use explicit offline/locked Cargo settings.

## Acceptance and proof separation

| Evidence tier | Permitted claim | Required proof |
| --- | --- | --- |
| Source | Implemented, not yet validated | Indexed code, tests and review |
| Build | Compiled locally | Actual offline fmt, Clippy, workspace tests and release build with exit 0 |
| Decoder | Native exact PCM parity | Own binary vs independently generated/reference PCM, counts, digest and adversarial tests |
| Host | Rust_Boy integration verified | Single host-owned RAII audio lease across Music/Radio/TV, focus refusal, error/unload and fixed 1024-byte panel |
| Hardware | Measured output behavior | Named platform/device, negotiated sample rate/bit depth/channels, playback/underrun/stop/drain evidence and bit-exact capture if claimed |
| Production | Release eligible | All applicable tiers, reproducible offline packaging and rollback checklist complete |

For all codec/container additions (ALAC/MP4/CAF, RF64, WavPack), keep them disabled in production admission until exact native PCM parity, malformed input rejection, and appropriate host/output validation. Deny AAC/DRM masquerading as M4A; never enable a format simply because parsing succeeds.

## Production release checklist (all required)

1. The release commit is identified; changes are reviewable, license and local-only dependency audit clean, and no user audio/private data included.
2. Entire offline gate passes from a clean, locally provisioned Rust toolchain, with command output and exit code preserved. No remote fetch; release build run for each claimed target.
3. Native lossless decoders pass independent PCM vector checks, adversarial malformed-input checks and full-stream digest/frame-count checks where applicable. Private six-track FLAC parity must be run by an authorized operator without uploading tracks.
4. Audio output is measured on each advertised operating system/device class. If physical loopback is not measured, refrain from bit-perfect claims; if a target is untested, label it experimental/unsupported.
5. Host has exclusive audio-focus ownership, safe cancellation/stop/unload, correct denial before DAC/focus for unsupported input, no background scan/autoplay, and measured micro-panel control behavior.
6. Run a failure/recovery pass: corrupt file, truncated file, wrong extension, no device, device removal, interrupted playback, zero-length file, oversized metadata, denied audio lease, repeated load/unload and loss of focus.
7. Operator confirms distributable artifact can launch offline, validates file permissions and release notes, and records known limitations and a rollback path to previous known-good commit/binary.
8. Only after all applicable evidence is linked in the roadmap and PR should a human explicitly authorize release/merge. Codex must not self-certify hardware, merge, tag a production release or deploy merely because CI is green.

## Current checkpoint

As of 2026-10-29, branch `build/native-codecs-g0`, draft PR #1: the locally authored baseline and gate/index scaffold exist. Operator-reported native AIFF/AIFF-C seven-vector parity is distinct from full offline workspace validation. Rustfmt repairs were committed but fresh full gate completion, own-decoder private FLAC parity, and device/host field acceptance remain unverified. **Start at RM-001** and respect its dependencies; never mark production-ready without new evidence.

## Codex completion report template

```text
TASK: RM-### / G##
BRANCH / HEAD:
FILES MODIFIED:
INVARIANTS + NEGATIVE TESTS:
COMMAND / ENVIRONMENT / EXIT:
SOURCE REVIEW:
BUILD / PCM / HOST / HARDWARE EVIDENCE:
DEPENDENCY + SECURITY REVIEW:
ROADMAP STATUS CHANGES:
BLOCKERS / OPERATOR STEPS:
NEXT UNBLOCKED TASK:
PR STATUS: draft | reviewable | human-authorized release
```
