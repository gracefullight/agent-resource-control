"""Recalculate review evidence before emitting task-8 summaries."""
import datetime
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import sys

sys.dont_write_bytecode = True
OUT = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('runner', OUT / 'run.py')
r = importlib.util.module_from_spec(spec)
spec.loader.exec_module(r)
ROOT = r.runner.ROOT
DEST = OUT.parents[1]
RAW = OUT / 'workspace-raw'
read = lambda p: json.loads(p.read_text())
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
def save(p, value):
    p.write_text(json.dumps(value, indent=2, ensure_ascii=False)+'\n')

source = read(OUT/'context.json')['source']
assert r.runner.fingerprint() == source
assert len(source['files']) == 51
commands = {}
for name, argv in r.ALLOWED.items():
    d = read(OUT/(name+'.json'))
    assert d['argv'] == argv and d['exitCode'] == (1 if name == 'harness' else 0)
    assert d['sourceBefore'] == d['sourceAfter'] == source['sha256']
    assert d['productUnchanged'] and d['processExitObserved'] and not d['timedOut']
    assert d['outputSha256'] == sha(OUT/(name+'.txt'))
    commands[name] = d
text = (OUT/'workspace.txt').read_text()
rows = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out', text)
counts = dict(zip(['passed','failed','ignored','measured','filtered'], map(sum, zip(*(map(int, row) for row in rows)))))
assert counts == {'passed':183,'failed':0,'ignored':0,'measured':0,'filtered':0}
required_tests = [
    'headers_and_trickled_bodies_have_absolute_deadlines',
    'header_progress_does_not_renew_the_body_budget',
    'retained_tasks_reap_and_rejections_do_not_call_factory',
    'cancellation_reaps_unpolled_handlers_and_drops_all_guards',
    'request_admission_races_and_unwind_never_exceed_cap',
    'nonreading_peer_expires_under_write_backpressure',
    'blocked_handler_write_releases_request_and_response_reservations',
    'oversized_echo_uses_one_small_fallback_then_closes',
    'stop_after_final_precheck_before_sql_is_sticky_even_with_early_interrupt',
    'active_sql_is_interrupted_and_worker_joins',
    'shutdown_timeout_retains_handle_until_native_thread_really_exits',
    'lock_wait_is_bounded_and_shutdown_cancels_actual_busy_callback',
    'enqueue_between_empty_check_and_park_is_not_lost',
    'stop_between_empty_check_and_park_is_not_lost',
    'concurrent_cancelled_admission_preserves_queue_and_outstanding_bounds',
    'typed_worker_errors_reach_protocol_and_actual_cli_then_recover',
    'full_request_and_connection_saturation_recovers_without_restart',
    'health_completes_while_native_sql_preparation_is_deliberately_stalled',
    'hundred_thousand_connection_attempts_never_spawn_rejection_tasks',
    'hundred_thousand_request_attempts_have_no_waiting_admission',
    'hundred_thousand_database_attempts_bound_retained_cancelled_work',
    'native_probe_then_full_100000_attempt_workload',
    'saturated_sigint_finishes_within_five_seconds',
    'saturated_sigterm_finishes_within_five_seconds',
    'signal_while_backing_off_is_accounted',
    'report_write_failure_preserves_cleanup_and_actual_worker_accounting',
]
for name in required_tests:
    assert re.search(r'test (?:\S*::)?'+name+r' \.\.\. ok', text), name
preserve = read(OUT/'preservation-targets.json')
assert all(v['byteIdentical'] for v in preserve['frozen'].values())
assert all(v['byteIdentical'] for v in preserve['preserved'].values())
assert preserve['reviewBoundary']['socketPreparationAndBinding']['byteIdentical']
assert len(preserve['originalServerTestNames']) == 10 and all(preserve['originalServerTestNames'].values())
assert not any(v['ignoredAttributes'] for v in preserve['testAttributes'].values())
assert preserve['srs']['sha256'] == 'a8ef945251f9f3fd7c3e337d4efc77ff9914f95114212ca9e6c90ae2ce3e3d9e'
assert {v['line'] for v in preserve['srs']['lines']} == {2426,2428}

