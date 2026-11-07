"""Validate current repair receipts without rewriting prior evidence."""
import hashlib
import json
import pathlib
import re

root = pathlib.Path(__file__).resolve().parents[3]
out = pathlib.Path(__file__).resolve().parent
inventory = json.loads((out/'repair-3-source-inventory.json').read_text())
current = {x['path']:x['sha256'] for x in inventory['inputs']}
assert all(hashlib.sha256((root/path).read_bytes()).hexdigest() == digest
           for path,digest in current.items())
previous = json.loads((out/'repair-2-source-inventory.json').read_text())
old = {x['path']:x['sha256'] for x in previous['inputs']}
changed = sorted(path for path in current if current[path] != old.get(path))
assert changed == sorted(['crates/arc-detector/src/rules.rs',
                         'crates/arc-detector/src/tests.rs',
                         'crates/arc-daemon/tests/classification.rs',
                         'crates/arc-ipc/tests/classification.rs'])
checks = {}
for suffix in ['ssh-green','focused','final-fmt','final-clippy','final-workspace','all-tests','harness']:
    name = 'repair-3-' + suffix
    receipt = json.loads((out/(name+'.json')).read_text())
    assert receipt['sourcesMatch']
    assert receipt['sourcesBefore'] == current == receipt['sourcesAfter']
    assert receipt['outputSha256'] == hashlib.sha256((out/(name+'.txt')).read_bytes()).hexdigest()
    checks[suffix] = {key:receipt[key] for key in ['argv','exitCode','elapsedSeconds','outputSha256']}
    checks[suffix].update(receipt=name+'.json',log=name+'.txt',sourcesMatchCurrent=True)
red = json.loads((out/'repair-3-ssh-red.json').read_text())
assert red['exitCode'] == 101 and checks['ssh-green']['exitCode'] == 0
assert red['argv'] == checks['ssh-green']['argv']
assert 'left: SshForwarding' in (out/'repair-3-ssh-red.txt').read_text()
assert 'right: SshAmbiguous' in (out/'repair-3-ssh-red.txt').read_text()
assert checks['final-fmt']['exitCode'] == checks['final-clippy']['exitCode'] == 0
text = (out/'repair-3-all-tests.txt').read_text()
counts = re.findall(r'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;',text)
totals = {name:sum(int(row[index]) for row in counts)
          for index,name in enumerate(['passed','failed','ignored'])}
assert totals == {'passed':204,'failed':1,'ignored':0}, totals
assert text.count('test privacy::required_loopback_endpoint_nonexecution_and_privacy ... FAILED') == 1
assert 'PermissionDenied' in text and 'Operation not permitted' in text
for test in ['ssh_option_values_are_not_forwarding_flags',
             'ssh_option_values_preserve_typed_daemon_and_cli_observations',
             'tests::syntax_quotes_aggregation_and_ssh_negative_matrix',
             'saturation::maximum_and_exhausting_classification_load_preserves_health_and_shutdown']:
    assert 'test '+test+' ... ok' in text
assert checks['final-workspace']['exitCode'] == checks['all-tests']['exitCode'] == 101
assert checks['harness']['exitCode'] == 1
assert 'Script not found "test:coverage"' in (out/'repair-3-harness.txt').read_text()
observations = {}
for path in sorted((out/'repair-3-observations').glob('classification-*.json')):
    observations[path.stem] = {'path':str(path.relative_to(out)),
                              'sha256':hashlib.sha256(path.read_bytes()).hexdigest(),
                              'value':json.loads(path.read_text())}
load = observations['classification-saturation']['value']
assert load['sameConnectionRecovery'] and load['classificationLoadContinued']
assert load['completed'] > load['classificationsAtRecovery'] >= load['classificationsBeforeRecovery'] >= 16
assert load['signalToExitUs'] <= 3500000
summary = {
    'status':'partial','taskId':'task-4','attempt':3,'skillsUsed':['oma-backend'],
    'baselineHead':inventory['baselineHead'],'head':inventory['head'],
    'sourceInputsMatch':True,'sourceChangesSincePreviousAttempt':changed,
    'fmtPassed':True,'clippyPassed':True,'workspaceTestsPassed':False,
    'requiredCargoGatesPassed':False,'fullSuite':totals,'sshRepairVerified':True,
    'interfaceChanged':False,'inputArtifactsChanged':False,'independentReviewComplete':False,
    'migrationUnchanged':inventory['invariants']['crates/arc-storage/migrations/0001_phase0.sql']['unchanged'],
    'harnessUnchanged':inventory['invariants']['scripts/harness/validate.sh']['unchanged'],
    'sourceLineLimitPassed':inventory['sourceLineLimitPassed'],
    'rustSourceFiles':sum(x['path'].endswith('.rs') for x in inventory['inputs']),
    'maximumRustLines':max(x['lines'] for x in inventory['inputs'] if x['path'].endswith('.rs')),
    'harness':{'command':'./scripts/harness/validate.sh','exitCode':1,'coverageStatus':'unresolved'},
    'checks':checks,'observations':observations,
    'blockers':[
        {'path':'crates/arc-daemon/tests/classification/privacy.rs:95',
         'description':'Required TCP loopback monitor bind is denied with errno 1; no permitted escalation or alternate execution environment is supplied.'},
        {'paths':['.arc-verify/command-classification/contract.md:115',
                  '.arc-verify/command-classification/contract.md:117',
                  '.arc-verify/command-classification/cli-requirements.md:46',
                  '.arc-verify/command-classification/cli-requirements.md:49',
                  '.arc-verify/command-classification/cli-requirements.md:69',
                  '.arc-verify/command-classification/cli-requirements.md:80'],
         'description':'Chief disposition and input-owner synchronization remain required for contradictory empty/oversized input and human output requirements; no disposition was supplied.'}],
    'remainingObligations':['permitted TCP privacy execution','Chief interface disposition and input-owner synchronization',
                            'independent oma-qa review','harness coverage','broader detector catalog',
                            'scheduling','managed-execution','desktop','native Windows'],
    'reports':['repair-3-report.md','repair-3-static-inspection.md','repair-3-source-inventory.json']}
(out/'repair-3-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
claim = {'status':'partial','changedFiles':changed+sorted({str(path.relative_to(root)) for path in out.rglob('*') if path.is_file() and path.relative_to(out).parts[0].startswith('repair-3-')} | {str((out/'repair-3-claim.json').relative_to(root))}),
         'unresolved':[x['description'] for x in summary['blockers']]+
                      ['Harness test:coverage remains missing; independent review is separately assigned.'],
         'artifacts':[str((out/name).relative_to(root)) for name in
                      ['repair-3-report.md','repair-3-summary.json','repair-3-source-inventory.json',
                       'repair-3-static-inspection.md']]}
(out/'repair-3-claim.json').write_text(json.dumps(claim,indent=2)+'\n')
print(json.dumps({'status':summary['status'],'fullSuite':totals,'sourceInputsMatch':True,
                  'sshRepairVerified':True,'maxRustLines':summary['maximumRustLines']}))
