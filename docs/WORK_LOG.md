# Rust Music work log

This log records observed repository work and the next release gates. The
machine-readable task state is authoritative in [roadmap-index.json](roadmap-index.json).
Private audio paths and recordings are deliberately excluded.

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
