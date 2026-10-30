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

## Additional album AAC rejection samples

Two tracks were selected from separate Archive.org Netlabels albums and
downloaded on 2026-10-29 as small real-world M4A ingestion cases. Archive.org
lists both as `MPEG-4 Audio`; local `ffprobe` confirms AAC, 44,100 Hz, stereo.
They are rejection fixtures only and do not add ALAC decoder parity.

| Fixture | Archive.org item / album / creator | Track | Item license | SHA-256 |
| --- | --- | --- | --- | --- |
| `ia-ltts001a-amazing-grace-aac.m4a` | [`LTTS001A`](https://archive.org/details/LTTS001A), *Winter's Child*, Lost to the Sea | `03. Amazing Grace.m4a` | [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/) | `ca33660a87bf7d02676562851a5806f2bbdd6c5022fa8b17dadd4715e9835243` |
| `ia-sonichuwave-tape-rip-aac.m4a` | [`christian-thbe-hegdgehgog-boys-vapor-wave-tape-rip`](https://archive.org/details/christian-thbe-hegdgehgog-boys-vapor-wave-tape-rip), sonichuwave クィック | `02 Ｓｏｎｉｃｈｕ＇ｓ Ｚｉｐ.m4a` | [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) | `2406cdcb798c50b7cdf4b23a2f94986c82f373785957d04974972a30570987ca` |

Attribution is preserved here with each source item and track. The Archive.org
license values are uploader-supplied and are not an independent rights audit.
The optional corpus script verifies each fixture hash and observed codec,
indexes each file, and confirms the decoder refuses AAC.

## Additional Archive.org album AAC denial samples

The user requested a small real-world M4A corpus selected from Archive.org.
Two additional album items were selected from the Netlabels search results;
the downloaded tracks are kept byte-for-byte and counted only as AAC
ingestion/codec-denial fixtures. Their uploader-supplied licenses are recorded
for attribution and are not an independent rights audit.

| Fixture | Archive.org item / album / creator | Track | Observed stream | Item license | SHA-256 |
| --- | --- | --- | --- | --- | --- |
| `ia-utopia-nothing-changes-aac.m4a` | [`MARAV955A`](https://archive.org/details/MARAV955A), *Nothing Changes*, Utopia | `02 Nothing Changes.m4a` | AAC, 48,000 Hz, mono | [CC BY-NC-ND 4.0](https://creativecommons.org/licenses/by-nc-nd/4.0/) | `02de330aa3d135bd4faea0c47dadb1a0b28a8501fc486adde2db69da2c3743cc` |
| `ia-foot292-movimento-iii-aac.m4a` | [`foot292`](https://archive.org/details/foot292), *Movimento*, Filipe Miranda | `foot292_03-filipe_miranda-movimento_iii.m4a` | AAC, 44,100 Hz, stereo | [CC BY-NC-SA 4.0](https://creativecommons.org/licenses/by-nc-sa/4.0/) | `ca20e55a053594f39d28ec7920434c861a64dac83ed84cc226d6658a4916f0b9` |

Both are AAC rejection inputs, never playback fixtures. The mono 48 kHz track
adds a second real channel/rate profile to the denial corpus; it does not add
lossless mono ALAC coverage.

## Corpus coverage and remaining files needed

| Test need | Checked-in evidence | Still needed |
| --- | --- | --- |
| ALAC positive parity | Six Musopen files: five stereo 16-bit/44.1 kHz and one stereo 24-bit/48 kHz; exact Rust/FFmpeg PCM equality | An independent ALAC source/encoder, plus real M4A samples covering supported 20-bit/32-bit and mono profiles |
| AAC denial in M4A | One CC0-labeled Archive.org sample, four separately licensed netlabel album tracks, and one generated AAC sample; all six are codec-probed and rejected | Broader container layouts only where they exercise parser profiles we intend to support |
| MP4-family filename ingestion | Real `.m4a` inputs and byte-identical `.mp4a`/`.mp4` filename aliases are scanned by content signature | Multi-track/video-bearing container layouts remain untested; extension alone must never grant codec support |
| CAF ALAC/LPCM | Generated ALAC 16/24-bit and signed LPCM 16/24/32-bit vectors pass exact PCM parity | Independently sourced real-world CAF files |

Corpus rows marked “still needed” are evidence gaps, not implied support. AAC
fixtures validate fail-closed admission only; they are never playback inputs.
