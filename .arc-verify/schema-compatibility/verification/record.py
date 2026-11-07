#!/usr/bin/env python3
"""Record exact commands against audited source inputs; writes only verification/."""
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import platform
import resource
import signal
import subprocess
import sys
import time

ROOT = Path.cwd()
OUT = ROOT / '.arc-verify/schema-compatibility/verification'
BASE = 'bf28a35b594f5ae83ac97c216bd3b09e60a2dfa1'


def now():
    return dt.datetime.now(dt.timezone.utc).isoformat()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def save(name, obj):
    (OUT / name).write_text(json.dumps(obj, indent=2) + '\n')


def manifest():
    paths = {ROOT / p for p in ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
             'scripts/harness/validate.sh', 'docs/schema-compatibility-contract.md',
             'docs/bounded-ipc-recovery.md']}
    # Hash/length audit inventory, not source search or symbol discovery.
    paths.update(p for p in (ROOT / 'crates').rglob('*')
                 if p.is_file() and p.suffix in {'.rs', '.toml', '.sql'})
    return {str(p.relative_to(ROOT)): {'sha256': digest(p.read_bytes()),
            'bytes': p.stat().st_size, 'lines': len(p.read_bytes().splitlines())}
            for p in sorted(paths)}


def run(label, command):
    before = manifest()
    started = now()
    began = time.monotonic()
    env = os.environ.copy()
    env['CARGO_NET_OFFLINE'] = 'true'
    env.pop('SKIP_COVERAGE', None)
    if label == 'harness':
        # The unchanged harness owns/creates its scratch directory; retain it.
        env.pop('WORKSPACE_ROOT', None)
    stdout_path, stderr_path = OUT / f'{label}.stdout.txt', OUT / f'{label}.stderr.txt'
    combined_path = OUT / f'{label}.txt'
    # Combined output preserves actual OS stream arrival; separate streams are
    # unnecessary because both complete streams are redirected to this log.
    with combined_path.open('wb') as log:
        child = subprocess.Popen(command, cwd=ROOT, env=env, stdout=log,
                                 stderr=subprocess.STDOUT, start_new_session=True)
        save(f'{label}.running.json', {'command': command, 'cwd': str(ROOT),
             'pid': child.pid, 'processGroup': child.pid, 'startedAt': started,
             'timeoutSeconds': 120, 'sourcesBefore': before})
        timed_out = False
        try:
            code = child.wait(timeout=120)
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(child.pid, signal.SIGTERM)
            try:
                code = child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                code = child.wait(timeout=5)
    after = manifest()
    output = combined_path.read_bytes()
    receipt = {'command': ' '.join(command), 'argv': command, 'cwd': str(ROOT),
               'pid': child.pid, 'processGroup': child.pid, 'startedAt': started,
               'finishedAt': now(), 'elapsedSeconds': time.monotonic()-began,
               'exitCode': code, 'timedOut': timed_out, 'reaped': True,
               'sourceUnchanged': before == after, 'sourcesBefore': before,
               'sourcesAfter': after, 'log': str(combined_path.relative_to(ROOT)),
               'logSha256': digest(output), 'logBytes': len(output),
               'environment': {'CARGO_NET_OFFLINE': 'true',
                    'CARGO_TARGET_DIR': env.get('CARGO_TARGET_DIR'),
                    'RUSTUP_TOOLCHAIN': env.get('RUSTUP_TOOLCHAIN'),
                    'SKIP_COVERAGE': env.get('SKIP_COVERAGE'),
                    'WORKSPACE_ROOT': env.get('WORKSPACE_ROOT')}}
    if label == 'harness':
        current = (ROOT / 'scripts/harness/validate.sh').read_bytes()
        original = subprocess.check_output(['git', 'show', BASE+':scripts/harness/validate.sh'])
        receipt.update(unchanged=current == original, actualResultRecorded=True,
                       harnessSha256=digest(current),
                       coverage={'skipped': False, 'passed': code == 0,
                                 'missingScript': b'Script not found "test:coverage"' in output})
    save(f'{label}.json', receipt)
    print(json.dumps({k: receipt[k] for k in ['command', 'exitCode', 'elapsedSeconds', 'sourceUnchanged']}))
    print(output.decode(errors='replace')[-4500:])
    return code


if __name__ == '__main__':
    label, *command = sys.argv[1:]
    sys.exit(run(label, command))
