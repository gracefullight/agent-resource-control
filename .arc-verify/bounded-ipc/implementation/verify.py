"""Read-only verification after collection; never regenerate command receipts."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
sys.dont_write_bytecode=True
import record
ROOT,OUT=record.ROOT,record.OUT

def read(path): return json.loads(path.read_text())
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()

index=read(OUT/'evidence.json')
for item in index['artifacts']:
    path=ROOT/item['path']
    assert path.is_file(),item['path']
    assert path.stat().st_size==item['bytes'],item['path']
    assert sha(path)==item['sha256'],item['path']
source=read(OUT/'source-evidence.json')
assert record.fingerprint()==source['productFingerprint']
assert index['sourceFingerprint']==source['productFingerprint']['sha256']
for name,exit_code in [('fmt-final',0),('clippy-final',0),('workspace-final',0),('harness',1)]:
    receipt=read(OUT/(name+'.json'))
    assert receipt['exitCode']==exit_code and not receipt['timedOut']
    assert receipt['sourceBefore']==receipt['sourceAfter']==index['sourceFingerprint']
    assert sha(OUT/receipt['output'])==receipt['outputSha256']
for path,value in source['frozen'].items(): assert sha(ROOT/path)==value['actualSha256']==value['acceptedBaselineSha256']
performance=read(OUT/'performance.json')
assert performance['nativeMeasurementsRecorded'] and performance['srsComparisonComplete']
latency=read(OUT/'workspace-final-raw/small-message-samples.json')
assert len(latency['samples_ns'])==10000 and latency['failed']==0
assert sorted(latency['samples_ns'])[9499]==performance['small_message']['p95_ns']
assert performance['idle']['max_rss_bytes']<50_000_000
assert performance['small_message']['p95_ns']<=20_000_000
caps={'connections':'connections','requests':'requests','tracked_tasks':'connections','db_queued':'db_queue'}
for path in (OUT/'workspace-final-raw').glob('*.json'):
    data=read(path)
    report=data.get('report')
    if report is None: continue
    for key in ['accounted','handlers_reaped','worker_joined','connection_closed','gauges_zero','socket_removed','deadline_met']:
        assert report[key] is True,(path,key)
    assert report['handlers_remaining']==0
    m=report['metrics']
    for key in list(caps)+['request_body_bytes','response_bytes','db_executing','db_outstanding','rejection_work']:
        assert m[key]['current']==0,(path,key)
        bound=report['limits'][caps[key]] if key in caps else (
            report['limits']['requests']*1048576 if key.endswith('_bytes') else (
                report['limits']['db_queue']+1 if key=='db_outstanding' else 1))
        assert m[key]['high_water']<=bound,(path,key)
    assert m['handlers_spawned']==m['handlers_reaped']
    assert m['worker_started']==m['worker_joined']==1
    if path.stem in ['SIGINT','SIGTERM']:
        assert data['signal_to_exit_us']<=5_000_000
        assert data['reopen_integrity']=='ok'
        assert data['restart_report']['accounted'] is True
result={'status':'matches-current-checkout','observedReviewHead':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT).decode().strip(),
    'recordedActorHead':source['observedLocalHead'],'acceptedBaselineHead':source['baselineHead'],
    'sourceFiles':len(source['productFingerprint']['files']),'artifacts':len(index['artifacts']),
    'sourceFingerprint':index['sourceFingerprint'],'harnessExit':1,'unresolvedHarnessFailuresRetained':True}
print(json.dumps(result))
