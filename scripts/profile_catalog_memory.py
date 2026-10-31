#!/usr/bin/env python3
"""Sample process RSS before/after synthetic catalog scans on macOS.

The delta is an OS-observed process RSS difference, not exact catalog heap
accounting: allocator behavior, mapped pages, filesystem metadata and system
pressure can affect it. Fixtures are either 12-byte RIFF/WAVE signatures or
minimal FLAC STREAMINFO plus bounded Vorbis comments, with no FLAC frames or
audio samples. The probe never uses user media or starts playback.
"""

import argparse
import os
import selectors
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

MAX_TRACKS = 20_000
DEFAULT_TIMEOUT_SECONDS = 30.0
MAX_PROTOCOL_LINE_BYTES = 64


def read_line(process, timeout, description):
    deadline = time.monotonic() + timeout
    data = bytearray()
    with selectors.DefaultSelector() as selector:
        selector.register(process.stdout, selectors.EVENT_READ)
        while time.monotonic() < deadline:
            if not selector.select(max(0.0, deadline - time.monotonic())):
                break
            chunk = os.read(process.stdout.fileno(), 1)
            if not chunk:
                if process.poll() is not None:
                    raise RuntimeError(
                        f"probe exited {process.returncode} before {description}"
                    )
                continue
            data.extend(chunk)
            if len(data) > MAX_PROTOCOL_LINE_BYTES:
                raise RuntimeError("probe protocol line exceeds 64 bytes")
            if chunk == b"\n":
                try:
                    return data[:-1].decode("ascii")
                except UnicodeDecodeError as error:
                    raise RuntimeError("probe protocol line is not ASCII") from error
    raise TimeoutError(f"timed out waiting for probe {description}")


def rss_kib(pid):
    result = subprocess.run(
        ["/bin/ps", "-o", "rss=", "-p", str(pid)],
        check=True,
        capture_output=True,
        text=True,
        timeout=5,
    )
    value = result.stdout.strip()
    if not value:
        raise RuntimeError(f"process {pid} disappeared during RSS sampling")
    return int(value)


def stable_rss_kib(pid, samples=3, interval=0.1):
    values = []
    for index in range(samples):
        values.append(rss_kib(pid))
        if index + 1 < samples:
            time.sleep(interval)
    return int(statistics.median(values)), values


def parse_ready(line, expected_tracks, expected_tagged, expected_pid):
    fields = dict(item.split("=", 1) for item in line.split()[1:])
    if line.split(maxsplit=1)[0] != "READY":
        raise RuntimeError(f"unexpected probe message: {line!r}")
    if int(fields.get("pid", "-1")) != expected_pid:
        raise RuntimeError("probe PID does not match launched process")
    if int(fields.get("tracks", "-1")) != expected_tracks:
        raise RuntimeError("probe reported a different track count")
    if int(fields.get("tagged", "-1")) != expected_tagged:
        raise RuntimeError("probe reported a different tagged count")


