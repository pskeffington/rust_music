# Rust Music phased gates

Check each gate with an actual assertion, report or hardware test. A repository commit or compile alone is not a performance/bit-perfect certification.

| Gate | Scope | Acceptance | Status |
| --- | --- | --- | --- |
| **G0** Research and ownership | Independent module convention, license, machine citations, index validator, no codec deps | Offline validation + source traceability | Source implemented; offline Rust compilation pending |
| **G1** Integer PCM foundation | typed format, bounded block, WAV decoder, explicit player state, no autoplay | Unit tests and inspect WAV | Source implemented; offline Rust compilation pending |
| **G2** Native FLAC decoder | STREAMINFO, full CRC, Rice/fixed/LPC, stereo, frame continuity, decoded-PCM MD5 | CLI FLAC->WAV full parity against reference plus IETF corpus | Integrity implementation added; Rust compile, corpus and physical device acceptance pending |
| **G3** Output devices | own CoreAudio/AudioQueue and ALSA thin API adapters, true device matching and underrun recovery | macOS/Linux physical audio, USB DAC, device switch + bit-perfect loopback | Initial adapters; hardware unverified |
| **G4** Expanded lossless | Own ALAC + MP4/M4A or CAF demux, RF64, optional seek support; build on G7 WAVE and G10 AIFF | independent PCM golden parity + malformed chunk tests | Planned |
| **G5** Rust_Boy micro integration | app slot, commands, playlist/index, compact CRT panel and host audio-focus arbiter; WASAPI/AAudio ports | drop-in install/unload; cross-module exclusive audio focus; Android/Windows targets | Source slot and G6 bitmap panel implemented; host deployment, real arbitration and other OS ports unverified |
| **G6** Tiny display | locally authored 1 KiB micro-panel | deterministic bitmap render and hardware host display | Source implemented; integration pending |
| **G8** Real-world FLAC corpus | Six private files, public hash-only vectors, exact reference PCM parity, offline own-decoder replay | 6/6 FLAC/FFmpeg parity verified; Rust CLI parity blocked by absent cargo |
| **G7** Strict WAV PCM interoperability | RIFF bound checks, odd padding, extensible PCM and exact export | offline WAV reference vectors, real Cargo PCM parity, hardware follow-up | Source implemented; six fixtures pass; Cargo and hardware pending |

| **G9** Exact sample-derived timing | Declared PCM frame count, playback progress, compact micro-screen clock | Offline compilation/tests + corpus parity + display review | Source implemented; compilation and physical screen unverified |

| **G10** Audiophile-only format gate and AIFF | Native AIFF/PCM AIFF-C, strict lossless codec checks, minimum 16-bit/44.1k playback | Python fixture evidence, offline Rust test/Clippy, complete decoder parity, hardware audio | Source implemented; operator-run 7/7 native AIFF PCM vectors PASS; full workspace gate and hardware unverified |

## Immediate priorities

1. Run `sh scripts/offline-gate.sh` on an offline Rust 1.85+ machine (cargo/rustfmt/clippy preinstalled). The optional GitHub job must use an operator-owned offline runner. Prior hosted CI runs had `startup_failure`; no compilation result can be inferred.
2. Run `python3 scripts/test_flac_reference.py --fixtures-only` without a Rust toolchain to validate seven deterministic reference fixtures (8/16/24/32-bit, 1/2/6 channels). Then run it without the flag in CI to compare decoded PCM and corruption rejection. Run the separately pinned IETF FLAC testbench and record exact corpus outcomes.
3. macOS AudioQueue hardware pass: exact frame delivery, no unwanted conversion, stop/drain, device change, clean unload.
4. Compile G10 AIFF with the offline toolchain; run the AIFF reference corpus and reject lossy AIFF-C. Preserve the 16-bit/44.1k playback floor without rejecting low-resolution decoder test fixtures.
5. Isolated ALAC packet decoder with custom CAF/MP4 demux and test vectors from an independently verified reference. Reject AAC in M4A, WavPack hybrid without correction and every unverified lossy codec. Sample-accurate seek and bounded metadata come after parity.
6. Rust_Boy host plugin adapter and validate the phosphor-green minimum UI on real Rust_Boy hardware.

## Scorecard

Track: load latency, steady RSS and peak RSS, decoded samples/s, output buffer underruns, seconds to output, CPU on low-end ARM, file coverage, corrupt-input fail-safe, byte-for-byte output parity. Do not provide assumed benchmark numbers.

## Zero-dependency policy gate

All Cargo dependencies must be local path edges. No third-party native sound library, default audio server, remote asset or build-time downloader is permitted. `rust-music-audit` rejects policy violations; see [Dependency policy](DEPENDENCY_POLICY.md). Source/ABI audit is not equivalent to successful cargo compilation or hardware playback.


## G6 addendum - standalone micro panel

- **Source delivered:** locally authored `rust-music-panel` 1-bit renderer,
  fixed 128 x 64 / 1024-byte buffer, font, host button translator, and CLI PBM
  preview. See [G6 micro panel](G6_MICRO_PANEL.md).
- **Offline source gate:** no new registry/Git/audio library or remote asset.
- **Pending acceptance:** Rust compilation, ABI-compatible real display host,
  actual phosphor-green screen test, and Radio/TV/Music arbitration end-to-end.
- **Later passes:** true metadata/tag parser, duration/progress and seek, native
  output hardware qualification, ALAC/AIFF. Never claim these controls until
  backed by working implementation and tests.

## G7 addendum - strict WAV and extensible PCM

See [G7 conformance ledger](G7_RIFF_CONFORMANCE.md). Legacy PCM and WAVE_FORMAT_EXTENSIBLE PCM now have source implementations with declared RIFF bounds and pad validation. Six stdlib reference fixtures passed; compiled decoder parity, Rustfmt/Clippy and physical DAC validation remain pending. RF64 and exact speaker-mask routing are separate future gates.

## G8 addendum - user-provided FLAC regression

See [G8 corpus ledger](G8_REALWORLD_FLAC_CORPUS.md). Six real 44.1kHz/16-bit stereo FLAC files with SEEKTABLE, VORBIS_COMMENT and PICTURE metadata passed reference checksum and PCM parity. No recordings are committed. Running `scripts/test_flac_user_corpus.py --mode rust` on a provisioned offline Rust toolchain is an explicit acceptance requirement; reference-only results do not certify the internal decoder.
