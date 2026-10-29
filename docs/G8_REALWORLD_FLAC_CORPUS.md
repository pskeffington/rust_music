# G8: Real-world FLAC differential corpus (private audio, public checksums)

Observed 2026-10-29. Test data are six **user-provided FLAC recordings**, intentionally **not** included in the public Git repository. No permission to redistribute the recordings is assumed. `.gitignore` excludes `*.flac` and `*.wav`. The manifest stores SHA-256 digests and technical parameters, not encoded/decoded audio or artwork.

## Verified local evidence

| Case | Encoded bytes | Frames/channel | Duration | `flac -t` | FLAC vs FFmpeg raw s16le |
| --- | ---: | ---: | ---: | --- | --- |
| L1 | 3,836,026 | 2,981,991 | 1:07.619 | PASS | exact SHA-256 match |
| L2 | 11,239,886 | 6,231,348 | 2:21.300 | PASS | exact SHA-256 match |
| L3 | 3,482,121 | 1,942,524 | 0:44.048 | PASS | exact SHA-256 match |
| G4 | 15,801,340 | 8,540,061 | 3:13.652 | PASS | exact SHA-256 match |
| G5 | 5,130,940 | 3,203,723 | 1:12.647 | PASS | exact SHA-256 match |
| G6 | 5,975,108 | 4,411,670 | 1:40.038 | PASS | exact SHA-256 match |
| **Total** | **45,465,421** | **27,311,317** | **10:19.304** | **6/6** | **6/6** |

All recordings: native FLAC, 44,100 Hz, 16-bit, 2 channels. Each contains STREAMINFO, SEEKTABLE, VORBIS_COMMENT and PICTURE metadata blocks. Total independently decoded PCM: 109,245,268 bytes (104.18 MiB). `flac -t` verified the reference streams' frame integrity and STREAMINFO MD5. `flac --decode ...` and `ffmpeg -f s16le` produced identical raw PCM hashes, stored in [`tests/corpus/guiezio-flac-sha256.json`](../tests/corpus/guiezio-flac-sha256.json).

**This is not Rust_Music decoder acceptance:** `cargo`/`rustc` are absent in the examination environment. Reference programs were used solely as test oracles and are not linked or required by Rust_Music.

## Reproduce with the locally retained recordings

```sh
# Reference integrity, full FLAC/FFmpeg PCM parity. Test-only external executables.
python3 scripts/test_flac_user_corpus.py --mode oracle --require-complete /private/music

# Real Rust_Music decoder; entirely offline; needs only local Cargo/Rust toolchain.
# Compares complete integer PCM output with expected hashes in manifest.
python3 scripts/test_flac_user_corpus.py --mode rust --require-complete /private/music

# Both paths: do not report a full decoder PASS unless this completes successfully.
python3 scripts/test_flac_user_corpus.py --mode all --require-complete /private/music
```

The script matches files by their entire compressed-file SHA-256 rather than by names or order (so UTF-8 filenames and renamed files work), validates STREAMINFO and expected metadata profiles, streams both reference decoders into hash functions without storing raw audio, and decodes Rust output into a temporary WAV which is removed after checking. Unknown or duplicate input digests fail closed; `--require-complete` insists on all six cases. Optional `--report /path/report.json` writes an audit report containing only hashes and technical evidence, never music files.

## Acceptance distinctions and limitations

- **PASS**: supplied original files verified by FLAC; `flac` and FFmpeg reference PCM output match bit-for-bit on all six tracks; manifest/profile integrity checks; reference-only harness.
- **BLOCKED**: actual `rust-music-cli` compilation, `inspect`, and decoded PCM parity. Do not count this corpus as passing the Rust native decoder until `--mode rust`/`--mode all` is run successfully on a machine with Cargo.
- **NOT TESTED**: any audible playback, DAC bit-perfect status, 24/32-bit real recordings, multichannel real recordings, Ogg FLAC, seeking or MP4/ALAC.
- **SCOPE**: audio files are never shipped, committed, bundled, or fetched by the normal build or test. Only local operators with these exact recordings can rerun these six vectors; separately generated public-domain/synthetic test fixtures remain necessary for universal CI.

## Next targeted pass

Run the existing offline Cargo gate, then this corpus in `--mode rust` to diagnose any frame/metadata errors. Compare failing files one at a time and preserve the original checksums. If Rust parity succeeds, run long-playback and audio-focus tests without claiming bit-perfect DAC output unless physically measured.
