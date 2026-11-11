"""Restore authenticated task-9 inputs into an explicitly selected workspace.

All sources and destinations are verified against the retained predecessor report.
Existing different files and symlinked/non-directory parents are rejected.
The script does not touch the Git index or the deleted task-16 manifest.
"""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
from datetime import datetime, timezone

HIST = Path('/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/025db4e3-dd16-4743-ba66-93110c401133/workspace')
SHADOW = Path('/Users/gracefullight/workspace/arc/.agent-valley/maintenance-logs/task19-shadow-workspace/target/exec-fail-open/task19/frozen')
REPORT_HASH = '78f8a822b4fb1296eb264fcdd6618582a9018de900d94552399ce7b7d20c02ea'
COLLECTION_HASH = '3bad816a4f054f1f52fbbb7479a278c89a119f35fb8f0c074dc8166a9cb246ac'
ORIGINAL_HASH = '4456ce62e9949a3a8e1b208983ec67318cf0ff73e596c76640f7a9149f6e9366'
ROOT: Path
TARGET: Path


def select_workspace(value):
    root = Path(value)
    if not root.is_absolute() or '..' in root.parts:
        raise ValueError('workspace root must be an absolute path without ..')
    # Reject symlinks anywhere above or at the selected root as well as under it.
    for parent in reversed((root, *root.parents)):
        if not stat.S_ISDIR(parent.lstat().st_mode):
            raise ValueError(f'workspace path is not a real directory: {parent}')
    return root


def event(**item):
    print(json.dumps(item, ensure_ascii=False, sort_keys=True), flush=True)


def verify_bytes(raw, size, digest, label):
    actual = hashlib.sha256(raw).hexdigest()
    if len(raw) != size or actual != digest:
        raise RuntimeError(f'{label}: got {len(raw)} bytes and {actual}, expected {size} and {digest}')


def verify_file(path, size, digest):
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode):
        raise RuntimeError(f'not a regular non-symlink file: {path}')
    with path.open('rb') as stream:
        actual = hashlib.file_digest(stream, 'sha256').hexdigest()
    if info.st_size != size or actual != digest:
        raise RuntimeError(f'{path}: got {info.st_size} bytes and {actual}, expected {size} and {digest}')


def parent_directories(path):
    relative = path.relative_to(ROOT)
    if relative.parts[:2] != ('target', 'exec-fail-open'):
        raise RuntimeError(f'destination outside task-9 scope: {path}')
    current = ROOT
    for part in relative.parts[:-1]:
        current = current / part
        try:
            os.mkdir(current, 0o755)
        except FileExistsError:
            pass
        info = current.lstat()
        if not stat.S_ISDIR(info.st_mode):
            raise RuntimeError(f'non-directory or symlink parent: {current}')


def install(destination, raw, size, digest, source):
    verify_bytes(raw, size, digest, source)
    parent_directories(destination)
    try:
        fd = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, 'O_NOFOLLOW', 0), 0o644)
    except FileExistsError:
        verify_file(destination, size, digest)
        disposition = 'existing-identical'
    else:
        with os.fdopen(fd, 'wb') as stream:
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
        verify_file(destination, size, digest)
        disposition = 'created'
    event(kind='destination', path=str(destination), size=size, sha256=digest, source=source, result=disposition)


def shadow_bytes(relative, size, digest):
    path = SHADOW / relative
    verify_file(path, size, digest)
    raw = path.read_bytes()
    verify_bytes(raw, size, digest, str(path))
    return raw


