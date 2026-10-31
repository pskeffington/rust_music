# Codex engineering kickoff — library-load autoplay, EQ and Winamp-like text controls

**Status: proposed product requirements and acceptance gates, NOT implemented claims.** This instruction file was authored on a remote branch that may lag the operator's unpushed local checkout. Codex must integrate non-destructively; do not replace newer local AGENTS, roadmap or code with this remote snapshot.

## FIRST: agent_skills

Before mutable engineering work, perform a read-only Git safety inspection, then load and pin a verified development revision of `pskeffington/agent_skills` and read the applicable SKILL.md contracts. Use delivery orchestration for task sequencing, implementation/build integration for code, test verification for evidence, security/code review for independent checks and release readiness for acceptance. Pair with Rust Music's codec_engineer, audio_backend, host_integrator, index_curator and gate_verifier. One writer per overlapping file scope. Never add agent_skills as a runtime dependency.

## Product goal

Deliver a genuinely usable **ultra-lightweight, text-only, entirely Rust** player with Winamp-like library browsing, explicit library import/scan, artist/album/title/genre metadata and search, named playlists, persistent queue, Play/Pause/Stop/Next/Previous, repeat/shuffle, volume, optional autoplay after deliberate library loading, and a native bypassable 10-band equalizer.

**ZERO images**: no cover art, artwork extraction, picture blocks retained, graphical visualizers, skins, thumbnails, placeholders, image decoding/caching, image network retrieval or image dependencies. Text/control 128x64 1024-byte Rust Boy framebuffer is permitted.

**Policy delta:** earlier docs prohibit all autoplay. New authorized behavior is *optional autoplay after an explicit user-initiated load/import*. No autoplay on process launch, background refresh, restart, file-watcher change or reconnect. Default **off** for existing users and new installs until explicitly enabled via setting (or explicit per-import Play action). Update contradictory docs and agent guidance only after reconciling the current local checkout.

## Autoplay transaction state machine

1. User invokes Load Library/Import; host selects only approved directory roots with existing symlink/path constraints.
2. Bounded, cancelable, non-blocking scan builds a candidate catalog, sorted deterministically; on cancel, invalid metadata or exceeded budget, preserve prior committed catalog and do not play.
3. Commit successful catalog; communicate progress and number of eligible tracks. No implicit playback while scanning.
4. Evaluate persisted `autoplay_on_explicit_load` setting; if disabled, leave player stopped with selection ready.
5. If enabled: select first eligible track (or explicit user choice) deterministically, validate against lossless playback floor (>=16-bit, >=44.1kHz integer PCM source) using authoritative decoder admission, request host audio focus; **only after focus grant** open output and start playback. Never steal Radio/TV focus.
6. If focus unavailable, no autoplay and no automatic delayed surprise start. Indicate ready/blocked; require new explicit Play action to retry. Deny/skip invalid tracks within a bounded retry count.
7. Subsequent queue advance honors explicit initiated session, repeat/shuffle, stop and focus revocation; shutdown tears down child/output cleanly. No background scans, unexpected playback or media writes.

### Autoplay gate (new ML-AUTO)

Tests: default-off; user load enabled; cancel scan; no matches; invalid FLAC spoof; scan quota exceeded; duplicate roots; device unavailable; focus denied; focus revoked; Radio/TV contending; app launch/restart; stale removable media. Assert no playback or focus acquisition except after eligible explicit-load policy. Tests must include focus ownership and no surprise deferred start.

## Equalizer DSP (new ML-EQ)

- Implement own Rust real-time-safe stereo-aware 10-band EQ, suggested centers 31, 62, 125, 250, 500, 1k, 2k, 4k, 8k, 16k Hz, gain -12..+12 dB, bounded preamp/headroom, flat/user presets, reset, enable/bypass.
- At each negotiated sample rate, safely design stable coefficients; enforce Nyquist restrictions (at 44.1kHz, 16kHz is below Nyquist), finite coefficient and gain checks. Avoid denormals and clipping/overflow; use finite internal floating point when EQ enabled with explicit rounding/dither policy only when required for output format.
- Audio callback: no locks, allocations, file IO, network, image work or coefficient computation. Update parameters outside callback and swap bounded filter state with smoothing/crossfade to prevent zipper noise. Preserve channel separation, coherent state across stop/reopen, buffer sizes and sample rates.
- **Bypass is zero DSP on samples**: when device format permits original integer PCM, bit-exact output from decoded source to requested device buffer; never label physical DAC bit-perfect without measured output. EQ-enabled output is intentionally **processed**, not bit-perfect; do not claim lossless unchanged playback.
- Offer API independent of UI and test using synthetic impulses, sinusoids and multitone references. Test flat/bypass identity, measured response per band and rate, clipping/headroom, channel independence, coefficient stability, toggling while playing, malformed settings, volume interactions, CPU and memory cost. Avoid an always-active heavy DSP path.

## Other workstreams (new ML-UX)

- Integrate the existing stable track-ID catalog, bounded search/pagination, M3U8 model and Unix persistent queue. Audit latest operator's local code before additions; no duplicate crates or regressions.
- Implement persistent **named** playlists and attach library/search/playlist/queue to minimal text-first standalone UI and Rust Boy controls. Queue selection must not start playback; actual Play must own focus. Honor Unix-only limitations and define portability strategy.
- Headless CLI mirrors UI controls for testing; no external frameworks or third-party Cargo crates.
- Benchmark 0,1,100,2000,20000 tracks; record scan, p95 search, cold start, peak and **incremental** RSS (not process RSS mistakenly labeled catalog overhead), audio callback latency, EQ CPU overhead and binary footprint.

## Release ladder and tasking

P0: inspect actual HEAD, state and agent_skills revision; reconcile offline gates and operator-reported successful FLAC 6/6, WAVE 6/6, AIFF 7/7, ALAC corpus and WavPack fixtures without claiming reruns.
P1: merge named-playlist/queue/standalone UI workflow behind tests.
P2: ML-AUTO state-machine/setting and focus denial tests.
P3: ML-EQ own DSP, pure math/response and lifecycle tests; bypass proof.
P4: cross-module host Radio/TV focus, macOS LaunchServices/AudioQueue/device failure and recovery, physical DAC/panel acceptance and distribution signing.
P5: independent review, measured performance, canonical roadmap/index updates, complete offline premerge check and documented release decision.

Report observed commands/exits, tasks/SHAs, equipment availability, blocked gates and next steps. All success claims require evidence. Do not force-push, erase stash/backups, upload private music/logs or merge an unverified PR. Maintain existing release gates until their evidence is recorded.

## Acceptance definition

A user explicitly loads a local music directory. Rust Music safely scans it and shows indexed text tracks; optional user-enabled autoplay requests and obtains focus then plays the first eligible file; transport/queue/playlists/metadata work; enabling EQ audibly changes frequency response in a reproducible verified way and bypass preserves unprocessed PCM; no images or external dependencies; host/hardware checks are documented. The independent production release decision remains gated on actual physical and integration evidence.
