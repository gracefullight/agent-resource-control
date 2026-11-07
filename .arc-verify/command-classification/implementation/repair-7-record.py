"""Capture exact checks in the SSH boundary repair checks without changing reviewed evidence."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[2]
PREP = json.loads((OUT / 'repair-7-preparation.json').read_text())


def inputs():
    return {i['path']: hashlib.sha256((ROOT / i['path']).read_bytes()).hexdigest()
            for i in PREP['inputs'] + [{'path': 'crates/arc-detector/tests/support/ssh.rs'}]}


if __name__ == '__main__':
    name, *argv = sys.argv[1:]
    assert name.startswith('repair-7-') and '/' not in name
    permitted = [
        ['cargo', 'test', '-p', 'arc-daemon', '--test', 'classification',
         'privacy::required_loopback_endpoint_nonexecution_and_privacy', '--', '--exact'],
        ['cargo', 'fmt', '--all', '--', '--check'],
        ['cargo', 'clippy', '--workspace', '--all-targets', '--', '-D', 'warnings'],
        ['cargo', 'test', '--workspace'],
        ['./scripts/harness/validate.sh'],
    ]
    permitted.extend([['cargo', 'test', '-p', crate, 'ssh_post_destination', '--', '--nocapture'] for crate in ['arc-detector', 'arc-ipc', 'arc-daemon']])
    permitted.append(['cargo', 'test', '-p', 'arc-detector', 'tests::production_work_exhaustion_trace', '--', '--exact', '--nocapture'])
    permitted.append(['git', 'diff', '--check', '--', 'Cargo.toml', 'Cargo.lock', 'crates', 'scripts/harness'])
    assert argv in permitted
    assert not (OUT / (name + '.json')).exists()
    before = inputs()
    env = os.environ.copy()
    env['CARGO_NET_OFFLINE'] = 'true'
    env['ARC_RUNTIME_EVIDENCE_DIR'] = str(OUT / 'repair-7-observations')
    if argv == ['./scripts/harness/validate.sh']:
        env.pop('SKIP_COVERAGE', None)
    started = datetime.datetime.now(datetime.timezone.utc).isoformat()
    start = time.monotonic()
    with (OUT / (name + '.txt')).open('xb') as log:
        try:
            code = subprocess.run(argv, cwd=ROOT, env=env, stdout=log,
                                  stderr=subprocess.STDOUT, timeout=120).returncode
        except subprocess.TimeoutExpired:
            code = 124
            log.write(b'\nRecorder deadline exceeded (120 seconds).\n')
    after = inputs()
    receipt = {
        'argv': argv, 'command': ' '.join(argv), 'cwd': str(ROOT), 'exitCode': code,
        'finishedAt': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'startedAt': started,
        'elapsedSeconds': time.monotonic() - start,
        'environment': {'CARGO_NET_OFFLINE': 'true',
                        'ARC_RUNTIME_EVIDENCE_DIR': env['ARC_RUNTIME_EVIDENCE_DIR']},
        'sourcesBefore': before, 'sourcesAfter': after, 'sourcesMatch': before == after,
        'outputSha256': hashlib.sha256((OUT / (name + '.txt')).read_bytes()).hexdigest(),
    }
    if argv == ['./scripts/harness/validate.sh']:
        receipt.update(coverageStatus='unresolved', coverageSkipEnabled=False)
    with (OUT / (name + '.json')).open('x') as output:
        output.write(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps({k: receipt[k] for k in
                      ['command', 'exitCode', 'elapsedSeconds', 'sourcesMatch']}))
    sys.exit(code)
