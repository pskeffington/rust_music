#!/bin/sh
# Capture the authoritative offline gate with evidence outside the repository.
# No downloads, third-party packages, or automatic roadmap status changes.
set -u
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd) || exit 2
cd "$ROOT" || exit 2
if ! command -v git >/dev/null 2>&1; then
  echo "BLOCKED: git required for evidence SHA" >&2
  exit 3
fi
HEAD=$(git rev-parse HEAD 2>/dev/null) || exit 2
BRANCH=$(git branch --show-current 2>/dev/null) || BRANCH=unknown
if [ -n "$(git status --porcelain 2>/dev/null)" ]; then
  echo "BLOCKED: working tree not clean; preserve edits and commit/review first" >&2
  exit 3
fi
case "$BRANCH" in
  build/native-codecs-g0) ;;
  *) echo "BLOCKED: expected build/native-codecs-g0, got $BRANCH" >&2; exit 3 ;;
esac
if [ -z "${RUST_MUSIC_EVIDENCE_DIR:-}" ]; then
  RUST_MUSIC_EVIDENCE_DIR="${HOME:?}/rust-music-gate-evidence"
fi
case "$RUST_MUSIC_EVIDENCE_DIR" in
  /*) ;;
  *) echo "BLOCKED: RUST_MUSIC_EVIDENCE_DIR must be absolute" >&2; exit 3 ;;
esac
mkdir -p "$RUST_MUSIC_EVIDENCE_DIR" || exit 2
EVDIR=$(CDPATH= cd -- "$RUST_MUSIC_EVIDENCE_DIR" && pwd -P) || exit 2
case "$EVDIR/" in
  "$ROOT/"*) echo "BLOCKED: evidence directory must be outside repository" >&2; exit 3 ;;
esac
chmod 700 "$EVDIR" || exit 2
STAMP=$(date -u '+%Y%m%dT%H%M%SZ') || exit 2
PREFIX="$EVDIR/rm001-$STAMP-$(printf '%s' "$HEAD" | cut -c 1-12)-$$"
LOG="$PREFIX.log"
META="$PREFIX.summary"
umask 077
: > "$LOG" || exit 2
{
  printf 'task=RM-001\n'
  printf 'branch=%s\n' "$BRANCH"
  printf 'head=%s\n' "$HEAD"
  printf 'started_utc=%s\n' "$STAMP"
  printf 'platform=%s\n' "$(uname -s)"
  printf 'rustc=%s\n' "$(rustc --version 2>/dev/null || printf unavailable)"
  printf 'cargo=%s\n' "$(cargo --version 2>/dev/null || printf unavailable)"
  printf 'gate_command=sh scripts/offline-gate.sh\n'
  printf 'log_path=%s\n' "$LOG"
} > "$META" || exit 2
sh scripts/offline-gate.sh > "$LOG" 2>&1
STATUS=$?
{
  printf 'finished_utc=%s\n' "$(date -u '+%Y%m%dT%H%M%SZ')"
  printf 'exit_code=%s\n' "$STATUS"
  if [ "$STATUS" -eq 0 ]; then
    printf 'gate_result=PASS\n'
  elif [ "$STATUS" -eq 3 ]; then
    printf 'gate_result=BLOCKED\n'
  else
    printf 'gate_result=FAIL\n'
  fi
} >> "$META"
printf 'RM-001 gate exit=%s\nEvidence summary: %s\nPrivate log: %s\n' "$STATUS" "$META" "$LOG"
exit "$STATUS"
