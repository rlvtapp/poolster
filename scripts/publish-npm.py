#!/usr/bin/env python3
"""Publish packed npm artifacts; recover only when existing bytes match exactly."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import subprocess
import tarfile


def publish(path, tag, run=subprocess.run):
    path = Path(path)
    with tarfile.open(path, 'r:gz') as archive:
        manifest = json.load(archive.extractfile('package/package.json'))
    package = f"{manifest['name']}@{manifest['version']}"
    result = run(['npm', 'view', package, 'dist.integrity', '--json'], capture_output=True, text=True)
    if result.returncode == 0:
        integrity = json.loads(result.stdout)
        expected = 'sha512-' + base64.b64encode(hashlib.sha512(path.read_bytes()).digest()).decode()
        if integrity != expected:
            raise RuntimeError(f'{package} already exists with different artifact bytes; refusing recovery')
        print(f'{package}: identical artifact already published; preserving existing dist-tags', flush=True)
        return
    try:
        error = json.loads(result.stdout).get('error', {})
    except (ValueError, AttributeError):
        error = {}
    if error.get('code') != 'E404':
        raise RuntimeError(f'Cannot check {package}: {result.stderr or result.stdout}')
    print(f'Publishing {package}', flush=True)
    run(['npm', 'publish', str(path), '--ignore-scripts', '--access', 'public', '--tag', tag], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tag', required=True)
    parser.add_argument('artifacts', nargs='+', type=Path)
    args = parser.parse_args()
    # Check all input files before the first irreversible registry write.
    for path in args.artifacts:
        if not path.is_file():
            parser.error(f'missing artifact: {path}')
    for path in args.artifacts:
        publish(path, args.tag)


if __name__ == '__main__':
    main()
