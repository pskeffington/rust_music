# Rust Music for macOS

This is Rust Music's first standalone native desktop interface. The SwiftUI
window is a thin local UI shell; `rust-music-cli` performs file inspection,
decoding, and AudioQueue output. This keeps the decoder and PCM output adapter
in Rust and lets the embedded Pip-Boy module continue using `MusicSlot` and its
host-provided shared audio-focus lease.

The interface uses a compact, text-only phosphor-green player panel. It is
designed to sit inside Pip-Boy's outer wrapper, so the app does not draw a
second chassis, hardware bezel, mascot, or visualizer. A narrow navigation rail
switches between Player, Library, Queue, and Playlists; transport and volume
stay visible in a dedicated bottom bar. It remains Rust Music's own window and
module identity; the host owns module switching, shared audio focus, and
Settings when embedded.

The app lets the operator select a local library folder, runs a bounded
cancellable catalog scan only on that request, and supports text search,
named playlists, and a persistent queue with repeat and shuffle. Play, Pause,
Resume, Stop, Next, Previous, and volume use the Rust CLI package protocol and
native macOS AudioQueue output. Queue and playlist selection do not start
playback. AAC/MP3 and unsupported profiles are rejected by the Rust decoder.

Build from the repository root on macOS:

```sh
sh scripts/build-macos-app.sh
```

The app bundle is created under `target/macos/RustMusic.app`; Cargo and Swift
use only the installed local toolchains and Apple frameworks. No music files
are bundled. The UI contains text and native vector rules only; it has no
artwork, image, waveform, or equalizer visualization subsystem. Digital volume
attenuation is not hardware gain or bit-perfect output.

The local build script removes a stale generated code-resource seal, compiles
for the declared macOS 13.0 minimum, ad-hoc signs the completed bundle, and
verifies its signature. Ad-hoc signing supports local LaunchServices testing;
it is not Developer ID signing, notarization, or distribution Gatekeeper
acceptance.

The package process is serialized behind a private app queue. Window close
sends Stop and Shutdown, closes its pipes, waits for exit, then escalates
through terminate and kill with a reap. Request deadlines also terminate the
unresponsive child. Simulated AudioQueue failure/teardown regressions and
LaunchServices startup have local evidence; audible device disconnect and
recovery, DAC output, Rust Boy shared-focus contention, and Developer ID
distribution signing still require separate acceptance.
