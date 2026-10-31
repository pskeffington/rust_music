#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
TEMP_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/rust-music-build-path-test.XXXXXX")
trap 'rm -rf "$TEMP_ROOT"' EXIT HUP INT TERM
SENTINEL="$TEMP_ROOT/external-sentinel"
printf 'preserve-this-file\n' > "$SENTINEL"

for relative in \
  target \
  target/release \
  target/macos \
  target/swift-module-cache \
  target/macos/RustMusic.app \
  target/macos/RustMusic.app/Contents \
  target/macos/RustMusic.app/Contents/MacOS \
  target/macos/RustMusic.app/Contents/Info.plist \
  target/macos/RustMusic.app/Contents/PkgInfo \
  target/macos/RustMusic.app/Contents/MacOS/rust-music-cli \
  target/macos/RustMusic.app/Contents/MacOS/RustMusic \
  target/macos/PlaybackUIStateTests \
  target/.rustc_info.json \
  target/CACHEDIR.TAG \
  target/.future-incompat-report.json
do
  TEST_ROOT="$TEMP_ROOT/case"
  rm -rf "$TEST_ROOT"
  mkdir -p "$TEST_ROOT/scripts"
  cp "$SCRIPT_DIR/check_macos_app_build_paths.sh" "$TEST_ROOT/scripts/"
  parent=$(dirname "$TEST_ROOT/$relative")
  mkdir -p "$parent"
  ln -s "$SENTINEL" "$TEST_ROOT/$relative"

  if sh "$TEST_ROOT/scripts/check_macos_app_build_paths.sh" "$TEST_ROOT" >/dev/null 2>&1; then
    echo "FAIL: accepted symlinked build path $relative" >&2
    exit 1
  else
    result=$?
  fi
  if [ "$result" -ne 2 ]; then
    echo "FAIL: $relative returned $result instead of refusal status 2" >&2
    exit 1
  fi
  if [ "$(cat "$SENTINEL")" != "preserve-this-file" ]; then
    echo "FAIL: symlink guard altered the external sentinel for $relative" >&2
    exit 1
  fi
done

# Nested output symlinks can redirect compiler-created files despite safe roots.
for relative in \
  target/release/deps \
  target/macos/RustMusic.app/Contents/Resources \
  target/swift-module-cache/modules
do
  TEST_ROOT="$TEMP_ROOT/nested-case"
  rm -rf "$TEST_ROOT"
  mkdir -p "$TEST_ROOT/scripts" "$TEST_ROOT/$(dirname "$relative")"
  cp "$SCRIPT_DIR/check_macos_app_build_paths.sh" "$TEST_ROOT/scripts/"
  ln -s "$SENTINEL" "$TEST_ROOT/$relative"

  if sh "$TEST_ROOT/scripts/check_macos_app_build_paths.sh" "$TEST_ROOT" >/dev/null 2>&1; then
    echo "FAIL: accepted nested symlinked build path $relative" >&2
    exit 1
  else
    result=$?
  fi
  if [ "$result" -ne 2 ]; then
    echo "FAIL: nested $relative returned $result instead of refusal status 2" >&2
    exit 1
  fi
  if [ "$(cat "$SENTINEL")" != "preserve-this-file" ]; then
    echo "FAIL: nested symlink guard altered the external sentinel for $relative" >&2
    exit 1
  fi
done

echo "PASS: 18 symlinked macOS app build paths refused without changing the external sentinel"
