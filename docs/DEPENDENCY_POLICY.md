# Rust_Music: zero third-party dependency policy

Status: source and manifest audit implemented 2026-10-29; Rust compiler and hardware verification still pending. Applies to **every** Rust_Music crate, including codecs, CLI, engine, future panels and platform adapters.

## Definition

- **All product functionality is locally authored Rust**. Crate dependencies must be `path = "../..."` entries resolving to declared workspace packages; no registry, Git, source archive, build script, package installer, downloaded asset, prebuilt codec or sound library is allowed.
- **No network at build or playback**. `cargo --offline --locked` is enforced; playback reads only explicitly selected local paths. No web API, streaming service, telemetry, cloud account, license server, automatic updater or hidden subprocess.
- **Platform essentials are exceptions, not bundled dependencies**. The Rust toolchain and Rust standard library must already exist on the build machine. To reach a DAC, only the operating system's built-in kernel/device ABI may be used: on Linux direct ALSA kernel ioctl (`/dev/snd/pcmC*D*p`) via the OS C calling convention; on macOS Apple's built-in AudioToolbox. The operating system, drivers and hardware must be installed; there is no physically meaningful audio playback with literally zero platform dependencies.
- **Not allowed as runtime or build dependencies**: `libasound`, GStreamer, FFmpeg, libFLAC, CPAL, Rodio, Symphonia, Claxon, NPM, JavaScript runtimes, external codecs, vendor-provided audio SDKs, PulseAudio, PipeWire, JACK, and plugin downloads.
- **Research references are not dependencies**. The linked standards, players, and optional `scripts/test_flac_reference.py` use an **external reference encoder for development comparison only**, never for a normal build, CI offline gate, or playback. Python research tools are optional and are not run by the offline build gate.

## Run offline

Prerequisite: locally installed compatible Rust 1.85+ toolchain, rustfmt and clippy, and the checked-out source. Disconnect networking before running:

```sh
sh scripts/offline-gate.sh
```

The gate executes `cargo run --locked --offline -p rust-music-audit`, then fmt, clippy, all workspace tests, release builds and the available native backend feature checks with `CARGO_NET_OFFLINE=true`. It performs no package installation, download or network access. `Cargo.lock` includes exactly the workspace packages and no `source` or `checksum` entries.

To build only the headless engine:

```sh
cargo build --locked --offline -p rust-music
```

To select an explicit Linux kernel device (experimental, 64-bit Linux only):

```sh
cargo run --locked --offline -p rust-music-cli --features alsa-output -- play /path/to/track.flac hw:0,0
```

**No `default`, `plughw`, `dmix`, or guessed Linux device names**: the direct kernel adapter only opens explicit `hw:CARD,DEVICE` endpoints and fails if the device refuses the exact requested format. It will not load a userspace sound-server fallback. For macOS the experimental adapter uses the OS AudioQueue framework; hardware conversion/bit-perfect output still requires testing.

## Safeguards

- The self-authored `rust-music-audit` executable examines all workspace manifests, exact path edges, Cargo.lock package set, build scripts, `.rs` files, forbidden network APIs and native `#[link]` directives. It allows a single, exact macOS system framework link. This check is defensive/static, not a substitute for compilation or code review.
- Repository `.cargo/config.toml` defaults Cargo to offline mode, including direct `cargo build`; the offline+locked gate blocks registry access or lockfile mutation. Generated code or future features must also pass the same audit and tests.
- The GitHub Actions workflow is optional `workflow_dispatch` on a pre-provisioned **operator-owned offline runner** with the exact commit already checked out. It uses no third-party action or package installer. No CI runner is claimed available until a job actually runs.
- The public runtime does not require Python, FLAC reference tools, GitHub, a Rust build daemon, or internet access.

## Acceptance ledger

| Requirement | Source audit | Cargo validation | Physical validation |
| --- | --- | --- | --- |
| All Cargo dependencies are workspace paths | Pass (static) | Pending (cargo unavailable here) | N/A |
| No third-party native audio library | Removed libasound; static pass | Pending feature compile | Pending ALSA hardware |
| Headless FLAC/WAV output offline | No network paths in decoder | Pending tests | N/A |
| macOS system AudioQueue | System framework only | Pending macOS feature compile | Pending physical DAC |
| Windows and Android playback | Not yet implemented | Not represented as supported | Pending |
| Optional reference FLAC comparison | Outside offline gate | Pending differential corpus | N/A |

All-gates-green or bit-perfect claims are prohibited until measured.
