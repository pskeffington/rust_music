#!/usr/bin/env python3
"""Validate private, user-provided FLAC files without checking recordings into git.

Modes:
    oracle  - reference FLAC integrity plus independent FLAC/FFmpeg PCM equality
    rust    - offline Rust_Music CLI decode vs committed expected PCM SHA-256
    all     - run both

The Rust mode requires only the locally installed Rust toolchain and the
self-authored workspace; reference executables are optional, test-only oracles.
No audio is uploaded, copied into this repository, or sent over the network.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import wave

REPO = pathlib.Path(__file__).resolve().parents[1]
MANIFEST = REPO / 'tests' / 'corpus' / 'guiezio-flac-sha256.json'
MAX_METADATA_BLOCKS = 128
MAX_METADATA_SIZE = 16 * 1024 * 1024
MAX_TOTAL_METADATA = 64 * 1024 * 1024


def sha256_file(path: pathlib.Path) -> str:
    h = hashlib.sha256()
    with path.open('rb') as source:
        for block in iter(lambda: source.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def sha256_output(argv: list[str]) -> tuple[str, int]:
    """Hash a decoder's raw PCM without buffering the decoded track in RAM."""
    process = subprocess.Popen(argv, cwd=REPO, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE)
    assert process.stdout is not None
    digest = hashlib.sha256()
    total = 0
    while True:
        block = process.stdout.read(1024 * 1024)
        if not block:
            break
        digest.update(block)
        total += len(block)
    assert process.stderr is not None
    stderr = process.stderr.read().decode('utf-8', errors='replace')
    if process.wait() != 0:
        raise RuntimeError(f"decoder failed: {argv[0]}: {stderr[-600:]}")
    return digest.hexdigest(), total


def parse_flac_metadata(path: pathlib.Path) -> dict:
    size = path.stat().st_size
    with path.open('rb') as source:
        if source.read(4) != b'fLaC':
            raise ValueError('not a native FLAC stream')
        offset = 4
        types = []
        total_metadata = 0
        streaminfo = None
        for i in range(MAX_METADATA_BLOCKS):
            header = source.read(4)
            if len(header) != 4:
                raise ValueError('truncated metadata header')
            final = bool(header[0] & 128)
            kind = header[0] & 127
            length = int.from_bytes(header[1:], 'big')
            if length > MAX_METADATA_SIZE:
                raise ValueError('overlong metadata block')
            total_metadata += length + 4
            if total_metadata > MAX_TOTAL_METADATA or offset + 4 + length > size:
                raise ValueError('invalid metadata size')
            types.append(kind)
            if i == 0:
                if kind != 0 or length != 34:
                    raise ValueError('STREAMINFO is not the first metadata block')
                raw = source.read(34)
                packed = int.from_bytes(raw[10:18], 'big')
                streaminfo = {
                    'sample_rate_hz': (packed >> 44) & 0xfffff,
                    'channels': ((packed >> 41) & 7) + 1,
                    'bits_per_sample': ((packed >> 36) & 31) + 1,
                    'total_samples_per_channel': packed & ((1 << 36) - 1),
                    'streaminfo_pcm_md5': raw[18:34].hex(),
                }
            else:
                source.seek(length, os.SEEK_CUR)
            offset += length + 4
            if final:
                break
        else:
            raise ValueError('too many metadata blocks')
        assert streaminfo is not None
        streaminfo['metadata_types'] = types
        return streaminfo