def run_once(binary, count, tagged_count, timeout):
    fixture_root = Path(tempfile.mkdtemp(prefix="rust-music-rss-profile-"))
    process = None
    result = None
    failure = None
    cleanup_errors = []
    try:
        process = subprocess.Popen(
            [
                str(binary),
                "--root",
                str(fixture_root),
                "--tracks",
                str(count),
                "--tagged-count",
                str(tagged_count),
            ],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=False,
        )
        ready = read_line(process, timeout, "READY")
        parse_ready(ready, count, tagged_count, process.pid)
        before, before_samples = stable_rss_kib(process.pid)

        process.stdin.write(b"scan\n")
        process.stdin.flush()
        scanned = read_line(process, timeout, "SCANNED")
        if scanned != f"SCANNED tracks={count} tagged={tagged_count}":
            raise RuntimeError(f"unexpected scan result: {scanned!r}")
        after, after_samples = stable_rss_kib(process.pid)

        process.stdin.write(b"release\n")
        process.stdin.flush()
        released = read_line(process, timeout, "RELEASED")
        if released != "RELEASED":
            raise RuntimeError(f"unexpected probe result: {released!r}")
        status = process.wait(timeout=timeout)
        if status != 0:
            raise RuntimeError(f"probe exited with status {status}")
        result = {
            "before_kib": before,
            "after_kib": after,
            "delta_kib": after - before,
            "before_samples": before_samples,
            "after_samples": after_samples,
        }
    except BaseException as error:
        failure = (error, error.__traceback__)

    if process is not None:
        if process.poll() is None:
            try:
                process.kill()
            except ProcessLookupError:
                # It exited after poll(); still wait below to reap it.
                pass
            except OSError as error:
                cleanup_errors.append(f"process kill failed: {error}")
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            try:
                process.kill()
            except ProcessLookupError:
                pass
            except OSError as error:
                cleanup_errors.append(f"process kill after timeout failed: {error}")
            try:
                process.wait(timeout=5)
            except BaseException as error:
                cleanup_errors.append(f"process reap failed: {error}")
        except BaseException as error:
            cleanup_errors.append(f"process reap failed: {error}")
        for stream in (process.stdin, process.stdout, process.stderr):
            if stream is not None:
                try:
                    stream.close()
                except BaseException as error:
                    cleanup_errors.append(f"stream close failed: {error}")

    try:
        shutil.rmtree(fixture_root)
    except FileNotFoundError:
        pass
    except BaseException as error:
        cleanup_errors.append(f"temporary root cleanup failed: {error}")

    if failure is not None:
        error, traceback = failure
        if cleanup_errors:
            message = "cleanup errors: " + "; ".join(cleanup_errors)
            if hasattr(error, "add_note"):
                error.add_note(message)
            else:
                print(message, file=sys.stderr)
        raise error.with_traceback(traceback)
    if cleanup_errors:
        raise RuntimeError("; ".join(cleanup_errors))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path, help="built profile_micro_library_memory binary")
    parser.add_argument(
        "--counts",
        type=int,
        nargs="+",
        default=[2_000, 20_000],
        help=f"synthetic file counts (each must be 0..{MAX_TRACKS})",
    )
    parser.add_argument(
        "--tagged-count",
        type=int,
        default=0,
        help="number of files with synthetic FLAC TITLE/ARTIST/ALBUM comments",
    )
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--timeout", type=float, default=DEFAULT_TIMEOUT_SECONDS)
    args = parser.parse_args()

    if sys.platform != "darwin":
        parser.error("this probe uses macOS /bin/ps RSS units")
    binary = args.binary.resolve(strict=True)
    if not binary.is_file():
        parser.error("binary must be a file")
    if not args.counts or any(count < 0 or count > MAX_TRACKS for count in args.counts):
        parser.error(f"every count must be between 0 and {MAX_TRACKS}")
    if args.tagged_count < 0 or any(args.tagged_count > count for count in args.counts):
        parser.error("tagged-count must be between 0 and every requested count")
    if args.repeats < 1 or args.repeats > 10:
        parser.error("repeats must be between 1 and 10")
    if args.timeout <= 0 or args.timeout > 600:
        parser.error("timeout must be greater than 0 and no more than 600 seconds")

    print(
        "RSS delta is an OS-observed process difference, not exact catalog allocation; "
        "tagged fixtures contain bounded Vorbis comments and no FLAC frames."
    )
    for count in args.counts:
        rows = []
        for repeat in range(1, args.repeats + 1):
            row = run_once(binary, count, args.tagged_count, args.timeout)
            rows.append(row)
            print(
                f"tracks={count} tagged={args.tagged_count} run={repeat} "
                f"baseline_rss_kib={row['before_kib']} "
                f"after_scan_rss_kib={row['after_kib']} "
                f"incremental_rss_estimate_kib={row['delta_kib']} "
                f"baseline_samples_kib={row['before_samples']} "
                f"after_samples_kib={row['after_samples']}"
            )
        deltas = [row["delta_kib"] for row in rows]
        print(
            f"tracks={count} tagged={args.tagged_count} repeats={args.repeats} "
            f"median_incremental_rss_estimate_kib={statistics.median(deltas)} "
            f"range_kib={min(deltas)}..{max(deltas)}"
        )


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"FAIL: {error}", file=sys.stderr)
        raise SystemExit(1)
