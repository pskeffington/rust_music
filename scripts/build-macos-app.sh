#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
APP="$ROOT/target/macos/RustMusic.app"
CONTENTS="$APP/Contents"
MACOS="$CONTENTS/MacOS"

# The bundle is generated, but cp/redirection/compiler output paths can follow
# a symlink at the final filename as well as in a directory component.
sh "$ROOT/scripts/test_macos_app_build_path_guard.sh"
sh "$ROOT/scripts/check_macos_app_build_paths.sh" "$ROOT"

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
