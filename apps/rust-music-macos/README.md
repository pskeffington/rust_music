# Rust Music for macOS

This is Rust Music's first standalone native desktop interface. The SwiftUI
window is a thin local UI shell; `rust-music-cli` performs file inspection,
decoding, and AudioQueue output. This keeps the decoder and PCM output adapter
in Rust and lets the embedded Pip-Boy module continue using `MusicSlot` and its
host-provided shared audio-focus lease.

The interface uses a restrained phosphor-green palette aligned with Pip-Boy's
existing handheld shell. It remains Rust Music's own window and module identity;
the host owns module switching, shared audio focus, and Settings when embedded.

The app opens one file only after the operator chooses it. It validates the
file before enabling Play, never scans a directory in the background, and
never starts playback automatically. AAC/MP3 and unsupported profiles are
rejected by the Rust decoder.

Build from the repository root on macOS:

```sh
sh scripts/build-macos-app.sh
```

The app bundle is created under `target/macos/RustMusic.app`; Cargo and Swift
use only the installed local toolchains and Apple frameworks. No music files
are bundled. Pause, playlist persistence, artwork, and hardware bit-perfect
certification are not claimed.

App-managed playback uses the local `play-managed` CLI entry point and a
private stdin pipe. Stop and window close close the app's pipe writer; if the
app process is terminated, the operating system closes it. The Rust child
observes EOF, calls the player stop path, and exits. A five-second watchdog
exits the child if native output fails to return in time. The watchdog is a
last-resort process cleanup path and does not prove a graceful AudioQueue
drain/dispose on hardware.
