# G10 — Audiophile-only formats and native AIFF

Date: 2026-10-29. Applies to the standalone engine and the Rust_Boy MUSIC module.

## Playback policy

- Only lossless formats. Minimum playback profile: 16-bit integer PCM at 44,100 Hz. Source formats meeting this include 16/44.1, 16/48, 24/48, 24/96, 24/192 and 32-bit integer at approved rates.
- No conversion from a lower-quality source to an artificial higher rate or bit depth. No automatic float DSP, normalization, replay gain, crossfade, dither or sample-rate conversion.
- The decode API still accepts low-resolution 8-bit and low-rate PCM fixtures for conformance testing; Player::load rejects them before opening the DAC or taking Rust_Boy audio focus.
- Source-level losslessness is not proof of mastering quality or provenance. A previously lossy MP3 transcoded into FLAC cannot be recovered. PCM parity is not proof of bit-perfect device output.

## Format acceptance matrix

| Files | Actual audio codec | Implementation status | Quality restrictions |
| --- | --- | --- | --- |
| .flac | Native FLAC | Seven generated vectors and six private real-world tracks pass native PCM/integrity checks | Stream CRC + decoded-PCM MD5 at EOF, 16-bit/44.1k+ for playback |
| .wav | RIFF/WAVE signed integer PCM or 8-bit unsigned PCM | Six generated native CLI vectors pass PCM parity; physical output pending | PCM WAVE_FORMAT_EXTENSIBLE supported; 8-bit only in decoder tests |
| .aif, .aiff | AIFF signed integer PCM | Native CLI PCM parity: PASS (7-case mixed corpus); offline workspace gate passes; hardware pending | 8/16/24/32-bit big-endian, exact extended-rate parsing |
| .aifc | AIFF-C signed PCM | Native CLI PCM parity: PASS (7-case mixed corpus); offline workspace gate passes; hardware pending | Valid FVER and NONE, twos or sowt only; no ADPCM/ulaw/alaw |
| .m4a / .mp4 | MP4 containing ALAC | Bounded profile passes generated parity and four Internet Archive CC0 M4A files (three 16-bit/44.1 kHz and one 24-bit/48 kHz) with exact PCM parity against FFmpeg; scanner indexes `.m4a`/`.mp4` by `ftyp`, while codec admission stays in the decoder. AAC, DRM, external data, unsupported layouts and malformed bounds fail closed. Broader ALAC/CAF corpus remains open | AAC in identical container prohibited |
| .caf | ALAC or signed integer LPCM | Bounded profile passes generated FFmpeg-encoded CAF parity: ALAC 16/24-bit and LPCM 16/24/32-bit; strict cookie, packet-table, chunk-bound and codec checks; no OS decoder fallback. Broad real-world CAF corpus and host/output validation remain open | Mono/stereo only; reject AAC, ADPCM, float PCM and unsupported channel layouts |
| .wv | WavPack pure lossless or hybrid with verified correction | Planned | Reject lossy-only hybrid files |
| RF64/BWF .wav | Extended large-file PCM | Native source implemented | Bounded ds64 parser and synthetic >4 GiB streaming/sample parity; real large-file corpus and hardware remain unverified |
| .oga / .ogg | Ogg FLAC mapping | Later | Reject Vorbis and Opus |
| .dsf, .dff | DSD | Optional native-device path | Never claim DSD-bit-perfect after PCM conversion |
| .ape | Monkey's Audio | Optional research | Local decoder and parity required |
| .mp3 / AAC / Opus / Vorbis / lossy WMA | Lossy | Excluded | Never transcode for fallback |

The current library scan is a fast filename-extension and magic-byte probe, not exhaustive codec verification. An indexed item can be rejected during decoder initialization or format validation; rejected audio is not played.

## Native AIFF implementation

A new locally owned crate, rust-music-aiff, decodes AIFF and uncompressed AIFF-C signed integer PCM without external codec libraries. The facade sniffs the binary FORM marker and accepts AIFF/AIFC only through this decoder.

Parser rules: checked FORM length, bounded IFF chunks, odd-byte padding, max 512 metadata headers, max 16 MiB per skipped metadata chunk, max 64 MiB aggregate skipped metadata, checked SSND offset and data length, and equality of SSND audio bytes with COMM declared frames × channels × sample width. Nonzero SSND block-alignment schemes are explicitly unsupported. The parser produces at most 4,096 PCM frames per block and preserves signed 8/16/24/32-bit samples. AIFF-C compressed codec identifiers fail closed.

The AIFF 80-bit extended rate is decoded with integer math and must represent an exact whole number of Hz, never rounded. The renderer labels AIFF and remains exactly 128×64 pixels / 1,024 bytes.

Known limits: chunks following the audio SSND are not exhaustively parsed. Local workspace build, Clippy, tests and release build now pass, but this does not certify a physical DAC. The quick scanner may include a provisional file that the full decoder later rejects.

## Reproduction and evidence

Python-stdlib-only generated reference vectors cover uncompressed AIFF and AIFF-C, 16/24/32-bit PCM, endian swap, SSND offsets, odd-length chunks, FORM overrun, truncation, wrong frame counts and lossy codec rejection. An 8-bit parser-only fixture is deliberately not an accepted playback format.

    python3 scripts/test_aiff_reference.py --fixtures-only
    python3 scripts/test_aiff_reference.py
    python3 scripts/test_caf_reference.py
    python3 scripts/test_m4a_corpus.py
    sh scripts/offline-gate.sh
    python3 scripts/test_flac_user_corpus.py --mode rust --require-complete /private/music

The second and subsequent commands require a locally installed Rust toolchain; the last requires the original private six-track FLAC files. No recordings or encoded artwork are committed. Do not mark Rust compile, full PCM parity or physical DAC acceptance as PASS until they have run. Keep PR #1 draft.

