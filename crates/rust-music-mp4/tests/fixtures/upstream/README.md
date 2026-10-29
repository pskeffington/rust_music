# Upstream ALAC/M4A fixtures

These files were downloaded on 2026-10-29 from Internet Archive item
[`musopen-chopin`](https://archive.org/details/musopen-chopin), which lists
them as Apple Lossless Audio and records CC0 1.0 in item metadata. Musopen's
official Chopin project FAQ states its commissioned recordings are released
under CC0: <https://www.kickstarter.com/projects/Musopen/set-chopin-free/faqs>.
The item identifies its creator as Aaron Dunn; `scripts/test_m4a_corpus.py`
independently verifies both ALAC codec and stream parameters with local
`ffprobe` before parity. Retain this provenance note with the files.

| File | Internet Archive file | Observed stream | SHA-256 |
| --- | --- | --- |
| `ia-canon-f-minor.m4a` | `Canon in F minor.m4a` | ALAC, 48,000 Hz, stereo, 24-bit | `239f85a564581fef5db513e78b63e7fa9b50d48e63e1927a9943dc647f578b69` |
| `ia-mazurka-op24-no2.m4a` | `Mazurka Op. 24 no. 2 in C major.m4a` | ALAC, 44,100 Hz, stereo, 16-bit | `e1ba599d407fd360ab631d5b1508eae8b20822952c3bb49637f8d88cd6ff9c3c` |
| `ia-mazurka-op7-no4.m4a` | `Mazurka Op. 7 no. 4 in A flat major.m4a` | ALAC, 44,100 Hz, stereo, 16-bit | `bccc40830b8a8d72185801ae9159af8b8360e7ed4f91d107bb234f892eb02840` |
| `ia-mazurka-op24-no3.m4a` | `Mazurka Op. 24 no. 3 in A flat major.m4a` | ALAC, 44,100 Hz, stereo, 16-bit | `c27735d5b4b2a5d898a7848b3aa67b68e84fe80080bc25fdc61b61909da59899` |

The corpus contains no user music. FFmpeg/ffprobe are optional local test
tools only and are not Cargo or runtime dependencies. These four tracks add
real-world packet/table coverage to generated vectors, but all come from one
collection and do not establish compatibility across independent encoders or
devices.
