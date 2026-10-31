# Rust Music — phased engineering roadmap

The **machine-readable task, gate and agent registry** is [roadmap-index.json](roadmap-index.json). Keep this file readable for operators; let the registry own stable RM task IDs, gate states, dependencies and Codex routing. Do not infer test results from a commit, source review or a filename.

## Non-negotiable release contract

- Playback accepts only verified **lossless** codecs and at least 16-bit / 44.1 kHz PCM. Lower-resolution decoder fixtures are permitted for tests, never playback.
- Own all runtime decoders and adapters; Cargo dependencies are workspace-local, zero remote registry/Git packages or bundled third-party codecs. Native OS/device APIs are explicit platform prerequisites.
- Verify exact decoded integer PCM independently of hardware. **Bit-perfect DAC** needs physical capture and exact device negotiation, not just FLAC-to-WAV parity.
- Keep Rust_Boy host-owned single audio focus, no unrequested autoplay or background scanning, and the 128×64, 1024-byte display budget. Optional autoplay remains future work: it may run only after explicit user library load, persistent opt-in (default off), a successful bounded scan, and host focus grant; never on startup, refresh, or reconnect.
- The offline Rust gate has no Python dependency; optional agent/index maintenance checks use Python stdlib only.

## Gate status and ownership

| Gate | Scope | Codex owner | Evidence state | Next acceptance |
| --- | --- | --- | --- | --- |
| **G0** | Ownership, standards and offline audit | `index_curator` | partial evidence | Offline dependency audit, fmt, Clippy, workspace tests, release build and macOS feature check pass locally on Rust 1.99.1; target hardware and host integration remain open |
| **G1** | Integer PCM core and explicit playback state | `codec_engineer` | source implemented | Workspace tests pass, including lossless playback floor and fail-closed output state; actual device behavior remains unverified |
| **G2** | Native FLAC with CRC, Rice/LPC, PCM MD5 | `codec_engineer` | partial evidence | Seven generated vectors and six private real-world tracks pass native PCM parity; wider codec/hardware claims remain separate |
| **G3** | Mac AudioQueue / Linux kernel PCM output | `audio_backend` | source implemented | Source present; DAC format, drain and underrun measurement pending |
| **G4** | Expanded lossless umbrella | `codec_engineer` | partial evidence | AIFF, generated M4A/ALAC, and bounded CAF profiles are implemented; broader ALAC admission remains |
| **G5** | Rust_Boy socket and shared audio-focus | `host_integrator` | partial evidence | Rust Music has a bounded RBPK v1 contract, spawned CLI lifecycle coverage, and a local release-binary hello/activate/shutdown smoke; Pip-Boy catalogs MUSIC as a distinct uninstalled package and has a supervised runner. Trusted release registration, live mount, user input/worker bridge, shared Radio/TV/Music contention, and field acceptance remain open |
| **G6** | 128x64 monochrome 1KiB panel | `host_integrator` | source implemented | Renderer exists; hardware screen/controls pending |
| **G7** | Strict RIFF/WAVE PCM and extensible WAV | `codec_engineer` | partial evidence | Six native RIFF PCM cases and synthetic RF64 >4 GiB streaming/sample parity pass; actual large-file corpus and hardware output remain unverified |
| **G8** | Six-file private FLAC differential corpus | `gate_verifier` | partial evidence | Reference FLAC/FFmpeg and native Rust PCM hashes match on all six private files; artwork and hardware stress remain separate |
| **G9** | Exact declared-frame timing and display progress | `host_integrator` | source implemented | Workspace tests pass; physical display and Rust_Boy button behavior remain unverified |
| **G10** | Audiophile-only admission and AIFF/AIFF-C | `codec_engineer` | partial evidence | Local offline gate passes on Rust 1.99.1/macOS and seven AIFF/AIFF-C native PCM vectors pass; physical DAC and host integration remain open |
| **G11** | ALAC decoder and M4A/CAF demultiplexers | `codec_engineer` | partial evidence | Raw packet parity, six Internet Archive M4A ALAC parity cases, AAC denial, and five generated CAF ALAC/LPCM vectors pass; broader encoders, real-world CAF, and host/output acceptance remain |
| **G12** | RF64 and WavPack lossless extensions | `codec_engineer` | partial evidence | Bounded RF64/ds64 and large-size synthetic streaming pass. WavPack's bounded decoder passes generated FFmpeg PCM vectors and independent Tiny Encoder 4.40 parity for selected packed-16/24-bit profiles, including a 1,063,147-frame deterministic synthetic 24-bit stress vector. Native real-world WavPack files, broader profile/encoder coverage, trailing tags, hybrid correction and public playback admission remain open |
| **G13** | Bounded metadata and sample-accurate seek | `codec_engineer` | partial evidence | Bounded FLAC/AIFF tags, synthetic seek tests, and PCM-exact seeks at every populated SEEKTABLE point in six private FLAC streams pass; other encoders and host/hardware evidence remain |
| **G14** | Windows WASAPI and Android AAudio ports | `audio_backend` | planned | Platform-local compile, focus and physical output acceptance needed |
| **G15** | Desktop micro-library browsing and playback controls | `host_integrator` | partial evidence | Exact rebuilt-app UI exercised Play/Pause/Resume/Stop/active Next/Previous and volume on generated nonzero 440/660 Hz WAVs; bundled CLI AudioQueue playback exited 0 and the user confirms audible output. At `fb5e70d`, the compact phosphor-green UI fits player, queue, library, playlists and bottom transport into the inspected app window; synthetic library load and playlist create/delete worked. Fresh offline gate passes. A 2k-entry app catalog loaded in <1 s, exact search yielded one result, and one same-process RSS sample increased 1,568 KiB. Device recovery, shared host focus and repeatable startup/incremental-memory/latency measurements remain |

## Codex task queue

