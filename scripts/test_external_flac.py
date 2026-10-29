#!/usr/bin/env python3
"""External FLAC corpus gate. Music stays local, never added to the repository.

Run:
  python3 scripts/test_external_flac.py --reference-only /music/*.flac
  python3 scripts/test_external_flac.py /music/*.flac

The second command requires an offline Cargo toolchain. Reference flac is a
test-only oracle and is never linked into the Rust_Music player.
"""
import argparse
import hashlib
import json
import pathlib
import shutil
import subprocess
import sys
import tempfile
import wave


def command(args):
    return subprocess.run(args, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          check=False)


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as f:
        while chunk := f.read(1024 * 1024):
            h.update(chunk)
    return h.hexdigest()


def streaminfo(path):
    # Parse the FLAC header without assuming STREAMINFO is the final block.
    with path.open("rb") as f:
        if f.read(4) != b"fLaC":
            raise ValueError("not native FLAC")
        header = f.read(4)
        if len(header) != 4 or (header[0] & 0x7f) != 0 or int.from_bytes(header[1:], "big") != 34:
            raise ValueError("STREAMINFO missing or malformed")
        data = f.read(34)
        if len(data) != 34:
            raise ValueError("truncated STREAMINFO")
    packed = int.from_bytes(data[10:18], "big")
    return {
        "rate": (packed >> 44) & 0xfffff,
        "channels": ((packed >> 41) & 7) + 1,
        "bits": ((packed >> 36) & 31) + 1,
        "samples": packed & ((1 << 36) - 1),
        "pcm_md5": data[18:34].hex()
    }


def run_file(path, cargo, reference_only):
    result = {
        "file": path.name, "bytes": path.stat().st_size,
        "sha256": digest(path), "streaminfo": streaminfo(path)
    }
    integrity = command(["flac", "-t", "-s", str(path)])
    result["reference_integrity"] = "pass" if integrity.returncode == 0 else "fail"
    if integrity.returncode:
        result["error"] = integrity.stderr.decode("utf-8", errors="replace")[-400:]
        return result
    if reference_only:
        result["custom_pcm_parity"] = "not_run"
        return result

    info = result["streaminfo"]
    if info["bits"] not in (8, 16, 24, 32):
        result["custom_pcm_parity"] = "unsupported_test_width"
        return result

    with tempfile.TemporaryDirectory(prefix="rust-music-external-") as tmp:
        root = pathlib.Path(tmp)
        expected = root / "reference.pcm"
        actual = root / "custom.wav"
        oracle = command(["flac", "-d", "-s", "-f", "--force-raw-format",
                          "--endian=little", "--sign=signed",
                          "-o", str(expected), str(path)])
        if oracle.returncode:
            result["custom_pcm_parity"] = "oracle_failed"
            result["error"] = oracle.stderr.decode(errors="replace")[-400:]
            return result
        decoder = command([cargo, "run", "--offline", "--locked", "--quiet",
                           "-p", "rust-music-cli", "--", "decode", str(path), str(actual)])
        if decoder.returncode:
            result["custom_pcm_parity"] = "decoder_failed"
            result["error"] = decoder.stderr.decode(errors="replace")[-500:]
            return result
        try:
            with wave.open(str(actual), "rb") as w:
                metadata_matches = (w.getframerate() == info["rate"]
                    and w.getnchannels() == info["channels"]
                    and w.getsampwidth() * 8 == info["bits"]
                    and (not info["samples"] or w.getnframes() == info["samples"]))
                samples = w.readframes(w.getnframes())
                if info["bits"] == 8:
                    samples = bytes((b ^ 0x80) for b in samples)
        except (EOFError, wave.Error) as exc:
            result["custom_pcm_parity"] = "malformed_wav"
            result["error"] = str(exc)
            return result
        result["reference_pcm_sha256"] = digest(expected)
        result["custom_pcm_sha256"] = hashlib.sha256(samples).hexdigest()
        result["custom_pcm_parity"] = ("pass" if metadata_matches
             and result["reference_pcm_sha256"] == result["custom_pcm_sha256"]
             else "fail")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-only", action="store_true")
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--report", type=pathlib.Path)
    parser.add_argument("files", nargs="+", type=pathlib.Path)
    options = parser.parse_args()
    if not shutil.which("flac"):
        parser.error("reference flac executable is required for this test gate")
    if not options.reference_only and not shutil.which(options.cargo):
        parser.error("cargo unavailable; use --reference-only for file-integrity preflight")
    rows = []
    for path in options.files:
        if not path.is_file():
            rows.append({"file": str(path), "reference_integrity": "missing"})
            continue
        try:
            rows.append(run_file(path, options.cargo, options.reference_only))
        except (OSError, ValueError) as exc:
            rows.append({"file": str(path), "reference_integrity": "error", "error": str(exc)})
        print(json.dumps(rows[-1], ensure_ascii=False))
    if options.report:
        options.report.write_text(json.dumps(rows, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    failed = any(r.get("reference_integrity") != "pass" or
                 (not options.reference_only and r.get("custom_pcm_parity") != "pass")
                 for r in rows)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
