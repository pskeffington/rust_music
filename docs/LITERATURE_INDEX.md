# Rust Music literature navigation

Updated: 2026-10-29. See [matrix](LITERATURE_MATRIX.md) and [JSON index](literature-index.json).

## Codec, container, and native API standards

- **LIT-001** [RFC 9639: FLAC](https://www.rfc-editor.org/rfc/rfc9639.html) - Implement native FLAC frames, metadata, CRC, Rice, fixed and LPC, channel reconstruction. Gate G2.
- **LIT-009** [Apple ALAC reference and cookie](https://github.com/macosforge/alac) - Design independent ALAC prediction and CAF/MP4 demux; do not link or copy the reference. Gate G4.
- **LIT-010** [Apple AudioQueueNewOutput](https://developer.apple.com/documentation/audiotoolbox/audioqueuenewoutput) - Thin native PCM output ABI; decode on worker, not playback callback. Gate G3.
- **LIT-011** [Windows WASAPI](https://learn.microsoft.com/en-us/windows/win32/coreaudio/wasapi) - Thin Windows audio endpoint adapter with no resampling surprises. Gate G3.
- **LIT-012** [WASAPI exclusive streams](https://learn.microsoft.com/en-us/windows/win32/coreaudio/exclusive-mode-streams) - Prefer capability negotiation, warn before exclusive device seizure. Gate G3.
- **LIT-013** [Android NDK AAudio](https://developer.android.com/ndk/guides/audio/aaudio/aaudio) - Thin Android output ABI for deployable micro player. Gate G5.
- **LIT-018** [Linux ALSA kernel PCM UAPI](https://github.com/torvalds/linux/blob/master/include/uapi/sound/asound.h) - Use local Rust ioctl structs against /dev/snd device nodes, not the libasound sound library or a daemon. Gate G3.

## Comparison decoders and conformance tools (never dependencies)

- **LIT-002** [IETF FLAC decoder testbench](https://github.com/ietf-wg-cellar/flac-test-files) - Use subset/uncommon/faulty cases, including rejection and hang-resistance. Gate G2.
- **LIT-003** [Xiph FLAC reference](https://github.com/xiph/flac) - Differential oracle only; do not bundle or copy decoder code. Gate G2.
- **LIT-004** [Claxon pure Rust FLAC](https://github.com/ruuda/claxon) - Study how small a lossless Rust decoder can be; compare performance methodology. Gate G2.
- **LIT-005** [Symphonia audio](https://github.com/pdeljanov/Symphonia) - Feature-controlled decoder comparison; explicitly excluded from runtime. Gate G1.

## Comparable music players and product architecture

- **LIT-006** [Music Player Daemon](https://github.com/MusicPlayerDaemon/MPD) - Separate codec, music database, control, and physical outputs. Gate G1.
- **LIT-007** [Termusic](https://github.com/tramhao/termusic) - Study keyboard-first playlists and compact status, not dependency surface. Gate G5.
- **LIT-008** [Firefly TUI music player](https://github.com/ly-ns0/firefly) - Contrast compact transport controls and library paths. Gate G5.

## Rust code quality, portability, and security

- **LIT-014** [MPD ALSA output guidance](https://mpd.readthedocs.io/en/stable/plugins.html) - Avoid implicit dmix/resample for strict audio; request direct hw device explicitly. Gate G3.
- **LIT-015** [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) - Typed errors and portable Read-driven decoders with explicit no-panics. Gate G0.
- **LIT-016** [Rust cargo features](https://doc.rust-lang.org/cargo/reference/features.html) - Keep optional OS backends gated and dependency-free core default. Gate G0.
- **LIT-019** [Cargo offline and locked builds](https://doc.rust-lang.org/cargo/reference/config.html#netoffline) - Require offline locked workspace builds with no registry/git packages and no build.rs hooks. Gate G0.

## Embedding boundary with Rust Boy and sibling modules

- **LIT-017** [Rust Radio standalone embedding](https://github.com/pskeffington/rust_radio/blob/main/docs/EMBEDDING.md) - Reuse module shape: independent lib, explicit Play, host-owned storage and output focus. Gate G5.

