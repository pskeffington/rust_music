#!/bin/sh
set -eu

ROOT=${1:?usage: check_macos_app_build_paths.sh REPOSITORY_ROOT}
APP="$ROOT/target/macos/RustMusic.app"
CONTENTS="$APP/Contents"
MACOS="$CONTENTS/MacOS"

for path in \
  "$ROOT/target" \
  "$ROOT/target/release" \
  "$ROOT/target/macos" \
  "$ROOT/target/swift-module-cache" \
  "$APP" \
  "$CONTENTS" \
  "$MACOS"
do
  if [ -L "$path" ]; then
    echo "Refusing symlinked macOS app build directory: $path" >&2
    exit 2
  fi
  if [ -e "$path" ] && [ ! -d "$path" ]; then
    echo "Refusing non-directory macOS app build path: $path" >&2
    exit 2
  fi
done

# Build tools create nested directories under these output roots. Refuse any
# pre-existing symlink below them so a compiler cannot follow one outside the
# repository even when the top-level directory is safe.
for path in "$ROOT/target/release" "$ROOT/target/macos" "$ROOT/target/swift-module-cache"
do
  if [ -d "$path" ]; then
    nested=$(find "$path" -type l -print -quit)
    if [ -n "$nested" ]; then
      echo "Refusing nested symlink in macOS app build output: $nested" >&2
      exit 2
    fi
  fi
done

for path in \
  "$CONTENTS/Info.plist" \
  "$CONTENTS/PkgInfo" \
  "$MACOS/rust-music-cli" \
  "$MACOS/RustMusic" \
  "$ROOT/target/macos/PlaybackUIStateTests" \
  "$ROOT/target/.rustc_info.json" \
  "$ROOT/target/CACHEDIR.TAG" \
  "$ROOT/target/.future-incompat-report.json"
do
  if [ -L "$path" ]; then
    echo "Refusing symlinked macOS app build output: $path" >&2
    exit 2
  fi
done
