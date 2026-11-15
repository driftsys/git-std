#!/usr/bin/env python3
"""Collect this build's man pages using Cargo's public JSON messages."""
import argparse
import json
from pathlib import Path, PurePosixPath
import shutil
import sys


def collect(metadata, messages, destination, is_cross):
    """Match the binary manifest, then select its build-script package ID."""
    def host_path(value):
        if is_cross:
            # cross v0.2.5 mounts the target directory at /target and may mount
            # the workspace at /project instead of its original host path.
            path = PurePosixPath(value)
            for container, host in [('/target', metadata['target_directory']),
                                    ('/project', metadata['workspace_root'])]:
                if path.is_relative_to(container):
                    return Path(host).joinpath(*path.relative_to(container).parts).resolve()
        return Path(value).resolve()

    finished = [row for row in messages if row.get('reason') == 'build-finished']
    if len(finished) != 1 or finished[0].get('success') is not True:
        raise ValueError('expected one successful build')
    manifests = {Path(package['manifest_path']).resolve()
                 for package in metadata['packages'] if package['name'] == 'git-std'}
    if len(manifests) != 1:
        raise ValueError('expected one git-std package in Cargo metadata')
    packages = {row['package_id'] for row in messages
                if row.get('reason') == 'compiler-artifact'
                and row.get('target', {}).get('name') == 'git-std'
                and row['target'].get('kind') == ['bin']
                and host_path(row['manifest_path']) in manifests}
    if len(packages) != 1:
        raise ValueError('expected one git-std binary artifact for the workspace manifest')
    outputs = {host_path(row['out_dir']) for row in messages
               if row.get('reason') == 'build-script-executed'
               and row.get('package_id') in packages}
    if len(outputs) != 1:
        raise ValueError('expected one git-std build-script output directory')
    source = outputs.pop() / 'man'
    if not (source / 'git-std.1').is_file():
        raise ValueError(f'git-std.1 not found in reported build output: {source}')
    pages = sorted(source.glob('git-std*.1'))
    destination.mkdir(parents=True, exist_ok=True)
    # Remove only previously staged man pages, never unrelated user files.
    for stale in destination.glob('git-std*.1'):
        stale.unlink()
    for page in pages:
        shutil.copyfile(page, destination / page.name)


def main():
    """Read a successful Cargo build stream and stage its generated pages."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('metadata', type=Path)
    parser.add_argument('messages', type=Path)
    parser.add_argument('destination', type=Path)
    parser.add_argument('--cross', action='store_true')
    args = parser.parse_args()
    try:
        metadata = json.loads(args.metadata.read_text(encoding='utf-8'))
        messages = [json.loads(line) for line in
                    args.messages.read_text(encoding='utf-8').splitlines()
                    if line.startswith('{')]
        collect(metadata, messages, args.destination, args.cross)
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.exit(1, f'error: {error}\n')


if __name__ == '__main__':
    main()
