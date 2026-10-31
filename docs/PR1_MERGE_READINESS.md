# PR #1 merge-readiness and evidence gate — 2026-10-31

**Current decision: scripted local checks pass; PR #1 is not release-merge eligible.** The private FLAC, WAVE, AIFF and offline Rust gates pass locally. Rust_Boy live MUSIC/RADIO/TV acceptance, device disconnect/recovery, physical panel/DAC acceptance, distribution signing and detached release approval remain open. A GitHub `mergeable=true` value is about branch conflict resolution, not quality assurance.

## Text library foundation and cancellable scans — 2026-10-31

At source commit `e5d6b34`, named playlists use bounded, versioned `.rmp`
storage with safe relative paths and atomic replacement, and explicit scans
can be cancelled without returning a partial catalog. The library suite passed
27/27; `python3 scripts/check_roadmap_index.py` and
`python3 scripts/check_literature.py` exited 0. A fresh
`sh scripts/offline-gate.sh` at `e5d6b34` exited 0 across 17 local crates,
32 path-only edges, zero registry/Git crates, workspace tests, release build,
and macOS feature checks. The first offline-gate attempt after adding the new
cancellation error exited 101 because the CLI's exhaustive error mapping lacked
a cancellation arm; that arm was added and the committed rerun passed. Private
codec parity evidence was not rerun because this change does not touch decoder
or PCM paths. These APIs are not yet wired to the standalone UI or host queue.
`sh scripts/build-macos-app.sh` also exited 0 at this source commit; the local
bundle's ad-hoc signature passed strict `codesign` verification. Developer ID,
notarization, and distribution signing remain unverified.

At app commit `8f0d64d`, the remaining SF Symbol widgets were removed from the
macOS shell to keep the current interface text-only. The artwork/image search
over the app, IPC and panel source returned no matches; the macOS bundle build
and strict local ad-hoc signature verification exited 0. The shell remains a
single-file Play/Stop inspector and does not yet implement the library UX.

## 2026-10-31 catalog and package hardening pass

At this recording, the latest implementation snapshot on
`build/native-codecs-g0` is `84f2096` (this subsequent commit only refreshes
this document). Local `origin/main`
(`c02ef6c9b86cff3a86ceaaf5e35900593f592fbf`) is an ancestor, and that
snapshot is 58 commits ahead of local
`origin/build/native-codecs-g0` tracking ref (`4e3d24a8fe6bea68b4cd40736ddd4b7db6368011`).
That tracking ref is known to be stale; this is not a fresh GitHub comparison
and is not authorization to push. Recent commits add bounded text search,
deterministic sort/pagination, root-scoped IDs, a bounded non-persistent M3U8
model, a versioned Unix-persistent queue/repeat/shuffle model, a synthetic
0/1/100/2k/20k catalog profile, and simulated AudioQueue failure/teardown
handling. The full `sh scripts/offline-gate.sh` exited 0 at the queue/native
implementation snapshot `845b32c`; the profile example was separately built,
run and checked with warnings-denied Clippy. The app bundle build, plist lint
and strict signature verification exited 0 at `845b32c`; private corpus parity
was not rerun. Audible Rust Music host contention, live TV, physical device
disconnect/recovery, panel/DAC, distribution signing and release approval
still block merge.

A separate Pip-Boy commit `bfbb9c76d147911feb6d359ba0f2342a8a0cd97e` adds
and passes a 1/1 real Rust Music child test for MUSIC/RADIO broker handoff:
MUSIC excludes RADIO, suspend releases focus, RADIO acquires, resume
reacquires MUSIC, and shutdown releases. This is host broker lifecycle
evidence, not audible playback or live TV contention. The change remains on
its separate clean branch and is unpushed; it is not in Rust Music history.

## Local post-commit verification — 2026-10-31

Local commit `0bc3f1673a635e688b928406c63baa46bbbb22da` is clean, includes the
RM-019 child-lifecycle changes, and has local `origin/main` as an ancestor. The
operator-provisioned Rust/Cargo 1.99.1 toolchain completed
`PRIVATE_FLAC_DIRECTORY=<private six-file directory> RUST_MUSIC_EVIDENCE_DIR=<private-output-directory> sh scripts/premerge-check.sh`:
roadmap/literature checks and eight roadmap unit tests passed, the complete
offline Rust gate exited 0, WAVE passed 6/6, AIFF/AIFF-C passed 7/7, and the
six exact manifest FLACs (L1/L2/L3/G4/G5/G6) passed native PCM/metadata/hash
parity. The checker printed its final automated-check PASS. Private files and
captured logs remain outside Git.

This completes the scripted local premerge checks at that checkpoint, but does
not satisfy the remaining host shared-focus, physical panel/DAC, native device
failure and recovery, or release approval requirements below. PR #1 remained
draft and unpushed.

## Standalone package and LaunchServices — 2026-10-31

At clean local commit `bbd5dc3c52d3e187a1a332010f07d2a72906766c`,
`sh scripts/build-macos-app.sh` exited 0. `plutil -lint` and
`codesign --verify --deep --strict` passed; the app's `LC_BUILD_VERSION`
minimum is macOS 13.0, matching `LSMinimumSystemVersion`. After unregistering
the generated app (`lsregister -u`, exit 0), unsandboxed `open -n` by path
auto-registered and launched it (exit 0); unified logs contain
LaunchServices CHECKIN for `dev.rustmusic.standalone`. Rebuilt-app UI checks
inspected and played a generated 16-bit/44.1 kHz WAV, stopped playback, then
closed the window during playback; Activity Monitor showed no
`rust-music-cli` process afterward. The sandboxed `open` error and `lsregister`
`-10822` were sandbox service-access failures. The local ad-hoc signature is
not accepted by `spctl`; Developer ID signing/notarization and distribution
acceptance remain unverified. This closes local RM-019 launch acceptance, not
the independent hardware gates.

## GitHub evidence verified on 2026-10-31

- `build/native-codecs-g0` incorporated `main` in a real, non-force two-parent merge `ed241ac60ddd9b3a2f22ebc71bc557597d505d5a`. GitHub compare reported **74 ahead, 0 behind** before the subsequent premerge-script commit.
- PR #1 is open and draft. No required status checks or review submissions are recorded on the audited head.
- The remote PR head remains `7dc75e13ab242d6ac7353139f0955ccce96b10cf`; the local development branch is at `bbd5dc3c52d3e187a1a332010f07d2a72906766c`. Its PR-hosted status checks are empty; local test evidence is not GitHub CI evidence.
- Local Rust/Cargo 1.99.1 is available, and the complete offline and corpus premerge gate passed at local commit `8a057792586ba050521e6989137db4492f1ffb27`. Later local edits are limited to the macOS app build/signing script and documentation; the macOS app build itself passed on `bbd5dc3`.
- The local remote-tracking ref is stale. GitHub PR #1 has six remote-only commits since common base `4e3d24a`; local has 47 commits from that base. Integrate both histories without force before any eventual authorized push.
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
