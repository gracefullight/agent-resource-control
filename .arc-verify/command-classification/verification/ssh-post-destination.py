#!/usr/bin/env python3
"""Read-only QA probe: submit synthetic SSH text to real IPC and arcctl only."""
import datetime
import hashlib
import json
import os
import pathlib
import signal
import socket
import struct
import subprocess
import tempfile
import time

ROOT = pathlib.Path.cwd()
OUT = ROOT / '.arc-verify/command-classification/verification'


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def snapshot():
    rows = json.loads((OUT / 'source-inputs-before.json').read_text())
    for row in rows:
        path = ROOT / row['path']
        data = path.read_bytes()
        row.update(sha256=hashlib.sha256(data).hexdigest(), bytes=len(data),
                   lines=len(data.splitlines()), mode=oct(path.stat().st_mode & 0o777))
    return rows


def receive(stream, size):
    value = b''
    while len(value) < size:
        chunk = stream.recv(size - len(value))
        if not chunk:
            raise RuntimeError('premature EOF')
        value += chunk
    return value


def expected(kind):
    if kind == 'unknown':
        return {'classification': 'UNKNOWN', 'locality': 'UNKNOWN', 'uncertainty': 'OBSERVE'}
    if kind == 'local':
        return {'classification': 'TEST', 'locality': 'HOST_LOCAL',
                'reason': 'CARGO_TEST', 'uncertainty': 'NONE'}
    return {'classification': 'REMOTE_EXEC', 'locality': 'REMOTE',
            'reason': 'SSH_REMOTE_COMMAND', 'uncertainty': 'NONE'}


cases = [
    ('post-n', 'ssh build01 -N', 'unknown'),
    ('post-g', 'ssh build01 -G', 'unknown'),
    ('post-n-with-command', "ssh build01 -N 'make test'", 'unknown'),
    ('post-g-with-command', "ssh build01 -G 'make test'", 'unknown'),
    ('post-l', 'ssh build01 -L8080:localhost:80', 'unknown'),
    ('post-r', 'ssh build01 -R8080:localhost:80', 'unknown'),
    ('post-d', 'ssh build01 -D8080', 'unknown'),
    ('post-w', 'ssh build01 -W target:80', 'unknown'),
    ('post-quoted-n', "ssh build01 '-N'", 'unknown'),
    ('compound-n', 'cargo test && ssh build01 -N', 'unknown'),
    ('compound-g', 'cargo test && ssh build01 -G', 'unknown'),
    ('pre-n-control', 'ssh -N build01', 'unknown'),
    ('opaque-word-control', 'ssh build01 make -N', 'remote'),
    ('opaque-quoted-command-control', "ssh build01 'make -N'", 'remote'),
    ('terminated-before-destination-control', 'ssh -- build01 -N', 'remote'),
    ('terminated-after-destination-control', 'ssh build01 -- -N', 'remote'),
    ('quoted-local-argument-control', "cargo test 'ssh build01 -N'", 'local'),
]

started, tick, before = now(), time.monotonic(), snapshot()
rows, child = [], None
daemon = ROOT / 'target/debug/arc-daemon'
cli = ROOT / 'target/debug/arcctl'
binaries = [{'path': str(p.relative_to(ROOT)), 'sha256': hashlib.sha256(p.read_bytes()).hexdigest()}
            for p in [daemon, cli]]
