# Rust Music independent-codec literature matrix

Updated: 2026-10-29

All sources are research references **only**. No third-party decoder/player implementation is copied or linked.
Project claims are not a hardware benchmark; status is recorded in the registry.

[Machine-readable registry](literature-index.json) | [Navigation](LITERATURE_INDEX.md) | [Architecture](ARCHITECTURE.md) | [Roadmap index](roadmap-index.json)

## Engineering transfer matrix

| ID | Source | Decision | Validation | Gate | License boundary |
| --- | --- | --- | --- | --- | --- |
| **LIT-001** | [RFC 9639: FLAC](https://www.rfc-editor.org/rfc/rfc9639.html) | Implement native FLAC frames, metadata, CRC, Rice, fixed and LPC, channel reconstruction. | Decode/CRC tests and IETF corpus; verify PCM against reference. | G2 | IETF standards text; write our own decoder |
| **LIT-002** | [IETF FLAC decoder testbench](https://github.com/ietf-wg-cellar/flac-test-files) | Use subset/uncommon/faulty cases, including rejection and hang-resistance. | Run testbench, compare PCM/STREAMINFO MD5 and record pass counts. | G2 | CC0 test corpus; fixture provenance tracked |
| **LIT-003** | [Xiph FLAC reference](https://github.com/xiph/flac) | Differential oracle only; do not bundle or copy decoder code. | Compare PCM and error behavior with pinned reference version. | G2 | Reference libraries BSD-like; tools GPL; no code reuse |
| **LIT-004** | [Claxon pure Rust FLAC](https://github.com/ruuda/claxon) | Study how small a lossless Rust decoder can be; compare performance methodology. | Benchmark decoded samples/s on same hardware with a documented corpus. | G2 | Apache-2.0; architecture only; do not copy |
| **LIT-005** | [Symphonia audio](https://github.com/pdeljanov/Symphonia) | Feature-controlled decoder comparison; explicitly excluded from runtime. | No Symphonia dependency in cargo metadata or lockfile. | G1 | Open source; design reference, not a dependency |
| **LIT-006** | [Music Player Daemon](https://github.com/MusicPlayerDaemon/MPD) | Separate codec, music database, control, and physical outputs. | Each module builds without UI or native audio libs. | G1 | GPL project; do not transplant code |
| **LIT-007** | [Termusic](https://github.com/tramhao/termusic) | Study keyboard-first playlists and compact status, not dependency surface. | Micro controls tested at small display sizes. | G5 | Mixed MIT/GPL code; behavior-only study |
| **LIT-008** | [Firefly TUI music player](https://github.com/ly-ns0/firefly) | Contrast compact transport controls and library paths. | Compare startup/memory with equivalent local file library. | G5 | Review upstream license; no code reuse |
| **LIT-009** | [Apple ALAC reference and cookie](https://github.com/macosforge/alac) | Design independent ALAC prediction and CAF/MP4 demux; do not link or copy the reference. | Differential ALAC PCM golden vectors and malformed MP4 atoms. | G4 | Apache-2.0 reference; independent implementation planned |
| **LIT-010** | [Apple AudioQueueNewOutput](https://developer.apple.com/documentation/audiotoolbox/audioqueuenewoutput) | Thin native PCM output ABI; decode on worker, not playback callback. | macOS signed PCM format negotiation, device swap and drain tests. | G3 | OS API; adapter code owned by Rust Music |
| **LIT-011** | [Windows WASAPI](https://learn.microsoft.com/en-us/windows/win32/coreaudio/wasapi) | Thin Windows audio endpoint adapter with no resampling surprises. | WASAPI shared/exclusive bit-match acceptance on device. | G3 | OS API; adapter planned |
| **LIT-012** | [WASAPI exclusive streams](https://learn.microsoft.com/en-us/windows/win32/coreaudio/exclusive-mode-streams) | Prefer capability negotiation, warn before exclusive device seizure. | Windows format capability and system-audio behavior tests. | G3 | Official platform documentation |
| **LIT-013** | [Android NDK AAudio](https://developer.android.com/ndk/guides/audio/aaudio/aaudio) | Thin Android output ABI for deployable micro player. | Actual Android 26+ format, focus, standby and output tests. | G5 | Android native API; adapter planned |
| **LIT-014** | [MPD ALSA output guidance](https://mpd.readthedocs.io/en/stable/plugins.html) | Avoid implicit dmix/resample for strict audio; request direct hw device explicitly. | Linux ALSA direct-device format/no-resample field verification. | G3 | Documentation/behavior reference |
| **LIT-015** | [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) | Typed errors and portable Read-driven decoders with explicit no-panics. | Fmt/clippy/test plus public rustdoc review. | G0 | Guidance, not a runtime dependency |
| **LIT-016** | [Rust cargo features](https://doc.rust-lang.org/cargo/reference/features.html) | Keep optional OS backends gated and dependency-free core default. | Cargo tree and MSRV target matrices. | G0 | Official Rust documentation |
| **LIT-017** | [Rust Radio standalone embedding](https://github.com/pskeffington/rust_radio/blob/main/docs/EMBEDDING.md) | Reuse module shape: independent lib, explicit Play, host-owned storage and output focus. | Rust Boy adapter can load and unload without second audio stream. | G5 | Sibling repository interface reference only |
| **LIT-018** | [Linux ALSA kernel PCM UAPI](https://github.com/torvalds/linux/blob/master/include/uapi/sound/asound.h) | Use local Rust ioctl structs against /dev/snd device nodes, not the libasound sound library or a daemon. | 64-bit ABI layout tests plus playback hardware negotiation and underrun testing. | G3 | Linux kernel interface, not a bundled decoder or userspace sound SDK |
| **LIT-019** | [Cargo offline and locked builds](https://doc.rust-lang.org/cargo/reference/config.html#netoffline) | Require offline locked workspace builds with no registry/git packages and no build.rs hooks. | Run the built-in Rust audit and offline build gate from disconnected workstation. | G0 | Rust toolchain itself is an explicit prerequisite; no dependencies are downloaded |
| **LIT-020** | [WavPack 4 and 5 binary file/block format](https://www.wavpack.com/WavPack5FileFormat.pdf) | Use the block header and metadata layout to implement bounded, original WavPack parsing; accept only independently verified lossless profiles. | Header/sub-block bounds, codec rejection, and independent integer PCM parity with missing/corrupt correction-stream cases. | G12 | Format documentation; all decoder code remains original Rust |

## Independent Rust coding standards

| ID | Rule | Gate | Implementation status | Evidence | Acceptance |
| --- | --- | --- | --- | --- | --- |
| **RUST-001** | No runtime decoder deps: do not link FLAC/ALAC, Symphonia, Claxon, FFmpeg, or libFLAC. | G0 | implemented | LIT-001, LIT-003, LIT-004, LIT-005 | cargo metadata proves decoder dependency closure is internal only |
| **RUST-002** | Preserve integer samples and explicit sample format; do not silently apply floats, DSP, gain or resampling. | G1 | implemented | LIT-001, LIT-014 | golden integer PCM bit patterns match source |
| **RUST-003** | Bound metadata, block sizes, allocation, Rice unary and file chunk scanning. | G2 | implemented | LIT-001, LIT-002 | malformed and oversize inputs fail with typed errors |
| **RUST-004** | Check frame header CRC8 and full-frame CRC16 before writing PCM. | G2 | implemented | LIT-001, LIT-002 | corruption test rejects changed CRC |
| **RUST-005** | Verify decoded PCM STREAMINFO MD5 across the complete file; distinguish zero digest. | G2 | implemented | LIT-001, LIT-002, LIT-003 | unit checksum vectors plus full-file reference PCM/MD5 parity on GitHub Actions |
| **RUST-006** | Constructors, metadata scanning and module registration never autoplay or open output devices. | G1 | implemented | LIT-006, LIT-017 | player state tests: Ready does not call sink.open |
| **RUST-007** | Rust Boy coordinates active TV, RADIO and MUSIC audio ownership; one active sink at a time. | G5 | planned | LIT-006, LIT-017 | integration tests demonstrate only one device owner |
| **RUST-008** | Do not label an output bit-perfect until hardware format/rate/channel path is verified. | G3 | planned | LIT-010, LIT-011, LIT-012, LIT-014 | record exact device format and loopback sample comparison |
| **RUST-009** | Audio callback is nonblocking, allocation-free, and free of filesystem work. | G3 | planned | LIT-010, LIT-013 | stress callback and underrun/fault insertion |
| **RUST-010** | Track selection and indexing never trigger network calls or scanning unrelated roots. | G5 | planned | LIT-006, LIT-017 | air-gapped tests and bounded explicit library scan |
| **RUST-011** | Test valid/uncommon/faulty FLAC and compare output to pinned reference. | G2 | planned | LIT-002, LIT-003 | corpus report with SHA, exact fixture counts and outcomes |
| **RUST-012** | Pure headless core compiles independent of macOS/Windows/Linux/Android OS backends. | G0 | implemented | LIT-010, LIT-011, LIT-013, LIT-016 | cross-target no-default-feature check |
| **RUST-013** | Every source and standard is uniquely identified with evidence and validation gates. | G0 | implemented | LIT-015 | scripts/check_literature.py succeeds offline |
| **RUST-014** | Do not mark ALAC as supported until demux and decoder pass corpus comparison. | G4 | planned | LIT-009 | ALAC source/PCM signed comparison with golden tests |
| **RUST-015** | Every Cargo dependency stays inside this workspace; reject crates.io, git, build.rs and undeclared native libraries. | G0 | implemented | LIT-016, LIT-019 | cargo run --locked --offline -p rust-music-audit proves manifest, lock and FFI boundaries |
| **RUST-016** | Do not link libasound, PulseAudio, JACK or PipeWire; Linux PCM sink must use only a /dev/snd kernel device. | G3 | implemented | LIT-014, LIT-018 | Feature-gated native ABI size checks, real PCM device setup and DAC loopback evidence |
