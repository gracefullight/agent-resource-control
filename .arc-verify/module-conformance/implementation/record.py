#!/usr/bin/env python3
"""Run one requested check offline and retain source-bound lossless output."""
import datetime as dt
import json
import os
from pathlib import Path
import platform
import resource
import runpy
import signal
import subprocess
import sys
import time

HERE = Path(__file__).resolve().parent
api = runpy.run_path(str(HERE.parent / 'baseline/capture.py'))
ROOT = api['ROOT']
sha = api['sha']


def save(name, value):
    (HERE / name).write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def record(name, argv):
    before = api['source_fingerprint']()
    start = time.monotonic()
    started = dt.datetime.now(dt.timezone.utc)
    overrides = {'CARGO_NET_OFFLINE': 'true', 'RUSTUP_AUTO_INSTALL': '0'}
    env = {**os.environ, **overrides}
    env.pop('SKIP_COVERAGE', None)
    child = subprocess.Popen(argv, cwd=ROOT, env=env, stdout=subprocess.PIPE,
                             stderr=subprocess.STDOUT, start_new_session=True)
    save(name + '.running.json', {'argv': argv, 'pid': child.pid,
         'ownedProcessGroup': child.pid, 'startedAt': started.isoformat(),
         'sourceFingerprintBefore': before, 'timeoutSeconds': 120})
    timed_out = False
    try:
        data, _ = child.communicate(timeout=120)
    except subprocess.TimeoutExpired:
        timed_out = True
        os.killpg(child.pid, signal.SIGTERM)
        try:
            data, _ = child.communicate(timeout=10)
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL)
            data, _ = child.communicate()
    lines = data.splitlines(keepends=True)
    parts = []
    for n, offset in enumerate(range(0, len(lines), 400), 1):
        path = f'{name}.part-{n:02}.txt'
        chunk = b''.join(lines[offset:offset + 400])
        (HERE / path).write_bytes(chunk)
        parts.append({'path': path, 'sha256': sha(chunk), 'lines': chunk.count(b'\n')})
    receipt = {'command': argv, 'cwd': str(ROOT), 'startedAt': started.isoformat(),
        'finishedAt': dt.datetime.now(dt.timezone.utc).isoformat(),
        'elapsedSeconds': time.monotonic() - start, 'exitCode': child.returncode,
        'timedOut': timed_out, 'processReaped': child.poll() is not None,
        'pid': child.pid, 'environmentOverrides': overrides, 'environmentRemoved': ['SKIP_COVERAGE'],
        'environmentContext': {k: env.get(k) for k in ['PATH', 'CARGO_HOME', 'RUSTUP_HOME',
                                'CARGO_TARGET_DIR', 'RUSTUP_TOOLCHAIN', 'WORKSPACE_ROOT']},
        'host': platform.platform(), 'cpuCount': os.cpu_count(),
        'resourceLimits': {k: resource.getrlimit(getattr(resource, k))
                           for k in ['RLIMIT_CPU', 'RLIMIT_AS', 'RLIMIT_NOFILE']},
        'acceptedRevision': api['git']('rev-parse', 'HEAD').decode().strip(),
        'sourceFingerprintBefore': before, 'sourceFingerprintAfter': api['source_fingerprint'](),
        'outputStreams': 'stdout and stderr merged by subprocess.STDOUT in arrival order',
        'outputBytes': len(data), 'outputSha256': sha(data), 'outputParts': parts}
    save(name + '.json', receipt)
    print(json.dumps({'name': name, 'exitCode': child.returncode, 'timedOut': timed_out,
                      'tail': data.decode(errors='replace')[-2200:]}), flush=True)


if __name__ == '__main__':
    record(sys.argv[1], sys.argv[2:])
