# Rust Music G5 headless host foundation: validation ledger

Recorded 2026-10-29; updated 2026-10-31. Local workspace build, Clippy, tests and release build pass; actual Rust_Boy/Rust_Radio/Rust_TV arbitration and physical audio remain **unverified**.

| Check | Result | Evidence / reproducible command |
| --- | --- | --- |
| Workspace closure | PASS (static and offline audit) | 16 Cargo workspace crates; 29 internal path edges; no registry/Git crates |
| Literature matrix integrity | PASS | `python3 scripts/check_literature.py` (21 sources, 16 rules, exact generated markdown parity) |
| Reference FLAC fixture validity | PASS (reference-only) | `python3 scripts/test_flac_reference.py --fixtures-only` (seven FLAC files, 8/16/24/32-bit mono/stereo/six-channel, reference MD5) |
| Bounded library tests | PASS | Local `sh scripts/offline-gate.sh`; symlink avoidance, scan limits, stable ordering |
| Host slot and exclusive focus tests | PASS (mock focus) | Local `sh scripts/offline-gate.sh`; no autoplay, EOF/Stop/Drop release, denial before output, device-open/write failures, explicit-stop/EOF drain failures, suspend, and catalog replacement release the host lease |
| Playback engine failure tests | PASS | Local `sh scripts/offline-gate.sh`; format mismatch, low-quality denial, and drain failure safety |
| All workspace fmt, clippy, test and build | PASS locally | Rust/Cargo 1.99.1/macOS; audit, rustfmt, Clippy, workspace tests, release build and macOS feature check |
| Pip-Boy MUSIC package identity | PASS (reference-only) | Pip-Boy commit `bccd59d` adds `org.rustboy.music` as a separate manifest with `media.audio`, no background permission and `design_fixture_not_installed`; registry fixture and five-identity lifecycle tests pass |
| Pip-Boy host audio-focus foundation | PASS (partial integration) | Pip-Boy `pipboy-core::audio_focus` passes 2 lease tests; live Radio output startup/replacement acquires `HOST_AUDIO_FOCUS`; app/core test suites pass 77/77 and 283/283 |
| Bounded package process framing | PASS (codec/endpoint source) | Rust Music `rust-music-ipc` and `rust-music-cli serve` implement strict `RBPK` v1 frames, shared hello vector, identity/sequence checks, no-autoplay activation, and lazy output creation. In-memory serve-loop tests reject a wrong package identity, Activate before Hello, and skipped request IDs without creating a sink. Spawned child behavior is recorded separately below. |
| Spawned Rust Music process lifecycle | PASS (macOS local subprocess) | `cargo test --locked --offline -p rust-music-cli --features macos-output --test package_process` launches the Cargo-built CLI executable and verifies hello, inert activation, shutdown acknowledgment, clean child exit, and fail-closed behavior for a well-formed Hello with the wrong package ID (no reply; unsuccessful exit within the test deadline). This is not a packaged release binary or live Pip-Boy mount. |
| Release Rust Music unit protocol smoke | PASS (local macOS release binary) | `cargo build --locked --offline --release -p rust-music-cli --features macos-output` followed by `python3 scripts/test_release_package_process.py --binary target/release/rust-music-cli` completed RBPK v1 Hello, inert Activate (19-byte empty/no-audio view), and Shutdown, then observed clean exit and empty stderr. SHA-256 `d1de04c2d8e3af36b75fa5fd5eb8147245144db87dbb6ab485170fd58beb3ffa`. The binary is not registered in Pip-Boy's trusted executable roster or mounted by live startup. |
| Supervised package process runner | IMPLEMENTED LOCALLY; focused tests reported PASS | Pip-Boy's `pipboy-core::package_process` contains fixed-path launch, deadlines, timeout kill/reap, stderr drain, process-exit focus cleanup, and shutdown handling. Its `docs/PACKAGE_PROCESS_PROTOCOL.md` records the targeted supervisor tests as passing. This Rust Music session inspected the implementation and ledger but did not rerun those tests. |
| Production package mount and acceptance | PENDING | Pip-Boy's reviewed executable roster is empty; live MUSIC registration/mount, host-supervised release binary, shared audio contention, shell/mobile acceptance and platform process behavior remain open. The supervisor is fault containment, not an OS sandbox. |
| Standalone macOS app bundle and UI exercise | PASS (local offline build, verified signature, LaunchServices launch and child lifecycle) | At clean local commit `bbd5dc3c52d3e187a1a332010f07d2a72906766c`, `sh scripts/build-macos-app.sh` exited 0. The script removes the generated bundle's stale `_CodeSignature`, targets the declared macOS 13.0 minimum, ad-hoc signs the completed bundle, and requires `codesign --verify --deep --strict`; the final app and plist verify, with app `minos 13.0`, arm64, executable mode 755, and `PkgInfo=APPL????`. A 2026-10-31 hardening change rejects symlinked/non-directory bundle path components before build mutation; `sh -n` and a fresh app rebuild plus strict signature verification exited 0. After unregistering only the generated bundle (`lsregister -u`, exit 0), unsandboxed `/usr/bin/open -n` by bundle path auto-registered and launched it (exit 0); unified logs showed LaunchServices CHECKIN for `dev.rustmusic.standalone`. The desktop UI inspected generated 16-bit/44.1 kHz silent WAV, exercised Play/Stop and window close, and Activity Monitor showed no `rust-music-cli` process after close. Earlier UI exercise refused a generated MP3. The four `package_process` tests and parent-death script passed as recorded below. `spctl --assess` rejects the local ad-hoc signature; Developer ID/notarization and distribution Gatekeeper acceptance remain unverified. Watchdog escalation does not prove graceful AudioQueue teardown; device disconnect/recovery and physical DAC remain separate hardware gates. RM-019 local acceptance is complete; hardware and distribution evidence is not implied. |

