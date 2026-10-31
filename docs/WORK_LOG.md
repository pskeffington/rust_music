# Rust Music work log

This log records observed repository work and the next release gates. The
machine-readable task state is authoritative in [roadmap-index.json](roadmap-index.json).
Private audio paths and recordings are deliberately excluded.

## 2026-10-31 — Connect standalone text library, queue, and playlists

- Added the macOS UI workflow for explicit library selection, cancellable
  catalog load, bounded text search, persistent queue editing/navigation,
  repeat/shuffle, named playlist CRUD, playlist-to-queue, and explicit
  Play/Stop. Queue selection and Previous/Next remain selection-only.
- At implementation commit `2546cae`, the CLI queue/playlist state tests passed
  12/12 within `sh scripts/offline-gate.sh` (exit 0); the gate covered 17 local
  crates and 32 path-only edges with no registry/Git crates. The macOS app
  bundle build/signature check exited 0. Roadmap/literature validators and
  `git diff --check` exited 0; app image/visualizer symbol search had no hits.
- The UI remains text-only and uses no new runtime dependencies. Queue path
  validation occurs before managed playback. Pause/volume, native host focus
  transport, opt-in autoplay, EQ, cold startup/incremental-memory profiles,
  hardware output and live Radio/TV contention remain open. Private codec
  parity was not rerun.

## 2026-10-31 — Add durable playlists and cancellation-aware scans

- Added `rust-music-library::playlist_store` with named playlist CRUD,
  versioned `.rmp` persistence, 128-playlist/10,000-entry/1 MiB bounds, safe
  relative paths, same-directory atomic replacement, and malformed-state and
  symlink refusal. This stores text only; it never opens or modifies media.
- Added `scan_cancellable` with caller-owned cancellation and no partial catalog
  return. Added a deterministic cancellation regression and mapped the typed
  cancellation result in the CLI error response.
- At commit `e5d6b34`, `cargo test --locked --offline -p rust-music-library`
  passed 27/27; `python3 scripts/check_roadmap_index.py`,
  `python3 scripts/check_literature.py`, and `git diff --check` exited 0.
  `sh scripts/offline-gate.sh` exited 0: 17 local crates, 32 path-only edges,
  0 registry/Git crates, full workspace tests, release build and macOS feature
  check. An initial attempt exited 101 on an exhaustive CLI error match; fixed
  before the passing run. Private decoder parity was not rerun.
- RM-020 remains in progress. Standalone library/search/playlist/queue UI,
  persistent queue-to-host wiring, opt-in explicit-load autoplay, Pause,
  equalizer, measured startup/incremental memory, native device recovery, live
  Radio/TV contention, panel/DAC, and distribution signing remain open.
- At app commit `8f0d64d`, removed SF Symbol image widgets from the macOS shell
  so its header, transport buttons, status and policy notice render as text
  only. `rg -n 'Image\\(|systemImage|AsyncImage|NSImage|UIImage|visualiz'
  apps/rust-music-macos crates/rust-music-panel crates/rust-music-ipc` found
  no matches, and `sh scripts/build-macos-app.sh` exited 0 with strict local
  bundle signature verification. The app remains a single-file inspector with
  Play/Stop; no library browser workflow is claimed.

## 2026-10-30 — Add WavPack high-quality cross-channel predictor coverage

- Extended the bounded decorrelation subset with terms -2/-1 and lag terms
  4/5. Cross-channel inverse reconstruction now restores the dependent channel
  before using it as the second predictor and applies the cross-term weight
  clamp; unrelated predictor terms and mono cross-channel streams remain
  rejected.
- Added a Tiny Encoder 4.40 `-h` pure-lossless two-block vector over the same
  deterministic 20,000-frame stereo PCM source as the default-mode fixture.
  The encoded terms are `[18,18,18,-2,2,3,5,-1,17,4]`; Rust matches all 40,000
  samples and FFmpeg 9.0.2 decodes byte-for-byte to the existing PCM golden.
  WavPack SHA-256:
  `8db14f06ecb6c44d5282908908bf575a1d2d471ada391ac960cbc5fa71899cec`; shared
  PCM SHA-256 remains
  `f73bb995471938f0edc927a840f4de77e65da3f03a5e3e0e5a376eb6e79b7e35`.
- `cargo fmt --all -- --check` and `cargo test --locked --offline
  -p rust-music-wavpack` passed (48/48). `sh scripts/offline-gate.sh` exited 0
  on 17 local crates and 32 path-only edges, with zero registry/Git crates;
  roadmap/literature validators and `git diff --check` passed.
- This vector establishes only the tested Tiny Encoder high-mode profile.
  WavPack remains unregistered; native real-world WavPack files, other
  predictor profiles/depths, trailing tags, hybrid correction, and playback
  host acceptance remain open.

## 2026-10-30 — Extend WavPack default-mode predictor coverage

- Added bounded decorrelation predictor terms 2 and 3, which extend the
  previously verified 17/18 subset. Terms outside 2, 3, 17, and 18 remain
  rejected. The predictor state keeps a fixed eight-slot history per channel;
  metadata histories are length-checked for each admitted term.
- Added `tiny-encoder-4.40-random-default-stereo16.wv`, a synthetic 20,000-frame
  stereo stream produced by Tiny Encoder 4.40 default pure-lossless mode. Both
  blocks carry terms `[18, 18, 2, 17, 3]`. Rust matches all 40,000 integer
  samples, and FFmpeg 9.0.2 independently decodes the fixture byte-for-byte to
  the retained PCM golden. WavPack SHA-256:
  `551802997b9ee302a74d387c7dace173b9844ccedc950c112a65f2ecc788df0e`; PCM
  SHA-256: `f73bb995471938f0edc927a840f4de77e65da3f03a5e3e0e5a376eb6e79b7e35`.
  The fixture README records its deterministic recipe and encoder provenance.
- Focused validation passed: `cargo test --locked --offline
  -p rust-music-wavpack` (47/47), including exact term admission and malformed
  history-length rejection. `ffprobe` identified the stream as stereo 44.1 kHz
  WavPack and the FFmpeg decode `cmp` passed.
- `sh scripts/offline-gate.sh` exited 0 on the updated source: 17 local crates,
  32 internal path-only edges, zero registry/Git crates; formatting, warnings-
  denied Clippy, workspace tests, release build, and macOS feature check passed.
  Roadmap and literature validators plus `git diff --check` also passed.
- WavPack remains unregistered and unadmitted. Real-world native `.wv` files,
  other predictor terms/depths, trailing tags, hybrid correction pairing, and
  playback/host acceptance remain open.

## 2026-10-30 — Smoke-test the local release package process

- Built the macOS release CLI with
  `cargo build --locked --offline --release -p rust-music-cli
  --features macos-output` (exit 0). Added and ran
  `python3 scripts/test_release_package_process.py --binary
  target/release/rust-music-cli` (exit 0); the helper sends correctly framed
  RBPK v1 requests and verifies Hello negotiation, an inert empty Activate
  view, Shutdown acknowledgment, clean exit within three seconds, and empty
  stderr. It never builds or installs the executable itself.
- Release CLI SHA-256:
  `d1de04c2d8e3af36b75fa5fd5eb8147245144db87dbb6ab485170fd58beb3ffa`.
  This identifies the locally built executable only; Pip-Boy has not reviewed
  or allowlisted this digest, and no live package mount or shared audio-focus
  contention was exercised. RM-018 remains in progress.
- Updated G5, RM-018, package-process evidence, and the repository index. No
  Pip-Boy files were modified.

## 2026-10-30 — Recheck bounded package-unit protocol

- Re-ran the unit codec, in-memory endpoint checks, and spawned macOS process
  lifecycle: `cargo test --locked --offline -p rust-music-ipc` (7/7),
  `cargo test --locked --offline -p rust-music-cli --features macos-output
  package_stream_` (4/4 endpoint tests), and
  `cargo test --locked --offline -p rust-music-cli --features macos-output
  --test package_process` (1/1 subprocess test); all exited 0.
- `python3 scripts/check_roadmap_index.py` and `git diff --check` exited 0.
  The unit contract and RM-018 index already cover the fixed frame, field,
  catalog, queue, and lifecycle limits; no protocol change was needed. These
  local checks do not prove Pip-Boy allowlisting, live mounting, shared audio
  contention, or platform acceptance.
- `sh scripts/offline-gate.sh` also completed with its final PASS: 17 local
  crates, 32 path-only edges, zero registry/Git crates, workspace tests,
  release build, and macOS feature checks.

## 2026-10-30 — Verify package identity rejection in child process

- Extended the macOS subprocess test to send a syntactically valid Hello for
  `org.rustboy.radio` to the Rust Music unit. The child returns no protocol
  frame and exits unsuccessfully within the three-second deadline. The happy
  path still proves hello, inert activation, shutdown acknowledgment, and clean
  exit.
- `cargo fmt --all -- --check` and the focused check
  `cargo test --locked --offline -p rust-music-cli --features macos-output
  --test package_process` pass; the subprocess suite is now 2/2. The first
  formatter check identified line wrapping, which was corrected before the
  successful rerun. The subsequent `sh scripts/offline-gate.sh` completed
  with final PASS across 17 local crates, 32 path-only edges, zero registry/Git
  crates, workspace tests, release build, and macOS feature checks. Roadmap and
  literature validators and `git diff --check` also exited 0.

## 2026-10-30 — Add CC0 recording-derived WavPack stress parity

- Verified Internet Archive item `hr102` identifies track 6 “SPOT” under an
  uploader-supplied CC0 1.0 Universal label and public-domain declaration.
  The 32-bit/44.1 kHz source FLAC SHA-256 is
  `bddf6b1f45de52810a75679a853c425cb36ff2883d5fdd7bbf310170b4114b50`; the
  source FLAC is not stored in the repo. FFmpeg 9.0.2 converted it to a
  packed-24-bit PCM test target; this conversion changes samples, so no parity
  claim is made against the original 32-bit FLAC.
- Tiny Encoder 4.40 fast lossless mode encoded the derived PCM to legacy stream
  version `0x405`. Rust matched all 1,063,147 stereo frames sample-by-sample,
  streaming comparisons one decoded WavPack block at a time. FFmpeg 9.0.2 also
  decoded the `.wv` byte-for-byte against the derived PCM. WavPack SHA-256:
  `fdc150e9202e92501f5b11738e931d853dceee933c2678e1de45f12630b65c24`; PCM
  SHA-256:
  `5cf11ed0fc0f0097b458969e967a470948a2368c855501a2e50ff016c79d3917`.
- The fixture README records item/license provenance, conversion caveat,
  hashes, and reproduction steps. Only the derived `.wv` and packed PCM golden
  are included; no source FLAC, third-party encoder code, or executable is in
  the repo or workspace.
- `cargo fmt --all -- --check`, `cargo test --locked --offline
  -p rust-music-wavpack` (45/45), the full offline gate (17 local crates, 32
  local path edges, no registry/Git crates), roadmap/literature validators,
  and `git diff --check` all passed. Native WavPack inputs, trailing tags,
  further independent encoders, and hybrid correction remain open.

## 2026-10-30 — Verify packed-16/24 WavPack with Tiny Encoder 4.40

- Used the official BSD-licensed Tiny Encoder 4.40 source as a temporary
  independent oracle outside the repository. Its default compression profile
  uses unsupported decorrelation terms and remains rejected; fast mode emits
  supported lossless terms. Tiny Encoder emits legacy stream version `0x405`,
  while the matching FFmpeg 9.0.2 fixtures use `0x410`.
- Exact parity passed for 4,410 high-entropy stereo frames at packed 16-bit and
  packed 24-bit. The packed-24 fixture revealed the decoder only admitted
  packed 16-bit or 24-in-32 streams, so the bounded profile gate now admits
  3-byte packed 24-bit PCM and enforces its signed range. The independent
  stream also contains optional duplicate entropy metadata; the decoder skips
  optional metadata while still rejecting unknown mandatory metadata and
  duplicate required metadata.
- Tiny Encoder stream SHA-256 values:
  16-bit `0a7b48c427168dcdc1e2c67b6321e3b318839888f568a8758fcae7319fc76019`;
  24-bit `4dea0dc5820f32c7f1a6bf0febc094ae2e5a2aeb7f9996234a91bc5c196bf5aa`.
  The external archive SHA-256 and full synthetic generation recipe are in the
  fixture README. No reference code or executable was added to the repository
  or Cargo workspace.
- `cargo fmt --all -- --check` and `cargo test --locked --offline
  -p rust-music-wavpack` passed (44/44). `sh scripts/offline-gate.sh` passed
  across 17 local crates and 32 path-only edges with no registry/Git crates;
  roadmap/literature validators and `git diff --check` also passed. WavPack
  remains unregistered: real-world files, other independent encoders, trailing
  tags, hybrid correction pairing, and playback admission remain open.

## 2026-10-30 — Add high-entropy padded-24-bit WavPack parity