| Task | Priority | Gate | Owner | State | Prerequisite |
| --- | --- | --- | --- | --- | --- |
| `RM-001` — Rerun full offline Rust quality gate after formatter repair | P0 | G10 | `gate_verifier` | done | None |
| `RM-002` — Replay six private FLACs through native Rust decoder | P0 | G8 | `gate_verifier` | done | RM-001 |
| `RM-003` — Prove native RIFF/WAVE decoded PCM roundtrip | P0 | G7 | `gate_verifier` | done | RM-001 |
| `RM-004` — Verify lossless-only floor and focus denial through Rust_Boy | P0 | G10 | `host_integrator` | done | RM-001 |
| `RM-005` — Qualify macOS AudioQueue output with physical DAC | P1 | G3 | `audio_backend` | blocked | RM-001 (done); physical DAC required |
| `RM-006` — Integrate independent MUSIC socket with Radio and TV ownership | P1 | G5 | `host_integrator` | in_progress | RM-004 |
| `RM-007` — Validate fixed 1KiB panel on actual Rust_Boy display | P1 | G6 | `host_integrator` | blocked | RM-006 |
| `RM-008` — Develop independent ALAC bitstream and PCM decoder | P1 | G11 | `codec_engineer` | done | RM-001 |
| `RM-009` — Implement M4A/MP4 ALAC-only demux and AAC denial | P1 | G11 | `codec_engineer` | done | RM-008 |
| `RM-010` — Implement CAF ALAC and integer LPCM demux | P2 | G11 | `codec_engineer` | done | RM-008 |
| `RM-011` — Add bounded local metadata without cover-art memory spikes | P2 | G13 | `codec_engineer` | done | RM-001 |
| `RM-012` — Implement validated sample-accurate FLAC seek | P2 | G13 | `codec_engineer` | done | RM-002 |
| `RM-013` — Implement RF64 large PCM files with ds64 limits | P2 | G12 | `codec_engineer` | done | RM-003 |
| `RM-014` — Implement WavPack only in verified lossless modes | P2 | G12 | `codec_engineer` | in progress | RM-001 |
| `RM-015` — Port native output to Windows and Android without codec dependencies | P2 | G14 | `audio_backend` | blocked | RM-005, RM-006 |
| `RM-016` — Keep code, literature, agent roles and format registry indexed | P1 | G0 | `index_curator` | done | None |
| `RM-017` — Index M4A/MP4 in library and validate Archive.org ALAC corpus | P1 | G11 | `codec_engineer` | done | RM-009 |
| `RM-018` — Define bounded Pip-Boy package-process protocol | P1 | G5 | `host_integrator` | in_progress | RM-006 |
| `RM-019` — Qualify standalone macOS app lifecycle and package | P1 | G5 | `host_integrator` | done | RM-001 |
| `RM-020` — Build zero-artwork micro media library experience | P1 | G13 | `host_integrator` | in_progress | RM-019 |

**Priority order:** RM-008 raw ALAC parity, RM-009 bounded M4A demux/AAC denial, RM-010 CAF ALAC/LPCM demux, RM-017 M4A catalog ingestion plus six licensed upstream ALAC files, RM-011 bounded text tags, and RM-012 FLAC seek acceptance pass their recorded criteria. Six supplied real-world streams now pass exact range parity at every populated SEEKTABLE point and the following sample; broader encoder coverage remains open. Next decoder evidence is ALAC parity across independent sources/encoders and real-world CAF coverage. Physical output, Rust_Boy shared audio focus, and display acceptance remain separate production gates; do not infer those from host mocks or source review. Each Pip-Boy module retains its own module identity, package/registration record, and lifecycle while using the shared container.

## Acceptance ladder (no skipped proofs)

1. **Source:** code, locally-authored dependencies, bounded parser and tests exist.
2. **Compiled:** Rust fmt, Clippy, workspace tests and release build actually exit successfully on a defined toolchain.
3. **PCM-parity:** own decoder reproduces independent lossless PCM for named fixtures, rejects corruption and records counts/checksums.
4. **Host:** Rust_Boy focus, module unloading and 1024-byte screen work together without Radio/TV conflict.
5. **Hardware:** exact format/rate and loopback/measurement show device behavior, including errors, underruns and stop/drain. Only this level can justify hardware bit-perfect wording.

Each task closes only when its `acceptance` line has observed evidence. Record status changes in the machine index and update the human ledger. A reported success on a reference oracle does not establish our decoder passed.

## Current evidence snapshot — 2026-10-31

