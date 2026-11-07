"""Bind task-5 local evidence; does not write mission state or product source."""
import datetime
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
sys.dont_write_bytecode = True
import record

ROOT, OUT = record.ROOT, record.OUT
BASELINE = '42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0'
SNAPSHOT = '39fa9765dcb70a6c12049d84e59bc0086487b539'
OWNED = ['crates/arc-daemon/tests/' + x for x in ['bounded.rs','worker_runtime.rs','support/runtime.rs','support/native.rs','support/measurements.rs','support/admission_load.rs','support/deadline_recovery.rs']]

def git(*args):
    return subprocess.check_output(['git',*args],cwd=ROOT).decode().strip()
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def save(name,value): (OUT/name).write_text(json.dumps(value,indent=2)+'\n')
def read(name): return json.loads((OUT/name).read_text())
def ref(name):
    p=subprocess.run(['git','rev-parse','--verify',name],cwd=ROOT,capture_output=True,text=True)
    return {'ref':name,'exitCode':p.returncode,'resolvedCommit':p.stdout.strip() if p.returncode==0 else None,'stderr':p.stderr.strip()}

fingerprint=record.fingerprint()
for name,status in [('fmt-final',0),('clippy-final',0),('workspace-final',0),('harness',1)]:
    receipt=read(name+'.json')
    assert receipt['exitCode']==status and not receipt['timedOut']
    assert receipt['sourceBefore']==receipt['sourceAfter']==fingerprint['sha256']
    assert receipt['outputSha256']==sha(OUT/receipt['output'])
    assert receipt['cwd']==str(ROOT)

assert git('rev-parse',BASELINE+'^{tree}')=='c802f71ac4d91ed6b8a602d5c83bf9cf57116b76'
assert git('rev-parse',SNAPSHOT+'^{tree}')=='edd2a960a87599dcba29942c6a4ff557c1ed95fc'
tracked=git('ls-files','--','crates','Cargo.toml','Cargo.lock','rust-toolchain.toml','scripts/harness/validate.sh').splitlines()
changed=[]
for path in tracked:
    original=subprocess.check_output(['git','show',SNAPSHOT+':'+path],cwd=ROOT)
    if (ROOT/path).read_bytes()!=original: changed.append(path)
assert changed==[],changed
for path in OWNED:
    assert (ROOT/path).is_file()
    assert len((ROOT/path).read_text().splitlines())<=500
frozen={}
for path in ['crates/arc-storage/migrations/0001_phase0.sql','scripts/harness/validate.sh']:
    original=subprocess.check_output(['git','show',BASELINE+':'+path],cwd=ROOT)
    assert (ROOT/path).read_bytes()==original
    frozen[path]={'actualSha256':sha(ROOT/path),'acceptedBaselineSha256':hashlib.sha256(original).hexdigest(),'byteIdentical':True}
source={'version':1,'path':str(ROOT),'sourceWorkspacePath':str(ROOT.parents[3]),'branch':git('branch','--show-current'),'missionId':'b137e253-def6-46ef-a75d-25ab414784ea','taskId':'task-5','attempt':1,
    'baselineHead':BASELINE,'baselineTree':git('rev-parse',BASELINE+'^{tree}'),
    'snapshotCommit':SNAPSHOT,'snapshotTree':git('rev-parse',SNAPSHOT+'^{tree}'),
    'snapshotRef':'refs/agent-valley/task-evidence/d6c6b45f4a6c3769-e7c08de777760acd-1/snapshot',
    'workerRef':'refs/agent-valley/task-evidence/d6c6b45f4a6c3769-e7c08de777760acd-1/worker',
    'observedLocalHead':git('rev-parse','HEAD'),'observedLocalBranch':git('branch','--show-current'),
    'observedRefs':[ref('refs/agent-valley/task-evidence/d6c6b45f4a6c3769-e7c08de777760acd-1/'+name) for name in ['snapshot','worker']],
    'scope':'Actor-local snapshot plus seven owned uncommitted test files. Runtime implementation is inherited snapshot input; it is not asserted to exist in accepted committed baseline. Refs absent locally stay unresolved; no replacement revision is inferred.',
    'productFingerprint':fingerprint,'ownedFiles':OWNED,'trackedProductChangesFromSnapshot':changed,
    'frozen':frozen,'injectedClaimPath':None,'omaRunId':None}
