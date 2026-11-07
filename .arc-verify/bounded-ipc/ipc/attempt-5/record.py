"""Fresh task-2 evidence only; do not mutate product files or refs."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import resource
import subprocess
import sys
import time

ROOT = Path.cwd()
OUT = ROOT / '.arc-verify/bounded-ipc/ipc/attempt-5'
IPC = OUT.parent
REVIEW = ROOT.parents[3]
BASE = '42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0'
OLD = '3816bafaa684c8099dad78be2d0cb0e474c5f08b'
SNAP = 'd2004475a01cb395753340977cc9b1b29003f6cd'
FILES = ['crates/arc-daemon/' + p for p in (
    'src/admission.rs', 'src/connection.rs', 'src/limits.rs', 'src/platform.rs',
    'src/runtime_metrics.rs', 'src/transport.rs', 'src/transport_test_support.rs',
    'src/transport_tests.rs', 'tests/transport_unit.rs')]
INPUTS = ['crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
          'scripts/harness/validate.sh', 'docs/bounded-ipc-contract.md',
          'docs/bounded-ipc-targets.md']
ENV = dict(os.environ, CARGO_NET_OFFLINE='true', CARGO_TARGET_DIR=str(ROOT / 'target'))

def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()

def sha(data):
    return hashlib.sha256(data).hexdigest()

def save(name, value):
    (OUT / name).write_text(json.dumps(value, indent=2) + '\n')

def command(argv, cwd=ROOT, timeout=20):
    start = now()
    try:
        p = subprocess.run(argv, cwd=cwd, env=ENV, capture_output=True, timeout=timeout)
        return dict(argv=argv, cwd=str(cwd), startedAt=start, finishedAt=now(),
                    exitCode=p.returncode, stdout=p.stdout.decode(errors='replace'),
                    stderr=p.stderr.decode(errors='replace'), pty=False)
    except subprocess.TimeoutExpired as e:
        return dict(argv=argv, cwd=str(cwd), startedAt=start, finishedAt=now(),
                    exitCode=None, timedOut=True, stdout=(e.stdout or b'').decode(errors='replace'),
                    stderr=(e.stderr or b'').decode(errors='replace'), pty=False)

def fingerprint():
    r = command(['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z', '--', *INPUTS])
    assert r['exitCode'] == 0, r
    entries = []
    for name in sorted(set(r['stdout'].split('\0')) - {''}):
        p = ROOT / name
        entries.append(dict(path=name, sha256=sha(p.read_bytes()), mode=oct(p.stat().st_mode & 0o777)))
    return dict(scope=INPUTS, files=entries,
                sha256=sha(json.dumps(entries, sort_keys=True).encode()),
                algorithm='sha256 of sorted canonical JSON path/content/mode entries')

def prepare():
    for name in ['delivery.md', 'evidence.json']:
        (OUT / ('preceding-root-' + name)).write_bytes((IPC / name).read_bytes())
    previous = []
    for p in sorted(IPC.rglob('*')):
        if p.is_file() and OUT not in p.parents and '__pycache__' not in p.parts:
            previous.append(dict(path=p.relative_to(ROOT).as_posix(), sha256=sha(p.read_bytes())))
    original = (IPC / 'original-delivery.md').read_bytes()
    expected = {p:h for h,p in re.findall(r'^([a-f0-9]{64})  (crates/arc-daemon/[^\n]+)$', original.decode(), re.M)}
    hashes = [dict(path=p, sha256=sha((ROOT/p).read_bytes()), originalSha256=expected[p],
                   matchesOriginal=sha((ROOT/p).read_bytes()) == expected[p],
                   reviewSha256=sha((REVIEW/p).read_bytes()),
                   matchesReview=(ROOT/p).read_bytes() == (REVIEW/p).read_bytes()) for p in FILES]
    refs = [BASE, OLD, SNAP, 'HEAD'] + [f'refs/agent-valley/task-evidence/d6c6b45f4a6c3769-78686d2704d4d690-{n}/{suffix}' for n in (1,5) for suffix in ('snapshot','worker')]
    receipts = []
    for cwd in (ROOT, REVIEW):
        for ref in refs:
            receipts.append(command(['git','rev-parse','--verify',ref],cwd))
            receipts.append(command(['git','rev-parse','--verify',ref+'^{tree}'],cwd))
        for ref in (OLD,SNAP):
            receipts.append(command(['git','merge-base','--is-ancestor',BASE,ref],cwd))
        for ref in (BASE,OLD,'HEAD'):
            receipts.append(command(['git','ls-tree','-r',ref,'--',*FILES],cwd))
        receipts.append(command(['git','status','--porcelain=v1','--untracked-files=all'],cwd))
        receipts.append(command(['git','status','--porcelain=v1','--untracked-files=all','--',*INPUTS],cwd))
    preservation = []
    for p in ['crates/arc-storage/migrations/0001_phase0.sql','scripts/harness/validate.sh']:
        r=command(['git','show',BASE+':'+p]); assert r['exitCode']==0, r
        b=r['stdout'].encode(); current=(ROOT/p).read_bytes()
        preservation.append(dict(path=p, argv=r['argv'], baselineSha256=sha(b), currentSha256=sha(current), byteIdentical=b==current))
    save('provenance-before.json',dict(recordedAt=now(), baselineHead=BASE,
         baselineTree='c802f71ac4d91ed6b8a602d5c83bf9cf57116b76', originalSnapshot=OLD,
         suppliedSnapshot=SNAP, suppliedSnapshotTree='2ee8c1b555aceeaed149c7315e4feb32a8e79e5e',
         sourceHashes=hashes, gitReceipts=receipts, preservation=preservation,
         productFingerprint=fingerprint(), precedingArtifacts=previous,
         originalReportSha256=sha(original)))
    save('execution-context.json',dict(recordedAt=now(), actorId='backend',taskId='task-2',attempt=5,
         missionId='b137e253-def6-46ef-a75d-25ab414784ea', sessionId=os.getenv('CODEX_SESSION_ID'),
         omaRunId=None,injectedClaimPath=None,
         environment={k:ENV[k] for k in ['PATH','TMPDIR','CARGO_HOME','CARGO_TARGET_DIR','CARGO_NET_OFFLINE','RUSTUP_HOME','RUSTUP_TOOLCHAIN','RUSTFLAGS','AGENT_VALLEY_MANAGED_RUN'] if k in ENV},
         cachePaths=dict(cargoHome=ENV.get('CARGO_HOME',str(Path.home()/'.cargo')),target=ENV['CARGO_TARGET_DIR']),
         cacheChoice='Standard worktree-local target selected before checks; preceding attempt-4 retains shared-target permission failures. No download/install.',
         tools=[command(a) for a in [['rustc','-Vv'],['cargo','-Vv'],['rustup','show','active-toolchain'],['uname','-a']]],
         hostLimits={k:resource.getrlimit(getattr(resource,k)) for k in ['RLIMIT_NOFILE','RLIMIT_NPROC','RLIMIT_AS','RLIMIT_CPU']},
         budget=dict(maxDurationSec=86400,maxRuns=200,maxRetries=3,maxParallel=3,concurrentCargoCommands=1,commandTimeoutSec=120,
                     note='Inherited spent budgets preserved; exact mission start/deadline not injected. No new mission or retries initiated.')))
    save('review-environment-probe.json',dict(scope='Codex noninteractive route only; does not prove qwen runner recovery',receipt=command(['/bin/pwd'],REVIEW,10)))
    print(json.dumps(dict(prepared=True,sourceFingerprint=fingerprint()['sha256'],originalMatches=sum(h['matchesOriginal'] for h in hashes),reviewMatches=sum(h['matchesReview'] for h in hashes))))

def gate(name):
    argv={'fmt':['cargo','fmt','--all','--','--check'],
          'clippy':['cargo','clippy','--workspace','--all-targets','--','-D','warnings'],
          'workspace-test':['cargo','test','--workspace']}[name]
    before=fingerprint(); start=now(); clock=time.monotonic(); timedout=False
    with (OUT/(name+'.txt')).open('wb') as log:
        p=subprocess.Popen(argv,cwd=ROOT,env=ENV,stdout=log,stderr=subprocess.STDOUT)
        save(name+'.running.json',dict(argv=argv,cwd=str(ROOT),startedAt=start,pid=p.pid,pty=False))
        try: status=p.wait(timeout=120)
        except subprocess.TimeoutExpired:
            timedout=True; p.terminate()
            try: status=p.wait(timeout=5)
            except subprocess.TimeoutExpired: p.kill(); status=p.wait(timeout=5)
    after=fingerprint()
    r=dict(argv=argv,cwd=str(ROOT),startedAt=start,finishedAt=now(),elapsedSeconds=time.monotonic()-clock,
           exitCode=status,timedOut=timedout,pid=p.pid,processExitObserved=True,pty=False,
           context='execution-context.json',output=name+'.txt',outputIncludes='full combined stdout/stderr',
           outputSha256=sha((OUT/(name+'.txt')).read_bytes()),sourceBefore=before['sha256'],sourceAfter=after['sha256'],productInputsUnchanged=before==after)
    save(name+'.json',r); print(json.dumps(r))

if __name__=='__main__':
    if sys.argv[1]=='prepare': prepare()
    else: gate(sys.argv[1])
