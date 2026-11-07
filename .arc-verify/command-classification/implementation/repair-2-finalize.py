"""Bind repair evidence to current inputs without replacing prior receipts."""
import hashlib
import json
import pathlib
import re

root = pathlib.Path(__file__).resolve().parents[3]
out = pathlib.Path(__file__).resolve().parent
inventory = json.loads((out / 'repair-2-source-inventory.json').read_text())
current = {entry['path']: entry['sha256'] for entry in inventory['inputs']}
assert all(hashlib.sha256((root / path).read_bytes()).hexdigest() == digest
           for path, digest in current.items())
previous = json.loads((out / 'source-inventory.json').read_text())
old = {entry['path']: entry['sha256'] for entry in previous['inputs']}
changed = sorted(path for path in current if old.get(path) != current[path])
assert changed == ['crates/arc-daemon/tests/classification/saturation.rs']
checks = {}
for name in ['repair-2-final-fmt', 'repair-2-final-clippy',
             'repair-2-final-workspace', 'repair-2-all-tests', 'repair-2-harness']:
    receipt = json.loads((out / (name + '.json')).read_text())
    assert receipt['sourcesMatch']
    assert receipt['sourcesBefore'] == current == receipt['sourcesAfter']
    assert receipt['outputSha256'] == hashlib.sha256(
        (out / (name + '.txt')).read_bytes()).hexdigest()
    checks[name] = {key: receipt[key] for key in
                    ['argv', 'exitCode', 'elapsedSeconds', 'outputSha256']}
    checks[name]['receipt'] = name + '.json'
    checks[name]['log'] = name + '.txt'
    checks[name]['sourcesMatchCurrent'] = True
text = (out / 'repair-2-all-tests.txt').read_text()
counts = re.findall(r'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;', text)
totals = {name: sum(int(row[index]) for row in counts)
          for index, name in enumerate(['passed', 'failed', 'ignored'])}
assert totals == {'passed': 202, 'failed': 1, 'ignored': 0}
assert text.count('test privacy::required_loopback_endpoint_nonexecution_and_privacy ... FAILED') == 1
assert 'PermissionDenied' in text and 'Operation not permitted' in text
assert 'saturation::maximum_and_exhausting_classification_load_preserves_health_and_shutdown ... ok' in text
assert 'Script not found "test:coverage"' in (out / 'repair-2-harness.txt').read_text()
observations = {}
for name in ['classification-saturation', 'classification-accounting', 'classification-privacy']:
    path = out / 'repair-2-observations' / (name + '.json')
    observations[name] = {'path': str(path.relative_to(out)),
                          'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                          'value': json.loads(path.read_text())}
load = observations['classification-saturation']['value']
assert load['sameConnectionRecovery'] and load['classificationLoadContinued']
assert load['classificationsBeforeRecovery'] >= 16
assert load['completed'] > load['classificationsAtRecovery'] >= load['classificationsBeforeRecovery']
assert load['malformedRecoveryUs'] < 2_000_000
assert load['signalToExitUs'] <= 3_500_000
summary = {
    'status': 'partial', 'taskId': 'task-4', 'attempt': 2,
    'skillsUsed': ['oma-backend'], 'baselineHead': inventory['baselineHead'],
    'head': inventory['head'], 'sourceInputsMatch': True,
    'sourceChangesSincePreviousAttempt': changed,
    'fmtPassed': checks['repair-2-final-fmt']['exitCode'] == 0,
    'clippyPassed': checks['repair-2-final-clippy']['exitCode'] == 0,
    'workspaceTestsPassed': checks['repair-2-final-workspace']['exitCode'] == 0,
    'requiredCargoGatesPassed': False, 'fullSuite': totals,
    'saturationRepairVerified': True, 'interfaceChanged': False,
    'inputArtifactsChanged': False, 'independentReviewComplete': False,
    'migrationUnchanged': inventory['invariants']['crates/arc-storage/migrations/0001_phase0.sql']['unchanged'],
    'harnessUnchanged': inventory['invariants']['scripts/harness/validate.sh']['unchanged'],
    'sourceLineLimitPassed': inventory['sourceLineLimitPassed'],
    'rustSourceFiles': sum(x['path'].endswith('.rs') for x in inventory['inputs']),
    'maximumRustLines': max(x['lines'] for x in inventory['inputs'] if x['path'].endswith('.rs')),
    'harness': {'command': './scripts/harness/validate.sh',
                'exitCode': checks['repair-2-harness']['exitCode'],
                'coverageStatus': 'unresolved'},
    'checks': checks, 'observations': observations,
    'blockers': [
        {'path': 'crates/arc-daemon/tests/classification/privacy.rs:95',
         'description': 'Mandatory loopback TCP bind is still denied with errno 1; no permitted escalation route is available in this session.'},
        {'paths': ['.arc-verify/command-classification/contract.md:115',
                   '.arc-verify/command-classification/contract.md:117',
                   '.arc-verify/command-classification/cli-requirements.md:46',
                   '.arc-verify/command-classification/cli-requirements.md:49',
                   '.arc-verify/command-classification/cli-requirements.md:69',
                   '.arc-verify/command-classification/cli-requirements.md:80'],
         'description': 'Chief disposition and owner synchronization remain required for empty/oversized input and human output conflicts; no disposition was supplied.'}],
    'remainingObligations': ['permitted TCP privacy execution', 'Chief interface disposition and owner synchronization',
                             'independent oma-qa review', 'harness coverage', 'broader detector catalog',
                             'scheduling', 'managed-execution', 'desktop', 'native Windows'],
    'reports': ['repair-2-report.md', 'repair-2-source-inventory.json'],
}
(out / 'repair-2-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
claim = {'status': 'partial', 'changedFiles': changed + [str(out.relative_to(root)) + '/'],
         'unresolved': [x['description'] for x in summary['blockers']] +
                       ['Unchanged harness lacks test:coverage; independent review remains separately assigned.'],
         'artifacts': [str((out / name).relative_to(root)) for name in
                       ['repair-2-report.md', 'repair-2-summary.json', 'repair-2-source-inventory.json']]}
(out / 'repair-2-claim.json').write_text(json.dumps(claim, indent=2) + '\n')
print(json.dumps({'status': 'partial', 'sourceInputsMatch': True,
                  'saturationRepairVerified': True, 'fullSuite': totals}))
