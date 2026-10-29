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
