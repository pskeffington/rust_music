# Rust Music G5 headless host foundation: validation ledger

Recorded 2026-10-29; updated 2026-10-31. Local workspace build, Clippy, tests and release build pass. Actual Rust_Boy/Rust_Radio/Rust_TV arbitration remains **unverified**. A physical headphone unplug/reconnect during active playback observed macOS default-route switching and return; acoustic continuity, negotiated DAC output and an AudioQueue invalid-device recovery remain unverified.

| Check | Result | Evidence / reproducible command |
| --- | --- | --- |
| Workspace closure | PASS (static and offline audit) | `sh scripts/offline-gate.sh` at `a65fbe9`: 17 local crates, 32 path-only internal edges, no registry/Git crates, one Apple OS framework link |
| Literature matrix integrity | PASS | `python3 scripts/check_literature.py` (21 sources, 16 rules, exact generated markdown parity) |
| Reference FLAC fixture validity | PASS (reference-only) | `python3 scripts/test_flac_reference.py --fixtures-only` (seven FLAC files, 8/16/24/32-bit mono/stereo/six-channel, reference MD5) |
| Bounded library tests | PASS | Local `sh scripts/offline-gate.sh`; symlink avoidance, scan limits, stable ordering |
| Host slot and exclusive focus tests | PASS (mock focus) | Local `sh scripts/offline-gate.sh`; no autoplay, EOF/Stop/Drop release, denial before output, device-open/write failures, explicit-stop/EOF drain failures, suspend, and catalog replacement release the host lease |
| Playback engine failure tests | PASS | Local `sh scripts/offline-gate.sh`; format mismatch, low-quality denial, and drain failure safety |
| All workspace fmt, clippy, test and build | PASS locally | Rust/Cargo 1.99.1/macOS; audit, rustfmt, Clippy, workspace tests, release build and macOS feature check |
| Pip-Boy MUSIC package identity | PASS (reference-only) | Pip-Boy commit `bccd59d` adds `org.rustboy.music` as a separate manifest with `media.audio`, no background permission and `design_fixture_not_installed`; registry fixture and five-identity lifecycle tests pass |
| Pip-Boy host audio-focus foundation | PASS (partial integration) | Pip-Boy `pipboy-core::audio_focus` passes 2 lease tests; live Radio output startup/replacement acquires `HOST_AUDIO_FOCUS`; app/core test suites pass 77/77 and 283/283 |
| Bounded package process framing | PASS (codec/endpoint source) | Rust Music `rust-music-ipc` and `rust-music-cli serve` implement strict `RBPK` v1 frames, shared hello vector, identity/sequence checks, no-autoplay activation, and lazy output creation. In-memory serve-loop tests reject a wrong package identity, Activate before Hello, and skipped request IDs without creating a sink. Spawned child behavior is recorded separately below. |
| Spawned Rust Music process lifecycle | PASS (macOS local subprocess) | `cargo test --locked --offline -p rust-music-cli --features macos-output,test-support --test package_process` passed 5/5 at `110bfed` plus the current regression. In addition to repeated parent-pipe closure, playback error cleanup, handshake/activation/shutdown and wrong package identity, the spawned process scans a generated track, observes its removal before Play, returns a bounded decoder error, answers a follow-up snapshot as Stopped with no audio claim, and exits cleanly after Shutdown. This proves stale-media rejection before output creation, not active playback device loss or a live Pip-Boy mount. |
| macOS app-owned child cleanup regression | PASS (headless macOS subprocess) | `cargo test --locked --offline -p rust-music-cli --features macos-output,test-support --test package_process` passed 4/4, including repeated parent-pipe closure and failed managed playback. `python3 scripts/test_macos_parent_exit.py --binary target/debug/rust-music-cli` exited 0: the real `serve` process exited 0 after normal stdin EOF, exited 1 after malformed protocol input, completed three repeated launch/EOF/exit cycles with observed PIDs, and exited after its app-like parent was SIGKILLed. This proves child-process exit/reap paths without opening an audio device; it does not simulate active AudioQueue output failure, device disconnect/recovery, or graceful native callback teardown. |
| Release Rust Music unit protocol smoke | PASS (local macOS release binary) | `cargo build --locked --offline --release -p rust-music-cli --features macos-output` followed by `python3 scripts/test_release_package_process.py --binary target/release/rust-music-cli` completed RBPK v1 Hello, inert Activate (19-byte empty/no-audio view), and Shutdown, then observed clean exit and empty stderr. SHA-256 `d1de04c2d8e3af36b75fa5fd5eb8147245144db87dbb6ab485170fd58beb3ffa`. The binary is not registered in Pip-Boy's trusted executable roster or mounted by live startup. |
| Supervised package process runner | IMPLEMENTED LOCALLY; focused tests reported PASS | Pip-Boy's `pipboy-core::package_process` contains fixed-path launch, deadlines, timeout kill/reap, stderr drain, process-exit focus cleanup, and shutdown handling. Its `docs/PACKAGE_PROCESS_PROTOCOL.md` records the targeted supervisor tests as passing. This Rust Music session inspected the implementation and ledger but did not rerun those tests. |
| Real Rust Music child / MUSIC-RADIO focus handoff | PASS (focused host broker regression; no audio output) | Pip-Boy commit `bfbb9c76d147911feb6d359ba0f2342a8a0cd97e` on branch `codex/rust-music-shared-focus-integration` adds an adapter-owned test. Freshly rerun 2026-10-31 against Rust Music app-bundled CLI built at `0dd8f3104c0a62393632c75e9821a9f1b656cd15`: `RUST_MUSIC_ADAPTER_BIN=<built-RustMusic.app>/Contents/MacOS/rust-music-cli CARGO_TARGET_DIR=<temporary-target-dir> CARGO_NET_OFFLINE=true cargo test --locked --offline -p pipboy-core actual_rust_music_child_runs_under_host_supervisor_when_configured -- --nocapture` exited 0 (1/1, 304 filtered out). The real Rust Music child is supervised, the host MUSIC claim excludes RADIO, Settings suspend permits RADIO to acquire, resume reacquires MUSIC, and shutdown releases it. The Pip-Boy worktree remained clean and unmodified. This proves broker lifecycle handoff only, not audible playback, a Rust Music sink/lease bridge, or TV contention. |
| Production package mount and acceptance | PENDING | Pip-Boy's reviewed executable roster is empty; live MUSIC registration/mount, host-supervised release binary, shared audio contention, shell/mobile acceptance and platform process behavior remain open. The supervisor is fault containment, not an OS sandbox. |
| Standalone macOS app bundle and UI exercise | PASS (local offline build, verified signature, LaunchServices launch, nonzero output and child lifecycle) | At current built source, `sh scripts/build-macos-app.sh` exited 0 and strict signature/plist checks passed. Unsandboxed `/usr/bin/open -n` exited 0; LaunchServices checked in `dev.rustmusic.standalone` in 0.065434 s. Outside the restricted shell, `system_profiler` listed MacBook Air Speakers as the 48 kHz default output plus HDMI/headphone outputs. The bundled CLI played a generated nonzero 440 Hz 16-bit/44.1 kHz WAV and exited 0; the user confirms audible output. In the app UI, generated 10-second 440/660 Hz WAVs exercised Play, active Next/Previous, Pause/Resume, volume 100→95→100 and Stop. App PID 93068 owned `rust-music-cli serve` PID 93107; after Cmd-Q both were absent from the process list and LaunchServices marked Rust Music stopped. This is native output-path evidence, not acoustic loopback or bit-perfect DAC measurement. Existing FLAC/WAV/AIFF parity was retained. `spctl --assess` rejects the local ad-hoc signature; Developer ID/notarization remain unverified. Device disconnect/recovery, physical DAC, Rust Boy live focus, and panel remain separate gates. |

