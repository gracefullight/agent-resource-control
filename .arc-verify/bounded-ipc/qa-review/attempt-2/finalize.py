"""Validate fresh raw receipts, then write only task-8 deliverables."""
import datetime
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import sys

sys.dont_write_bytecode = True
OUT = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('runner', OUT.parent / 'run.py')
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)
ROOT = runner.ROOT
DEST = OUT.parents[1]
RAW = OUT / 'workspace-raw'
def read(p): return json.loads(p.read_text())
def save(p, value): p.write_text(json.dumps(value, indent=2, ensure_ascii=False)+'\n')
def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()

source = read(OUT / 'context.json')['source']
assert runner.fingerprint() == source
commands = {}
for name, expected, argv in [
    ('fmt', 0, ['cargo','fmt','--all','--','--check']),
    ('clippy', 0, ['cargo','clippy','--workspace','--all-targets','--','-D','warnings']),
    ('workspace', 0, ['cargo','test','--workspace']),
    ('harness', 1, ['./scripts/harness/validate.sh']),
]:
    r = read(OUT / (name+'.json'))
    assert r['argv'] == argv and r['exitCode'] == expected and not r['timedOut'], r
    assert r['processExitObserved'] and r['productUnchanged']
    assert r['sourceBefore'] == r['sourceAfter'] == source['sha256']
    assert r['outputSha256'] == sha(OUT / (name+'.txt'))
    commands[name] = r

text = (OUT/'workspace.txt').read_text()
rows = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out', text)
counts = dict(zip(['passed','failed','ignored','measured','filtered'], map(sum,zip(*(map(int,r) for r in rows)))))
assert counts == {'passed':173,'failed':0,'ignored':0,'measured':0,'filtered':0}, counts
audit = read(OUT/'dependency-test-audit.json')
assert len(audit['indexedArtifacts']) == 107 and all(i['matches'] for i in audit['indexedArtifacts'])
assert all(audit['preservedByteIdentical'].values()) and all(audit['serverTestsRetained'].values())
assert not any(i['ignoredAttributes'] for i in audit['sourceTestAudit'].values())
preserve = read(OUT/'preservation-targets.json')
assert all(i['byteIdentical'] for i in preserve['frozen'].values())
assert preserve['srs']['sha256'] == 'a8ef945251f9f3fd7c3e337d4efc77ff9914f95114212ca9e6c90ae2ce3e3d9e'
assert {r['line'] for r in preserve['srs']['lines']} == {2426,2428}

validated = []
def check_metrics(m, caps):
    bounds = {'connections':caps['connections'],'requests':caps['requests'],
        'tracked_tasks':caps['connections'],'db_queued':caps['db_queue'],
        'db_executing':1,'db_outstanding':caps['db_queue']+1,'rejection_work':1,
        'request_body_bytes':caps['requests']*1048576,'response_bytes':caps['requests']*1048576}
    for key, cap in bounds.items():
        assert m[key]['current'] == 0 and 0 <= m[key]['high_water'] <= cap, (key,m[key],cap)
    assert m['handlers_spawned'] == m['handlers_reaped']
    assert m['worker_started'] == m['worker_joined']
def check_report(r):
    for k in ['accounted','handlers_reaped','worker_joined','connection_closed','gauges_zero','socket_removed','deadline_met']:
        assert r[k] is True, (k,r)
    assert r['handlers_remaining'] == 0
    check_metrics(r['metrics'],r['limits'])
    assert r['metrics']['worker_started'] == r['metrics']['worker_joined'] == 1
    assert r['supervisor']['retry_tasks'] == 0
    assert r['supervisor']['supervisors'] == {'current':1,'high_water':1}
for p in sorted(RAW.glob('*.json')):
    d = read(p)
    for key in ['report','restart_report']:
        if key in d: check_report(d[key]); validated.append(str(p.relative_to(ROOT))+':'+key)
attempts = []
for name, admitted in [('connection',8),('request',4),('db',9)]:
    d = read(RAW/(name+'-100000.json'))
    assert d['attempted'] == 100000 and d['admitted'] == admitted
    assert d['rejected']+admitted == 100000
    assert sum(d.get(k,0) for k in ['completed','timed_out','shutdown_cancelled','released_without_dispatch','client_cancelled']) == admitted
    check_metrics(d['final'],d['final']['caps'])
    assert d['final']['worker_started'] == (1 if name == 'db' else 0)
    attempts.append({k:v for k,v in d.items() if k!='final'})
