# G13 — Bounded metadata and sample-accurate seek

## RM-011 — bounded local text tags

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

Sample-accurate seek remains unimplemented. Do not expose seeking until
validated frame/seektable behavior and exact PCM-frame tests pass.
