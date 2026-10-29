#!/usr/bin/env python3
"""Verify the checked-in CC0 ALAC/M4A corpus, Rust PCM parity and ingestion.

FFmpeg and ffprobe are optional local oracle tools; neither is a runtime
dependency. The AAC negative fixture is a deterministic synthetic file.
"""
import hashlib
import pathlib
import shutil
import subprocess
import tempfile
import wave

ROOT = pathlib.Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "crates/rust-music-mp4/tests/fixtures"
UPSTREAM = FIXTURES / "upstream"
CASES = {
    "ia-canon-f-minor.m4a": ("239f85a564581fef5db513e78b63e7fa9b50d48e63e1927a9943dc647f578b69", 48000, 24),
    "ia-mazurka-op24-no2.m4a": ("e1ba599d407fd360ab631d5b1508eae8b20822952c3bb49637f8d88cd6ff9c3c", 44100, 16),
    "ia-mazurka-op7-no4.m4a": ("bccc40830b8a8d72185801ae9159af8b8360e7ed4f91d107bb234f892eb02840", 44100, 16),
    "ia-mazurka-op24-no3.m4a": ("c27735d5b4b2a5d898a7848b3aa67b68e84fe80080bc25fdc61b61909da59899", 44100, 16),
    "ia-mazurka-op17-no3.m4a": ("3b59179aec77376f5609a7130ad7cd9a19d9ae484b98f3a6f4c80152922d0d10", 44100, 16),
    "ia-mazurka-op17-no4.m4a": ("e231321ea7671950c16edd0d5bd93863479efbd1cb8acae183d2d9c5c16eb3bb", 44100, 16),
}


def command(args, *, ok=True):
    result = subprocess.run(args, cwd=ROOT, text=True, capture_output=True, check=False)
    if ok and result.returncode:
        raise AssertionError(f"command failed ({result.returncode}): {args!r}\n{result.stdout}\n{result.stderr}")
    return result


def rust_cli(*args, ok=True):
    return command(["cargo", "run", "--locked", "--offline", "--quiet", "-p", "rust-music-cli", "--", *map(str, args)], ok=ok)


def codec(path):
    return command([
        "ffprobe", "-v", "error", "-select_streams", "a:0",
        "-show_entries", "stream=codec_name,sample_rate,channels,bits_per_raw_sample",
        "-of", "default=noprint_wrappers=1", str(path),
    ]).stdout


def wav_pcm(path):
    with wave.open(str(path), "rb") as audio:
        spec = (audio.getnchannels(), audio.getsampwidth(), audio.getframerate(), audio.getnframes())
        return spec, audio.readframes(audio.getnframes())


def run():
    for tool in ("cargo", "ffmpeg", "ffprobe"):
        if not shutil.which(tool):
            raise RuntimeError(f"{tool} is required for the optional M4A corpus test")
    with tempfile.TemporaryDirectory(prefix="rust-music-m4a-corpus-") as temp:
        work = pathlib.Path(temp)
        for name, (expected_hash, expected_rate, expected_bits) in CASES.items():
            source = UPSTREAM / name
            digest = hashlib.sha256(source.read_bytes()).hexdigest()
            if digest != expected_hash:
                raise AssertionError(f"fixture hash mismatch: {name}")
            details = dict(line.split("=", 1) for line in codec(source).splitlines())
            expected = {"codec_name": "alac", "sample_rate": str(expected_rate), "channels": "2", "bits_per_raw_sample": str(expected_bits)}
            if any(details.get(key) != value for key, value in expected.items()):
                raise AssertionError(f"unexpected stream details for {name}: {details}")

            reference = work / f"{name}-reference.wav"
            decoded = work / f"{name}-rust.wav"
            command(["ffmpeg", "-nostdin", "-v", "error", "-y", "-i", str(source), "-map", "0:a:0", "-c:a", f"pcm_s{expected_bits}le", str(reference)])
            rust_cli("decode", source, decoded)
            if wav_pcm(reference) != wav_pcm(decoded):
                raise AssertionError(f"decoded PCM differs from FFmpeg for {name}")
            print(f"PASS {name}: ALAC {expected_rate} Hz/{expected_bits}-bit exact PCM")

        # Content-based library ingestion accepts the MP4 container. Codec
        # admission is a later decoder boundary and must still reject AAC.
        scan_root = work / "scan"
        scan_root.mkdir()
        for path in [*(UPSTREAM / name for name in CASES), FIXTURES / "generated_aac.m4a"]:
            shutil.copyfile(path, scan_root / path.name)
        scan = rust_cli("scan", scan_root).stdout
        if (
            f"{len(CASES) + 1} tracks" not in scan
            or "Mp4" not in scan
            or any(name not in scan for name in CASES)
        ):
            raise AssertionError(f"M4A signature ingestion failed: {scan}")
        aac_details = dict(line.split("=", 1) for line in codec(FIXTURES / "generated_aac.m4a").splitlines())
        if aac_details.get("codec_name") != "aac":
            raise AssertionError(f"negative fixture is not AAC: {aac_details}")
        denied = rust_cli("inspect", FIXTURES / "generated_aac.m4a", ok=False)
        if denied.returncode == 0:
            raise AssertionError(f"AAC negative fixture was not rejected: {denied.stderr}")
        print("PASS ingestion: upstream ALAC M4As and AAC container catalogued; AAC decode rejected")
    print(f"PASS: {len(CASES)}/{len(CASES)} checked-in Internet Archive ALAC/M4A parity cases; temporary PCM outputs removed")


if __name__ == "__main__":
    run()