- **Current candidate refresh (`657cb505151fe949800574aa395fce4a1cac40c0`):** `sh scripts/offline-gate.sh` exited **0** at current source: 17 local crates, 32 path-only edges, no registry/Git crates, and 17/17 simulated AudioQueue tests. The explicitly enabled package-process suite exited **0** (7/7), including active-playback parent close, playback error and repeated launch/exit. `sh scripts/build-macos-app.sh` exited **0** with optimized UI/catalog harnesses and strict local bundle signature verification. The Swift search harness measured 20k production-index construction at 121.094/123.805 ms p50/p95 (nine runs), last-item search at 5.839/6.127 ms (101 runs), and 1.3 MB cached UTF-8 payload. This synthetic in-memory profile is not filesystem scan or app-wide memory evidence. The worktree was clean after local commit; branch is 114 commits ahead of its local tracking ref and local `origin/main` is an ancestor. External GitHub state was not refreshed; no push or merge. Real device-error recovery, live host audio-focus contention, physical DAC/panel and repeatable app-memory acceptance remain open.
- **Rebuilt-app playlist/queue workflow:** On the same app source, three generated one-second 16-bit/44.1 kHz silent WAVs were explicitly loaded; the UI showed three tracks and stayed stopped. A named playlist was created with one selected track, and two tracks were added to the persistent queue without starting playback. After the UI returned to Ready / No Track, explicit reload of the same root restored one playlist and both queue entries. No process-exit evidence was captured for the intervening Ready state, so the app-level restart element remains covered primarily by automated persistence/restart tests. Synthetic fixtures were removed.
- **Offline gate:** Local `sh scripts/offline-gate.sh` exited 0 on Rust/Cargo 1.99.1 and macOS: 17 local crates, 32 path-only edges, zero registry/Git crates, zero third-party system links and one Apple OS framework link. Rustfmt, warnings-denied Clippy, workspace tests, release build and macOS feature check passed on the recorded worktree. This is build/test evidence, not physical audio acceptance. [G10 evidence](G10_AUDIOPHILE_FORMATS.md).
- **Current macOS lifecycle/UI:** At app code `b03c592ec67d102325c4b0af0adc93acc71e9651`, offline gate and app-bundle build/signature passed. With the rebuilt app open and an existing CLI child (playback finished), Activity Monitor showed app PID 95299 and child PID 95500; after Cmd-Q the old pair disappeared, a fresh CUA-started app PID 9117 had no CLI child, and a later Quit showed `isRunning=false` with no Rust Music/CLI rows. Reopening showed Ready/No Track, disabled transport, and no child; all four navigation sections were runtime-checked. On 2026-10-31, the current app loaded only the generated `silent-60s.wav` fixture and showed “PLAYING … THROUGH RUST MUSIC AND MACOS AUDIOQUEUE”; Activity Monitor showed app PID 11612 and `rust-music-cli` PID 11664. Cmd-Q was sent with Rust Music foreground. After refreshing Activity Monitor, neither process appeared, and `cua.listApps()` reported `isRunning=false`. This verifies idle and active silent-playback child shutdown, plus no startup autoplay/scan; it is not audible-content or DAC evidence. After confirming the app was stopped, five fresh-process launches via `cua.getApp()` took 891, 858, 753, 798 and 779 ms. Each showed Ready / No Track with no scan or autoplay, and `listApps` confirmed the app stopped before the next launch. Median was 798 ms (range 753–891 ms). These are five fresh-process app launches, not system-boot cold starts or a threshold result. Separately, one idle Activity Monitor Memory view showed app PID 12236 at `anonymousMemory` 28.2 MB with no CLI child; this is one anonymous-memory sample, not RSS. After Cmd-Q and refresh, `listApps` reported `isRunning=false` and Activity Monitor had no app/CLI rows. Two direct synthetic 0/1/100/2k/20k catalog profiles exited 0; at 20k entries build took 519/636 ms and search p95 was 734/802 µs. The `/usr/bin/time -l` wrapper exited 1 on sandbox-denied RSS sysctl, so peak RSS remains unmeasured. Device disconnect/recovery, live host focus and physical output remain open.
- **RM-019 subprocess regression refresh (2026-10-31):** After rebuilding the debug CLI with `macos-output,test-support`, `scripts/test_macos_parent_exit.py` exited 0. It observed normal EOF (exit 0), malformed protocol (exit 1), three repeated launch/exit cycles, and real child exit after its app-like parent was force-killed. Host process inspection confirmed all six reported child PIDs were absent afterward. This complements the GUI active-playback SIGKILL evidence; it is test-sink/process evidence, not hardware qualification.
- **RM-020 memory and latest lifecycle:** Code commit 7c1b1d0ba795a8af6cd0ff36bb55abf3fd087bcc passes sh scripts/offline-gate.sh (17 local crates, 32 path-only edges, no registry/Git crates), the macOS app bundle build, Info.plist lint, and strict local signature verification. Focused macOS CLI tests pass 19/19 unit plus 7/7 subprocess. Compact bounded UTF-8 metadata spans are measured with three synthetic 20k catalog RSS deltas per mix: 5,248 KiB no-tag, 5,568 KiB 10%-tagged, and 7,824 KiB all-tagged medians, all below 8 MiB. These are process RSS estimates, not exact heap or full-app memory. The rebuilt bundle launched via open -n (exit 0) and showed Ready / No Track with no startup scan, autoplay, or CLI child. Earlier active-playback SIGKILL testing at a40758d observed the app and CLI child exit within 8 seconds. Device disconnect/recovery, physical DAC/panel, Rust Boy shared focus, app-wide RSS, cold system startup, and transition latency remain open.
- **Current candidate macOS recovery and catalog profile (code `01ac0e6384039cc078dda0f0a1c8ff6e325a2039`):** The Swift playback-state regression test and app bundle build/signature passed; the full offline gate passed at the same product code. Three synthetic catalog RSS runs measured 800 KiB median at 2k entries and 5,232 KiB at 20k entries. Two serial 20k search profiles measured catalog builds of 419/313 ms and p95 search of 739/728 µs. A later rebuilt-app UI pass at `4d11059` completed four active synthetic silent-track Next/Previous transitions in 729–1,093 ms (median 886 ms, including CUA/accessibility overhead). These timings have no accepted threshold and are not acoustic latency. The memory/search values are catalog-core estimates, not full SwiftUI app RSS or interactive search latency. The feature-gated CLI lifecycle tests passed 7/7, and the rebuilt bundle's LaunchServices `open -n` exited 0. Physical disconnect/recovery, DAC/panel, live host focus, app-wide RSS and cold-start thresholds remain open. See [macOS v1.0 evidence](CODEX_MACOS_V1_DELIVERABLE.md) and [G5 validation](G5_VALIDATION.md).
- **Current package-process suite (2026-10-31):** `cargo test --locked --offline -p rust-music-cli --features macos-output,test-support --test package_process` passed 7/7 at source `eb69a8c`; covers protocol rejection, removed-track fail-closed behavior, playback error, repeated parent-pipe closure, active playback closure, and Shutdown child exit. This injected test-sink evidence is not physical device recovery.
- **G5 simulated AudioQueue failure recovery (2026-10-31, commit `a65fbe95970f6d2a8171d3011cbcebe65d6d10db`):** `sh scripts/offline-gate.sh` exited 0 across 17 local crates, 32 path-only edges, zero registry/Git crates and one Apple OS framework link. The native adapter suite passed 17/17. Two serialized fake-API cases cover active `kAudioQueueErr_InvalidDevice` (`-66680`) followed by same-API reopen, and callback starvation through the bounded write timeout followed by immediate stop/dispose and same-API reopen. Independent test verification and source review passed. These simulate API failure and callback starvation only; asynchronous route-loss detection and physical unplug/reconnect remain untested. RM-005 physical device/DAC qualification remains blocked on operator hardware evidence.
- **Device-loss recovery regression (2026-10-31, candidate based on `ae0478003fc796a3ad73a5c743eda3312b759365`):** New engine and host fake-sink tests each passed 1/1. Injected output-write loss releases the failed sink and host focus lease; after the fake device returns, recovery requires a fresh explicit Play, reacquires focus, plays and releases focus at EOF. `sh scripts/offline-gate.sh` exited 0 with 17/17 native AudioQueue simulations, 12 engine tests and 18 host tests. A separate short headphone probe ended before an unplug was observed and is recorded as inconclusive. Physical invalid-device error/reopen and DAC qualification remain open.
- **Committed candidate app refresh (`5e1e94edd199ed2fbd5cf49937c423bcdf582214`):** `sh scripts/build-macos-app.sh` exited 0; 18 app-build path guards and strict bundle signature passed. The synthetic 20k catalog harness measured search p50/p95 6.199/7.836 ms, indexed model construction p50/p95 116.170/126.887 ms, and 1.3 MB cached index. The package-process suite passed 8/8 and roadmap validator passed. Sandboxed `open -n` returned `kLSNoExecutableErr`; host-context LaunchServices launch of the same rebuilt bundle exited 0, showed Ready/No Track with no startup scan/autoplay, then Cmd-Q resulted in `isRunning=false`. These checks do not close app-wide memory, device-error, physical DAC/panel or Rust Boy focus gates.
- **Repeated full-app catalog footprint sample (2026-10-31):** Two fresh app processes loaded 20,000 synthetic 12-byte WAV signatures while remaining stopped. Five `proc_pid_rusage(RUSAGE_INFO_V4)` physical-footprint samples were stable before and after each scan; same-process deltas were 6.734 and 6.516 MiB (median 6.625 MiB). Resident-size deltas were noisy, so are not used. This is below the 8 MiB budget for a minimal untagged synthetic catalog; typical tagged-library, search/UI, startup and threshold acceptance remain open. Test fixture and probe were removed and both app runs quit.
- **Spawned package device-error recovery (2026-10-31):** Feature-gated `cargo test --locked --offline -p rust-music-cli --features macos-output,test-support --test package_process` passed 9/9, including an injected first-write loss that reports Failed/unclaimed, requires explicit Play to reopen and deliver frames, then Stop/Shutdown exits the child cleanly. The full `sh scripts/offline-gate.sh` exited 0 with 17 native AudioQueue simulations. This is fake-sink process/IPC coverage, not a physical AudioQueue invalid-device callback; recorded in the Mac v1.0 evidence ledger.
- **Tagged full-app library memory profile (2026-10-31):** The macOS app explicitly scanned generated 129-byte FLAC metadata-only signatures (TITLE/ARTIST/ALBUM, no audio frames) and stayed stopped. `proc_pid_rusage` five-read same-process deltas were 3.594 and 2.156 MiB for separate fresh 2k-track runs (median 2.875 MiB); a 20k-track stress run measured 9.313 MiB. The minimal untagged 20k WAV fixture measured 6.625 MiB in two runs. Thus 2k tagged profiles are below the 8 MiB budget, but vary between runs; 20k tagged profiles exceed it. Results are application physical footprint, not heap, and neither typical-library nor threshold acceptance is closed. Fixtures and app were removed.
- **Current bundled child / host-focus broker check (2026-10-31):** The Pip-Boy host test `actual_rust_music_child_runs_under_host_supervisor_when_configured` passed 1/1 (exit 0) using the current built Rust Music release CLI; it verified MUSIC excludes RADIO, suspend/resume hands off and reacquires MUSIC, and shutdown releases the lease. The Pip-Boy checkout remained clean; Cargo target was outside it. No output, TV contention, physical panel or DAC was exercised. Full evidence and binary SHA-256 are in [G5 validation](G5_VALIDATION.md).
- **AIFF/AIFF-C:** Local native CLI PCM differential tests passed 7/7, including signed 24-bit/192 kHz, 32-bit, little-endian AIFF-C and malformed-input rejection; this does not certify audio hardware. [G10 evidence](G10_AUDIOPHILE_FORMATS.md).
- **FLAC:** Six user-provided private FLAC files passed reference FLAC/FFmpeg parity and native Rust full-PCM hash/byte-count parity. Do not commit audio. [G8 ledger](G8_REALWORLD_FLAC_CORPUS.md).
- **WAVE:** Local native CLI PCM parity passed 6/6, with malformed RIFF boundary, missing pad and nonzero precision-padding rejection. Physical device output remains untested. [G7 ledger](G7_RIFF_CONFORMANCE.md).
- **CAF:** Local FFmpeg-generated CAF parity passed 5/5 exact PCM vectors: signed LPCM 16/24/32-bit and ALAC 16/24-bit. FFmpeg is test-vector tooling only; custom channel layouts and unsupported codecs fail closed. Real-world CAF corpus and hardware remain open. [G10 evidence](G10_AUDIOPHILE_FORMATS.md).
- **M4A/MP4:** Six Internet Archive ALAC M4A files from one CC0-labeled collection pass exact PCM parity: five 16-bit/44.1 kHz tracks and one 24-bit/48 kHz track. Three additional Archive.org music M4As (one CC0-labeled sample and two tracks from distinct CC BY/CC BY-SA albums) plus one generated M4A are verified AAC and rejected. The optional script checks hashes, codec identity, catalog ingestion and the `.mp4a` alias. Archive license labels are uploader-supplied. Independent ALAC source/encoder, 20/32-bit and mono M4A parity, real-world CAF, and broad ALAC admission remain open. [Corpus provenance and coverage needs](../crates/rust-music-mp4/tests/fixtures/upstream/README.md).
- **Host and hardware:** The bounded IPC crate and endpoint implement RBPK v1, with process lifecycle coverage; this Rust Music worktree does not mount the package in a live Pip-Boy host. NullSink and pure render paths are not actual shared focus, screen wiring or DAC tests. Rust Music exposes a distinct stable package identity and suspend lifecycle; Pip-Boy's reference registry includes MUSIC as a distinct, explicitly uninstalled package. A trusted runner, shared focus, host display integration and device acceptance remain unverified. [Package-process contract](PACKAGE_PROCESS.md). [Pip-Boy adapter contract](PIP_BOY_ADAPTER.md). [Host contract](HOST_CONTRACT.md).

