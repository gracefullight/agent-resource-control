"""Record exact argv/exits and before/after source hashes; never analyze commands."""
import datetime
import hashlib
import json
import os
import pathlib
import subprocess
import sys
import time

root = pathlib.Path(__file__).resolve().parents[3]
out = pathlib.Path(__file__).resolve().parent
name, *argv = sys.argv[1:]

def inputs():
    paths = [*root.glob('crates/**/*.rs'), *root.glob('crates/**/Cargo.toml'),
             *root.glob('crates/**/*.sql'), root/'Cargo.toml', root/'Cargo.lock',
             root/'scripts/harness/validate.sh',
             root/'.arc-verify/command-classification/contract.json',
             root/'.arc-verify/command-classification/contract.md',
             root/'.arc-verify/command-classification/cli-requirements.md']
    return {str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in sorted(paths)}

before = inputs()
environment = os.environ.copy()
environment['CARGO_NET_OFFLINE'] = 'true'
environment['ARC_TEST_EVIDENCE_DIR'] = str(out/'repair-2-observations')
# The exact harness must run coverage; no host skip setting can waive it.
if argv == ['./scripts/harness/validate.sh']:
    environment.pop('SKIP_COVERAGE', None)
start = time.monotonic()
with (out / (name + '.txt')).open('wb') as log:
    try:
        result = subprocess.run(argv, cwd=root, env=environment, stdout=log,
                                stderr=subprocess.STDOUT, timeout=180)
        code = result.returncode
    except subprocess.TimeoutExpired:
        code = 124
        log.write(b'\nRecorder deadline exceeded (180 seconds).\n')
after = inputs()
receipt = {'argv': argv, 'cwd':str(root), 'exitCode':code,
           'elapsedSeconds':time.monotonic()-start,
           'at':datetime.datetime.now(datetime.timezone.utc).isoformat(),
           'environment':{'CARGO_NET_OFFLINE':'true',
                          'ARC_TEST_EVIDENCE_DIR':'implementation/repair-2-observations'},
           'sourcesBefore':before, 'sourcesAfter':after,
           'sourcesMatch':before == after,
           'outputSha256':hashlib.sha256((out/(name+'.txt')).read_bytes()).hexdigest()}
(out / (name + '.json')).write_text(json.dumps(receipt, indent=2)+'\n')
print(json.dumps({'name':name,'exitCode':code,
                  'elapsedSeconds':receipt['elapsedSeconds'],
                  'sourcesMatch':receipt['sourcesMatch']}))
sys.exit(code)
