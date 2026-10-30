# Rust Music in Pip-Boy's universal package host

This is the Rust Music side of the integration contract. It describes how an
independent Rust Music build can be hosted by Pip-Boy without taking ownership
of the shell, Settings, or the shared audio device. It does not claim that a
Pip-Boy adapter is already implemented or hardware-tested.

Pip-Boy commit `bccd59d` now includes `org.rustboy.music` as a distinct
reference manifest in its universal package catalog. The entry is explicitly
`design_fixture_not_installed`; this is package identity/registry evidence,
not a mounted adapter or playable module.

## Universal dock and Radio replacement model

Treat Pip-Boy's universal package host as the **dock/container**, not as a
module merger. Every module keeps its own stable identity, package metadata,
state, controls, and lifecycle; Rust Music, Radio, and TV remain distinct
packages even when the container changes which one is foreground. Rust Music
is a replaceable package that enters the shared foreground slot, while the
shell continues to own device access, Settings, navigation, and package
lifecycle. Rust Music must have its own stable package identity (for example,
`org.rustboy.music`) and must not impersonate or overwrite Radio's identity.

For the common “replace Radio with Music” operation, install/register both
trusted packages, then select Music in the one-foreground host. The host
suspends Radio before activating/resuming Music; selecting Radio later reverses
the handoff. This is foreground replacement, not destructive uninstall or
registry replacement. The inspected v1 host can select among already-mounted
trusted built-ins, but has no dynamic external module loader or general
`replace()` API. A truly swappable third-party module requires a separately
reviewed package loader/IPC boundary and is outside this adapter-ready contract.

The current built-in Radio is a passive catalogue and does not expose playback
intents. A useful Music-for-Radio dock still requires a trusted runtime adapter,
user-facing Play/Stop/navigation actions, worker scheduling, and a bridge from
Pip-Boy's shared Radio/TV audio arbitration to `AudioFocus`. Keep the package
identity and adapter separate so Music can coexist with Radio and the operator
can switch between them safely.

## Keep the product crate independent

`rust-music-host` exposes the framework-free `MusicSlot`, typed commands,
read-only playback snapshots, and the `AudioFocus` / RAII `AudioLease`
boundary. `rust_music` must remain buildable offline with workspace-local
dependencies; it should not depend on Pip-Boy's SDK or import Pip-Boy internals.
The thin `UniversalPackageAdapter` wrapper belongs on the host side and should
depend on a reviewed, explicitly pinned SDK revision.

Pip-Boy's current v1 SDK is documented as a development preview, not a stable
Rust ABI, sandbox, or external package loader. Its package view is bounded,
inert display data (`heading`, `text`, `status`, and `list` blocks). The
128x64/1024-byte `rust-music-panel` framebuffer is a separate renderer for
hosts that expose that exact display surface; it is not a `PackageView`.

## Required wrapper behavior

| Pip-Boy lifecycle or intent | Rust Music behavior |
| --- | --- |
| `Activate`, `Resume`, `Resize` | Return a view from the current snapshot. Do not scan a directory, load a track, acquire focus, or start output. |
| Authorized `play` intent with `media.audio` | Call `slot.handle(Command::Play, &audio_focus_bridge)`. `MusicSlot` loads first, then obtains its sole RAII lease through `AudioFocus::try_acquire_music()`, then starts output. If loading fails before lease acquisition, the host must release its pre-authorized media reservation when the intent returns an error. |
| Authorized `stop` intent with `media.audio` | Issue `Command::Stop`; release focus even if native drain/close returns an error. |
| `Suspend` / pop-out | Call `slot.suspend()`: stop output and release the lease while preserving the mounted Music instance, catalog, and selection. Popping back in does not autoplay; require a fresh Play intent. |
| `Shutdown`, explicit unmount, adapter drop | Drop only the Music adapter/slot instance, stopping playback and releasing any lease. Do not remove or overwrite Radio or another module. |
| Decoder `Poll` | Schedule on a worker, one bounded block at a time. Never run it on the UI/render thread or native audio callback. |
| `Next`, `Previous`, `Select` | Deliver only through a host-reviewed user input path. They stop the current track and never autoplay. |
| Settings | Ask the shell to open its existing Settings screen; Rust Music does not render a Settings substitute. |

The wrapper must map a snapshot to bounded plain text, such as filename,
container, PCM format, playback state, and sample-derived elapsed time. The
catalog exposes only bounded, validated FLAC/AIFF text tags and sanitized title
fallbacks; it does not expose artwork. `MusicSlot` supports FLAC frame seek via
`Command::SeekFrame` when a declared frame count and valid seek data are
available. A successful seek drains output, releases focus, and leaves the slot
ready for an explicit Play. The panel currently has no seek control, so the
host must not expose seek until it maps reviewed user input to that command.
Pause/resume remains unsupported.

## Audio ownership boundary

The Pip-Boy host must arbitrate Music against Radio and TV as one device owner.
Its current `media.audio` intent policy accepts only `play`, `resume`, `pause`,
and `stop`; Music should expose only operations it implements. Authorization
and the `MusicSlot` lease must form one ownership path: deny Play before a DAC
opens, release a pending grant when startup fails, and release the active owner
on Stop, EOF, decoder/output failure, Suspend, Shutdown, and Drop. Do not add a
second music-only mutex that can disagree with the host's owner.

The adapter boundary needs a host-side lease bridge: Pip-Boy's authorization
policy is not itself an `AudioLease`. Do not acquire a claim separately in the
wrapper and then pass it to `MusicSlot`; that would create two acquisition
paths. After the host authorizes Play, the wrapper passes the bridge to
`MusicSlot::handle`, and the bridge's `try_acquire_music()` performs the one
lease transfer. Its RAII token releases that same host-owned claim on EOF,
output/decoder errors, Stop, or drop. The host must also release a pre-authorized
reservation when loading fails before `MusicSlot` calls the bridge, so failed
Play cannot strand the package as media owner.

## Current compatibility gaps

- The v1 `PackageView` is display-only. The inspected host documentation says
  package input actions are deferred, so Play/Stop/track-navigation controls
  are not yet wired from the package view to `Intent`.
- The host's audio policy has play/stop vocabulary, but there is no verified
  adapter that bridges it to `AudioFocus` and handles failed start/stop paths.
- `MusicSlot::Poll` requires a worker scheduler; the synchronous adapter trait
  does not by itself provide one.
- Pip-Boy has a Music reference manifest, but no trusted Music adapter is
  installed or mounted. No Radio/TV/Music interoperability or physical
  display/audio test is recorded.
- The host's current trusted built-in bootstrap is not a universal third-party
  package loader; module foreground switching is supported conceptually, but
  dynamic install, unload-and-replace, and external isolation are not.

These gaps require host-side implementation and acceptance. Until that work is
authorized and complete, Rust Music is **adapter-ready at its public host API**
but is not installed or production-integrated in Pip-Boy. See the
[host slot contract](HOST_CONTRACT.md) and [G5 validation ledger](G5_VALIDATION.md).
