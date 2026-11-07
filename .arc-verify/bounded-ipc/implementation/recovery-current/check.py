"""Read-only evidence verifier. Inspect, then run python3 -B check.py.

No tests, builds, writes, staging, object imports, refs or mission changes occur.
A local run exits 2 while actual receiving collection is pending. After normal
collection supply --receiving-delivery with AV's actual delivery.json. That
file must supply this payload and binding in changes and a real delivery-tree
blob. Historical mismatches retain their original failed status.
"""
import argparse
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import re
import subprocess
import sys

sys.dont_write_bytecode = True
OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[3]
BASE = ROOT / '.arc-verify/bounded-ipc'
OLD = BASE / 'implementation/resume-attempt-3'
A8 = BASE / 'implementation/collection-repair/attempt-8'
REPORT = '.agents/results/result-test-engineer-task-5-uninjected-attempt-3-b137e253-def6-46ef-a75d-25ab414784ea.md'
REPORT_SHA = '43c6657a2ddb223d78e1fa4299b719499a8527d90d741e843d66a6878fce991d'
ACCEPTED = '42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0'
EXPECTED = '9e3335b98f73d9ef964809c8fbb08fb1530661adb1d6e2c1adac492d28543970'
DOCS = {'docs/bounded-ipc-recovery.md', 'docs/runtime-identity-usage.md'}
SCOPE = ['crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml']


def load(p):
    return json.loads(p.read_text())


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def require(ok, message):
    if not ok:
        raise AssertionError(message)


def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT, stderr=subprocess.PIPE)


def paths(*scope):
    return set(git('ls-files', '--cached', '--others', '--exclude-standard', '-z', '--', *scope).decode().split('\0')) - {''}


def meta(path):
    link = path.is_symlink()
    raw = os.readlink(path).encode() if link else path.read_bytes()
    return {'kind': 'symlink' if link else 'file', 'bytes': len(raw), 'sha256': sha(raw), 'mode': oct(path.lstat().st_mode & 0o777)}


def fingerprint():
    files = [{'path': p, 'sha256': sha((ROOT/p).read_bytes()), 'mode': oct((ROOT/p).stat().st_mode & 0o777)} for p in sorted(paths(*SCOPE))]
    return {'sha256': sha(json.dumps(files, sort_keys=True).encode()), 'scope': SCOPE, 'files': files}


def verify_artifacts(rows):
    seen = set()
    for row in rows:
        p = row['path']
        require(p not in seen, 'duplicate artifact: '+p)
        seen.add(p)
        actual = meta(ROOT/p)
        require(actual['kind'] == 'file' and actual['bytes'] == row['bytes'] and actual['sha256'] == row['sha256'], 'artifact mismatch: '+p)
    return len(seen)


def git_oid(kind, raw):
    return hashlib.sha1(kind.encode()+b' '+str(len(raw)).encode()+b'\0'+raw).hexdigest()


def tree_oid(entries):
    nodes = {}
    for path, entry in entries.items():
        bits = path.split('/')
        node = nodes
        for bit in bits[:-1]:
            node = node.setdefault(bit, {})
        node[bits[-1]] = (entry['mode'], entry['oid'])

    def visit(node):
        raw = b''
        for name, value in sorted(node.items(), key=lambda kv: (kv[0]+('/' if isinstance(kv[1], dict) else '\0')).encode()):
            mode, oid = ('40000', visit(value)) if isinstance(value, dict) else value
            raw += mode.encode()+b' '+name.encode()+b'\0'+bytes.fromhex(oid)
        return git_oid('tree', raw)
    return visit(nodes)