- Generated deterministic 4,410-frame stereo signed-24-bit PCM with full-range
  extrema and pseudorandom samples, then encoded it with local FFmpeg 9.0.2.
  Rust's bounded decoder accepts the exact padded-24-bit `INT32_INFO` profile
  and matches all 8,820 samples over its 4,096- and 314-frame blocks. FFmpeg
  also decodes the tagless stream byte-for-byte to the original generated PCM.
- WavPack SHA-256:
  `9e51d6b43aca74ca6204135788fc886d93b31d74f51f53e50233ac43472ad243`;
  PCM SHA-256:
  `9c4ab90d7e5c55513d72afef6f1d8397ea6ff7cd61bd1e7d249ae191327c82cc`.
  The exact generation recipe and hashes are in the fixture README.
- The focused test `cargo test --locked --offline -p rust-music-wavpack
  decodes_deterministic_noise_padded_24_bit_stereo_blocks_exactly --
  --nocapture` exited 0 (1/1). This broadens generated encoder coverage; it
  does not close independent-encoder, real-world, hybrid-correction, or
  playback-admission gates. The full crate suite passed 42/42, followed by
  `sh scripts/offline-gate.sh` (17 local crates, 32 local path edges, no
  registry/Git crates), `python3 scripts/check_roadmap_index.py`,
  `python3 scripts/check_literature.py`, and `git diff --check`; all passed.

## 2026-10-30 — Add bounded padded-24-bit WavPack profiles

- Extended the WavPack decoder to accept only the observed padded-24-bit
  `INT32_INFO=[0, 8, 0, 0]` profile in addition to packed 16-bit. The decoder
  checks the WavPack decoded-sample checksum before validating signed 24-bit
  range; it does not claim support for general 32-bit PCM, other integer
  layouts, or WVX data. Extended layouts are explicitly tested for exact
  metadata admission/rejection.
- Added generated 4,410-frame mono and stereo streams with 4,096- and
  314-frame blocks, plus packed signed-24-bit PCM golden data. All four streams
  (two blocks each) match the source PCM exactly. WavPack remains
  unregistered/unadmitted.
- `cargo test --locked --offline -p rust-music-wavpack`: **exit 0**, 41/41;
  this includes padded-24-bit PCM parity, exact `INT32_INFO` admission tests,
  the 16-bit mono/stereo corpus, and malformed stream bounds.
- Stereo WavPack SHA-256:
  `a7b51346754d60ab260220c2f1ceb3ef0f2fce312deb8aaaf49db105bcd3e30a`.
  stereo PCM SHA-256:
  `909cfa8bbc39faaa86887d125a08b65910633b3ef02a0d5529237ca0b0345edf`.
- Mono WavPack SHA-256:
  `9a6f0627d0c48150ecf8428d3b5bf07edea3601938f6b9947d91ad5a7f870c12`.
  Mono PCM SHA-256:
  `7049d5b60f120cc6d5902590fc9b5ac30458b577ca2ed4f2ba977e0bbc362357`.
- Updated the G10/G12 ledgers, RM-014 blocker, fixture guide, and repository
  index. `sh scripts/offline-gate.sh` exited 0 across 17 local crates and 32
  path-only edges, with zero registry/Git crates; warnings-denied Clippy,
  workspace tests (including WavPack 41/41), release build, and macOS feature
  checks passed. The roadmap/literature validators and `git diff --check`
  exited 0.

## 2026-10-30 — Add a deterministic WavPack stereo stress vector

- Added a 4,410-frame deterministic pseudorandom 16-bit stereo PCM source and
  an FFmpeg WavPack fixture. The two contiguous blocks use joint stereo for
  4,096 frames and true stereo for the final 314 frames. The checked-in PCM
  source remains the independent golden; no third-party music is included.
- Rust decoded all 8,820 interleaved samples exactly, including the mode
  transition; the block decoder validates each decoded-sample checksum. The
  focused test `cargo test --locked --offline -p rust-music-wavpack
  decodes_deterministic_noise_stereo_multiblock_pcm_exactly -- --nocapture`
  exited 0. FFmpeg is the only local WavPack encoder available, so independent
  encoder and real-world coverage remain open.
- The first full offline-gate attempt stopped at rustfmt for line wrapping in
  the new test. After `cargo fmt --all`, `sh scripts/offline-gate.sh` exited 0:
  17 local crates, 32 path-only edges, zero registry/Git crates, warnings-
  denied Clippy, workspace tests (including WavPack 38/38), release build,
  and macOS feature checks passed. Roadmap/literature validators and
  `git diff --check` exited 0.
- WavPack SHA-256: `b40c22e2e5e0ab4cecd1545b4121c6177e09616053234d28edc7ce5e87db567d`.
  PCM SHA-256: `83e264462b2b805777a3eab217f79e3cddbc92be2411c160639eb130ac5a27af`.
- Updated the fixture recipe, RM-014 blocker, G10/G12 evidence, and tracked the
  PCM fixture explicitly through `.gitignore` exceptions.

## 2026-10-30 — Fix WavPack stereo adaptive-weight parity

- The bounded true-stereo decoder's first mismatch was at interleaved sample
  2,101 and differed by one PCM unit. The WavPack technical description and
  5.8.1 decoder source show that positive-term predictor weights are not clipped
  to unit gain; the local updater incorrectly clamped them at ±1024. Removed
  that clamp and added a direct regression for both sides of the former bound.
- Replaced the fail-closed true-stereo checks with exact PCM regressions. The
  mixed-mode five-block stream and the all-true-stereo five-block stream now
  match all 4,410 stereo frames (8,820 interleaved samples) against FFmpeg
  9.0.2 PCM, with every decoded block checksum validated. WavPack remains
  unregistered and unadmitted pending broader compatibility evidence.
- Focused tests: `cargo test --locked --offline -p rust-music-wavpack
  multiblock_stereo -- --nocapture` and
  `cargo test --locked --offline -p rust-music-wavpack
  decodes_ffmpeg_true_stereo_multiblock_pcm_exactly -- --nocapture`: **exit 0**.
- `cargo test --locked --offline -p rust-music-wavpack`: **exit 0**, 37/37.
  `sh scripts/offline-gate.sh`: **exit 0** across 17 local crates, 32
  path-only edges, zero registry/Git crates, and one OS framework link; fmt,
  warnings-denied Clippy, workspace tests, release build, and macOS feature
  checks passed. `python3 scripts/check_roadmap_index.py` and
  `git diff --check` also passed. The decoder remains unregistered; the
  remaining RM-014 work is broader encoder/real-world and profile coverage.

## 2026-10-30 — Characterize WavPack stereo multiblock failure

- Added generated FFmpeg 9.0.2 16-bit stereo WavPack streams with five
  contiguous blocks: one mixed joint/true-stereo file and one all-true-stereo
  file. The first joint-stereo 1,024-frame block matches exactly. True-stereo
  blocks are now refused explicitly before reconstruction.
- A temporary diagnostic bypass confirmed the original mixed-mode stream's
  second block has a one-unit PCM mismatch at interleaved sample index 2,101
  and fails the decoded-sample checksum; checksum enforcement was restored,
  then the decoder was narrowed to reject the currently unsupported true-
  stereo mode before it can emit PCM. WavPack remains unregistered/unadmitted.
- `cargo test --locked --offline -p rust-music-wavpack`: **exit 0**, 36/36.
  The mono five-block stream continues to match all 4,410 frames exactly.
- `sh scripts/offline-gate.sh`: **exit 0** across 17 local crates and 32
  path-only edges, including formatting, Clippy, tests, release build and macOS
  feature checks. Roadmap validation and `git diff --check` passed. Broader
  WavPack parity and true-stereo decoder support remain open.

## 2026-10-30 — Harden package-unit admission checks

- Added serve-loop regressions proving the Rust Music unit rejects a mismatched
  package identity and an Activate request before Hello. Both failures produce
  no reply and do not initialize the audio sink; the existing skipped-request
  ID regression remains in place.
- `cargo test --locked --offline -p rust-music-cli --features macos-output`:
  **exit 0**, 10 unit tests and 1 spawned child lifecycle test passed.
- `sh scripts/offline-gate.sh` reached its final success marker after the
  workspace tests, release build and macOS feature checks. The complete Rust
  gate passed across 17 local crates and 32 path-only edges.
- This strengthens local process-boundary evidence only. It does not verify a
  reviewed release binary, live Pip-Boy adapter mount, shared audio contention,
  or device acceptance; RM-006 and RM-018 remain open for those host proofs.

## 2026-10-30 — Establish the Rust Music package-unit contract

- Added `docs/PACKAGE_PROCESS.md` as the Rust Music endpoint-side contract for
  `org.rustboy.music` over RBPK v1: wire bounds, lifecycle side effects,
  host-only authorization/audio ownership, deadlines, and the distinction
  between local child-process evidence and production host acceptance.
- Linked and indexed the contract, aligned RM-018's blocker text with the
  recorded implementation, and kept RM-018 in progress because no reviewed
  release binary is mounted through live Pip-Boy startup.
- `cargo test --locked --offline -p rust-music-ipc`: **exit 0**, 7/7 tests.
  `cargo test --locked --offline -p rust-music-cli --features macos-output
  --test package_process`: **exit 0**, 1/1 spawned child lifecycle test.
- `sh scripts/offline-gate.sh`: **exit 0** across 17 local crates and 32
  path-only edges; offline audit, formatting, warnings-denied Clippy, workspace
  tests, release build, and macOS feature checks passed. The first attempt
  stopped on rustfmt in the existing WavPack source; formatting was applied
  before the passing run.
- `python3 scripts/check_roadmap_index.py`,
  `python3 scripts/check_literature.py`, and `git diff --check`: **exit 0**.
  No Pip-Boy files were edited in this pass.

## 2026-10-30 — Bound package-unit lifecycle and in-flight work

- Expanded `docs/PACKAGE_PROCESS.md` with the unit process state sequence
  (startup, ready, shutdown, fault/EOF), invalid-frame termination behavior,
  serialized one-request-at-a-time semantics, and the host supervisor's
  deadline/kill/reap responsibilities.
- Indexed the operational limits already enforced by the endpoint: one
  request/reply in flight, a one-element request/reply worker channel, and
  default library scan caps of 8,192 entries, 2,048 tracks, and depth 6.
  The contract explicitly states that byte bounds do not guarantee wall-clock
  completion and that child-process isolation is not an OS sandbox.
- No implementation behavior changed and no tests were run for this
  documentation-only clarification. RM-018 remains in progress pending a
  reviewed packaged executable, live host mount, shared audio contention, and
  platform acceptance evidence.

## 2026-10-30 — Validate WavPack v5 encoded-block checksums

- Added fail-closed checksum validation to `read_block` for WavPack v5 blocks.
  It validates checksum metadata shape, the required header flag, 16-bit folded
  and 32-bit checksum values, and every metadata boundary before returning the
  encoded block to downstream decoders.
- Synthetic regressions validate both widths, reject a covered metadata bit
  flip, and reject a header flag with no checksum metadata. The official
  WavPack v5 format and reference implementation show that this checksum is
  over encoded block words and is distinct from the header checksum over
  decoded audio samples. The latter remains unimplemented; the codec remains
  unregistered.
- `cargo test --locked --offline -p rust-music-wavpack` passed 30/30.

## 2026-10-30 — Record WavPack stereo reconstruction evidence

- Current RM-014 source tests reconstruct decorrelated and joint-stereo samples
  for a generated 882-frame stereo fixture and compare all 1,764 interleaved
  samples to the retained PCM source. Updated G12, G10, fixture provenance and
  roadmap blocker text to reflect this evidence without claiming full WavPack
  support.
- `cargo test --locked --offline -p rust-music-wavpack` passed 28/28 in the
  latest full offline gate. CRC validation, mono PCM parity, broader independent
  encoders/real recordings, multiblock decode, and hybrid correction remain
  incomplete.

## 2026-10-29 — RM-014 bounded entropy word sequencing

Added a stateful bounded WavPack mono/stereo word stage around the existing
median residual decoder. It resolves the paired unary-count state, bounded
escape prefixes, interleaved channel sequence, and zero-run reset. Per-word
and whole-frame-batch failures restore decoder state and bit cursor; frame
requests above the block limit fail before allocation. Synthetic tests cover
mono pair behavior, zero-run continuation, escape counts, rollback, and
stereo interleave with distinct channel medians. The existing FFmpeg
synthetic mono block now traverses exactly all 882 residual words. This is
residual-stream traversal only: no real stereo stream, decorrelation,
reconstructed PCM, audio CRC or PCM parity is asserted, and WavPack remains
unregistered.

**Observed checks:** `cargo test --locked --offline -p rust-music-wavpack`
passed 27/27. The full `sh scripts/offline-gate.sh` exited 0 across 16 local
crates and 29 path-only edges. Roadmap and literature validators and
`git diff --check` passed. Read-only review confirmed the stereo/interleave
and multi-frame rollback findings were closed, while confirming this remains
residual-level evidence only.

## 2026-10-29 — RM-014 median-zone and residual primitives

