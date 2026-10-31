#!/usr/bin/env python3
"""Verify normal, failed, repeated, and forced-close package-child cleanup."""

import argparse
import os
import selectors
import signal
import subprocess
import time
from pathlib import Path

PACKAGE_ID = b"org.rustboy.music"
PROCESS_DEADLINE_SECONDS = 5


def encode_hello():
    payload = (
        bytes((1, 1))
        + (1).to_bytes(8, "big")
        + len(PACKAGE_ID).to_bytes(2, "big")
        + PACKAGE_ID
        + bytes((0,))
        + (1).to_bytes(2, "big")
        + (1).to_bytes(2, "big")
    )
    return b"RBPK" + len(payload).to_bytes(4, "big") + payload


def read_exact(stream, count, timeout=PROCESS_DEADLINE_SECONDS):
    deadline = time.monotonic() + timeout
    result = bytearray()
    with selectors.DefaultSelector() as selector:
        selector.register(stream, selectors.EVENT_READ)
        while len(result) < count:
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not selector.select(remaining):
                raise RuntimeError("serve child hello reply deadline exceeded")
            chunk = os.read(stream.fileno(), count - len(result))
            if not chunk:
                raise RuntimeError("serve child exited before completing its hello reply")
            result.extend(chunk)
    return bytes(result)


def launch_serve(binary):
    return subprocess.Popen(
        [str(binary), "serve"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        close_fds=True,
    )


def wait_for_child(child, expected_status=None):
    try:
        status = child.wait(timeout=PROCESS_DEADLINE_SECONDS)
    except subprocess.TimeoutExpired as error:
        child.kill()
        try:
            child.wait(timeout=PROCESS_DEADLINE_SECONDS)
        except subprocess.TimeoutExpired as reap_error:
            raise RuntimeError(
                f"serve child PID {child.pid} could not be reaped after SIGKILL"
            ) from reap_error
        raise RuntimeError(f"serve child PID {child.pid} did not exit before deadline") from error
    if expected_status is not None and status != expected_status:
        raise RuntimeError(f"serve child PID {child.pid} exited {status}, expected {expected_status}")
    return status


def kill_and_reap(child):
    if child.poll() is None:
        child.kill()
        try:
            child.wait(timeout=PROCESS_DEADLINE_SECONDS)
        except subprocess.TimeoutExpired as error:
            raise RuntimeError(f"serve child PID {child.pid} remained after SIGKILL") from error


def test_normal_serve_shutdown(binary):
    child = launch_serve(binary)
    try:
        child.stdin.write(encode_hello())
        child.stdin.flush()
        header = read_exact(child.stdout, 8)
        if header[:4] != b"RBPK":
            raise RuntimeError("serve child returned a non-RBPK hello frame")
        length = int.from_bytes(header[4:], "big")
        if not 1 <= length <= 16_384:
            raise RuntimeError(f"serve child returned invalid hello length {length}")
        reply = read_exact(child.stdout, length)
        if (
            reply[0:2] != bytes((1, 2))
            or int.from_bytes(reply[2:10], "big") != 1
            or int.from_bytes(reply[10:12], "big") != len(PACKAGE_ID)
            or reply[12 : 12 + len(PACKAGE_ID)] != PACKAGE_ID
        ):
            raise RuntimeError("serve child returned an invalid hello envelope")
        if reply[12 + len(PACKAGE_ID) :] != bytes((0, 0, 1)):
            raise RuntimeError("serve child did not negotiate RBPK protocol version 1")
        child.stdin.close()
        wait_for_child(child, expected_status=0)
        print(f"PASS: normal EOF reaped real serve child PID {child.pid} (exit 0)")
    finally:
        kill_and_reap(child)
        if child.stdin and not child.stdin.closed:
            child.stdin.close()
        child.stdout.close()


def test_protocol_error_shutdown(binary):
    child = launch_serve(binary)
    try:
        child.stdin.write(b"bad!")
        child.stdin.flush()
        child.stdin.close()
        status = wait_for_child(child)
        if status == 0:
            raise RuntimeError("malformed package input unexpectedly exited successfully")
        print(f"PASS: malformed protocol reaped real serve child PID {child.pid} (exit {status})")
    finally:
        kill_and_reap(child)
        if child.stdin and not child.stdin.closed:
            child.stdin.close()
        child.stdout.close()


def test_repeated_serve_shutdown(binary, count=3):
    pids = []
    for _ in range(count):
        child = launch_serve(binary)
        pids.append(child.pid)
        child.stdin.close()
        wait_for_child(child, expected_status=0)
        child.stdout.close()
    print(f"PASS: {count} repeated serve launch/EOF/exit cycles reaped PIDs {pids}")


def send_child_pid(fd, pid):
    os.write(fd, f"{pid}\n".encode("ascii"))


def read_line(fd, timeout_seconds):
    deadline = time.monotonic() + timeout_seconds
    data = bytearray()
    with selectors.DefaultSelector() as selector:
        selector.register(fd, selectors.EVENT_READ)
        while time.monotonic() < deadline:
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not selector.select(remaining):
                break
            part = os.read(fd, 1)
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


def reap_supervisor(pid, timeout_seconds):
    deadline = time.monotonic() + timeout_seconds
    while time.monotonic() < deadline:
        try:
            result, status = os.waitpid(pid, os.WNOHANG)
        except ChildProcessError:
            return
        if result == pid:
            if not os.WIFSIGNALED(status) or os.WTERMSIG(status) != signal.SIGKILL:
                raise RuntimeError("app-like supervisor did not exit from SIGKILL")
            return
        time.sleep(0.02)
    raise RuntimeError(f"app-like supervisor PID {pid} was not reaped before deadline")


def test_force_killed_parent(binary):
    report_read, report_write = os.pipe()
    supervisor_pid = os.fork()
    if supervisor_pid == 0:
        os.close(report_read)
        try:
            child = subprocess.Popen(
                [str(binary), "serve"],
                stdin=subprocess.PIPE,
                stdout=subprocess.DEVNULL,
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
        reap_supervisor(supervisor_pid, PROCESS_DEADLINE_SECONDS)
        wait_for_exit(child_pid, 8)
        print(f"PASS: force-killed app-like parent; real serve child PID {child_pid} exited")
    finally:
        os.close(report_read)
        if process_exists(supervisor_pid):
            os.kill(supervisor_pid, signal.SIGKILL)
            reap_supervisor(supervisor_pid, PROCESS_DEADLINE_SECONDS)
        if child_pid is not None and process_exists(child_pid):
            os.kill(child_pid, signal.SIGKILL)
            wait_for_exit(child_pid, PROCESS_DEADLINE_SECONDS)


def run(binary):
    binary = binary.resolve(strict=True)
    if not binary.is_file():
        raise RuntimeError("Rust Music CLI is not a file")

    test_normal_serve_shutdown(binary)
    test_protocol_error_shutdown(binary)
    test_repeated_serve_shutdown(binary)
    test_force_killed_parent(binary)


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