## Next expansion

**G11:** Locally authored ALAC, bounded M4A/MP4 and CAF demultiplexers, AAC/DRM denial, and M4A catalog ingestion are implemented. Six Archive.org M4A ALAC files from one source and generated cases pass exact PCM parity. Four real/generated AAC cases are rejected. Independent ALAC encoder/source, supported 20/32-bit and mono M4A parity, and real-world CAF coverage are still required before broad ALAC admission.
**G12:** RF64/ds64 is implemented with bounded synthetic large-file evidence. WavPack has exact PCM parity and decoded-header checksum validation on generated packed-16-bit mono/joint-stereo one-block fixtures, five-block mono, mixed-mode and all-true-stereo fixtures, deterministic pseudorandom stereo fixtures with a joint-to-true-stereo transition, and padded-24-bit mono/stereo `INT32_INFO=[0,8,0,0]` profiles, including a high-entropy signed-24-bit stereo vector. An independent WavPack Tiny Encoder 4.40 implementation produces exact packed-16/24-bit parity for high-entropy stereo sources using supported fast-mode terms, plus exact packed-16-bit default-, high- and very-high-quality-mode parity on 20,000 stereo frames (the high-mode profile exercises terms `[18,18,18,-2,2,3,5,-1,17,4]`), plus exact 1,063,147-frame parity on deterministic synthetic packed-24-bit PCM. Positive-term adaptive weights are not clamped at ±1024; this correction closes the previously observed one-unit mismatch in the second mixed-mode block. It also has adjusted-binary vectors, adaptive median zones, and fail-closed WavPack 5 encoded-block checksum validation for 16/32-bit checksum metadata. Native WavPack real-world source files, additional encoder parity, other sample profiles, trailing tags, and hybrid correction support remain open; WavPack is still not admitted by the facade.
**G13:** Safe bounded metadata and accurate frame seek, without embedded artwork exhausting the micro UI.  
**G14:** Independent WASAPI and AAudio device adapters with real target qualification.
**RM-020 / ML-00–ML-07:** The catalog has stable canonical-root IDs, bounded text/container filtering, deterministic sorting/pagination, cancel-aware explicit scans, bounded M3U8 editing, Unix-persistent queue state with repeat/seeded shuffle, and bounded versioned named playlists. The standalone macOS UI provides explicit folder load/search, queue/playlists and Play/Pause/Resume/Stop/Next/Previous/software volume. Rebuilt-app checks covered silent-library browsing, playlist/queue restore after restart, no autoplay, and an earlier nonzero synthetic WAV smoke through AudioQueue with operator-reported audible output. At 7c1b1d0, the full offline gate and app bundle/plist/strict local signature checks passed; focused CLI tests passed 19/19 plus 7/7. Three 20k synthetic process-RSS profiles measured 5,248 / 5,568 / 7,824 KiB medians for no-tag/10%-tagged/all-tagged mixes. These are RSS estimates, not full-app memory qualification. Remaining work: app-wide memory and cold-start profiling, transition latency, multiple-root reconciliation, non-Unix queue persistence, device disconnect/recovery, physical DAC/panel, and Rust Boy shared focus. Optional explicit-load autoplay and native EQ remain follow-on gates. Preserve the zero-artwork policy.

