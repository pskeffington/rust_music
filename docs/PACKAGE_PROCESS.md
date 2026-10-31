# Rust Music package-process contract

This document defines the obligations of the Rust Music executable when it is
run as a Pip-Boy package unit. The matching host protocol specification is
Pip-Boy's `docs/PACKAGE_PROCESS_PROTOCOL.md`; this file records the unit-side
contract and its evidence boundaries.

## Package identity and transport

- Stable package ID: `org.rustboy.music`. It remains distinct from Radio, TV,
  and every other module.
- Launch `rust-music-cli serve` as a child process. stdin and stdout carry only
  RBPK frames; diagnostics belong on stderr. The endpoint uses local stdio and
  introduces no socket, runtime network, or external codec dependency.
- A frame is `RBPK` + a 4-byte unsigned big-endian payload length + payload.
  Payload length is 1–16,384 bytes and is checked before allocation.
- Payloads are version 1, strict UTF-8, typed requests/replies. Package IDs,
  request IDs, message tags, field sizes and trailing bytes are validated.
  Hello is first with request ID 1; each later request ID increments by one.
  The synchronous channel permits one outstanding request at a time.
- The parent must enforce wall-clock deadlines around blocking reads and
  writes. Frame bounds do not bound I/O time. A timeout must terminate and
  reap the child before the host treats its resources as released.

## Process lifecycle and failure rules

The unit process has one serialized request/reply stream and one playback
worker. The worker queue holds at most one request and one reply; the endpoint
does not accept pipelined requests. The host sends the next request only after
receiving the prior reply. This keeps in-flight protocol state and retained
request data bounded.

| Phase | Accepted input | Required unit behavior |
| --- | --- | --- |
| Startup | `Hello` only, request ID 1 | Validate identity and version; reply with the selected version before accepting commands. Do not create an audio sink or scan files. |
| Ready | One request at a time, consecutive IDs | Validate each complete frame and request before dispatch. Return one reply with the same ID. |
| Shutdown | `Shutdown` with the next ID | Stop playback, release unit-local playback state, send `Ack`, then exit normally. |
| Fault / EOF | Malformed, truncated, oversized, out-of-order input, transport failure, or stdin EOF | Do not dispatch an invalid request. Stop playback and exit; do not attempt protocol recovery on the same stream. |

The endpoint rejects a bad handshake, identity, sequence, frame, or payload by
terminating the protocol session. It does not send an uncorrelated error frame
for input it cannot safely decode. A valid but unsupported or denied command
may receive a typed `Error` reply. After a transport fault, the host must treat
the child as failed, terminate it if it has not exited, and reap it before
releasing the package slot or shared audio reservation.

The unit enforces byte, count, and catalog-depth bounds locally. It cannot
enforce a hard wall-clock limit on blocking stdio or a native device call from
inside this synchronous protocol. The host supervisor owns transaction and
shutdown deadlines, kill/reap behavior, and process-exit cleanup. Killing the
child is a fault-containment action, not an in-process graceful shutdown or an
OS sandbox guarantee.

## Requests and side effects

| Request | Unit behavior |
| --- | --- |
| `Hello` | Negotiate RBPK v1 before any other request. |
| `Activate`, `Resume`, `Resize`, `Snapshot` | Return bounded display state; do not scan or start playback. |
| `ScanLibrary` | Scan only the explicitly supplied absolute root, after the host has separately authorized the request. The path is never returned in a view. |
| `Select`, `Previous`, `Next` | Change selection without autoplay. |
| `Play` | Accept only as an explicit host-authorized user intent. Output is created lazily on this path. |
| `Stop`, `Suspend`, `Shutdown`, EOF | Stop playback and release the unit's local playback state; acknowledge Shutdown before normal exit. |

The host owns package identity, permissions, foreground selection and the
shared Radio/Music/TV audio-focus claim. A valid `Play` frame is not permission
and a unit-local `audio_claimed` view bit is not proof of host ownership. The
host must gate Play, keep its claim held until stop/exit is confirmed, and
release any reservation if startup fails. The package process is fault
containment, not an OS sandbox; the child otherwise runs with the host user's
OS permissions.

## Fixed limits

| Field | Bound |
| --- | ---: |
| Frame payload | 16,384 bytes |
| Package ID | 96 bytes |
| Explicit library root | 4,096 bytes |
| Total catalog entries | 2,048 |
| Entries in one view | 32 |
| Display title | 128 bytes |
| Error text | 256 bytes |
| Requests in flight | 1 |
| Library scan | 8,192 examined entries, 2,048 retained tracks, depth 6 |
| Audio worker queue | 1 request and 1 reply |

Views contain display text, container/PCM metadata and playback counters only;
they do not contain artwork, executable data, arbitrary markup, or filesystem
paths.

The library scan values are the Rust Music `ScanLimits::default()` used by the
unit endpoint. A scan may take time proportional to those bounds and local
filesystem behavior; hosts must keep it under their own request deadline.

## Evidence and release gate

`rust-music-ipc` owns the codec and frame bounds. `rust-music-cli serve` owns
the no-autoplay stdio endpoint. The shared hello vector, malformed/oversized
frames and strict identity/sequence checks are covered by local tests. The
macOS spawned-child tests prove hello, inert activation, shutdown and clean
exit, plus fail-closed behavior for an incorrect package identity, for the
Cargo-built executable. A separate local release-feature build
also completed the same RBPK lifecycle through
`scripts/test_release_package_process.py`; its digest and scope are recorded
in `docs/G5_VALIDATION.md`. Neither result
proves host review/allowlisting of that digest, live Pip-Boy mounting, shared
audio contention, or platform process behavior. RM-018 remains in progress
until those host gates are observed.
