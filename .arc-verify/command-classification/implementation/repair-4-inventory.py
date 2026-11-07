"""Bind this reconciliation to actual inputs; keep earlier receipts immutable."""
import hashlib
import json
import pathlib
import re
import subprocess

from importlib.util import module_from_spec, spec_from_file_location

ROOT = pathlib.Path(__file__).resolve().parents[3]
OUT = pathlib.Path(__file__).resolve().parent
BASELINE = '05f0812d0841485bbe07b3f1f8f6fde3861a1d55'
spec = spec_from_file_location('recorder', OUT/'repair-4-record.py')
recorder = module_from_spec(spec)
spec.loader.exec_module(recorder)
current = recorder.inputs()
prep = json.loads((OUT/'repair-4-preparation.json').read_text())


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


entries = []
for name, digest in current.items():
    data = (ROOT/name).read_bytes()
    entries.append({'path': name, 'sha256': digest, 'bytes': len(data),
                    'lines': len(data.splitlines())})
invariants = {}
for name in ['crates/arc-storage/migrations/0001_phase0.sql', 'scripts/harness/validate.sh']:
    old = subprocess.check_output(['git', 'show', f'{BASELINE}:{name}'], cwd=ROOT)
    invariants[name] = {'unchanged': old == (ROOT/name).read_bytes(),
                        'baselineSha256': hashlib.sha256(old).hexdigest(),
                        'currentSha256': current[name]}
changed = sorted(name for name in current.keys() | prep['initialInputs'].keys()
                 if current.get(name) != prep['initialInputs'].get(name))
expected = sorted(['crates/arc-cli/src/main.rs', 'crates/arc-cli/tests/classification.rs',
                   'crates/arc-detector/src/tests.rs',
                   'crates/arc-detector/tests/support/contract.rs',
                   'crates/arc-ipc/tests/classification.rs',
                   'crates/arc-daemon/tests/classification.rs'])
assert changed == expected, changed
prior_changed = [name for name, digest in prep['priorArtifacts'].items()
                 if not (ROOT/name).is_file() or sha(ROOT/name) != digest]
assert not prior_changed, prior_changed
contract = json.loads((ROOT/'.arc-verify/command-classification/contract.json').read_text())
assert contract['markdownSha256'] == current['.arc-verify/command-classification/contract.md']
requirements = (ROOT/'.arc-verify/command-classification/cli-requirements.md').read_text()
for name in ['contract.md', 'contract.json']:
    assert current['.arc-verify/command-classification/'+name] in requirements
    assert current['.arc-verify/command-classification/'+name] == prep['initialInputs'][
        '.arc-verify/command-classification/'+name]
assert current['.arc-verify/command-classification/cli-requirements.md'] == prep['initialInputs'][
    '.arc-verify/command-classification/cli-requirements.md']
assert (len(contract['fixtures']), len(contract['dispositionFixtures']),
        len(contract['malformedPayloadFixtures'])) == (45, 7, 10)
assert set(re.findall(r'^\| (C\d\d) \|', requirements, re.M)) == {
    f'C{i:02d}' for i in range(1, 27)}
source_ok = all(entry['lines'] <= 500 for entry in entries if entry['path'].endswith('.rs'))
assert source_ok and all(value['unchanged'] for value in invariants.values())
historical = json.loads((OUT/'repair-3-source-inventory.json').read_text())
old_inputs = {entry['path']: entry['sha256'] for entry in historical['inputs']}
report = {'baselineHead': BASELINE,
          'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
          'inputs': entries, 'invariants': invariants, 'sourceLineLimitPassed': source_ok,
          'changesThisAttempt': changed, 'preservedPriorArtifacts': len(prep['priorArtifacts']),
          'priorArtifactsChanged': prior_changed, 'inputSpecificationsUnchangedThisAttempt': True,
          'historicalInputDifferences': sorted(name for name in current.keys() | old_inputs.keys()
                                             if current.get(name) != old_inputs.get(name)),
          'fixtures': {'original': 45, 'disposition': 7, 'orderingVariants': 2,
                       'cliSupplementalLiterals': 14, 'totalSuccessMatrix': 68,
                       'malformed': 10, 'cliMatrixRows': 26}}
(OUT/'repair-4-source-inventory.json').write_text(json.dumps(report, indent=2)+'\n')
print(json.dumps({'inputs': len(entries), 'rustFiles': sum(x['path'].endswith('.rs') for x in entries),
                  'maximumRustLines': max(x['lines'] for x in entries if x['path'].endswith('.rs')),
                  'changesThisAttempt': changed, 'priorArtifactsPreserved': len(prep['priorArtifacts']),
                  'migrationAndHarnessUnchanged': True, 'specificationDigestsVerified': True}))
