"""Review-only controlled subprocess probes of byte-identical coverage scripts.

Fake tool executables exercise collection failures; they are not Rust coverage
measurements. Every fixture owns its root, old pointer, reports and processes.
"""
from pathlib import Path
import datetime
import hashlib
import json
import os
import shutil
import subprocess
import sys

ROOT = Path.cwd()
QA = ROOT / '.arc-verify/rust-coverage/qa'
CRATES = ['arc-core', 'arc-detector', 'arc-storage', 'arc-ipc', 'arc-daemon', 'arc-cli']
STUB = r'''
from pathlib import Path
import json, os, sys, time
root=Path(os.environ['PROBE_ROOT']); mode=os.environ['PROBE_MODE']
args=sys.argv[1:]; name=Path(sys.argv[0]).name
if name in ['llvm-cov','llvm-profdata']:
 print('LLVM version 22.1.2'); sys.exit(0)
if name in ['arcctl','arc-daemon']:
 if mode=='bad-child-exit': sys.exit(8)
 if mode!='missing-child-profile':
  p=Path(os.environ['LLVM_PROFILE_FILE'].replace('%p',str(os.getpid())).replace('%m','123'))
  p.write_bytes(b'' if mode=='empty-child-profile' else b'controlled-profile')
 sys.exit(0 if name=='arcctl' else 1)
assert args[:2]==['run','1.95.0']; args=args[2:]
if args==['rustc','-Vv']:
 print('rustc 1.95.0\ncommit-hash: fixture\nhost: aarch64-apple-darwin\nrelease: 1.95.0\nLLVM version: 22.1.2');sys.exit(0)
if args==['rustc','--print','sysroot']: print(root/'sysroot');sys.exit(0)
if args==['cargo','llvm-cov','--version']:
 print('cargo-llvm-cov '+('0.0.0' if mode=='wrong-tool-version' else '0.6.21'));sys.exit(0)
if args[:2]==['cargo','metadata']:
 crates=['arc-core','arc-detector','arc-storage','arc-ipc','arc-daemon','arc-cli']
 print(json.dumps({'workspace_members':crates,'packages':[{'id':c,'name':c,'manifest_path':str(root/'crates'/c/'Cargo.toml')} for c in crates]}));sys.exit(0)
target=Path(os.environ['CARGO_LLVM_COV_TARGET_DIR'])
if '--no-report' in args:
 if mode=='failed-tests': sys.exit(23)
 if mode=='changed-config': (root/'mise.toml').write_text('# changed during collection\n')
 (target/'debug').mkdir(parents=True,exist_ok=True)
 for binary in ['arcctl','arc-daemon']:
  out=target/'debug'/binary;out.write_text(Path(__file__).read_text());out.chmod(0o755)
 sys.exit(0)
assert args[:3]==['cargo','llvm-cov','report']
if mode=='failed-export': sys.exit(24)
profiles=sorted(target.glob('*.profraw'))
if mode!='missing-merge-list':
 (target/'workspace-profraw-list').write_text('\n'.join(str(p) for p in (profiles[:-1] if mode=='merge-mismatch' else profiles))+'\n')
out=Path(args[args.index('--output-path')+1]);native='--json' in args
if mode==('missing-json' if native else 'missing-lcov'):sys.exit(0)
files=sorted((root/'crates').glob('*/src/lib.rs'));records=[];data=[]
for i,p in enumerate(files):
 hits=[1,1,1,0,0] if mode=='one-crate-below' and i==0 else [1,1,1,1,0]
 records.append('SF:'+str(p)+'\n'+''.join(f'DA:{n},{h}\n' for n,h in enumerate(hits,1))+f'LF:5\nLH:{sum(hits)}\nend_of_record\n')
 segments=[]
 for n,h in enumerate(hits,1):segments.extend([[n,1,h,True,True,False],[n,2,0,False,False,False]])
 data.append({'filename':str(p),'summary':{'lines':{'count':5,'covered':sum(hits)}},'segments':segments})
out.write_text(json.dumps({'type':'llvm.coverage.json.export','version':'3.1.0','data':[{'files':data}]}) if native else ''.join(records))
if mode==('stale-json' if native else 'stale-lcov'):os.utime(out,(1,1))
if mode==('malformed-json' if native else 'malformed-lcov'):out.write_text('malformed')
'''