**RM-020 catalog-core profile refresh (2026-10-31, binary built at source
`4afaf1b`):** five release-profile runs used 0/1/100/2k/20k generated
WAV-signature entries. Search p95 median/range was 78.1 µs (74.3–87.8) at 2k
and 745.6 µs (743.1–748.3) at 20k; 20k catalog construction was 599 ms
median (344–672 ms). These are Rust core measurements, not SwiftUI
search/render or whole-app performance acceptance. App startup, full-app
memory, transition qualification, and real tagged-library behavior remain
open; RM-020 stays in progress.

**RM-020 macOS acceptance update (2026-10-31):** SwiftUI chassis source at
`d65efc16f5e694ba40acdac6b4662ae94542b88d` builds and signs; the host
LaunchServices `/usr/bin/open -n` path returned 0 and the UI remained Ready / No
Track. The bundled RBPK Hello/Activate/Shutdown smoke exited 0. A generated
2,000-entry explicit scan and single-match search completed without playback.
App-wide RSS remains unqualified: the same-process resident set fell from a
96,384 KiB idle median to 69,440 KiB after the catalog scan, so page reclamation
confounded the incremental measurement. Device disconnect/recovery, DAC/panel,
and live Rust Boy shared focus remain open; keep RM-020 in progress and PR #1
draft.

**RM-020 desktop 20k continuation (2026-10-31):** standalone CLI catalog and
`serve --desktop` explicitly opt into 20,000 tracks; ordinary host scanning
keeps its default 2,048 limit. IPC caps at 20,000 with 32-row pages. Swift
reads at most 20 MiB of catalog output plus one byte, terminates
over-producing children, and rejects gaps, duplicates, or reordered indexes.
The full offline gate and app bundle build passed. Generated 20k UI load,
last-item search and selection succeeded while stopped; CLI-process peak RSS
was 11,059,200 bytes in a single `ru_maxrss` sample. Full-app 20k memory,
repeatable UI latency, physical DAC/device recovery and live host focus remain
open. Activity Monitor showed one post-scan same-PID pair of 33.5/33.3 MB
anonymousMemory; there is no same-process baseline, and `footprint` could not
read the process through the shell sandbox. RM-020 stays in progress.

**Current candidate recheck (2026-10-31, source HEAD `be6615e`):** the full
offline gate exited 0 (17 local crates, 32 path-only edges, zero registry/Git
crates, zero third-party system links); the app bundle built and passed strict
ad-hoc signature verification; the focused package-process suite passed 7/7.
The current app loaded two generated silent WAVs, showed active AudioQueue
playback, and was quit from its app menu. Activity Monitor reported its CLI
child as Terminated with zero memory and then showed no Rust Music/CLI process.
This adds current graceful-close evidence to RM-019; device disconnect/recovery,
physical output, DAC/panel, live Rust Boy focus, and app-wide incremental memory
remain open. No private corpus was rerun.

**RM-020 app subprocess output hardening and headphone reconnect smoke
(2026-10-31, code `3737959c6750ff8280188a3b8dcc6857be965c31`):** Swift state
commands now enforce the CLI's 6 MiB stdout/stderr bound. Exact-limit output,
over-limit termination, and read-error cleanup pass in the Foundation test
harness. `sh scripts/build-macos-app.sh` and `sh scripts/offline-gate.sh`
exited 0; the latter passed 17 workspace crates and 17/17 simulated AudioQueue
tests. Independent security review found no concrete defect. After reconnect,
External Headphones was selected and unmuted; a generated -40 dBFS test WAV
started and finished through AudioQueue without a surfaced error. This does
not prove acoustic output, DAC fidelity, or AudioQueue invalid-device
recovery. Full-app memory/startup, transition performance, physical device
error/recovery, DAC/panel, and live Rust Boy focus remain open. RM-020 remains
in progress; PR #1 remains draft.

**RM-020 exploratory full-app sample (2026-10-31):** Activity Monitor sampled
the same Rust Music PID at 29.1 MB `anonymousMemory` before a generated 2k
signature-only library, 85.3 MB immediately after scanning, and 32.0 MB on the
next settled reading. During library/search interaction it later varied
50.5–104.0 MB, then read 50.3 MB after five seconds. The host was under memory
pressure (14.17–14.40/16 GB used, 8.39–8.56 GB swap); Open and Save Panel
Service separately used about 58 MB. These values are not a qualified
incremental-memory measure. Search returned one expected match; the 2.124 s
CUA/accessibility round trip is not direct search latency. Playback remained
stopped and no CLI child appeared after scan. Cmd-Q removed the app and CLI
rows; a fresh launch was Ready / No Track with no autoplay. Generated files
were removed. Full-app memory and direct UI latency remain open.

**RM-019 current child-exit regression (2026-10-31):** a fresh feature-gated
debug CLI build exited 0; `python3 scripts/test_macos_parent_exit.py --binary
target/debug/rust-music-cli` exited 0. Normal EOF reaped child PID 43932
(exit 0), malformed protocol reaped PID 43935 (exit 1), three repeated EOF
cycles reaped PIDs 43936–43938, and the real serve child PID 43940 exited
after its app-like parent received SIGKILL. This verifies CLI child lifecycle
with the test sink, not a new GUI active-audio close or physical device
recovery. Earlier GUI active-playback exit evidence remains in the ledger;
RM-020 and hardware gates remain open.

The focused `cargo test --locked --offline -p rust-music-cli --features
macos-output,test-support --test package_process` exited 0 (7/7). The bundled
release CLI completed the RBPK v1 release-process smoke with exit 0 and SHA-256
`3305565f8b4c42598b242a3b498f30caceee2345d042162f3e794322a6f95e4e`. App
memory measurement remains coarse: xctrace is absent (exit 72) and vmmap
cannot inspect the process without additional privileges (exit 255); no
privileged inspection was attempted.

**MAC-02 host output check (2026-10-31):** host `system_profiler` exited 0 and
listed MacBook Air Speakers as the 48 kHz default output. A generated
low-amplitude 440 Hz WAV played by the app-bundled CLI outside the restricted
shell exited 0. The sandboxed attempt returned AudioQueueStart `-66680`
(`kAudioQueueErr_InvalidDevice`); the host retry passed. This records the
execution boundary and native output path, not an acoustic/DAC measurement or
disconnect/recovery test. The physical-device gate remains open.

**MAC-01/02 rebuilt-app confirmation (2026-10-31, bundle built at `5da9393`,
no app/Rust source changes through `17dff90`):** the standalone UI loaded two
generated 60-second WAVs, search returned one track without starting playback,
and explicit Play,
Pause/Resume, active Next/Previous, volume adjustment (99%→98% during active
playback), and Stop reached the expected UI states. Activity Monitor showed
app PID 28923 and CLI child PID 33396 during playback. After Cmd-Q, the old
active instance and child were gone; a fresh Ready / No Track instance was PID
33445 with no CLI child. Closing that idle instance left `cua.listApps()` at
`isRunning=false` and Activity Monitor's refreshed Rust filter empty for Music
and CLI. Generated fixtures were removed; queue/playlist state was not
changed. Physical disconnect/recovery, acoustic capture and DAC fidelity remain
open.

