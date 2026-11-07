#!/usr/bin/env python3
"""Independent hash/receipt audit. Writes only this review's audit.json."""
import datetime
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path.cwd()
OUT = ROOT / '.arc-verify/schema-compatibility/review'
VERIFY = ROOT / '.arc-verify/schema-compatibility/verification'
BASE = 'bf28a35b594f5ae83ac97c216bd3b09e60a2dfa1'


def sha(data):
    return hashlib.sha256(data).hexdigest()


def load(name):
    return json.loads((VERIFY / name).read_text())


def properties(path):
    data = path.read_bytes()
    return {'sha256': sha(data), 'bytes': len(data), 'lines': len(data.splitlines())}


manifest = load('source-manifest.json')
current = {path: properties(ROOT / path) for path in manifest}
assert current == manifest, 'current source differs from task-6 manifest'
# Inventory for hash coverage only, not source/symbol discovery.
crate_inputs = {str(p.relative_to(ROOT)) for p in (ROOT / 'crates').rglob('*')
                if p.is_file() and p.suffix in {'.rs', '.sql', '.toml'}}
assert crate_inputs <= manifest.keys(), 'unbound Cargo input'
summary = load('summary.json')
fingerprint = sha((VERIFY / 'source-manifest.json').read_bytes())
assert fingerprint == summary['sourceManifestSha256']
assert load('source-manifest.before.json') == current

commands = []
expected = {
    'fmt': (['cargo', 'fmt', '--all', '--', '--check'], 0),
    'clippy': (['cargo', 'clippy', '--workspace', '--all-targets', '--', '-D', 'warnings'], 0),
    'workspace-test': (['cargo', 'test', '--workspace'], 0),
    'harness': (['./scripts/harness/validate.sh'], 1),
}
for label, (argv, code) in expected.items():
    receipt = load(label + '.json')
    assert receipt['argv'] == argv
    assert receipt['cwd'] == str(ROOT)
    assert receipt['exitCode'] == code
    assert receipt['reaped'] and not receipt['timedOut']
    assert receipt['sourcesBefore'] == receipt['sourcesAfter'] == current
    data = (ROOT / receipt['log']).read_bytes()
    assert sha(data) == receipt['logSha256']
    assert len(data) == receipt['logBytes']
    commands.append({'command': receipt['command'], 'exitCode': code,
                     'receiptSha256': sha((VERIFY / (label + '.json')).read_bytes()),
                     'logSha256': sha(data), 'currentSourcesMatch': True,
                     'reused': True})

artifacts = load('artifact-manifest.json')
for path, recorded in artifacts.items():
    actual = properties(VERIFY / path)
    assert all(actual[k] == v for k, v in recorded.items()), path

immutable = {}
for path in load('audit.json')['immutableInputs']:
    base = subprocess.check_output(['git', 'show', BASE + ':' + path])
    data = (ROOT / path).read_bytes()
    assert data == base, path
    immutable[path] = {'sha256': sha(data), 'matchesBase': True, 'bytes': len(data)}
assert immutable['crates/arc-storage/migrations/0001_phase0.sql']['sha256'] == (
    '046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384')
lengths = load('audit.json')['sourceFileLengths']
for path, recorded in lengths.items():
    assert properties(ROOT / path) == recorded, path
    assert recorded['lines'] <= 500, path
assert all(v['lines'] <= 500 for p, v in current.items() if p.endswith('.rs'))
assert re.search(r'pub const PROTOCOL_VERSION: u32 = 1;',
                 (ROOT / 'crates/arc-ipc/src/lib.rs').read_text())

log = (VERIFY / 'workspace-test.txt').read_text()
lines = log.splitlines()
passed = re.findall(r'^test (.+) \.\.\. ok$', log, re.M)
assert len(passed) == 247
assert not re.search(r'^test .+ \.\.\. (FAILED|ignored)$', log, re.M)
totals = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', log)
assert sum(int(t[0]) for t in totals) == 247
assert all(t[1:] == ('0', '0') for t in totals)
cases = load('acceptance-cases.json')
references = 0
for case in cases:
    for test in case['tests']:
        assert lines[test['logLine'] - 1] == 'test ' + test['test'] + ' ... ok'
        references += 1
assert len(cases) == 26
harness = load('harness.json')
assert harness['environment']['SKIP_COVERAGE'] is None
assert harness['unchanged'] and harness['actualResultRecorded']
assert b'Script not found "test:coverage"' in (VERIFY / 'harness.txt').read_bytes()

result = {
    'checkedAt': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'command': 'python3 .arc-verify/schema-compatibility/review/audit.py',
    'sourceFingerprint': fingerprint,
    'fingerprintDefinition': 'SHA-256 of exact verification/source-manifest.json bytes',
    'sourceInputCount': len(current), 'allCrateInputsBound': True,
    'currentSourcesMatch': True, 'receiptsMatchCurrentSources': True,
    'artifactManifestEntriesVerified': len(artifacts),
    'immutableInputs': immutable, 'sourceLengthInputsVerified': len(lengths),
    'sourceFilesWithin500Lines': True, 'protocolVersion': 1,
    'testInvocationsPassed': len(passed), 'distinctTestsClaimed': False,
    'acceptanceGroupsWithValidatedLogReferences': len(cases),
    'validatedTestReferences': references, 'commands': commands,
    'harnessPassed': False, 'coverageExecuted': False,
    'coverageIssue': 'Missing test:coverage; unchanged harness exited 1',
    'executedProductChecksDuringReview': [], 'allAssertionsPassed': True,
}
(OUT / 'audit.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
