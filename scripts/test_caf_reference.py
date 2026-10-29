#!/usr/bin/env python3
"""Independent generated CAF -> native Rust PCM parity (FFmpeg + stdlib only).

All audio is deterministic synthetic PCM in a temporary directory. FFmpeg is
only a local CAF encoder for test vectors; it is never a runtime dependency.
"""
import pathlib
import shutil
import struct
import subprocess
import tempfile
import wave

ROOT = pathlib.Path(__file__).resolve().parents[1]


def pcm_bytes(samples, bits):
    width = bits // 8
    return b"".join(int(sample).to_bytes(width, "little", signed=True) for sample in samples)


def expected_samples(bits, frames=12000):
    limit = 1 << (bits - 1)
    values = []
    for frame in range(frames):
        values.extend((-limit + (frame * 7919) % (2 * limit), limit - 1 - (frame * 3571) % (2 * limit)))
    return values


def write_wave(path, bits, samples):
    with wave.open(str(path), "wb") as output:
        output.setnchannels(2)
        output.setsampwidth(bits // 8)
        output.setframerate(44100)
        output.writeframes(pcm_bytes(samples, bits))


def wav_data(path):
    data = path.read_bytes()
    if data[:4] != b"RIFF" or data[8:12] != b"WAVE":
        raise AssertionError(f"invalid Rust WAV output: {path}")
    position = 12
    while position + 8 <= len(data):
        kind = data[position:position + 4]
        size = struct.unpack_from("<I", data, position + 4)[0]
        position += 8
        if position + size + (size & 1) > len(data):
            raise AssertionError("Rust WAV output contains an invalid chunk boundary")
        if kind == b"data":
            return data[position:position + size]
        position += size + (size & 1)
    raise AssertionError("Rust WAV output has no data chunk")


def command(args, *, cwd=ROOT):
    result = subprocess.run(args, cwd=cwd, text=True, capture_output=True, check=False)
    if result.returncode:
        raise AssertionError(f"command failed ({result.returncode}): {args!r}\n{result.stdout}\n{result.stderr}")
    return result


def run():
    if not shutil.which("ffmpeg") or not shutil.which("cargo"):
        raise RuntimeError("ffmpeg and cargo are required for CAF differential validation")
    cases = (("lpcm16", 16, "pcm_s16le"), ("lpcm24", 24, "pcm_s24le"),
             ("lpcm32", 32, "pcm_s32le"), ("alac16", 16, "alac"),
             ("alac24", 24, "alac"))
    with tempfile.TemporaryDirectory(prefix="rust-music-caf-g11-") as temp:
        root = pathlib.Path(temp)
        for name, bits, codec in cases:
            source = root / f"{name}.wav"
            caf = root / f"{name}.caf"
            output = root / f"{name}-native.wav"
            expected = pcm_bytes(expected_samples(bits), bits)
            write_wave(source, bits, expected_samples(bits))
            command(["ffmpeg", "-nostdin", "-v", "error", "-y", "-i", str(source),
                     "-c:a", codec, "-f", "caf", str(caf)])
            command(["cargo", "run", "--locked", "--offline", "--quiet", "-p",
                     "rust-music-cli", "--", "decode", str(caf), str(output)])
            actual = wav_data(output)
            if actual != expected:
                raise AssertionError(f"native CAF {codec} PCM mismatch for {name}: {len(actual)} vs {len(expected)} bytes")
            print(f"PASS {name}: {len(expected)} synthetic PCM bytes exactly match")
    print(f"PASS: {len(cases)} generated CAF vectors; FFmpeg test tooling only")


if __name__ == "__main__":
    run()