save('source-evidence.json',source)
raw=OUT/'workspace-final-raw'
performance=json.loads((raw/'native-measurements.json').read_text())
observations=json.loads((raw/'native-observations.json').read_text())
latency=json.loads((raw/'small-message-samples.json').read_text())
assert len(latency['samples_ns'])==10000
assert sorted(latency['samples_ns'])[9499]==performance['small_message']['p95_ns']
assert len(observations['idle'])==100 and observations['load']
performance['idle']['observed_sample_span_us']=observations['idle'][-1]['elapsed_us']-observations['idle'][0]['elapsed_us']
performance['idle']['window_note']='window_seconds and cadence_ms are nominal; raw monotonic timestamps disclose actual scheduling'
performance['sourceEvidence']='source-evidence.json'
performance['sourceFingerprint']=fingerprint['sha256']
performance['rawArtifacts']=['workspace-final-raw/native-measurements.json','workspace-final-raw/native-observations.json','workspace-final-raw/small-message-samples.json']
srs=Path('/Users/gracefullight/Downloads/Agent_Resource_Control_ARC_Requirements_SRS_2026-11-06.md')
assert sha(srs)==performance['srs']['source_sha256']
performance['srs']['sourcePath']=str(srs)
performance['srs']['sourceVerifiedInThisRun']=True
performance['srs']['requirements']={'NFR-002':'section 18, line 2426: idle RSS below 50 MB','NFR-004':'section 18, line 2428: local small-message IPC round-trip P95 within 20 ms'}
save('performance.json',performance)

accounting=[]
for name,terminal in [('connection-100000','shutdown_cancelled'),('request-100000','released_without_dispatch'),('db-100000','client_cancelled')]:
    data=json.loads((raw/(name+'.json')).read_text())
    assert data['attempted']==data['admitted']+data['rejected']==100000
    assert data['admitted']==data['completed']+data['timed_out']+data[terminal]
    accounting.append({'artifact':'workspace-final-raw/'+name+'.json',**{key:data[key] for key in ['scope','attempted','admitted','rejected','completed','timed_out',terminal]}})
accounting.append({'artifact':'workspace-final-raw/native-measurements.json','scope':'real production daemon, four persistent same-user clients','attempted':100000,'admitted':100000,'completed':100000,'rejected':0,'timed_out':0,'shutdown_cancelled':0})
save('accounting.json',{'note':'Four independent experiments. Do not add them into a claim of 400000 network requests. Admission is an intermediate state; terminal outcomes are disjoint. Rejected attempts were never admitted.','experiments':accounting})

workspace=(OUT/'workspace-final.txt').read_text()
invocations=sum(map(int,re.findall(r'test result: ok\. (\d+) passed;',workspace)))
assert invocations==173
assert not re.search(r'test result: FAILED|[1-9]\d* ignored|[1-9]\d* filtered out',workspace)
harness=re.sub(r'\x1b\[[0-9;]*m','',(OUT/'harness.txt').read_text())
failures=['crates/arc-daemon/tests/resident.rs has 901 lines','crates/arc-ipc/src/lib.rs has 551 lines','Script not found "test:coverage"']
for failure in failures: assert failure in harness
save('acceptance.json',{'status':'task-5 locally complete; mission final review remains open','skillsUsed':['oma-backend'],
    'workspaceTestInvocationsPassed':invocations,'ignored':0,'filteredOut':0,'newIntegrationTargets':['bounded','worker_runtime'],
    'checks':{name:read(name+'.json')['exitCode'] for name in ['fmt-final','clippy-final','workspace-final','harness']},
    'unresolvedHarnessFailures':failures,'harnessUnchanged':True,'migrationUnchanged':True,
    'sourceFingerprint':fingerprint['sha256'],'remaining':['Independent collection/review and final mission evidence promotion','Linux and Windows native runtime verification','Other Phase 0 features and long-run CPU qualification','av.yaml configured-target inspection is still permission-denied'],
    'interfaceCompatibility':'No production source, manifests, registry, lockfile, prior tests, task definitions or existing evidence were modified. Uses production ARC_RUNTIME_REPORT; controlled SQL delay exists only in this test target.'})

