#!/usr/bin/env python3
"""Independent task-5 checks. Writes only assigned evidence paths."""
import datetime, hashlib, json, os, pathlib, platform, resource, socket, subprocess, time
ROOT = pathlib.Path.cwd()
OUT = ROOT / '.arc-verify/command-classification/verification'
BASE = '05f0812d0841485bbe07b3f1f8f6fde3861a1d55'
def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()
def digest(data):
    return hashlib.sha256(data).hexdigest()
def write(name, value):
    (OUT / name).write_text(json.dumps(value, indent=2) + '\n')
def inventory():
    paths = subprocess.check_output(['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard', '--', 'crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.cargo', 'scripts/harness']).decode().split('\0')
    paths += ['.arc-verify/command-classification/' + p for p in ['contract.md', 'contract.json', 'cli-requirements.md']]
    rows = []
    tracked = set(subprocess.check_output(['git', 'ls-files', '-z', '--', 'crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.cargo', 'scripts/harness']).decode().split('\0'))
    for name in sorted(set(paths) - {''}):
        path = ROOT / name
        if not path.is_file():
            rows.append({'path': name, 'missing': True})
            continue
        data = path.read_bytes()
        rows.append({'path': name, 'sha256': digest(data), 'bytes': len(data), 'lines': len(data.splitlines()), 'tracked': name in tracked, 'mode': oct(path.stat().st_mode & 0o777)})
    return rows
CONTEXT = {'uid': os.getuid(), 'euid': os.geteuid(), 'gid': os.getgid(), 'pid': os.getpid(), 'platform': platform.platform(), 'machine': platform.machine(), 'transport': 'supported noninteractive exec_command; no GUI or permission bypass', 'environmentOverrides': {'CARGO_NET_OFFLINE': 'true'}, 'resources': {k: list(resource.getrlimit(v)) for k, v in [('cpu', resource.RLIMIT_CPU), ('addressSpace', resource.RLIMIT_AS), ('openFiles', resource.RLIMIT_NOFILE)]}, 'parallelChecks': 1, 'perCommandTimeoutSeconds': 120, 'missionBudget': 'supplied maxDurationSec=86400; maxRuns=200; no dispatch or retry performed'}
initial = inventory()
write('source-inputs-before.json', initial)
write('context.json', dict(CONTEXT, cwd=str(ROOT), startedAt=now(), baselineHead=BASE, actualHead=subprocess.check_output(['git', 'rev-parse', 'HEAD']).decode().strip()))
probe = {'at': now(), 'context': CONTEXT, 'operation': 'socket.bind/listen 127.0.0.1:0; close owned socket', 'operatorNote': '2026-11-07T20:28:55.793Z supersedes prior TCP denial'}
try:
    with socket.socket() as s:
        s.bind(('127.0.0.1', 0))
        s.listen(1)
        probe.update(permitted=True, address=list(s.getsockname()))
except OSError as exc:
    probe.update(permitted=False, errno=exc.errno, error=str(exc))
write('tcp-prerequisite.json', probe)
checks = []
if probe['permitted']:
    checks.append(('privacy', ['cargo', 'test', '-p', 'arc-daemon', '--test', 'classification', 'privacy::required_loopback_endpoint_nonexecution_and_privacy', '--', '--exact']))
checks += [('fmt', ['cargo', 'fmt', '--all', '--', '--check']), ('clippy', ['cargo', 'clippy', '--workspace', '--all-targets', '--', '-D', 'warnings']), ('workspace-tests', ['cargo', 'test', '--workspace']), ('harness', ['./scripts/harness/validate.sh'])]
for name, argv in checks:
    before = inventory()
    started = now()
    tick = time.monotonic()
    env = os.environ.copy()
    env['CARGO_NET_OFFLINE'] = 'true'
    env['ARC_TEST_EVIDENCE_DIR'] = str(OUT / 'runtime' / name)
    log = OUT / (name + '.txt')
    timed_out = False
    with log.open('wb') as output:
        try:
            result = subprocess.run(argv, cwd=ROOT, env=env, stdout=output, stderr=subprocess.STDOUT, timeout=120)
            code = result.returncode
        except subprocess.TimeoutExpired:
            code, timed_out = None, True
    after = inventory()
    receipt = {'command': ' '.join(argv), 'argv': argv, 'cwd': str(ROOT), 'executionContext': CONTEXT, 'startedAt': started, 'finishedAt': now(), 'elapsedSeconds': time.monotonic()-tick, 'exitCode': code, 'timedOut': timed_out, 'log': str(log.relative_to(ROOT)), 'logSha256': digest(log.read_bytes()), 'inputsBefore': before, 'inputsAfter': after, 'sourceInputsMatch': before == after == initial, 'runtimeEvidenceDirectory': str((OUT / 'runtime' / name).relative_to(ROOT))}
    if name == 'harness':
        receipt['coverageStatus'] = 'unresolved' if b'test:coverage' in log.read_bytes() and code != 0 else 'not_established'
    write(name + '.json', receipt)
    print(json.dumps({k: receipt[k] for k in ['command', 'exitCode', 'elapsedSeconds', 'sourceInputsMatch']}), flush=True)
write('source-inputs-after.json', inventory())