signals = {}
for name in ['SIGINT','SIGTERM']:
    d = read(RAW/(name+'.json')); assert d['signal_to_exit_us'] < 5000000
    assert d['reopen_integrity'] == 'ok' and d['report']['reason'] == name
    m=d['report']['metrics']
    assert [m[k]['high_water'] for k in ['connections','requests','tracked_tasks','db_queued','db_executing','db_outstanding']] == [8,4,8,2,1,3]
    signals[name]={'signalToReapedExitUs':d['signal_to_exit_us'],
        'handlersSpawnedReaped':m['handlers_spawned'],'workerStartedJoined':1,
        'allGaugesZero':True,'reopenIntegrity':'ok','restartVerified':True}
frames=read(RAW/'frames.json')['report']['metrics']
assert frames['header_timeouts'] == frames['frame_timeouts'] == 1
nonreader=read(RAW/'nonreader.json'); assert nonreader['report']['metrics']['write_timeouts']==1
assert nonreader['closure_us']<800000
recovery=read(RAW/'deadline-saturation.json'); assert recovery['recovered_same_process'] and recovery['recovery_us'] <= 550000
for k in ['header_timeouts','frame_timeouts']: assert recovery['report']['metrics'][k] == 4
for k in ['request_rejected','connection_rejected']: assert recovery['report']['metrics'][k] == 1
health=read(RAW/'native-sql-health.json'); assert health['health_cli_us'] < 500000

perf=read(RAW/'native-measurements.json')
assert perf['attempts']=={'attempted':100000,'admitted':100000,'completed':100000,'rejected':0,'timed_out':0,'shutdown_cancelled':0}
obs=read(RAW/'native-observations.json'); samples=read(RAW/'small-message-samples.json')
assert len(obs['idle'])==100 and obs['load']
assert max(r['rss_bytes'] for r in obs['idle'])==perf['idle']['max_rss_bytes']
assert len(samples['samples_ns'])==10000 and samples['failed']==0
assert sorted(samples['samples_ns'])[9499]==perf['small_message']['p95_ns']
assert max(r['threads'] for r in obs['idle']+obs['load'])==perf['sampled_native_thread_peak']
assert perf['lifetime_peak_rss_bytes']>0
assert perf['idle']['max_rss_bytes']<50000000 and perf['small_message']['p95_ns']<=20000000
probe=read(RAW/'native-probe.json'); assert probe['attempts']==probe['completed']==200
perf.update({'scope':'Fresh independent task-8 attempt-2 execution of integrated worktree, not inherited benchmark results',
    'sourceFingerprint':source['sha256'],'sourceFileCount':len(source['files']),
    'measuredAt':commands['workspace']['startedAt'],'nativeMeasurementsRecorded':True,'srsComparisonComplete':True,
    'rawArtifacts':[str(p.relative_to(ROOT)) for p in sorted(RAW.glob('*.json'))]})
perf['small_message']['sizes']=samples['sizes']
perf['srs'].update({'sourcePath':preserve['srs']['path'],'sourceVerifiedInIndependentReview':True,
    'comparisonScope':'NFR-002 idle RSS and NFR-004 small-message P95 on this macOS debug host; no SRS peak-load RSS, thread or task threshold supplied'})
perf['idle']['observed_sample_span_us']=obs['idle'][-1]['elapsed_us']-obs['idle'][0]['elapsed_us']
save(DEST/'performance.json',perf)

