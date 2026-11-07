"""Serial, source-bound execution of explicitly assigned checks; no product edits."""
import datetime
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import time

sys.dont_write_bytecode = True
OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[3]
spec = importlib.util.spec_from_file_location('retained_record', OUT.parent / 'record.py')
record = importlib.util.module_from_spec(spec)
spec.loader.exec_module(record)
record.ROOT = ROOT
record.OUT = OUT
record.ENV['CARGO_TARGET_DIR'] = str(ROOT / 'target')
DEADLINE = datetime.datetime.fromisoformat('2026-11-08T10:52:35.601+00:00')


def main():
    name, argv = sys.argv[1], sys.argv[2:]
    now = datetime.datetime.now(datetime.timezone.utc)
    if now >= DEADLINE:
        raise SystemExit('Saved mission deadline exhausted; no new verification admitted')
    allowed = {
        'focused': ['cargo', 'test', '-p', 'arc-daemon', '--test', 'bounded', '--test', 'worker_runtime', '--', '--nocapture'],
        'historical-verifier': ['python3', '-B', '.arc-verify/bounded-ipc/implementation/resume-attempt-3/verify.py'],
        'attempt8-checker': ['python3', '-B', '.arc-verify/bounded-ipc/implementation/collection-repair/attempt-8/check.py'],
    }
    assert allowed.get(name) == argv
    assert not (OUT / (name + '.json')).exists(), 'Do not replace any execution receipt'
    record.save(name + '-admission.json', {'at': now.isoformat(), 'remainingSeconds': (DEADLINE-now).total_seconds(),
        'cancellationObserved': False, 'basis': 'No cancellation instruction received; no exposed cancellation tool; no mission state mutated',
        'parallelCommands': 1, 'savedLimitsUnchanged': True})
    status = record.gate(name, argv)
    record.save(name + '-source-after.json', record.fingerprint())
    raise SystemExit(status)


if __name__ == '__main__':
    main()