**Native transport and current sandbox launch recheck (2026-10-31, source HEAD
`713c7e9`):** rebuilt bundle, Info.plist path, and strict ad-hoc signature
verified. With two generated silent 16-bit/44.1 kHz WAVs, the UI reached active
AudioQueue playback; settled Next/Previous states, Pause, Resume, and Stop all
matched the requested transitions. Cmd-Q followed by refreshed Activity
Monitor showed no app or `rust-music-cli` process. Sandboxed shell
`open -n` returned 1 (`kLSNoExecutableErr`); `lsregister -f` returned -10822,
and direct executable launch aborted in AppKit registration. The same bundle
launched through CUA. An approved outside-sandbox `/usr/bin/open -n` retry
exited 0; the UI showed Ready / No Track with disabled transport, and Activity
Monitor showed the app with no CLI child. Host LaunchServices launch is
verified; sandbox service access is unavailable in that shell, and
Developer ID/notarized distribution remains unverified. Silent transport is
not audible-output or device-recovery evidence; DAC/panel, device
disconnect/recovery, Rust Boy shared focus, and app-scale memory remain open.

**Synthetic app-memory observation (2026-10-31):** one same-process Activity
Monitor sample rose from 84.1 MB to 87.3 MB `anonymousMemory` while the app held
a generated 2,000-entry catalog of 12-byte RIFF/WAVE signatures. The app showed
playback stopped; no track was played. This single pair is not RSS, exact
catalog heap, a repeated profile, or a passed memory threshold. The app-wide
memory gate remains open.

**Footprint sampler follow-up (2026-10-31):** `/usr/bin/footprint -p 23269
--sample 1 --sample-duration 12 --noCategories` exited 0, but its same-process
series ranged 42–96 MB while the folder picker and scan were active, then
settled at 43 MB. A separate idle sample held at 37 MB. The scan interval did
not isolate retained catalog memory, so no incremental delta is claimed and
the memory gate remains open.

**Refined same-process memory sample (2026-10-31):** separate five-second
footprint series after the folder picker closed measured 2,000 generated
12-byte WAV signatures at +6–7 MB in two fresh app processes. A third run in
byte mode measured `phys_footprint` 28,296,080 bytes idle and 34,046,864 bytes
after scanning (increase **5,750,784 bytes / 5.48 MiB**); five post-scan samples
were identical. These are synthetic 2k app-process results, not a 20k or real
tagged-library memory pass. An explicit 20k app load was rejected with the
configured 2,048-track budget error. The app-wide 20k, cold-start and real
library memory gates remain open.

**Repeated 2k UI search observation (2026-10-31):** five generated catalog
queries returned expected counts 1, 1, 10, 1,000 and 0. Field-set plus
accessibility round-trips ranged **854–1,443 ms** (median **1,099 ms**).
Automation/accessibility overhead is included; this is not direct search
latency or p95. No test entry was played.

**RM-020 same-process physical-footprint profile (2026-10-31):** a temporary
local helper using macOS `proc_pid_rusage(RUSAGE_INFO_V4)` sampled the same
fresh Rust Music PID before and after an explicit generated signature-only
library load. The settled idle physical-footprint median was 31,425,424 bytes;
after loading 20,000 synthetic 12-byte WAV-signature entries it was
50,938,768 bytes, a sampled delta of 19,513,344 bytes (18.61 MiB). The
post-search median was 71,992,256 bytes; that UI/accessibility interaction
adds transient work and is not a catalog-only figure. Last-entry search
returned one result; playback remained stopped and no CLI child was present.
Samples were taken under host memory pressure, from one process/run, without a
repeatable acceptance threshold or representative tagged audio. This is
exploratory full-app process-footprint evidence, not a memory-gate pass. The
portable-in-repo probe is `scripts/profile_macos_process_memory.c`; process
inspection can be denied by macOS privacy policy (`proc_pid_rusage` returned
EPERM for PID 1). The 20k fixture and temporary compiled helpers were removed.

**Candidate gate refresh (2026-10-31, code HEAD `de7c5642`):**
`sh scripts/offline-gate.sh` exited **0** with 17 local crates, 32 path-only
edges, and 17/17 simulated AudioQueue tests. `sh scripts/build-macos-app.sh`
exited **0**, including Swift UI/catalog-state tests and strict local bundle
signature validation. Roadmap/literature validators and `git diff --check`
also exited **0**. Private decoder parity suites were not rerun. These checks
do not close the open host, physical-output, device-error, or full-app
performance acceptance gates.

**RM-019 / MAC-02 reconnect and quit observation (2026-10-31):** with System
Settings showing External Headphones / Headphone port selected, Rust Music
loaded and played a generated two-second -40 dBFS 16-bit/44.1 kHz WAV; the UI
reported AudioQueue PLAYING then PLAYBACK FINISHED. Activity Monitor showed
app PID 44595 and CLI child PID 45461 before Cmd-Q. After quit, the old CLI
row was absent and only a fresh Ready / No Track app PID 45526 remained, with
no CLI child. The synthetic fixture was removed. This is normal-quit process
cleanup and selected-route evidence; no acoustic capture or actual device
error/reopen was observed. `system_profiler` listed no devices and shell
`pgrep` could not access `sysmond`, so System Settings and Activity Monitor
were the direct observations. RM-019 lifecycle evidence is strengthened;
physical output and device-recovery acceptance stay open.

**RM-020 direct Swift catalog-search profile (2026-10-31):** the standalone
app now uses the same pure bounded filter that its optimized Swift harness
tests. On a warmed 20,000-row synthetic catalog, 101 last-item searches
measured **5.749 ms p50 / 6.030 ms p95**; the previous four separate
`localizedCaseInsensitiveContains` checks measured **73.380 / 74.487 ms** in
the same harness. The cached folded UTF-8 index contains 65 payload bytes per
synthetic row (1.3 MB at 20k). Tests cover empty, last-item, artist/path,
Unicode composed/decomposed forms, case folding, controls/NUL and the 512-byte
query cap. `sh scripts/build-macos-app.sh` exited **0**, including this
optimized correctness/profile harness, prior Swift regressions and strict
local signature validation; the full offline Rust gate exited **0**. This
times the production filter only: scan/index construction, SwiftUI rendering,
full interaction latency and an accepted threshold remain unqualified.

The rebuilt bundle was also opened from its explicit path and explicitly
loaded three generated one-second WAV tracks. The Library UI initially showed
three rows; setting search to `nEoN hOrIzOnS` showed exactly the `Neon Horizons`
row and left playback stopped. After Cmd-Q, Activity Monitor showed no Rust
Music app or CLI process (only the transient AutoFill helper); the generated
directory was removed, and a fresh launch returned to Ready / No Track without
autoplay.

Expanded lossless **G4** remains the umbrella milestone for G10–G13, not a claim that ALAC or WavPack is currently supported.

## How Codex uses this map

