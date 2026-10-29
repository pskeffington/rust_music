#!/usr/bin/env python3
"""Offline WAVE/RIFF acceptance vectors; stdlib only and no runtime dependencies.

--fixtures-only validates deterministic fixture bytes without Rust installed.
The default runs the actual Rust CLI and checks exact integer PCM parity,
strict malformed-RIFF rejection, and correctly padded exported WAVE files.
"""
import argparse
import pathlib
import shutil
import struct
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
GUID_PCM = bytes.fromhex("0100000000001000800000aa00389b71")


def pcm(value, bits):
    if bits == 8:
        return bytes([(value + 128) & 255])
    return value.to_bytes(bits // 8, "little", signed=True)


def chunk(name, data):
    assert len(name) == 4
    return name + struct.pack("<I", len(data)) + data + (b"\0" if len(data) & 1 else b"")


def make_wave(values, channels, bits, valid_bits=None, mask=0, junk=False):
    valid_bits = valid_bits or bits
    assert len(values) % channels == 0
    assert valid_bits in (8, 16, 24, 32) and valid_bits <= bits
    packed_pcm = b"".join(
        pcm(value << (bits - valid_bits), bits) for value in values
    )
    alignment = channels * bits // 8
    extensible = channels > 2 or bits != valid_bits
    fmt = struct.pack("<HHIIHH", 0xFFFE if extensible else 1,
                      channels, 44100, 44100 * alignment, alignment, bits)
    if extensible:
        fmt += struct.pack("<HHI", 22, valid_bits, mask) + GUID_PCM
    data = b"WAVE" + chunk(b"fmt ", fmt)
    if junk:
        data += chunk(b"JUNK", b"odd")
    data += chunk(b"data", packed_pcm)
    encoded = b"RIFF" + struct.pack("<I", len(data)) + data
    expected = b"".join(pcm(value, valid_bits) for value in values)
    return encoded, expected


def data_chunk(wav):
    if wav[:4] != b"RIFF" or wav[8:12] != b"WAVE":
        raise AssertionError("bad RIFF/WAVE signature")
    assert struct.unpack_from("<I", wav, 4)[0] == len(wav) - 8
    pos = 12
    while pos + 8 <= len(wav):
        kind = wav[pos:pos + 4]
        size = struct.unpack_from("<I", wav, pos + 4)[0]
        pos += 8
        assert pos + size + (size & 1) <= len(wav), "chunk exceeds RIFF"
        if kind == b"data":
            return wav[pos:pos + size]
        pos += size + (size & 1)
    raise AssertionError("missing PCM data")


def decoder(*args, ok=True):
    completed = subprocess.run(
        ["cargo", "run", "--locked", "--offline", "--quiet", "-p", "rust-music-cli", "--", *map(str, args)],
        cwd=ROOT, text=True, capture_output=True, check=False
    )
    if ok and completed.returncode != 0:
        raise AssertionError(f"Rust CLI failed {args!r}\n{completed.stdout}\n{completed.stderr}")
    return completed


CASES = (
    ("mono-8-odd", [-128, -50, 0, 50, 127], 1, 8, 8, 0, False),
    ("stereo-16", [-32768, 32767, 100, -100], 2, 16, 16, 0, True),
    ("stereo-24", [-8388608, 8388607, -1, 1], 2, 24, 24, 0, False),
    ("stereo-32", [-2147483648, 2147483647, 0, 1000], 2, 32, 32, 0, False),
    ("six-channel-16", [1, 2, 3, 4, 5, 6], 6, 16, 16, 0x3F, False),
    ("stereo-valid24-in32", [-8388608, 8388607, 0, 1234], 2, 32, 24, 3, False),
)


def run(fixtures_only=False):
    if not fixtures_only and shutil.which("cargo") is None:
        raise RuntimeError("cargo unavailable; use --fixtures-only for structural reference tests")
    with tempfile.TemporaryDirectory(prefix="rust-music-wave-g7-") as tmp:
        root = pathlib.Path(tmp)
        for name, values, channels, bits, valid_bits, mask, junk in CASES:
            wav, expected_pcm = make_wave(values, channels, bits, valid_bits, mask, junk)
            container_pcm = data_chunk(wav)
            restored = []
            for pos in range(0, len(container_pcm), bits // 8):
                raw = container_pcm[pos:pos + bits // 8]
                signed = (raw[0] - 128) if bits == 8 else int.from_bytes(raw, "little", signed=True)
                padding = bits - valid_bits
                assert signed & ((1 << padding) - 1) == 0
                restored.append(signed >> padding)
            assert restored == values, f"reference PCM decode mismatch: {name}"
            source = root / f"{name}.wav"
            output = root / f"{name}-roundtrip.wav"
            source.write_bytes(wav)
            if fixtures_only:
                print(f"REFERENCE {name}: {len(expected_pcm)} canonical PCM bytes; RIFF size valid")
                continue
            decoder("inspect", source)
            decoder("decode", source, output)
            rendered = output.read_bytes()
            actual_pcm = data_chunk(rendered)
            assert actual_pcm == expected_pcm, f"integer PCM mismatch in {name}"
            assert rendered[20:22] == struct.pack("<H", 0xFFFE if channels > 2 else 1)
            if len(actual_pcm) & 1:
                assert rendered[-1:] == b"\0", f"missing RIFF pad in {name}"
            print(f"PASS {name}: {len(actual_pcm)} bytes PCM bit-identical")

        if not fixtures_only:
            source, _ = make_wave([-1, 1], 2, 16)
            # Declared RIFF size ends inside the data payload.
            wrong = bytearray(source)
            wrong[4:8] = struct.pack("<I", len(source) - 10)
            invalid = root / "bad-riff.wav"
            invalid.write_bytes(wrong)
            assert decoder("inspect", invalid, ok=False).returncode != 0
            # Entire data payload is present, but required odd pad was removed.
            odd, _ = make_wave([0], 1, 8)
            invalid.write_bytes(odd[:-1])
            assert decoder("inspect", invalid, ok=False).returncode != 0
            # Extensible PCM low padding bits must be zero when valid bits < container.
            padded, _ = make_wave([1234, -1234], 2, 32, 24)
            broken = bytearray(padded)
            data_start = broken.index(b"data") + 8
            broken[data_start] |= 1
            invalid.write_bytes(broken)
            assert decoder("inspect", invalid, ok=False).returncode != 0
            print("PASS malformed WAVE rejection: RIFF boundary, missing pad, nonzero low bits")
    print(f"PASS: {len(CASES)} WAVE cases; fixtures-only={fixtures_only}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixtures-only", action="store_true")
    args = parser.parse_args()
    run(fixtures_only=args.fixtures_only)
