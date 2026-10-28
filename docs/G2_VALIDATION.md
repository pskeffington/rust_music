# G2 native FLAC conformance ledger

Status: **implementation available for review; not fully certified** (2026-10-28).

## Local observed results

- `python3 scripts/check_literature.py` → PASS; 17 cited sources, 14 indexed engineering standards; markdown registry parity.
- `python3 scripts/test_flac_reference.py --fixtures-only` → PASS; seven FLAC 1.5.0 reference encodes verified with `flac -t`, byte-count and STREAMINFO MD5 checked against independently generated signed PCM.
- The fixture set exercises 8-, 16-, 24- and 32-bit integer samples; mono, stereo and six-channel input; constant, chirp, ramp and noise signals.
- `cargo test --workspace`, `cargo fmt --all -- --check`, `cargo clippy ...` and the **Rust** PCM differential path were not run locally: no Rust compiler/toolchain was available in this execution environment.
- GitHub Actions runs 40786189605 and 40786224969 returned `startup_failure`, and each reported zero scheduled jobs. That is an **infrastructure startup blocker**, not evidence of decoder compilation failure or success. Investigate repository Actions permissions, runner availability and account billing before relying on CI.

## What source implements

- RFC 1321 incremental MD5 using project-owned Rust code, fed signed little-endian interleaved PCM with sign-extension to whole bytes per RFC 9639.
- CRC8 for the FLAC frame header and CRC16 for the full frame before returning audio; signed 32-bit residual value bounds and Rice partition limits.
- Frame/sample index continuity checks, canonical frame-number encoding, per-block and aggregate metadata limits.
- Nonzero STREAMINFO MD5 checked only after complete decoding at EOF; a zero digest means no MD5 validation is available. MD5 is not an authenticity mechanism.

## Required acceptance commands

```sh
python3 scripts/check_literature.py
python3 scripts/test_flac_reference.py --fixtures-only
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python3 scripts/test_flac_reference.py
```

The reference differential script compares WAV PCM byte-for-byte and explicitly rejects changed STREAMINFO MD5, CRC16 footer and truncation. Do not mark any of those checks passed until the `cargo` path executes successfully. Next, pin the official IETF `flac-test-files` corpus revision and capture per-fixture accepted/rejected status and duration; then measure actual output device bit correctness in G3.
