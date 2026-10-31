# WavPack decoder fixtures

`ffmpeg-synthetic-mono16.wv` is a 20 ms synthetic 997 Hz sine wave encoded
locally with FFmpeg's WavPack encoder. It contains no user or third-party
recording and is used only to validate the bounded framing parser. The optional
FFmpeg tool is not a runtime or Cargo dependency.

Reproduction command:

```sh
ffmpeg -nostdin -v error -f lavfi \
  -i 'sine=frequency=997:sample_rate=44100:duration=0.02' \
  -c:a wavpack -y ffmpeg-synthetic-mono16.wv
```

`ffprobe` reports WavPack, mono, 44,100 Hz, 16-bit raw samples, and 882 frames.
SHA-256: `79b2a625223caa17f14aade28a64b929ad0e9101bfde90616834851277a38577`.

The Rust test parses the complete block and decodes all 882 mono samples with
the bounded packed-16-bit path. The result matches the retained PCM reference
decoded from this `.wv` file by FFmpeg 9.0.2, and the decoded-audio header
checksum passes. This is one generated mono profile; WavPack remains
unsupported by the Rust Music playback facade.

Reference PCM reproduction command:

```sh
ffmpeg -nostdin -v error -i ffmpeg-synthetic-mono16.wv -map 0:a:0 \
  -c:a pcm_s16le -f s16le -y ffmpeg-synthetic-mono16.pcm
```

Reference PCM SHA-256:
`28eae5aa0d1ad7e125c1e428b8a55383d13432446db1721843fb5a7f3b1428a9`.

`ffmpeg-synthetic-multiblock16.wv` is a 100 ms synthetic 997 Hz mono stream
with five contiguous audio blocks (four 1,024-frame blocks and one 314-frame
block). Its bounded incremental decode matches every sample in the FFmpeg
9.0.2 PCM reference, validates the declared 4,410-frame total, and rejects a
truncated final block. The checked-in WavPack block stream omits FFmpeg's
trailing APEv2 tag; external trailing tags are not yet part of the decoder
profile.

Run these from this fixture directory to reproduce the tagless stream and PCM
golden:

```sh
ffmpeg -nostdin -v error -f lavfi \
  -i 'sine=frequency=997:sample_rate=44100:duration=0.1' \
  -c:a wavpack -y /tmp/rust-music-multiblock-with-tags.wv
python3 - <<'PY'
from pathlib import Path
source = Path('/tmp/rust-music-multiblock-with-tags.wv').read_bytes()
offset = 0
while offset + 32 <= len(source) and source[offset:offset + 4] == b'wvpk':
    size = int.from_bytes(source[offset + 4:offset + 8], 'little') + 8
    if size < 32 or offset + size > len(source):
        raise SystemExit('invalid generated WavPack block size')
    offset += size
if source[offset:offset + 8] != b'APETAGEX':
    raise SystemExit('expected FFmpeg APEv2 trailer')
Path('ffmpeg-synthetic-multiblock16.wv').write_bytes(source[:offset])
PY
ffmpeg -nostdin -v error -i ffmpeg-synthetic-multiblock16.wv \
  -map 0:a:0 -c:a pcm_s16le -f s16le -y ffmpeg-synthetic-multiblock16.pcm
```

WavPack SHA-256:
`3b4761fda073198792e5bc4a36415fc2dab49b764fa73ed1d3446c0d5002fdff`.
PCM SHA-256:
`bbae3efd264f97ab3f2d3763b83684af85206ec47da53170d89eb0fd995c6841`.

`ffmpeg-synthetic-multiblock-stereo16.wv` is a 100 ms synthetic two-tone
stereo stream encoded by FFmpeg 9.0.2. It contains five contiguous blocks.
The first block uses joint stereo and the following blocks switch to true
stereo. All five blocks now decode to the independent FFmpeg PCM exactly and
pass their decoded-sample checksums. The prior one-unit mismatch at
interleaved sample index 2,101 came from clamping positive-term adaptive
weights to ±1024; WavPack permits these weights to exceed unit gain. As with
the mono stream, the checked-in block sequence omits the trailing APEv2 tag so
trailing-tag support remains an explicit limitation.