sigint=json.loads((raw/'SIGINT.json').read_text())
sigterm=json.loads((raw/'SIGTERM.json').read_text())
deadline=json.loads((raw/'deadline-saturation.json').read_text())
nonreader=json.loads((raw/'nonreader.json').read_text())
sql=json.loads((raw/'native-sql-health.json').read_text())
perf=f'''# Task-5 native measurements

Measured on {performance['host']['uname']}. Hardware: Apple M1 Pro, 8 logical CPUs, 17,179,869,184 bytes RAM. Rust/Cargo 1.95.0, debug test profile, real `target/debug/arc-daemon`, default IPC limits C=64/R=16/Q=8, two configured Tokio worker threads and one SQLite worker. The source is the Actor-local snapshot {SNAPSHOT} plus owned uncommitted tests; exact product bytes are bound by `source-evidence.json`, fingerprint `{fingerprint['sha256']}`. This does not attribute the implementation to the accepted baseline {BASELINE}.

A separate 200-request owned subprocess probe ran first and verified the native sampler, final report, successful exit and integrity. The full measured child then settled for 5 seconds. We collected 100 idle samples at a nominal 100 ms cadence; actual first-to-last sample span was {performance['idle']['observed_sample_span_us']/1e6:.6f} seconds. Monotonic timestamps in `workspace-final-raw/native-observations.json` disclose scheduler jitter. The idle threshold applies to idle RSS, not to load peak.

| Measurement | Observed | SRS or scope |
| --- | ---: | --- |
| Maximum observed idle RSS | {performance['idle']['max_rss_bytes']:,} bytes ({performance['idle']['rss_mb']:.6f} MB; {performance['idle']['rss_mib']:.6f} MiB) | NFR-002 <50,000,000 bytes: pass |
| Daemon native lifetime peak RSS | {performance['lifetime_peak_rss_bytes']:,} bytes | wait4 ru_maxrss; no SRS saturation-peak threshold |
| Sampled OS thread peak | {performance['sampled_native_thread_peak']} | proc_pidinfo; no SRS thread threshold |
| Application retained-handler/task high water | {performance['report']['metrics']['tracked_tasks']['high_water']} | Runtime report, including completed-but-unreaped entries; separate supervisor=1 |
| 10,000 small-message P95 | {performance['small_message']['p95_ns']/1e6:.6f} ms | NFR-004 <=20 ms: pass |
| P50 / max | {performance['small_message']['p50_ns']/1e6:.6f} / {performance['small_message']['max_ns']/1e6:.6f} ms | All samples retained; zero failures |
| After-load 200-message P95 | {performance['after_load_p95_ns']/1e6:.6f} ms | Recovery observation; no during-saturation P95 claim |
| Separate four-client 100,000-request load | {performance['load_elapsed_us']/1e6:.6f} seconds | 100,000 completed, zero rejected/timeouts/cancellation |

The latency sample follows 200 warmups on a persistent same-user Unix connection. It alternates hello/health with ID `small` and null payload. Encoded bodies are 85/86 request bytes and 124 response bytes, with an additional four-byte prefix each way. `std::time::Instant` spans write through full decoded response, including serialization; connection setup is separate ({performance['small_message']['connection_setup_ns']} ns). Nearest-rank P95 is sorted index 9,499. Raw 10,000 nanosecond durations, sizes and P50/max are in `workspace-final-raw/small-message-samples.json`. Failed exchanges would fail the fixture; none are silently removed.

macOS `proc_pidinfo(PROC_PIDTASKINFO)` supplies actual daemon RSS bytes and native thread counts. Linux support uses `/proc/PID/status`, converting VmRSS KiB to bytes, but was not executed here. Owned-child wait4 supplies lifetime ru_maxrss (bytes on macOS, KiB converted on Linux). Idle sampling is nominally 100 ms and load sampling 10 ms, performed in the parent; brief thread spikes can escape sampling. Runtime fixed counters/report remain enabled in the daemon. Sampling overhead and instrumented-versus-uninstrumented cost were not quantified. Parent client/sampler threads are excluded from daemon measurements. Application task gauges do not measure Tokio internal tasks or OS threads; structural byte reservations are not RSS measurements.

The ARC SRS was re-read in place at `{srs}`, SHA-256 `{performance['srs']['source_sha256']}`. Section 18 lines 2426/2428 defines these idle RSS and local small-message round-trip targets. Decimal MB is the disclosed convention; the source does not define MB more precisely. Its front matter says 1.1.0-draft while summary/footer retain 1.0.0-draft; file hash and requirement IDs identify this source. Comparisons are complete and both targets passed for this host/debug run. No accepted-baseline performance delta, release-build result, long-run CPU qualification or saturation-latency claim is made. Linux/Windows measurements and wider Phase 0 qualification remain open.

Reproduce with `ARC_TEST_EVIDENCE_DIR=<existing-output-directory> cargo test --workspace` so the CLI prerequisite is compiled. `bounded::native_probe_then_full_100000_attempt_workload` is active, not ignored. Focused tests can run afterward with `cargo test -p arc-daemon --test bounded --test worker_runtime -- --nocapture`. `record.py` provides serial 120-second command deadlines, output hashes and source-before/after receipts without building separately. This task leaves final shared performance/QA artifacts to their assigned writers; `performance.json` here is the promotable measurement input.
'''
(OUT/'performance.md').write_text(perf)

