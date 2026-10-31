#!/usr/bin/env python3
"""Smoke-test an already-built Rust Music RBPK release executable."""

import argparse
import hashlib
import queue
import struct
import subprocess
import threading
import time
from pathlib import Path

PACKAGE_ID = b"org.rustboy.music"
MAX_FRAME_BYTES = 16 * 1024
DEADLINE_SECONDS = 3.0
STDERR_CAPTURE_BYTES = 64 * 1024


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def read_exact(stream, length):
    result = bytearray()
    while len(result) < length:
        part = stream.read(length - len(result))
        if not part:
            if not result:
                return None
            raise RuntimeError("truncated release-process frame")
        result.extend(part)
    return bytes(result)


def reply_reader(stream, messages):
    try:
        while True:
            header = read_exact(stream, 8)
            if header is None:
                messages.put(None)
                return
            require(header[:4] == b"RBPK", "release process emitted a non-RBPK frame")
            length = struct.unpack(">I", header[4:])[0]
            require(1 <= length <= MAX_FRAME_BYTES, "release reply length is outside bound")
            payload = read_exact(stream, length)
            require(payload is not None, "release process truncated a reply payload")
            messages.put(payload)
    except BaseException as error:  # propagate reader-thread failures to the caller
        messages.put(error)


def stderr_reader(stream, captured, overflow):
    while True:
        chunk = stream.read(4096)
        if not chunk:
            return
        remaining = STDERR_CAPTURE_BYTES - len(captured)
        if remaining > 0:
            captured.extend(chunk[:remaining])
        if len(chunk) > remaining:
            overflow[0] = True


def encode_request(request_id, tag, extra=b""):
    body = bytes((tag,)) + extra
    payload = (
        bytes((1, 1))
        + struct.pack(">QH", request_id, len(PACKAGE_ID))
        + PACKAGE_ID
        + body
    )
    require(len(payload) <= MAX_FRAME_BYTES, "test request exceeds frame bound")
    return b"RBPK" + struct.pack(">I", len(payload)) + payload


def send_request(stdin, request_id, tag, extra=b""):
    stdin.write(encode_request(request_id, tag, extra))
    stdin.flush()


def receive_reply(messages, expected_request_id):
    try:
        payload = messages.get(timeout=DEADLINE_SECONDS)
    except queue.Empty as error:
        raise RuntimeError("release-process reply deadline exceeded") from error
    if isinstance(payload, BaseException):
        raise RuntimeError("release-process stdout reader failed") from payload
    require(payload is not None, "release process exited before replying")
    require(len(payload) >= 13, "release reply envelope is truncated")
    version, kind = payload[:2]
    request_id = struct.unpack(">Q", payload[2:10])[0]
    package_length = struct.unpack(">H", payload[10:12])[0]
    package_end = 12 + package_length
    require(package_end <= len(payload), "release reply package ID is truncated")
    package_id = payload[12:package_end]
    body = payload[package_end:]
    require(version == 1 and kind == 2, "release reply has the wrong version or kind")
    require(request_id == expected_request_id, "release reply has the wrong request ID")
    require(package_id == PACKAGE_ID, "release reply has the wrong package ID")
    return body


def validate_empty_view(body):
    require(len(body) == 19, "Activate view has the wrong encoded size")
    require(body[0] == 2, "Activate did not return a view")
    require(body[1] == 0, "newly activated unit is not empty")
    require(body[2] == 0, "newly activated unit has a selected track")
    require(body[3:8] == bytes(5), "newly activated catalog is not empty")
    require(body[8] == 0, "newly activated unit reports an audio format")
    require(body[9:17] == bytes(8), "newly activated unit reports delivered frames")
    require(body[17:19] == bytes(2), "newly activated unit reports audio state")


def sha256_file(path):
    digest = hashlib.sha256()
    with path.open("rb") as binary:
        for chunk in iter(lambda: binary.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def run(binary):
    binary = binary.resolve(strict=True)
    require(binary.is_file(), "release binary path is not a file")
    child = subprocess.Popen(
        [str(binary), "serve"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        bufsize=0,
    )
    messages = queue.Queue(maxsize=4)
    captured_stderr = bytearray()
    stderr_overflow = [False]
    stdout_thread = threading.Thread(
        target=reply_reader, args=(child.stdout, messages), daemon=True
    )
    stderr_thread = threading.Thread(
        target=stderr_reader, args=(child.stderr, captured_stderr, stderr_overflow), daemon=True
    )
    stdout_thread.start()
    stderr_thread.start()
    try:
        send_request(child.stdin, 1, 0, struct.pack(">HH", 1, 1))
        require(receive_reply(messages, 1) == b"\x00\x00\x01", "Hello negotiation mismatch")

        send_request(child.stdin, 2, 1, struct.pack(">HH", 480, 320))
        validate_empty_view(receive_reply(messages, 2))

        send_request(child.stdin, 3, 12)
        require(receive_reply(messages, 3) == b"\x01", "Shutdown acknowledgment mismatch")
        child.stdin.close()
        try:
            exit_code = child.wait(timeout=DEADLINE_SECONDS)
        except subprocess.TimeoutExpired as error:
            raise RuntimeError("release process exit deadline exceeded") from error
        require(exit_code == 0, f"release process exited with status {exit_code}")
        require(messages.get(timeout=DEADLINE_SECONDS) is None, "release process emitted extra frames")
        stderr_thread.join(timeout=DEADLINE_SECONDS)
        require(not stderr_thread.is_alive(), "stderr drain did not finish")
        require(not stderr_overflow[0], "release process emitted excessive stderr")
        require(not captured_stderr, f"release process wrote stderr: {captured_stderr!r}")
        print(f"PASS: RBPK v1 lifecycle for {binary}")
        print(f"SHA-256: {sha256_file(binary)}")
    finally:
        if child.poll() is None:
            child.kill()
            child.wait()
        for stream in (child.stdin, child.stdout, child.stderr):
            if stream is not None and not stream.closed:
                stream.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path, help="already-built release CLI")
    arguments = parser.parse_args()
    try:
        run(arguments.binary)
    except Exception as error:
        parser.exit(1, f"FAIL: {error}\n")


if __name__ == "__main__":
    main()
