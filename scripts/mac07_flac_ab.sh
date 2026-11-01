#!/bin/sh
# MAC07-02: same-file native CLI / desktop-GUI parity protocol.
# Operator confirmation is deliberately required: this script cannot assert audibility.
set -eu
if [ "$#" -ne 1 ]; then
  printf 'Usage: sh scripts/mac07_flac_ab.sh /absolute/path/to/track.flac\n' >&2
  exit 64
fi
input=$1
case "$input" in
  /*) ;;
  *) printf 'Input must be an absolute path.\n' >&2; exit 64 ;;
esac
if [ ! -f "$input" ]; then
  printf 'Missing FLAC input: %s\n' "$input" >&2
  exit 66
fi
case "$input" in
  *.[Ff][Ll][Aa][Cc]) ;;
  *) printf 'Expected a .flac input.\n' >&2; exit 65 ;;
esac
if [ "$(uname -s)" != Darwin ]; then
  printf 'MAC07 native audio A/B needs macOS.\n' >&2
  exit 69
fi
cd "$(dirname "$0")/.."
printf 'SHA256 of test input:\n'
shasum -a 256 "$input"
printf '\nStep A: native decoder inspection\n'
cargo run --locked --offline -p rust-music-cli --features macos-output -- inspect "$input"
printf '\nStep B: direct native CLI output. Listen and note audible result, then return here.\n'
cargo run --locked --offline -p rust-music-cli --features macos-output -- play "$input"
printf '\nStep C: macOS GUI A/B. Build the same-source app, then open it.\n'
sh scripts/build-macos-app.sh
open target/macos/RustMusic.app
printf '\nIn GUI: load the parent library folder, select the EXACT same track, press Play.\n'
printf 'Record CLI audible yes/no; GUI audible yes/no; status; volume; output device; stderr count if present.\n'
printf 'Do not treat GUI Playing status as proof of audible output. Keep PR draft until verified.\n'
