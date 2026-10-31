# PR #1 merge-readiness and evidence gate — 2026-10-31

**Current decision: not yet release-merge eligible.** [PR #1](https://github.com/pskeffington/rust_music/pull/1) is technically conflict-free, but compilation and several acceptance gates have not passed with current evidence. A GitHub `mergeable=true` value is about branch conflict resolution, not quality assurance.

## Local post-commit verification — 2026-10-31

Local commit `0bc3f1673a635e688b928406c63baa46bbbb22da` is clean, includes the
RM-019 child-lifecycle changes, and has local `origin/main` as an ancestor. The
operator-provisioned Rust/Cargo 1.99.1 toolchain completed
`PRIVATE_FLAC_DIRECTORY=<private six-file directory> RUST_MUSIC_EVIDENCE_DIR=/private/tmp/rust-music-gate-evidence sh scripts/premerge-check.sh`:
roadmap/literature checks and eight roadmap unit tests passed, the complete
offline Rust gate exited 0, WAVE passed 6/6, AIFF/AIFF-C passed 7/7, and the
six exact manifest FLACs (L1/L2/L3/G4/G5/G6) passed native PCM/metadata/hash
parity. The checker printed its final automated-check PASS. Private files and
captured logs remain outside Git.

This completes the scripted local premerge checks, but does not satisfy the
remaining host shared-focus, physical panel/DAC, native device failure and
recovery, LaunchServices installation, or release approval requirements below.
At this checkpoint local `HEAD` is one commit ahead of the GitHub PR head; keep
PR #1 draft and do not push or merge until all release gates and required
approval are recorded.

## GitHub evidence verified on 2026-10-31

- `build/native-codecs-g0` incorporated `main` in a real, non-force two-parent merge `ed241ac60ddd9b3a2f22ebc71bc557597d505d5a`. GitHub compare reported **74 ahead, 0 behind** before the subsequent premerge-script commit.
- PR #1 is open and draft. No required status checks or review submissions are recorded on the audited head.
- The latest head has **zero successful check runs**. All six historical Actions attempts ended in `startup_failure` and scheduled no jobs; they cannot be counted as test results.
- GitHub's current workflow is operator-dispatched, bound to a preprovisioned offline self-hosted runner. This environment has no local `cargo` or `rustc` and could not execute the Rust compiler.
- [Issue #2](https://github.com/pskeffington/rust_music/issues/2) remains open for `RM-001`. Do not close it from static inspection.

## Read-only static audit in this pass

The repository's workspace manifests and machine-readable registries were fetched and checked: **12 workspace members; 12 matching lockfile packages; 21 dependency edges, all local `path`; 15 roadmap gates; 16 task IDs; 6 Codex agents; and matching 16-bit/44.1 kHz playback-floor records**. No violations were observed in this specific static comparison. Fifteen Rust source files were inspected (51 `#[test]` markers); no executable network client reference was observed. This is **not** a rustfmt/Clippy, Rust compilation, native PCM parity or hardware verification.

## One-command operator acceptance

Run from a clean local checkout on the feature branch, with the exact current PR head and an offline Rust toolchain already installed:

```sh
git fetch origin
git switch build/native-codecs-g0
git pull --ff-only
PRIVATE_FLAC_DIRECTORY="/path/to/private/six-flac-tracks" sh scripts/premerge-check.sh
```

The new [premerge checker](../scripts/premerge-check.sh) is entirely offline once the checkout is prepared. It refuses a dirty or out-of-sync checkout, validates the roadmap and literature indexes, runs the real `capture_offline_gate.sh` Rust build/test/Clippy/fmt/release gate, then invokes native WAVE/AIFF and private six-track Rust FLAC differential tests. No private audio or logs are committed or uploaded, and it never executes `git fetch`, installs anything, edits branches or merges. Its evidence wrapper writes private transcript and summary **outside Git**. Do not paste the full private transcript into a public issue.

**Interpretation:** An exit-zero result qualifies only the scripted gates. It does not establish real shared Music/Radio/TV focus, 128×64 physical screen input/output, negotiated DAC samples or bit-perfect hardware. For those, follow `RM-004` through `RM-007` and record named hardware evidence without sensitive metadata.

## Merge/cleanup instructions after evidence

1. Review the generated private evidence, record the exact HEAD/commands/exit codes and reconcile task statuses in `docs/roadmap-index.json`, `docs/ROADMAP.md`, and Issue #2 without guessing.
2. Verify native private FLAC PCM parity and WAVE/AIFF tests **independently** of the static manifest audit and `cargo test`.
3. Complete Rust_Boy shared-focus, physical DAC, panel and error-path acceptance for the release target or explicitly narrow any supported platform scope; never claim bit-perfect without measurement.
4. Reconfirm `main` is an ancestor of the PR head and GitHub's head SHA has not moved. Then mark PR ready, obtain the authorized merge, use a non-force GitHub merge with expected head SHA, and verify `main` contains the result.
5. Only after verifying that the branch's work is present on `main`, assess whether the old branch may be deleted. Never force-delete the only implementation branch or close an incomplete issue.
6. Update root `AGENTS.md`, README, roadmap metadata and PR references after merge so Codex does not check out an obsolete branch; no production tags/releases without independent release acceptance.

**Policy:** The user's request to work toward merge authorizes the reconciliation and preparation work. It does not convert an unexecuted gate into a PASS. See [production playbook](CODEX_PRODUCTION_PLAYBOOK.md), [RM-001 operator gate](RM001_OPERATOR_GATE.md), and [roadmap](ROADMAP.md).
