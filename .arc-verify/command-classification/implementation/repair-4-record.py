"""Append unique receipts without replacing prior runs or changing test argv."""
import datetime
import hashlib
import json
import os
import pathlib
import subprocess
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parents[3]
OUT = pathlib.Path(__file__).resolve().parent


def inputs():
    paths = [*ROOT.glob('crates/**/*.rs'), *ROOT.glob('crates/**/Cargo.toml'),
             *ROOT.glob('crates/**/*.sql'), ROOT/'Cargo.toml', ROOT/'Cargo.lock',
             ROOT/'scripts/harness/validate.sh',
             *[(ROOT/'.arc-verify/command-classification'/name)
               for name in ['contract.md', 'contract.json', 'cli-requirements.md']]]
    return {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in sorted(paths)}


if __name__ == '__main__':
    name, *argv = sys.argv[1:]
    assert name.startswith('repair-4-') and '/' not in name
    assert not (OUT/(name+'.json')).exists() and not (OUT/(name+'.txt')).exists()
    if argv[:3] == ['cargo', 'test', '--workspace'] or any(
            'required_loopback_endpoint_nonexecution_and_privacy' in arg for arg in argv):
        probe = json.loads((OUT/'repair-4-tcp-prerequisite.json').read_text())
        assert probe['permitted'], 'TCP prerequisite unavailable; do not replay blocked checks'
    before = inputs()
    env = os.environ.copy()
    env['CARGO_NET_OFFLINE'] = 'true'
    env['ARC_TEST_EVIDENCE_DIR'] = str(OUT/'repair-4-observations')
    if argv == ['./scripts/harness/validate.sh']:
        env.pop('SKIP_COVERAGE', None)
    start = time.monotonic()
    with (OUT/(name+'.txt')).open('xb') as log:
        try:
            code = subprocess.run(argv, cwd=ROOT, env=env, stdout=log,
                                  stderr=subprocess.STDOUT, timeout=120).returncode
        except subprocess.TimeoutExpired:
            code = 124
            log.write(b'\nRecorder deadline exceeded (120 seconds).\n')
    after = inputs()
    receipt = {'argv': argv, 'cwd': str(ROOT), 'exitCode': code,
               'at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
               'elapsedSeconds': time.monotonic()-start,
               'environment': {'CARGO_NET_OFFLINE': 'true',
                               'ARC_TEST_EVIDENCE_DIR': 'implementation/repair-4-observations'},
               'sourcesBefore': before, 'sourcesAfter': after, 'sourcesMatch': before == after,
               'outputSha256': hashlib.sha256((OUT/(name+'.txt')).read_bytes()).hexdigest()}
    (OUT/(name+'.json')).write_text(json.dumps(receipt, indent=2)+'\n')
    print(json.dumps({key: receipt[key] for key in ['argv', 'exitCode', 'elapsedSeconds', 'sourcesMatch']}))
    sys.exit(code)
