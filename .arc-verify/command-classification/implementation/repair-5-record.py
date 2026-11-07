"""Record allowed checks without replacing earlier attempt evidence."""
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
PREPARATION = json.loads((OUT / 'repair-5-preparation.json').read_text())


def inputs():
    return {item['path']: hashlib.sha256((ROOT / item['path']).read_bytes()).hexdigest()
            for item in PREPARATION['inputs']}


if __name__ == '__main__':
    name, *argv = sys.argv[1:]
    assert name.startswith('repair-5-') and '/' not in name
    assert not (OUT / (name + '.json')).exists()
    assert not (OUT / (name + '.txt')).exists()
    # This attempt has no newly established TCP-capable execution context.
    assert argv[:3] != ['cargo', 'test', '--workspace']
    assert not any('required_loopback_endpoint_nonexecution_and_privacy' in a for a in argv)
    permitted = [
        ['cargo', 'test', '-p', 'arc-detector', '-p', 'arc-ipc', '-p', 'arc-cli'],
        ['cargo', 'test', '-p', 'arc-daemon', '--test', 'classification',
         'actual_daemon_and_cli_delivered_contract_matrix', '--', '--exact'],
        ['cargo', 'fmt', '--all', '--', '--check'],
        ['cargo', 'clippy', '--workspace', '--all-targets', '--', '-D', 'warnings'],
        ['./scripts/harness/validate.sh'],
    ]
    assert argv in permitted, 'Only the planned exact checks are allowed'
    before = inputs()
    env = os.environ.copy()
    env['CARGO_NET_OFFLINE'] = 'true'
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
        'environment': {'CARGO_NET_OFFLINE': 'true'},
        'sourcesBefore': before, 'sourcesAfter': after, 'sourcesMatch': before == after,
        'outputSha256': hashlib.sha256((OUT / (name + '.txt')).read_bytes()).hexdigest(),
    }
    if argv == ['./scripts/harness/validate.sh']:
        receipt['coverageStatus'] = 'unresolved'
        receipt['coverageSkipEnabled'] = False
    (OUT / (name + '.json')).write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps({k: receipt[k] for k in
                      ['command', 'exitCode', 'elapsedSeconds', 'sourcesMatch']}))
    sys.exit(code)