### 2026-10-31 refreshed Rust Music child / host-focus broker test

Pip-Boy checkout `codex/rust-music-shared-focus-integration` at
`bfbb9c76d147911feb6d359ba0f2342a8a0cd97e` was clean before and after this
read-only test. Against the Rust Music app-bundled release CLI rebuilt from
product candidate `5e1e94edd199ed2fbd5cf49937c423bcdf582214` (SHA-256
`14fe5c2cdd3a611b715a2a2fad626c4342bdc2a303da3005ec62e1c7b19dcaff`), the
command below exited **0**, passing **1/1** test (304 filtered):

```sh
CARGO_TARGET_DIR=/private/tmp/rust-music-pipboy-target CARGO_NET_OFFLINE=true RUST_MUSIC_ADAPTER_BIN=/Users/null/GitHub/rust_music/target/macos/RustMusic.app/Contents/MacOS/rust-music-cli cargo test --locked --offline -p pipboy-core actual_rust_music_child_runs_under_host_supervisor_when_configured -- --nocapture
```

The real child ran under the host supervisor: its MUSIC claim excluded RADIO,
Settings suspend allowed RADIO to acquire, resume reacquired MUSIC, and
shutdown released the claim. Cargo output stayed under `/private/tmp`; no
Pip-Boy source or tracked build output changed. This verifies broker lifecycle
handoff only; it does not test audible playback, a Rust Music sink/lease
bridge, live TV contention, physical panel, or DAC output.