Reproduction commands:

```sh
ffmpeg -nostdin -v error -f lavfi \
  -i 'aevalsrc=0.7*sin(2*PI*997*t)|0.5*sin(2*PI*439*t):s=44100:d=0.1,aformat=sample_fmts=s16' \
  -c:a wavpack -y /tmp/rust-music-multiblock-stereo-with-tags.wv
python3 - <<'PY'
from pathlib import Path
source = Path('/tmp/rust-music-multiblock-stereo-with-tags.wv').read_bytes()
offset = 0
while offset + 32 <= len(source) and source[offset:offset + 4] == b'wvpk':
    size = int.from_bytes(source[offset + 4:offset + 8], 'little') + 8
    if size < 32 or offset + size > len(source):
        raise SystemExit('invalid generated WavPack block size')
    offset += size
if source[offset:offset + 8] != b'APETAGEX':
    raise SystemExit('expected FFmpeg APEv2 trailer')
Path('ffmpeg-synthetic-multiblock-stereo16.wv').write_bytes(source[:offset])
PY
ffmpeg -nostdin -v error -i /tmp/rust-music-multiblock-stereo-with-tags.wv \
  -map 0:a:0 -c:a pcm_s16le -f s16le -y ffmpeg-synthetic-multiblock-stereo16.pcm
```

WavPack SHA-256:
`e7cfc75cf08ef4622f65505a86ebfe42089f79b35b580718705b89db5daa4cca`.
PCM SHA-256:
`b4346ac3b4f465376d78c9302a9195a3edde0821a4ff99175b0dd955bae7ea90`.

`ffmpeg-synthetic-multiblock-stereo-nojoint16.wv` is the same generated source
encoded with `-joint_stereo 0`, so all five audio blocks use true stereo. All
5,120 interleaved samples decode exactly against the PCM reference above and
each block checksum passes. Strip its APEv2 trailer with the Python block-walk shown above;
the tagged source is reproduced with:

```sh
ffmpeg -nostdin -v error -f lavfi \
  -i 'aevalsrc=0.7*sin(2*PI*997*t)|0.5*sin(2*PI*439*t):s=44100:d=0.1,aformat=sample_fmts=s16' \
  -c:a wavpack -joint_stereo 0 -y /tmp/rust-music-multiblock-stereo-nojoint-with-tags.wv
```

The tagless WavPack stream SHA-256 is
`c95de905e2e099e70ea79f312525058a138d96e54bf77d789dc14612cd00f8cd`.

`ffmpeg-synthetic-random-stereo16.wv` covers a deterministic 16-bit stereo
pseudorandom PCM source. It contains two contiguous blocks: 4,096 frames in
joint stereo followed by 314 frames in true stereo. Rust decodes all 4,410
frames exactly against the original PCM bytes. This stresses predictor weight
adaptation and a block mode change; it remains FFmpeg-encoder fixture evidence,
not independent-encoder or real-world coverage.

Reproduce the PCM source, WavPack stream, and PCM golden with Python 3 and
FFmpeg 9.0.2:

```sh
python3 - <<'PY'
import random
import wave

random_source = random.Random(0x52555354)
pcm = bytearray()
for _ in range(4410 * 2):
    pcm.extend(random_source.randrange(-32768, 32768).to_bytes(2, "little", signed=True))
with wave.open("ffmpeg-synthetic-random-stereo16.wav", "wb") as output:
    output.setnchannels(2)
    output.setsampwidth(2)
    output.setframerate(44100)
    output.writeframes(pcm)
open("ffmpeg-synthetic-random-stereo16.pcm", "wb").write(pcm)
PY
ffmpeg -nostdin -v error -i ffmpeg-synthetic-random-stereo16.wav \
  -c:a wavpack -y ffmpeg-synthetic-random-stereo16.wv
```

