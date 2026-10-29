#!/usr/bin/env python3
"""Reproducible external FLAC oracle (test-only; never linked at runtime).

With --fixtures-only, validate WAV/FLAC provenance and reference MD5 without a
Rust toolchain. The default additionally runs our CLI and compares every PCM
sample, checks MD5 tampering, a bad frame CRC, and truncated input.
"""

import argparse
import hashlib
import math
import pathlib
import random
import shutil
import struct
import subprocess
import tempfile
import wave

REPO = pathlib.Path(__file__).resolve().parents[1]
SEED = 13264
FRAMES = 16_777
CASES = (
    (1, 8, "constant"),
    (1, 16, "constant"),
    (2, 16, "chirp"),
    (2, 24, "ramp"),
    (2, 24, "noise"),
    (2, 32, "noise"),
    (6, 16, "chirp"),
)


def generate(path, depth, channels, kind):
    rand = random.Random(SEED)
    rate = 96_000 if kind == "chirp" else 44_100
    peak = (1 << (depth - 1)) - 1
    wav_pcm, signed_pcm = bytearray(), bytearray()
    for i in range(FRAMES):
        for c in range(channels):
            if kind == "constant":
                value = 111 if c % 2 else -111
            elif kind == "chirp":
                value = int(peak * 0.6 * math.sin(i * i / 120_000 + c * 0.3))
            elif kind == "ramp":
                value = ((i * 117 + c * 31) % (2 * peak + 1)) - peak
            else:
                value = rand.randint(-peak - 1, peak)
            value = max(-peak - 1, min(value, peak))
            encoded = value.to_bytes(depth // 8, "little", signed=True)
            signed_pcm.extend(encoded)
            if depth == 8:  # RIFF 8-bit PCM is unsigned; FLAC input is signed.
                wav_pcm.append((value + 128) & 255)
            else:
                wav_pcm.extend(encoded)
    if channels > 2:
        # The reference FLAC CLI requires WAVE_FORMAT_EXTENSIBLE to assign
        # multichannel positions. The decoder under test still gets raw FLAC.
        alignment = channels * (depth // 8)
        fmt = struct.pack("<HHIIHHHHI", 0xfffe, channels, rate,
                          rate * alignment, alignment, depth, 22, depth, 0x3f)
        fmt += bytes.fromhex("0100000000001000800000aa00389b71")
        body = b"WAVEfmt " + struct.pack("<I", len(fmt)) + fmt
        body += b"data" + struct.pack("<I", len(wav_pcm)) + wav_pcm
        path.write_bytes(b"RIFF" + struct.pack("<I", len(body)) + body)
    else:
        with wave.open(str(path), "wb") as out:
            out.setnchannels(channels)
            out.setsampwidth(depth // 8)
            out.setframerate(rate)
            out.writeframes(wav_pcm)
    return bytes(wav_pcm), hashlib.md5(signed_pcm, usedforsecurity=False).digest()


def command(*args, ok=True):
    result = subprocess.run(args, cwd=REPO, text=True, capture_output=True, check=False)
    if ok and result.returncode != 0:
        raise AssertionError(f"command failed: {args!r}\n{result.stdout}\n{result.stderr}")
    return result


def decoder(*args, ok=True):
    return command(
        "cargo", "run", "--locked", "--offline", "--quiet", "-p", "rust-music-cli",
        "--", *map(str, args), ok=ok
    )


def flac_md5(data):
    assert data[:8] == b"fLaC\x00\x00\x00\x22" or data[:8] == b"fLaC\x80\x00\x00\x22"
    return data[26:42]


def run(fixtures_only=False):
    if not shutil.which("flac"):
        raise RuntimeError("reference flac executable not installed")
    if not fixtures_only and not shutil.which("cargo"):
        raise RuntimeError("Rust cargo unavailable: use --fixtures-only for reference fixture validation")
    with tempfile.TemporaryDirectory(prefix="rust-music-flac-") as temp:
        root = pathlib.Path(temp)
        for channels, depth, kind in CASES:
            name = f"{channels}ch-{depth}bit-{kind}"
            wave_in = root / f"{name}.wav"
            flac = root / f"{name}.flac"
            wave_out = root / f"{name}-out.wav"
            original, expected_md5 = generate(wave_in, depth, channels, kind)
            command("flac", "-8", "--verify", "-f", "-o", str(flac), str(wave_in))
            command("flac", "-t", str(flac))
            encoded = flac.read_bytes()
            assert flac_md5(encoded) == expected_md5, f"reference FLAC STREAMINFO MD5 differs: {name}"
            digest = hashlib.sha256(encoded).hexdigest()[:16]
            if fixtures_only:
                print(f"REFERENCE {name}: {len(original)} PCM bytes, sha256:{digest}, MD5 valid")
                continue

            decoder("inspect", flac)
            decoder("decode", flac, wave_out)
            with wave.open(str(wave_out), "rb") as wav_result:
                assert wav_result.getnchannels() == channels
                assert wav_result.getsampwidth() == depth // 8
                assert wav_result.getframerate() == (96_000 if kind == "chirp" else 44_100)
                actual = wav_result.readframes(wav_result.getnframes())
                assert actual == original, f"sample mismatch: {name}"
            print(f"PASS PCM {name}: {len(original)} bytes, sha256:{digest}")

            tampered = bytearray(encoded)
            tampered[26] ^= 1  # STREAMINFO digest; frame CRCs remain untouched.
            bad_md5 = root / f"{name}-bad-md5.flac"
            bad_md5.write_bytes(tampered)
            result = decoder("inspect", bad_md5, ok=False)
            assert result.returncode != 0 and "md5" in result.stderr.lower(), name

            bad_crc = root / f"{name}-bad-crc.flac"
            bad_crc.write_bytes(encoded[:-1] + bytes([encoded[-1] ^ 1]))
            assert decoder("inspect", bad_crc, ok=False).returncode != 0, name

            truncated = root / f"{name}-truncated.flac"
            truncated.write_bytes(encoded[:-3])
            assert decoder("inspect", truncated, ok=False).returncode != 0, name
            print(f"PASS integrity {name}: STREAMINFO MD5, CRC16 and truncation rejected")

    print(f"PASS: {len(CASES)} reference FLAC cases; fixtures-only={fixtures_only}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixtures-only", action="store_true", help="skip cargo; validate the reference encoder and fixture MD5")
    args = parser.parse_args()
    run(fixtures_only=args.fixtures_only)