harness=(OUT/'harness.txt').read_text()
assert 'test:coverage' in harness and '901 lines' in harness and '551 lines' in harness
(DEST/'harness.txt').write_bytes((OUT/'harness.txt').read_bytes())
old=read(OUT/'preceding-qa.json')
qa={k:old[k] for k in ['schemaVersion','actor','task','baselineHead','findings','interfaceCompatibility','codeIntelligence']}
qa.update({'attempt':2,'recordedAt':datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'skill':'oma-qa','status':'partial','reviewResult':'NOT_ASSESSED','verificationCompleteness':'partial',
    'localRuntimeVerification':'completed','scope':'Fresh independent local runtime/compatibility verification; three harness findings and configured-target access gap retained',
    'behavioralContractVerified':True,'compatibilityVerified':True,'migrationUnchanged':True,
    'cargoFmtPassed':True,'cargoClippyPassed':True,'cargoTestsPassed':True,'harnessExecuted':True,
    'harnessUnchanged':True,'unresolvedHarnessFailuresRetained':True,'harnessPassed':False,'harnessExitCode':1,
    'sourceFingerprint':source['sha256'],'sourceFileCount':len(source['files']),'testCounts':counts,
    'attemptExperiments':attempts,'nativeNetworkWorkload':perf['attempts'],'signalCases':signals,
    'commandReceipts':{k:str((OUT/(k+'.json')).relative_to(ROOT)) for k in commands},
    'containerHealthVerified':False,'configuredTargetApplicabilityVerified':False,'missionCompletionClaimed':False,
    'verificationGaps':[{'path':'av.yaml','reason':preserve['containerConfiguration'].get('error'),
        'effect':'Cannot establish configured deployed targets; actual health, availability and recent-error evidence is unmet if targets exist.',
        'owner':'Chief Director: arrange supported configuration access; separately assign any authorized external observation/recovery.'},
        {'path':'platform qualification','reason':'This execution covers macOS only; Linux and Windows runtime remain unexecuted.',
        'owner':'Chief Director / platform implementers','effect':'No platform-wide or full Phase 0 completion claim.'},
        {'path':'docs/bounded-ipc-final.md','line':11,
        'reason':'Preceding task-9 report cites attempt-1 fingerprint f9c6c5baeb3e95c48603717f08cf2abb88baf85f334250db739eb903c44b4999 and earlier QA receipts; current source is b3610549adfebffdd626a760741a200c848ec7ee26a88482bcf7e38e563383c4.',
        'owner':'Chief Director / task-9',
        'effect':'Refresh current-source fingerprint, current receipt links and measurements in the final assessment. Earlier execution remains historical evidence; QA has no ownership to edit this report.'}],
    'remainingObligations':['Three unchanged-harness failures','Configured deployed-target applicability/access and observations','Linux and Windows runtime qualification','Remaining Phase 0 and task-9 final assessment'],
    'ownership':'Only assigned QA/performance/harness deliverables and attempt-2 review receipts written; product/test/manifest bytes, framework reviews and prior receipts preserved.',
    'failedAttempts':[{'operation':'Known-path cat of db_worker/query.rs and db_worker/queue.rs','result':'No such files; continued with actual known execution.rs path, no mutation'},
        {'operation':'Read av.yaml','result':preserve['containerConfiguration'].get('error')},
        {'operation':'mise tracking link at shell entry','result':'Operation not permitted; recorded in shell output, Cargo checks still passed'}]})
save(DEST/'qa.json',qa)
save(OUT/'runtime-validation.json',{'sourceFingerprint':source['sha256'],'reportsValidated':validated,
    'attemptExperiments':attempts,'signalCases':signals,'testCounts':counts,
    'recomputedP95Ns':sorted(samples['samples_ns'])[9499],
    'recomputedIdleRssBytes':max(r['rss_bytes']for r in obs['idle']),
    'recomputedNativeThreadPeak':max(r['threads']for r in obs['idle']+obs['load']),
    'assertions':'All metrics current zero, high waters within configured caps; accounting reconciles; both signals strictly under five seconds; restart/integrity reports validated',
    'dependencyArtifactsVerified':107,'precedingEvidence':'Preserved at original paths; prior top-level deliverables copied to preceding-* before replacement'})

