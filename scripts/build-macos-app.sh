#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
APP="$ROOT/target/macos/RustMusic.app"
CONTENTS="$APP/Contents"
MACOS="$CONTENTS/MacOS"

# The bundle is a generated artifact, but several commands below replace its
# contents. Refuse symlinked path components so a redirected target tree cannot
# make cleanup/copy/sign operations mutate a location outside this checkout.
for path in \
  "$ROOT/target" \
  "$ROOT/target/macos" \
  "$APP" \
  "$CONTENTS" \
  "$MACOS"
do
  if [ -L "$path" ]; then
    echo "Refusing symlinked app build path: $path" >&2
    exit 2
  fi
  if [ -e "$path" ] && [ ! -d "$path" ]; then
    echo "Refusing non-directory app build path: $path" >&2
    exit 2
  fi
done

cd "$ROOT"
CARGO_NET_OFFLINE=true cargo build --locked --offline --release -p rust-music-cli --features macos-output
mkdir -p "$MACOS"
mkdir -p "$ROOT/target/swift-module-cache"
# This path is a generated build artifact. Remove only its previous signature
# before replacing bundle contents so CodeResources cannot seal stale files.
rm -rf "$CONTENTS/_CodeSignature"
cp apps/rust-music-macos/Info.plist "$CONTENTS/Info.plist"
printf 'APPL????' > "$CONTENTS/PkgInfo"
cp target/release/rust-music-cli "$MACOS/rust-music-cli"
SWIFT_ARCH=$(uname -m)
case "$SWIFT_ARCH" in
  arm64|x86_64) ;;
  *) echo "Unsupported macOS build architecture: $SWIFT_ARCH" >&2; exit 2 ;;
esac
swiftc -module-cache-path "$ROOT/target/swift-module-cache" \
  apps/rust-music-macos/PlaybackUIState.swift \
  apps/rust-music-macos/PlaybackUIStateTests.swift \
  -o "$ROOT/target/macos/PlaybackUIStateTests"
"$ROOT/target/macos/PlaybackUIStateTests"
swiftc -target "$SWIFT_ARCH-apple-macosx13.0" -module-cache-path "$ROOT/target/swift-module-cache" -O -parse-as-library -framework AppKit -framework SwiftUI \
  apps/rust-music-macos/PlaybackUIState.swift apps/rust-music-macos/RustMusicApp.swift -o "$MACOS/RustMusic"
codesign --force --deep --sign - --timestamp=none "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"
printf 'Built native app: %s\n' "$APP"