def main():
    out = []
    cases = {
        'positive-80': None,
        'failed-tests': 'TEST_FAILURE: exited 23',
        'wrong-tool-version': 'TOOL_VERSION',
        'missing-tool': 'MISSING_TOOL',
        'bad-child-exit': 'CHILD_FAILURE arcctl: exited 8',
        'missing-child-profile': 'CHILD_PROFILE: no profile',
        'empty-child-profile': 'CHILD_PROFILE: empty',
        'missing-merge-list': 'CHILD_PROFILE: missing merge input list',
        'merge-mismatch': 'CHILD_PROFILE: merge input mismatch',
        'failed-export': 'REPORT_FAILURE: exited 24',
        'missing-lcov': 'MISSING_REPORT',
        'missing-json': 'MISSING_REPORT',
        'stale-lcov': 'STALE_REPORT',
        'stale-json': 'STALE_REPORT',
        'changed-config': 'STALE_REPORT: source/configuration changed',
        'one-crate-below': 'BELOW_THRESHOLD',
        'malformed-lcov': 'MALFORMED_REPORT',
        'malformed-json': 'JSON Parse error',
    }
    scripts = ROOT / 'scripts/coverage'
    for mode, diagnostic in cases.items():
        root = QA / 'fixtures' / mode
        dest = root / 'scripts/coverage'
        dest.mkdir(parents=True, exist_ok=True)
        copies = {}
        for src in scripts.iterdir():
            if src.is_file():
                shutil.copyfile(src, dest / src.name)
                copies[src.name] = hashlib.sha256(src.read_bytes()).hexdigest()
                assert (dest / src.name).read_bytes() == src.read_bytes()
        for c in CRATES:
            p = root / 'crates' / c / 'src'
            p.mkdir(parents=True, exist_ok=True)
            (p / 'lib.rs').write_text('pub fn fixture() {\n let a = 1;\n let b = 2;\n let _ = a+b;\n}\n')
        for p in ['Cargo.toml', 'Cargo.lock', 'mise.toml', 'rust-toolchain.toml', 'package.json']:
            (root / p).write_text('{}\n' if p == 'package.json' else '# controlled fixture\n')
        evidence = root / '.arc-verify/rust-coverage'
        (evidence / 'contract').mkdir(parents=True, exist_ok=True)
        (evidence / 'contract/accounting.json').write_text('{}\n')
        (evidence / 'baseline').mkdir(exist_ok=True)
        old = {'pass': True, 'runId': 'old-passing-report', 'directory': 'old-passing-report'}
        (evidence / 'baseline/latest.json').write_text(json.dumps(old))
        binary_dir = root / 'bin'
        binary_dir.mkdir(exist_ok=True)
        stub = '#!' + sys.executable + '\n' + STUB
        for p in [binary_dir / 'rustup', *[root / 'sysroot/lib/rustlib/aarch64-apple-darwin/bin' / n for n in ['llvm-cov', 'llvm-profdata']]]:
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(stub)
            p.chmod(0o755)
        env = dict(os.environ, PATH=str(binary_dir) + ':' + os.environ['PATH'], PROBE_ROOT=str(root), PROBE_MODE=mode)
        for k in ['RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'LLVM_COV_FLAGS', 'LLVM_PROFDATA_FLAGS', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'CARGO_BUILD_TARGET']:
            env.pop(k, None)
        argv = [shutil.which('bun'), str(dest / 'run.ts')]
        # Establish a complete genuine passing fixture run in this same root,
        # rather than merely creating a pointer that claims a prior pass.
        prior = subprocess.run(argv, cwd=root, env={**env, 'PROBE_MODE':'positive-80'}, capture_output=True, text=True, timeout=30)
        assert prior.returncode == 0, prior.stderr
        old = json.loads((evidence / 'baseline/latest.json').read_text())
        assert old['pass'] and (root / old['directory'] / 'summary.json').exists()
        if mode == 'missing-tool':
            (binary_dir / 'rustup').write_text('#!' + sys.executable + '\nimport sys;sys.exit(127)\n')
        start = datetime.datetime.now(datetime.timezone.utc).isoformat()
        result = subprocess.run(argv, cwd=root, env=env, capture_output=True, text=True, timeout=30)
        latest = json.loads((evidence / 'baseline/latest.json').read_text())
        passed = result.returncode == 0 if diagnostic is None else result.returncode != 0 and diagnostic in result.stderr
        assert latest['runId'] != old['runId']
        assert latest['pass'] == (diagnostic is None)
        record = dict(case=mode, argv=argv, cwd=str(root), started=start, ended=datetime.datetime.now(datetime.timezone.utc).isoformat(), exit=result.returncode, stdout=result.stdout, stderr=result.stderr, expectedDiagnostic=diagnostic, assertionsPass=passed, latest=latest, olderPassingRun=old, olderPassingExit=prior.returncode, olderPassingStdout=prior.stdout, copiedScripts=copies, synthetic=True)
        (QA / 'receipts' / f'driver-complete-prior-{mode}.json').write_text(json.dumps(record, indent=2) + '\n')
        out.append({k: record[k] for k in ['case', 'exit', 'expectedDiagnostic', 'assertionsPass', 'synthetic']})
        print(mode, result.returncode, passed)
        assert passed, result.stderr
    (QA / 'driver-probes.json').write_text(json.dumps(out, indent=2) + '\n')


if __name__ == '__main__':
    main()