Remove FFmpeg's trailing APEv2 tag from the `.wv` output using the block-walk
shown above before checking in the fixture. WavPack SHA-256:
`b40c22e2e5e0ab4cecd1545b4121c6177e09616053234d28edc7ce5e87db567d`.
PCM SHA-256:
`83e264462b2b805777a3eab217f79e3cddbc92be2411c160639eb130ac5a27af`.

`ffmpeg-synthetic-stereo24.wv` and `ffmpeg-synthetic-mono24.wv` cover only
WavPack's padded 24-bit integer layout with `INT32_INFO` bytes `[0, 8, 0, 0]`:
24 significant bits are stored in 32-bit words with eight low zero bits in the
external 32-bit representation. The Rust decoder validates the WavPack block
checksum on the reconstructed 24-bit integer samples, then checks their signed
24-bit range. Both 4,096- and 314-frame blocks match the original signed
24-bit PCM for mono and stereo. Other `INT32_INFO` layouts, 32-bit PCM, and
WavPack WVX data remain unsupported.

Reproduce the source and WavPack stream with FFmpeg 9.0.2, extract the packed
24-bit source PCM as the independent golden, and strip the `.wv` APEv2 trailer
with the block-walk shown above:

```sh
ffmpeg -nostdin -v error -f lavfi \
  -i 'aevalsrc=0.7*sin(2*PI*997*t)|0.5*sin(2*PI*439*t):s=44100:d=0.1' \
  -c:a pcm_s24le -y ffmpeg-synthetic-stereo24.wav
ffmpeg -nostdin -v error -i ffmpeg-synthetic-stereo24.wav \
  -map 0:a:0 -c:a pcm_s24le -f s24le -y ffmpeg-synthetic-stereo24.pcm
ffmpeg -nostdin -v error -i ffmpeg-synthetic-stereo24.wav \
  -c:a wavpack -y /tmp/ffmpeg-synthetic-stereo24-with-tags.wv
ffmpeg -nostdin -v error -f lavfi \
  -i 'aevalsrc=0.7*sin(2*PI*331*t):s=44100:d=0.1' \
  -c:a pcm_s24le -y ffmpeg-synthetic-mono24.wav
ffmpeg -nostdin -v error -i ffmpeg-synthetic-mono24.wav \
  -map 0:a:0 -c:a pcm_s24le -f s24le -y ffmpeg-synthetic-mono24.pcm
ffmpeg -nostdin -v error -i ffmpeg-synthetic-mono24.wav \
  -c:a wavpack -y /tmp/ffmpeg-synthetic-mono24-with-tags.wv
```

WavPack SHA-256:
`a7b51346754d60ab260220c2f1ceb3ef0f2fce312deb8aaaf49db105bcd3e30a`.
PCM SHA-256:
`909cfa8bbc39faaa86887d125a08b65910633b3ef02a0d5529237ca0b0345edf`.
The mono WavPack SHA-256 is
`9a6f0627d0c48150ecf8428d3b5bf07edea3601938f6b9947d91ad5a7f870c12`;
its PCM SHA-256 is
`7049d5b60f120cc6d5902590fc9b5ac30458b577ca2ed4f2ba977e0bbc362357`.

`ffmpeg-synthetic-random-stereo24.wv` stresses the same exact padded-24-bit
profile with high-entropy signed sample values, including both extrema, zero,
and small values. It contains 4,410 stereo frames in 4,096- and 314-frame
blocks. The Rust decoder reproduces all 8,820 interleaved samples exactly from
the deterministic packed-24-bit PCM golden. FFmpeg 9.0.2 also decodes the
stream back byte-for-byte to that original PCM. This is still generated
FFmpeg-encoder evidence, not independent WavPack encoder or real-world
compatibility evidence.

Reproduce from this directory with Python 3 and FFmpeg 9.0.2:

