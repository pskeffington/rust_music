# G13 — Bounded metadata and sample-accurate seek

## RM-011 — bounded local text tags

The dedicated [micro library zero-artwork policy](MICRO_LIBRARY_ZERO_ARTWORK.md)
documents the allocation boundary and its 2 MiB synthetic picture-field test.

The explicit, offline library scan extracts only allowlisted display fields:

- Native FLAC Vorbis comments: `TITLE`, `ARTIST`, and `ALBUM`.
- AIFF/AIFF-C text chunks: `NAME` and `AUTH`.

Tag values must be valid UTF-8, are trimmed, stripped of control characters,
limited to 512 input bytes and 120 displayed Unicode scalar values. The
filename remains available as the deterministic fallback. Duplicate fields
use the first valid value. At most 4096 Vorbis comment entries are examined
per file. Unknown fields are skipped. Vorbis-embedded picture
data is not decoded or copied into the panel; the scanner reads only a short
key prefix and seeks over the remaining value. The panel renderer receives
only `Track::display_title()` and remains fixed at 1024 bytes.

The scanner reuses one open file handle for signature recognition and metadata
reads. This avoids reopening a changed path between those operations; the
existing host contract still documents that portable path checks cannot close
all hostile local filesystem races without OS-specific no-follow opens.

Deterministic library tests cover Unicode titles, invalid UTF-8, oversized
FLAC and AIFF values, unknown fields, and a 2 MiB embedded picture field. The
oversized/invalid title cases use the filename fallback, and the picture field
does not appear in the exposed tag model.

Observed local evidence on 2026-10-29:

- `sh scripts/offline-gate.sh`: **exit 0**; 15 local crates, 28 path-only internal edges, zero registry/Git crates and zero third-party system links; fmt, warnings-denied Clippy, workspace tests (including library 6/6, panel 5/5, host 6/6), release build and macOS feature check passed.
- Read-only `safety_reviewer` review: no blocking parser findings; its reopen-race observation was addressed by using one file handle.

## RM-012 — FLAC seek

Implemented for seekable FLAC readers with a declared STREAMINFO sample count.
The decoder validates the bounded SEEKTABLE, selects the nearest preceding
point (or the first frame), decodes/discards frames, and trims PCM at the
requested sample. Fixed- and variable-block fixtures verify exact PCM at frame
boundaries and within frames, end-of-stream, invalid targets, and malformed
offset rejection. The host `Command::SeekFrame` drains active output, releases
the audio lease and returns to `Ready`; it never resumes automatically.

Synthetic coverage is supplemented by real-world tests across the six supplied
streams. Compatibility with other encoders and host input routing remain open.
A partial seek cannot verify the whole-stream PCM MD5, so successful seek does
not claim that checksum validation. The panel does not expose a seek control
until the host provides a reviewed input path.

On 2026-10-29, extended the private-corpus harness with `--verify-seek`. The
offline CLI now has `decode-range FILE START_FRAME FRAME_COUNT OUTPUT.wav`,
which requires a seek-capable decoder, declared total, in-range sample bounds,
and a WAV-size-safe requested range. With the six supplied FLACs, the harness
compares up to 257-frame Rust output at every real SEEKTABLE sample and the
following sample against FFmpeg PCM. All 132 ranges across six streams passed
at every populated seekpoint. The source FLACs and all temporary reference/range
PCM remained outside Git and were removed from temporary storage. This validates
these six encodes only; whole-file decoding/FLAC integrity is checked separately,
and a partial seek does not validate the complete stream MD5.

Observed command:

```sh
CARGO_NET_OFFLINE=true python3 scripts/test_flac_user_corpus.py --mode all --verify-seek --require-complete \
  '/path/to/private-flac-directory'
```
