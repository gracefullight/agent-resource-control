#!/usr/bin/env python3
"""Retain stdout suppressed by the default successful workspace test run."""
import datetime, hashlib, json, os, pathlib, subprocess, time
root = pathlib.Path.cwd()
out = root / '.arc-verify/command-classification/verification'
def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()
def snapshot():
    rows = json.loads((out / 'source-inputs-before.json').read_text())
    for row in rows:
        data = (root / row['path']).read_bytes()
        row.update(sha256=hashlib.sha256(data).hexdigest(), bytes=len(data), lines=len(data.splitlines()), mode=oct((root / row['path']).stat().st_mode & 0o777))
    return rows
argv = ['cargo', 'test', '-p', 'arc-detector', 'tests::production_work_exhaustion_trace', '--', '--exact', '--nocapture']
before, started, tick = snapshot(), now(), time.monotonic()
env = os.environ.copy()
env['CARGO_NET_OFFLINE'] = 'true'
with (out / 'work-trace.txt').open('wb') as log:
    result = subprocess.run(argv, cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=120)
after = snapshot()
receipt = dict(command=' '.join(argv), argv=argv, cwd=str(root), startedAt=started, finishedAt=now(), elapsedSeconds=time.monotonic()-tick, exitCode=result.returncode, timedOut=False, executionContext=json.loads((out / 'context.json').read_text()), inputsBefore=before, inputsAfter=after, sourceInputsMatch=before == after == json.loads((out / 'source-inputs-before.json').read_text()), log='.arc-verify/command-classification/verification/work-trace.txt', logSha256=hashlib.sha256((out / 'work-trace.txt').read_bytes()).hexdigest(), rationale='Retain the existing production counter trace hidden by default successful-test output; no source/test edits.')
(out / 'work-trace.json').write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps({'exitCode':result.returncode,'sourceInputsMatch':receipt['sourceInputsMatch']}))
print((out / 'work-trace.txt').read_text())
