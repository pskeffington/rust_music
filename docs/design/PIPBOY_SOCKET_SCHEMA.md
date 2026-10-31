# RM-SOCKET-01 — Portable MUSIC surface and Pip-Boy socket schema
Status: architecture adopted; **implementation pending**. Date: 2026-10-31.
Primary UI: [PRIMARY_UI_DIRECTION.md](PRIMARY_UI_DIRECTION.md).
Existing host: [HOST_CONTRACT.md](../HOST_CONTRACT.md); micro panel: [G6_MICRO_PANEL.md](../G6_MICRO_PANEL.md).
Release precedence: [CODEX_MACOS_V1_DELIVERABLE.md](../CODEX_MACOS_V1_DELIVERABLE.md).

## Decision

One Rust music engine, one authoritative session, two (or more) *projections*.
The visual reference is a **Pip-Boy mountable panel/shell design**: its bronze
chassis and phosphor interior are owned by the presentation/host; Music owns
only its content/state. No nested full-size frame inside an existing Pip-Boy
frame. Host chooses available space and device tier.

Do not duplicate decoders, queue, playback state, native audio session, or audio
focus to implement a miniature player. Desktop SwiftUI is an adapter. Pip-Boy
is a separate adapter to the same Rust domain commands and snapshots.

### Proposed layered modules (not currently verified as implemented)

```text
                native Rust lossless decoder + player
                                  |
                         rust-music-host
                    commands / audio focus / state
                                  |
                      bounded read-only projection
                          /             \
                  desktop adapter     RBPK MUSIC slot
                  native macOS UI     Pip-Boy host
                   full / mini      normal / 128x64
                                           |
                                 host chrome + host settings
                                 host audio focus, RADIO/TV
```

Existing code's `rust-music-ipc` RBPK v1 and `rust-music-panel` are authoritative
integration starting points. Inspect implementations and avoid second protocols.

## Socket descriptor v1 (proposed schema)

| Field | Contract |
| --- | --- |
| `package_id` | stable `org.rustboy.music` |
| `schema_version` | integer v1; explicit incompatibility handling |
| `slot_id` | host-defined instance identifier |
| `category` | MUSIC, distinct from RADIO and TV |
| `capabilities` | negotiated bitset, never assume pause, seek, volume, playlists, devices |
| `lifecycle` | discovered -> mounted -> inactive -> active -> suspended -> unmounted |
| `surface_tier` | desktop-full, desktop-mini, pipboy-panel, micro-128x64 |
| `audio_focus` | lease **only** from host shared arbiter |
| `storage` | host-authorized user local paths; no scan at mount |
| `input` | host-translated named actions, no raw button coupling |
| `settings` | host-owned navigation + music-scoped preferences where available |
| `resource_budget` | negotiated frame, memory, input and refresh limits |

Do not change deployed RBPK wire fields without backward compatibility,
versioned negotiation and synthetic roundtrip fixtures.

## Proposed bounded read-only MusicViewSnapshot v1

- `generation: u64`: monotonically changes after meaningful authoritative
  state mutations; lets adapters ignore stale responses.
- `selected_track_id: Option<TrackId>`, `playing_track_id: Option<TrackId>`:
  separate *selection* from *actual stream*.
- `playback: Stopped|Ready|Playing|Paused|Error`; `Paused` must be advertised
  only if real backend semantics exist.
- `media: Option<Metadata>`: bounded display title, artist, album,
  container, codec, channels, sample rate, integer bit depth, only known values.
- `position_frames: Option<u64>`, `total_frames: Option<u64>`: omit unknown
  duration; the UI must never invent a seekable timeline.
- `queue: BoundedPage<TrackRow>`, `library: BoundedPage<TrackRow>`: paginated
  text-only result sets; request limits and stable cursors.
- `mode`: normal/repeat/shuffle only when functional.
- `can`: `play`, `pause`, `stop`, `next`, `previous`, `seek`, `volume`,
  `playlist_edit`, `select_device`, `eq`, `crossfade`. Each reflects actual
  backend support, audio focus and effective policy.
- `device`: optional host-resolved output display name; never expose device
  enumeration as an unconditional capability.
- `error`: bounded user-safe code/message; no absolute private path.
- `curie`: optional presentation cue (`idle|playing|paused|error`) derived
  from state, never a required engine dependency.

Every string, vector, page, and envelope is bounded; no user audio bytes or
artwork transmitted in status frames. Do not expose Rust structs as an assumed
stable ABI; map to existing versioned RBPK messages.

