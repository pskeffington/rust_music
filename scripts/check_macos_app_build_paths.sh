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

for path in \
  "$CONTENTS/Info.plist" \
  "$CONTENTS/PkgInfo" \
  "$MACOS/rust-music-cli" \
  "$MACOS/RustMusic" \
  "$ROOT/target/macos/PlaybackUIStateTests"
do
  if [ -L "$path" ]; then
    echo "Refusing symlinked macOS app build output: $path" >&2
    exit 2
  fi
done
