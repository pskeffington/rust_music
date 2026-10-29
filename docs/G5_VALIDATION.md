# Rust Music G5 headless host foundation: validation ledger

Recorded 2026-10-29. New source is unmerged, and all tests that require an installed Rust toolchain remain **pending**, not passed.

| Check | Result | Evidence / reproducible command |
| --- | --- | --- |
| Workspace closure | PASS (static) | Ten Cargo workspace crates; Cargo.lock lists the same ten; all internal dependencies are relative workspace paths; no registry/source/checksum entries |
| Literature matrix integrity | PASS | `python3 scripts/check_literature.py` (19 sources, 16 rules, exact generated markdown parity) |
| Reference FLAC fixture validity | PASS (reference-only) | `python3 scripts/test_flac_reference.py --fixtures-only` (seven FLAC files, 8/16/24/32-bit mono/stereo/six-channel, reference MD5) |
| Bounded library tests | NOT RUN | `cargo test --locked --offline -p rust-music-library` (signature detection, symlink avoidance, budget failure, stable ordering) |
| Host slot and exclusive focus tests | NOT RUN | `cargo test --locked --offline -p rust-music-host` (no autoplay, EOF/Stop/Drop release, focus denial) |
| Playback engine failure tests | NOT RUN | `cargo test --locked --offline -p rust-music-engine` (format mismatch and drain failure safety) |
| All workspace fmt, clippy, test and build | BLOCKED | `sh scripts/offline-gate.sh` requires preinstalled Cargo/Rust 1.85+ and lint components. They are absent in this execution environment |
| Rust_Radio/Rust_TV real audio focus | PENDING | Rust_Boy integration uses same arbiter; not yet implemented in parent shell |
| Physical audio / bit-perfect output | PENDING | Actual macOS/Linux DAC/loopback measurement; not implied by NullSink or structural tests |

## Operational requirements

- The `rust-music-host` source does not call `try_acquire_music` until the host explicitly issues `Play`. Repeated Play while already playing is idempotent.
- `Command::Next`, `Previous`, `Select` and `Stop` close the sink and release the host-owned RAII lease, without autoplay.
- The library scanner has no background threads and does not follow symbolic links. A local hostile filesystem TOCTOU race remains possible and requires a platform-specific no-follow open API when that threat model applies.
- `Poll` must execute on a dedicated worker, not on the GUI or audio device callback.
- `AudioLease` drop is guaranteed by Rust ownership only when Rust_Boy's implementation correctly releases the shared claim in its own Drop.

## Next gate

Provision a preinstalled offline toolchain, run `sh scripts/offline-gate.sh`, repair actual compiler warnings/errors, then verify real cross-module audio focus and DAC behavior. Do not merge or claim bit-perfect until acceptance evidence is recorded.
