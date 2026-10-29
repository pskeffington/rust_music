# External FLAC corpus acceptance (G8)

Date: 2026-10-29. Six user-supplied real-world FLAC files were tested **locally** against the reference `flac -t -s` integrity checker. The user-provided audio and embedded artwork are **not** committed to this repository.

## Preflight result
- 6/6 files passed the reference FLAC integrity check (frame CRC and STREAMINFO PCM MD5 verified by the reference decoder).
- All 6: native FLAC, 44,100 Hz, 16-bit, stereo.
- All 6 contain embedded image metadata. Parsing must skip large metadata blocks while respecting allocation limits.
- Total: 45,465,421 bytes of FLAC data; 27,311,317 PCM sample frames across the corpus.
- The six private files pass combined reference and native Rust_Music validation: `CARGO_NET_OFFLINE=true python3 scripts/test_flac_user_corpus.py --mode all --require-complete <six supplied file paths>` exited 0. Reference FLAC integrity, FLAC/FFmpeg PCM equality, Rust inspection, complete PCM hashes and byte counts all matched. Seven separately generated FLAC vectors also pass native Rust parity.

## Reproducible test without shipping music

```sh
python3 scripts/test_external_flac.py --reference-only "/local/music/"*.flac
python3 scripts/test_external_flac.py --report /tmp/rust-music-corpus-results.json "/local/music/"*.flac
```

The second command runs `rust-music-cli decode` through `cargo run --offline --locked`, uses the reference FLAC decoder to generate signed, little-endian PCM, and compares SHA-256 of every decoded PCM byte plus audio format and declared sample count. For 8-bit WAV output the harness converts the unsigned samples back into signed PCM before comparison. It exits nonzero on any incomplete or failed case. Reference `flac` is **test-only**: no runtime dependency is added.

Use a preinstalled Rust toolchain and local reference FLAC executable. Do not add copyrighted source tracks, decoded PCM, cover art, or exact source paths to version control. Hashes may be kept in private operator audit records to match previously tested inputs; do not represent hashes as permission to redistribute any audio.

## Next gates
1. Test decode with an artwork-heavy file and a metadata size above the configured limit; report explicit rejection instead of silently skipping a failed file.
2. Add fuzzed/truncated copies generated only in a local temporary directory; retain no distributed source material.
3. Integrate and exercise the host-owned shared audio-focus bridge.
4. Run physical DAC tests separately; PCM parity alone cannot certify bit-perfect hardware output.
