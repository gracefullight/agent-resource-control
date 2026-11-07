"""Bind attempt-5 checks to sources while retaining incomplete acceptance."""
import hashlib
import json
from pathlib import Path
import re
import subprocess

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[2]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(name, value):
    with (OUT / name).open('x') as output:
        output.write(json.dumps(value, indent=2) + '\n')


prep = json.loads((OUT / 'repair-5-preparation.json').read_text())
current = []
for original in prep['inputs']:
    path = ROOT / original['path']
    data = path.read_bytes()
    current.append({'path': original['path'], 'sha256': sha(path),
                    'bytes': len(data), 'lines': len(data.splitlines())})
hashes = {item['path']: item['sha256'] for item in current}
assert hashes == {item['path']: item['sha256'] for item in prep['inputs']}
assert not prep['differencesFromAttempt4Inputs']
assert {str(p.relative_to(ROOT)) for p in ROOT.glob('crates/**/*.rs')} == {
    item['path'] for item in current if item['path'].endswith('.rs')}
source = [item for item in current if item['path'].endswith('.rs')]
assert all(item['lines'] <= 500 for item in source)
assert all(len(p.read_bytes().splitlines()) <= 500 for p in OUT.glob('repair-5-*.py'))
prior_changed = [item['path'] for item in prep['priorArtifacts']
                 if sha(ROOT / item['path']) != item['sha256']]
assert not prior_changed
contract_root = OUT.parent
contract = json.loads((contract_root / 'contract.json').read_text())
assert contract['markdownSha256'] == sha(contract_root / 'contract.md')
assert len(contract['fixtures']) == 45
assert len(contract['dispositionFixtures']) == 7
assert len(contract['malformedPayloadFixtures']) == 10
rows = re.findall(r'^\| (C\d\d) \|', (contract_root / 'cli-requirements.md').read_text(), re.M)
assert rows == [f'C{i:02}' for i in range(1, 27)]
mapped = re.findall(r'^\| (C\d\d) \|', (OUT / 'repair-5-test-matrix.md').read_text(), re.M)
assert mapped == rows
invariants = {}
baseline = '05f0812d0841485bbe07b3f1f8f6fde3861a1d55'
for path in ['crates/arc-storage/migrations/0001_phase0.sql',
             'scripts/harness/validate.sh', 'crates/arc-daemon/src/limits.rs',
             'crates/arc-daemon/src/platform.rs']:
    data = subprocess.check_output(['git', 'show', f'{baseline}:{path}'], cwd=ROOT)
    baseline_sha = hashlib.sha256(data).hexdigest()
    invariants[path] = {'baselineSha256': baseline_sha,
                        'currentSha256': hashes[path], 'unchanged': baseline_sha == hashes[path]}
    assert invariants[path]['unchanged']

retained = []
old_summary = json.loads((OUT / 'repair-4-summary.json').read_text())
for name, check in old_summary['checks'].items():
    receipt_path, log_path = OUT / check['receipt'], OUT / check['log']
    receipt = json.loads(receipt_path.read_text())
    logs_match = receipt['outputSha256'] == sha(log_path)
    sources_match = receipt['sourcesBefore'] == hashes and receipt['sourcesAfter'] == hashes
    assert logs_match
    retained.append({'name': name, 'receipt': check['receipt'], 'log': check['log'],
                     'exitCode': receipt['exitCode'], 'logHashMatches': logs_match,
                     'inputsMatchCurrent': sources_match,
                     'evidenceScope': 'retained; not rerun in attempt 5'})

