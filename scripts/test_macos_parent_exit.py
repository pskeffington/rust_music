#!/usr/bin/env python3
"""Verify a playback child exits when its app-like parent is force-killed."""

import argparse
import os
import signal
import subprocess
import tempfile
import time
from pathlib import Path


def send_child_pid(fd, pid):
    os.write(fd, f"{pid}\n".encode("ascii"))


def read_line(fd, timeout_seconds):
    deadline = time.monotonic() + timeout_seconds
    data = bytearray()
    while time.monotonic() < deadline:
        try:
            part = os.read(fd, 1)
        except InterruptedError:
            continue
        if not part:
            break
        if part == b"\n":
            return bytes(data).decode("ascii")
        data.extend(part)
    raise RuntimeError("supervisor did not report the playback child PID")


def process_exists(pid):
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def wait_for_exit(pid, timeout_seconds):
    deadline = time.monotonic() + timeout_seconds
    while time.monotonic() < deadline:
        if not process_exists(pid):
            return
        time.sleep(0.02)
    raise RuntimeError(f"playback child PID {pid} remained after the parent was killed")


def run(binary):
    binary = binary.resolve(strict=True)
    if not binary.is_file():
        raise RuntimeError("Rust Music CLI is not a file")

    with tempfile.TemporaryDirectory(prefix="rust-music-parent-exit-") as directory:
        marker = Path(directory) / "playback-child.stdout"
        report_read, report_write = os.pipe()
        supervisor_pid = os.fork()
        if supervisor_pid == 0:
            os.close(report_read)
            try:
                with marker.open("wb", buffering=0) as output:
                    child = subprocess.Popen(
                        [str(binary), "__test-parent-guard"],
                        stdin=subprocess.PIPE,
                        stdout=output,
                        stderr=subprocess.DEVNULL,
                        close_fds=True,
                    )
                    send_child_pid(report_write, child.pid)
                    while True:
                        time.sleep(60)
            finally:
                os._exit(0)

        os.close(report_write)
        child_pid = None
        try:
            child_pid = int(read_line(report_read, 5))
            os.kill(supervisor_pid, signal.SIGKILL)
            _, status = os.waitpid(supervisor_pid, 0)
            if not os.WIFSIGNALED(status) or os.WTERMSIG(status) != signal.SIGKILL:
                raise RuntimeError("app-like supervisor was not observed dying by SIGKILL")

            wait_for_exit(child_pid, 8)
            output = marker.read_text(encoding="utf-8")
            if output != "parent-stdin-closed\n":
                raise RuntimeError(f"playback child did not observe parent EOF: {output!r}")
            print(f"PASS: force-killed app-like parent; playback child PID {child_pid} exited")
        finally:
            os.close(report_read)
            if process_exists(supervisor_pid):
                os.kill(supervisor_pid, signal.SIGKILL)
                os.waitpid(supervisor_pid, 0)
            if child_pid is not None and process_exists(child_pid):
                os.kill(child_pid, signal.SIGKILL)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path, help="debug CLI with macos-output")
    args = parser.parse_args()
    try:
        run(args.binary)
    except Exception as error:
        parser.exit(1, f"FAIL: {error}\n")


if __name__ == "__main__":
    main()