def check_metrics(m, caps):
    bounds = {'connections':caps['connections'], 'requests':caps['requests'],
        'tracked_tasks':caps['connections'], 'db_queued':caps['db_queue'],
        'db_executing':1, 'db_outstanding':caps['db_queue']+1, 'rejection_work':1,
        'request_body_bytes':caps['requests']*1048576, 'response_bytes':caps['requests']*1048576}
    result = {}
    for key, cap in bounds.items():
        v = m[key]
        assert v['current'] == 0 and 0 <= v['high_water'] <= cap, (key,v,cap)
        result[key] = {'cap':cap, 'highWater':v['high_water'], 'final':v['current']}
    assert m['handlers_spawned'] == m['handlers_reaped']
    assert m['worker_started'] == m['worker_joined']
    return result

reports = {}
for p in sorted(RAW.glob('*.json')):
    d = read(p)
    for key in ['report','restart_report']:
        if key not in d:
            continue
        v = d[key]
        for flag in ['accounted','handlers_reaped','worker_joined','connection_closed',
                     'gauges_zero','socket_removed','deadline_met']:
            assert v[flag] is True, (p,key,flag)
        assert v['handlers_remaining'] == 0
        assert v['metrics']['worker_started'] == v['metrics']['worker_joined'] == 1
        assert v['supervisor']['retry_tasks'] == 0
        assert v['supervisor']['supervisors'] == {'current':1,'high_water':1}
        reports[p.name+':'+key] = check_metrics(v['metrics'],v['limits'])
attempts = {}
for name, admitted in [('connection',8),('request',4),('db',9)]:
    d = read(RAW/(name+'-100000.json'))
    assert d['attempted'] == 100000 and d['admitted'] == admitted
    assert d['rejected'] + admitted == 100000
    assert sum(d.get(k,0) for k in ['completed','timed_out','shutdown_cancelled',
        'released_without_dispatch','client_cancelled']) == admitted
    bounds = check_metrics(d['final'],d['final']['caps'])
    assert d['final']['worker_started'] == (1 if name == 'db' else 0)
    attempts[name] = {k:v for k,v in d.items() if k != 'final'}
    attempts[name]['bounds'] = bounds
signals = {}
for name in ['SIGINT','SIGTERM']:
    d = read(RAW/(name+'.json'))
    assert d['signal_to_exit_us'] < 5000000 and d['reopen_integrity'] == 'ok'
    assert d['report']['reason'] == name
    m = d['report']['metrics']
    assert [m[k]['high_water'] for k in ['connections','requests','tracked_tasks',
            'db_queued','db_executing','db_outstanding']] == [8,4,8,2,1,3]
    assert m['db_timeouts'] == 0
    assert m['request_rejected'] == m['connection_rejected'] == 1
    assert m['handlers_spawned'] == m['handlers_reaped'] == 11
    signals[name] = {'signalToReapedExitUs':d['signal_to_exit_us'],
        'handlersSpawnedReaped':11, 'workerStartedJoined':1, 'allGaugesZero':True,
        'reopenIntegrity':'ok', 'restartVerified':True,
        'nativePeakRssBytes':d['native_peak_rss_bytes']}
frames = read(RAW/'frames.json')['report']['metrics']
assert frames['header_timeouts'] == frames['frame_timeouts'] == 1
nonreader = read(RAW/'nonreader.json')
assert nonreader['report']['metrics']['write_timeouts'] == 1 and nonreader['closure_us'] < 800000
recovery = read(RAW/'deadline-saturation.json')
assert recovery['recovered_same_process'] and recovery['recovery_us'] <= 550000
for k in ['header_timeouts','frame_timeouts']:
    assert recovery['report']['metrics'][k] == 4
for k in ['connection_rejected','request_rejected']:
    assert recovery['report']['metrics'][k] == 1
health = read(RAW/'native-sql-health.json')
assert health['health_cli_us'] < 500000
perf = read(RAW/'native-measurements.json')
obs = read(RAW/'native-observations.json')
samples = read(RAW/'small-message-samples.json')
assert perf['attempts'] == {'attempted':100000,'admitted':100000,'completed':100000,
                          'rejected':0,'timed_out':0,'shutdown_cancelled':0}
