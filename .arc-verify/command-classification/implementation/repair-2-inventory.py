"""Inventory all Rust/manifests and delivered inputs, including untracked files."""
import hashlib
import json
import pathlib
import subprocess

root = pathlib.Path(__file__).resolve().parents[3]
out = pathlib.Path(__file__).resolve().parent
baseline = '05f0812d0841485bbe07b3f1f8f6fde3861a1d55'
paths = sorted([*root.glob('crates/**/*.rs'), *root.glob('crates/**/Cargo.toml'),
                *root.glob('crates/**/*.sql'), root/'Cargo.toml', root/'Cargo.lock',
                root/'scripts/harness/validate.sh',
                root/'.arc-verify/command-classification/contract.json',
                root/'.arc-verify/command-classification/contract.md',
                root/'.arc-verify/command-classification/cli-requirements.md'])
entries = []
for path in paths:
    data = path.read_bytes()
    entries.append({'path':str(path.relative_to(root)),
                    'sha256':hashlib.sha256(data).hexdigest(),
                    'bytes':len(data), 'lines':len(data.splitlines())})
invariants = {}
for path in ['crates/arc-storage/migrations/0001_phase0.sql', 'scripts/harness/validate.sh']:
    old = subprocess.check_output(['git', 'show', f'{baseline}:{path}'], cwd=root)
    new = (root/path).read_bytes()
    invariants[path] = {'unchanged':new == old,
                        'baselineSha256':hashlib.sha256(old).hexdigest(),
                        'currentSha256':hashlib.sha256(new).hexdigest()}
report = {'baselineHead':baseline,
          'head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),
          'inputs':entries, 'invariants':invariants,
          'sourceLineLimitPassed':all(x['lines'] <= 500 for x in entries if x['path'].endswith('.rs'))}
(out/'repair-2-source-inventory.json').write_text(json.dumps(report, indent=2)+'\n')
contract = json.loads((root/'.arc-verify/command-classification/contract.json').read_text())
assert hashlib.sha256((root/'.arc-verify/command-classification/contract.md').read_bytes()).hexdigest() == contract['markdownSha256']
assert report['sourceLineLimitPassed'] and all(x['unchanged'] for x in invariants.values())
print(json.dumps({'rustFiles':sum(x['path'].endswith('.rs') for x in entries),
                  'maxRustLines':max(x['lines'] for x in entries if x['path'].endswith('.rs')),
                  'invariantsPassed':True, 'contractDigestPassed':True}))
