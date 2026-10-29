# G7: Strict RIFF/WAVE and PCM round-trip engineering ledger

Date: 2026-10-29. Scope: **source implementation, locally executed Rust compilation, and reference PCM parity**, not hardware certification.

The decoder and exporter remain locally authored; all Cargo packages use local workspace paths. No multimedia library, registry package, codec plugin or service is introduced.

## Implemented

- `rust-music-wave` accounts for the advertised RIFF size from the initial header. It rejects any `fmt`, `data`, or skipped chunk exceeding the declared RIFF boundary, truncated headers, duplicate `fmt`, invalid frame alignment, and unsupported sample subtypes.
- Odd-sized RIFF chunks consume a pad byte. Odd-length `data` requires a pad byte at the end of decoding; a missing pad causes an error rather than a silent success.
- PCM `WAVE_FORMAT_EXTENSIBLE` is supported, using the PCM GUID, channel-mask consistency checks, explicit container bits versus valid bits, and signed integer conversion for left-aligned 24-in-32 and 16-in-24 samples.
- Nonzero padding bits are **rejected**; silently right-shifting such data would violate the precision-preservation goal. Supported output precision is 8, 16, 24, or 32 bits.
- The CLI WAV export now includes required odd-byte padding and uses a 40-byte `fmt` chunk with PCM subtype for 3+ channels. It uses a **zero speaker mask** because the current `AudioFormat` does not know the original speakers. It does not invent a speaker configuration.
- The CLI size limit now reserves RIFF header and optional odd-data padding space before writing PCM. Both `decode` and `preview` use exclusive file creation to prevent overwriting existing music, output files or symlinks.
- Unit tests exercise signed 24-bit, unsigned 8-bit, 24 valid bits stored in 32, multichannel PCM, bad GUID, impossible sample precision, wrong RIFF size, missing trailing pad, odd metadata chunks, export headers and zero low padding bits.

## Offline tests

From a pre-provisioned Rust 1.85+ machine:

```sh
sh scripts/offline-gate.sh
python3 scripts/test_wave_reference.py
```

The second command is an **optional**, stdlib-only differential harness requiring a local Python interpreter and Cargo. It creates six deterministic RIFF fixtures, runs our actual `inspect`/`decode` CLI with `--locked --offline`, and compares the exact integer PCM bytes and exported RIFF structure. Three malformed inputs must fail (container boundary, missing odd padding, nonzero extensible low bits).

Without a local Rust toolchain:

```sh
python3 scripts/test_wave_reference.py --fixtures-only
python3 scripts/check_literature.py
python3 scripts/test_flac_reference.py --fixtures-only
```

Passing `--fixtures-only` **does not run the Rust decoder**. The full native harness was later run locally: all six vectors matched exact PCM and the malformed RIFF, missing-pad, and nonzero-padding cases were rejected. This does not establish bit-perfect DAC behavior or real Rust_Boy deployment.

## Documented exclusions

- RF64, RIFX, float WAVE, compressed WAV, ADPCM, 20-bit precision, and wave seeking are not yet supported. Malformed metadata fails explicitly.
- The decoder represents PCM audio precision, **not a speaker-routing map**. An application requiring exact channel mapping must add a channel-mask-aware API before a certified multichannel playback release.
- This reader does not promise hostile filesystem TOCTOU safety or physical DAC bit-perfect output.

## Standards and traceability

- [Microsoft: WAVEFORMATEXTENSIBLE](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ksmedia/ns-ksmedia-waveformatextensible) for valid versus container bits, PCM GUID and channel mask.
- [Microsoft: RIFF](https://learn.microsoft.com/en-us/windows/win32/xaudio2/resource-interchange-file-format--riff-) for chunk lengths, even-byte padding and RIFF file-size accounting.

## Gate checklist

| Gate | Requirement | Status |
| --- | --- | --- |
| G7-01 | RIFF and chunk bounds checked before PCM | Implemented; workspace and malformed-input tests pass |
| G7-02 | WAVE_FORMAT_EXTENSIBLE PCM supported without precision loss | Implemented; workspace and native parity tests pass |
| G7-03 | Odd WAV data exported with required padding | Implemented; workspace and native parity tests pass |
| G7-04 | Six stdlib WAV reference fixtures are structurally sound | PASS (fixtures-only) |
| G7-05 | Actual Rust decoder/export PCM parity | PASS (6 generated native CLI vectors) |
| G7-06 | Native 24/32-bit DAC and 6-channel routing | NOT TESTED |