def reference_check(path: pathlib.Path, case: dict) -> dict:
    if not shutil.which('flac') or not shutil.which('ffmpeg'):
        raise RuntimeError('oracle mode needs test-only flac and ffmpeg executables')
    result = subprocess.run(['flac', '--test', '--silent', str(path)],
                            capture_output=True, text=True, check=False)
    if result.returncode != 0:
        raise AssertionError(f'FLAC CRC/MD5 rejected {case["id"]}: {result.stderr[-500:]}')
    pcm_flac = sha256_output(['flac', '--decode', '--stdout', '--force-raw-format',
                              '--endian=little', '--sign=signed', '--silent', str(path)])
    pcm_ffmpeg = sha256_output(['ffmpeg', '-nostdin', '-hide_banner', '-loglevel',
                                'error', '-i', str(path), '-map', '0:a:0',
                                '-f', 's16le', '-c:a', 'pcm_s16le', '-'])
    expected = (case['pcm_s16le_sha256'], case['pcm_bytes'])
    if pcm_flac != expected:
        raise AssertionError(f'FLAC decoded PCM mismatch for {case["id"]}')
    if pcm_ffmpeg != expected:
        raise AssertionError(f'FFmpeg decoded PCM mismatch for {case["id"]}')
    return {'flac_integrity': 'pass', 'flac_pcm_sha256': pcm_flac[0],
            'ffmpeg_pcm_sha256': pcm_ffmpeg[0], 'reference_pcm_equal': True}


def reference_pcm_file(path: pathlib.Path, destination: pathlib.Path) -> None:
    result = subprocess.run(['ffmpeg', '-nostdin', '-hide_banner', '-loglevel',
                             'error', '-i', str(path), '-map', '0:a:0',
                             '-f', 's16le', '-c:a', 'pcm_s16le', str(destination)],
                            cwd=REPO, capture_output=True, text=True, check=False)
    if result.returncode != 0:
        raise RuntimeError(f'FFmpeg seek oracle failed: {result.stderr[-600:]}')


