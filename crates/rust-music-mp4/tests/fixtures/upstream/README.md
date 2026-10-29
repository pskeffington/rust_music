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
| `ia-mazurka-op17-no3.m4a` | `Mazurka Op. 17 no. 3 in A flat major.m4a` | ALAC, 44,100 Hz, stereo, 16-bit | `3b59179aec77376f5609a7130ad7cd9a19d9ae484b98f3a6f4c80152922d0d10` |
| `ia-mazurka-op17-no4.m4a` | `Mazurka Op. 17 no. 4 in A minor.m4a` | ALAC, 44,100 Hz, stereo, 16-bit | `e231321ea7671950c16edd0d5bd93863479efbd1cb8acae183d2d9c5c16eb3bb` |

The corpus contains no user music. FFmpeg/ffprobe are optional local test
tools only and are not Cargo or runtime dependencies. These six tracks add
real-world packet/table coverage to generated vectors, but all come from one
collection and do not establish compatibility across independent encoders or
devices. Two tracks expand same-source coverage; they do not close the
independent-source/encoder gap.

## AAC ingestion-denial sample

`ia-stificash-starstedede-aac.m4a` was downloaded from Internet Archive item
[`starstedede`](https://archive.org/details/starstedede), credited to
Stificash and labeled CC0 1.0 in the item's metadata. The item describes it as
a remix. The checked-in 3.5 MB file is verified as AAC, 44,100 Hz, stereo by
the optional corpus script. It is an ingestion/codec-denial fixture only: AAC
is lossy and must never be decoded for playback by Rust Music. SHA-256:
`b3735fba8acf607bf51438fa43ad4c314da46c07d75482eae99629bc3d422857`.

The AAC sample exercises real MP4-family container ingestion and fail-closed
codec admission. It does not expand ALAC decoder parity coverage. The item's
rights label is uploader-supplied Archive.org metadata, not an independent
rights audit.