```sh
python3 - <<'PY'
from pathlib import Path
import random

source = random.Random(0x525553543234)
samples = [source.randrange(-(1 << 23), 1 << 23) for _ in range(4410 * 2)]
samples[:16] = [-(1 << 23), (1 << 23) - 1, -1, 0, 1, -2, 2, -32768,
                32767, 32768, -32769, 1234567, -7654321, 4194304,
                -4194304, 42]
pcm = b"".join(value.to_bytes(3, "little", signed=True) for value in samples)
Path("ffmpeg-synthetic-random-stereo24.pcm").write_bytes(pcm)
Path("/tmp/ffmpeg-synthetic-random-stereo24.raw").write_bytes(pcm)
PY
ffmpeg -nostdin -v error -f s24le -ar 44100 -ac 2 \
  -i /tmp/ffmpeg-synthetic-random-stereo24.raw -c:a wavpack \
  -y /tmp/ffmpeg-synthetic-random-stereo24-with-tags.wv
python3 - <<'PY'
from pathlib import Path

source = Path("/tmp/ffmpeg-synthetic-random-stereo24-with-tags.wv").read_bytes()
offset = 0
while offset + 32 <= len(source) and source[offset:offset + 4] == b"wvpk":
    length = int.from_bytes(source[offset + 4:offset + 8], "little") + 8
    if length < 32 or offset + length > len(source):
        raise SystemExit("invalid generated WavPack block")
    offset += length
if source[offset:offset + 8] != b"APETAGEX":
    raise SystemExit("expected FFmpeg APEv2 trailer")
Path("ffmpeg-synthetic-random-stereo24.wv").write_bytes(source[:offset])
PY
ffmpeg -nostdin -v error -i ffmpeg-synthetic-random-stereo24.wv \
  -map 0:a:0 -c:a pcm_s24le -f s24le \
  -y /tmp/ffmpeg-synthetic-random-stereo24-decoded.raw
cmp ffmpeg-synthetic-random-stereo24.pcm \
  /tmp/ffmpeg-synthetic-random-stereo24-decoded.raw
```

WavPack SHA-256:
`9e51d6b43aca74ca6204135788fc886d93b31d74f51f53e50233ac43472ad243`.
PCM SHA-256:
`9c4ab90d7e5c55513d72afef6f1d8397ea6ff7cd61bd1e7d249ae191327c82cc`.

`tiny-encoder-4.40-random-stereo16.wv` and
`tiny-encoder-4.40-random-stereo24.wv` encode the same deterministic high-
entropy source PCM as the preceding FFmpeg fixtures, but with the official
WavPack Tiny Encoder 4.40 in pure lossless `-f` mode. This is a separate
encoder implementation and stream version: Tiny Encoder emits legacy `0x405`
blocks while FFmpeg 9.0.2 emits `0x410` blocks. Rust decodes both streams
exactly against the original generated PCM; FFmpeg 9.0.2 also restores both
sources byte-for-byte. The 24-bit stream uses packed 3-byte samples with no
`INT32_INFO`, which adds independent evidence for that distinct profile.
The Tiny Encoder's default and high-quality compression modes use broader
decorrelation sets than fast mode. In addition to terms 17 and 18, the bounded
Rust decoder now supports cross-channel terms -2/-1 and same-channel lag terms
2 through 5, as exercised by the separate default- and high-mode regressions.
Other predictor terms remain rejected. The BSD-licensed C source and binary
were used only in a temporary directory outside the repository; neither is
part of this repository or any product dependency.

Reproduce from the repository root. The command downloads the official source
archive, whose SHA-256 is
`d2a0e6a52782d0d33fa89cf23784c6a4d625f8f64c124697c90679f10fbad5e8`:

