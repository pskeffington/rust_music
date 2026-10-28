# Rust Music phased gates

Check each gate with an actual assertion, report or hardware test. A repository commit or compile alone is not a performance/bit-perfect certification.

| Gate | Scope | Acceptance | Status |
| --- | --- | --- | --- |
| **G0** Research and ownership | Independent module convention, license, machine citations, index validator, no codec deps | Offline validation + source traceability | Implemented; CI pending |
| **G1** Integer PCM foundation | typed format, bounded block, WAV decoder, explicit player state, no autoplay | Unit tests and inspect WAV | Implemented; CI pending |
| **G2** Native FLAC decoder | STREAMINFO, full CRC, Rice/fixed/LPC, stereo, frame continuity, decoded-PCM MD5 | CLI FLAC->WAV full parity against reference plus IETF corpus | Integrity implementation added; Rust compile, corpus and physical device acceptance pending |
| **G3** Output devices | own CoreAudio/AudioQueue and ALSA thin API adapters, true device matching and underrun recovery | macOS/Linux physical audio, USB DAC, device switch + bit-perfect loopback | Initial adapters; hardware unverified |
| **G4** Expanded lossless | Own AIFF/ALAC + MP4/M4A or CAF demux, WAVE extensible and RF64, optional seek support | independent PCM golden parity + malformed chunk tests | Planned |
| **G5** Rust_Boy micro integration | app slot, commands, playlist/index, compact CRT panel and host audio-focus arbiter; WASAPI/AAudio ports | drop-in install/unload; cross-module exclusive audio focus; Android/Windows targets | Planned |

## Immediate priorities

1. Resolve GitHub Actions `startup_failure` with zero jobs scheduled (observed on PR #1) and then require green fmt, clippy, unit, and reference FLAC differential jobs. Do not infer compilation success from a run that scheduled no jobs.
2. Run `python3 scripts/test_flac_reference.py --fixtures-only` without a Rust toolchain to validate seven deterministic reference fixtures (8/16/24/32-bit, 1/2/6 channels). Then run it without the flag in CI to compare decoded PCM and corruption rejection. Run the separately pinned IETF FLAC testbench and record exact corpus outcomes.
3. macOS AudioQueue hardware pass: exact frame delivery, no unwanted conversion, stop/drain, device change, clean unload.
4. Implement sample-accurate seek and bounded local library metadata/playlist scan.
5. Isolated ALAC packet decoder with custom CAF/MP4 demux and test vectors from an independently verified reference.
6. Rust_Boy host plugin adapter and phosphor-green minimum UI.

## Scorecard

Track: load latency, steady RSS and peak RSS, decoded samples/s, output buffer underruns, seconds to output, CPU on low-end ARM, file coverage, corrupt-input fail-safe, byte-for-byte output parity. Do not provide assumed benchmark numbers.