Added `MedianChannel`, which selects the recursive Golomb interval from an
already-resolved unary partition count and adapts all three Q4 median
breakpoints with checked arithmetic. Added an atomic signed-residual decoder
using the adjusted-binary reader; on truncation it restores both bit and
median state. Deterministic tests cover all first four recursive zones,
positive zero/negative-one sign mapping, truncation rollback and update
overflow. This is entropy primitive evidence only; unary pair translation,
zero runs, full real-stream decode, decorrelation, block CRC and PCM parity
remain open. The behavior follows the published WavPack entropy description
already indexed as LIT-020; no reference implementation code was copied.

**Observed checks:** `cargo test --locked --offline -p rust-music-wavpack`
passed 22/22. `sh scripts/offline-gate.sh` exited 0 across 16 local crates and
29 path-only edges, including formatting, warnings-denied Clippy, workspace
tests, release build and macOS feature check. Both roadmap and literature
validators and `git diff --check` passed. A read-only safety review found no
actionable issue in the reviewed primitives and explicitly did not treat them
as full word-stream or PCM-parity evidence.

## 2026-10-29 — RM-014 adjusted-binary entropy primitive

Implemented a bounded WavPack adjusted-binary reader on top of the existing
LSB-first bit reader. It reads the truncated-binary prefix and optional suffix,
rejects an empty range, handles the full `u32` range without shift overflow,
and restores the cursor if a codeword is truncated. Test vectors cover every
codeword in the official WavPack Golomb table for ranges 6–11, plus the
`u32::MAX` boundary and atomic truncated-read behavior. The decoder remains
unregistered and unsupported for playback: median-zone selection/adaptation,
residual word sequencing, sample reconstruction, CRC, and independent PCM
parity are still missing.

**Observed checks:** the full `sh scripts/offline-gate.sh` exited 0 on the
final change across 16 local crates and 29 path-only edges, including
formatting, warnings-denied Clippy, workspace tests, release build and macOS
feature check. The WavPack suite passed 19/19, including the explicit
`u32::MAX` assertion. `python3 scripts/check_roadmap_index.py`,
`python3 scripts/check_literature.py`, and `git diff --check` passed. A
read-only safety review found no actionable correctness issue in the
primitive; it does not validate WavPack word decoding or PCM parity.

## 2026-10-29 — Archive.org AAC ingestion-denial sample