## 2026-10-31 local integration pass

- Verified checkout at `62e44a77ed632bf7925b30e0993a5f87728a2696` on
  `build/native-codecs-g0`; it was clean before the catalog worker's scoped
  change and had 48 local commits ahead of the stale local tracking ref.
- `cargo test --locked --offline -p rust-music-library` exited 0 (9/9),
  `cargo fmt --all -- --check` exited 0, and `git diff --check` exited 0.
  Independent catalog review found no blocker; it notes IDs are not
  authentication tokens and query sorts the bounded full catalog.
- `sh scripts/offline-gate.sh` exited 0 after the catalog change: 17 local
  crates, 32 local path edges, zero registry/Git crates, Rustfmt,
  warnings-denied Clippy, workspace tests, release build, and macOS feature
  check.
- Hardened the macOS app builder against symlinked/non-directory generated
  bundle path components. `sh -n scripts/build-macos-app.sh` and a fresh
  `sh scripts/build-macos-app.sh` exited 0; strict code-signature verification
  passed as part of the build.
- No private FLAC corpus was rerun. Existing 6/6 FLAC, 6/6 WAVE and 7/7
  AIFF evidence remains tied to the earlier recorded checkpoint. No new
  LaunchServices UI or physical device test was performed in this pass.
| Rust Music shared lease bridge / TV ownership | PENDING | Music adapter/user-input/worker bridge and TV use of the same broker are not implemented; no real Music-vs-Radio/TV contention has been exercised |
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
samples. The first host-side focus phase is now implemented in Pip-Boy: RADIO
holds a shared in-process lease while its output session is active. Next G5
acceptance is a distinct MUSIC adapter using that exact broker, TV ownership,
user-authorized input, worker polling, and contention/unload tests. Physical DAC
qualification remains separate. The bounded package protocol and supervised
runner source now exist; a trusted packaged executable and live MUSIC mount
remain required for end-to-end acceptance. Do not merge or claim bit-perfect
until the corresponding evidence is recorded.
