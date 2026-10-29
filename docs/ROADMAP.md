# Rust Music phased gates

Check each gate with an actual assertion, report or hardware test. A repository commit or compile alone is not a performance/bit-perfect certification.

| Gate | Scope | Acceptance | Status |
| --- | --- | --- | --- |
| **G0** Research and ownership | Independent module convention, license, machine citations, index validator, no codec deps | Offline validation + source traceability | Source implemented; offline Rust compilation pending |
| **G1** Integer PCM foundation | typed format, bounded block, WAV decoder, explicit player state, no autoplay | Unit tests and inspect WAV | Source implemented; offline Rust compilation pending |
| **G2** Native FLAC decoder | STREAMINFO, full CRC, Rice/fixed/LPC, stereo, frame continuity, decoded-PCM MD5 | CLI FLAC->WAV full parity against reference plus IETF corpus | Integrity implementation added; Rust compile, corpus and physical device acceptance pending |
| **G3** Output devices | own CoreAudio/AudioQueue and ALSA thin API adapters, true device matching and underrun recovery | macOS/Linux physical audio, USB DAC, device switch + bit-perfect loopback | Initial adapters; hardware unverified |
| **G4** Expanded lossless | Own AIFF/ALAC + MP4/M4A or CAF demux, WAVE extensible and RF64, optional seek support | independent PCM golden parity + malformed chunk tests | Planned |
| **G5** Rust_Boy micro integration | bounded scanner, app slot, commands, compact CRT panel and host audio-focus arbiter; WASAPI/AAudio ports | drop-in install/unload; cross-module exclusive audio focus; Android/Windows targets | **Headless scan/slot/lease source implemented; integration and hardware tests pending** |

## Immediate priorities

1. Run `sh scripts/offline-gate.sh` on an offline Rust 1.85+ machine (cargo/rustfmt/clippy preinstalled). The optional GitHub job must use an operator-owned offline runner. Prior hosted CI runs had `startup_failure`; no compilation result can be inferred.
2. Run `python3 scripts/test_flac_reference.py --fixtures-only` without a Rust toolchain to validate seven deterministic reference fixtures (8/16/24/32-bit, 1/2/6 channels). Then run it without the flag in CI to compare decoded PCM and corruption rejection. Run the separately pinned IETF FLAC testbench and record exact corpus outcomes.
3. macOS AudioQueue hardware pass: exact frame delivery, no unwanted conversion, stop/drain, device change, clean unload.
4. Finish native Rust compilation, offline tests and physical output acceptance. The bounded signature-only library scan and headless MUSIC slot are implemented in source; sample-accurate seek and embedded tag parser remain planned.
5. Isolated ALAC packet decoder with custom CAF/MP4 demux and test vectors from an independently verified reference.
6. Bind the host slot to Rust_Boy’s shared Radio/TV/Music focus mechanism and build the phosphor-green minimum UI; do not add a second output engine.

## Scorecard

Track: load latency, steady RSS and peak RSS, decoded samples/s, output buffer underruns, seconds to output, CPU on low-end ARM, file coverage, corrupt-input fail-safe, byte-for-byte output parity. Do not provide assumed benchmark numbers.

## Zero-dependency policy gate

All Cargo dependencies must be local path edges. No third-party native sound library, default audio server, remote asset or build-time downloader is permitted. `rust-music-audit` rejects policy violations; see [Dependency policy](DEPENDENCY_POLICY.md). Source/ABI audit is not equivalent to successful cargo compilation or hardware playback.

## Current next-pass notes (G5 foundation)

The `rust-music-library` crate performs explicit, signature-based scans with configurable entry/track/depth limits and symlink rejection. It contains no metadata downloader, database or audio engine. `rust-music-host` provides a single MUSIC slot with exclusive, host-owned RAII focus lease and no autoplay; Next/Previous releases focus. `rust-music-engine` now refuses format changes before handing PCM to output. See [host contract](HOST_CONTRACT.md). These are source-level changes pending offline Cargo verification.
