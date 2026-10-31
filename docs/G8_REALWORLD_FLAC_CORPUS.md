# G8: Real-world FLAC differential corpus (private audio, public checksums)

Observed 2026-10-29. Test data are six **user-provided FLAC recordings**, intentionally **not** included in the public Git repository. No permission to redistribute the recordings is assumed. `.gitignore` excludes `*.flac` and `*.wav`. The manifest stores SHA-256 digests and technical parameters, not encoded/decoded audio or artwork.

## Verified local evidence

| Case | Encoded bytes | Frames/channel | Duration | `flac -t` | FLAC vs FFmpeg raw s16le | Rust_Music native PCM |
| --- | ---: | ---: | ---: | --- | --- | --- |
| L1 | 3,836,026 | 2,981,991 | 1:07.619 | PASS | exact SHA-256 match | exact SHA-256 match |
| L2 | 11,239,886 | 6,231,348 | 2:21.300 | PASS | exact SHA-256 match | exact SHA-256 match |
| L3 | 3,482,121 | 1,942,524 | 0:44.048 | PASS | exact SHA-256 match | exact SHA-256 match |
| G4 | 15,801,340 | 8,540,061 | 3:13.652 | PASS | exact SHA-256 match | exact SHA-256 match |
| G5 | 5,130,940 | 3,203,723 | 1:12.647 | PASS | exact SHA-256 match | exact SHA-256 match |
| G6 | 5,975,108 | 4,411,670 | 1:40.038 | PASS | exact SHA-256 match | exact SHA-256 match |
| **Total** | **45,465,421** | **27,311,317** | **10:19.304** | **6/6** | **6/6** | **6/6** |

All recordings: native FLAC, 44,100 Hz, 16-bit, 2 channels. Each contains STREAMINFO, SEEKTABLE, VORBIS_COMMENT and PICTURE metadata blocks. Total independently decoded PCM: 109,245,268 bytes (104.18 MiB). `flac -t` verified the reference streams' frame integrity and STREAMINFO MD5. `flac --decode ...` and `ffmpeg -f s16le` produced identical raw PCM hashes, stored in [`tests/corpus/guiezio-flac-sha256.json`](../tests/corpus/guiezio-flac-sha256.json).

**Reference evidence:** the earlier reference-only run used `flac` and FFmpeg solely as test oracles; neither is linked or required by Rust_Music.

**Current decoder checkpoint:** On 2026-10-29, the authorized run
`CARGO_NET_OFFLINE=true python3 scripts/test_flac_user_corpus.py --mode all --require-complete <six supplied file paths>`
exited 0. All six files passed reference FLAC integrity, FLAC/FFmpeg PCM
equality, compressed-file SHA-256 and STREAMINFO/metadata checks, Rust CLI
inspection, and complete Rust decoded PCM SHA-256 and byte-count parity
against the manifest. The six recordings were read from their supplied
locations; none were copied into the worktree or committed. Separately,
`CARGO_NET_OFFLINE=true python3 scripts/test_flac_reference.py` passed seven
generated native Rust PCM vectors with digest, CRC and truncation rejection.

## Reproduce with the locally retained recordings

## Revalidation on 2026-10-31

The exact six manifest-matched user files were supplied again in a local
operator corpus and passed the clean-branch premerge gate at Rust Music commit
`0bc3f1673a635e688b928406c63baa46bbbb22da`. The run checked the complete
offline gate, WAVE (6/6), AIFF/AIFF-C (7/7), and native Rust PCM/metadata/hash
parity for L1, L2, L3, G4, G5, and G6. The premerge script reached its final
automated-check PASS; its non-certification notice remains applicable to
Rust_Boy shared focus, physical panel/DAC and release acceptance. The operator
corpus and captured private gate log remain outside Git.

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
- **PASS**: Rust_Music native `inspect` and complete decoded PCM parity on all six supplied files, confirmed in the combined `--mode all` run. The private recordings remain outside the repository.
- **NOT TESTED**: any audible playback, DAC bit-perfect status, 24/32-bit real recordings, multichannel real recordings, Ogg FLAC, seeking or MP4/ALAC.
- **SCOPE**: audio files are never shipped, committed, bundled, or fetched by the normal build or test. Only local operators with these exact recordings can rerun these six vectors; separately generated public-domain/synthetic test fixtures remain necessary for universal CI.

## Next targeted pass

The offline Cargo gate and all six private-file native PCM parity checks have passed locally. Next, qualify shared host audio-focus and target panel behavior, then run long-playback and physical output tests. Do not claim bit-perfect DAC output unless it is physically measured.