def delivery_proof(receipt, required_tree=None, verify_current=True):
    """Verify the complete Merkle root without importing absent delivery objects.

    Snapshot objects come from Git. Every change's before OID/mode must match
    that snapshot. Every after blob is recomputed from the received file bytes.
    Recompute all tree objects up to the independently supplied delivery root.
    This also works when the collector retained JSON/patch but no tree object.
    """
    d = load(receipt)
    snapshot = d['workspace']['snapshotCommit']
    entries = {}
    for row in git('ls-tree', '-rz', '--full-tree', snapshot).split(b'\0'):
        if row:
            header, p = row.split(b'\t', 1)
            mode, kind, oid = header.decode().split()
            require(kind in ['blob', 'commit'], 'unsupported tree kind')
            entries[p.decode()] = {'mode': mode, 'oid': oid}
    require(tree_oid(entries) == d['workspace']['snapshotTree'], 'snapshot Merkle root: '+str(receipt))
    blobs = []
    for change in d['changes']:
        p, after = change['path'], change['after']
        require(entries.get(p) == change['before'], 'before blob differs from snapshot: '+p)
        if after is None:
            entries.pop(p)
        else:
            entries[p] = after
            if verify_current:
                actual = meta(ROOT/p)
                raw = os.readlink(ROOT/p).encode() if actual['kind'] == 'symlink' else (ROOT/p).read_bytes()
                mode = '120000' if actual['kind'] == 'symlink' else ('100755' if int(actual['mode'], 8) & 0o111 else '100644')
                require(git_oid('blob', raw) == after['oid'] and mode == after['mode'], 'delivery blob/mode: '+p)
                blobs.append({'path': p, 'blob': after['oid'], **actual})
    calculated = tree_oid(entries)
    require(calculated == d['deliveryTree'], 'delivery Merkle root: '+str(receipt))
    if required_tree:
        require(calculated == required_tree, 'independently supplied delivery root differs')
    return {'snapshotCommit': snapshot, 'snapshotTree': d['workspace']['snapshotTree'], 'deliveryTree': calculated,
            'completeMerkleRootRecomputed': True, 'changedBlobCount': len(blobs), 'blobs': blobs}


def receipt_check(folder, name, argv, code, expected):
    r = load(folder/(name+'.json'))
    require(r['argv'] == argv and r['exitCode'] == code and not r['timedOut'], 'argv/exit/timeout: '+name)
    require(r['processExitObserved'] and r['productInputsUnchanged'], 'process/source receipt: '+name)
    require(r['sourceBefore'] == r['sourceAfter'] == expected['sha256'], 'source binding: '+name)
    require(load(folder/(name+'-source.json')) == expected, 'complete before inputs/modes: '+name)
    after = folder/(name+'-source-after.json')
    if after.exists():
        require(load(after) == expected, 'complete after inputs/modes: '+name)
    output = folder/r['output']
    require(sha(output.read_bytes()) == r['outputSha256'], 'output hash: '+name)
    require(r['finishedAt'] >= r['startedAt'], 'UTC interval: '+name)
    return {'path': str((folder/(name+'.json')).relative_to(ROOT)), 'argv': argv, 'exitCode': code,
            'startedAt': r['startedAt'], 'finishedAt': r['finishedAt'], 'outputSha256': r['outputSha256'],
            'sourceFingerprint': expected['sha256'], 'completeInputFiles': len(expected['files'])}


def check_gauges(m, limits):
    caps = {'connections': limits['connections'], 'requests': limits['requests'], 'tracked_tasks': limits['connections'],
            'db_queued': limits['db_queue'], 'db_executing': 1, 'db_outstanding': limits['db_queue']+1,
            'request_body_bytes': limits['requests']*1048576, 'response_bytes': limits['requests']*1048576, 'rejection_work': 1}
    for key, cap in caps.items():
        require(m[key]['current'] == 0 and 0 <= m[key]['high_water'] <= cap, 'gauge cap/final: '+key)
    return {key: {'highWater': m[key]['high_water'], 'cap': cap, 'final': m[key]['current']} for key, cap in caps.items()}


def check_report(r):
    for key in ['accounted', 'handlers_reaped', 'worker_joined', 'connection_closed', 'gauges_zero', 'socket_removed', 'deadline_met']:
        require(r[key] is True, 'shutdown accounting: '+key)
    require(r['handlers_remaining'] == 0 and r['metrics']['handlers_spawned'] == r['metrics']['handlers_reaped'], 'remaining handlers')
    require(r['metrics']['worker_started'] == r['metrics']['worker_joined'] == 1, 'native worker join')
    return check_gauges(r['metrics'], r['limits'])


