"""Capture the required source-bound production work counter trace."""
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
PREP = json.loads((OUT / 'repair-6-preparation.json').read_text())


def inputs():
    return {i['path']: hashlib.sha256((ROOT / i['path']).read_bytes()).hexdigest()
            for i in PREP['inputs']}


if __name__ == '__main__':
    name, *argv = sys.argv[1:]
    assert name.startswith('repair-6-') and '/' not in name
    permitted = [
        ['cargo', 'test', '-p', 'arc-daemon', '--test', 'classification',
         'privacy::required_loopback_endpoint_nonexecution_and_privacy', '--', '--exact'],
        ['cargo', 'test', '-p', 'arc-detector', 'tests::production_work_exhaustion_trace', '--', '--exact', '--nocapture'],
        ['cargo', 'clippy', '--workspace', '--all-targets', '--', '-D', 'warnings'],
        ['cargo', 'test', '--workspace'],
        ['./scripts/harness/validate.sh'],
    ]
    assert argv in permitted
    assert json.loads((OUT / 'repair-6-tcp-prerequisite.json').read_text())['success']
    assert not (OUT / (name + '.json')).exists()
    before = inputs()
    env = os.environ.copy()
    env['CARGO_NET_OFFLINE'] = 'true'
    env['ARC_RUNTIME_EVIDENCE_DIR'] = str(OUT / 'repair-6-observations')
    if argv == ['./scripts/harness/validate.sh']:
        env.pop('SKIP_COVERAGE', None)
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
        'at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
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