cmdrows='\n'.join('| `'+ ' '.join(v['argv'])+'` | '+str(v['exitCode'])+' | '+f"{v['elapsedSeconds']:.3f} s"+' | attempt-2/'+k+'.json / .txt |' for k,v in commands.items())
qa_md=f'''# Independent bounded IPC review

## Review Result: NOT_ASSESSED

Status: partial. The local bounded-runtime and compatibility checks passed. Three MEDIUM harness conformance findings remain; deployed-target applicability cannot be established because the supported read of av.yaml is denied. No CRITICAL or HIGH defect was confirmed. This is not a clean-harness or mission-completion verdict. Actor reviewer; task-8; attempt 2; skill oma-qa, with execution policy, result contract, QA execution protocol and checklist.

Existing changes were inspected first. HEAD remains accepted `42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0`; implementation is uncommitted. All 50 source/test/manifest/toolchain files fingerprint to `{source['sha256']}` using sorted path, SHA-256 and mode records. Exact commands bind the same before/after bytes. No product, test, manifest, verification contract, saved task or mission-state edit was made. No new agents, installs, downloads, standalone builds or browser checks were used. Serena tools are absent from the available inventory; review used known-path reads and scoped Git diffs. No claim path or OMA run ID was injected; the assigned qa.md/qa.json are the material handoff. No new mission run was created.

## Findings and remediation ownership

### CRITICAL

None confirmed.

### HIGH

None confirmed.

### MEDIUM

- `crates/arc-daemon/tests/resident.rs:501`: retained 901-line suite exceeds the unchanged harness 500-line cap. Chief Director / Runtime Test Engineer own a separately coordinated split. Move existing test groups into focused integration modules, preserving support::serial, fixture lifetime and every assertion; do not raise the cap or remove tests.
- `crates/arc-ipc/src/lib.rs:501`: 551-line module exceeds that cap (accepted HEAD: 549). Chief Director / IPC Engineer own a behavior-preserving split. Concrete structure: retain the public API in lib.rs and use `#[cfg(test)] mod tests;`, placing the existing test body in src/tests.rs. QA made no such change.
- `scripts/harness/validate.sh:420`: unchanged harness invokes `bun run test:coverage`, which exits with missing script. Chief Director / repository harness owner must resolve the Rust-workspace versus inherited coverage contract outside task-8. Retain this failure; do not add dummy scripts, skip flags or tooling to manufacture a pass. There is no authorized task-8 code patch for that contract mismatch.

### LOW

None confirmed.

## Executed checks

All required commands ran serially from this mission worktree, with CARGO_NET_OFFLINE=true and the prepared cache. Each receipt records exact argv/cwd, timestamps, PID/process group, observed exit, timeout status, raw-output hash and source fingerprints. Commands were bounded to 120 seconds, with owned-process shutdown on timeout. None timed out. Saved mission limits were retained: 86,400 seconds, 200 runs, three retries, at most three concurrent workspace tasks; this review used one command at a time. The original absolute mission deadline was not injected, so no new mission allowance was inferred. Host limits are recorded in context.json.

| Command | Exit | Elapsed | Raw receipt |
| --- | --- | --- | --- |
{cmdrows}

`cargo test --workspace` ran 173 passing invocations; zero failed, ignored, measured or filtered. Count includes duplicated production unit modules and child-entry tests that return in ordinary parent execution; it is not 173 unique scenarios. Parent scenarios actually launch the controlled subprocess entries. The native probe and full workload executed as part of the exact required workspace command. No additional Cargo replay was needed.

The harness ran unchanged in all-tracked mode over 1,337 entries, including the restored task-5 report. Its actual exit is 1 with the three findings above. New untracked Rust modules are outside its tracked inventory; they are included in the independent 50-file fingerprint, lint/tests and dependency-test-audit.json. No coverage skip was supplied. Its successful secret/layer/shell checks are limited to its inventory and do not constitute a comprehensive dependency-vulnerability audit.

## Runtime and race audit

`admission.rs:36` reaps ready JoinSet entries before nonwaiting admission, authenticates the native peer, checks retained length and acquires a permit before invoking the factory. One supervisor owns that set; completed entries remain counted until reaped. Rejection drops the socket inline with high water one; it creates no waiting task. RequestGuard fields drop the accounting gauge before releasing the permit, preventing replacement from counting both owners. Cancellation-before-poll, retained completion, unwind and concurrent producer tests passed.

`connection.rs:36` starts one frame clock before the header. Request admission precedes body allocation and invalid-prefix response work. `transport.rs:24` validates the big-endian declared size before allocating at most 1 MiB; header/body progress never renews deadlines. Readiness and partial-write loops check the original absolute deadline. The response writer bounds length and retained reservation, including escaping expansion, before emitting any prefix. One small overflow fallback shares the original write deadline. Parsing is synchronous over the admitted finite frame; byte gauges describe body/response reservations, not total heap or native RSS. Payload/body are dropped before database awaits.

Defaults: C=64 connections and retained handlers, R=16 requests, Q=8 queued DB jobs plus one executing. Header/frame/write deadlines are 1,000/2,000/500 ms; DB response/execution/busy budgets 1,500/1,000/100 ms; shutdown is 3,500 ms. Test profile: C=8/R=4/Q=2; transport 150/300/100 ms; DB 500/300/50 ms. Invalid limits/profiles are actively rejected. Documentation agrees with implementation.

`db_worker.rs:132` creates one tracked native worker and finite mpsc queue. Extra queue permits stay owned through dequeue gauge release; canceled queued tickets remain counted until drained. The worker alone owns SQLite and runs two fixed read-only diagnostics. No request SQL enters it. Stopping/cancellation/deadline checks surround statements; sticky progress and bounded busy callbacks cover early-interrupt/query-start races. Tests exercise active SQLite VM cancellation, lock waits, enqueue/stop-before-park, concurrent canceled producers, execution/response deadlines, full queue discard and native join retention after a timeout.

`lifecycle.rs:107` prioritizes signals, reaps continuously and uses inline capped accept-error backoff, with no retry tasks. Shutdown drops admission/listener, starts DB cancellation, aborts/reaps handlers, joins the real worker and removes only the owned socket inode under one absolute deadline. The instance lock survives handler/worker join; an active subprocess asserts this before reacquisition. Fatal accept, startup and report failures retain cleanup. Worker Drop joins rather than detaching. Native filesystem/kernel stalls remain a limitation; the executed signal tests establish the tested conditions, not all possible OS stalls.

`runtime_report.rs:37` uses private-directory validation and create-new/no-follow descriptor opening, validates UID/0600/type/link count, retains the descriptor and caps final encoding at 4,096 bytes. Active tests cover unsafe/replaced parents, replacement after open, report size, real write errors and cleanup. No request-derived labels/log history accumulate.

## Behavioral evidence and acceptance checklist

Fresh raw outputs are under qa-review/attempt-2/workspace-raw. The independent runtime-validation.json recomputes counters, every high water, terminal zero gauges and sample percentiles; it does not trust summary booleans.

| Check | Actual evidence | Result |
| --- | --- | --- |
| 100,000 connection attempts | 8 admitted + 99,992 rejected; 8 canceled/reaped; retained tasks high water 8, rejection work 1 | Verified |
| 100,000 request admissions | 4 admitted + 99,996 rejected; four released; recovered admission succeeds | Verified |
| 100,000 DB attempts | 9 admitted + 99,991 Busy; canceled/drained; Q=8 plus executing=1; actual worker joins/reopen integrity ok | Verified |
| Native workload | Four persistent clients × 25,000 = 100,000 completed; zero rejections/timeouts; separately counted probe/warmup/latency/CLI traffic | Verified |
| Absolute deadlines | Paused-time trickled header/body and actual write backpressure; native stalled headers/bodies, malformed/version recovery | Verified |
| Full saturation recovery | Test caps attained, status/doctor rejected then recover in same process after {recovery['recovery_us']} µs; 300 ms deadline plus 250 ms disclosed native margin | Verified |
| Non-reading peer | No draining before HUP; write timeout counter 1; closure observed after {nonreader['closure_us']} µs against 100 ms write budget plus native observation margin | Verified |
| Slow DB / health | Controlled real SQLite preparation blocked while actual status CLI completes in {health['health_cli_us']} µs; normal SQL follows release | Verified |
| SIGINT | {signals['SIGINT']['signalToReapedExitUs']} µs signal-to-reaped exit; 11/11 handlers, 1/1 worker; zero gauges, socket removed, reopen integrity ok, restart works | Verified |
| SIGTERM | {signals['SIGTERM']['signalToReapedExitUs']} µs; same accounting, integrity and restart checks | Verified |
| Protocol / CLI / authentication / identity | Existing CLI/core/storage/instance files byte-identical; active protocol-1/error/version/permissions/symlink/TMPDIR/single-instance cases pass | Verified locally |
| Frozen migration | Actual bytes equal accepted HEAD; SHA-256 {preserve['frozen']['crates/arc-storage/migrations/0001_phase0.sql']['sha256']} | Verified |
| Native performance / SRS | Fresh measurements independently recomputed and compared in performance.json/md | Verified on this host |
| Harness | Actual unchanged command exit 1, all three failures retained | Executed; conformance unresolved |
| Configured deployed targets | av.yaml read denied; no actual target health, availability or recent-error observations | Unmet applicability/access |

These four 100,000-attempt experiments are distinct, not 400,000 network exchanges. Direct admission experiments assert bounds after each attempt; native high-water atomics retain transient maxima. Controlled race tests supplement the single-owner admission loops. Structural bounds are not performance measurements.

All ten original server test names remain in the relocated module, with protocol and filesystem assertions inspected. Resident.rs has only the reviewed process-lock handoff correction: it releases the file lock while retaining its thread mutex through child exit, preserving the serialization assertion. Existing CLI binary, core, storage, NFR-010 and instance files are byte-identical. The 107 indexed task-5 artifacts hash-match. Framework reviews and their historical receipts remain untouched; older boundary verifiers are historical and were not rewritten to accept these new QA outputs.

## Remaining work and limits

Chief Director must retain the three harness failures and coordinate any repairs with the owners above. Supported av.yaml access is required to establish whether deployed container targets exist; logs or local code checks cannot prove their health, availability or recent-error status. No external effect, restart or rollback occurred.

macOS results do not qualify Linux or Windows. Windows Named Pipe IPC/runtime/security, Windows tray and native measurements remain unexecuted. Remaining Phase 0 includes exec/Managed Job, desktop/menu-bar/tray integration on required OSes, hook/locality fixtures and two-OS integration. Vendor detection/classification and Meter/Traffic/Sweeper remain unfinished; existing CLI truthfully reports those limits. NFR-001 long-run idle CPU, signing/packaging and full crash-mode qualification are not completed by this review. Task-9 final mission assessment belongs to the Chief.

Failed read attempts: av.yaml returned Operation not permitted; two guessed db_worker subpaths did not exist and review continued with execution.rs. Shell entry emitted a denied mise tracking-link warning; all Cargo checks nevertheless passed. No restriction was bypassed. Previous top-level task-8 outputs were preserved as preceding-* in the attempt-2 receipt directory before replacement. Changed paths are only the five assigned top-level QA/performance/harness files and associated qa-review/attempt-2 receipts; interfaces are unchanged.

Current handoff to Chief / task-9: `docs/bounded-ipc-final.md:11` still cites the earlier f9c6c5ba source fingerprint and attempt-1 receipt set. It needs the current b3610549 fingerprint, attempt-2 receipt links, 5.619712 MB idle RSS, 6,651,904-byte native lifetime peak, 90,750 ns P95 and current signal timings. This is stale downstream evidence, not a newly demonstrated runtime defect. That file and preceding framework task reviews were preserved; QA did not claim their current-tree reapproval. The new reports supersede the top-level summaries while archived preceding-* copies retain their exact earlier bytes.
'''
(DEST/'qa.md').write_text(qa_md)
pm=f'''# Independent native performance measurements

Fresh task-8 attempt-2 measurements ran through `cargo test --workspace` at {perf['measuredAt']}. No inherited measurement was substituted. Source fingerprint: `{source['sha256']}` (50 files). Raw receipts and all 10,000 latency values are in qa-review/attempt-2/workspace-raw; runtime-validation.json independently recalculates P95, RSS maxima, thread peak and accounting.

| Measurement | Observed | Target / interpretation |
| --- | --- | --- |
| Idle RSS maximum | {perf['idle']['max_rss_bytes']:,} bytes = {perf['idle']['rss_mb']:.6f} MB = {perf['idle']['rss_mib']:.6f} MiB | SRS NFR-002: below 50 MB; passes using disclosed decimal MB convention |
| Native daemon lifetime peak RSS | {perf['lifetime_peak_rss_bytes']:,} bytes = {perf['lifetime_peak_rss_bytes']/1048576:.6f} MiB | wait4 child ru_maxrss; no SRS saturation-peak threshold supplied |
| Native thread sampled peak | {perf['sampled_native_thread_peak']} | Native observations, not inferred configuration; sampling can miss brief peaks |
| Tracked handler task high water | {perf['report']['metrics']['tracked_tasks']['high_water']} / 64 default cap | Structural accounting, not a native thread measurement |
| Successful small-message P95 | {perf['small_message']['p95_ns']:,} ns = {perf['small_message']['p95_ns']/1000000:.6f} ms | SRS NFR-004: ≤20 ms; passes |
| Small-message P50 / maximum | {perf['small_message']['p50_ns']:,} / {perf['small_message']['max_ns']:,} ns | 10,000 sequential successes, zero failures |
| Post-load P95 | {perf['after_load_p95_ns']:,} ns | Separate 200-exchange recovery sample; no during-saturation latency claim |
| 100,000 network exchanges | {perf['load_elapsed_us']:,} µs elapsed; all completed | Four persistent clients; workload is separate from latency baseline |

The ARC SRS was read directly from `{preserve['srs']['path']}` and hash-verified as `{preserve['srs']['sha256']}`. Section 18, lines 2426 and 2428 supply NFR-002 idle RSS and NFR-004 local small-message round-trip P95. Its front matter and summary have inconsistent draft version labels; filename/hash/requirement IDs identify the source. These are development targets requiring platform measurements. Neither structural buffer reservations nor task caps establish measured performance, and the idle threshold is not a saturation-peak threshold. No SRS native thread/task threshold exists in the located source. Comparison completeness is true for these two targets on this native host; it is not full-SRS completion.

Host: {perf['host']['uname']}. Hardware: {perf['host']['hardware'].replace(chr(10),'; ')} (17,179,869,184 bytes RAM; 8 logical CPUs; Apple M1 Pro). Profile: debug/unoptimized with debug information. Rust: {perf['host']['rustc'].splitlines()[0]}; {perf['host']['cargo']}. Default daemon limits are C=64/R=16/Q=8 plus one executing worker; two Tokio runtime threads, main and the SQLite worker were observed as four native threads. The final supervisor gauge is one while it exports its report; observed process exit establishes its completion. Handler gauges exclude the supervisor and native SQLite worker, which have separate accounting.

An owned 200-exchange native probe ran first, all successful. The main child then settled for five seconds, followed by 100 idle RSS/thread samples at nominal 100 ms cadence (observed sample span {perf['idle']['observed_sample_span_us']:,} µs). Load sampling used nominal 10 ms cadence. macOS proc_pidinfo(PROC_PIDTASKINFO) reads the child RSS and thread count; wait4 ru_maxrss reports the owned daemon lifetime peak in bytes. The sampler runs in the test parent; its memory/threads are not attributed to the daemon. The Linux code converts KiB, but Linux was not executed.

Warmup: 200 hello/health exchanges. A new persistent socket then carried 10,000 sequential alternating daemon.hello/daemon.health exchanges, using request ID small and null payload. Encoded body sizes are 85/86 bytes for requests and 124 bytes for responses, each with a four-byte framing prefix. Clock: std::time::Instant; timing starts before request write and ends after complete decoded response. Socket setup ({perf['small_message']['connection_setup_ns']:,} ns) is reported separately. Sorted sample index 9,499 is nearest-rank P95 (ceil(0.95*N)-1); no failure was discarded. This fixture warms a different connection before opening the measured one; the first measured exchange therefore includes that connection's first server dispatch. No SQL doctor work enters the latency baseline.

The four-client × 25,000 load completed 100,000 attempts with no rejection, timeout or cancellation. Its final report contains {perf['report']['metrics']['frames_completed']:,} frames including disclosed latency/warmup/recovery/CLI traffic; admission overload is tested separately. Controlled saturation reaches C=8/R=4/Q=2+1, while separate 100,000 direct DB attempts reach Q=8+1. Task/gauge caps passed independent raw validation. No accepted-HEAD performance baseline was built or measured, so no speedup, RSS reduction or causal before/after performance claim is made.

SIGINT and SIGTERM saturated subprocess exits were {signals['SIGINT']['signalToReapedExitUs']:,} and {signals['SIGTERM']['signalToReapedExitUs']:,} µs, respectively, below five seconds, with every handler/worker accounted for, successful reopen/integrity and restart. Those controlled-subprocess values are not conflated with the default measurement child's lifetime peak or thread sample. Raw signal reports preserve their own peak RSS values.

Remaining: native Linux and Windows qualification, Windows Named Pipe/tray/runtime measurements, long-run idle CPU NFR-001, remaining Phase 0, configured deployed-target applicability/health access, and the unchanged harness's three conformance failures. No participant or commercial experiment, deployed observation or external delivery occurred. The results apply to this host/profile/workload and do not certify indefinitely stalled OS filesystem operations.
'''
(DEST/'performance.md').write_text(pm)
print(json.dumps({'status':'local-verification-completed-with-retained-gaps','sourceFingerprint':source['sha256'],
    'testCounts':counts,'idleRssBytes':perf['idle']['max_rss_bytes'],'p95Ns':perf['small_message']['p95_ns'],
    'harnessExit':commands['harness']['exitCode'],'signals':signals}))
