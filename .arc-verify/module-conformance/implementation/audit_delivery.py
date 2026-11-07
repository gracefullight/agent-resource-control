#!/usr/bin/env python3
"""Check implementation mappings, discovery, receipts, tracking and boundaries."""
import datetime as dt
import json
from pathlib import Path
import re
import runpy

HERE = Path(__file__).resolve().parent
BASE = HERE.parent / 'baseline'
api = runpy.run_path(str(BASE / 'capture.py'))
sha, ROOT = api['sha'], api['ROOT']
load = lambda name: json.loads((HERE / name).read_text())


def save(name, obj):
    (HERE / name).write_text(json.dumps(obj, ensure_ascii=False, indent=2) + '\n')


def output(folder, receipt):
    chunks = []
    for part in receipt['outputParts']:
        data = (folder / part['path']).read_bytes()
        assert sha(data) == part['sha256']
        assert data.count(b'\n') == part['lines'] <= 400
        chunks.append(data)
    data = b''.join(chunks)
    assert sha(data) == receipt['outputSha256']
    assert len(data) == receipt['outputBytes']
    return data.decode()


mapping = load('mapping.json')
fp = api['source_fingerprint']()
assert mapping['sourceFingerprint'] == fp
baseline_inventory = json.loads((BASE / 'inventory.json').read_text())
assert sorted(e['originalId'] for e in mapping['entries']) == sorted(e['id'] for e in baseline_inventory)
assert len(mapping['entries']) == 51
assert sum(len(e['comparison']['assertions']) for e in mapping['entries']) == 137
assert len(mapping['attributes']) == 28

# Preserve the dependency deliverables, including original receipt bytes.
baseline_artifacts = json.loads((BASE / 'artifacts.json').read_text())
for artifact in baseline_artifacts:
    assert sha((ROOT / artifact['path']).read_bytes()) == artifact['sha256'], artifact['path']

for item in load('protected-paths.json'):
    assert sha((ROOT / item['path']).read_bytes()) == item['baselineSha256']
ipc = (ROOT / api['IPC']).read_bytes().splitlines(keepends=True)
assert sha(b''.join(ipc[:354])) == json.loads((BASE / 'baseline.json').read_text())['ipcProductionPrefix']['sha256']

commands = []
for path in sorted(HERE.glob('*.json')):
    receipt = json.loads(path.read_text())
    if not isinstance(receipt, dict) or not all(k in receipt for k in ['command', 'outputParts', 'exitCode']):
        continue
    data = output(HERE, receipt)
    assert receipt['processReaped'] and not receipt['timedOut'], path.name
    assert receipt['sourceFingerprintBefore'] == receipt['sourceFingerprintAfter']
    commands.append({'receipt': path.name, 'command': receipt['command'],
                     'exitCode': receipt['exitCode'], 'sourceFingerprint': receipt['sourceFingerprintAfter'],
                     'outputSha256': receipt['outputSha256']})
for name, command in [
    ('fmt-final', ['cargo', 'fmt', '--all', '--', '--check']),
    ('clippy', ['cargo', 'clippy', '--workspace', '--all-targets', '--', '-D', 'warnings']),
    ('workspace-test', ['cargo', 'test', '--workspace'])]:
    receipt = load(name + '.json')
    assert receipt['command'] == command and receipt['exitCode'] == 0
    assert receipt['sourceFingerprintBefore'] == fp == receipt['sourceFingerprintAfter']

names = []
for original_path, kind in [(api['RESIDENT'], 'resident'), (api['IPC'], 'ipc')]:
    old_receipt = json.loads((BASE / (kind + '-discovery.json')).read_text())
    new_receipt = load(kind + '-discovery.json')
    assert new_receipt['exitCode'] == 0 and new_receipt['sourceFingerprintAfter'] == fp
    old_names = re.findall(r'^(.+): test$', output(BASE, old_receipt), re.MULTILINE)
    new_names = re.findall(r'^(.+): test$', output(HERE, new_receipt), re.MULTILINE)
    entries = [e for e in mapping['entries'] if e['kind'] == 'test' and e['original']['path'] == original_path]
    assert sorted(old_names) == sorted(e['original']['qualifiedName'] for e in entries)
    assert sorted(new_names) == sorted(e['destination']['qualifiedName'] for e in entries)
    for entry in entries:
        names.append({'before': entry['original']['qualifiedName'],
                      'after': entry['destination']['qualifiedName'],
                      'destinationPath': entry['destination']['path']})
assert sum(n['after'] == 'fixture_serialization_probe' for n in names) == 1
coordination = output(HERE, load('coordination-handoff.json'))
assert 'running 1 test' in coordination
assert 'coordination::fixture_coordination_is_shared_across_processes ... ok' in coordination
assert '1 passed; 0 failed' in coordination
tracked = output(HERE, load('tracked-modules.json'))
lengths = load('source-lengths.json')
for item in lengths:
    assert item['path'] in tracked
    data = (ROOT / item['path']).read_bytes()
    assert sha(data) == item['sha256']
    assert data.count(b'\n') == item['lines'] <= 500
    index_blob = api['git']('rev-parse', ':' + item['path']).decode().strip()
    working_blob = api['git']('hash-object', item['path']).decode().strip()
    assert index_blob == working_blob

harness_receipt = load('harness.json')
assert harness_receipt['command'] == ['./scripts/harness/validate.sh']
assert 'SKIP_COVERAGE' in harness_receipt['environmentRemoved']
assert 'SKIP_COVERAGE' not in harness_receipt['environmentOverrides']
assert harness_receipt['sourceFingerprintBefore'] == fp == harness_receipt['sourceFingerprintAfter']
harness = output(HERE, harness_receipt)
assert 'No oversized files found' in harness
length_violations = len(re.findall(r'File exceeds 500-line cap:|Grandfathered file grew above', harness))
assert length_violations == 0
# Preserve the actual exit and all unrelated findings; do not make this gate green.
summary = {'task': 'task-2', 'status': 'completed', 'skill': 'oma-refactor',
    'checkedAt': dt.datetime.now(dt.timezone.utc).isoformat(), 'sourceFingerprint': fp,
    'baselineRevision': mapping['baselineRevision'], 'mappedTests': 24, 'mappedHelpers': 27,
    'mappedAssertionSites': 137, 'mappedAttributes': 28, 'mappedDeclarations': len(mapping['declarations']),
    'productionAndProtectedFilesPreserved': True, 'baselineArtifactsUnchanged': True,
    'probeSelectorUnchanged': True, 'coordinationHandoffPassed': True,
    'requiredCargoChecksPassed': True, 'newModulesTrackedAndIndexMatchesWorkingBytes': True,
    'fileLengthViolations': length_violations, 'harnessExitCode': harness_receipt['exitCode'],
    'harnessOtherFailures': ['Tests or coverage check failed; bun run test:coverage reports missing script']
        if 'Script not found "test:coverage"' in harness else ['See complete harness output'] if harness_receipt['exitCode'] else [],
    'independentQa': 'pending subsequent assigned reviewer; not claimed by implementer',
    'globalCleanliness': 'unverified: av.yaml access denial',
    'failedAttempts': [{'receipt': 'fmt.json', 'exitCode': 1,
                        'resolution': 'rustfmt sorted module wiring and removed boundary blank lines; fmt-final passed'}]}
save('result.json', summary)
(HERE / 'discovery.json').write_text('[\n' + ',\n'.join(json.dumps(n) for n in names) + '\n]\n')
(HERE / 'commands.json').write_text('[\n' + ',\n'.join(json.dumps(c) for c in commands) + '\n]\n')
print(json.dumps(summary, indent=2))
