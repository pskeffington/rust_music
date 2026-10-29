#!/usr/bin/env python3
"""G10 deterministic lossless AIFF/AIFF-C reference corpus, Python stdlib only.

--fixtures-only tests independently constructed AIFF IFF structure and sample
bytes without Cargo. Default also runs our Rust CLI and compares exported
integer PCM byte-for-byte against a separately constructed reference.
No decoder library, remote files, installed codec or Python package is needed.
"""
import argparse
import pathlib
import shutil
import struct
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
VERSION = bytes.fromhex("a2805140")


def iff_chunk(kind, payload):
    assert len(kind) == 4
    return kind + struct.pack(">I", len(payload)) + payload + (b"\0" if len(payload) & 1 else b"")


def rate80(rate):
    assert 1 <= rate <= 1_048_575
    exponent = rate.bit_length() - 1
    mantissa = rate << (63 - exponent)
    return struct.pack(">H", 16383 + exponent) + mantissa.to_bytes(8, "big")


def make_aiff(values, channels, bits, rate, aifc=False, codec=b"NONE", junk=False, offset=b""):
    assert bits in (8, 16, 24, 32)
    assert len(values) % channels == 0
    order = "little" if codec == b"sowt" else "big"
    samples = b"".join(value.to_bytes(bits // 8, order, signed=True) for value in values)
    comm = struct.pack(">HIH", channels, len(values) // channels, bits) + rate80(rate)
    if aifc:
        comm += codec + b"\0\0"
    sound = struct.pack(">II", len(offset), 0) + offset + samples
    body = b"AIFC" if aifc else b"AIFF"
    if aifc:
        body += iff_chunk(b"FVER", VERSION)
    body += iff_chunk(b"COMM", comm)
    if junk:
        body += iff_chunk(b"JUNK", b"odd")
    body += iff_chunk(b"SSND", sound)
    encoded = b"FORM" + struct.pack(">I", len(body)) + body
    expected = b"".join(
        (bytes([(value + 128) & 255]) if bits == 8 else value.to_bytes(bits // 8, "little", signed=True))
        for value in values
    )
    return encoded, expected


def chunk_data(file, kind, endian):
    assert file[:4] in (b"FORM", b"RIFF")
    container_length = int.from_bytes(file[4:8], endian)
    assert container_length == len(file) - 8, "container length mismatch"
    pos = 12
    while pos + 8 <= len(file):
        chunk_kind = file[pos:pos + 4]
        length = int.from_bytes(file[pos + 4:pos + 8], endian)
        pos += 8
        assert pos + length + (length & 1) <= len(file), "chunk overrun"
        if chunk_kind == kind:
            return file[pos:pos + length]
        pos += length + (length & 1)
    raise AssertionError(f"chunk {kind!r} missing")


def cli(*args, ok=True):
    outcome = subprocess.run(
        ["cargo", "run", "--locked", "--offline", "--quiet",
         "-p", "rust-music-cli", "--", *map(str, args)],
        cwd=ROOT, capture_output=True, text=True, check=False,
    )
    if ok and outcome.returncode != 0:
        raise AssertionError(f"Rust CLI failed: {args!r}\n{outcome.stdout}\n{outcome.stderr}")
    return outcome


CASES = [
    ("mono-16", [-32768, 0, 32767], 1, 16, 44100, False, b"NONE", False, b""),
    ("stereo-24-192k", [-8388608, 8388607, 0, 12345], 2, 24, 192000, False, b"NONE", True, b""),
    ("stereo-32", [-2147483648, 2147483647, 1, -1], 2, 32, 96000, False, b"NONE", False, b""),
    ("aifc-none", [111, -222], 1, 16, 48000, True, b"NONE", False, b""),
    ("aifc-sowt-24", [-8388608, 8388607], 1, 24, 48000, True, b"sowt", False, b""),
    ("offset-16", [-1, 1], 1, 16, 44100, False, b"NONE", False, b"padding"),
    # A deliberately low-quality PCM parser fixture. Playback policy must reject.
    ("parser-only-8", [-128, 0, 127], 1, 8, 44100, False, b"NONE", False, b""),
]


def run(fixtures_only=False):
    if not fixtures_only and shutil.which("cargo") is None:
        raise RuntimeError("cargo unavailable: run --fixtures-only for reference vectors")
    with tempfile.TemporaryDirectory(prefix="rust-music-aiff-g10-") as tmp:
        root = pathlib.Path(tmp)
        for name, values, channels, bits, rate, aifc, codec, junk, offset in CASES:
            encoded, expected = make_aiff(values, channels, bits, rate, aifc, codec, junk, offset)
            assert encoded[:4] == b"FORM"
            comm = chunk_data(encoded, b"COMM", "big")
            assert struct.unpack_from(">I", comm, 2)[0] == len(values) // channels
            assert comm[8:18] == rate80(rate)
            ssnd = chunk_data(encoded, b"SSND", "big")
            skip = struct.unpack_from(">I", ssnd, 0)[0]
            assert skip == len(offset)
            audio = ssnd[8 + skip:]
            signed = [int.from_bytes(audio[i:i + bits // 8],
                                     "little" if codec == b"sowt" else "big", signed=True)
                      for i in range(0, len(audio), bits // 8)]
            assert signed == values, f"independent AIFF PCM mismatch in {name}"
            src = root / (name + (".aifc" if aifc else ".aiff"))
            dst = root / (name + "-decoded.wav")
            src.write_bytes(encoded)
            if not fixtures_only:
                cli("inspect", src)
                cli("decode", src, dst)
                actual = chunk_data(dst.read_bytes(), b"data", "little")
                assert actual == expected, f"Rust PCM mismatch in {name}"
            print(f"{'REFERENCE' if fixtures_only else 'PASS'} {name}: {len(expected)} exact PCM bytes")

        if not fixtures_only:
            good, _ = make_aiff([1], 1, 16, 44100)
            bad = bytearray(good)
            bad[4:8] = struct.pack(">I", 20)
            invalid = root / "invalid.aiff"
            invalid.write_bytes(bad)
            assert cli("inspect", invalid, ok=False).returncode != 0
            bad = bytearray(good)
            comm_pos = bad.index(b"COMM") + 8
            bad[comm_pos + 2:comm_pos + 6] = struct.pack(">I", 2)
            invalid.write_bytes(bad)
            assert cli("inspect", invalid, ok=False).returncode != 0
            invalid.write_bytes(good[:-1])
            assert cli("inspect", invalid, ok=False).returncode != 0
            lossy, _ = make_aiff([1], 1, 16, 44100, aifc=True, codec=b"ulaw")
            invalid.write_bytes(lossy)
            assert cli("inspect", invalid, ok=False).returncode != 0
            print("PASS corrupt FORM, contradictory COMM, truncation and lossy AIFC rejected")
    print(f"PASS: {len(CASES)} AIFF/AIFF-C vectors; fixtures-only={fixtures_only}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixtures-only", action="store_true")
    args = parser.parse_args()
    run(fixtures_only=args.fixtures_only)