```sh
repo_root="$(pwd)"
task_tmp="$(mktemp -d)"
trap 'rm -rf "$task_tmp"' EXIT
curl -fL https://www.wavpack.com/tiny_encoder.zip \
  -o "$task_tmp/tiny_encoder.zip"
echo 'd2a0e6a52782d0d33fa89cf23784c6a4d625f8f64c124697c90679f10fbad5e8  '"$task_tmp/tiny_encoder.zip" | shasum -a 256 -c -
unzip -oq "$task_tmp/tiny_encoder.zip" -d "$task_tmp/source"
cc -O2 -o "$task_tmp/tinypack" \
  "$task_tmp/source/tiny_encoder/tinypack.c" \
  "$task_tmp/source/tiny_encoder/wputils.c" \
  "$task_tmp/source/tiny_encoder/pack.c" \
  "$task_tmp/source/tiny_encoder/words.c" \
  "$task_tmp/source/tiny_encoder/bits.c"
export RUST_MUSIC_TMP="$task_tmp"
python3 - <<'PY'
from pathlib import Path
import os
import wave

fixtures = Path("crates/rust-music-wavpack/tests/fixtures")
task_tmp = Path(os.environ["RUST_MUSIC_TMP"])
for name, width in [("random-stereo16", 2), ("random-stereo24", 3)]:
    pcm = (fixtures / f"ffmpeg-synthetic-{name}.pcm").read_bytes()
    with wave.open(str(task_tmp / f"rust-music-tiny-{name}.wav"), "wb") as out:
        out.setnchannels(2)
        out.setsampwidth(width)
        out.setframerate(44100)
        out.writeframes(pcm)
PY
cd "$task_tmp"
./tinypack -f rust-music-tiny-random-stereo16.wav \
  tiny-encoder-4.40-random-stereo16.wv
./tinypack -f rust-music-tiny-random-stereo24.wav \
  tiny-encoder-4.40-random-stereo24.wv
cp tiny-encoder-4.40-random-stereo16.wv \
  "$repo_root/crates/rust-music-wavpack/tests/fixtures/"
cp tiny-encoder-4.40-random-stereo24.wv \
  "$repo_root/crates/rust-music-wavpack/tests/fixtures/"
ffmpeg -nostdin -v error -i tiny-encoder-4.40-random-stereo16.wv \
  -map 0:a:0 -c:a pcm_s16le -f s16le -y decoded16.raw
ffmpeg -nostdin -v error -i tiny-encoder-4.40-random-stereo24.wv \
  -map 0:a:0 -c:a pcm_s24le -f s24le -y decoded24.raw
cmp "$repo_root/crates/rust-music-wavpack/tests/fixtures/ffmpeg-synthetic-random-stereo16.pcm" decoded16.raw
cmp "$repo_root/crates/rust-music-wavpack/tests/fixtures/ffmpeg-synthetic-random-stereo24.pcm" decoded24.raw
```

Tiny Encoder 4.40 WavPack SHA-256 values:

- 16-bit: `0a7b48c427168dcdc1e2c67b6321e3b318839888f568a8758fcae7319fc76019`
- 24-bit: `4dea0dc5820f32c7f1a6bf0febc094ae2e5a2aeb7f9996234a91bc5c196bf5aa`

The FFmpeg-generated original PCM hashes listed above are shared goldens for
both independent encoder streams.

`tiny-encoder-4.40-random-default-stereo16.wv` is a 20,000-frame deterministic
stereo PCM stream encoded in Tiny Encoder 4.40's default pure-lossless mode
(without `-f`). Both WavPack blocks use terms `[18, 18, 2, 17, 3]`; the Rust
decoder matches all 40,000 interleaved samples and FFmpeg 9.0.2 independently
decodes the stream byte-for-byte to the same PCM. This validates terms 2 and 3
in one default-mode profile, not every WavPack predictor or quality mode.

Reproduce from the repository root with Python 3, a C compiler, FFmpeg 9.0.2,
and the official Tiny Encoder archive described above:

