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
| .flac | Native FLAC | Source implemented; cargo and native PCM parity pending | Stream CRC + decoded-PCM MD5 at EOF, 16-bit/44.1k+ for playback |
| .wav | RIFF/WAVE signed integer PCM or 8-bit unsigned PCM | Source implemented; cargo parity pending | PCM WAVE_FORMAT_EXTENSIBLE supported; 8-bit only in decoder tests |
| .aif, .aiff | AIFF signed integer PCM | New G10 implementation; cargo parity pending | 8/16/24/32-bit big-endian, exact extended-rate parsing |
| .aifc | AIFF-C signed PCM | New G10 implementation; cargo parity pending | Valid FVER and NONE, twos or sowt only; no ADPCM/ulaw/alaw |
| .m4a | ALAC | Planned; no native decoder yet | AAC in identical container prohibited |
| .caf | ALAC or verified integer PCM | Planned | Reject other CAF codecs |
| .wv | WavPack pure lossless or hybrid with verified correction | Planned | Reject lossy-only hybrid files |
| RF64/BWF .wav | Extended large-file PCM | Planned | ds64 and exact bounds required |
| .oga / .ogg | Ogg FLAC mapping | Later | Reject Vorbis and Opus |
| .dsf, .dff | DSD | Optional native-device path | Never claim DSD-bit-perfect after PCM conversion |
| .ape | Monkey's Audio | Optional research | Local decoder and parity required |
| .mp3 / AAC / Opus / Vorbis / lossy WMA | Lossy | Excluded | Never transcode for fallback |

The current library scan is a fast filename-extension and magic-byte probe, not exhaustive codec verification. An indexed item can be rejected during decoder initialization or format validation; rejected audio is not played.

## Native AIFF implementation

A new locally owned crate, rust-music-aiff, decodes AIFF and uncompressed AIFF-C signed integer PCM without external codec libraries. The facade sniffs the binary FORM marker and accepts AIFF/AIFC only through this decoder.

Parser rules: checked FORM length, bounded IFF chunks, odd-byte padding, max 512 metadata headers, max 16 MiB per skipped metadata chunk, max 64 MiB aggregate skipped metadata, checked SSND offset and data length, and equality of SSND audio bytes with COMM declared frames × channels × sample width. Nonzero SSND block-alignment schemes are explicitly unsupported. The parser produces at most 4,096 PCM frames per block and preserves signed 8/16/24/32-bit samples. AIFF-C compressed codec identifiers fail closed.

The AIFF 80-bit extended rate is decoded with integer math and must represent an exact whole number of Hz, never rounded. The renderer labels AIFF and remains exactly 128×64 pixels / 1,024 bytes.

Known limits: chunks following the audio SSND are not exhaustively parsed; source implementation is not yet compiled here. The quick scanner may include a provisional file that the full decoder later rejects.

## Reproduction and evidence

Python-stdlib-only generated reference vectors cover uncompressed AIFF and AIFF-C, 16/24/32-bit PCM, endian swap, SSND offsets, odd-length chunks, FORM overrun, truncation, wrong frame counts and lossy codec rejection. An 8-bit parser-only fixture is deliberately not an accepted playback format.

    python3 scripts/test_aiff_reference.py --fixtures-only
    python3 scripts/test_aiff_reference.py
    sh scripts/offline-gate.sh
    python3 scripts/test_flac_user_corpus.py --mode rust --require-complete /private/music

The second and subsequent commands require a locally installed Rust toolchain; the last requires the original private six-track FLAC files. No recordings or encoded artwork are committed. Do not mark Rust compile, full PCM parity or physical DAC acceptance as PASS until they have run. Keep PR #1 draft.

## Next priorities

1. Own ALAC packet decoder and strict M4A/MP4 demux, including correct ALAC sample-description/magic-cookie recognition and mandatory AAC rejection.
2. Own CAF ALAC/lpcm demux, no OS codec fallback.
3. Bounded metadata, safe cover art exclusion from the tiny framebuffer, exact seek.
4. Hardware output negotiation, anti-resampling checks, bit-perfect DAC loopback and shared Radio/TV focus.
5. Add WavPack lossless, RF64, optional Ogg FLAC and direct DSD only with reference PCM/bitstream conformance tests.

## Standards used for the format policy (research only)

- Apple AIFF format: https://ich.music.mcgill.ca/classes/synth/AudioIFF1.2.1/AudioIFF1.2.1.html
- McGill AIFF/AIFF-C format survey: https://www-mmsp.ece.mcgill.ca/MMSP/Documents/AudioFormats/AIFF/AIFF.html
- IETF RFC 9639 FLAC: https://www.rfc-editor.org/rfc/rfc9639
- Apple CAF specification: https://developer.apple.com/library/archive/documentation/MusicAudio/Reference/CAFSpec/CAF_spec/CAF_spec.html
- Apple Audio File Types: https://developer.apple.com/documentation/audiotoolbox/1576497-audio-file-types
- WavPack lossless/hybrid rules: https://www.wavpack.com/wavpack_doc.html
