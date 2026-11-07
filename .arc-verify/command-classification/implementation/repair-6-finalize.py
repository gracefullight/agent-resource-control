"""Validate attempt-6 source binding, unchanged boundaries and actual gate outcomes."""
import hashlib
import json
from pathlib import Path
import re
import subprocess

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[2]
BASELINE = '05f0812d0841485bbe07b3f1f8f6fde3861a1d55'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(name, value):
    with (OUT / name).open('x') as stream:
        stream.write(json.dumps(value, indent=2) + '\n')


prep = json.loads((OUT / 'repair-6-preparation.json').read_text())
current = []
for item in prep['inputs']:
    path = ROOT / item['path']
    data = path.read_bytes()
    current.append({'path': item['path'], 'sha256': sha(path),
                    'bytes': len(data), 'lines': len(data.splitlines())})
assert current == prep['inputs']
hashes = {item['path']: item['sha256'] for item in current}
rust = [item for item in current if item['path'].endswith('.rs')]
assert {str(p.relative_to(ROOT)) for p in ROOT.glob('crates/**/*.rs')} == {
    item['path'] for item in rust}
assert all(item['lines'] <= 500 for item in rust)
assert all(len(p.read_bytes().splitlines()) <= 500 for p in OUT.glob('repair-6-*.py'))
assert all(sha(ROOT / item['path']) == item['sha256'] for item in prep['priorArtifacts'])
contract = json.loads((OUT.parent / 'contract.json').read_text())
assert contract['markdownSha256'] == sha(OUT.parent / 'contract.md')
assert len(contract['fixtures']) == 45
assert len(contract['dispositionFixtures']) == 7
assert len(contract['malformedPayloadFixtures']) == 10
rows = re.findall(r'^\| (C\d\d) \|', (OUT.parent / 'cli-requirements.md').read_text(), re.M)
assert rows == [f'C{i:02}' for i in range(1, 27)]
assert rows == re.findall(r'^\| (C\d\d) \|', (OUT / 'repair-6-test-matrix.md').read_text(), re.M)
invariants = {}
for path in ['crates/arc-storage/migrations/0001_phase0.sql',
             'scripts/harness/validate.sh', 'crates/arc-daemon/src/limits.rs',
             'crates/arc-daemon/src/platform.rs']:
    data = subprocess.check_output(['git', 'show', f'{BASELINE}:{path}'], cwd=ROOT)
    baseline_sha = hashlib.sha256(data).hexdigest()
    invariants[path] = {'baselineSha256': baseline_sha, 'sha256': hashes[path],
                        'unchanged': baseline_sha == hashes[path]}
    assert invariants[path]['unchanged']
checks = {}
for name in ['tcp', 'fmt', 'clippy', 'workspace', 'harness', 'work-trace']:
    stem = 'repair-6-' + name
    receipt = json.loads((OUT / (stem + '.json')).read_text())
    log = OUT / (stem + '.txt')
    assert receipt['outputSha256'] == sha(log)
    assert receipt['sourcesBefore'] == hashes == receipt['sourcesAfter']
    assert receipt['exitCode'] == (1 if name == 'harness' else 0)
    passed = sum(map(int, re.findall(r'test result: ok\. (\d+) passed;', log.read_text())))
    checks[name] = {'receipt': stem + '.json', 'log': stem + '.txt',
                    'argv': receipt['argv'], 'exitCode': receipt['exitCode'],
                    'elapsedSeconds': receipt['elapsedSeconds'],
                    'sourcesMatchCurrent': True, 'testsPassed': passed,
                    'logSha256': sha(log)}
assert checks['workspace']['testsPassed'] == 205
assert checks['tcp']['testsPassed'] == 1
assert checks['work-trace']['testsPassed'] == 1
assert 'test:coverage' in (OUT / 'repair-6-harness.txt').read_text()
assert json.loads((OUT / 'repair-6-tcp-prerequisite.json').read_text())['success']
trace = re.search(r'work-exhaustion: bytes=(\d+) stats=AnalysisStats \{ ([^}]+) \}',
                  (OUT / 'repair-6-work-trace.txt').read_text())
assert trace
inventory = {'baselineHead': BASELINE, 'head': prep['head'], 'inputs': current,
             'invariants': invariants, 'sourceLineLimitPassed': True,
             'rustSourceFiles': len(rust), 'maximumRustLines': max(i['lines'] for i in rust),
             'sourceInputsMatchAttempt5': True, 'priorArtifactsUnchanged': True,
             'preservedPriorArtifacts': len(prep['priorArtifacts']),
             'productOrSpecificationChangesThisAttempt': [],
             'fixtures': {'success': 68, 'malformed': 10, 'cliRows': 26},
             'productionWorkTrace': {'bytes': int(trace.group(1)), 'counters': trace.group(2)}}
write('repair-6-source-inventory.json', inventory)
summary = {'status': 'completed', 'scope': 'task-4 implementation-owner handoff only',
           'taskId': 'task-4', 'attempt': 6, 'skillsUsed': ['oma-backend'],
           'baselineHead': BASELINE, 'head': prep['head'], 'sourceInputsMatch': True,
           'reconciledSpecificationsAgree': True, 'productOrSpecificationChangesThisAttempt': [],
           'interfaceCompatibilityPreserved': True, 'checks': checks,
           'workspaceTestsPassed': True, 'workspaceTestsCount': 205,
           'tcpPrivacyVerified': True, 'behaviorVerifiedByImplementationOwner': True,
           'compatibilityVerifiedByImplementationOwner': True,
           'fmtPassed': True, 'clippyPassed': True, 'requiredCargoGatesPassed': True,
           'migrationUnchanged': True, 'harnessUnchanged': True,
           'sourceLineLimitPassed': True, 'priorArtifactsUnchanged': True,
           'harnessCoverageStatus': 'unresolved', 'independentReviewComplete': False,
           'tcpPrerequisite': {'freshProbeSucceeded': True, 'unchangedTestPassed': True,
                               'historicalDenialSupersededForCurrentContext': True},
           'chiefAction': 'Proceed to independent oma-qa and final source-bound verification.',
           'remainingObligations': ['independent oma-qa', 'final mission verification',
                                    'harness coverage', 'remaining detector coverage',
                                    'scheduling', 'managed-execution', 'desktop', 'native Windows']}
write('repair-6-summary.json', summary)
print(json.dumps({'status': summary['status'], 'workspaceTests': 205,
                  'rustFiles': len(rust), 'maximumRustLines': inventory['maximumRustLines'],
                  'priorArtifactsPreserved': inventory['preservedPriorArtifacts'],
                  'workTrace': inventory['productionWorkTrace']}))