checks = {}
total_tests = 0
for name in ['unit-ipc-cli', 'daemon-matrix', 'fmt', 'clippy', 'harness']:
    stem = f'repair-5-{name}'
    receipt = json.loads((OUT / (stem + '.json')).read_text())
    log = OUT / (stem + '.txt')
    assert receipt['outputSha256'] == sha(log)
    assert receipt['sourcesBefore'] == hashes == receipt['sourcesAfter']
    expected = 1 if name == 'harness' else 0
    assert receipt['exitCode'] == expected
    passed = sum(map(int, re.findall(r'test result: ok\. (\d+) passed;', log.read_text())))
    total_tests += passed
    checks[name] = {'receipt': stem + '.json', 'log': stem + '.txt',
                    'argv': receipt['argv'], 'exitCode': receipt['exitCode'],
                    'sourcesMatchCurrent': True, 'testsPassed': passed,
                    'logSha256': sha(log)}
assert total_tests == 31
assert 'test:coverage' in (OUT / 'repair-5-harness.txt').read_text()
inventory = {'baselineHead': baseline, 'head': prep['head'], 'inputs': current,
             'invariants': invariants, 'sourceLineLimitPassed': True,
             'rustSourceFiles': len(source), 'maximumRustLines': max(i['lines'] for i in source),
             'changesToProductOrSpecificationsThisAttempt': [],
             'sourceInputsMatchAttempt4': True, 'preservedPriorArtifacts': len(prep['priorArtifacts']),
             'priorArtifactsChanged': prior_changed, 'retainedReceiptValidation': retained,
             'fixtures': {'original': 45, 'disposition': 7, 'orderingVariants': 2,
                          'cliSupplementalLiterals': 14, 'totalSuccessMatrix': 68,
                          'malformed': 10, 'cliMatrixRows': 26}}
write('repair-5-source-inventory.json', inventory)
summary = {'status': 'partial', 'taskId': 'task-4', 'attempt': 5,
           'skillsUsed': ['oma-backend'], 'baselineHead': baseline, 'head': prep['head'],
           'sourceInputsMatch': True, 'reconciledSpecificationsAgree': True,
           'productOrSpecificationChangesThisAttempt': [], 'interfaceCompatibilityPreserved': True,
           'freshTestsPassed': total_tests, 'checks': checks,
           'retainedChecks': retained, 'fmtPassed': True, 'clippyPassed': True,
           'workspaceTestsPassed': False, 'tcpPrivacyVerified': False,
           'requiredCargoGatesPassed': False, 'independentReviewComplete': False,
           'migrationUnchanged': True, 'harnessUnchanged': True,
           'sourceLineLimitPassed': True, 'priorArtifactsUnchanged': True,
           'harnessCoverageStatus': 'unresolved',
           'workspaceGate': {'command': 'cargo test --workspace',
                             'status': 'not_run_no_established_tcp_listen_context',
                             'latestHistoricalReceipt': 'repair-3-final-workspace.json',
                             'latestHistoricalExitCode': 101,
                             'historicalFailureIsNotPassingEvidence': True},
           'tcpPrerequisite': {'freshProbeOrTcpTestRun': False,
                               'historicalReceipt': 'repair-4-tcp-prerequisite.json',
                               'historicalErrno': 1,
                               'currentNetworkDisabledFlag': prep['context']['networkDisabled'],
                               'effectiveTcpListeningContextEstablished': False},
           'chiefAction': 'Stop identical retries. Resume missing gates only after a supported '
                          'noninteractive context establishes effective local TCP listening.',
           'remainingObligations': ['TCP privacy assertions', 'exact workspace gate',
                                    'independent oma-qa', 'final mission verification',
                                    'harness coverage', 'remaining detector coverage', 'scheduling',
                                    'managed-execution', 'desktop', 'native Windows']}
write('repair-5-summary.json', summary)
print(json.dumps({'status': summary['status'], 'freshTestsPassed': total_tests,
                  'sourceFiles': len(source), 'maximumRustLines': inventory['maximumRustLines'],
                  'priorArtifactsPreserved': inventory['preservedPriorArtifacts'],
                  'retainedReceiptsMatchingCurrentInputs': sum(i['inputsMatchCurrent'] for i in retained)}))
