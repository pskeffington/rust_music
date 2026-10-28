# Rust Music phased gates

Check each gate with an actual assertion, report or hardware test. A repository commit or compile alone is not a performance/bit-perfect certification.

| Gate | Scope | Acceptance | Status |
| --- | --- | --- | --- |
| **G0** Research and ownership | Independent module convention, license, machine citations, index validator, no codec deps | Offline validation + source traceability | Implemented; CI pending |
| **G1** Integer PCM foundation | typed format, bounded block, WAV decoder, explicit player state, no autoplay | Unit tests and inspect WAV | Implemented; CI pending |
| **G2** Native FLAC decoder | STREAMINFO, complete frame CRC, Rice/fixed/LPC, stereo transformations, corrupted-input rejection | CLI FLAC->WAV full parity against reference plus IETF corpus; later PCM MD5 | Initial implementation; full conformance pending |
| **G3** Output devices | own CoreAudio/AudioQueue and ALSA thin API adapters, true device matching and underrun recovery | macOS/Linux physical audio, USB DAC, device switch + bit-perfect loopback | Initial adapters; hardware unverified |
| **G4** Expanded lossless | Own AIFF/ALAC + MP4/M4A or CAF demux, WAVE extensible and RF64, optional seek support | independent PCM golden parity + malformed chunk tests | Planned |
| **G5** Rust_Boy micro integration | app slot, commands, playlist/index, compact CRT panel and host audio-focus arbiter; WASAPI/AAudio ports | drop-in install/unload; cross-module exclusive audio focus; Android/Windows targets | Planned |

## Immediate priorities

1. CI green (fmt, clippy, unit tests, reference FLAC differential) and repair observed fails.
2. Run IETF FLAC testbench pinned to a specific commit, record accepted/rejected cases and compare STREAMINFO MD5.
3. macOS AudioQueue hardware pass: exact frame delivery, no unwanted conversion, stop/drain, device change, clean unload.
4. Implement sample-accurate seek and bounded local library metadata/playlist scan.
5. Isolated ALAC packet decoder with custom CAF/MP4 demux and test vectors from an independently verified reference.
6. Rust_Boy host plugin adapter and phosphor-green minimum UI.

## Scorecard

Track: load latency, steady RSS and peak RSS, decoded samples/s, output buffer underruns, seconds to output, CPU on low-end ARM, file coverage, corrupt-input fail-safe, byte-for-byte output parity. Do not provide assumed benchmark numbers.
