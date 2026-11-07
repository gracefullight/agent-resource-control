"""Read-only receiving-checkout check for task-8 attempt-2 artifacts."""
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
OUT = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('run', OUT.parent/'run.py')
r = importlib.util.module_from_spec(spec)
spec.loader.exec_module(r)
ROOT = r.ROOT
read = lambda p: json.loads(p.read_text())
index = read(OUT/'evidence.json')
for item in index['artifacts']:
    p = ROOT/item['path']
    assert p.stat().st_size == item['bytes']
    assert hashlib.sha256(p.read_bytes()).hexdigest() == item['sha256'], item['path']
assert r.fingerprint() == read(OUT/'context.json')['source']
assert index['sourceFingerprint'] == r.fingerprint()['sha256']
for name, expected in [('fmt',0),('clippy',0),('workspace',0),('harness',1)]:
    receipt=read(OUT/(name+'.json'))
    assert receipt['exitCode']==expected and not receipt['timedOut']
    assert receipt['sourceBefore']==receipt['sourceAfter']==index['sourceFingerprint']
    assert receipt['productUnchanged'] and receipt['processExitObserved']
    assert receipt['outputSha256']==hashlib.sha256((OUT/(name+'.txt')).read_bytes()).hexdigest()
qa=read(OUT.parents[1]/'qa.json')
for key in ['behavioralContractVerified','compatibilityVerified','migrationUnchanged',
    'cargoFmtPassed','cargoClippyPassed','cargoTestsPassed','harnessExecuted',
    'harnessUnchanged','unresolvedHarnessFailuresRetained']:
    assert qa[key] is True, key
assert not qa['harnessPassed'] and qa['harnessExitCode']==1 and len(qa['findings'])==3
assert qa['status']=='partial' and not qa['missionCompletionClaimed']
assert qa['testCounts']=={'passed':173,'failed':0,'ignored':0,'measured':0,'filtered':0}
perf=read(OUT.parents[1]/'performance.json')
assert perf['nativeMeasurementsRecorded'] and perf['srsComparisonComplete']
raw=OUT/'workspace-raw'
samples=read(raw/'small-message-samples.json')
assert len(samples['samples_ns'])==10000
assert sorted(samples['samples_ns'])[9499]==perf['small_message']['p95_ns']
obs=read(raw/'native-observations.json')
assert max(x['rss_bytes']for x in obs['idle'])==perf['idle']['max_rss_bytes']
for name in ['SIGINT','SIGTERM']:
    assert qa['signalCases'][name]['signalToReapedExitUs']<5000000
assert (OUT.parents[1]/'harness.txt').read_bytes()==(OUT/'harness.txt').read_bytes()
print(json.dumps({'status':'verified-current-checkout','sourceFingerprint':index['sourceFingerprint'],
    'sourceFiles':len(r.fingerprint()['files']),'artifacts':len(index['artifacts']),
    'cargoGatesPassed':True,'harnessExit':1,
    'remaining':['three harness findings','av.yaml target applicability/access','task-9 evidence refresh','Phase 0/Linux/Windows']}))
