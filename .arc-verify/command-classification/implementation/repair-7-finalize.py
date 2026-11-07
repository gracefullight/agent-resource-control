"""Bind the SSH repair handoff to actual sources, regressions and check results."""
import datetime
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


prep = json.loads((OUT / 'repair-7-preparation.json').read_text())
paths = sorted({i['path'] for i in prep['inputs']} |
               {'crates/arc-detector/tests/support/ssh.rs'})
current = []
for relative in paths:
    path = ROOT / relative
    data = path.read_bytes()
    current.append(dict(path=relative, sha256=sha(path), bytes=len(data),
                        lines=len(data.splitlines())))
hashes = {i['path']: i['sha256'] for i in current}
before = {i['path']: i['sha256'] for i in prep['inputs']}
changed = [p for p in paths if hashes[p] != before.get(p)]
assert changed == sorted([
    'crates/arc-detector/src/rules.rs', 'crates/arc-detector/src/tests.rs',
    'crates/arc-detector/tests/support/contract.rs',
    'crates/arc-detector/tests/support/ssh.rs',
    'crates/arc-ipc/tests/classification.rs',
    'crates/arc-daemon/tests/classification.rs'])
rust = [i for i in current if i['path'].endswith('.rs')]
assert {str(p.relative_to(ROOT)) for p in ROOT.glob('crates/**/*.rs')} == {
    i['path'] for i in rust}
assert all(i['lines'] <= 500 for i in rust)
assert all(len(p.read_bytes().splitlines()) <= 500 for p in OUT.glob('repair-7-*.py'))
assert all(sha(ROOT / i['path']) == i['sha256'] for i in prep['priorArtifacts'])
contract = json.loads((OUT.parent / 'contract.json').read_text())
assert contract['markdownSha256'] == sha(OUT.parent / 'contract.md')
invariants = {}
for relative in [
    'crates/arc-storage/migrations/0001_phase0.sql', 'scripts/harness/validate.sh',
    'crates/arc-daemon/src/admission.rs', 'crates/arc-daemon/src/limits.rs',
    'crates/arc-daemon/src/platform.rs', 'crates/arc-daemon/src/lifecycle.rs',
]:
    data = subprocess.check_output(['git', 'show', f'{BASELINE}:{relative}'], cwd=ROOT)
    baseline_sha = hashlib.sha256(data).hexdigest()
    assert baseline_sha == hashes[relative]
    invariants[relative] = dict(sha256=hashes[relative], baselineSha256=baseline_sha,
                                unchanged=True)
checks = {}
names = ['green-detector-final', 'green-ipc', 'green-daemon', 'tcp', 'fmt',
         'clippy', 'workspace', 'harness', 'work-trace-final', 'diff-check']
for name in names:
    stem = 'repair-7-' + name
    receipt = json.loads((OUT / (stem + '.json')).read_text())
    log = OUT / (stem + '.txt')
    assert receipt['outputSha256'] == sha(log)
    assert receipt['sourcesBefore'] == hashes == receipt['sourcesAfter']
    assert receipt['exitCode'] == (1 if name == 'harness' else 0)
    count = sum(map(int, re.findall(r'test result: ok\. (\d+) passed;', log.read_text())))
    checks[name] = dict(receipt=stem + '.json', log=stem + '.txt', argv=receipt['argv'],
                        exitCode=receipt['exitCode'], testsPassed=count,
                        elapsedSeconds=receipt['elapsedSeconds'],
                        sourcesMatchCurrent=True, logSha256=sha(log))
assert checks['workspace']['testsPassed'] == 209
assert checks['tcp']['testsPassed'] == 1
assert 'test:coverage' in (OUT / 'repair-7-harness.txt').read_text()
red = {}
for layer in ['detector', 'ipc', 'daemon']:
    stem = 'repair-7-red-' + layer
    receipt = json.loads((OUT / (stem + '.json')).read_text())
    assert receipt['exitCode'] == 101 and receipt['sourcesMatch']
    assert receipt['sourcesBefore']['crates/arc-detector/src/rules.rs'] == before[
        'crates/arc-detector/src/rules.rs']
    assert receipt['outputSha256'] == sha(OUT / (stem + '.txt'))
    assert 'FAILED' in (OUT / (stem + '.txt')).read_text()
    red[layer] = dict(receipt=stem + '.json', log=stem + '.txt', exitCode=101,
                       preRepairClassifierSha256=before['crates/arc-detector/src/rules.rs'])
rows = [json.loads(line.removeprefix('ssh-boundary: ')) for line in
        (OUT / 'repair-7-green-detector-final.txt').read_text().splitlines()
        if line.startswith('ssh-boundary: ')]
