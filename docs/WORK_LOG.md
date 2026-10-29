# Rust Music work log

This log records observed repository work and the next release gates. The
machine-readable task state is authoritative in [roadmap-index.json](roadmap-index.json).
Private audio paths and recordings are deliberately excluded.

## 2026-10-29 — offline release gate and private FLAC parity

**Branch hygiene:** work stayed on the existing `build/native-codecs-g0`
branch. It was the only local branch; existing remote refs were left intact.
No branch was created or deleted, and no push or merge was performed.

**Verified work:**

- `sh scripts/offline-gate.sh` exited 0 on Rust/Cargo 1.99.1/macOS. The
  dependency audit, formatting, Clippy with warnings denied, workspace tests,
  release build and macOS feature check passed.
- `cargo test --locked --offline -p rust-music-host` exited 0 (5/5 tests),
  including sink-open failure cleanup and host focus lease release.
- `CARGO_NET_OFFLINE=true python3 scripts/test_flac_user_corpus.py --mode all --require-complete <six supplied FLAC file paths>` exited 0. All six tracks passed reference FLAC integrity, FLAC/FFmpeg PCM equality, Rust inspection, PCM SHA-256 and byte-count parity.
- `python3 scripts/check_roadmap_index.py` and
  `python3 scripts/check_literature.py` passed; `git diff --check` was clean.

The six source recordings remained at their supplied locations. The script
used temporary decoded WAV files and removed them on exit; no audio, artwork,
or absolute source paths were added to Git.

**Pip-Boy universal dock clarification:** documented the intended Radio-to-Music
foreground handoff in `PIP_BOY_ADAPTER.md`. Music keeps a distinct package ID;
selecting it suspends Radio and transfers the shared foreground slot. This is
not destructive module replacement or dynamic third-party installation: the
current host only bootstraps trusted built-ins and still lacks Music controls
and the shared audio-focus bridge. After a later, explicit user authorization,
the separate Pip-Boy PR worktree received one scoped lifecycle regression and
documentation commit (`dcc7f5c`); it was not pushed, merged, or treated as Music
integration evidence. No additional sibling-repository changes are in scope.

**Rust Music revalidation:** on working tree `314913d` plus the documentation
delta above, `sh scripts/offline-gate.sh` passed again, including locked offline
workspace build/tests, release build, Clippy, formatting and macOS feature
check. `python3 scripts/check_roadmap_index.py`,
`python3 scripts/check_literature.py`, and `git diff --check` also passed.
This revalidates the local Rust Music release floor; it does not clear the
host-integration or physical-hardware gates.

**Remaining acceptance:** `RM-005` needs physical DAC evidence. `RM-006` and
`RM-007` need host-side shared Music/Radio/TV focus, package controls and panel
acceptance; the Pip-Boy worktree remains read-only under the current scope.
`RM-008` is the next planned P1 implementation task. No PR state transition
or push was performed while producing this log.
