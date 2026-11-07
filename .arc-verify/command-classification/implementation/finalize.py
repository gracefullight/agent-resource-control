"""Summarize actual receipts without changing product or mission state."""
import hashlib
import json
import pathlib
import re
import subprocess

root = pathlib.Path(__file__).resolve().parents[3]
out = pathlib.Path(__file__).resolve().parent
inventory = json.loads((out/'source-inventory.json').read_text())
current = {x['path']:x['sha256'] for x in inventory['inputs']}
checks = {}
for name in ['verified-fmt','verified-clippy','verified-all-tests','verified-workspace-tests','final-harness']:
    receipt = json.loads((out/(name+'.json')).read_text())
    assert receipt['sourcesMatch']
    assert receipt['sourcesBefore'] == current == receipt['sourcesAfter']
    assert receipt['outputSha256'] == hashlib.sha256((out/(name+'.txt')).read_bytes()).hexdigest()
    checks[name] = {'argv':receipt['argv'], 'exitCode':receipt['exitCode'],
                    'elapsedSeconds':receipt['elapsedSeconds'], 'sourceInputsMatch':True,
                    'receipt':name+'.json', 'log':name+'.txt'}
text = (out/'verified-all-tests.txt').read_text()
counts = re.findall(r'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;', text)
totals = {name:sum(int(row[index]) for row in counts)
          for index,name in enumerate(['passed','failed','ignored'])}
assert totals == {'passed':202, 'failed':1, 'ignored':0}
assert text.count('test privacy::required_loopback_endpoint_nonexecution_and_privacy ... FAILED') == 1
assert 'Operation not permitted' in text
assert 'Script not found "test:coverage"' in (out/'final-harness.txt').read_text()
tracked = subprocess.check_output(['git','diff','--name-only','--','Cargo.toml','Cargo.lock','crates'],cwd=root,text=True).splitlines()
untracked = subprocess.check_output(['git','ls-files','--others','--exclude-standard','--','crates'],cwd=root,text=True).splitlines()
changed = sorted(set(tracked+untracked))
trace = json.loads((out/'source-audit-detector.json').read_text())
detector_inputs = [key for key in current if key.startswith('crates/arc-detector/') or key in ['Cargo.toml','Cargo.lock','.arc-verify/command-classification/contract.json']]
assert all(trace['sourcesAfter'][key] == current[key] for key in detector_inputs)
summary = {
    'status':'partial', 'taskId':'task-4', 'actor':'backend', 'skillUsed':['oma-backend'],
    'baselineHead':inventory['baselineHead'], 'head':inventory['head'],
    'implementationPresent':True, 'finalSourceInputsMatch':True,
    'fmtPassed':checks['verified-fmt']['exitCode']==0,
    'clippyPassed':checks['verified-clippy']['exitCode']==0,
    'workspaceTestsPassed':checks['verified-workspace-tests']['exitCode']==0,
    'requiredCargoGatesPassed':False, 'fullSuite':totals,
    'compatibilityRegressionSuitesPassed':True,
    'allNonexecutionEvidenceComplete':False,
    'migrationUnchanged':inventory['invariants']['crates/arc-storage/migrations/0001_phase0.sql']['unchanged'],
    'harnessUnchanged':inventory['invariants']['scripts/harness/validate.sh']['unchanged'],
    'sourceLineLimitPassed':inventory['sourceLineLimitPassed'],
    'rustSourceFiles':sum(x['path'].endswith('.rs') for x in inventory['inputs']),
    'maximumRustLines':max(x['lines'] for x in inventory['inputs'] if x['path'].endswith('.rs')),
    'harness':{'command':'./scripts/harness/validate.sh','exitCode':checks['final-harness']['exitCode'], 'coverageStatus':'unresolved'},
    'checks':checks, 'changedProductPaths':changed,
    'preservedInputs':{key:current[key] for key in current if key.startswith('.arc-verify/')},
    'workTrace':{'generator':'crates/arc-detector/src/tests.rs::production_work_exhaustion_trace',
                 'commandBytes':12033,'work':60203,'tokens':6,'invocations':2,'maxDepth':1,
                 'reason':'WORK_LIMIT','detectorSourceInputsMatch':True},
    'blockers':[
        {'path':'crates/arc-daemon/tests/classification/privacy.rs:95',
         'description':'Required loopback TCP monitor bind denied with errno 1 Operation not permitted; test remains mandatory and failing.'},
        {'paths':['.arc-verify/command-classification/contract.md','.arc-verify/command-classification/cli-requirements.md'],
         'description':'Human label/UNKNOWN explanation and empty/whitespace/input-limit outcome requirements conflict. Concrete task-1 contract implemented; Chief reconciliation remains.'}],
    'independentReviewComplete':False,
    'remainingObligations':['required TCP endpoint evidence','Chief interface-conflict resolution','independent oma-qa review','harness coverage repair outside this slice','broader detector catalog','scheduling','managed-execution','desktop','native Windows'],
    'reports':['report.md','test-matrix.md','static-inspection.md','preparation.md'],
}
(out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps({'status':summary['status'],'productPaths':len(changed),'fullSuite':totals,'sourceInputsMatch':True,'cargoGatesPassed':False,'harnessExit':summary['harness']['exitCode']}))
