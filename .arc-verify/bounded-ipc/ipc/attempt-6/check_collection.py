"""Read-only receiving-checkout verification. No copies, refs or commits."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

root = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path.cwd()
evidence = root / '.arc-verify/bounded-ipc/ipc/evidence.json'
d = json.loads(evidence.read_text())
errors = []
for e in d['artifacts'] + d['sourceHashes']:
    p = root / e['path']
    if not p.is_file():
        errors.append({'path':e['path'], 'error':'missing'})
    elif hashlib.sha256(p.read_bytes()).hexdigest() != e['sha256']:
        errors.append({'path':e['path'], 'error':'hash mismatch'})
for e in d['preservation']:
    p = root / e['path']
    r = subprocess.run(['git','show',d['baselineHead']+':'+e['path']], cwd=root, capture_output=True)
    if r.returncode or r.stdout != p.read_bytes():
        errors.append({'path':e['path'], 'error':'accepted baseline byte comparison failed'})
print(json.dumps({'checkedRoot':str(root), 'artifactCount':len(d['artifacts']),
                  'sourceCount':len(d['sourceHashes']), 'errors':errors,
                  'pass':not errors, 'scope':'read-only artifact/source collection; no test replay'},indent=2))
sys.exit(bool(errors))