## Operator-reported native AIFF validation (2026-10-29)

The operator ran the checked-in Python-stdlib fixture harness on a macOS machine with a working Rust toolchain and shared the following terminal results:

- `python3 scripts/test_aiff_reference.py --fixtures-only`: **PASS: 7 AIFF/AIFF-C vectors; fixtures-only=True**.
- `python3 scripts/test_aiff_reference.py`: **PASS: 7 AIFF/AIFF-C vectors; fixtures-only=False**. Each case invokes the native Rust CLI through `cargo run --locked --offline`, performs `inspect` and `decode`, then compares exported WAV integer PCM with an independent fixture expectation.
- Passed cases: `mono-16`, `stereo-24-192k`, `stereo-32`, `aifc-none`, `aifc-sowt-24`, `offset-16`, and `parser-only-8`. The 8-bit example tests the decoder, not permitted playback.
- The native-harness output also reports rejection of corrupt FORM boundaries, contradictory COMM frame counts, truncated data and lossy AIFF-C.

**Evidence basis:** the original seven-vector transcript was user-supplied, not a CI artifact. A later local run of the same harness confirmed the seven synthetic PCM parity cases. This does **not** prove that the six real FLACs passed native Rust parity or that audio reaches a physical DAC bit-perfect. Continue to keep PR #1 draft.

Note for interactive Zsh: pasted comment lines may cause `zsh: command not found: #` unless `setopt interactivecomments` is enabled. These shell-comment messages are unrelated to the Rust decoder test outcomes.

## Next priorities

1. Expand ALAC parity over independently generated depths and a real-world M4A/CAF corpus; retain strict AAC/DRM denial.
3. Bounded metadata, safe cover art exclusion from the tiny framebuffer, exact seek.
4. Hardware output negotiation, anti-resampling checks, bit-perfect DAC loopback and shared Radio/TV focus.
5. Add WavPack lossless, optional Ogg FLAC and direct DSD only with reference PCM/bitstream conformance tests. RF64/ds64 source support exists, with broad real-file and hardware acceptance still open.

## Standards used for the format policy (research only)

- Apple AIFF format: https://ich.music.mcgill.ca/classes/synth/AudioIFF1.2.1/AudioIFF1.2.1.html
- McGill AIFF/AIFF-C format survey: https://www-mmsp.ece.mcgill.ca/MMSP/Documents/AudioFormats/AIFF/AIFF.html
- IETF RFC 9639 FLAC: https://www.rfc-editor.org/rfc/rfc9639
- Apple CAF specification: https://developer.apple.com/library/archive/documentation/MusicAudio/Reference/CAFSpec/CAF_spec/CAF_spec.html
- Apple Core Audio channel-layout tags: https://developer.apple.com/documentation/coreaudiotypes/audio-channel-layout-tags
- Apple Audio File Types: https://developer.apple.com/documentation/audiotoolbox/1576497-audio-file-types
- WavPack lossless/hybrid rules: https://www.wavpack.com/wavpack_doc.html

## G10 offline quality-gate repair (2026-10-29)

An operator's macOS gate log reports that the local-only dependency audit passed:
**12 local crates; 21 path-only edges; zero registry/Git crates; zero third-party system links; one approved Apple framework link**. The subsequent `cargo fmt --all -- --check` failed and printed rustfmt diffs across 14 Rust source files. The gate exits at that check; Clippy, complete workspace tests, release builds, and physical audio are not established by this log.

The exact 252 rustfmt hunks supplied by the operator were matched against GitHub source and applied without intentionally changing decoder algorithms. Three compiler warnings for unnecessary Boolean-expression parentheses in `crates/rust-music-audit/src/main.rs` were also removed. The changes are published in four commits ending at `13f8e85975f7b4cae921593df2ab0e7f310b8d30`. A GitHub commit comparison confirms only the expected 14 Rust files changed.

At the time of that operator report, the full gate had not passed. A subsequent local gate run is recorded below. The earlier operator run command was:

    cd /Users/null/GitHub/rust_music
    git pull --ff-only origin build/native-codecs-g0
    sh scripts/offline-gate.sh

Keep PR #1 draft until private FLAC corpus, actual shared audio focus and device gates pass.

## G10 local offline quality-gate follow-up (2026-10-29)

The current PR head `b8c7de527fa3c0cfb95a33000a3e71f51df842e5` plus current local worktree changes was checked with Rust/Cargo 1.99.1 on macOS. `sh scripts/offline-gate.sh` exited **0**. The gate passed the dependency audit (12 local crates, 21 local edges, zero registry/Git crates), rustfmt, Clippy with warnings denied, workspace unit tests, the optimized workspace build, and the macOS output-feature check. The current host regression also verifies that a sink-open failure releases the host focus lease.

Additional local native CLI parity evidence:

- `python3 scripts/test_wave_reference.py` exited **0**: 6/6 WAVE vectors matched exact PCM; RIFF overrun, missing odd pad and nonzero extensible low bits were rejected.
- `python3 scripts/test_aiff_reference.py` exited **0**: 7/7 AIFF/AIFF-C vectors matched exact PCM; corrupt FORM, contradictory COMM frame count, truncation and lossy AIFF-C were rejected.
- `CARGO_NET_OFFLINE=true python3 scripts/test_flac_reference.py` exited **0**: 7/7 generated FLAC vectors matched exact PCM, and each rejected altered STREAMINFO MD5, CRC16 corruption and truncation.

These results establish local build, generated-vector parity and six-track private FLAC reference/native parity. No Rust_Boy/Rust_Radio/Rust_TV integration, physical DAC loopback, device underrun/drain measurement or bit-perfect claim is established. Keep PR #1 draft.