with tempfile.TemporaryDirectory(prefix='arcqa-', dir='/private/tmp') as temporary:
    fixture = pathlib.Path(temporary)
    endpoint = fixture / 'ipc'
    env = os.environ.copy()
    env.update(ARC_SOCKET_PATH=str(endpoint), ARC_DB_PATH=str(fixture / 'db'),
               ARC_RUNTIME_REPORT=str(fixture / 'runtime.json'))
    env.pop('ARC_IPC_PROFILE', None)
    with (fixture / 'stdout').open('wb') as stdout, (fixture / 'stderr').open('wb') as stderr:
        child = subprocess.Popen([str(daemon)], cwd=ROOT, env=env,
                                 stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr)
        try:
            deadline = time.monotonic() + 10
            while not endpoint.exists():
                if child.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError('owned daemon startup failed: ' + (fixture / 'stderr').read_text())
                time.sleep(.01)
            with socket.socket(socket.AF_UNIX) as stream:
                stream.settimeout(3)
                stream.connect(str(endpoint))
                for identifier, text, kind in cases:
                    payload = {'protocol_version': 1, 'request_id': 'qa-' + identifier,
                               'operation': 'job.classify', 'payload': {'command': text}}
                    frame = json.dumps(payload).encode()
                    stream.sendall(struct.pack('>I', len(frame)) + frame)
                    response = json.loads(receive(stream, struct.unpack('>I', receive(stream, 4))[0]))
                    outputs = []
                    for machine in [False, True]:
                        argv = [str(cli), 'classify'] + (['--json'] if machine else []) + ['--', text]
                        process = subprocess.run(argv, cwd=ROOT, env=env, capture_output=True, timeout=8)
                        outputs.append({'argv': argv, 'exitCode': process.returncode,
                                        'stdout': process.stdout.decode(), 'stderr': process.stderr.decode()})
                    actual = response.get('result', {})
                    want = expected(kind)
                    matches = all(actual.get(k) == v for k, v in want.items())
                    cli_agrees = json.loads(outputs[1]['stdout']) == actual
                    human = ''.join(f'{label}: {actual[field]}\n' for label, field in
                                    [('Classification', 'classification'), ('Locality', 'locality'),
                                     ('Reason', 'reason'), ('Uncertainty', 'uncertainty')])
                    cli_agrees &= outputs[0]['stdout'] == human + 'Command not executed.\n'
                    assert response['status'] == 'ok' and cli_agrees
                    assert all(p['exitCode'] == 0 and p['stderr'] == '' for p in outputs)
                    row = {'id': identifier, 'syntheticInput': text, 'expectedFields': want,
                           'response': response, 'cli': outputs, 'expectedMatched': matches,
                           'ipcAndBothCliModesAgree': cli_agrees}
                    rows.append(row)
                    print(json.dumps({'id': identifier, 'actual': actual,
                                      'expectedMatched': matches}), flush=True)
        finally:
            if child.poll() is None:
                child.send_signal(signal.SIGTERM)
            try:
                code = child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait()
                raise
    assert code == 0 and not endpoint.exists()
    daemon_stdout, daemon_stderr = (fixture / 'stdout').read_text(), (fixture / 'stderr').read_text()
    runtime = json.loads((fixture / 'runtime.json').read_text())
after = snapshot()
receipt = {'command': 'python3 .arc-verify/command-classification/verification/ssh-post-destination.py',
           'argv': ['python3', '.arc-verify/command-classification/verification/ssh-post-destination.py'],
           'cwd': str(ROOT), 'executionContext': json.loads((OUT / 'context.json').read_text()),
           'startedAt': started, 'finishedAt': now(), 'elapsedSeconds': time.monotonic() - tick,
           'exitCode': 1 if any(not row['expectedMatched'] for row in rows) else 0,
           'inputsBefore': before, 'inputsAfter': after, 'sourceInputsMatch': before == after,
           'binaries': binaries, 'cases': rows, 'mismatchCount': sum(not r['expectedMatched'] for r in rows),
           'daemonExitCode': code, 'daemonStdout': daemon_stdout, 'daemonStderr': daemon_stderr,
           'runtimeReport': runtime, 'analyzedToolsExecuted': False,
           'scope': 'Synthetic commands are data in IPC requests and arcctl argv. Only owned daemon/arcctl run.'}
(OUT / 'ssh-post-destination.json').write_text(json.dumps(receipt, indent=2) + '\n')
raise SystemExit(receipt['exitCode'])