def main(args):
    results, failures = {}, {}
    current = fingerprint()

    def run(name, fn):
        try:
            results[name] = fn()
        except (AssertionError, OSError, KeyError, ValueError, subprocess.CalledProcessError) as e:
            failures[name] = str(e)

    def historical():
        index = load(OLD/'evidence.json')
        require(verify_artifacts(index['artifacts']) == 107, 'historical index count')
        old = load(OLD/'final-source.json')
        require(sha(json.dumps(old['files'], sort_keys=True).encode()) == old['sha256'] == index['sourceFingerprint'], 'historical fingerprint invalid')
        receipt_check(OUT, 'historical-verifier', ['python3', '-B', '.arc-verify/bounded-ipc/implementation/resume-attempt-3/verify.py'], 1, current)
        require('line 52' in (OUT/'historical-verifier.txt').read_text(), 'original failure location')
        receipt_check(OUT, 'attempt8-checker', ['python3', '-B', '.arc-verify/bounded-ipc/implementation/collection-repair/attempt-8/check.py'], 1, current)
        retained_failures = load(OUT/'attempt8-checker.txt')['failures']
        require(set(retained_failures) == {'repairBoundary', 'currentProduct', 'task6Attribution', 'historicalBoundary'}, 'attempt-8 failure set differs')
        return {'artifactsVerified': 107, 'indexUnchangedSha256': sha((OLD/'evidence.json').read_bytes()),
                'historicalFingerprint': old['sha256'], 'historicalSourceEqualsCurrent': old == current,
                'historicalVerifierExit': load(OUT/'historical-verifier.json')['exitCode'],
                'historicalVerifierStatus': 'failed at verify.py:52; retained, not renamed a pass',
                'attempt8CheckerExit': 1, 'attempt8Failures': retained_failures}

    def collection():
        r = load(OUT/'collection-binding.json')
        require((OUT/'report-payload.md').is_file(), 'collection prerequisite: report-payload.md is absent')
        require((ROOT/REPORT).is_file(), 'collection prerequisite: original receiving report is absent: '+REPORT)
        payload = (OUT/'report-payload.md').read_bytes()
        original = (ROOT/REPORT).read_bytes()
        require(len(payload) == 8417 and sha(payload) == REPORT_SHA and payload == original == (OLD/'delivery.md').read_bytes(), 'original/payload/report mismatch')
        require(r['bytes'] == 8417 and r['sha256'] == REPORT_SHA and r['originalDestination'] == REPORT, 'collection binding differs')
        p = subprocess.run(['git', 'show', '6cab57f62ebeb0ae38ee9609c974f15816390abc:'+REPORT], cwd=ROOT, capture_output=True)
        require(p.returncode == 128, 'historical tree omission changed')
        local = {'payloadBytes': len(payload), 'sha256': sha(payload), 'originalReceivingReportPresent': True,
                 'originalAndPayloadByteIdentical': True, 'historicalDeliveryTreeOmitsOriginal': True,
                 'receivingCollectionVerified': False, 'status': 'pending actual supplied changeSet and delivery-tree binding'}
        if args.receiving_delivery:
            d = load(Path(args.receiving_delivery))
            changes = {x['path']: x for x in d['changes']}
            bindings = []
            for path in [str((OUT/'report-payload.md').relative_to(ROOT)), str((OUT/'collection-binding.json').relative_to(ROOT))]:
                require(path in changes and changes[path]['after'] is not None, 'collection prerequisite: supplied changeSet omits '+path)
                raw = (ROOT/path).read_bytes()
                require(git_oid('blob', raw) == changes[path]['after']['oid'], 'supplied changeSet blob differs: '+path)
                received = git('show', d['deliveryTree']+':'+path)
                require(received == raw, 'receiving delivery-tree blob differs: '+path)
                bindings.append({'tree': d['deliveryTree'], 'path': path, 'blob': changes[path]['after']['oid'], 'bytes': len(raw), 'sha256': sha(raw)})
            local.update(receivingCollectionVerified=True, status='verified against actual supplied receiving delivery', observedBindings=bindings)
        return local

    def attribution():
        old = load(OLD/'final-source.json')
        previous = {x['path']: x for x in old['files']}
        actual = {x['path']: x for x in current['files']}
        diffs = [{'path': p, 'before': previous.get(p), 'after': actual.get(p)} for p in sorted(previous.keys() | actual.keys()) if previous.get(p) != actual.get(p)]
        require({x['path'] for x in diffs} == {'crates/arc-daemon/src/lifecycle.rs', 'crates/arc-daemon/src/database_failure_tests.rs'}, 'unexplained source differences')
        integration = load(BASE/'integration/evidence.json')
        require(integration['sourceHashes'] == current['files'] and integration['sourceFingerprint'] == current['sha256'], 'task-4 current source binding')
        count = verify_artifacts(integration['artifacts'])
        require(count == 303 and set(integration['changedProductPaths']) == {x['path'] for x in diffs}, 'task-4 ownership index')
        task6 = load(BASE/'task-6/attempt-4/artifacts.json')['files']
        count6 = verify_artifacts([{'path': p, **row} for p, row in task6.items()])
        proof6 = delivery_proof(OUT/'task6-delivery.json', 'aee07c312f488891b288f6fa1e06973417b414cc')
        require(sha((OUT/'task6-changes.patch').read_bytes()) == load(OUT/'task6-delivery.json')['patchSha256'], 'task-6 patch hash')
        require({x['path'] for x in proof6['blobs']} == set(task6) | {'.arc-verify/bounded-ipc/task-6/attempt-4/artifacts.json'}, 'task-6 delivery/index set')
        source6 = load(BASE/'task-6/attempt-4/source-evidence.json')
        require(source6 == load(OUT/'task6-delivery.json')['workspace'], 'task-6 provenance mismatch')
        require(load(BASE/'task-6/attempt-4/source-before.json') == {x['path']: {k: x[k] for k in ['sha256', 'mode']} for x in current['files']}, 'task-6 source inputs')
        proof7 = delivery_proof(OUT/'task7-delivery.json', '200679950bc1e49ebcdbf14df1407bcf0458eaf2')
        follow7 = load(OUT/'task7-followup-delivery.json')
        require(follow7['changes'] == [], 'task-7 followup is not unchanged')
        require(git('show', follow7['deliveryTree']+':docs/bounded-ipc-value-scope.md') == (ROOT/'docs/bounded-ipc-value-scope.md').read_bytes(), 'task-7 accepted followup document')
        owner9 = load(OUT/'task9-owner-checks.json')
        require(owner9['task'] == 'task-9' and owner9['reportSha256'] == sha((ROOT/'docs/bounded-ipc-final.md').read_bytes()), 'task-9 document ownership')
        require(git('show', '0192ccb29570654d2a5408162ad262deafdcd63d:docs/bounded-ipc-final.md') == (ROOT/'docs/bounded-ipc-final.md').read_bytes(), 'task-9 retained snapshot document')
        boundary = []
        for item in load(OLD/'before-boundary.json')['files']:
            m = meta(ROOT/item['path'])
            if any(m[k] != item[k] for k in ['sha256', 'mode']):
                boundary.append({'path': item['path'], 'before': item, 'after': m})
        owners = {'crates/arc-daemon/src/lifecycle.rs': 'task-4 classification repair',
                  'crates/arc-daemon/tests/resident.rs': 'task-5 original owned tests, matches historical final-source',
                  'crates/arc-daemon/tests/support/mod.rs': 'task-5 original owned tests, matches historical final-source',
                  'docs/bounded-ipc-final.md': 'task-9 historical report; must refresh after current task-8 QA',
                  'docs/bounded-ipc-value-scope.md': 'task-7 accepted delivery', **{p: 'task-6 accepted attempt-4 delivery' for p in DOCS}}
        require({x['path'] for x in boundary} == set(owners), 'unexplained historical boundary change')
        for row in boundary:
            row['ownerEvidence'] = owners[row['path']]
            if row['path'].endswith(('resident.rs', 'support/mod.rs')):
                require(actual[row['path']] == previous[row['path']], 'original test bytes not retained')
        before8 = load(A8/'before.json')
        modified8 = []
        for item in before8['files']:
            m = meta(ROOT/item['path'])
            if m != {k: item[k] for k in m}:
                modified8.append({'path': item['path'], 'before': item, 'after': m})
        require({x['path'] for x in modified8} == DOCS, 'unexplained attempt-8 snapshot modification')
        proof8 = delivery_proof(OUT/'attempt8-delivery.json', '6cab57f62ebeb0ae38ee9609c974f15816390abc')
        original_set = {x['path'] for x in before8['files']+before8['unreadable']}
        start_set = {x['path'] for x in load(OUT/'before.json')['files']+load(OUT/'before.json')['unreadable']}
        added = start_set-original_set
        expected_added = {REPORT} | {x['path'] for x in proof8['blobs']} | {x['path'] for x in proof6['blobs'] if x['path'] not in DOCS}
        require(added == expected_added and original_set <= start_set, 'unexplained attempt-8 snapshot additions/deletions')
        return {'sourceDifferences': diffs, 'sourceOwner': 'task-4 accepted classification repair', 'task4ArtifactsVerified': count,
                'historicalBoundaryDifferences': boundary, 'attempt8SnapshotModified': modified8,
                'attempt8SnapshotAdded': [{'path': p, 'owner': 'task-6 attempt-4' if p.startswith('.arc-verify/bounded-ipc/task-6/attempt-4/') else 'task-5 attempt-8 collection evidence', **meta(ROOT/p)} for p in sorted(added)],
                'task6IndexedArtifactsVerified': count6, 'task6Delivery': proof6, 'task7Delivery': proof7, 'attempt8Delivery': proof8,
                'task9': {'reportSha256': owner9['reportSha256'], 'historicalSourceFingerprint': owner9['sourceFingerprint'], 'currentAcceptanceClaimed': False}, 'unexplainedDifferences': []}

    def boundary():
        before = load(OUT/'before.json')
        for row in before['files']:
            require(meta(ROOT/row['path']) == {k: row[k] for k in ['kind', 'bytes', 'sha256', 'mode']}, 'changed recovery input: '+row['path'])
        require({x['path'] for x in before['unreadable']} == {'av.yaml'}, 'unexpected unreadable inputs')
        initial = {x['path'] for x in before['files']+before['unreadable']}
        actual = paths()
        require(initial <= actual, 'recovery deleted existing path')
        prefix = OUT.relative_to(ROOT).as_posix()+'/'
        added = actual-initial
        require(all(p.startswith(prefix) for p in added), 'unowned recovery addition')
        return {'unchangedInputs': len(before['files']), 'unreadableInput': 'av.yaml', 'addedPaths': sorted(added), 'productEdits': []}

    def sources():
        require(current['sha256'] == EXPECTED and len(current['files']) == 51, 'unexpected current fingerprint')
        sys.path.insert(0, str(OLD))
        spec = importlib.util.spec_from_file_location('original_verifier', OLD/'verify.py')
        original = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(original)
        require(original.record.record.fingerprint() == current, 'original algorithm reproduction')
        require(load(OUT/'focused-source.json') == load(OUT/'focused-source-after.json') == current, 'fresh current input binding')
        require(load(OUT/'current-source.json') == current, 'published current-source manifest differs')
        frozen = {}
        for p in ['crates/arc-storage/migrations/0001_phase0.sql', 'scripts/harness/validate.sh']:
            raw = (ROOT/p).read_bytes()
            require(raw == git('show', ACCEPTED+':'+p), 'accepted HEAD preservation: '+p)
            frozen[p] = {'sha256': sha(raw), 'bytes': len(raw), 'acceptedHeadByteIdentical': True}
        return {'fingerprint': current['sha256'], 'files': len(current['files']), 'originalAlgorithmReproduced': True, 'frozen': frozen}

    def acceptance():
        gates = []
        f4 = BASE/'integration/repair-2026-11-07-attempt-7'
        for name, argv, code in [('fmt', ['cargo', 'fmt', '--all', '--', '--check'], 0),
                                 ('clippy', ['cargo', 'clippy', '--workspace', '--all-targets', '--', '-D', 'warnings'], 0),
                                 ('workspace-test', ['cargo', 'test', '--workspace'], 0),
                                 ('harness', ['./scripts/harness/validate.sh'], 1)]:
            gates.append(receipt_check(f4, name, argv, code, current))
        text = (f4/'workspace-test.txt').read_text()
        rows = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out', text)
        require(sum(int(r[0]) for r in rows) == 183 and len(rows) == 17 and all(all(int(x) == 0 for x in r[1:]) for r in rows), 'task-4 workspace totals')
        fresh = receipt_check(OUT, 'focused', ['cargo', 'test', '-p', 'arc-daemon', '--test', 'bounded', '--test', 'worker_runtime', '--', '--nocapture'], 0, current)
        rows = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out', (OUT/'focused.txt').read_text())
        require(len(rows) == 2 and sum(int(r[0]) for r in rows) == 56 and all(all(int(x) == 0 for x in r[1:]) for r in rows), 'focused suite totals')
        raw = OUT/'focused-raw'
        reports = {}
        for p in sorted(raw.glob('*.json')):
            d = load(p)
            for key in ['report', 'restart_report']:
                if key in d:
                    reports[p.name+':'+key] = check_report(d[key])
        attempts = {}
        for name, admitted in [('connection', 8), ('request', 4), ('db', 9)]:
            d = load(raw/(name+'-100000.json'))
            require(d['attempted'] == 100000 and d['admitted'] == admitted and d['rejected']+admitted == 100000, 'attempt reconciliation: '+name)
            terminal = sum(d.get(k, 0) for k in ['completed', 'timed_out', 'shutdown_cancelled', 'released_without_dispatch', 'client_cancelled'])
            require(terminal == admitted, 'terminal reconciliation: '+name)
            bounds = check_gauges(d['final'], d['final']['caps'])
            require(d['final']['handlers_spawned'] == d['final']['handlers_reaped'], 'load handler accounting')
            require(d['final']['worker_started'] == d['final']['worker_joined'] == (1 if name == 'db' else 0), 'load worker accounting')
            attempts[name] = {k: v for k, v in d.items() if k != 'final'}
            attempts[name]['bounds'] = bounds
        signals = {}
        for name in ['SIGINT', 'SIGTERM']:
            d = load(raw/(name+'.json'))
            require(d['signal_to_exit_us'] <= 5000000 and d['reopen_integrity'] == 'ok' and d['report']['reason'] == name, 'signal lifecycle: '+name)
            require(d['report']['metrics']['db_outstanding']['high_water'] == 3 and d['report']['metrics']['tracked_tasks']['high_water'] == 8, 'saturated signal conditions')
            signals[name] = {'signalToReapedExitUs': d['signal_to_exit_us'], 'handlersReaped': d['report']['metrics']['handlers_reaped'], 'workerJoined': True, 'restartIntegrity': 'ok'}
        require(load(raw/'nonreader.json')['report']['metrics']['write_timeouts'] == 1, 'native write backpressure not observed')
        require(load(raw/'native-sql-health.json')['health_cli_us'] < 500000, 'health waited on slow SQL')
        h = (f4/'harness.txt').read_text()
        for fragment in ['901', '551', 'test:coverage']:
            require(fragment in h, 'missing retained harness finding: '+fragment)
        return {'retainedTask4Gates': gates, 'retainedWorkspacePassedInvocations': 183, 'freshFocused': fresh, 'freshPassedInvocations': 56,
                'attempts': attempts, 'signals': signals, 'reportsChecked': len(reports), 'reportBounds': reports,
                'harnessFailuresRetained': ['resident.rs 901 lines', 'arc-ipc/src/lib.rs 551 lines', 'missing test:coverage'],
                'freshExecution': 'focused suite only; task-4 exact Cargo/harness gates reused after full source/receipt binding; independent task-8 remains required'}

    def performance():
        raw = OUT/'focused-raw'
        d = load(raw/'native-measurements.json')
        probe = load(raw/'native-probe.json')
        require(probe['attempts'] == probe['completed'] == 200 and probe['rss_bytes'] > 0 and probe['threads'] > 0, 'small native probe')
        obs, samples = load(raw/'native-observations.json'), load(raw/'small-message-samples.json')
        require(len(obs['idle']) == 100 and len(obs['load']) > 0, 'native observation counts')
        require(len(samples['samples_ns']) == 10000 and samples['failed'] == 0, 'latency samples')
        require(sorted(samples['samples_ns'])[9499] == d['small_message']['p95_ns'], 'nearest-rank P95 arithmetic')
        idle = max(x['rss_bytes'] for x in obs['idle'])
        threads = max(x['threads'] for x in obs['idle']+obs['load'])
        require(idle == d['idle']['max_rss_bytes'] and threads == d['sampled_native_thread_peak'] and d['lifetime_peak_rss_bytes'] > 0, 'native metrics arithmetic')
        require(d['attempts'] == {'attempted': 100000, 'admitted': 100000, 'completed': 100000, 'rejected': 0, 'timed_out': 0, 'shutdown_cancelled': 0}, 'native workload reconciliation')
        srs = load(OLD/'preservation-targets.json')['srs']
        source = Path(srs['path']).read_bytes()
        require(sha(source) == srs['sha256'] == d['srs']['source_sha256'], 'authoritative SRS hash/access')
        lines = source.decode().splitlines()
        for item in srs['requirements']:
            require(lines[item['line']-1] == item['text'], 'authoritative SRS requirement line')
        require(idle < 50000000 and d['small_message']['p95_ns'] <= 20000000, 'SRS target not met')
        require(d['report']['profiles']['ipc'] == 'default', 'native default configuration')
        return {'sourceFingerprint': current['sha256'], 'nativeMeasurementsRecorded': True, 'srsComparisonComplete': True,
                'srsSource': srs, 'idleRssBytes': idle, 'nativeLifetimePeakRssBytes': d['lifetime_peak_rss_bytes'],
                'sampledNativeThreadPeak': threads, 'trackedTaskHighWater': d['report']['metrics']['tracked_tasks']['high_water'],
                'p95Ns': d['small_message']['p95_ns'], 'afterLoadP95Ns': d['after_load_p95_ns'],
                'nativeSamples': {'idle': len(obs['idle']), 'load': len(obs['load']), 'latency': 10000},
                'host': d['host'], 'structuralCountersArePerformanceMeasurements': False}

    for name, fn in [('historicalArtifactIntegrity', historical), ('actualCollectionBinding', collection),
                     ('ownerAttributedIntegrationChanges', attribution), ('recoveryWriteBoundary', boundary),
                     ('currentProductInputs', sources), ('currentAcceptanceEvidence', acceptance), ('currentNativeMeasurements', performance)]:
        run(name, fn)
    def published_manifests():
        for filename, domain in [('attribution.json', 'ownerAttributedIntegrationChanges'), ('performance.json', 'currentNativeMeasurements'), ('acceptance.json', 'currentAcceptanceEvidence')]:
            require(load(OUT/filename) == results[domain], 'published result differs from live verification: '+filename)
        index = OUT/'evidence.json'
        require(index.is_file(), 'supplemental artifact index is absent')
        count = verify_artifacts(load(index)['artifacts'])
        return {'supplementalArtifactsVerified': count, 'publishedManifestsMatchLiveChecks': True}
    run('publishedRecoveryArtifacts', published_manifests)
    pending = not results.get('actualCollectionBinding', {}).get('receivingCollectionVerified', False)
    return {'status': 'failed' if failures else ('pending-receiving-collection' if pending else 'verified-receiving-checkout'),
            'results': results, 'failures': failures, 'receivingCollectionPending': pending,
            'independentTechnicalDirectorReviewComplete': False, 'currentTask8IndependentQAComplete': False,
            'task9RefreshComplete': False, 'configuredTargetApplicabilityVerified': False, 'missionCompletionClaimed': False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--receiving-delivery', help='Actual AV receiving delivery.json containing supplied changes and deliveryTree; only after normal collection')
    answer = main(parser.parse_args())
    print(json.dumps(answer, indent=2, ensure_ascii=False))
    sys.exit(1 if answer['failures'] else (2 if answer['receivingCollectionPending'] else 0))