```sh
repo_root="$(pwd)"
task_tmp="$(mktemp -d)"
trap 'rm -rf "$task_tmp"' EXIT
curl -fL https://www.wavpack.com/tiny_encoder.zip \
  -o "$task_tmp/tiny_encoder.zip"
unzip -oq "$task_tmp/tiny_encoder.zip" -d "$task_tmp/source"
cc -O2 -o "$task_tmp/tinypack" \
  "$task_tmp/source/tiny_encoder/tinypack.c" \
  "$task_tmp/source/tiny_encoder/wputils.c" \
  "$task_tmp/source/tiny_encoder/pack.c" \
  "$task_tmp/source/tiny_encoder/words.c" \
  "$task_tmp/source/tiny_encoder/bits.c"
export RUST_MUSIC_TMP="$task_tmp"
python3 - <<'PY'
from pathlib import Path
import os
import random
import wave

task_tmp = Path(os.environ["RUST_MUSIC_TMP"])
random_source = random.Random(0x524D303134)
pcm = b"".join(
    random_source.randrange(-32768, 32768).to_bytes(2, "little", signed=True)
    for _ in range(20_000 * 2)
)
(task_tmp / "random-default-stereo16.pcm").write_bytes(pcm)
with wave.open(str(task_tmp / "random-default-stereo16.wav"), "wb") as output:
    output.setnchannels(2)
    output.setsampwidth(2)
    output.setframerate(44100)
    output.writeframes(pcm)
PY
cd "$task_tmp"
./tinypack random-default-stereo16.wav \
  tiny-encoder-4.40-random-default-stereo16.wv
ffmpeg -nostdin -v error -i tiny-encoder-4.40-random-default-stereo16.wv \
  -map 0:a:0 -c:a pcm_s16le -f s16le -y decoded.pcm
cmp random-default-stereo16.pcm decoded.pcm
cp tiny-encoder-4.40-random-default-stereo16.wv \
  "$repo_root/crates/rust-music-wavpack/tests/fixtures/"
cp random-default-stereo16.pcm \
  "$repo_root/crates/rust-music-wavpack/tests/fixtures/tiny-encoder-4.40-random-default-stereo16.pcm"
```

Default-mode WavPack SHA-256:
`551802997b9ee302a74d387c7dace173b9844ccedc950c112a65f2ecc788df0e`.
PCM SHA-256:
`f73bb995471938f0edc927a840f4de77e65da3f03a5e3e0e5a376eb6e79b7e35`.

`tiny-encoder-4.40-random-high-stereo16.wv` uses Tiny Encoder 4.40 high-quality
mode (`-h`) with the same 20,000-frame deterministic PCM golden. Its two
legacy `0x405` blocks use `[18,18,18,-2,2,3,5,-1,17,4]`, exercising both
cross-channel predictor directions and same-channel lags 2–5. Rust matches all
40,000 interleaved samples; FFmpeg 9.0.2 independently decodes the stream
byte-for-byte to the shared PCM golden. This is one encoder profile and does
not validate all WavPack high-quality streams or the other predictor terms.

Reproduce after building `tinypack` as described above, from the same temporary
directory containing `random-default-stereo16.wav` and its PCM golden:

```sh
./tinypack -h random-default-stereo16.wav \
  tiny-encoder-4.40-random-high-stereo16.wv
ffmpeg -nostdin -v error -i tiny-encoder-4.40-random-high-stereo16.wv \
  -map 0:a:0 -c:a pcm_s16le -f s16le -y high-decoded.pcm
cmp random-default-stereo16.pcm high-decoded.pcm
cp tiny-encoder-4.40-random-high-stereo16.wv \
  "$repo_root/crates/rust-music-wavpack/tests/fixtures/"
```

High-mode WavPack SHA-256:
`8db14f06ecb6c44d5282908908bf575a1d2d471ada391ac960cbc5fa71899cec`.
The shared PCM hash is
`f73bb995471938f0edc927a840f4de77e65da3f03a5e3e0e5a376eb6e79b7e35`.

`ffmpeg-synthetic-long-stereo24.wv` is a deterministic, pseudorandom
1,063,147-frame packed-24-bit stereo stress stream generated locally. Its PCM
source uses Python seed `0x525553544C4F4E47` and includes signed extrema and
small boundary values at the beginning. No recording or user audio is used.
Rust matches all 2,126,294 samples while decoding one block at a time; FFmpeg
9.0.2 independently decodes the stream byte-for-byte to the retained PCM.

