# Start here — Rust Music

This page is the short route from clone to a working developer build. Read [README](../README.md) for product scope, [architecture](ARCHITECTURE.md) for components, and [roadmap](ROADMAP.md) for outstanding work.

## Status and branch choice

On 2026-10-31, `main` and `build/native-codecs-g0` were identical at `deb6f879`; subsequent documentation changes are on `main`. PR #1 is merged. Do **not** follow historical instructions to merge PR #1 or treat main as README-only. Inspect the actual current checkout and preserve uncommitted changes before making edits. This is not a production-certified build.

## Prerequisites

Use macOS with locally installed Xcode command line tools/Swift tooling and Rust/Cargo (workspace MSRV Rust 1.85). Builds use locally available tools; Cargo commands are explicitly offline and locked. The desktop bundle is macOS-only; Linux and Windows GUI support is future work.

## Build the macOS app

From the repository root:

```sh
sh scripts/build-macos-app.sh
```

The script builds and locally signs a development app bundle; this is not Developer ID signing or notarization. Follow the script output for the exact bundle location. No user music is packaged.

## Compile and test the Rust workspace

```sh
cargo build --locked --offline --workspace
cargo test --locked --offline --workspace
sh scripts/offline-gate.sh
```

The offline gate is the broader project acceptance check. Compile success does not establish device/DAC fidelity, private-corpus parity, or live Pip-Boy audio-focus compatibility.

## Try a library scan or inspect a track

```sh
cargo run --locked --offline -p rust-music-cli -- scan "$HOME/Music"
cargo run --locked --offline -p rust-music-cli -- inspect "/absolute/path/to/track.flac"
```

Loading a library is explicit. Do not introduce unrequested scanning or playback; lossless decoder admission is determined from file content, not merely filename extensions.

## Developer navigation

- [Codex and agent contract](../AGENTS.md) — precedence, constraints and task routing
- [Roadmap](ROADMAP.md) and [task index](roadmap-index.json) — planned versus verified work
- [Mac v1 acceptance](CODEX_MACOS_V1_DELIVERABLE.md) — evidence and remaining release gates
- [Host integration](HOST_CONTRACT.md) and [Pip-Boy adapter](PIP_BOY_ADAPTER.md) — shared audio-focus boundaries
- [Portable library deployment](design/PORTABLE_LIBRARY_DEPLOYMENT.md) — future export/sync contract
- [Release and merge evidence](PR1_MERGE_READINESS.md) — historical PR #1 record

## Troubleshooting

If an offline build reports an unavailable package or toolchain, verify that the local Rust toolchain is installed, and do not silently enable network downloads. If native macOS packaging fails, inspect its script output and the locally installed Apple build tools. Keep private music and test corpora outside Git. Report commands, exit codes, machine/OS versions and evidence when filing validation failures.
