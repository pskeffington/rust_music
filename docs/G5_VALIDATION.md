# Rust Music G5 headless host foundation: validation ledger

Recorded 2026-10-29. Local workspace build, Clippy, tests and release build pass; actual Rust_Boy/Rust_Radio/Rust_TV arbitration and physical audio remain **unverified**.

| Check | Result | Evidence / reproducible command |
| --- | --- | --- |
| Workspace closure | PASS (static and offline audit) | 16 Cargo workspace crates; 29 internal path edges; no registry/Git crates |
| Literature matrix integrity | PASS | `python3 scripts/check_literature.py` (20 sources, 16 rules, exact generated markdown parity) |
| Reference FLAC fixture validity | PASS (reference-only) | `python3 scripts/test_flac_reference.py --fixtures-only` (seven FLAC files, 8/16/24/32-bit mono/stereo/six-channel, reference MD5) |
| Bounded library tests | PASS | Local `sh scripts/offline-gate.sh`; symlink avoidance, scan limits, stable ordering |
| Host slot and exclusive focus tests | PASS (mock focus) | Local `sh scripts/offline-gate.sh`; no autoplay, EOF/Stop/Drop release, denial before output, and device-open failure releases the host lease |
| Playback engine failure tests | PASS | Local `sh scripts/offline-gate.sh`; format mismatch, low-quality denial, and drain failure safety |
| All workspace fmt, clippy, test and build | PASS locally | Rust/Cargo 1.99.1/macOS; audit, rustfmt, Clippy, workspace tests, release build and macOS feature check |
| Rust_Radio/Rust_TV real audio focus | PENDING | Unit tests use a mock lease; actual shared host arbiter integration is not in this repository |
| Physical audio / bit-perfect output | PENDING | Actual macOS/Linux DAC/loopback measurement; not implied by NullSink or structural tests |

## Operational requirements

- The `rust-music-host` source does not call `try_acquire_music` until the host explicitly issues `Play`. Repeated Play while already playing is idempotent.
- `Command::Next`, `Previous`, `Select` and `Stop` close the sink and release the host-owned RAII lease, without autoplay.
- The library scanner has no background threads and does not follow symbolic links. A local hostile filesystem TOCTOU race remains possible and requires a platform-specific no-follow open API when that threat model applies.
- `Poll` must execute on a dedicated worker, not on the GUI or audio device callback.
- `AudioLease` drop is guaranteed by Rust ownership only when Rust_Boy's implementation correctly releases the shared claim in its own Drop.

## Next gate

The six supplied private FLACs pass whole-track native/reference PCM parity and
exact range parity at all 132 populated SEEKTABLE positions and their following
samples. The next G5 acceptance is host-side shared focus arbitration across
Radio/TV/Music; physical DAC qualification remains separate. Do not merge or
claim bit-perfect until the corresponding evidence is recorded.
