#!/bin/sh
# Fail-closed pre-merge checks for Rust Music PR #1.
# Never installs dependencies, fetches code, uploads media, changes branches or merges.
# Requires operator-provisioned Rust toolchain and private six-track corpus.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P) || exit 2
cd "$ROOT" || exit 2

blocked() {
  printf 'BLOCKED premerge: %s\n' "$1" >&2
  exit 3
}

command -v git >/dev/null 2>&1 || blocked 'git is not installed'
CURRENT=$(git branch --show-current) || blocked 'cannot determine active branch'
[ "$CURRENT" = "build/native-codecs-g0" ] || blocked 'checkout build/native-codecs-g0'
[ -z "$(git status --porcelain)" ] || blocked 'working tree is dirty; preserve and review edits'
git rev-parse --verify 'refs/remotes/origin/main^{commit}' >/dev/null 2>&1 ||
  blocked 'origin/main is unavailable; refresh remote refs deliberately outside this script'
git merge-base --is-ancestor refs/remotes/origin/main HEAD ||
  blocked 'PR branch is behind or diverged from local origin/main; reconcile before merging'

printf 'PR branch: %s\n' "$CURRENT"
printf 'Commit: %s\n' "$(git rev-parse HEAD)"
printf 'Local origin/main: %s\n' "$(git rev-parse refs/remotes/origin/main)"
command -v cargo >/dev/null 2>&1 || blocked 'cargo unavailable; provision offline toolchain'
command -v rustc >/dev/null 2>&1 || blocked 'rustc unavailable; provision offline toolchain'
command -v python3 >/dev/null 2>&1 || blocked 'python3 needed only for operator acceptance scripts'
printf 'Toolchain: %s; %s\n' "$(rustc --version)" "$(cargo --version)"

python3 scripts/check_roadmap_index.py
python3 scripts/check_literature.py
python3 -m unittest discover -s tests -p test_roadmap_index.py

# The wrapper records HEAD, toolchain, exact exit status and a private log outside Git.
# Its exit status is the offline Rust gate exit status, not an assumed pass.
sh scripts/capture_offline_gate.sh

# Native decoder parity is separate from an offline compile/test success.
python3 scripts/test_wave_reference.py
python3 scripts/test_aiff_reference.py
[ -n "${PRIVATE_FLAC_DIRECTORY:-}" ] ||
  blocked 'PRIVATE_FLAC_DIRECTORY required for six-track own-decoder parity; never commit audio'
[ -d "$PRIVATE_FLAC_DIRECTORY" ] ||
  blocked 'private FLAC directory unavailable; never upload or commit private tracks'
python3 scripts/test_flac_user_corpus.py --mode rust --require-complete "$PRIVATE_FLAC_DIRECTORY"

printf '%s\n' 'PASS: automated offline, reference and private-corpus premerge checks completed.'
printf '%s\n' 'NOT CERTIFIED: Rust_Boy Radio/TV audio-focus, physical panel/DAC, and release acceptance need independent evidence.'
printf '%s\n' 'This script never merges, closes GitHub issues, updates the roadmap, or certifies hardware fidelity.'
