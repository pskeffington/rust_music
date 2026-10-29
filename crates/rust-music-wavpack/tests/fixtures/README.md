# WavPack parser fixture

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

The Rust test parses the first complete audio block and consumes every bounded
metadata sub-block. It does not decode or compare PCM; WavPack remains
unsupported by the Rust Music playback facade.