## Commands and ownership

| UI intent | Domain action | Critical invariant |
| --- | --- | --- |
| Select track | Select(id) | Selection alone does not play |
| Play | Play | Explicit intent and exclusive host lease |
| Pause/Resume | Pause/Resume, if capability | Real native output control, no fabricated state |
| Stop | Stop | Idempotent; releases lease |
| Previous/Next | Previous/Next | Preserve existing selection-only contract unless explicit validated play-next queue mode is introduced and version-negotiated |
| Seek | SeekFrame(frame), if capability | Existing seek semantics; no imaginary scrub |
| Library import | ScanAuthorizedRoot | Explicit permission, bounded worker, no constructor scan |
| Edit queue/playlist | Typed mutations | Atomic persistence, stable identities and error handling |
| Volume | SetVolume bounded, if capability | Not silently treated as source bit-perfect |
| Settings | OpenHostSettings | Host route, not nested settings window |
| Unmount/Suspend | Host lifecycle | Release lease and worker resources; never resume automatically |

**Compatibility issue to resolve in code:** older host Previous/Next are
selection-only, while the standalone Mac v1.0 target expects real playback
transitions. Specify a separate explicit `AdvanceAndPlay` operation or a
well-tested, negotiated queue playback mode; do not silently change legacy
button behavior in Rust Boy.

## Presentation schema derived from approved image

1. **Shell**: host-owned outer Pip-Boy frame (desktop can draw an equivalent
   lightweight native border); intrinsic breakpoints, safe content bounds.
2. **Now Playing**: essential metadata, truthful transport, conditional progress.
3. **Playlist**: virtualized, paged rows and mutation affordances.
4. **Library**: explicit import, query and category filters backed by real tags.
5. **Settings**: working capabilities only, with disabled unavailable controls.
6. **Mini Player**: another view of the *same* session and queue, no second sink.
7. **Curie**: optional state-derived presentation overlay; omit on low-memory
   micro tier or reduced motion.

**Existing policy conflict:** AGENTS.md and the Mac v1.0 target explicitly
prohibit artwork, images, skins and visualizers. This design reference is a
**layout/color/hierarchy guide**, not permission to ship the reference image
as runtime artwork or to allocate a large ornamental skin. The bronze surround
can be rendered as lightweight native shapes after policy review; Curie image
assets/animation are a separately gated post-v1 feature requiring explicit
policy reconciliation. Do not remove zero-artwork policy inadvertently.

## Codex indexed passes

- **SOCK-01 (P0, spec/test)** Map actual RBPK v1, MusicSlot, macOS SwiftUI and
  G6 APIs to this projection. Publish an exact existing/new field ledger.
- **SOCK-02 (P0, implementation)** Create a local-crate, dependency-free
  bounded projection adapter with capability masks and versioned tests. No
  duplicate player or state machine.
- **SOCK-03 (P0, implementation)** Bind desktop now-playing/library/playlist
  transport to snapshots/commands. Preserve MAC-01 gates; test selection vs play.
- **SOCK-04 (P1, integration)** Pip-Boy socket descriptor + lifecycle mount,
  suspend, unmount; enforce single audio arbiter with Radio/TV; denied focus,
  EOF, rapid switching, failure and no-autorun fixtures.
- **SOCK-05 (P1, UI)** Reuse reference's content hierarchy in host panel;
  micro 128x64 retains exactly 1024-byte deterministic renderer. No nested chrome.
- **SOCK-06 (P2, optional)** Mini-player/Curie resource profile, accessibility,
  reduced motion and policy approval; do not block Mac v1.0 on decoration.
- **SOCK-07 (P0 for release, verification)** Offline gate and macOS device
  acceptance; live sibling Pip-Boy mounting remains unverified until tested.

### Acceptance contracts

Test snapshot determinism, stale generation handling, invalid/oversized fields,
pagination bounds, incompatible schema rejection, unavailable capability UI,
empty library, focus contention, device removal, lifecycle resource cleanup,
radio->music handoff, no side effects in render, no scan/autoplay on mount,
and micro frame byte size. Keep mocked/pure unit results separate from actual
macOS DAC and Pip-Boy device evidence.

One writer per overlapping scope, preserve local unpublished work. Inspect
`docs/roadmap-index.json` before changing existing RM statuses. On this
documentation pass, no source implementation or hardware test is claimed.
