#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
APP="$ROOT/target/macos/RustMusic.app"
CONTENTS="$APP/Contents"
MACOS="$CONTENTS/MacOS"

cd "$ROOT"
CARGO_NET_OFFLINE=true cargo build --locked --offline --release -p rust-music-cli --features macos-output
mkdir -p "$MACOS"
mkdir -p "$ROOT/target/swift-module-cache"
cp apps/rust-music-macos/Info.plist "$CONTENTS/Info.plist"
printf 'APPL????' > "$CONTENTS/PkgInfo"
cp target/release/rust-music-cli "$MACOS/rust-music-cli"
swiftc -module-cache-path "$ROOT/target/swift-module-cache" -O -parse-as-library -framework AppKit -framework SwiftUI \
  apps/rust-music-macos/RustMusicApp.swift -o "$MACOS/RustMusic"
printf 'Built native app: %s\n' "$APP"
