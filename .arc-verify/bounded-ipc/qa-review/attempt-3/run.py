"""Execute assigned gates serially; preserve all preceding review receipts."""
import datetime
import importlib.util
import json
import os
from pathlib import Path
import resource
import sys

sys.dont_write_bytecode = True
OUT = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('retained_runner', OUT.parent / 'run.py')
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)
runner.OUT = OUT
DEADLINE = datetime.datetime.fromisoformat('2026-11-08T10:52:35.601+00:00')
ALLOWED = {
    'fmt': ['cargo', 'fmt', '--all', '--', '--check'],
    'clippy': ['cargo', 'clippy', '--workspace', '--all-targets', '--', '-D', 'warnings'],
    'workspace': ['cargo', 'test', '--workspace'],
    'harness': ['./scripts/harness/validate.sh'],
}

def main():
    now = datetime.datetime.now(datetime.timezone.utc)
    if now >= DEADLINE:
        raise SystemExit('Saved mission deadline exhausted; no check admitted')
    name = sys.argv[1]
    if name == 'context':
        assert not (OUT / 'context.json').exists()
        runner.save('context.json', {
            'recordedAt': now.isoformat(), 'actor': 'reviewer', 'task': 'task-8',
            'attempt': 3, 'workspace': str(runner.ROOT),
            'session': os.environ.get('CODEX_SESSION_ID'), 'injectedClaimPath': None,
            'skill': 'oma-qa', 'source': runner.fingerprint(),
            'budget': {'savedDeadline': DEADLINE.isoformat(), 'deadlineEvidence':
                '.arc-verify/bounded-ipc/implementation/recovery-current/run.py',
                'remainingSecondsAtAdmission': (DEADLINE-now).total_seconds(),
                'missionMaxDurationSec': 86400, 'missionMaxRuns': 200,
                'missionMaxRetries': 3, 'missionMaxParallel': 3, 'parallelCommands': 1,
                'commandTimeoutSec': 120, 'savedLimitsUnchanged': True},
            'cancellation': 'No cancellation received; no exposed mission cancellation API; mission state untouched',
            'hostLimits': {k: resource.getrlimit(getattr(resource,k)) for k in
                ['RLIMIT_NOFILE','RLIMIT_NPROC','RLIMIT_AS','RLIMIT_CPU']},
            'codeIntelligence': {'provider': 'Serena', 'mode': 'unavailable',
                'fallback': 'Known-path reads, Git diffs and plain documentation only'},
        })
        print(json.dumps({'source': runner.fingerprint()['sha256'],
                          'files': len(runner.fingerprint()['files'])}))
        return 0
    argv = sys.argv[2:]
    assert ALLOWED.get(name) == argv, 'Exact assigned command required'
    assert not (OUT / (name+'.json')).exists(), 'Preserve execution receipts'
    runner.save(name+'-admission.json', {'at': now.isoformat(),
        'remainingSeconds': (DEADLINE-now).total_seconds(),
        'cancellationObserved': False, 'savedLimitsUnchanged': True})
    return runner.run(name, argv)

if __name__ == '__main__':
    raise SystemExit(main())