assert len(rows) == 99
cases = {row['case']['id']: row for row in rows}
assert len(cases) == 99
assert all(row['actual'] == row['case']['result'] for row in rows)
probe = json.loads((OUT.parent / 'verification/ssh-post-destination.json').read_text())
retained = []
for old in probe['cases']:
    row = cases[old['id']]
    assert row['case']['payload']['command'] == old['syntheticInput']
    assert all(row['actual'][k] == v for k, v in old['expectedFields'].items())
    retained.append(dict(id=old['id'], command=old['syntheticInput'],
                         expected=row['case']['result'], allFourLayers=True))
assert len(retained) == 17
mapping = dict(retainedProbeSha256=sha(OUT.parent / 'verification/ssh-post-destination.json'),
               fixturePath='crates/arc-detector/tests/support/ssh.rs',
               fixtureSha256=hashes['crates/arc-detector/tests/support/ssh.rs'],
               tests=dict(detector='tests::ssh_post_destination_matrix',
                          work='tests::ssh_post_destination_work_checks_refuse_before_overrun',
                          ipc='ssh_post_destination_typed_dispatch',
                          daemonAndActualHumanJsonCli='ssh_post_destination_daemon_human_and_json_cli_matrix'),
               retainedProbeCases=retained, permanentCaseCount=99, cases=rows,
               coverage='Every case: deterministic typed detector, typed protocol-1 dispatch, actual daemon IPC, actual arcctl human and JSON; no synthetic SSH input executed.')
write('repair-7-regression-mapping.json', mapping)
inventory = dict(baselineHead=BASELINE, currentHead=prep['head'], inputs=current,
                 sourceChanges=[dict(path=p, beforeSha256=before.get(p),
                                     afterSha256=hashes[p]) for p in changed],
                 invariants=invariants, rustSourceFiles=len(rust),
                 maximumRustLines=max(i['lines'] for i in rust), sourceLineLimitPassed=True,
                 priorArtifactsUnchanged=True, priorArtifactsPreserved=len(prep['priorArtifacts']))
write('repair-7-source-inventory.json', inventory)
static = dict(
    scope='Implementation-owner source inspection; independent QA remains task-5.',
    inspectedInputs={p: hashes[p] for p in paths if p.startswith('crates/arc-detector/')},
    dependency='Production arc-detector uses existing Serde only; no manifest or lockfile change this attempt.',
    path='classify/analyze -> bounded lexer/parser -> recognize/ssh -> fixed enums -> aggregation',
    newOperations='Bounded slice advancement, token/empty checks, charged -- equality, charged leading-byte check; existing opaque-argument scan.',
    work='Every added token/empty visit is precharged 2; -- comparison uses Budget.eq; prefix length/byte check is precharged 2; no work after refused charge.',
    forbiddenEffects='No new process, shell, filesystem, environment, resolver, network, endpoint, logging, cache or storage calls in the detector.',
    privacy='Existing unmodified executable traps, substitution markers, Unix/TCP monitors and live SQLite/WAL/log canary scans pass on final sources.',
    compatibility='Protocol 1, request/result types, dispatch(request, doctor), version-first validation and payload disposal unchanged from preparation; admission/authentication/lifecycle/limits equal baseline.',
    interpretation='Recognizes command syntax only; UNKNOWN/OBSERVE conveys uncertainty without admission or execution decisions.')
write('repair-7-static-inspection.json', static)
summary = dict(status='completed', scope='task-4 implementation repair and handoff', taskId='task-4',
               attempt=7, skillsUsed=['oma-backend'], baselineHead=BASELINE,
               head=prep['head'], changedPaths=changed, sourceInputsMatch=True,
               priorArtifactsPreserved=len(prep['priorArtifacts']),
               acceptedSpecificationsUnchanged=True, interfaceCompatibilityPreserved=True,
               sshPostDestinationRepairedByImplementationOwner=True,
               retainedProbeCasesCovered=17, permanentSshCases=99, redEvidence=red,
               checks=checks, workspaceTestsCount=209, requiredCargoGatesPassed=True,
               tcpPrivacyVerified=True, migrationUnchanged=True, harnessUnchanged=True,
               sourceLineLimitPassed=True, harnessCoverageStatus='unresolved',
               independentReviewComplete=False, chiefAction='Deliver repaired sources and receipts to task-5 for fresh independent oma-qa.',
               remainingObligations=['independent QA on repaired sources', 'final mission verification',
                                     'harness coverage', 'remaining detector coverage', 'scheduling',
                                     'managed-execution', 'desktop', 'native Windows'])
write('repair-7-summary.json', summary)
print(json.dumps(dict(changedPaths=changed, retainedProbeCases=17, permanentSshCases=99,
                      workspaceTests=209, rustFiles=len(rust),
                      maximumRustLines=inventory['maximumRustLines'],
                      priorArtifactsPreserved=len(prep['priorArtifacts']),
                      harnessExit=checks['harness']['exitCode'], independentQA=False)))