def main():
    event(kind='start', utc=datetime.now(timezone.utc).isoformat(), workspace=str(ROOT), historicalRepository=str(HIST), shadow=str(SHADOW))
    report = (ROOT / '.arc-verify/traffic/predecessor-recovery.md').read_bytes()
    if hashlib.sha256(report).hexdigest() != REPORT_HASH:
        raise RuntimeError('predecessor recovery report hash differs')
    collection = (ROOT / '.arc-verify/exec-fail-open/implementation/collection-next.json').read_bytes()
    if hashlib.sha256(collection).hexdigest() != COLLECTION_HASH:
        raise RuntimeError('accepted collection hash differs')
    text = report.decode('utf-8')
    archive_part = text.split('## Restored task-13 archive\n', 1)[1].split('## Restored task-19 frozen inputs\n', 1)[0]
    frozen_part = text.split('## Restored task-19 frozen inputs\n', 1)[1].split('## Remaining prerequisite', 1)[0]
    archive = re.findall(r'^\| `([0-9a-f]{64})` \| ([0-9]+) \| `([^`]+)` \|$', archive_part, re.M)
    frozen = re.findall(r'^\| `([^`]+)` \| ([0-9]+) \| `([0-9a-f]{64})` \|$', frozen_part, re.M)
    if len(archive) != 26 or len(frozen) != 13:
        raise RuntimeError(f'predecessor table count differs: {len(archive)}, {len(frozen)}')
    inline = []
    for digest, size_string, source in archive:
        size = int(size_string)
        if source.startswith('inline in '):
            if source != 'inline in bf617511ec535937e756c3b9410b712830fb9a66':
                raise RuntimeError(f'unexpected inline source: {source}')
            inline.append((digest, size))
            continue
        if not re.fullmatch(r'[0-9a-f]{40}', source):
            raise RuntimeError(f'invalid blob ID: {source}')
        raw = subprocess.run(['git', '-C', str(HIST), 'cat-file', 'blob', source], check=True, stdout=subprocess.PIPE).stdout
        install(TARGET / 'task13-preserved/archive' / f'{digest}.json', raw, size, digest, f'{HIST} git blob {source}')
    if len(inline) != 2:
        raise RuntimeError(f'expected two inline entries, got {len(inline)}')
    original_path = 'target/exec-fail-open/task13-preserved/collection-next-original.json'
    original = shadow_bytes(original_path, 152867036, ORIGINAL_HASH)
    container = json.loads(original)
    if container.get('schemaVersion') != 1 or container.get('kind') != 'retained-evidence-container' or len(container.get('entries', [])) != 53:
        raise RuntimeError('original container format differs')
    for digest, size in inline:
        matches = [entry for entry in container['entries'] if entry.get('sha256') == digest]
        if len(matches) != 1 or matches[0].get('encoding') != 'base64' or matches[0].get('size') != size:
            raise RuntimeError(f'inline selector differs: {digest}')
        raw = base64.b64decode(matches[0]['bytes'], validate=True)
        install(TARGET / 'task13-preserved/archive' / f'{digest}.json', raw, size, digest, f'{SHADOW / original_path} selector {matches[0]["path"]}')
    del container, original
    original_locations = 0
    for relative, size_string, digest in frozen:
        parts = Path(relative).parts
        if Path(relative).is_absolute() or '..' in parts:
            raise RuntimeError(f'unsafe frozen path: {relative}')
        size = int(size_string)
        raw = shadow_bytes(relative, size, digest)
        source = str(SHADOW / relative)
        install(TARGET / 'task19/frozen' / relative, raw, size, digest, source)
        if parts[:2] == ('target', 'exec-fail-open'):
            install(ROOT / relative, raw, size, digest, source)
            original_locations += 1
    if original_locations != 8:
        raise RuntimeError(f'expected eight original target locations, got {original_locations}')
    event(kind='finish', status='passed', archiveFiles=26, frozenFiles=13, originalLocationFiles=8, utc=datetime.now(timezone.utc).isoformat())


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--workspace-root', required=True, help='absolute path of the authorized destination workspace')
    args = parser.parse_args()
    try:
        ROOT = select_workspace(args.workspace_root)
        TARGET = ROOT / 'target/exec-fail-open'
        main()
    except Exception as exc:
        event(kind='failure', error=repr(exc), utc=datetime.now(timezone.utc).isoformat())
        sys.exit(1)
