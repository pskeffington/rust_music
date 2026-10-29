# rust_music native decoder and audio architecture

## Charter

Rust Music is a self-contained, locally developed lossless audio module for Rust_Boy, and also a standalone player. Its codec implementation must not rely on or wrap Symphonia, Claxon, FFmpeg, libFLAC, Rodio, CPAL, or another player/decoder. The only external audio boundary is the operating system's supported PCM device API.

The source tree is MIT licensed. Specifications and comparison projects are cited in the machine literature matrix but not incorporated or copied.

## Crate map

```text
rust-music-core        integer PCM, decoder/sink traits, typed errors, null test sink
rust-music-flac        native RFC 9639 decoder: STREAMINFO, CRC, fixed/LPC, Rice, channel stereo
rust-music-wave        native PCM RIFF/WAVE parser and bounded frame decoder
rust-music-engine      explicit Load -> Ready -> Play -> Step -> Finish/Stop state machine
rust-music-native      optional own AudioQueue macOS and ALSA Linux PCM API shims
rust-music             public façade: content sniffing + host integration exports
rust-music-cli         inspect / convert back to PCM WAVE / optional native output
```

**State:** `load()` never auto-opens an output device. `play()` must only occur after Rust_Boy grants audio focus; `step()` must execute off the UI thread. Music and Radio/TV must not claim devices simultaneously. The shell has one slot for a Music module; there are no menu sidebars, background scanners, or network requirements.

## Data plane

```text
explicit local File open
  -> bounded container parser
  -> own FLAC/WAVE decoder on non-real-time worker
  -> interleaved signed i32 exact integer PCM blocks
  -> optional local output sink (platform API, no codec)
  -> hardware / USB DAC
```

Decoder blocks preserve original signed sample values. 8-bit unsigned WAVE is normalized to centered signed integer PCM and converted back on output. All shifts/predictions happen with larger integer intermediates to prevent overflow. A result is not declared hardware bit-perfect until actual DAC sample rate/width, volume and signal-path conversion are verified. Metadata CRC is checked before committing a frame to the output sink.

## FLAC implementation boundary

- Native FLAC marker and exact STREAMINFO block check, unknown metadata bounded/stream-skipped.
- Frame header CRC8, frame footer CRC16 and zero pad validation; stereo assignment modes and up to eight channels.
- Rice-4/Rice-5, escaped residuals, constant/verbatim, fixed order 0-4, LPC order 1-32, wasted bits.
- Bounded 8 MiB frame, 16 MiB metadata block, 128 metadata headers, 1 Mi unary cap; invalid input errors instead of silent fallback.
- Source `total_samples` and sequential frame/sample numbers checked; nonzero STREAMINFO MD5 is verified against the complete signed interleaved decoded PCM at EOF. An all-zero STREAMINFO MD5 means **unknown**, not verified. Seektable random access remains an outstanding gate.
- STREAMINFO's MD5 detects accidental mismatch, not malicious tampering or provenance. CRC16 protects each frame before samples are offered to the sink; complete-file MD5 validation only occurs at EOF, so physical playback may already have started before a file-level mismatch is discovered. A strict host should preflight the full file before granting audio focus.
- Metadata is bounded both per-block (16 MiB) and in aggregate (64 MiB). Frame/sample number continuity and canonical encoded numbers are checked.
- No Ogg FLAC mapping at this stage; no MP4/M4A or ALAC marked supported.

## Output adapters and capability honesty

Mac AudioQueue and the Linux ALSA **kernel ioctl UAPI** are optional app-owned device shims, not decoding libraries. No libasound or daemon library is linked. Mac's system audio mixer or ALSA default plugin might resample even when Rust Music returns mathematically exact PCM. `hw:CARD,DEVICE` maps directly to `/dev/snd/pcmC*D*p`; no default ALSA plugin, libasound, or sound server is used. This is 64-bit Linux only and still needs actual hardware validation. Native adapter compile/device performance is separate from codec parser tests. Windows WASAPI and Android AAudio are planned, not yet implemented.

## Micro-interface contract

The host asks for `File -> AudioFormat` and calls `Player::load`, `play`, `step`, `stop`, `status`. For Rust_Boy, provide a minimal panel showing `TRACK / ARTIST / 00:00 / PLAY-PAUSE / PREV-NEXT / AUDIO`, with one narrow settings affordance. A host retains screen and audio-focus ownership, local playlist storage, and platform file permissions. The `rust_music` crate never launches a new window, browser, socket, server, or independent mixer.

## Non-goals in G0-G2

- Streaming, DRM, licensed-store media, ads, or trackers.
- Automatic network library scanning, microphone permissions, and cloud account code.
- Synthetic upsampling, equalization, gain correction or crossfade in strict listening mode.
- Native encoding of new FLAC or ALAC files; decoding is first priority.

## Local-only build and FFI policy

Every crate in the workspace has locally authored source. `Cargo.lock` carries no registry or Git packages. The self-authored `rust-music-audit` crate checks every path edge, build hook, native link and network API; `scripts/offline-gate.sh` builds the full workspace in Cargo offline+locked mode. See [Dependency policy](DEPENDENCY_POLICY.md). CI is optional and uses an operator-provisioned offline runner; it is not required to build or run the player.

## Research provenance

[Canonical source matrix](LITERATURE_MATRIX.md), [navigable index](LITERATURE_INDEX.md), [machine registry](literature-index.json), [phased roadmap](ROADMAP.md).
