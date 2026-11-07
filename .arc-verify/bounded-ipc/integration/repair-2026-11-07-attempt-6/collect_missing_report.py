"""Restore the exact task-4 artifact, then verify the receiving checkout.

No source, historical evidence, Git refs or mission state are changed.
"""
import datetime
import hashlib
import json
import os
from pathlib import Path
import resource
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
EVIDENCE = ROOT / '.arc-verify/bounded-ipc/integration/evidence.json'
REPORT = '.agents/results/result-backend-task-4-attempt-5-b137e253-def6-46ef-a75d-25ab414784ea.md'
EXPECTED_SHA = '8388a5252ff58f31a1fab8d6bdf5d4a4ba49b1b9fdbe1bb359e0a1e2369b0165'
DEADLINE = datetime.datetime.fromisoformat('2026-11-08T10:52:35.601+00:00')


def now():
    return datetime.datetime.now(datetime.timezone.utc)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def save(name, data):
    with (OUT / name).open('x') as handle:
        handle.write(json.dumps(data, indent=2) + '\n')


def snapshot(evidence):
    entries = evidence['sourceHashes'] + evidence['artifacts'] + evidence['historicalArtifacts']
    paths = sorted(set(x['path'] for x in entries) | {str(EVIDENCE.relative_to(ROOT))})
    return [{'path': p, 'sha256': sha((ROOT / p).read_bytes()),
             'mode': oct((ROOT / p).stat().st_mode & 0o777)}
            if (ROOT / p).exists() else {'path': p, 'missing': True} for p in paths]


def verify(name, evidence):
    argv = [sys.executable, '-B', '.arc-verify/bounded-ipc/integration/repair-2026-11-07-attempt-5/verify.py']
    started, clock = now(), time.monotonic()
    before = snapshot(evidence)
    result = subprocess.run(argv, cwd=ROOT, capture_output=True, timeout=60,
                            env=dict(os.environ, PYTHONDONTWRITEBYTECODE='1'))
    output = result.stdout + result.stderr
    with (OUT / (name + '.txt')).open('xb') as handle:
        handle.write(output)
    after = snapshot(evidence)
    save(name + '.json', {'argv': argv, 'cwd': str(ROOT),
         'startedAt': started.isoformat(), 'finishedAt': now().isoformat(),
         'elapsedSeconds': time.monotonic() - clock, 'exitCode': result.returncode,
         'processExitObserved': True, 'timeoutSeconds': 60, 'timedOut': False,
         'environment': {'PYTHONDONTWRITEBYTECODE': '1',
                         'AGENT_VALLEY_MANAGED_RUN': os.environ.get('AGENT_VALLEY_MANAGED_RUN')},
         'python': sys.version, 'output': name + '.txt', 'outputSha256': sha(output),
         'inputSnapshotBeforeSha256': sha(json.dumps(before, sort_keys=True).encode()),
         'inputSnapshotAfterSha256': sha(json.dumps(after, sort_keys=True).encode()),
         'inputsUnchanged': before == after})
    assert before == after, 'Verifier unexpectedly changed inputs'
    print(name, result.returncode, output.decode(errors='replace'))
    return result


def main():
    assert now() < DEADLINE, 'Saved mission deadline exhausted'
    evidence = json.loads(EVIDENCE.read_text())
    save('preflight.json', {'recordedAt': now().isoformat(), 'savedDeadline': DEADLINE.isoformat(),
         'remainingSeconds': (DEADLINE - now()).total_seconds(), 'taskAttempt': 6,
         'maxParallel': 3, 'maxRuns': 200, 'recordedMissionRuns': 99, 'maxRetries': 3,
         'route': 'Already executing supplied codex/native route; no dispatch probe needed.',
         'scope': 'Exact artifact collection and read-only verification; no product replay.',
         'serenaAvailable': False, 'omaRunId': None, 'injectedClaimPath': None,
         'hostLimits': {k: resource.getrlimit(getattr(resource, k)) for k in
                        ('RLIMIT_NOFILE', 'RLIMIT_NPROC', 'RLIMIT_AS', 'RLIMIT_CPU')},
         'budgetNote': 'Supplied mission counts and retained deadline; no resets or mission-state writes.'})
    before = snapshot(evidence)
    save('before-boundary.json', before)
    assert not (ROOT / REPORT).exists(), 'Do not overwrite a receiving artifact'
    failed = verify('collection-before', evidence)
    assert failed.returncode != 0 and REPORT.encode() in failed.stderr
    source = Path(evidence['sourceWorkspace']) / REPORT
    data = source.read_bytes()
    assert len(data) == 6665 and sha(data) == EXPECTED_SHA
    with (ROOT / REPORT).open('xb') as handle:
        handle.write(data)
    # Retain a nonignored delivery copy so generic collectors can recover this
    # generated result even when their ordinary Git tree collection excludes it.
    with (OUT / 'result-backend-task-4-attempt-5.md').open('xb') as handle:
        handle.write(data)
    save('restored-artifact.json', {'source': str(source), 'destination': REPORT,
         'bytes': len(data), 'sha256': sha(data), 'exactCopy': True,
         'deliveryCopy': str((OUT / 'result-backend-task-4-attempt-5.md').relative_to(ROOT)),
         'recordedAt': now().isoformat()})
    passed = verify('collection-after', evidence)
    assert passed.returncode == 0, 'Retain failure and report without modifying historical evidence'
    after = snapshot(evidence)
    save('after-boundary.json', after)
    differences = [{'before': a, 'after': b} for a, b in zip(before, after) if a != b]
    assert len(differences) == 1 and differences[0]['after']['path'] == REPORT
    save('boundary-check.json', {'onlyRestoredPath': REPORT, 'differences': differences,
         'sourceFilesUnchanged': True, 'historicalReceiptsAndIndexUnchanged': True,
         'migrationVerifiedAgainstAcceptedHead': True, 'harnessVerifiedAgainstAcceptedHead': True,
         'collectionVerifiedInReceivingCheckout': True, 'independentReviewClaimed': False,
         'missionCompletionClaimed': False})


if __name__ == '__main__':
    main()