Start with [AGENTS.md](../AGENTS.md), then [agent routing](CODEX_AGENT_MAP.md) and the selected `RM-###` entry in [roadmap-index.json](roadmap-index.json). Use one code-writing agent per overlapping scope; `gate_verifier` and `safety_reviewer` are read-only. The project-scoped `.codex/agents/*.toml` definitions are optional Codex tooling, not runtime player dependencies.

To validate links, agent names, IDs, statuses, dependency DAG and format-policy alignment without third-party Python packages:

```sh
python3 scripts/check_roadmap_index.py
python3 scripts/check_literature.py
sh scripts/offline-gate.sh
```

The first two are optional maintenance validators. The last is the authoritative Rust-only release gate and must not imply a PASS until it actually completes.

## Portable library and Pip-Boy socket — 2026-10-31 roadmap update

The approved visual direction is [PRIMARY_UI_DIRECTION](design/PRIMARY_UI_DIRECTION.md); the portable socket contract is [PIPBOY_SOCKET_SCHEMA](design/PIPBOY_SOCKET_SCHEMA.md), and the desktop-to-device export protocol is [PORTABLE_LIBRARY_DEPLOYMENT](design/PORTABLE_LIBRARY_DEPLOYMENT.md). These specifications do **not** certify source implementation.

**Delivery dependency:** finish the outstanding RM-020/macOS v1.0 playback and audio-focus work first. Parallel read-only socket/manifest inventory is acceptable; do not conflate standalone Mac release and live Pip-Boy hardware certification.

| Task | Priority | Route | Deliverable |
| --- | --- | --- | --- |
| RM-021 | P0 | host_integrator | Existing library, IPC and RBPK contract inventory |
| RM-022 | P1 | host_integrator | Versioned bounded portable manifest, stable ID and sort fixtures |
| RM-023 | P1 | host_integrator | Atomic portable export/import and validated incremental plan |
| RM-024 | P1 | host_integrator | macOS deploy-to-device flow |
| RM-025 | P1 | host_integrator | Pip-Boy socket consumption with host-owned audio focus |
| RM-026 | P2 | host_integrator initially; split platform ownership later | Linux and Windows desktop adapters |
| RM-027 | P2 | host_integrator + read-only gate_verifier | Physical Pi/Pip-Boy and removable-media acceptance |

RM-022 and RM-023 are **in progress**, with partial bounded manifest and pure delta-planner source/tests included in the offline gate at `f8052f28`. The `RMSYNC01` manifest and deterministic planner are data-model components only; they do not constitute a package writer, exporter, or importer. Playlists/full metadata, trusted computed digests, lossless admission, complete cross-platform Unicode/path policy, safe source hashing/open, atomic publication, cancellation/recovery, and destination validation remain open. RM-021 and RM-024–RM-027 remain **planned**. These documents and source tests do not certify release readiness or publisher behavior. Do not ship ornamental image/skin assets from the approved reference while the text-only/zero-artwork release policy remains in force; any future visual-policy change requires a separate gated decision. Update `docs/roadmap-index.json` together with this section and run the existing index checker before declaring the registry valid.

## RM-020 synthetic library workflow smoke — 2026-10-31

The rebuilt standalone app explicitly loaded three generated one-second,
stereo, 16-bit/44.1 kHz silent WAVs and displayed all three tracks without
starting playback. A single click on the visible `Workflow Check` playlist
selected it and enabled its actions; adding the selected track changed the
playlist count to one. Queue Playlist added one entry while transport remained
stopped (Play enabled, Pause disabled). The test playlist and queue entry were
removed, the app was quit and relaunched to Ready / No Track, and the temporary
fixture was removed. This verifies a small UI workflow only; RM-020 remains in
progress, with broader search/restart/performance and release acceptance still
open.

A second generated-library run created a one-track named playlist and queue,
then force-quit the app after both writes completed. The fresh launch returned
to Ready / No Track without scanning or playback; after an explicit reload,
both the playlist row and queue item were restored, with Play enabled and Pause
disabled. The test entries and temporary files were removed. This is restart
acceptance after completed writes, not an interruption-during-write guarantee.

## RM-020 current gate refresh — 2026-10-31

At product source `1941cadab95eae3490bf4d599748acb60b21b8b1` (documentation
HEAD `793638f21be9b2691e39fe36e581d91365e31c60`),
`sh scripts/offline-gate.sh` exited **0**: 17 local crates, 32 path-only
internal edges, zero registry/Git crates, zero third-party system links, and
17/17 simulated AudioQueue tests. The focused package-process integration
suite passed **9/9**; `sh scripts/build-macos-app.sh` exited **0** with 18/18
symlink guards, lifecycle/process-drain harnesses, catalog/UI checks, and strict
local ad-hoc bundle signature validation. The current optimized 20k catalog
harness measured search at 5,626.208 µs p50 / 5,770.333 µs p95 and indexed
construction at 116.770 / 120.649 ms p50/p95. These are synthetic core timings,
not full scan or interactive UI measurements.

Three fresh-app-process explicit scans of the same generated 2,000-track
metadata-only profile measured physical-footprint deltas of 1.328–2.438 MiB;
tracks had no audio frames and playback stayed stopped. This is a limited
synthetic app measurement, not a representative-library threshold or startup
qualification. Shure analog headphone reconnection returned the System Settings
route to External Headphones, but no AudioQueue error/reopen transition was
captured. RM-020 remains **in progress**: real invalid-device recovery, direct
interactive performance, representative-library behavior, physical DAC/panel,
live Rust Boy focus and distribution acceptance remain open. No private codec
corpus was rerun; PR #1 remains draft and no remote synchronization occurred.

**RM-020 current-bundle tagged-library UI exercise (2026-10-31, product
source `1941cadab95eae3490bf4d599748acb60b21b8b1`):** the rebuilt bundle
explicitly loaded 2,000 generated 187-byte FLAC metadata-only entries with
unique titles and bounded TITLE/ARTIST/ALBUM comments, but no audio frames.
Visible search counts matched the fixtures for an artist (100), album (50),
missing query (0), exact title (1), and broad title (2,000). Six set-value plus
accessibility readback round trips took 901–1,421 ms (median 1,030 ms); the
measure includes CUA/AX overhead and is not direct UI latency or an acceptance
threshold. A temporary named playlist was created, the selected track was
added, and playlist queueing showed one queued track while playback remained
stopped. Queue and playlist test entries were removed, the app quit, and the
fixture was deleted. This extends current-bundle search and selection-only
queue evidence; representative-library behavior and direct interactive latency
remain open. No private media was used.