def flac_seekpoints(path: pathlib.Path) -> list[int]:
    points = []
    size = path.stat().st_size
    with path.open('rb') as source:
        if source.read(4) != b'fLaC':
            raise ValueError('not a native FLAC stream')
        for _ in range(MAX_METADATA_BLOCKS):
            header = source.read(4)
            if len(header) != 4:
                raise ValueError('truncated metadata header while reading SEEKTABLE')
            final = bool(header[0] & 128)
            kind = header[0] & 127
            length = int.from_bytes(header[1:], 'big')
            if length > MAX_METADATA_SIZE or source.tell() + length > size:
                raise ValueError('invalid metadata size while reading SEEKTABLE')
            if kind == 3:
                if length % 18:
                    raise ValueError('SEEKTABLE length is not a multiple of 18 bytes')
                for _ in range(length // 18):
                    point = source.read(18)
                    if len(point) != 18:
                        raise ValueError('truncated SEEKTABLE point')
                    sample = int.from_bytes(point[:8], 'big')
                    if sample != (1 << 64) - 1:
                        points.append(sample)
            else:
                source.seek(length, os.SEEK_CUR)
            if final:
                break
        else:
            raise ValueError('too many metadata blocks while reading SEEKTABLE')
    if not points or points != sorted(set(points)):
        raise ValueError('corpus file has no strictly increasing SEEKTABLE points')
    return points


def verify_seek_ranges(binary: pathlib.Path, path: pathlib.Path, case: dict,
                       temp_dir: pathlib.Path) -> int:
    reference = temp_dir / f'{case["id"]}-seek-reference.pcm'
    reference_pcm_file(path, reference)
    if reference.stat().st_size != case['pcm_bytes']:
        raise AssertionError(f'FFmpeg seek oracle length differs for {case["id"]}')

    total_frames = case['total_samples_per_channel']
    bytes_per_frame = case['channels'] * (case['bits_per_sample'] // 8)
    if bytes_per_frame != 4:
        raise AssertionError('the private seek corpus oracle currently expects stereo s16le')
    targets = set()
    for point in flac_seekpoints(path):
        if point < total_frames:
            targets.add(point)
        if point + 1 < total_frames:
            targets.add(point + 1)

    with reference.open('rb') as expected_pcm:
        for index, target in enumerate(sorted(targets)):
            count = min(257, total_frames - target)
            output = temp_dir / f'{case["id"]}-seek-{index}.wav'
            result = subprocess.run(
                [str(binary), 'decode-range', str(path), str(target), str(count), str(output)],
                cwd=REPO, capture_output=True, text=True, check=False)
            if result.returncode != 0:
                raise AssertionError(
                    f'Rust seek failed for {case["id"]} at frame {target}: {result.stderr[-600:]}')
            with wave.open(str(output), 'rb') as decoded:
                attributes = (decoded.getframerate(), decoded.getnchannels(),
                              decoded.getsampwidth() * 8, decoded.getnframes())
                expected_attributes = (case['sample_rate_hz'], case['channels'],
                                       case['bits_per_sample'], count)
                if attributes != expected_attributes:
                    raise AssertionError(
                        f'seek WAV format differs for {case["id"]}: {attributes} != {expected_attributes}')
                actual = decoded.readframes(count)
            expected_pcm.seek(target * bytes_per_frame)
            expected = expected_pcm.read(count * bytes_per_frame)
            if actual != expected:
                raise AssertionError(
                    f'sought PCM differs for {case["id"]} at frame {target}')
            output.unlink()
    return len(targets)


def cli_check(binary: pathlib.Path, path: pathlib.Path, case: dict,
              temp_dir: pathlib.Path, verify_seek: bool = False) -> dict:
    inspection = subprocess.run([str(binary), 'inspect', str(path)], cwd=REPO,
                                capture_output=True, text=True, check=False)
    if inspection.returncode != 0:
        raise AssertionError(f'Rust inspect failed for {case["id"]}: {inspection.stderr[-600:]}')
    wav_file = temp_dir / f'{case["id"]}.wav'
    decoded = subprocess.run([str(binary), 'decode', str(path), str(wav_file)],
                             cwd=REPO, capture_output=True, text=True, check=False)
    if decoded.returncode != 0:
        raise AssertionError(f'Rust decode failed for {case["id"]}: {decoded.stderr[-600:]}')
    h = hashlib.sha256()
    with wave.open(str(wav_file), 'rb') as result:
        attributes = (result.getframerate(), result.getnchannels(),
                      result.getsampwidth() * 8, result.getnframes())
        expected_attrs = (case['sample_rate_hz'], case['channels'],
                          case['bits_per_sample'], case['total_samples_per_channel'])
        if attributes != expected_attrs:
            raise AssertionError(f'Rust output format differs: {attributes} != {expected_attrs}')
        total = 0
        while True:
            chunk = result.readframes(65_536)
            if not chunk:
                break
            h.update(chunk)
            total += len(chunk)
    wav_file.unlink()
    if h.hexdigest() != case['pcm_s16le_sha256'] or total != case['pcm_bytes']:
        raise AssertionError(f'Rust PCM SHA-256 or byte count differs for {case["id"]}')
    result = {'rust_inspect': 'pass', 'rust_pcm_sha256': h.hexdigest(),
              'rust_pcm_equal': True}
    if verify_seek:
        result['real_seek_positions_pcm_equal'] = verify_seek_ranges(
            binary, path, case, temp_dir)
    return result


def files_from_args(inputs: list[str]) -> list[pathlib.Path]:
    items = []
    for item in inputs:
        path = pathlib.Path(item)
        if path.is_dir():
            items.extend(sorted(p for p in path.iterdir() if p.suffix.lower() == '.flac'))
        elif path.is_file():
            items.append(path)
        else:
            raise FileNotFoundError(f'cannot read supplied file or directory: {path}')
    unique = {}
    for item in items:
        if item.is_symlink():
            raise ValueError(f'symlink supplied, refusing: {item}')
        unique[item.resolve()] = item
    return list(unique.values())


def run() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('inputs', nargs='+', help='private FLAC file(s) or one directory (nonrecursive)')
    parser.add_argument('--mode', choices=('oracle', 'rust', 'all'), default='all')
    parser.add_argument('--require-complete', action='store_true',
                        help='require all six known track SHA-256 digests')
    parser.add_argument('--verify-seek', action='store_true',
                        help='compare Rust seek output at every real SEEKTABLE point and adjacent sample against FFmpeg; requires --mode all')
    parser.add_argument('--report', type=pathlib.Path,
                        help='write a hash-only machine-readable report; never writes audio')
    args = parser.parse_args()
    if args.verify_seek and args.mode != 'all':
        parser.error('--verify-seek requires --mode all for independent PCM reference')
    manifest = json.loads(MANIFEST.read_text(encoding='utf-8'))
    assert manifest['schema_version'] == 1
    cases = {entry['flac_sha256']: entry for entry in manifest['cases']}
    if len(cases) != len(manifest['cases']):
        raise ValueError('duplicate corpus digests')

    binary = None
    if args.mode in {'rust', 'all'}:
        cargo = shutil.which('cargo')
        if cargo is None:
            raise RuntimeError('cargo missing: full Rust parity unavailable; use --mode oracle for reference-only checks')
        build = subprocess.run([cargo, 'build', '--offline', '--locked', '--quiet',
                                '-p', 'rust-music-cli'], cwd=REPO, capture_output=True,
                               text=True, check=False)
        if build.returncode != 0:
            raise RuntimeError(f'offline Rust build failed: {build.stderr[-900:]}')
        target_dir = pathlib.Path(os.environ.get('CARGO_TARGET_DIR', str(REPO / 'target')))
        binary = target_dir / 'debug' / ('rust-music-cli.exe' if os.name == 'nt' else 'rust-music-cli')
        if not binary.is_file():
            raise FileNotFoundError(f'compiled Rust CLI not found: {binary}')

    checked = set()
    results = []
    selected = files_from_args(args.inputs)
    if not selected:
        raise ValueError('no FLAC files found')
    with tempfile.TemporaryDirectory(prefix='rust-music-private-corpus-') as tmp:
        temp_dir = pathlib.Path(tmp)
        for path in selected:
            digest = sha256_file(path)
            case = cases.get(digest)
            if case is None:
                raise ValueError(f'unrecognized FLAC digest (not in corpus): {path.name}; do not silently skip unknown inputs')
            if case['id'] in checked:
                raise ValueError(f'duplicate corpus track {case["id"]}')
            checked.add(case['id'])
            if path.stat().st_size != case['flac_size_bytes']:
                raise AssertionError(f'compressed byte length differs for {case["id"]}')
            metadata = parse_flac_metadata(path)
            for field, value in metadata.items():
                if case[field] != value:
                    raise AssertionError(f'FLAC metadata mismatch for {case["id"]}: {field}')
            result = {'case': case['id'], 'flac_sha256': digest,
                      'streaminfo_md5': case['streaminfo_pcm_md5'],
                      'pcm_sha256_expected': case['pcm_s16le_sha256'],
                      'metadata': 'pass'}
            if args.mode in {'oracle', 'all'}:
                result.update(reference_check(path, case))
            if args.mode in {'rust', 'all'}:
                assert binary is not None
                result.update(cli_check(binary, path, case, temp_dir,
                                        verify_seek=args.verify_seek))
            results.append(result)
            seek_summary = ''
            if 'real_seek_positions_pcm_equal' in result:
                seek_summary = (
                    f', {result["real_seek_positions_pcm_equal"]} real seek ranges')
            print(f'PASS {case["id"]}: metadata, SHA-256'
                  f'{", reference FLAC/FFmpeg" if "flac_integrity" in result else ""}'
                  f'{", Rust PCM" if "rust_inspect" in result else ""}'
                  f'{seek_summary}', flush=True)
    missing = sorted({case['id'] for case in manifest['cases']} - checked)
    if args.require_complete and missing:
        raise AssertionError(f'missing corpus tracks: {missing}')
    report = {'schema_version': 1, 'mode': args.mode, 'all_passed': True,
              'passed': len(results), 'expected': len(cases), 'missing_ids': missing,
              'rust_decoder_exercised': args.mode in {'rust', 'all'},
              'audio_files_committed': False, 'cases': results}
    if args.report is not None:
        with args.report.open('x', encoding='utf-8') as dest:
            json.dump(report, dest, indent=2)
            dest.write('\n')
    print(f'PASS: {len(results)} FLAC tracks in {args.mode} mode. '
          f'Rust decoder exercised: {report["rust_decoder_exercised"]}.')
    return 0


if __name__ == '__main__':
    try:
        sys.exit(run())
    except (AssertionError, FileNotFoundError, RuntimeError, ValueError, OSError) as exc:
        print(f'FAIL: {exc}', file=sys.stderr)
        sys.exit(1)
