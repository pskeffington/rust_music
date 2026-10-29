# G2 native FLAC conformance ledger

Status: **implementation compiled and passed generated and six-track real-world native parity; hardware certification and broader conformance remain open** (updated 2026-10-29).

## Local observed results

- `python3 scripts/check_literature.py` → PASS; 19 references, 16 standards; exact Markdown registry parity.
- `python3 scripts/test_flac_reference.py --fixtures-only` → PASS; seven FLAC 1.5.0 reference encodes verified with `flac -t`, byte-count and STREAMINFO MD5 checked against independently generated signed PCM.
- The fixture set exercises 8-, 16-, 24- and 32-bit integer samples; mono, stereo and six-channel input; constant, chirp, ramp and noise signals.
- Local `sh scripts/offline-gate.sh` later passed on Rust/Cargo 1.99.1 and macOS, including fmt, Clippy, all workspace tests, release build and the macOS feature check.
- `CARGO_NET_OFFLINE=true python3 scripts/test_flac_reference.py` passed all seven generated reference cases through the native Rust CLI, with exact PCM comparisons and rejection of changed STREAMINFO MD5, CRC16 footer corruption and truncation for each case.
- `CARGO_NET_OFFLINE=true python3 scripts/test_flac_user_corpus.py --mode all --require-complete <six supplied file paths>` → PASS; all six tracks passed reference FLAC integrity, FLAC/FFmpeg PCM equality and native Rust full-PCM hash/byte-count parity. Audio remains outside the worktree and must not be copied into Git or uploaded.
- GitHub Actions runs 40786189605 and 40786224969 returned `startup_failure`, and each reported zero scheduled jobs. That is an **infrastructure startup blocker**, not evidence of decoder compilation failure or success. Investigate repository Actions permissions, runner availability and account billing before relying on CI.

## What source implements

- RFC 1321 incremental MD5 using project-owned Rust code, fed signed little-endian interleaved PCM with sign-extension to whole bytes per RFC 9639.
- CRC8 for the FLAC frame header and CRC16 for the full frame before returning audio; signed 32-bit residual value bounds and Rice partition limits.
- Frame/sample index continuity checks, canonical frame-number encoding, per-block and aggregate metadata limits.
- Nonzero STREAMINFO MD5 checked only after complete decoding at EOF; a zero digest means no MD5 validation is available. MD5 is not an authenticity mechanism.

## Required acceptance commands

```sh
python3 scripts/check_literature.py
sh scripts/offline-gate.sh
CARGO_NET_OFFLINE=true python3 scripts/test_flac_reference.py
```

The reference differential script compares WAV PCM byte-for-byte and explicitly rejects changed STREAMINFO MD5, CRC16 footer and truncation. Generated vectors and the six-track private corpus now pass locally; do not generalize that result to other files or physical output. Next, pin the official IETF `flac-test-files` corpus revision and measure actual output device behavior in G3.