assert len(obs['idle']) == 100 and len(obs['load']) > 0
assert max(v['rss_bytes'] for v in obs['idle']) == perf['idle']['max_rss_bytes']
assert max(v['threads'] for v in obs['idle']+obs['load']) == perf['sampled_native_thread_peak']
assert len(samples['samples_ns']) == 10000 and samples['failed'] == 0
assert sorted(samples['samples_ns'])[9499] == perf['small_message']['p95_ns']
assert perf['lifetime_peak_rss_bytes'] > 0
assert perf['idle']['max_rss_bytes'] < 50000000 and perf['small_message']['p95_ns'] <= 20000000
probe = read(RAW/'native-probe.json')
assert probe['attempts'] == probe['completed'] == 200
harness = (OUT/'harness.txt').read_text()
assert all(s in harness for s in ['901 lines','551 lines','Script not found "test:coverage"','Total violations: 3'])
(DEST/'harness.txt').write_bytes((OUT/'harness.txt').read_bytes())
perf.update({'scope':'Fresh independent task-8 attempt-3 current-tree execution through cargo test --workspace',
    'sourceFingerprint':source['sha256'], 'sourceFileCount':len(source['files']),
    'measuredAt':commands['workspace']['startedAt'], 'nativeMeasurementsRecorded':True,
    'srsComparisonComplete':True, 'rawArtifacts':[str(p.relative_to(ROOT)) for p in sorted(RAW.glob('*.json'))]})
perf['small_message']['sizes'] = samples['sizes']
perf['idle']['observed_sample_span_us'] = obs['idle'][-1]['elapsed_us']-obs['idle'][0]['elapsed_us']
perf['idle']['samples'] = len(obs['idle'])
perf['load_samples'] = len(obs['load'])
perf['srs'].update({'sourcePath':preserve['srs']['path'], 'sourceVerifiedInIndependentReview':True,
    'comparisonScope':'NFR-002 idle RSS and NFR-004 small-message P95 on this macOS debug workload; no SRS peak-load RSS, thread or task threshold supplied'})
save(DEST/'performance.json',perf)
qa = {k:read(OUT/'preceding-qa.json')[k] for k in ['schemaVersion','actor','task','baselineHead',
    'findings','interfaceCompatibility','codeIntelligence']}
qa.update({'attempt':3, 'recordedAt':datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'skill':'oma-qa', 'status':'partial', 'reviewResult':'NOT_ASSESSED', 'verificationCompleteness':'partial',
    'localRuntimeVerification':'completed',
    'scope':'Fresh independent verification of integrated 51-file source; harness failures and target access gap retained',
    'behavioralContractVerified':True, 'compatibilityVerified':True, 'migrationUnchanged':True,
    'cargoFmtPassed':True, 'cargoClippyPassed':True, 'cargoTestsPassed':True,
    'harnessExecuted':True, 'harnessUnchanged':True, 'unresolvedHarnessFailuresRetained':True,
    'harnessPassed':False, 'harnessExitCode':1, 'sourceFingerprint':source['sha256'],
    'sourceFileCount':len(source['files']), 'testCounts':counts, 'attemptExperiments':attempts,
    'nativeNetworkWorkload':perf['attempts'], 'signalCases':signals,
    'commandReceipts':{k:str((OUT/(k+'.json')).relative_to(ROOT)) for k in commands},
    'containerHealthVerified':False, 'configuredTargetApplicabilityVerified':False,
    'missionCompletionClaimed':False,
    'verificationGaps':[
        {'path':'av.yaml', 'reason':preserve['containerConfiguration'].get('error'),
         'effect':'Configured deployed targets cannot be established; health, availability and timestamped recent errors remain unmet if targets exist.',
         'owner':'Chief Director: arrange supported configuration access and assign any external observation separately.'},
        {'path':'docs/bounded-ipc-final.md', 'line':9,
         'reason':'Historical task-9 report cites 50-file fingerprint b3610549 and task-8 attempt-2; current source and fresh independent receipts cover 51 files.',
         'effect':'Final mission assessment needs task-9 refresh; preceding task review remains historical.',
         'owner':'Chief Director / task-9'},
        {'path':'platform qualification', 'reason':'Native execution covers macOS only.',
         'effect':'Linux, Windows and remaining Phase 0 are not certified.',
         'owner':'Chief Director / platform implementers'}],
    'remainingObligations':['Three harness conformance failures', 'Configured-target applicability/access and actual observations',
        'Task-9 current-source final assessment', 'Remaining Phase 0, Linux and Windows runtime qualification'],
    'failedAttempts':[
        {'operation':'Supported direct av.yaml read', 'result':preserve['containerConfiguration'].get('error')},
        {'operation':'Known-path .cargo/config.toml read', 'result':'No such file; Cargo used prepared existing cache successfully.'},
        {'operation':'Shell entry mise tracking link', 'result':'Operation not permitted; checks themselves completed.'}],
    'ownership':'Only five assigned top-level QA/performance/harness files and associated attempt-3 receipts changed; source/tests/manifests, mission state and framework reviews preserved.'})
