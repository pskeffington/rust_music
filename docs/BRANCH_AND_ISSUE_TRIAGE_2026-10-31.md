# Branch and issue reconciliation — 2026-10-31

## Verified remote snapshot

- Default branch: `main` at `c02ef6c9b86cff3a86ceaaf5e35900593f592fbf` before this pass. Its only post-fork change was a four-line README extension.
- Active source branch: `build/native-codecs-g0` at `056a7e19b6c7f95d1b4ef56e058c6b76265625e9` before this pass; 70 commits ahead and 1 behind main, sharing merge base `305ec0f273765fbcff458d46780cb34bc87d42bb`.
- One open draft implementation PR: [#1](https://github.com/pskeffington/rust_music/pull/1). GitHub API reported `mergeable_state=dirty` and `mergeable=false` prior to reconciliation.
- No preexisting stand-alone GitHub issues; [#2](https://github.com/pskeffington/rust_music/issues/2) tracks the first observable unblocker (RM-001).
- Six historical Actions workflow attempts reported `startup_failure`; no job ran. The current workflow is manual/offline-self-hosted only, not a green PR check.

## Cleanup policy

1. Preserve the single implementation branch and PR #1; do not delete it or squash 70 unreviewed commits into main.
2. Integrate main's README-only commit into the feature ancestry via a real two-parent merge with the feature tree as baseline, and explicitly preserve the README sentences describing a lightweight, locally carried or disjointed deployment. Avoid rebasing or forcing branch refs.
3. Keep PR #1 **draft and unmerged**. Resolving Git ancestry and textual conflicts is not decoder/host/hardware acceptance.
4. Do not delete the only active feature branch. After PR #1 is validated and merged, assess branch deletion from verified ancestry and remaining work; avoid force deletes.
5. Use one gate-specific issue rather than duplicating the entire roadmap as issues. Each closed issue requires its acceptance proof; no stand-alone old issues were available to close.
6. Stop before touching `main` source files, release tags, production deployments or private audio.

## Blocking verification queue

| Order | Work | Acceptance evidence |
| --- | --- | --- |
| P0 | [Issue #2](https://github.com/pskeffington/rust_music/issues/2), RM-001 / G10 | Fresh exit-zero full offline Rust gate on a recorded HEAD; evidence kept outside Git |
| P0 | RM-002 / G8 | All six private FLACs verified through **our** decoder against independent PCM; never commit media |
| P0 | RM-003 / G7 | Native WAVE reference PCM checks and malformed input rejection |
| P0 | RM-004 / G10 | Lossless floor, unsupported codec and Rust_Boy audio-focus fail-closed tests |
| P1 | RM-005–RM-007 | Physical DAC, Music/Radio/TV focus interoperability and 1 KiB display acceptance |

The already-reported AIFF/AIFF-C 7/7 result is operator-provided reference evidence, not a new workspace test. This audit environment has no Cargo/rustc. The branch reconciliation must not change machine-roadmap gate states; see [roadmap-index.json](roadmap-index.json), [ROADMAP.md](ROADMAP.md), [operator capture](RM001_OPERATOR_GATE.md), and [handoff](ENGINEERING_HANDOFF_2026-10-30.md).

## Agent instruction

On each continuation, compare `main...build/native-codecs-g0`, review PR #1 mergeability, track [#2](https://github.com/pskeffington/rust_music/issues/2), and dispatch only the highest-priority unblocked RM task. Record exact exit codes. If a new issue clearly duplicates #2, consolidate with a link and close as duplicate **only** after reviewing its acceptance scope. Never auto-close incomplete work.