**RM-006 / RM-018 current-bundle broker recheck (2026-10-31):** on the clean
Pip-Boy checkout at `bfbb9c76d147911feb6d359ba0f2342a8a0cd97e`, the focused
offline `actual_rust_music_child_runs_under_host_supervisor_when_configured`
test passed **1/1** against the current Rust Music app-bundled CLI (product
source `1941cadab95eae3490bf4d599748acb60b21b8b1`, SHA-256
`14fe5c2cdd3a611b715a2a2fad626c4342bdc2a303da3005ec62e1c7b19dcaff`). It
observed supervised handshake/activation, MUSIC excluding RADIO, suspend
releasing the claim, RADIO acquisition, resume reacquiring MUSIC, and shutdown
releasing the claim. Build output was directed to temporary storage and
Pip-Boy source stayed clean. This does not open the audio sink, verify audible
MUSIC output or TV contention, register/allowlist the package, or demonstrate
live shell/mobile acceptance. RM-006 and RM-018 remain in progress.

A fresh-target source-build rerun also passed on 2026-10-31: Pip-Boy's
`tools/test_supervised_music_process.py` built the current Rust Music CLI
offline into a new temporary target (exit **0**, binary SHA-256
`67611eea7afaf6c91bdf85086a19dc613203e73534a79c2d116a67fdbf83edc0`) and
passed the supervised-child broker test **1/1** (304 filtered). The Rust Music
checkout and sibling Pip-Boy checkout remained clean. It tests inert protocol
activation and MUSIC/RADIO broker suspend/resume/shutdown without opening the
sink; live audible output, TV contention and package roster/mount acceptance
remain open.

## RM-020 current-HEAD and root-scope recheck — 2026-10-31

At documentation HEAD `39a3c3007d8de5fb9eb3afebfafa740fef9cd696`,
`sh scripts/offline-gate.sh` exited **0**: 17 local crates, 32 path-only edges,
zero registry/Git crates, zero third-party system links, and 17/17 simulated
AudioQueue tests. The focused package-process suite passed **9/9**. The
macOS-app build exited **0**, including 18/18 symlink-path guards,
shutdown/process-drain harnesses, catalog/search and UI-state checks, and
strict deep ad-hoc bundle signature validation. The current 20k optimized
catalog harness measured search at 5,684.583 µs p50 / 6,107.083 µs p95,
indexed construction at 117.226 / 120.591 ms p50/p95, and 1,300,000 bytes of
cached-index payload. These synthetic core timings do not establish
representative-library, app-wide memory, cold-start, or interactive-latency
acceptance.

In a current-bundle UI exercise, two generated roots each contained two
12-byte RIFF/WAVE signatures without PCM. Root A had one queued item and a
named playlist with one entry. Loading root B showed an empty queue and
playlist list; returning to root A restored both records. Playback stayed
stopped. This is direct in-session evidence of per-root queue and playlist
isolation, while multiple-root reconciliation, multiple-root restart,
removable-media recovery, representative library behavior, physical device
recovery, DAC/panel, live Rust Boy focus, and distribution acceptance remain
open. The fixture was removed and private decoder corpora were not rerun; keep
RM-020 in progress and PR #1 draft.

The current rebuilt app also explicitly loaded a generated 20-second mono
16-bit/44.1 kHz WAV at approximately -46 dBFS and remained stopped until Play.
The UI reported AudioQueue Playing → Paused → Playing on Resume → Playback
Stopped on explicit Stop. After Cmd-Q, the app inventory reported
`isRunning=false`, Activity Monitor had no `rust-music-cli` row, and the Rust
Music app process disappeared on refresh. This is current-bundle control and
process-exit evidence; it is not acoustic loopback, headphone confirmation,
DAC fidelity, or real invalid-device recovery. The fixture was removed.

The 2,000-track tagged app-memory recheck was inconclusive. Five idle
`proc_pid_rusage` physical-footprint samples were 40,354,680 B; after the
explicit scan, the median was 35,832,720 B (delta -4,521,960 B), while median
resident size increased by 3,457,024 B. Activity Monitor showed 14.11 GB
memory used and 9.31 GB swap. The opposing metrics under host pressure do not
pass the 8 MiB target. The catalog profile driver also exited 1 because this
environment denies `/bin/ps`; the library example built successfully, but its
RSS run is unavailable here. Keep app-wide memory acceptance open.

The profiler now supports an optional `--memory-probe` path using the existing
local C helper and `proc_pid_rusage(RUSAGE_INFO_V4)` resident size, retaining
`/bin/ps` as the default. Three-repeat catalog-core RSS deltas were 1,104 KiB
median for 2k all-tagged FLAC metadata, 7,616 KiB for 20k all-tagged FLAC
metadata, and 5,264 KiB for 20k untagged WAV signatures. All 20k all-tagged
repeats were below 8,192 KiB. The fixtures had no audio frames; these are
catalog child-process estimates and do not qualify full-app memory.
The full Rust offline gate exited 0 in the same worktree: 17 local crates,
32 path-only edges, zero registry/Git crates, and 17/17 simulated AudioQueue
tests.

**RM-020 repeated full-app synthetic catalog footprint (2026-10-31):** three
fresh-process runs measured settled physical-footprint deltas of +2.625 MiB,
+2.984 MiB, and +2.094 MiB after an explicit 2,000-track metadata-only
synthetic FLAC scan; median delta was +2.625 MiB. Fixtures had bounded
TITLE/ARTIST/ALBUM comments and no audio frames, and playback stayed stopped.
Sampling used the local `proc_pid_rusage(RUSAGE_INFO_V4)` helper. Resident-size
readings fluctuated substantially while the host reported about 14 GB memory
use and 9.4 GB swap. This supports a bounded 2k synthetic profile only; a
representative library and 20k app profile remain unmeasured, so app-wide
memory acceptance stays open. The app exited and its final PID no longer
existed; temporary fixtures and helper were removed.

**RM-020 current-bundle 20k tagged catalog profile (2026-10-31):** two fresh
app processes explicitly loaded 20,000 generated 175-byte FLAC metadata
signatures with bounded TITLE/ARTIST/ALBUM comments, no frames, and playback
stopped. Settled `proc_pid_rusage` physical-footprint deltas were +16.969 MiB
and +8.844 MiB. The second run's immediate post-scan samples ranged from 45.9
to 53.0 MB before settling at 42.6 MB; the host had about 14.3–14.4 GB of 16
GB in use and 9.5–9.7 GB swap. A repeated UI load took 3.185 seconds from Load
action through AX readback. A path search for the last item returned one
result in 3.692 seconds including CUA/AX overhead. Treat these as synthetic
observations only: host pressure and divergent app-memory deltas leave
repeatable full-app memory acceptance open. The app and final PID exited and
temporary files were removed.

**RM-005 local AirPlay discovery (2026-10-31):** the macOS Sound Output list
showed External Headphones (selected), MacBook Air Speakers, and ASUS VG32V
HDMI. A three-second Bonjour browse found RAOP/AirPlay service names `PowerAmp
Living Room`, `White`, `Black`, and this Mac's `null’s Laptop`. The discovery
commands were stopped with SIGINT after each bounded browse (exit -2). The
names do not prove which are UniFi or iHome hardware, and no candidate was
available in the Sound Output list. No output route was changed or played.
Physical AirPlay output and device-recovery acceptance remain open.