save(DEST/'qa.json',qa)
save(OUT/'runtime-validation.json',{'sourceFingerprint':source['sha256'], 'testCounts':counts,
    'requiredTestsObservedPassing':required_tests, 'attemptExperiments':attempts,
    'reportBounds':reports, 'signalCases':signals,
    'recomputedIdleRssBytes':max(v['rss_bytes'] for v in obs['idle']),
    'recomputedP95Ns':sorted(samples['samples_ns'])[9499],
    'recomputedNativeThreadPeak':max(v['threads'] for v in obs['idle']+obs['load']),
    'allRawReportAssertionsPassed':True})

cmdrows='\n'.join('| `'+ ' '.join(d['argv'])+'` | '+str(d['exitCode'])+' | '+f"{d['elapsedSeconds']:.3f} s"+' | [receipt](qa-review/attempt-3/'+name+'.json) / [output](qa-review/attempt-3/'+name+'.txt) |' for name,d in commands.items())
qa_md=f'''# Independent bounded IPC review

## Review Result: NOT_ASSESSED

Verification completeness: partial. The current macOS bounded-runtime, compatibility and performance checks passed. Three MEDIUM harness conformance findings remain, and configured-target applicability is unverified because the supported read of av.yaml is denied. No CRITICAL or HIGH runtime defect was confirmed. This report does not certify mission completion.

Actor reviewer, task-8, attempt 3; skill oma-qa. Read the execution policy, result contract, QA execution protocol and checklist. Existing changes were inspected first. No product code, tests, manifest, saved assignment, acceptance criterion, mission state or framework task review was edited. No delegation, browser work, installation, download or standalone build occurred. Serena is absent from the exposed tools; review used known-path reads and scoped Git diffs. No run ID or claim path was injected, so these assigned files and associated receipts are the material handoff.

## Evidence boundary

HEAD remains accepted `42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0`. The implementation is an uncommitted overlay. All **51** crate source/test/manifest/toolchain files fingerprint to `{source['sha256']}`. The sorted path/hash/mode records are in [context](qa-review/attempt-3/context.json) and each command's source manifest. Exact command receipts bind identical before/after bytes. Prior task-8 top-level summaries were copied to preceding-* in attempt-3 before replacement; prior receipts and framework reviews remain at their original paths. This run supersedes the older 50-file QA execution without rewriting its history.

## Findings and remediation ownership

### CRITICAL

None confirmed.

### HIGH

None confirmed.

### MEDIUM

- `crates/arc-daemon/tests/resident.rs:501`: 901 lines exceeds the unchanged harness's 500-line cap. Chief Director / Runtime Test Engineer must coordinate a split into focused integration modules, preserving support::serial, fixture lifetimes and all assertions. Retain the original check and cap. QA has no test-edit ownership.
- `crates/arc-ipc/src/lib.rs:501`: 551 lines exceeds the same cap (accepted HEAD: 549). Chief Director / IPC Engineer own a behavior-preserving split. Concrete structure: `#[cfg(test)] mod tests;` in lib.rs, moving the existing test body to src/tests.rs while retaining the public wire/framing API.
- `scripts/harness/validate.sh:420`: the unchanged command runs missing `test:coverage` and exits 1. Chief Director / harness owner must resolve the Rust-workspace/inherited coverage contract outside task-8. Preserve this failure; do not add dummy coverage, skip flags, alternate verification commands or JavaScript tooling to manufacture conformance. No honest task-8 code patch can resolve that ownership conflict.

### LOW

None confirmed.

## Exact executed gates

Commands ran serially in this mission worktree with CARGO_NET_OFFLINE=true and prepared caches. No argv was changed. Raw receipts retain argv/cwd, timestamps, PID/process group, exit, timeout state, output hashes and source fingerprints. No command timed out. Saved limits were preserved: deadline 2026-11-08T10:52:35.601Z, duration 86,400 seconds, 200 runs, three retries, maximum three concurrent workspace tasks. This review used one check at a time, with a 120-second command timeout and bounded owned-process termination. Host resource limits and deadline provenance are recorded in context.json. No cancellation was received and no mission state was changed.

| Command | Exit | Elapsed | Evidence |
| --- | --- | --- | --- |
{cmdrows}

Workspace tests: **183 passing invocations**, zero failed/ignored/measured/filtered. These include imported production unit modules repeated across test targets and child-entry functions that return in parent execution; 183 is not a count of unique scenarios. Parent scenarios do launch their controlled subprocesses. The exact workspace command independently executed the delivered native probe/workload and all required signal/CLI cases. No extra runtime replay was needed after passing checks.

The unchanged harness ran in all-tracked mode over 1,337 entries and exited **1**, retaining exactly the three findings above. New untracked Rust modules are outside its tracked inventory but inside the independent 51-file source fingerprint, Clippy and test run. No coverage skip was supplied. Its secret/layer/shell checks pass within that inventory; no comprehensive dependency-vulnerability or license audit is claimed.

## Race, security and lifecycle audit

`admission.rs:36`: one supervisor reaps JoinSet entries before admission, authenticates the peer, checks retained length and acquires the permit before invoking the handler factory. Completed but unreaped entries remain bounded. Inline rejection drops the stream without creating a task; rejection-work high water is one. RequestGuard drops its gauge before returning its semaphore permit. Active tests cover unpolled cancellation, retained completions, concurrent request admission and unwind.

`connection.rs:36` / `transport.rs:10`: one frame clock starts before reading the four-byte big-endian header. Size validation and request admission precede body allocation. Header/body partial progress cannot extend expiry. Both readiness loops check the original absolute deadline. Capped response serialization checks retained capacity and length before any prefix; an oversize fallback shares its original write deadline. Request/body payload is discarded before database waits. Byte gauges bound frame/reservation ownership, not all decoded heap, native RSS or Tokio internals. Parsing remains bounded by the finite 1 MiB admitted frame.

`db_worker.rs:132` / `db_worker/execution.rs:87`: exactly one owned SQLite thread receives a finite mpsc queue. A separate queue permit survives dequeue until queue-gauge release; canceled queued work remains accounted until drain. Only two fixed read-only diagnostic statements run in production; clients cannot supply SQL. Sticky checks around statements plus progress and busy callbacks cover early-interrupt/query-start races. Unit tests actively execute SQLite VM cancellation, lock waits, execution/response deadlines, empty-check/park races, canceled concurrent producers, full-queue shutdown and native-handle retention after a shutdown timeout.

`lifecycle.rs:107`: signals have priority; handlers are continuously reaped. Accept failure backoff is inline and capped at 100 ms, creates no retry task and remains signal/reaping responsive. Shutdown drops the listener, stops the worker, aborts/reaps handlers, joins the actual worker, closes SQLite and removes only the owned socket inode under one 3,500 ms deadline. The instance lock remains held through joining; the executed lifecycle fixture checks reacquisition failure before release. Fatal listener, startup and report failures exercise cleanup. Worker Drop joins rather than detaching. Tests establish these controlled conditions; indefinitely stalled kernel/filesystem calls are not qualified.

`runtime_report.rs:37`: optional export validates a private parent, opens create-new/no-follow, verifies same UID/0600/regular type/link count, retains the descriptor and caps serialization at 4,096 bytes. Active tests cover unsafe/replaced paths, output overflow and real OS write failure without bypassing cleanup. No request-derived history/labels accumulate. Same-user native socket checks retain UID policy; live same-user/non-socket cases pass, while foreign-UID policy is statically preserved rather than claimed as a new cross-user runtime experiment.

The classification repair in `lifecycle.rs` maps operational/resource/unexpected DB failures to DB_BUSY and only positive SQLITE_CORRUPT/SQLITE_NOTADB or failed diagnostic reports to INTEGRITY_FAILED. Executed regressions pass typed errors through the production adapter/connection handler and actual CLI, checking exact IPC_ERROR versus INTEGRITY_FAILED, sanitization and status/doctor recovery. This is a typed failure injection; it does not claim the host actually exhausted memory or suffered disk I/O failure.

Defaults are C=64 connections/retained handlers, R=16 requests, Q=8 queued jobs plus one executing. Header/frame/write budgets: 1,000/2,000/500 ms; database response/execution/busy budgets: 1,500/1,000/100 ms. Test profile: C=8/R=4/Q=2, transport 150/300/100 ms, database 500/300/50 ms. Both use a 3,500 ms shutdown budget. Invalid limits/profiles are actively rejected. Hello/health bypass SQLite, but admission provides no reserved health slots during complete transport saturation.

## Acceptance evidence

The [runtime validator](qa-review/attempt-3/runtime-validation.json) independently recomputes every reported high water against its configured cap, final zero gauges, attempt reconciliation and sample percentiles. Fresh JSON observations are in qa-review/attempt-3/workspace-raw. Summary booleans were not taken as proof.

| Requirement | Actual fresh result | Assessment |
| --- | --- | --- |
| 100,000 connection attempts | 8 admitted + 99,992 rejected; 8 shutdown-canceled/reaped; retained-task high water 8, inline rejection-work 1 | Verified |
| 100,000 request admissions | 4 admitted + 99,996 rejected; four released without dispatch; recovered permit succeeds | Verified |
| 100,000 DB attempts | 9 admitted + 99,991 Busy; canceled/drained; queue 8 + executing 1; worker joins, reopen integrity ok | Verified |
| Native daemon exchanges | Four persistent clients ×25,000 = 100,000 completed; no rejection/timeout/cancellation | Verified |
| Absolute input deadlines | Paused-time header/body trickle and shared frame-clock cases; native stalled headers/partial bodies and invalid/version recovery | Verified |
| Real write backpressure | Non-reading 512 KiB echo with SO_RCVBUF=1024; write-timeout count 1; HUP observed at {nonreader['closure_us']} µs without draining before closure; both CLI commands recover | Verified |
| Transport saturation/recovery | C=8/R=4 attained; both overload CLI mappings checked; deadline releases and status/doctor recover in same process after {recovery['recovery_us']} µs | Verified |
| Slow SQL / health | Actual status CLI completes in {health['health_cli_us']} µs while bounded test-only authorizer gate stalls preparation of real integrity SQL; normal diagnostics recover after release | Verified |
| Saturated SIGINT | {signals['SIGINT']['signalToReapedExitUs']} µs to reaped successful exit; 11/11 handlers, 1/1 worker; zero gauges, SQLite closed, socket removed, restart and reopen integrity ok | Verified, below five seconds |
| Saturated SIGTERM | {signals['SIGTERM']['signalToReapedExitUs']} µs; same complete accounting/recovery | Verified, below five seconds |
| Protocol/CLI/identity/security | Existing protocol-1 framing, CLI output/error/version cases, TMPDIR-independent identity, single-instance and filesystem protections execute successfully | Verified locally |
| Frozen migration | Direct byte comparison with accepted HEAD; SHA-256 {preserve['frozen']['crates/arc-storage/migrations/0001_phase0.sql']['sha256']} | Unchanged |
| Native performance / SRS | Fresh samples recalculated; NFR-002 idle RSS and NFR-004 P95 pass on this host | Verified for this slice |
| Harness | Actual unchanged command exits 1; all three failures retained | Evidence complete; conformance unresolved |
| Configured targets | Direct av.yaml read denied; target existence, health, availability and recent errors not established | Unmet access/applicability |

The four 100,000-attempt experiments are separate; they are not 400,000 network exchanges. Direct boundary tests assert bounds after each attempt, and atomically retained high waters record transient maxima. Controlled races supplement the boundary loops. Native measurements and structural accounting remain separate. Both signal fixtures synchronize three DB jobs, a partial body and four stalled headers before signaling and require work to remain active at that point.

Direct accepted-HEAD comparisons confirm byte-identical CLI source/binary tests, core identity, storage source/NFR-010 and instance code. The 9,965-byte socket preparation/binding block remains byte-identical. All ten relocated server test names remain, with filesystem/protocol assertions inspected. Resident/support changes release only the shared process lock while retaining the thread mutex until the child exits; the coordination regression passes with its assertions retained. No tests were disabled, no gates weakened. Synchronous CLI framing remains; additive DB_BUSY uses the unchanged generic IPC_ERROR fallback. Web/JWT/CORS/XSS/visual accessibility checks do not apply to this Unix daemon/CLI change; successful text is stable and color-independent, but no assistive-technology runtime qualification is claimed.

## Handoff and remaining work

Chief Director must retain the three harness findings and route any repair to the owners above. The denied av.yaml read is an access gap, not evidence of absent or healthy deployed targets. Supported configuration access is needed before actual timestamped health/availability/recent-error observations can be collected. No external restart, rollback or delivery occurred.

Chief / task-9 must refresh `docs/bounded-ipc-final.md:9`, which still cites the 50-file b3610549 snapshot and attempt-2 QA results. Use this 51-file fingerprint, attempt-3 receipts, 183 passing invocations and current measurements/signals. QA preserved the historical final assessment and its framework review; neither is promoted to a current-source approval.

Remaining Phase 0 includes Windows Named Pipe IPC/runtime/security, exec/Managed Job, desktop/menu-bar/tray integration, hook/locality fixtures and two-OS integration. Linux and Windows runtime/performance qualification remain unexecuted here. Vendor detection/classification, Meter/Traffic/Sweeper, NFR-001 long-run idle CPU, signing/packaging and broad crash-mode qualification remain open. Successful graceful reopen does not certify every forced-termination mode.

Recorded failed operations: av.yaml read returned Operation not permitted; .cargo/config.toml was absent; shell startup emitted a denied mise tracking-link warning. All required Cargo gates still passed. No denied operation was bypassed. Changed paths are only qa.json, qa.md, performance.json, performance.md, harness.txt and associated qa-review/attempt-3 raw receipts/scripts. Product interfaces and other Actors' work remain preserved.
'''
(DEST/'qa.md').write_text(qa_md)
pm=f'''# Independent native performance measurements

Task-8 attempt 3 executed the delivered probe and full native workload through the exact `cargo test --workspace` command at {perf['measuredAt']}. This is a fresh independent run of the 51-file integrated snapshot `{source['sha256']}`. Earlier measurements remain historical. Raw data and all 10,000 latency values are in qa-review/attempt-3/workspace-raw; runtime-validation.json recalculates RSS maxima, P95, sampled thread peak and task accounting.

| Measurement | Actual observation | SRS comparison / meaning |
| --- | --- | --- |
| Idle RSS maximum | {perf['idle']['max_rss_bytes']:,} bytes = {perf['idle']['rss_mb']:.6f} MB = {perf['idle']['rss_mib']:.6f} MiB | NFR-002 below 50 MB: passes using disclosed decimal-MB convention |
| Native daemon lifetime peak RSS | {perf['lifetime_peak_rss_bytes']:,} bytes = {perf['lifetime_peak_rss_bytes']/1048576:.6f} MiB | wait4 child ru_maxrss, macOS bytes; no SRS peak-under-load threshold |
| Native thread sampled peak | {perf['sampled_native_thread_peak']} | Observed child threads; sampling can miss short peaks |
| Tracked handler task high water | {perf['report']['metrics']['tracked_tasks']['high_water']} / 64 default cap | Instrumented structural count; supervisor and worker separately accounted |
| Successful small-message P95 | {perf['small_message']['p95_ns']:,} ns = {perf['small_message']['p95_ns']/1000000:.6f} ms | NFR-004 within 20 ms: passes |
| Small-message P50 / max | {perf['small_message']['p50_ns']:,} / {perf['small_message']['max_ns']:,} ns | 10,000 successful sequential exchanges, zero failures |
| Post-load P95 | {perf['after_load_p95_ns']:,} ns | Separate 200-exchange recovery sample; no during-saturation latency claim |
| 100,000 native exchanges | {perf['load_elapsed_us']:,} µs elapsed; all completed | Four persistent clients; separate from latency baseline |

The available authoritative ARC SRS was read directly from `{preserve['srs']['path']}` and independently hash-verified as `{preserve['srs']['sha256']}`. Section 18 lines 2426 and 2428 supply NFR-002 idle RSS below 50 MB and NFR-004 local small-message round-trip P95 within 20 ms. The SRS does not define MB precisely; this report uses 50,000,000 bytes and also gives MiB. Its front matter and summary/footer carry inconsistent draft version labels; filename/hash/requirement IDs identify the source. Comparison completeness is true for these two applicable targets on this host, not for the whole SRS. No peak-RSS, thread or task target was invented.

Host: {perf['host']['uname']}. Hardware: {perf['host']['hardware'].replace(chr(10),'; ')}: 16 GiB RAM, eight logical CPUs, Apple M1 Pro. Rust: {perf['host']['rustc'].splitlines()[0]}; {perf['host']['cargo']}. Profile: debug/unoptimized with debug information. Default caps C=64/R=16/Q=8 plus one executing DB job. Four native threads were observed; this is separate from the configured two Tokio workers and instrumented handler high water. Handler counts are not a census of Tokio-internal tasks; the one supervisor and native SQLite worker have separate termination accounting.

Measurement method: an owned 200-exchange probe succeeds before the full workload. The measurement daemon settles five seconds, then supplies 100 idle RSS/thread samples at nominal 100 ms cadence (observed span {perf['idle']['observed_sample_span_us']:,} µs). Load observations number {len(obs['load'])}, at nominal 10 ms cadence. macOS proc_pidinfo(PROC_PIDTASKINFO) reads the child RSS/thread count; wait4 ru_maxrss captures that owned daemon's native lifetime peak in bytes at reaped exit. The sampler runs in the test parent and its resources are not counted as the daemon. Linux measurement code exists but did not execute in this run.

After 200 warmup exchanges, a new persistent connection carries 10,000 sequential alternating daemon.hello/daemon.health exchanges, request ID small and null payload. Request bodies are 85/86 bytes, responses 124 bytes, plus four prefix bytes per direction. Timing uses std::time::Instant from before request write through complete decoded response. Socket setup ({perf['small_message']['connection_setup_ns']:,} ns) is separate. P95 is sorted index 9,499, nearest rank ceil(0.95*N)-1; no failures were discarded. Warmup uses a different connection, so the first measured exchange includes that connection's first server dispatch. No doctor/SQL enters this baseline latency sample.

Four persistent clients ×25,000 then complete 100,000 real network exchanges with zero rejected/timed-out/canceled attempts. The final report includes {perf['report']['metrics']['frames_completed']:,} completed frames, including disclosed warmup/latency/recovery/CLI traffic. All native workload gauges end at zero; high waters stay inside default caps. Adversarial overload is separately exercised at C=8/R=4/Q=2+1 and through 100,000 direct admission attempts per connection/request/DB boundary. Direct DB admission attains Q=8+1. Those counters prove structural bounds, not measured memory or latency.

Saturated SIGINT and SIGTERM signal-to-reaped-exit durations are {signals['SIGINT']['signalToReapedExitUs']:,} and {signals['SIGTERM']['signalToReapedExitUs']:,} µs, respectively, both below five seconds. Each has 11/11 handlers reaped, one worker joined, closed SQLite, zero gauges, socket cleanup, successful database reopen/integrity and restart. Their native peak RSS values remain in their own raw files and are not substituted for the measurement daemon's lifetime peak.

No accepted-HEAD benchmark was built or measured; this report makes no speedup or RSS-reduction claim. It also makes no release-profile, during-saturation latency or cross-platform result claim. Remaining obligations: three unchanged-harness conformance failures, configured-target applicability/health access, Linux and Windows runtime/Named Pipe/tray/performance qualification, NFR-001 long-run idle CPU and the remaining Phase 0 features. Indefinitely stalled kernel/filesystem calls and full forced-termination qualification are outside the measured cases. No participant, commercial or deployed-service observation occurred.
'''
(DEST/'performance.md').write_text(pm)
print(json.dumps({'sourceFingerprint':source['sha256'], 'testCounts':counts,
    'idleRssBytes':perf['idle']['max_rss_bytes'], 'lifetimePeakRssBytes':perf['lifetime_peak_rss_bytes'],
    'p95Ns':perf['small_message']['p95_ns'], 'signals':signals,
    'status':'local-verification-complete-with-retained-gaps'}))
