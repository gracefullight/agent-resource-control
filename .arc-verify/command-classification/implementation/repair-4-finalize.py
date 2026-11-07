"""Validate current receipts; report the missing gate instead of waiving it."""
import hashlib
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
OUT = pathlib.Path(__file__).resolve().parent
subprocess.run([sys.executable, str(OUT/'repair-4-inventory.py')], cwd=ROOT, check=True)
inventory = json.loads((OUT/'repair-4-source-inventory.json').read_text())
current = {item['path']: item['sha256'] for item in inventory['inputs']}
checks = {}
names = ['reconciled-unit-ipc-cli', 'reconciled-daemon-matrix',
         'reconciled-daemon-malformed', 'reconciled-daemon-ssh',
         'reconciled-daemon-bounds', 'reconciled-daemon-unix-privacy',
         'reconciled-daemon-saturation', 'reconciled-compatibility',
         'reconciled-fmt', 'reconciled-clippy', 'harness']
test_count = 0
for name in names:
    stem = 'repair-4-'+name
    receipt = json.loads((OUT/(stem+'.json')).read_text())
    raw = (OUT/(stem+'.txt')).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == receipt['outputSha256'], name
    assert receipt['sourcesBefore'] == receipt['sourcesAfter'] == current, name
    assert receipt['sourcesMatch'], name
    assert receipt['exitCode'] == (1 if name == 'harness' else 0), name
    counts = [tuple(map(int, group)) for group in re.findall(
        r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored', raw.decode())]
    assert all(failed == ignored == 0 for _, failed, ignored in counts)
    count = sum(passed for passed, _, _ in counts)
    if receipt['argv'][:2] == ['cargo', 'test']:
        assert count > 0, name  # A misspelled filter must never certify a test.
        test_count += count
    checks[name] = {key: receipt[key] for key in
                    ['argv', 'exitCode', 'elapsedSeconds', 'outputSha256']}
    checks[name].update(receipt=stem+'.json', log=stem+'.txt', sourcesMatchCurrent=True,
                        testsPassed=count)
probe = json.loads((OUT/'repair-4-tcp-prerequisite.json').read_text())
assert not probe['permitted'] and probe['errno'] == 1
assert not (OUT/'repair-4-reconciled-workspace.json').exists()
harness = (OUT/'repair-4-harness.txt').read_text()
assert 'test:coverage' in harness
observations = {}
for path in sorted((OUT/'repair-4-observations').glob('*.json')):
    observations[path.stem] = {'path': str(path.relative_to(OUT)),
                               'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                               'value': json.loads(path.read_text())}
privacy = observations['classification-privacy']['value']
assert privacy['cases'] == 12 and privacy['liveSqliteAndWalChecked']
assert all(privacy[key] == 0 for key in
           ['toolTrapsStarted', 'substitutionMarkers', 'endpointContacts', 'canariesPersisted'])
load = observations['classification-saturation']['value']
assert load['sameConnectionRecovery'] and load['classificationLoadContinued']
assert load['signalToExitUs'] <= 3500000 and load['maximumHealthUs'] < 2000000
assert load['completed'] > load['classificationsAtRecovery'] >= load['classificationsBeforeRecovery']
accounting = observations['classification-accounting']['value']
assert all(accounting[key] for key in
           ['accounted', 'deadline_met', 'gauges_zero', 'handlers_reaped', 'socket_removed', 'worker_joined'])
assert accounting['metrics']['requests']['high_water'] <= 16
assert accounting['metrics']['connections']['high_water'] <= 64
assert accounting['metrics']['db_outstanding']['high_water'] == 0
scripts = [{'path': str(path.relative_to(ROOT)), 'lines': len(path.read_bytes().splitlines())}
           for path in sorted(OUT.glob('repair-4-*.py'))]
assert all(item['lines'] <= 500 for item in scripts)
summary = {
    'status': 'partial', 'taskId': 'task-4', 'attempt': 4, 'skillsUsed': ['oma-backend'],
    'baselineHead': inventory['baselineHead'], 'head': inventory['head'],
    'sourceInputsMatch': True, 'changesThisAttempt': inventory['changesThisAttempt'],
    'inputArtifactsChangedThisAttempt': False, 'reconciledSpecificationsAgree': True,
    'interfaceConflictResolved': True, 'wireOrPublicAPIChangedThisAttempt': False,
    'detectorRuntimeLogicChangedThisAttempt': False,
    'fixtures': inventory['fixtures'], 'permittedFinalTestsPassed': test_count,
    'fmtPassed': True, 'clippyPassed': True, 'workspaceTestsPassed': False,
    'requiredCargoGatesPassed': False, 'tcpPrivacyVerified': False,
    'workspaceGate': {'command': 'cargo test --workspace',
                      'status': 'not_run_prerequisite_unavailable',
                      'latestHistoricalReceipt': 'repair-3-final-workspace.json',
                      'latestHistoricalExitCode': 101,
                      'historicalReceiptCertifiesCurrentInputs': False},
    'independentReviewComplete': False, 'checks': checks, 'observations': observations,
    'migrationUnchanged': inventory['invariants']['crates/arc-storage/migrations/0001_phase0.sql']['unchanged'],
    'harnessUnchanged': inventory['invariants']['scripts/harness/validate.sh']['unchanged'],
    'sourceLineLimitPassed': inventory['sourceLineLimitPassed'],
    'rustSourceFiles': sum(item['path'].endswith('.rs') for item in inventory['inputs']),
    'maximumRustLines': max(item['lines'] for item in inventory['inputs'] if item['path'].endswith('.rs')),
    'evidenceScriptLengths': scripts,
    'priorArtifactsPreserved': inventory['preservedPriorArtifacts'],
    'harness': {'command': './scripts/harness/validate.sh', 'exitCode': 1,
                'coverageStatus': 'unresolved', 'receipt': 'repair-4-harness.json'},
    'blockers': [{'path': 'crates/arc-daemon/tests/classification/privacy.rs:95',
                  'prerequisiteReceipt': 'repair-4-tcp-prerequisite.json',
                  'description': 'No effective local TCP-listen permission; errno 1 at 127.0.0.1:0. Required TCP assertions and exact workspace gate remain unexecuted this attempt.'}],
    'chiefAction': 'Stop identical retries. Resume the retained TCP test and exact workspace gate only in a supported noninteractive context with effective local TCP-listen permission.',
    'remainingObligations': ['permitted TCP privacy execution', 'exact workspace test gate',
                              'independent oma-qa review', 'harness coverage',
                              'broader detector coverage', 'scheduling', 'managed-execution',
                              'desktop', 'native Windows'],
    'reports': ['repair-4-report.md', 'repair-4-test-matrix.md', 'repair-4-static-inspection.md',
                'repair-4-source-inventory.json']}
(OUT/'repair-4-summary.json').write_text(json.dumps(summary, indent=2)+'\n')
claim = {'status': 'partial', 'changedFiles': inventory['changesThisAttempt'],
         'unresolved': [item['description'] for item in summary['blockers']] +
                       ['Harness test:coverage remains unresolved; independent QA remains assigned separately.'],
         'artifacts': [str((OUT/name).relative_to(ROOT)) for name in summary['reports']] +
                      [str((OUT/'repair-4-summary.json').relative_to(ROOT))],
         'chiefAction': summary['chiefAction']}
(OUT/'repair-4-claim.json').write_text(json.dumps(claim, indent=2)+'\n')
print(json.dumps({'status': summary['status'], 'finalSourceTestsPassed': test_count,
                  'fmtPassed': True, 'clippyPassed': True,
                  'workspaceGate': summary['workspaceGate']['status'],
                  'harnessExitCode': 1, 'priorArtifactsPreserved': summary['priorArtifactsPreserved']}))
