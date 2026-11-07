#!/usr/bin/env python3
"""Run one native check with source-bound logs; never rewrite mission state."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path.cwd()
OUT = ROOT / '.arc-verify/schema-compatibility/backend'

def manifest():
    paths = [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', ROOT / 'scripts/harness/validate.sh']
    for crate in (ROOT / 'crates').iterdir():
        paths += [p for p in crate.rglob('*') if p.is_file() and p.suffix in {'.rs', '.toml', '.sql'}]
    paths += [ROOT / 'docs/schema-compatibility-contract.md', ROOT / 'docs/bounded-ipc-recovery.md']
    return {str(p.relative_to(ROOT)): {'sha256': hashlib.sha256(p.read_bytes()).hexdigest(),
            'bytes': p.stat().st_size, 'lines': len(p.read_bytes().splitlines())} for p in sorted(paths)}

if __name__ == '__main__':
    name, *command = sys.argv[1:]
    before = manifest()
    started = datetime.datetime.now(datetime.timezone.utc).isoformat()
    began = time.monotonic()
    with (OUT / f'{name}.txt').open('wb') as log:
        child = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT)
        pid = child.pid
        try:
            code = child.wait(timeout=120)
        except subprocess.TimeoutExpired:
            child.terminate()
            try:
                code = child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                code = child.wait()
    after = manifest()
    receipt = {'command': command, 'cwd': str(ROOT), 'pid': pid, 'startedAt': started,
               'elapsedSeconds': time.monotonic()-began, 'exitCode': code,
               'sourceUnchanged': before == after, 'sourcesBefore': before, 'sourcesAfter': after,
               'logSha256': hashlib.sha256((OUT / f'{name}.txt').read_bytes()).hexdigest(),
               'environment': {'CARGO_TARGET_DIR': os.getenv('CARGO_TARGET_DIR'),
                               'platform': sys.platform}}
    (OUT / f'{name}.json').write_text(json.dumps(receipt, indent=2)+'\n')
    print(json.dumps({k: receipt[k] for k in ['command','pid','exitCode','sourceUnchanged','elapsedSeconds']}))
    print((OUT / f'{name}.txt').read_text()[-12000:])
    sys.exit(code)
