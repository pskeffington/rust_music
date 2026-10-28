#!/usr/bin/env python3
"""Generate deterministic WAV audio; encode with libFLAC CLI as external test oracle.
No reference decoder or encoder is linked into Rust Music. Requires flac CLI.
"""
import math
import pathlib
import random
import subprocess
import tempfile
import wave

REPO = pathlib.Path(__file__).resolve().parents[1]

def generate(path, depth, channels, kind):
    randomizer = random.Random(13264)
    frames = 16_777
    sample_rate = 44_100 if kind != 'chirp' else 96_000
    max_magnitude = (1 << (depth - 1)) - 1
    pcm = bytearray()
    for i in range(frames):
        for c in range(channels):
            if kind == 'constant': sample = 111 if c % 2 else -111
            elif kind == 'chirp': sample = int(max_magnitude * 0.6 * math.sin(i * i / 120_000 + c * 0.3))
            elif kind == 'ramp': sample = ((i * 117 + c * 31) % (2 * max_magnitude + 1)) - max_magnitude
            else: sample = randomizer.randint(-max_magnitude, max_magnitude)
            sample = max(-max_magnitude - 1, min(sample, max_magnitude))
            pcm.extend(sample.to_bytes(depth // 8, 'little', signed=True))
    with wave.open(str(path), 'wb') as out:
        out.setnchannels(channels)
        out.setsampwidth(depth // 8)
        out.setframerate(sample_rate)
        out.writeframes(pcm)
    return bytes(pcm)

def run():
    with tempfile.TemporaryDirectory() as workspace:
        root = pathlib.Path(workspace)
        for channels, depth, kind in [(1,16,'constant'),(2,16,'chirp'),(2,24,'ramp'),(2,24,'noise')]:
            name = f'{channels}ch-{depth}bit-{kind}'
            wave_in, flac, wave_out = (root / (name + ext) for ext in ('.wav', '.flac', '-out.wav'))
            raw = generate(wave_in, depth, channels, kind)
            subprocess.run(['flac','-8','--verify','-f','-o',str(flac),str(wave_in)], check=True, capture_output=True)
            subprocess.run(['cargo','run','--quiet','-p','rust-music-cli','--','decode',str(flac),str(wave_out)],
                           cwd=REPO, check=True)
            with wave.open(str(wave_out), 'rb') as result:
                assert result.getnchannels() == channels and result.getsampwidth() == depth//8
                assert result.readframes(result.getnframes()) == raw, f'lossless PCM mismatch: {name}'
            print(f'PASS FLAC decoder parity: {name}, {len(raw)} bytes')

if __name__ == '__main__': run()