report=f'''# Runtime Test Engineer delivery

Status: task-5 implementation and local verification complete; ready for Chief Director collection and independent review. Mission completion is not claimed. Skill used: oma-backend, with backend-engineer role, execution policy, result contract, backend checklist/rule, i18n rule, task-1 contract/targets and task-4 executable integration. The existing Rust/Tokio/rusqlite stack was detected from Cargo manifests. No new dependency or external service was introduced.

## Changed paths and ownership

Seven new owned source files:

{chr(10).join('- `'+p+'`' for p in OWNED)}

Evidence and reproducible collectors are under `.arc-verify/bounded-ipc/implementation/`. Every inherited tracked product file matches snapshot {SNAPSHOT}; production modules, manifests/lockfiles, existing tests/support module, CLI sources, migration, harness and prior evidence remain unchanged. There is no ownership conflict or interface change. Test targets import the real production modules with `#[path]`; crate-wide dead-code allowance is confined to the imported-module test target. Existing tests remain active. The full suite has 173 passing test invocations, including duplicate production unit tests compiled into the controlled target; this is not a claim of 173 unique scenarios.

## Acceptance evidence

| Assigned criterion | Evidence |
| --- | --- |
| Deterministic unit, subprocess and CLI coverage | `workspace-final.txt`: 3 bounded tests and 48 worker-runtime tests, plus every preexisting suite, all passing with zero ignored/filtered tests. `accounting.json` and raw receipts reconcile four separately scoped 100,000-attempt experiments. |
| Every admission cap and bounded rejection | Unit experiments call production connection/request/DB admission 100,000 times each, asserting every independently atomic gauge/current/high-water after every attempt. Native socketpairs authenticate actual same-user peers; eight retained tasks admit 8 and reject 99,992 with rejection_work=1 and no rejection factory/task. Requests admit 4/reject 99,996. DB admits 9/rejects 99,991 with Q=8 plus one executing; canceled queued work stays counted until drain. Terminal categories are disjoint. |
| Real 100,000 request workload | Four persistent clients each send 25,000 real hello/health exchanges with matching response IDs. 100,000 admitted/completed, zero rejected/timed-out/shutdown-cancelled. Separate warmup, latency and CLI exchanges are disclosed; final frames_completed=110,402. Atomic high waters verify configured default caps throughout. This workload is not conflated with direct admission rejection tests. |
| Stalls, trickling, malformed input and real backpressure | Existing active paused-time transport tests prove absolute header/body and shared frame deadlines, trickle resistance, completion retention and response-buffer release. New real-daemon tests observe stalled-header/body closure and preserve JSON/version/length errors. A 512 KiB echoed ID plus reduced peer receive buffer produces real unread socket backpressure; write_timeouts=1, observed HUP in {nonreader['closure_us']/1000:.3f} ms, then both actual CLI commands succeed. |
| Saturation and same-process recovery | The controlled supervisor reaches all R=4 requests/C=8 connections; real CLI status/doctor return exactly DAEMON_UNAVAILABLE under predecode rejection. All 4 partial bodies and 4 headers expire; both CLI commands recover in the same process in {deadline['recovery_us']/1000:.3f} ms (300 ms frame budget + disclosed 250 ms observation margin). |
| DB overload and health independence | Controlled Q=2 plus executing=1 returns exact IPC_ERROR for real CLI doctor; status stays available. A separate test gates SQLite's native preparation callback for fixed PRAGMA integrity_check, confirms status in {sql['health_cli_us']/1000:.3f} ms before release, then executes real diagnostics and verifies doctor/status recovery. This controlled SQL-preparation delay is not a SQL compute benchmark. |
| Both signals within five seconds | SIGINT {sigint['signal_to_exit_us']} us; SIGTERM {sigterm['signal_to_exit_us']} us from signal through wait4-reaped successful exit, with 3 database requests, a partial body and 4 stalled headers active. Reports prove zero gauges/remaining handlers, equal spawned/reaped, worker started=joined=1, SQLite closed and socket removed. Actual new daemon admission on the same socket/database and successful CLI checks follow each case; database reopen/integrity succeeds. |
| Native measurements and SRS | `performance.json`, `performance.md` and raw timestamp/sample artifacts. Both NFR-002 idle RSS and NFR-004 P95 comparisons passed on this macOS/debug host; structural counters are separate. |
| Compatibility and preservation | Unchanged resident/startup/CLI/core/storage/protocol suites pass. They retain TMPDIR-independent identity, single-daemon admission, same-user policy, filesystem/link protections, protocol 1, CLI output and migration checks. Frozen migration hash is `{frozen['crates/arc-storage/migrations/0001_phase0.sql']['actualSha256']}` and bytes match accepted baseline. |

## Commands and outcomes

- `cargo fmt --all -- --check`: exit 0 (`fmt-final.json/txt`).
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0 (`clippy-final.json/txt`).
- `cargo test --workspace`: exit 0 in {read('workspace-final.json')['elapsedSeconds']:.3f} seconds; 173 passing invocations (`workspace-final.json/txt`).
- `./scripts/harness/validate.sh`: exit 1, unchanged script; three retained failures: resident.rs 901 lines, arc-ipc/lib.rs 551 lines, missing test:coverage. No skip flag, dummy coverage, JavaScript tooling or changed acceptance was added.
- Focused `cargo test -p arc-daemon --test bounded --test worker_runtime -- --nocapture` passed before the last two scenario additions. Their focused sql-health/deadline commands passed; the final exact workspace suite includes all final scenarios. Older focused receipts are labeled by their actual source fingerprints, not reused as final-source evidence.
- `verify.py` checks current collected source/artifact hashes and final receipt output hashes. It reports the review checkout's actual HEAD separately rather than requiring the Actor's snapshot HEAD.

The three final Cargo receipts and harness receipt bind identical product fingerprint `{fingerprint['sha256']}`. Receipts retain command argv, exact cwd, toolchain, offline environment, owned outer PID/process group, timeout, observed process exit and log hashes. Test children are owned by RAII guards and stopped/reaped on assertion failure; normal signal fixtures use wait4 and retain successful exit/accounting evidence.

## Provenance and failed attempts

Accepted committed baseline is {BASELINE}, tree c802f71ac4d91ed6b8a602d5c83bf9cf57116b76. Actor input snapshot is {SNAPSHOT}, tree edd2a960a87599dcba29942c6a4ff557c1ed95fc, observed local HEAD {source['observedLocalHead']}. The supplied snapshot/worker ref names do not resolve in this isolated checkout; `source-evidence.json` preserves their names and actual failed resolutions. No missing revision was replaced with another checkout's HEAD. The seven task-5 tests remain uncommitted; a review checkout must verify collected bytes with this receipt. Snapshot tracking does not establish tracking in the accepted baseline or review checkout.

`attempts.md`, focused-first/workspace-first/workspace-check logs and receipts retain the initial missing-CLI prerequisite, wrong collector root permission failure and macOS disconnected-socket timeout-option failure. Only the owned fixture/collector were repaired. Optional ps inventory and inherited av.yaml access were denied; no GUI/toolchain/security restriction workaround was attempted. All final required Cargo checks passed. Historical integration evidence was read, not rewritten.

## Remaining risks and handoff

The unchanged harness remains failing with all three concrete conformance failures retained. Its all-tracked mode does not inspect new untracked tests in this Actor checkout; all seven new files are separately recorded at <=500 lines. Independent QA and shared final evidence promotion belong to subsequent assigned writers. Do not mark mission-wide QA complete from this Actor report.

Measurements apply to this debug macOS host. Thread peaks are sampled, instrumentation overhead is unquantified, no baseline performance delta is established, and no latency during saturation is claimed. Linux and Windows runtime/signal/IPC/native measurements remain unexecuted. Remaining Phase 0 includes Windows Named Pipe IPC, two-OS tray/desktop integration, exec/Managed Job/hook/locality work, vendor detection/classification and later Meter/Traffic/Sweeper features; long-run idle CPU and broader crash-recovery qualification remain open. Configured deployed-container targets could not be inspected because av.yaml is permission-denied; no deployment health or external-effect proof is invented.
'''
(OUT/'delivery.md').write_text(report)
result=ROOT/'.agents/results/result-test-engineer-task-5-uninjected-b137e253-def6-46ef-a75d-25ab414784ea.md'
result.parent.mkdir(parents=True,exist_ok=True)
result.write_text(report+'\nNo OMA runId/claimPath was injected. The filename explicitly marks that missing transport identity; no new mission run or state was created.\n')
claim={'status':'completed','scope':'assigned task-5 local delivery; mission completion not claimed','changedFiles':OWNED,
    'unresolved':['Unchanged harness retains 3 conformance failures','Independent review/integration and final evidence promotion pending','Linux/Windows and remaining Phase 0 qualification not completed'],
    'artifacts':['.arc-verify/bounded-ipc/implementation/delivery.md','.arc-verify/bounded-ipc/implementation/performance.md','.arc-verify/bounded-ipc/implementation/performance.json','.arc-verify/bounded-ipc/implementation/source-evidence.json','.arc-verify/bounded-ipc/implementation/acceptance.json'],
    'injectedClaimPath':None,'omaRunId':None}
save('claim.json',claim)
artifacts=[]
for path in sorted(OUT.rglob('*')):
    if path.is_file() and path.name not in ['evidence.json','verification.json'] and '__pycache__' not in path.parts:
        artifacts.append({'path':str(path.relative_to(ROOT)),'sha256':sha(path),'bytes':path.stat().st_size})
save('evidence.json',{'sourceFingerprint':fingerprint['sha256'],'sourceEvidence':'source-evidence.json','artifacts':artifacts})
print(json.dumps({'status':'ready-for-independent-review','sourceFiles':len(fingerprint['files']),'artifacts':len(artifacts),'sourceFingerprint':fingerprint['sha256'],'workspaceInvocations':invocations,'harnessExit':1}))
