#!/bin/sh
# Entire Rust_Music product and test gate is runnable with no network connection.
# Prerequisite: a locally installed Rust toolchain with cargo, rustfmt and clippy.
set -eu
cd "$(dirname "$0")/.."
export CARGO_NET_OFFLINE=true
export CARGO_INCREMENTAL=0
if ! command -v cargo >/dev/null 2>&1; then
    echo 'BLOCKED: Rust toolchain not installed locally (cargo missing)' >&2
    exit 3
fi
cargo run --locked --offline --quiet -p rust-music-audit -- .
cargo fmt --all -- --check
cargo clippy --locked --offline --workspace --all-targets -- -D warnings
cargo test --locked --offline --workspace
cargo build --locked --offline --release --workspace
case "$(uname -s)" in
    Linux)
        cargo check --locked --offline -p rust-music-cli --features alsa-output
        cargo test --locked --offline -p rust-music-native --features alsa
        ;;
    Darwin)
        cargo check --locked --offline -p rust-music-cli --features macos-output
        ;;
esac
printf '%s\n' 'PASS: All Rust_Music workspace crates built/tested offline; zero registry/git crates.'