Added `ia-stificash-starstedede-aac.m4a` from Archive.org item
[`starstedede`](https://archive.org/details/starstedede), credited to
Stificash. The item labels the upload CC0 1.0; that uploader-supplied rights
claim is recorded as provenance, not independently audited. The checked-in
file is 3.5 MB, SHA-256
`b3735fba8acf607bf51438fa43ad4c314da46c07d75482eae99629bc3d422857`, and
`ffprobe` identifies AAC, 44.1 kHz stereo. It is only a real-world container
ingestion and lossy-codec denial fixture; it is not playback or ALAC parity
evidence.

Extended `scripts/test_m4a_corpus.py` to verify this sample's hash and actual
AAC codec, catalog it alongside the six existing Archive.org ALAC samples and
the generated AAC sample, preserve the `.mp4a` alias check, and require AAC
inspection to fail closed for both AAC files. The script passed all six exact
ALAC PCM comparisons, scanner checks and both AAC denials. Roadmap and
literature validators and `git diff --check` passed. The new corpus fixture
adds a second Archive.org item and a different uploader, but does not close
the independent lossless encoder/ALAC source gap; it retains the stated
metadata license without claiming a separate rights audit.

## 2026-10-29 — offline release gate and private FLAC parity

**Branch hygiene:** work stayed on the existing `build/native-codecs-g0`
branch. It was the only local branch; existing remote refs were left intact.
No branch was created or deleted, and no push or merge was performed.

**Verified work:**

- `sh scripts/offline-gate.sh` exited 0 on Rust/Cargo 1.99.1/macOS. The
  dependency audit, formatting, Clippy with warnings denied, workspace tests,
  release build and macOS feature check passed.
- `cargo test --locked --offline -p rust-music-host` exited 0 (5/5 tests),
  including sink-open failure cleanup and host focus lease release.
- `CARGO_NET_OFFLINE=true python3 scripts/test_flac_user_corpus.py --mode all --require-complete <six supplied FLAC file paths>` exited 0. All six tracks passed reference FLAC integrity, FLAC/FFmpeg PCM equality, Rust inspection, PCM SHA-256 and byte-count parity.
- `python3 scripts/check_roadmap_index.py` and
  `python3 scripts/check_literature.py` passed; `git diff --check` was clean.

The six source recordings remained at their supplied locations. The script
used temporary decoded WAV files and removed them on exit; no audio, artwork,
or absolute source paths were added to Git.

**Pip-Boy universal dock clarification:** documented the intended Radio-to-Music
foreground handoff in `PIP_BOY_ADAPTER.md`. Music keeps a distinct package ID;
selecting it suspends Radio and transfers the shared foreground slot. This is
not destructive module replacement or dynamic third-party installation: the
current host only bootstraps trusted built-ins and still lacks Music controls
and the shared audio-focus bridge. After a later, explicit user authorization,
the separate Pip-Boy PR worktree received one scoped lifecycle regression and
documentation commit (`dcc7f5c`); it was not pushed, merged, or treated as Music
integration evidence. No additional sibling-repository changes are in scope.

**Rust Music revalidation:** on working tree `314913d` plus the documentation
delta above, `sh scripts/offline-gate.sh` passed again, including locked offline
workspace build/tests, release build, Clippy, formatting and macOS feature
check. `python3 scripts/check_roadmap_index.py`,
`python3 scripts/check_literature.py`, and `git diff --check` also passed.
This revalidates the local Rust Music release floor; it does not clear the
host-integration or physical-hardware gates.

**Remaining acceptance:** `RM-005` needs physical DAC evidence. `RM-006` and
`RM-007` need host-side shared Music/Radio/TV focus, package controls and panel
acceptance; the Pip-Boy worktree remains read-only under the current scope.
`RM-008` is the next planned P1 implementation task. No PR state transition
or push was performed while producing this log.

## 2026-10-29 — distinct module boundary and ALAC packet scaffold

**Module identity:** clarified that Pip-Boy is the container/dock and each
package retains its own identity, metadata, state, controls, and lifecycle.
Foreground switching must not merge Music with, or overwrite, Radio. This is a
Rust Music integration contract; it does not assert that Pip-Boy has a dynamic
third-party package loader or that host integration has passed.

**RM-008 source work:** added a workspace-local `rust-music-alac` crate with a
bounded parser for the bare 24-byte ALAC cookie and raw packet element headers.
It handles uncompressed mono/stereo packet PCM and rejects compressed frames,
shifted-byte escape frames, unsupported channel layouts, malformed headers,
truncation, and configured size/frame bounds. Five deterministic crate tests
pass. This is a decoder scaffold only: RM-008 remains planned; predictor and
entropy decoding, independent 16/24/32-bit compressed PCM parity, and malformed
compressed-frame coverage are still required. ALAC remains unadmitted, and no
M4A/CAF demux or playback support is claimed.

**Observed local checks:** `cargo test --locked --offline -p rust-music-alac`
passed 5/5; `sh scripts/offline-gate.sh` exited 0 across 13 local crates and
22 path-only internal edges, including Clippy, workspace tests, release build,
and the macOS feature check. `python3 scripts/check_roadmap_index.py`,
`python3 scripts/check_literature.py`, and `git diff --check` passed. These
checks validate the bounded scaffold and workspace integration only; they do
not satisfy RM-008 PCM-parity acceptance.

**Compressed-packet experiment:** a locally generated synthetic 16-bit stereo
ALAC packet and source PCM were used to probe the predictor/Rice path. The
first independent decoder attempt did not reproduce source PCM, so that
unverified implementation and its experimental fixture were removed. The
compressed-frame rejection remains in place, RM-008 remains planned, and no
ALAC playback or parity claim is made. The next ALAC pass must establish
channel-stream alignment and exact PCM parity before retaining compressed
decoding code.

**Host test reliability:** the first full-gate rerun exposed a collision in
`rust-music-host`'s timestamp-only temporary directory names under parallel
tests (`AlreadyExists`). Added an atomic per-process sequence and collision
retry to the test fixture helper. The focused host suite then passed 5/5, and
the subsequent `sh scripts/offline-gate.sh` exited 0 across all 13 crates,
including Clippy, workspace tests, release build, and the macOS feature check.

## 2026-10-29 — RM-008 compressed ALAC parity progress

**Decoder and fixtures:** corrected the adaptive unary reader to consume the
ALAC one-prefix polarity, then implemented bounded Rice residual decoding,
predictor reconstruction, stereo matrixing, shifted low-byte restoration, and
required compressed-packet end-element validation. Added locally generated
stereo fixtures for 16-bit and shifted 24-bit samples. `cargo test -p
rust-music-alac --offline` passes 9/9, including exact integer PCM equality
against known synthetic source values and rejection of every truncated prefix
of the 16-bit compressed packet. A 32-bit signed-range escape packet also
passes; this does not establish compressed 32-bit parity.

FFmpeg was used only as a local test-vector encoder and PCM cross-check, never
as a crate, runtime dependency, or decoder fallback. The available encoder
emitted 24-bit ALAC even for a 32-bit PCM input (`bits_per_raw_sample=24`), so
compressed 32-bit independent parity remains unverified. RM-008 remains
planned and ALAC remains unadmitted; more malformed bitstream coverage and
M4A/CAF demux are still open.

## 2026-10-29 — compressed ALAC 32-bit parity

Added a synthetic 32-bit stereo compressed packet fixture derived from the
generated 24-bit vector using ALAC shifted-byte representation. FFmpeg's
independent ALAC decoder identified the CAF stream as 32-bit ALAC and emitted
128 samples exactly matching the deterministic 32-bit PCM values. The Rust
decoder now asserts those same 128 samples in
`compressed_stereo_32_bit_shifted_packet_matches_reference_pcm`; the focused
suite passes 13/13 after the additional matrix and malformed-packet cases.
The 32-bit packet was built by moving each generated 24-bit sample's low byte
into a two-byte shifted field and appending a zero low byte; the reference PCM
is the exact original sample value multiplied by 256. FFmpeg's independent
ALAC decoder emitted the same complete CAF PCM stream. FFmpeg is test-vector
generation/reference tooling only, not a build or runtime dependency.

Also added a 256-frame synthetic stereo packet with nonzero matrix parameters
(`mixBits=31`, `mixRes=1`). FFmpeg decoded all 512 samples exactly against the
deterministic source pattern, and the Rust matrix reconstruction matches it.
The 32-bit fixture now rejects every truncated byte prefix. An adversarial
matrix header with a 255-bit shift is rejected before reconstruction, keeping
the signed shift in the sample-width domain. A single-bit-flip sweep across
the full matrix packet also confirms no panic or frame-bound violation. The
ALAC raw-decoder task now meets its packet-level parity and malformed-input
acceptance; M4A/CAF demux remains separate and ALAC is still unadmitted.

## 2026-10-29 — module identity cross-repo verification

**Pip-Boy boundary:** reviewed the adapter instructions on draft PR #71 and
the host's universal-adapter contract. Modules keep distinct package IDs,
manifests, adapter instances, and lifecycle while sharing one foreground
container. The host regression
`distinct_music_module_can_replace_radio_and_pop_out_without_uninstalling_radio`
passed locally (1/1): Music can take the foreground and pop out while Radio
remains mounted. Pip-Boy already enforces the requested distinction, so no
additional host source change was needed. Existing unrelated Pip-Boy worktree
changes were left untouched.

**Rust Music gate:** the first post-change offline gate found Clippy's
`manual_div_ceil` warning in ALAC residual mapping. Replaced the handwritten
ceiling division with `div_ceil`; the subsequent `sh scripts/offline-gate.sh`
exited 0 across 13 local crates and 22 path-only edges, including formatting,
warnings-denied Clippy, workspace tests, release build, and macOS feature
check. Roadmap and literature validators and `git diff --check` passed. ALAC
remains unadmitted pending compressed 32-bit parity, broader malformed-input
coverage, and container demux; no hardware or final production acceptance is
claimed.

## 2026-10-29 — RM-009 bounded M4A/MP4 demux

Added the independent `rust-music-mp4` crate and wired content-sniffed `ftyp`
input through the public Rust Music facade. The parser uses checked ISO BMFF
box and sample-table bounds, recognizes the ALAC sample description and cookie,
and reads packet extents from the local `mdat`. Unsupported layouts, AAC,
encrypted entries/DRM indicators, external data references, invalid offsets,
truncated input, and every file-prefix truncation fail closed. The feature is
limited to a generated 24-bit M4A parity vector; raw ALAC packet parity remains
separately verified at 16/24/32-bit and stereo-matrix settings. No runtime
third-party dependency was added. M4A/ALAC is not broadly production-admitted;
CAF demux and wider real-world M4A coverage remain open.

Validation on the local Rust/Cargo 1.99.1 macOS environment:

- `sh scripts/offline-gate.sh`: **exit 0**; 14 local crates, 25 path-only
  edges, dependency audit, fmt, warnings-denied Clippy, workspace tests,
  optimized build, and macOS output-feature check passed.
- The MP4 crate's five tests passed, including exact generated PCM parity,
  AAC/wrong-codec/encrypted/external-reference denial, bad sample bounds, and
  every truncated file prefix. The facade's three tests passed, including
  content-based M4A ALAC admission and AAC rejection despite a `.flac` path.
- `python3 scripts/check_roadmap_index.py`,
  `python3 scripts/check_literature.py`, and `git diff --check`: **pass**.

The first gate attempt exposed a Clippy `let_unit_value` warning and a
concurrent facade test temp-directory collision. Both were fixed; the temp
directories now use process-local atomic IDs and retry if already present.
RM-009 is complete for its bounded acceptance. RM-010 CAF parsing, production
format corpus checks, host integration, distinct-module registration/input,
and physical audio acceptance are still outstanding.

## 2026-10-29 — RM-010 CAF ALAC/LPCM demux and module catalog integration

Added the local `rust-music-caf` decoder crate and wired CAF signature sniffing
through the facade, bounded local library catalog, and 1 KiB panel's format
label. The decoder accepts signed mono/stereo LPCM at 8/16/24/32 bits (the
8-bit path is decoder-only under the playback floor), both sample byte orders,
packed 24-bit and CAF's high-aligned 24-bit-in-32 storage. It accepts ALAC with
the bare 24-byte cookie or CAF's validated `frma`/`alac` atom wrapper and
bounded `pakt` packet-size descriptions. Unknown-sized `data` is accepted only
as the terminal chunk. Unsupported codecs, float PCM, explicit custom channel
maps, mismatched layouts, multichannel LPCM and malformed bounds fail closed;
no AudioToolbox/OS decoder fallback exists.

The accepted channel-map surface is intentionally limited to mono and standard
stereo/headphone tags because Rust Music's PCM/host contract does not yet carry
multichannel speaker mapping. A read-only `safety_reviewer` found the initial
implementation skipped CAF `chan` maps; the parser now verifies standard
mono/stereo tags and rejects unsupported maps and multichannel PCM. A second
read-only pass confirmed the fix, including three-channel LPCM with no layout.

Independent synthetic CAFs were generated locally with FFmpeg from deterministic
PCM; the optional oracle/encoder script then compared Rust CLI output bytes to
the original PCM. FFmpeg is test tooling only and is not a runtime dependency.

- `cargo test --locked --offline -p rust-music-caf -p rust-music-library -p rust-music-panel -p rust-music`: **exit 0**; CAF crate 9/9, library 3/3, panel 5/5, facade 4/4.
- `python3 scripts/test_caf_reference.py`: **exit 0**, 5/5 exact PCM vectors: LPCM 16/24/32-bit and ALAC 16/24-bit.
- `sh scripts/offline-gate.sh`: **exit 0**; 15 local crates, 28 path-only edges, zero registry/Git crates, zero third-party system links, one OS framework link; fmt, warnings-denied Clippy, workspace tests, release build and macOS feature check passed.
- `python3 scripts/check_roadmap_index.py`, `python3 scripts/check_literature.py`, `cargo fmt --all -- --check`, and `git diff --check`: **pass**.

The crate tests also cover chunk/packet limits, exact sample-rate parsing,
standard layout validation, wrong codecs, float refusal, malformed negative
chunk sizes, all truncated prefixes, and terminal unknown-sized `data`. CAF
generated parity is not real-world corpus, host-focus or hardware acceptance;
those remain open alongside broad M4A/ALAC corpus evidence and Rust_Boy/Pip-Boy
audio-focus/input integration. The sibling repo was not modified.

## 2026-10-29 — distinct Music package identity and suspend lifecycle

Added the stable `org.rustboy.music` identity to the standalone host crate,
explicitly distinct from `org.rustboy.radio`. `MusicSlot::suspend()` now stops
output and releases the host lease while retaining the mounted slot, catalog,
and selected track. Returning to the foreground does not autoplay; playback
requires a fresh operator command. The API provides a safe Rust-side lifecycle
boundary but does not register a package in Pip-Boy or implement Pip-Boy's
shared Radio/TV/Music focus arbiter.

- `cargo test --locked --offline -p rust-music-host`: **exit 0**, 6/6 tests,
  including distinct IDs, pop-out lease release, retained selection/catalog,
  and no autoplay on pop-in.
- `sh scripts/offline-gate.sh`: **exit 0**; 15 local crates, 28 path-only
  edges, zero registry/Git crates, fmt, warnings-denied Clippy, workspace
  tests, release build, and macOS feature check passed.
- `python3 scripts/check_roadmap_index.py`, `cargo fmt --all`, and
  `git diff --check`: **pass**.

Pip-Boy adapter registration, user-authorized playback intents, shared focus,
worker scheduling, actual display wiring, and hardware audio remain unverified.

## 2026-10-29 — RM-017 M4A catalog ingestion and Internet Archive ALAC corpus

Added `.m4a` and `.mp4` extension plus `ftyp` signature recognition to the
bounded library scanner, and an `M4A` label to the fixed panel. Container
cataloging is intentionally separate from codec admission: the facade still
opens only its supported ALAC profile and refuses AAC. Added two small M4A
files from Internet Archive's `musopen-chopin` item; the item page records
CC0 1.0 and Musopen's Chopin campaign FAQ independently states CC0 for its
commissioned recordings. Source URL, file names, observed streams, and SHA-256
hashes are kept beside the fixtures. No unrelated or user-supplied recordings
were added.

- `python3 scripts/test_m4a_corpus.py`: **exit 0**, exact PCM parity on both
  Archive.org M4As: stereo 24-bit/48 kHz and stereo 16-bit/44.1 kHz. The
  harness verifies fixture hashes/codecs, exercises `.m4a` scanner ingestion,
  and confirms its AAC negative fixture fails decoder admission. FFmpeg and
  ffprobe are local test tools only.
- `sh scripts/offline-gate.sh`: **exit 0**; 15 workspace crates, 28 local
  dependency edges, zero registry/Git dependencies, formatting, warnings-denied
  Clippy, all workspace tests, optimized build and macOS feature check passed.
- `python3 scripts/check_roadmap_index.py`,
  `python3 scripts/check_literature.py`, `cargo fmt --all -- --check`, and
  `git diff --check`: **pass**.

This is two samples from one collection, not broad independent-encoder ALAC
coverage. CAF real-world corpus, Pip-Boy host registration/shared audio focus,
panel hardware, and physical DAC acceptance remain open.

## 2026-10-29 — RM-017 Archive.org corpus expansion

Expanded the checked-in M4A corpus from two to four CC0 tracks by adding two
small ALAC files from the same Internet Archive `musopen-chopin` collection.
The provenance ledger records the upstream names, observed codec profiles,
and hashes. The corpus now exercises one 24-bit/48 kHz stream and three
16-bit/44.1 kHz streams; all four remain from one source/encoder collection,
so this adds real-world sample/table coverage without claiming independent
encoder compatibility.

- `python3 scripts/test_m4a_corpus.py`: **exit 0**, 4/4 exact PCM parity cases;
  M4A ingestion and AAC decode refusal pass.
- `python3 scripts/check_roadmap_index.py`,
  `python3 scripts/check_literature.py`, and `git diff --check`: **pass**.

## 2026-10-29 — RM-017 `.mp4a` ingestion coverage

The six-file CC0 Archive.org ALAC corpus was already present, so no duplicate
audio blobs were added. Expanded the bounded scanner's MP4-family extension
allowlist to include `.mp4a` and added a real-corpus rename/copy case to the
corpus harness. The test checks that `.mp4a` is cataloged from its `ftyp`
signature; AAC remains catalogable but rejected by the lossless decoder.

- `python3 scripts/test_m4a_corpus.py`: **exit 0**; all six existing Archive.org
  files passed exact PCM parity, and the harness catalogued the `.mp4a` alias
  together with all `.m4a` samples and the AAC negative fixture before
  confirming AAC decode rejection.
- `cargo test --locked --offline -p rust-music-library`: **exit 0**, 6/6 unit
  tests.
- `sh scripts/offline-gate.sh`: **exit 0**; 16 workspace crates, 29 path-only
  edges, format checks, warnings-denied Clippy, workspace tests, release build,
  and macOS feature check passed.
- `python3 scripts/check_roadmap_index.py`,
  `python3 scripts/check_literature.py`, and `git diff --check`: **pass**.
- Additional Archive.org downloads were not possible in this environment:
  `curl https://archive.org/metadata/musopen-chopin` failed DNS resolution.

## 2026-10-29 — RM-011 bounded local metadata

The explicit library scan now extracts only bounded, valid UTF-8 display tags:
FLAC Vorbis `TITLE`, `ARTIST`, `ALBUM` and AIFF `NAME`, `AUTH`. It keeps the
filename title as fallback, strips control characters, caps retained values,
and does not decode or retain embedded artwork. The 1KiB panel uses the
sanitized display title. Signature sniffing and metadata extraction share one
open file handle, addressing the safety review's reopen-race observation.

- `sh scripts/offline-gate.sh`: **exit 0**; 15 local crates, 28 local edges,
  fmt, warnings-denied Clippy, workspace tests (library 6/6, panel 5/5,
  host 6/6), release build and macOS feature check passed.
- Read-only `safety_reviewer`: no blocking findings; its one low-priority race
  observation was addressed.
- `python3 scripts/test_m4a_corpus.py`: **exit 0**, 4/4 exact PCM ALAC cases;
  M4A ingestion and AAC refusal pass.
- `python3 scripts/check_roadmap_index.py`,
  `python3 scripts/check_literature.py`, and `git diff --check`: **pass**.

Archive.org's item format label was not treated as codec evidence during the
independent-source search: two separately CC0-marked candidates labeled
"Apple Lossless Audio" were actually AAC/mp4a by `ffprobe`; Rust Music refused
the AAC at its ALAC sample-entry check. Neither is included as ALAC evidence
or retained as a corpus fixture.

## 2026-10-29 — RM-012 validated FLAC seek

Added bounded FLAC SEEKTABLE parsing and sample-frame seeking for seekable
inputs with declared STREAMINFO totals. Seek initializes at the closest
preceding valid point, decodes through the target, and trims the target block
sample-accurately. The host exposes `Command::SeekFrame`; success drains output,
releases the shared focus lease, and requires explicit Play. Unsupported and
out-of-range requests do not interrupt current playback. Whole-stream PCM MD5
is intentionally unavailable after a partial seek.

- `sh scripts/offline-gate.sh`: **exit 0**; 15 local crates, 28 path-only
  edges, fmt, warnings-denied Clippy, workspace tests, release build, and macOS
  feature check passed.
- `python3 scripts/test_m4a_corpus.py`: **exit 0**, 4/4 exact PCM ALAC cases;
  ingestion and AAC refusal passed.
- `python3 scripts/check_roadmap_index.py`,
  `python3 scripts/check_literature.py`, and `git diff --check`: **pass**.
- Synthetic fixed- and variable-block FLAC tests cover exact frame boundaries,
  mid-frame trimming, EOF, invalid targets, and invalid seektable offsets.
  Real-world encoder seektable compatibility and host input routing remain open.

## 2026-10-29 — RM-013 bounded RF64/ds64 support

`rust-music-wave` now accepts RF64 integer PCM with a required first `ds64`
chunk, bounded 1024-entry size table, checked 64-bit container/data/sample
counts, and sentinel chunk-size resolution. Metadata remains capped at 32 MiB;
PCM data is streamed in existing bounded blocks. Legacy RIFF behavior remains
unchanged. A sparse synthetic stream verifies correct frame accounting and
PCM block output for a 4 GiB data size without allocating that file.

- `cargo test --locked --offline -p rust-music-wave`: **exit 0**, 10/10 tests.
- `python3 scripts/test_wave_reference.py`: **exit 0**, 6/6 exact native RIFF
  PCM vectors and malformed-input rejection.
- `sh scripts/offline-gate.sh`: **exit 0**; 15 crates, 28 local edges,
  fmt, warnings-denied Clippy, workspace tests, release build, macOS feature
  check.
- `python3 scripts/test_m4a_corpus.py`: **exit 0**, 4/4 exact PCM cases.
- Read-only `safety_reviewer`: no blocking RF64 bounds or RIFF regression
  findings. Roadmap/literature validators and `git diff --check` pass.
- Real large-file corpus and physical playback remain unverified.

## 2026-10-29 — RM-014 WavPack framing foundation

Added a locally authored WavPack crate with bounded 32-byte v4/v5 block header
parsing, a 1 MiB block cap, a 131,072-frame block cap, safe metadata sub-block
length parsing, and strict refusal for hybrid-without-correction, float, DSD,
and multichannel profiles. The crate is deliberately not wired into the player:
entropy decoding, sample reconstruction/CRC checks, independent PCM parity,
and validated `.wvc` pairing are still missing. RM-014 remains planned.

- `cargo test --offline -p rust-music-wavpack`: **exit 0**, 5/5 framing tests.
- Official WavPack 4/5 block documentation is indexed as LIT-020. The project's
  no-reference-code-reuse rule remains in force.
- Read-only `safety_reviewer` found that the optional metadata flag was included
  in the parsed metadata ID. The parser now masks the ID to five bits; the
  corrected test and full workspace gate pass. No other framing/bounds finding.
- `sh scripts/offline-gate.sh`: **exit 0**; 16 local crates, 29 path-only
  edges, fmt, warnings-denied Clippy, workspace tests, release build and macOS
  feature check passed. `check_roadmap_index.py`, `check_literature.py`, and
  `git diff --check` pass.

## 2026-10-29 — RM-014 version-aware WavPack framing

Corrected the bounded WavPack header parser to treat reserved upper bytes as
unused before stream version `0x410`, use 40-bit fields from `0x410` onward,
and expose `total_samples` only on a block whose sample index is zero. Added
regression cases for the legacy `0x40f` and new-stream `0x410` boundary,
upper-byte values, and later blocks. The `0x410` threshold is confirmed by the
official porting guide: `0x407` is the legacy-compatible stream version.
This is parser correctness evidence only; WavPack remains unregistered because
entropy decoding, reconstruction, CRC parity, and hybrid correction pairing
remain absent.

- `cargo test --locked --offline -p rust-music-wavpack`: **exit 0**, 7/7 unit
  tests.
- `sh scripts/offline-gate.sh`: **exit 0**; 16 workspace crates, 29 local
  dependency edges, warnings-denied Clippy, workspace tests, release build, and
  macOS feature check passed.
- `git diff --check`: **pass**.
- Official references: [WavPack 4 & 5 Binary File / Block Format](https://www.wavpack.com/WavPack5FileFormat.pdf) and [WavPack 5 Porting Guide](https://www.wavpack.com/WavPack5PortingGuide.pdf).

The first implementation mistakenly treated `0x407` as the new-stream
threshold. Review of the official porting guide clarified that `0x407` is the
legacy-compatible stream and `0x410` is the WavPack 5 stream; the parser and
boundary tests were corrected before any WavPack decoder admission.

- `cargo test --locked --offline -p rust-music-wavpack`: **exit 0**, 8/8 unit
  tests, including both `0x407` and `0x40f` legacy cases plus `0x410` 40-bit
  fields.
- `sh scripts/offline-gate.sh`: **exit 0**; 16 crates, 29 path-only edges,
  warnings-denied Clippy, workspace tests, release build, and macOS feature
  check passed.
- The official WavPack 5 Porting Guide is now indexed as LIT-021.

## 2026-10-29 — G5 catalog replacement lifecycle regression

Added a host-slot regression for replacing a catalog while Music holds the
mock shared-audio lease. It verifies that the old stream stops and closes, the
lease is released, a still-present relative selection follows its track to a
new index, and replacement leaves playback stopped. This covers Music's local
module lifecycle boundary; it does not establish Pip-Boy/Radio/TV arbitration.

- `cargo test --locked --offline -p rust-music-host`: **exit 0**, 9/9 unit
  tests.
- `sh scripts/offline-gate.sh`: **exit 0**; 16 workspace crates, 29 local
  dependency edges, formatting, warnings-denied Clippy, workspace tests,
  release build, and macOS feature check passed.
- `git diff --check`: **pass**.

## 2026-10-29 — G5 output-write failure releases focus

Added an injected sink failure after Play has acquired the host lease. The
regression verifies that a failed worker `Poll` marks playback failed, calls
the sink's finish path, closes it, and drops the Music lease. This covers a
runtime device-write error in the local slot contract, not real device behavior.

- `cargo test --locked --offline -p rust-music-host`: **exit 0**, 10/10 unit
  tests.
- `sh scripts/offline-gate.sh`: **exit 0**; 16 workspace crates, 29 local
  dependency edges, formatting, warnings-denied Clippy, workspace tests,
  release build, and macOS feature check passed.
- `git diff --check`: **pass**.

## 2026-10-29 — RM-014 real-stream framing coverage

Added a 626-byte synthetic WavPack stream produced by the local FFmpeg encoder
and verified its codec, mono channel count, 44.1 kHz rate, 16-bit sample depth,
and 882-frame duration with `ffprobe`. The Rust framing test parses its complete
audio block and consumes every metadata sub-block. This exercises a real
encoder-produced stream while leaving all entropy/sample decoding unimplemented.
Fixture generation, SHA-256, and scope limits are recorded alongside the file.

- `cargo test --locked --offline -p rust-music-wavpack`: **exit 0**, 8/8 unit
  tests.
- `shasum -a 256` matched the recorded fixture digest; `ffprobe` confirmed the
  expected WavPack/mono/44.1 kHz/16-bit/882-frame stream properties.
- `sh scripts/offline-gate.sh`: **exit 0**; 16 workspace crates, 29 local
  dependency edges, formatting, warnings-denied Clippy, workspace tests,
  release build, and macOS feature check passed.
- `git diff --check`: **pass**.

## 2026-10-29 — RM-014 zero-copy block metadata iteration

Added a borrowed iterator that validates one complete WavPack block and walks
its metadata sub-blocks without allocating or copying their payloads. A
malformed entry yields one error and terminates iteration; a block-size
mismatch, including trailing bytes, is rejected up front. The generated
FFmpeg stream test now asserts the observed core metadata sequence.

- `cargo test --locked --offline -p rust-music-wavpack`: **exit 0**, 9/9 unit
  tests.
- `sh scripts/offline-gate.sh`: **exit 0**; 16 workspace crates, 29 path-only
  dependency edges, formatting, warnings-denied Clippy, workspace tests,
  release build, and macOS feature check passed.
- `git diff --check`: **pass**.

## 2026-10-29 — RM-014 contiguous stream validation

Added a stateful validator for the simple mono/stereo WavPack profile. It
skips metadata-only blocks, requires contiguous audio frame indexes, rejects
stream-wide PCM profile changes, and verifies the first block's optional
declared total at EOF. Adversarial tests cover gaps, overlaps, format changes,
empty streams, and incomplete declared totals. This is still framing and
sequence validation only: there is no entropy decode, sample reconstruction,
block CRC check, facade registration, or WavPack playback admission.

- `cargo test --locked --offline -p rust-music-wavpack`: **exit 0**, 12/12
  unit tests.
- `sh scripts/offline-gate.sh`: **exit 0**; 16 workspace crates, 29 local
  dependency edges, formatter, warnings-denied Clippy, workspace tests,
  release build, and macOS feature check passed.
- `python3 scripts/check_roadmap_index.py`: **exit 0**.
- `python3 scripts/check_literature.py`: **exit 0**.
- `git diff --check`: **pass**.

## 2026-10-29 — RM-014 bounded entropy bit reader

Added a bounded least-significant-bit-first reader for the compressed WavPack
metadata stream, including capped unary reads. Multi-bit truncation and
over-width requests fail without advancing the cursor. Tests cover bit order,
32-bit reads, exhausted input, atomic failures, terminated unary values, and
configured unary limits. The behavior follows the official published WavPack
algorithm description and is an original, small Rust implementation; no
reference decoder code was copied. This still does not implement entropy-word
decoding or expose WavPack to the playback facade.

- `cargo test --locked --offline -p rust-music-wavpack`: **exit 0**, 15/15
  unit tests.
- `sh scripts/offline-gate.sh`: **exit 0**; 16 workspace crates, 29 local
  dependency edges, formatter, warnings-denied Clippy, workspace tests,
  release build, and macOS feature check passed.
- `python3 scripts/check_roadmap_index.py`: **exit 0**.
- `python3 scripts/check_literature.py`: **exit 0**.
- `git diff --check`: **pass**.
- Algorithm reference: [WavPack compression algorithm, section 7.11](https://www.wavpack.com/WavPack.pdf).

## 2026-10-29 — RM-014 entropy-variable metadata

Added exact-length parsing for the mono/stereo entropy-variable metadata and
expanded its logarithmic median values with deterministic Q32 integer math.
The parser rejects unsupported channel counts, malformed payload sizes, and
encoded medians outside the 32-bit simple integer profile. The FFmpeg-generated
fixture now checks all three real mono median values. These initialized values
are not yet connected to residual decoding.

- `cargo test --locked --offline -p rust-music-wavpack`: **exit 0**, 17/17
  unit tests.
- `sh scripts/offline-gate.sh`: **exit 0**; 16 workspace crates, 29 local
  dependency edges, formatter, warnings-denied Clippy, workspace tests,
  release build, and macOS feature check passed.
- `python3 scripts/check_roadmap_index.py`: **exit 0**.
- `python3 scripts/check_literature.py`: **exit 0**.
- `git diff --check`: **pass**.

The Archive.org CC0 ALAC search surfaced two additional items reporting
“Apple Lossless Audio.” Actual `ffprobe` inspection of one file from each item
found AAC, so neither is retained as an ALAC corpus fixture:

- [La Primavera De Chile: Audio](https://archive.org/details/klv20200302laprimaveradechileaudio02032020), `AUDIO-2020-01-04-22-30-41.m4a`, SHA-256 `b0833c83d6c1858cf1c60a00f26c6ae3ec5ff98d5562bd696f370b3c6e4337ed`.
- [La Primavera de Chile. Unidos por la paz de Chile](https://archive.org/details/klvArc.Cl.O2019.upc_202001), `AUDIO-2020-01-11-08-52-40.m4a`, SHA-256 `fd5deaa2e5ef1dd5f2cf11da8bde9fb19031ca92f28c1acdd59ac424bb037920`.

## 2026-10-29 — Archive.org M4A corpus expansion

Added two more M4A ALAC tracks from the existing Musopen Chopin Archive.org
item. The CC0 license is recorded in item metadata and supported by Musopen's
project FAQ; the exact downloaded filenames, stream details, and SHA-256 values
are in `crates/rust-music-mp4/tests/fixtures/upstream/README.md`. The six-file
corpus still comes from one collection and does not expand independent
encoder/source coverage.

- `python3 scripts/test_m4a_corpus.py`: **exit 0**, all six files passed
  `ffprobe` codec/profile checks and exact Rust/FFmpeg PCM equality; the scanner
  indexed the M4A corpus and AAC negative fixture, and AAC decode was rejected.
- Downloaded fixtures are public CC0 source files, not private audio. The test
  removes decoded temporary WAV files on exit.
- The M4A/MP4 ingestion implementation was already present; this change extends
  its corpus regression coverage rather than changing container recognition.

## 2026-10-29 — Archive.org corpus source recheck

Rechecked Archive.org's public item search for CC0 audio tagged as Apple
Lossless or M4A. The existing six-file Musopen CC0 ALAC corpus remains the only
verified ALAC source in this search; the two additional items tagged Apple
Lossless were previously probed and found to contain AAC. No files were added
from an unverified label. Expanding beyond this collection requires finding
and verifying an independent M4A/ALAC source, rather than counting AAC files as
ALAC decoder coverage.

## 2026-10-29 — Archive.org album M4A ingestion samples

The user requested a small set of M4A album files and a list of the corpus
types still needed. The ALAC-positive gap remains an independent encoder/source
plus real M4A samples at supported 20/32-bit and mono profiles. Archive.org's
Netlabels format search yielded two separately licensed music albums suitable
for real M4A/AAC ingestion denial coverage. One short track from each was
downloaded and verified; neither is counted as lossless decoder evidence.

- `LTTS001A` / *Winter's Child* / Lost to the Sea, CC BY 4.0 uploader metadata:
  `03. Amazing Grace.m4a`, 1,469,859 bytes, AAC 44.1 kHz stereo,
  SHA-256 `ca33660a87bf7d02676562851a5806f2bbdd6c5022fa8b17dadd4715e9835243`.
- `christian-thbe-hegdgehgog-boys-vapor-wave-tape-rip`, sonichuwave クィック,
  CC BY-SA 4.0 uploader metadata: track 02, 3,287,206 bytes, AAC 44.1 kHz
  stereo, SHA-256
  `2406cdcb798c50b7cdf4b23a2f94986c82f373785957d04974972a30570987ca`.
- The source item/license/track attribution, hashes, and remaining corpus needs
  are recorded in `crates/rust-music-mp4/tests/fixtures/upstream/README.md`.
  The script now verifies both hashes, codec/profile, catalog ingestion, and
  fail-closed AAC refusal.
- `python3 scripts/test_m4a_corpus.py`: **exit 0**; six ALAC files remained
  byte-exact against FFmpeg, `.mp4a` alias ingestion passed, and all four AAC
  fixtures (three Archive.org plus one generated) were rejected.
- Archive.org's format label is not treated as codec evidence. A separately
  inspected voice-message candidate labeled Apple Lossless was AAC and was
  removed rather than retained as music or ALAC evidence.

## 2026-10-29 — Pip-Boy MUSIC package identity

Following the user's authorization to work in both repositories, Pip-Boy now
has a distinct `org.rustboy.music` reference manifest in its universal package
catalog. The manifest requests `media.audio`, disables background operation,
and remains explicitly `design_fixture_not_installed`; it does not mount a
runner or imply playable integration. Radio remains a separate package.

- Pip-Boy commit `bccd59d` updates its reference catalog, registry test, and
  universal adapter contract. The existing uncommitted changes in Pip-Boy's
  `crates/pipboy/src/main.rs` and `terminal_commands.rs` were preserved.
- In the Pip-Boy checkout,
  `cargo test --locked --offline -p pipboy-core` (using a temporary Cargo target
  directory): **exit 0**, 281/281 tests. The first sandboxed run had one
  unrelated loopback-listener permission failure; rerunning with local
  loopback permission passed the suite.
- Pip-Boy still lacks a trusted Music runner, shared Radio/TV/Music lease
  bridge, reviewed user-input/worker routing, and host/device acceptance. The
  catalog manifest is registry evidence only, not the RM-006 acceptance.

## 2026-10-29 — Additional Archive.org M4A ingestion corpus

The user asked to enumerate needed corpus types and pull a small selection of
Archive.org album files into the repo. Added two unchanged, attributed M4A
tracks from separate Netlabels album items after checking item metadata and
probing the actual streams. Both are AAC and therefore exercise container
ingestion plus fail-closed codec refusal; neither is ALAC decoder evidence.
One is mono/48 kHz, the other stereo/44.1 kHz.

- `MARAV955A`, *Nothing Changes*, Utopia, `02 Nothing Changes.m4a`: AAC,
  mono/48 kHz, uploader metadata says CC BY-NC-ND 4.0; SHA-256
  `02de330aa3d135bd4faea0c47dadb1a0b28a8501fc486adde2db69da2c3743cc`.
- `foot292`, *Movimento*, Filipe Miranda,
  `foot292_03-filipe_miranda-movimento_iii.m4a`: AAC, stereo/44.1 kHz,
  uploader metadata says CC BY-NC-SA 4.0; SHA-256
  `ca20e55a053594f39d28ec7920434c861a64dac83ed84cc226d6658a4916f0b9`.
- Attribution, license URLs, stream details, and remaining corpus types are
  indexed in `crates/rust-music-mp4/tests/fixtures/upstream/README.md`; the
  optional corpus script checks the hashes, codec, rate/channels, scan
  ingestion, and decode refusal.
- `python3 scripts/test_m4a_corpus.py`: **exit 0**; six ALAC files remained
  byte-exact against FFmpeg, byte-identical `.mp4a`/`.mp4` filename aliases
  were catalogued by signature, and all six AAC files were rejected.
- `python3 scripts/check_roadmap_index.py`: **exit 0**, 15 gates / 17 tasks.
- `python3 scripts/check_literature.py`: **exit 0**, 21 references / 16
  standards. `git diff --check`: **pass**.

Remaining corpus needs: independent real-world ALAC source/encoder coverage;
M4A/ALAC at supported 20-bit and 32-bit profiles and mono; a real `.mp4`-named
audio input for signature aliasing; and independently sourced real-world CAF
LPCM/ALAC files. The checked-in mono AAC sample does not close mono ALAC.

## 2026-10-29 — RM-012 real-world FLAC seek parity

Added a bounded `decode-range` CLI action for seek-capable inputs. It requires a
declared frame count, rejects zero/out-of-range requests and WAV-size overflow,
and exports only the requested frames without resampling. Extended the private
FLAC corpus harness to compare each populated SEEKTABLE sample and its next
sample against independent FFmpeg PCM. The six user recordings stayed in their
provided directory; reference PCM, range WAVs, and seek reports stayed in
temporary storage.

- `cargo test --locked --offline -p rust-music-cli`: **exit 0**, 6/6 tests,
  including exact range bytes and rejection before output creation on bounds
  failure.
- `CARGO_NET_OFFLINE=true python3 scripts/test_flac_user_corpus.py --mode all
  --verify-seek --require-complete <six supplied FLAC paths>`: **exit 0**;
  all six complete streams passed FLAC integrity, FLAC/FFmpeg/Rust whole-PCM
  parity, and exact PCM parity on all 132 ranges at every populated SEEKTABLE
  point plus the following sample.
- This closes the six-file real-world seektable evidence gap only. Other FLAC
  encoders and host seek-input routing remain unverified; partial seeks do not
  validate full-stream PCM MD5.

## 2026-10-29 — RM-006 Pip-Boy audio-focus foundation

The user authorized edits in Pip-Boy so distinct packages can share the host
container. Added a host-wide, non-preemptive in-process `AudioFocusBroker` with
RAII leases for RADIO, MUSIC, and TV. Every live Radio output start and output
replacement now acquires the RADIO lease before constructing its output engine;
stop, terminal cleanup, construction failure, and drop release the lease.
MUSIC remains a separate reference package and is not yet wired to the broker.

- Pip-Boy `universal_adapter` synthetic lifecycle coverage now includes all five
  package IDs independently, including `org.rustboy.music`; the registry API
  still labels the fixture as not installed.
- Indexed the shared audio-focus source in Pip-Boy's component catalog and
  updated package/socket docs to distinguish registry fixture, Radio lease
  integration, and the remaining Music/TV/UI bridge.
- `cargo test --locked --offline -p pipboy-core` (temporary Cargo target):
  **exit 0**, 283/283 tests.
- `cargo check --locked --offline -p pipboy` (temporary Cargo target): **exit 0**.
- `cargo test --locked --offline -p pipboy` (temporary Cargo target): **exit 0**,
  77/77 tests. One earlier full-suite run had two
  failures; after updating the expected design-fixture count from four to five,
  the affected task test passed alone and the full suite passed.
- `cargo test --locked --offline --workspace`: **exit 0**; 77 pipboy tests,
  283 core tests, 2 adapter SDK tests, and 12 broker tests passed.
- `cargo clippy --locked --offline --workspace --all-targets -- -D warnings`:
  **exit 0**. `cargo fmt --all -- --check`: **exit 0**.
- Pip-Boy component index and package fixture/catalog validators passed (18
  components / 57 code entrypoints; five package fixtures).

RM-006 advances to partial evidence: MUSIC's trusted adapter, explicit user
intents, worker polling, TV integration, actual shared MUSIC/RADIO/TV contention,
live shell mounting, and host/device acceptance remain open. The broker is only
in-process and does not provide an OS sandbox or cross-process lease service.

## 2026-10-30 — Additional Archive.org M4A denial profiles

Enumerated the remaining lossless corpus gaps and added two small M4A tracks
from distinct Archive.org album items. Both are AAC after `ffprobe`, so they
extend container ingestion and fail-closed refusal coverage only. The corpus
still needs an independent ALAC source/encoder, real ALAC M4A at 20-bit,
32-bit, and mono profiles, real-world CAF LPCM/ALAC, and representative
multi-track/video-bearing MP4 layouts if those remain intended parser targets.

- `ia-zyphr-festival-speed-aac.m4a`: *When the festival has speed*, Zyphr;
  Archive.org item license is uploader-labeled CC BY-NC-SA 4.0; 1,825,592
  bytes; AAC, 48 kHz stereo; SHA-256
  `5ca7c557002eaf29c4df323411989c6fd6441dee21e3d5fca607408accf30e91`.
- `ia-thomas-park-bellshop-aac.m4a`: *Dried Blood And Painted Glass*, Thomas
  Park; Archive.org item is uploader-marked CC0 1.0; 2,450,405 bytes; AAC,
  48 kHz mono; SHA-256
  `45cd792f9f604210431ab8e11af90e569b351a8a10a8732468840222272c71d4`.
- Source item links, track names, licenses, hashes, and caveat are indexed in
  the fixture README. The script validates their bytes and actual codecs and
  requires `inspect` to reject each one.
- Archive.org rights labels are uploader-supplied metadata, not independent
  rights verification. Neither file is treated as lossless or playback input.

## 2026-10-30 — Offline gate and private FLAC parity revalidation

Re-ran the mandatory local gate and the six-track private FLAC corpus against
the current checkout. The supplied FLAC files remained in their original
private directory; generated PCM and seek-range outputs were kept temporary.

- `sh scripts/offline-gate.sh`: **exit 0**. The local-only audit found 16
  workspace crates, 29 path-only edges, zero registry/Git crates and zero
  third-party system links. Rust formatting, warnings-denied Clippy, workspace
  tests, release build, and macOS feature check passed.
- `CARGO_NET_OFFLINE=true python3 scripts/test_flac_user_corpus.py --mode all
  --verify-seek --require-complete <six supplied FLAC paths>`: **exit 0**.
  All six passed reference FLAC integrity, reference FLAC/FFmpeg equality,
  native Rust full-PCM hash/byte-count parity, and 132 SEEKTABLE sample plus
  following-sample ranges.
- These results strengthen local offline and decoder-parity evidence. They do
  not establish Rust_Boy cross-module arbitration, panel wiring, or physical
  device/DAC fidelity; those acceptances remain open.
- `system_profiler SPAudioDataType -detailLevel mini` returned an empty device
  list in this macOS session; no physical DAC negotiation/capture test could be
  performed here. This records environment availability only, not hardware
  behavior.

## 2026-10-30 — Linux native adapter cross-target check

Used the already-installed `aarch64-unknown-linux-gnu` Rust standard library to
check the Linux direct PCM adapter from macOS without adding packages or
network access.

- `cargo check --locked --offline -p rust-music-native --all-targets --features
  alsa --target aarch64-unknown-linux-gnu`: **exit 0**; backend and test targets
  type-check.
- `cargo clippy --locked --offline -p rust-music-native --all-targets --features
  alsa --target aarch64-unknown-linux-gnu -- -D warnings`: **exit 0**.
- `cargo build --locked --offline --release -p rust-music-native --features
  alsa --target aarch64-unknown-linux-gnu`: **exit 0**; optimized Linux library
  build succeeds.
- `cargo test --locked --offline -p rust-music-native --features alsa --target
  aarch64-unknown-linux-gnu --no-run`: **not linked**; the host macOS linker
  rejected Linux `ld` flags (`--fix-cortex-a53-843419`, `--as-needed`, `-z`).
  Linux test execution remains unverified and needs a Linux host/toolchain
  linker. This does not validate ALSA device negotiation or playback.

## 2026-10-30 — Host lease release on drain errors

Added host-slot regression cases for a `PcmSink::finish()` failure during an
explicit Stop and during EOF cleanup. Both require the MUSIC lease to be
released even though the caller receives the drain error. These use a mock
focus implementation; they strengthen Rust Music's side of the host contract
without claiming shared RADIO/TV arbitration.

- `cargo fmt --all -- --check`: **exit 0**.
- `cargo test --locked --offline -p rust-music-host`: **exit 0**, 12/12 tests.
- `cargo clippy --locked --offline -p rust-music-host --all-targets -- -D
  warnings`: **exit 0**.
- `sh scripts/offline-gate.sh`: **exit 0**; all 16 workspace crates audited,
  formatted, linted, tested, release-built, and the macOS feature check passed.

## 2026-10-30 — Host and panel crate portability checks

Cross-checked the framework-free `rust-music-host` and `rust-music-panel`
dependency closures against the locally installed AArch64 Linux and Windows
GNU targets. This verifies source and test-target compilation for those targets;
it does not establish native output support or runtime behavior there.

- `cargo check --locked --offline -p rust-music-host -p rust-music-panel
  --all-targets --target aarch64-unknown-linux-gnu`: **exit 0**.
- `cargo clippy --locked --offline -p rust-music-host -p rust-music-panel
  --all-targets --target aarch64-unknown-linux-gnu -- -D warnings`: **exit 0**.
- The matching `cargo check` and warnings-denied Clippy commands with
  `--target x86_64-pc-windows-gnu`: **exit 0**.
- Test binaries were type-checked but not linked or executed for either
  cross-target. Native OS audio adapters, host integration and physical output
  remain separate acceptance gates.

## 2026-10-30 — Validate public PCM formats and sample widths

Hardened the public integer PCM boundary. `AudioFormat::validate()` now checks
fields even when callers construct a struct literal; `PcmBlock::new()` and
`PcmBlock::validate()` reject invalid layouts, overlong blocks and samples
outside the declared signed width. Frame counting no longer divides by zero on
an invalid public block. The playback engine validates decoder formats once at
load and checks each block before frame arithmetic or sink writes. Null, Mac
AudioQueue, and Linux ALSA sinks validate inputs at their boundary as well.

- Added core tests for zero-channel public values and 16/24-bit signed bounds;
  engine tests prove invalid formats fail before output open and out-of-range
  samples fail before any sink write.
- `cargo test --locked --offline -p rust-music-core -p rust-music-engine`:
  **exit 0**, 13/13 tests.
- `sh scripts/offline-gate.sh`: **exit 0**; 16 crates audited, formatted,
  warnings-denied Clippy, full workspace tests, release build and macOS feature
  check passed.
- `python3 scripts/test_m4a_corpus.py`: **PASS**; 6/6 Archive.org ALAC files
  remained PCM-identical to FFmpeg and all 8 AAC fixtures were rejected.
- `python3 scripts/test_aiff_reference.py`: **PASS**, 7/7 exact PCM vectors and
  malformed/lossy rejection. `python3 scripts/test_wave_reference.py`:
  **PASS**, 6/6 PCM vectors and malformed RIFF rejection.
- `python3 scripts/test_caf_reference.py`: **exit 0**, 5/5 exact PCM vectors.
- `CARGO_NET_OFFLINE=true python3 scripts/test_flac_user_corpus.py --mode all
  --verify-seek --require-complete <six supplied paths>`: **PASS**, 6/6 native
  full-PCM parity and 132 real seek ranges.
- Linux ALSA all-target `cargo check` and warnings-denied Clippy, plus Windows
  host/panel all-target `cargo check` and warnings-denied Clippy: **exit 0**.
  Cross-target test targets type-check; they were not linked or run.

## 2026-10-30 — Correct host scan contract

Updated `docs/HOST_CONTRACT.md` to match the current library scanner's bounded
extension allowlist and FLAC/WAVE/AIFF/AIFF-C/CAF/MP4 signature checks. The
contract now explicitly says that catalog discovery is provisional and that
decoder initialization remains the playback admission boundary. No scan or
playback behavior changed.

## 2026-10-30 — Exercise macOS output guards without hardware

Added macOS native-backend unit tests proving malformed public formats and
out-of-range samples are rejected before the AudioQueue handle/context is
created or used. These are pure early-rejection cases and do not open a device.
Added the macOS native-feature test command to the Darwin branch of the
mandatory offline gate so these regressions run on future Mac gate executions.

- `cargo test --locked --offline -p rust-music-native --features macos`:
  **exit 0**, 2/2 tests.
- `sh scripts/offline-gate.sh`: **exit 0**; workspace gates passed and the new
  macOS backend tests ran. No physical playback or DAC behavior was tested.

## 2026-10-30 — Add real stereo WavPack parser fixture

Added a locally generated 16-bit stereo WavPack fixture and verified that the
bounded parser reads its block, sub-block sequence and distinct per-channel
entropy medians. A focused attempt to traverse its residual words failed with
`WavPack unary code exceeds configured bound`; this reproduces the real-stereo
decoder gap and keeps WavPack out of the public facade. No PCM parity or codec
support is claimed. RM-014 is now in progress.

- Temporary real-stereo residual traversal experiment:
  **exit 101**, exposed `WavPack unary code exceeds configured bound`; that
  failing experiment was replaced with a passing parser/metadata regression,
  and the decoder failure remains documented as open evidence.
- Fixture SHA-256:
  `1abae79d2a9ab2293d8700f161b5a332f8e2c9e866f03a151fe4ea7b87e141ff`.
- `cargo fmt --all -- --check`, focused stereo parser/entropy test,
  `python3 scripts/check_roadmap_index.py`,
  `python3 scripts/check_literature.py`, and `git diff --check`: **exit 0**.
- `sh scripts/offline-gate.sh`: **exit 0**; all 16 workspace crates passed
  formatting, warnings-denied Clippy, tests, release build and macOS feature
  checks after the new fixture regression was added.

## 2026-10-30 — Correct WavPack adjusted-binary bit order

The real stereo fixture isolated the word-reader problem to the adjusted-binary
prefix: the wire prefix is assembled least-significant bit first. Updated the
reader and fixed codeword vectors to represent that wire order. The generated
stereo stream now traverses all 1,764 residual words, and the existing mono
stream still traverses all 882. This is not PCM reconstruction or parity;
decorrelation and CRC validation remain required, and WavPack stays unregistered.

- `cargo fmt --all -- --check`: **exit 0**.
- `cargo test --locked --offline -p rust-music-wavpack`: **exit 0**, 28/28.
- `sh scripts/offline-gate.sh`: **exit 0**; all 16 workspace crates passed
  formatting, warnings-denied Clippy, tests, release build and macOS feature
  checks; zero registry/Git crates were used.
- `python3 scripts/check_roadmap_index.py`,
  `python3 scripts/check_literature.py`, and `git diff --check`: **exit 0**.

# 2026-10-30 — Rust Music native GUI theme

- Added a Rust Music-owned SwiftUI interface and local macOS app build script.
  The window adopts the Pip-Boy phosphor-green palette while remaining a
  distinct Music app. It opens one user-selected file, validates with the Rust
  decoder, and starts/stops the Rust CLI AudioQueue path only on explicit input.
- `scripts/build-macos-app.sh` completed successfully using the local Rust and
  Swift toolchains; no audio corpus is bundled. `target/macos/RustMusic.app` is
  ignored build output.
- Demonstrated the White Zombie FLAC in the GUI; Rust reported 44.1 kHz,
  stereo, 16-bit PCM and FLAC CRC validation. This is decoder acceptance and
  app-path evidence, not hardware bit-perfect or DAC certification.
- Pip-Boy's working tree already has a shared Radio audio-focus broker and a
  distinct uninstalled MUSIC catalog record. The trusted MUSIC adapter, live
  package switch binding, user-selected library path, off-UI playback worker,
  and shared lease bridge remain the next G5 integration scope. The standalone
  palette does not count as embedded host theme or module activation evidence.
- Added a distinct phosphor-green style to the Pip-Boy handheld MUSIC reference
  row and a regression assertion. `node --check crates/pipboy/assets/micro.js`,
  `node tools/test_micro_ui.cjs`, and `node tools/test_micro_viewport.cjs`
  (with the local Chrome executable configured) passed. Repository navigation,
  component-index, and link validators also passed. The module row stays disabled.
## 2026-10-30 — Establish bounded Pip-Boy package process protocol

- Added `rust-music-ipc` with strict `RBPK` v1 length-prefixed stdio frames,
  typed lifecycle/library/playback requests, 16 KiB frame cap, bounded identity,
  strings and 32-track view pages. Added `rust-music-cli serve`; it handshakes
  before handling requests, does not scan on Activate, and accepts library scan
  only as an explicit absolute-root request. Music stays a distinct package.
- Added the matching Pip-Boy SDK codec/adapter bridge, shared cross-repository
  hello vector, strict page/library limits, and explicit `media.library.read`
  scope vocabulary. The parent host remains the owner of the shared audio
  lease; IPC `audio_claimed` is informational only.
- A spawned-process experiment exposed that unflushed binary stdout frames
  stall the request/reply channel. Both codecs now flush complete frames; the
  serve-loop test covers handshake, inert activation, and clean shutdown.
- Deferred creation of the Linux/macOS output sink until a successful explicit
  Play reaches `PcmSink::open`; the endpoint test asserts activation/shutdown
  leave the device factory untouched.
- Process transport is not a sandbox. Production fixed-path supervision,
  deadline enforcement, timeout kill/reap, process-exit cleanup, shell inputs
  and real MUSIC mounting are still open RM-018 acceptance items.
- Rewrote three WavPack channel-index loops as equivalent iterator loops after
  the workspace's warnings-denied Clippy gate flagged the pre-existing dirty
  WavPack implementation; decoder behavior is unchanged.
- Verified: `cargo test --locked --offline -p rust-music-ipc -p
  rust-music-cli` passed (13 tests); `cargo test --locked --offline -p
  rust-music-cli --features macos-output` passed (15 tests including the
  in-memory serve loop test). `python3 scripts/check_roadmap_index.py` and
  `python3 scripts/check_literature.py` passed. Pip-Boy
  `cargo fmt --all && cargo test --locked --offline -p rust-boy-adapter-sdk -p
  pipboy-core` exited 0 (7 SDK tests, 294 core tests, 5 core integration
  tests). The full `sh scripts/offline-gate.sh` then exited 0 across 17 local
  crates/32 local edges; roadmap/literature validators and `git diff --check`
  also passed. The test-only in-memory endpoint lifecycle test is covered with
  `--features macos-output`; actual OS process supervision remains open.

## 2026-10-30 — Reconcile package-process supervisor status

- Reviewed Pip-Boy's current `docs/PACKAGE_PROCESS_PROTOCOL.md` and
  `pipboy-core::package_process`. The sibling repo now records a supervised
  child runner with transaction deadlines, termination/reap, stderr draining,
  shutdown handling, and process-exit audio-focus cleanup; its ledger reports
  targeted tests passing. Production acceptance is still open because the
  trusted executable roster is empty and no released Rust Music unit is mounted
  by live startup.
- Corrected Rust Music's G5/roadmap integration status accordingly and clarified
  in the IPC crate that frame-size bounds do not provide an I/O deadline:
  blocking stdio reads must run under the host supervisor's wall-clock limit.
- No tests or builds were run in this documentation reconciliation pass.

## 2026-10-30 — Exercise the Rust Music child process protocol

- Added a macOS-only CLI integration test that launches the Cargo-built
  `rust-music-cli serve` executable as a child and exchanges real stdio frames.
  It validates protocol negotiation, inert activation (empty catalog, no audio
  claim), shutdown acknowledgment, and successful child exit. Each reply and
  process exit is bounded by a three-second test deadline; a child guard kills
  and reaps the process if an assertion fails.
- `cargo test --locked --offline -p rust-music-cli --features macos-output
  --test package_process`: **exit 0**, 1/1. This is local spawned-binary
  evidence, not packaged-release, live Pip-Boy mount, audio-focus, or hardware
  acceptance.
- The first full offline-gate attempt rejected the test's process spawn. The
  Rust-only auditor now permits only the exact Cargo-provided
  `CARGO_BIN_EXE_rust-music-cli` spawn on this one test line; a regression test
  rejects shell commands and the same spawn outside that file.
- `cargo test --locked --offline -p rust-music-cli --features macos-output`:
  **exit 0**, 8 unit tests and the spawned-process test passed.
- `sh scripts/offline-gate.sh`: **exit 0**; 17 local crates, 32 path-only
  edges, 21 Rust sources, no registry/Git crates or third-party system links;
  formatting, warnings-denied Clippy, workspace tests, release build, and
  macOS feature checks passed.
- `python3 scripts/check_roadmap_index.py`,
  `python3 scripts/check_literature.py`, and `git diff --check`: **exit 0**.

## 2026-10-30 — Validate WavPack decoded-audio checksum

- Added `verify_decoded_pcm_checksum` to the WavPack foundation. It bounds the
  expected interleaved sample count from the block header, computes the
  decoded-sample checksum with wrapping arithmetic, and fails closed on a
  count mismatch or checksum mismatch.
- Extended the generated stereo regression: after decorrelation and joint
  stereo reconstruction, all 1,764 samples match the retained PCM, the header
  checksum matches, and a one-bit sample mutation plus a shortened sample
  vector are rejected. This is one generated 16-bit stereo stream, not complete
  WavPack decoder or format admission evidence.
- `cargo test --locked --offline -p rust-music-wavpack`: **exit 0**, 30/30.
- `sh scripts/offline-gate.sh`: **exit 0** across 17 local crates and 32
  path-only edges, with no registry/Git crates or third-party system links.

## 2026-10-30 — Add WavPack packed-16-bit single-block decoding

- Added `decode_lossless_block` for one bounded packed-16-bit mono/stereo block.
  It requires unique mandatory decorrelation, entropy and audio-bitstream
  metadata; rejects hybrid, extended-integer, float, DSD, unsupported metadata
  and predictor modes; checks the encoded-block checksum; reconstructs PCM;
  and validates decoded sample count and header checksum.
- Added an independent mono PCM golden generated by decoding the synthetic
  WavPack stream with FFmpeg 9.0.2. Both the mono and stereo fixtures reproduce
  all 882 frames exactly and pass decoded-header CRC checks. This does not
  establish a streaming decoder, multiblock support, broad profile coverage or
  WavPack playback admission. Recorded PCM SHA-256 is
  `28eae5aa0d1ad7e125c1e428b8a55383d13432446db1721843fb5a7f3b1428a9`; added a
  fixture-specific `.gitignore` exception so the golden is present in clean
  checkouts.
- `cargo test --locked --offline -p rust-music-wavpack`: **exit 0**, 31/31.
- `sh scripts/offline-gate.sh`: **exit 0** across 17 local crates and 32
  path-only edges; dependency audit, formatting, warnings-denied Clippy,
  workspace tests, release build and macOS feature checks passed.

## 2026-10-31 — Recover development history and audit restored application work

- Completed the interrupted merge with semantic resolutions in `AGENTS.md` and
  `README.md`. Merge commit `bf607d7` has local checkpoint `01c4400` and remote
  development head `4e3d24a` as parents; `origin/build/native-codecs-g0` is an
  ancestor of HEAD. Development also contains `origin/main` through the remote
  development history. Applied `stash@{0}` after the merge without conflicts;
  retained the stash and `backup/native-codecs-20261031-024820`. Removed only
  the generated Python bytecode cache. No push or PR merge was performed.
- Pinned the agent framework to `6989d835c62f9e0858db56252f1ac7ff6ce65414`,
  the newer Codex-priority revision, because it descends from the verified
  software-agent hierarchy and includes its roadmap and kickoff additions.
  Recorded the role mapping and limitations in
  `docs/AGENT_SKILLS_INTEGRATION.md`; the local Rust Music specialists remain
  the execution and review roles.
- Audited the restored WavPack implementation, IPC contract, release-process
  scripts and macOS app. Added RM-018/RM-019 tracking and recorded the limited
  local package/app evidence. The standalone UI was manually launched against
  generated audio: supported WAV inspection, explicit Play/Stop and generated
  MP3 rejection were observed. Window close stopped audible playback, but
  independent child-process exit verification was unavailable; RM-019 remains
  in progress. No hardware fidelity or live Rust_Boy mount is claimed.
- Replaced a long fixture derived from an uploader-labeled CC0 recording after
  its rights could not be independently verified. The prior files remain in
  the preserved stash and outside the repository; the checked-in long test
  vector is deterministic synthetic PCM. Its Rust WavPack test passes, as do
  the complete 49 WavPack crate tests. This supersedes the earlier historical
  work-log reference to that recording as a checked-in fixture.
- Added `docs/MICRO_LIBRARY_ZERO_ARTWORK.md`, documenting the bounded allowlist,
  metadata skip behavior, 2 MiB synthetic artwork regression, IPC view and
  1024-byte panel boundary. Linked it from the README, repository index and
  G13 metadata ledger. This describes the local catalog boundary and does not
  claim that source files are scrubbed.
- Additional observed checks during this recovery: `python3
  scripts/check_roadmap_index.py`, `python3 scripts/check_literature.py`,
  `python3 -m unittest discover -s tests -p 'test_roadmap_index.py'`,
  `python3 scripts/test_wave_reference.py`, `python3
  scripts/test_aiff_reference.py`, `python3 scripts/test_caf_reference.py`,
  `python3 scripts/test_flac_reference.py --fixtures-only`, and `python3
  scripts/test_m4a_corpus.py` passed. These report their specific local
  vectors/corpora and do not certify physical output or broad encoder
  compatibility. A final full offline gate is required after this log update.

## 2026-10-31 — RM-019 managed playback child cleanup

- Changed the standalone macOS app to launch `play-managed` with a private
  stdin pipe. Explicit Stop and window close close the pipe writer; parent
  process death closes it in the kernel. The child observes EOF and calls the
  player's normal stop path. A five-second child watchdog and app-side
  terminate/SIGKILL escalation prevent an unresponsive child from persisting,
  but escalation is not evidence of graceful AudioQueue teardown.
- Added opt-in, debug-only `test-support` child instrumentation and automated
  regressions for repeated parent pipe closure and playback-open failure. The
  four package-process integration tests passed. The stdlib-only
  `scripts/test_macos_parent_exit.py` force-killed an app-like supervisor and
  observed the reported playback child PID exit (exit 0). A desktop UI run
  exercised generated WAV inspection, Play/Stop, and window close; Activity
  Monitor showed no `rust-music-cli` process on subsequent refreshes. No private
  user media was used in these checks.
- `sh scripts/build-macos-app.sh` and `sh scripts/offline-gate.sh` exited 0.
  AIFF (7), WAVE (6), CAF (5), WavPack (49 Rust tests), M4A/ALAC (6 parity
  fixtures plus AAC rejection/ingestion), and FLAC generated fixtures passed.
  The six-file private FLAC run was not attempted because
  `PRIVATE_FLAC_DIRECTORY` is unavailable in this environment.
- `open -n target/macos/RustMusic.app` fails with LaunchServices
  `kLSNoExecutableErr`; direct bundle-path UI launch did work. RM-019 remains
  in progress pending clean-checkout package launch/install verification and
  native device availability/failure/disconnect/recovery tests. Physical DAC
  qualification and graceful teardown under forced watchdog escalation remain
  unverified.

## 2026-10-31 — six-file private FLAC and premerge revalidation

- Re-ran `PRIVATE_FLAC_DIRECTORY=<operator-supplied six-file corpus>
  RUST_MUSIC_EVIDENCE_DIR=/private/tmp/rust-music-gate-evidence sh
  scripts/premerge-check.sh` on clean commit
  `0bc3f1673a635e688b928406c63baa46bbbb22da` with Rust/Cargo 1.99.1 on macOS.
- Roadmap and literature validators plus eight roadmap-index unit tests passed;
  the offline Rust gate exited 0; WAVE passed 6/6; AIFF/AIFF-C passed 7/7;
  native FLAC metadata, compressed-file digest and full PCM hash/byte-count
  parity passed for L1, L2, L3, G4, G5 and G6. The premerge checker reached
  its final automated-check PASS. Its private log and summary stay outside Git.
- This completes the scripted local gates only. Host shared-focus, actual
  panel/DAC and device failure/recovery, LaunchServices installation, and
  independent release approval remain open. PR #1 remains draft and unpushed.

## 2026-10-31 — RM-019 LaunchServices and RM-020 text catalog start

- Verified local checkout at `8a057792586ba050521e6989137db4492f1ffb27`,
  preserved backup/stash, clean tree, and `main` ancestry before edits. Loaded
  the pinned `agent_skills` reference at
  `6989d835c62f9e0858db56252f1ac7ff6ce65414` and used host, audio-backend,
  gate, security and release specialist reviews.
- Fixed `scripts/build-macos-app.sh`: removes only the generated app's stale
  `_CodeSignature`, compiles the SwiftUI executable for macOS 13.0, ad-hoc
  signs after all payload copies, and fails unless strict deep verification
  passes. From clean source commit `bbd5dc3c52d3e187a1a332010f07d2a72906766c`,
  `sh scripts/build-macos-app.sh` exited 0; plist lint and strict signature
  verification passed; `vtool` reports `minos 13.0` matching Info.plist.
- The constrained terminal sandbox cannot contact LaunchServices: sandboxed
  `lsregister -f` fails `-10822`, and sandboxed `open -n` reports `-10827`.
  In the host macOS context, unregistering only the generated bundle exited 0,
  then `open -n` by bundle path exited 0 and auto-registered it. Narrow unified
  logs recorded LaunchServices CHECKIN. After opening a generated silent WAV,
  Play/Stop worked; closing the window during playback left no
  `rust-music-cli` row in Activity Monitor. `spctl` rejects ad-hoc signing;
  Developer ID/notarization/distribution are not claimed.
- Marked RM-019 local app lifecycle acceptance done. RM-005 hardware and
  RM-006/RM-018 live host integration remain separate. ML-00 inventory found
  no stable catalog IDs, text search, playlists, or persistent queue; the
  host-integrator owns the initial ML-01/ML-03 catalog/search implementation in
  `crates/rust-music-library/src/lib.rs`. The zero-artwork policy now records
  the complete ML-00–ML-07 feature and performance contract.

## 2026-10-31 — Integrated standalone transport controls

- Added Pause/Resume, bounded 0–100 software volume and functional Next/
  Previous queue/catalog transitions to the standalone SwiftUI workflow using
  the persistent RBPK package process. Queue/library selection stays
  non-autoplay. State commands are blocked during transport and a deferred
  persistent-state reload handles a successful catalog load while Stop is
  finishing. Window shutdown sends Stop/Shutdown, closes pipes and escalates
  through terminate/SIGKILL with a child reap if needed.
- Rust transport work is committed in `fc47a19` (engine/host/IPC/CLI/panel
  controls) and `7ae312b` (AudioQueue/ALSA pause before first buffer). The
  integrated package-stream Play/Pause/Resume/volume/Stop E2E passes. The
  macOS injected AudioQueue suite passes 15/15; ALSA target compilation for
  `aarch64-unknown-linux-gnu` passes, but Linux device execution is unavailable
  on this Mac.
- At `7ae312b` plus the integration diff, `sh scripts/offline-gate.sh` exited
  0 (17 local crates, 32 local path edges, no registry/Git crates), and
  `sh scripts/build-macos-app.sh` exited 0 with strict local ad-hoc bundle
  signature verification. Roadmap/literature validators and `git diff --check`
  exited 0. Source search found no image or visualizer APIs. Independent Swift
  review found and verified a fix for Next/Previous while paused with an empty
  queue. No private codec corpus was rerun.
- MAC-01 is implemented pending observed interaction with this exact app
  window. Hardware output/device disconnect and recovery, physical panel/DAC,
  Rust Boy shared Radio/TV focus, cold startup/incremental RSS, Developer ID
  distribution signing, and final security review remain open. PR #1 stays
  draft and unmerged.
- Follow-up LaunchServices smoke on the rebuilt `0b1263c` app: `open -n`
  exited 0; `lsappinfo` recorded a 0.218 s launch-to-check-in for PID 89533.
  One RSS snapshot at about 20 s was 94,432 KiB, which includes app/framework
  baseline and is not incremental catalog memory. SIGTERM followed by `ps -p`
  exit status 1 confirmed validation PIDs 89533 and 88443 exited. No playback
  child was started; this is launch/exit evidence, not audible output.

## 2026-10-31 — Fresh audio, offline gate and 2k UI profile

- Revalidated source `9d1141975d5fa54c31fa2a7ce25c622c4218ebba` with
  `sh scripts/offline-gate.sh` (exit 0): 17 local crates, 32 path-only edges,
  zero registry/Git crates; workspace tests, Clippy, release build and macOS
  feature checks passed, including AudioQueue injected failure/teardown 15/15.
- The app-bundled CLI played a generated nonzero stereo 440 Hz, 16-bit/44.1
  kHz WAV through the native output path (exit 0). Unsandboxed
  `system_profiler SPAudioDataType -detailLevel mini` listed MacBook Air
  Speakers as the 48 kHz default output. No DAC loopback/capture was made and
  private FLAC parity was not rerun.
- The existing app loaded 2,000 generated 12-byte WAV-named catalog entries;
  UI state returned all 2,000 rows in under one second. Searching
  `track-01999` showed one match (3.3 seconds including UI automation). The
  same app PID RSS rose 103,312→104,880 KiB (+1,568 KiB), a single observation
  rather than a performance threshold. Normal Cmd-Q removed app PID 93393;
  the subsequent process query found no `rust-music-cli serve` child.
- Recorded this evidence in the macOS v1 matrix, G15/RM-020 roadmap entries,
  and the zero-artwork library ledger. Device loss/recovery, measured DAC,
  Rust Boy shared focus, repeatable startup/memory/latency measurements and
  final release review remain.

## 2026-10-31 — Stale removable-track process regression

- Added `spawned_cli_rejects_track_removed_after_scan_without_starting_playback`
  to the macOS package-process E2E suite. It scans a generated WAV signature,
  removes the file, then sends Play to the real spawned CLI process.
- The process returns the bounded `Decoder` error, remains queryable with
  playback `Stopped` and `audio_claimed=false`, and exits successfully after an
  explicit Shutdown. The focused test passed 1/1 and the full package-process
  integration suite passed 5/5. An initial test assertion expected `Empty`
  instead of the observed post-scan `Stopped` state; correcting that
  expectation made the regression pass without product-code changes.
- `sh scripts/offline-gate.sh` passed at working tree based on `110bfed` (17
  local crates, 32 local edges, zero registry/Git dependencies, all unit and
  workspace tests, Clippy, release build and macOS feature checks; injected
  AudioQueue failure/teardown 15/15). Physical device loss during active
  output, Rust Boy shared focus, DAC measurement, repeatable app profiling,
  distribution signing, and mainline release approval remain open.

## 2026-10-31 — Compact phosphor macOS player theme

- Reworked `apps/rust-music-macos/RustMusicApp.swift` as a clean text-first
  phosphor panel meant to sit inside the Pip-Boy wrapper. The UI now lays out
  player status and persistent queue above library/search and playlists, with
  Play/Pause/Stop/Previous/Next and volume pinned in a bottom bar. It uses no
  second chassis, art, mascot, waveform, EQ bar, or image API.
- Commit `fb5e70dd62a348053088a8bc03e9ea5e85e42a3e` built successfully with
  `sh scripts/build-macos-app.sh`; `plutil -lint` and strict deep codesign
  verification passed. Unsandboxed `open -n` exited 0; LaunchServices checked
  in after 0.0901999 seconds. The accessibility tree exposed all four panels
  and transport controls; a 974×620 screenshot showed the full layout.
- UI smoke used only the generated two-tone folder under `/private/tmp` (no
  private music scan). Two catalog entries loaded; queue remained stopped;
  playlist creation and deletion both worked. The user separately confirmed
  output and library load work. No playback was started during this theme pass.
- `cargo fmt --all -- --check`, `git diff --check`, and a source scan for image
  and visualization APIs passed. This pass changed SwiftUI presentation only;
  Rust offline test results remain those already recorded at `2984278`.

## 2026-10-31 — Transport accessibility identifiers

- Independent `gate_verifier` review of `fb5e70d` found no theme blocker and
  recommended distinct accessibility identifiers on transport controls for
  reliable automated UI targeting. Added identifiers for Previous, Play,
  Pause/Resume, Stop, Next and volume.
- The app source compiled successfully with the installed Swift compiler at
  macOS 13.0 target to a temporary output outside the repository (exit 0), and the
  source scan still found no image or visualization APIs. The app bundle was
  staged outside the repository using that Swift binary and the current bundled
  CLI. Its plist lint, strict deep signature check, arm64 executable check and
  macOS 13.0 minimum check passed. It was not opened or LaunchServices
  registered; the normal target bundle was not replaced because a live app
  session was present. Runtime accessibility-tree verification remains
  pending. Prior LaunchServices evidence stays tied to `fb5e70d`.
