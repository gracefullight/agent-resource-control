"""Read-only check of task-8 delivered files in the receiving checkout."""
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys

sys.dont_write_bytecode = True
OUT = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('runner', OUT/'run.py')
r = importlib.util.module_from_spec(spec)
spec.loader.exec_module(r)
ROOT = r.runner.ROOT
read = lambda p: json.loads(p.read_text())
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
index = read(OUT/'evidence.json')
for item in index['artifacts']:
    p = ROOT / item['path']
    assert p.stat().st_size == item['bytes'] and sha(p) == item['sha256'], item['path']
    assert oct(p.stat().st_mode & 0o777) == item['mode']
source = read(OUT/'context.json')['source']
assert r.runner.fingerprint() == source
assert source['sha256'] == index['sourceFingerprint'] and len(source['files']) == 51
for name, argv in r.ALLOWED.items():
    d = read(OUT/(name+'.json'))
    assert d['argv'] == argv and d['exitCode'] == (1 if name == 'harness' else 0)
    assert d['sourceBefore'] == d['sourceAfter'] == source['sha256']
    assert d['productUnchanged'] and d['processExitObserved'] and not d['timedOut']
    assert sha(OUT/(name+'.txt')) == d['outputSha256']
qa = read(OUT.parents[1]/'qa.json')
assert qa['attempt'] == 3 and qa['sourceFingerprint'] == source['sha256']
for k in ['behavioralContractVerified','compatibilityVerified','migrationUnchanged',
    'cargoFmtPassed','cargoClippyPassed','cargoTestsPassed','harnessExecuted',
    'harnessUnchanged','unresolvedHarnessFailuresRetained']:
    assert qa[k] is True, k
assert not qa['harnessPassed'] and qa['harnessExitCode'] == 1 and len(qa['findings']) == 3
assert qa['status'] == 'partial' and not qa['missionCompletionClaimed']
assert not qa['configuredTargetApplicabilityVerified'] and not qa['containerHealthVerified']
assert qa['testCounts'] == {'passed':183,'failed':0,'ignored':0,'measured':0,'filtered':0}
for path in ['crates/arc-storage/migrations/0001_phase0.sql','scripts/harness/validate.sh']:
    baseline = subprocess.check_output(['git','show',qa['baselineHead']+':'+path],cwd=ROOT)
    assert (ROOT/path).read_bytes() == baseline, path
assert (OUT.parents[1]/'harness.txt').read_bytes() == (OUT/'harness.txt').read_bytes()
perf = read(OUT.parents[1]/'performance.json')
assert perf['nativeMeasurementsRecorded'] and perf['srsComparisonComplete']
assert perf['sourceFingerprint'] == source['sha256']
assert sha(Path(perf['srs']['sourcePath'])) == perf['srs']['source_sha256']
raw = OUT/'workspace-raw'
samples = read(raw/'small-message-samples.json')
assert len(samples['samples_ns']) == 10000 and samples['failed'] == 0
assert sorted(samples['samples_ns'])[9499] == perf['small_message']['p95_ns']
obs = read(raw/'native-observations.json')
assert max(x['rss_bytes'] for x in obs['idle']) == perf['idle']['max_rss_bytes']
assert max(x['threads'] for x in obs['idle']+obs['load']) == perf['sampled_native_thread_peak']
for name in ['SIGINT','SIGTERM']:
    assert qa['signalCases'][name]['signalToReapedExitUs'] == read(raw/(name+'.json'))['signal_to_exit_us'] < 5000000
print(json.dumps({'deliveryIntegrity':'verified-current-checkout', 'sourceFingerprint':source['sha256'],
    'sourceFiles':51, 'artifacts':len(index['artifacts']), 'cargoGatesPassed':True,
    'harnessExit':1, 'reviewCompleteness':'partial', 'missionCompletionClaimed':False,
    'remaining':['three harness findings','configured-target applicability/access',
                 'task-9 final refresh','remaining Phase 0/Linux/Windows']}))