The scripted clean-source integration path was also rerun with a new temporary
target: `RUST_MUSIC_REPO=<Rust Music checkout>
RUST_BOY_PROCESS_TARGET=<temporary target> CARGO_NET_OFFLINE=true python3
tools/test_supervised_music_process.py`. It built the Rust Music CLI from the
current checkout with `cargo build --locked --offline -p rust-music-cli
--features macos-output --bin rust-music-cli` (exit **0**, no cache reuse in
the fresh target; binary SHA-256
`67611eea7afaf6c91bdf85086a19dc613203e73534a79c2d116a67fdbf83edc0`) and
reran the Pip-Boy supervisor integration test (exit **0**, 1/1 passed). All
Cargo artifacts were confined to temporary storage, and the Pip-Boy worktree
remained clean. This confirms source-build interoperability in addition to
the packaged-bundle check above, but still exercises inert activation and
broker ownership rather than audio sink creation or audible MUSIC/TV
contention.

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
| Audible Rust Music shared lease bridge / TV ownership | PENDING | The separate Pip-Boy regression proves host broker ownership around a real inert Rust Music child, including MUSIC/RADIO suspend/resume handoff. Audible MUSIC output, Rust Music sink/lease bridge, and live TV ownership/contention remain unverified. |
| AudioQueue OSStatus failure and teardown recovery | PASS (simulated API failures) | At `a65fbe9`, `cargo test --locked --offline -p rust-music-native --features macos` passed 17/17. Coverage includes create/partial-allocation failure, enqueue/start/stop/dispose errors, retained-orphan retry, graceful callback drain, timeout escalation, `simulated_synchronous_invalid_device_enqueue_failure_recovers_after_device_returns` (active fake queue returns Apple's `kAudioQueueErr_InvalidDevice`, -66680, then cleanup and same-API reopen), and `simulated_callback_starvation_timeout_escalates_teardown_and_allows_reopen` (bounded write timeout, immediate-stop/dispose, then same-API reopen). The OSStatus matches the installed macOS SDK's `AudioQueue.h:171`; see [Apple's InvalidDevice reference](https://developer.apple.com/documentation/audiotoolbox/kaudioqueueerr_invaliddevice). These simulate API failures; they do not cover asynchronous route/device-loss notification or an actual device error during removal. |
| Physical headphone route transition during active playback | PARTIAL (operator-assisted) | On 2026-10-31, a generated 330 Hz, 16-bit/44.1 kHz tone played through Rust Music and macOS AudioQueue while the operator unplugged/reconnected Shure headphones on the analog port. System Settings changed External Headphones → MacBook Air Speakers (46%) → External Headphones (93%); Rust Music remained PLAYING with no visible error, then Stop and Quit succeeded. This establishes route change/return, not acoustic continuity or a DAC sample measurement. See [MAC v1.0 device evidence](CODEX_MACOS_V1_DELIVERABLE.md#2026-10-31-active-headphone-route-loss-and-return). |
| Physical audio / bit-perfect output | PENDING | Actual macOS/Linux DAC/loopback measurement; not implied by NullSink, structural tests, or the observed default-route transition |

## 2026-10-31 current-bundle shared-focus broker recheck

The clean Pip-Boy checkout remained at
`bfbb9c76d147911feb6d359ba0f2342a8a0cd97e` on
`codex/rust-music-shared-focus-integration`. Against the current locally built
Rust Music bundle CLI from product source
`1941cadab95eae3490bf4d599748acb60b21b8b1` (CLI SHA-256
`14fe5c2cdd3a611b715a2a2fad626c4342bdc2a303da3005ec62e1c7b19dcaff`), the
focused test
`cargo test --locked --offline -p pipboy-core actual_rust_music_child_runs_under_host_supervisor_when_configured -- --nocapture`
exited **0** (1/1 passed, 304 filtered). `CARGO_NET_OFFLINE=true` and a
temporary `CARGO_TARGET_DIR` outside both repositories were used. Pip-Boy
source remained clean and unchanged.

The actual Rust Music child completed the supervised RBPK handshake and
inert activation. The broker's MUSIC claim excluded RADIO; suspending MUSIC
released the claim and allowed RADIO to acquire it; after RADIO released,
resuming MUSIC reacquired it; child shutdown released the claim. This refreshes
cross-process lifecycle/broker evidence only. It does not start an audio sink,
verify audible MUSIC output or TV contention, register/allowlist the bundle,
or demonstrate live shell/mobile acceptance.

## 2026-10-31 simulated AudioQueue error recovery and offline gate

- Source commit `a65fbe95970f6d2a8171d3011cbcebe65d6d10db` adds two serialized
  AudioQueue regressions. One starts a fake queue, injects
  `kAudioQueueErr_InvalidDevice` (`-66680`) on a later active enqueue, verifies
  failed-slot cleanup and registry release, then restores fake availability
  and opens, writes and finishes a new queue through the same fake API. The
  other fills all fake output slots, simulates missing completion callbacks,
  checks the bounded write timeout and immediate-stop/dispose escalation, then
  restores callbacks and successfully reopens through that same fake API.
- `cargo test --locked --offline -p rust-music-native --features macos` exited
  **0**, **17/17**; independent verification also exited **0** with 17/17.
  Release-mode `cargo check --locked --offline -p rust-music-native
  --features macos --release` exited **0**. `sh scripts/offline-gate.sh`
  exited **0**: 17 local crates, 32 path-only
  internal edges, zero registry/Git crates, one Apple OS framework link, and
  all workspace offline checks passed.
- These simulate API failure and callback starvation, not physical unplug or
  asynchronous route-change notification. No private corpus or hardware test
  was run; physical device disconnect/recovery and DAC qualification remain
  pending. The current adapter has no asynchronous route-loss observer.

## macOS playback-failure UI recovery — 2026-10-31

At candidate `01ac0e6384039cc078dda0f0a1c8ff6e325a2039`, the standalone app
maps terminal stopped, finished, and failed snapshots to inactive transport
controls. Package-command failures clear stale playing/paused state and request
one authoritative snapshot when transport was active. Non-playing snapshots
also cancel the pending UI polling flag. This allows an explicit Play retry
after a native output failure without acquiring focus automatically.

`swiftc apps/rust-music-macos/PlaybackUIState.swift
apps/rust-music-macos/PlaybackUIStateTests.swift -o
/private/tmp/rust-music-playback-ui-state-tests` and the resulting test binary
exited **0**; playing and paused flags are preserved only for states 2 and 6,
while stopped/finished/failed states clear both. `sh
scripts/build-macos-app.sh` exited **0**, including strict local ad-hoc
signature verification. `sh scripts/offline-gate.sh` exited **0** on this
candidate: 17 local crates, 32 path-only edges, zero registry/Git crates, zero
third-party system links, and one Apple OS framework link. The native
AudioQueue simulated recovery suite passed 17/17 as part of the gate.

The focused feature-gated subprocess suite
`cargo test --locked --offline -p rust-music-cli --features
macos-output,test-support --test package_process` exited **0**, **7/7**.
`python3 scripts/test_macos_parent_exit.py --binary
target/macos/RustMusic.app/Contents/MacOS/rust-music-cli` exited **0** and
observed normal EOF exit 0, malformed protocol exit 1, three repeated
launch/exit cycles, and a real serve child exit after its app-like parent was
force-killed. `python3 scripts/test_release_package_process.py --binary
target/macos/RustMusic.app/Contents/MacOS/rust-music-cli` exited **0**; the
bundled CLI SHA-256 was
`37cef5529b89937a7494f5d20af2dbff6ddd9dd19f4558036018aea15163ba2b`.
These CLI tests establish child-process behavior, not the SwiftUI app closing
its child during this pass or physical device behavior.

The rebuilt bundle was also launched twice through `/usr/bin/open -n`; the
captured second invocation exited **0**. `lsappinfo` identified bundle ID
`dev.rustmusic.standalone`, the expected `RustMusic.app` path, and
`Contents/MacOS/RustMusic`. The check-created idle instances were sent TERM
and disappeared; pre-existing Rust Music PID 24897 was left running. No
`rust-music-cli` child was present. This is a current LaunchServices startup
smoke, not a Cmd-Q or active-playback child-cleanup observation.

No private codec corpus was rerun. These checks do not simulate a physical
device disconnect, asynchronous route loss, audible output, DAC fidelity, or
Rust Boy live focus contention; those acceptance gates remain open.

At `4d110594a7b4c3c2641d86f166a2160d1f7cfd1f`, the macOS app builder now
compiles and runs `PlaybackUIStateTests.swift` before compiling and signing the
bundle. A fresh `sh scripts/build-macos-app.sh` exited **0** and printed the
regression's PASS line before successful signature verification. This keeps
the failure-state regression in the normal packaging path; it does not replace
the interactive playback-failure or device-recovery tests.

## 2026-10-31 restricted-shell device query

- Sandboxed `system_profiler SPAudioDataType -detailLevel full` exited 0 but
  reported no devices. This reflected the restricted shell environment, not
  the host's actual audio inventory; see the unsandboxed smoke evidence above.
- After the latest backend change, `sh scripts/build-macos-app.sh` exited 0;
  `plutil -lint target/macos/RustMusic.app/Contents/Info.plist` and
  `codesign --verify --deep --strict` also exited 0. This verifies packaging,
  not device loss/recovery or DAC fidelity.

## 2026-10-31 queue and integrated offline gate

- `cargo test --locked --offline -p rust-music-library` exited 0 (21/21),
  including the catalog/search, M3U8, queue navigation/repeat/shuffle and
  Unix atomic queue persistence tests. `next_candidate` and
  `previous_candidate` only select state; playback and focus remain explicit
  host actions. Queue file tests reject symlink destinations and confirm a
  source audio sentinel remains unchanged.
- The first full-gate run found Clippy's `should_implement_trait` warning for
  the queue method named `next`; it exited 101. The methods were renamed to
  candidate selection terms. The subsequent `sh scripts/offline-gate.sh`
  exited 0 across 17 local crates, 32 local edges, all tests, Clippy, release
  build and macOS feature checks. No private PCM corpus was rerun.
- `python3 scripts/check_roadmap_index.py`,
  `python3 scripts/check_literature.py`, `cargo fmt --all -- --check`, and
  `git diff --check` exited 0.

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