Reproduce the source and WavPack stream with Python 3 and FFmpeg 9.0.2:

```sh
python3 - <<'PY'
from pathlib import Path
import random
import wave
rng = random.Random(0x525553544C4F4E47)
samples = [rng.randrange(-(1 << 23), 1 << 23) for _ in range(1_063_147 * 2)]
samples[:16] = [-(1 << 23), (1 << 23) - 1, -1, 0, 1, -2, 2, -32768,
                32767, 32768, -32769, 1234567, -7654321, 4194304,
                -4194304, 42]
pcm = b"".join(value.to_bytes(3, "little", signed=True) for value in samples)
Path("long-stereo24.pcm").write_bytes(pcm)
with wave.open("long-stereo24.wav", "wb") as output:
    output.setnchannels(2)
    output.setsampwidth(3)
    output.setframerate(44100)
    output.writeframes(pcm)
PY
ffmpeg -nostdin -v error -i long-stereo24.wav -c:a wavpack -y long-stereo24-tagged.wv
python3 - <<'PY'
from pathlib import Path
source = Path("long-stereo24-tagged.wv").read_bytes()
offset = 0
while offset + 32 <= len(source) and source[offset:offset + 4] == b"wvpk":
    size = int.from_bytes(source[offset + 4:offset + 8], "little") + 8
    if size < 32 or offset + size > len(source):
        raise SystemExit("invalid WavPack block size")
    offset += size
if offset != len(source) and source[offset:offset + 8] != b"APETAGEX":
    raise SystemExit("unexpected WavPack trailer")
Path("ffmpeg-synthetic-long-stereo24.wv").write_bytes(source[:offset])
Path("ffmpeg-synthetic-long-stereo24.pcm").write_bytes(
    Path("long-stereo24.pcm").read_bytes())
PY
ffmpeg -nostdin -v error -i ffmpeg-synthetic-long-stereo24.wv \
  -map 0:a:0 -c:a pcm_s24le -f s24le -y decoded.pcm
cmp long-stereo24.pcm decoded.pcm
```

WavPack SHA-256:
`da1d6d7fcb40179f4e3b10f3e9c55a14f844a99682937ea1e07a2697be31fb1b`.
Packed PCM SHA-256:
`712fbd88aabc7b33ca1524a06426320e23e92455e8d2e45084ad0d1ff39fcdfe`.
The source WAV, temporary tagged stream and generated intermediates are not
checked in.

`ffmpeg-synthetic-stereo16.wv` is a 20 ms generated two-tone stereo PCM stream,
encoded locally with FFmpeg's WavPack encoder. The test parses its real stereo
block metadata and entropy-variable values, including distinct initial channel
medians, then traverses all 1,764 residual words. SHA-256:
`1abae79d2a9ab2293d8700f161b5a332f8e2c9e866f03a151fe4ea7b87e141ff`.

Reproduction commands:

```sh
ffmpeg -nostdin -v error -f lavfi \
  -i 'aevalsrc=0.7*sin(2*PI*997*t)|0.5*sin(2*PI*439*t):s=44100:d=0.02' \
  -c:a pcm_s16le -y stereo.wav
ffmpeg -nostdin -v error -i stereo.wav -c:a wavpack -y ffmpeg-synthetic-stereo16.wv
```

`ffprobe` reports stereo, 44,100 Hz, 16-bit raw samples, and 882 frames.
The stereo regression reconstructs and compares all 1,764 interleaved integer
PCM samples against `ffmpeg-synthetic-stereo16.pcm`, then checks the WavPack
header checksum against those decoded samples and rejects a mutated sample.
This is one generated joint-stereo profile only; together with the mono vector
it does not establish broad encoder compatibility, real-world files, true-
stereo or stereo multiblock decoding, or hybrid correction. All files are
synthetic conformance inputs; FFmpeg is optional oracle tooling and is not a
runtime or Cargo dependency.
