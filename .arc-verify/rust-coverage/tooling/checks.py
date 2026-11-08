import subprocess, json, time, hashlib
from pathlib import Path
root=Path.cwd()
commands=[
 ('coverage',['bun','run','test:coverage']),
 ('coverage-cli',['bun','test','scripts/coverage/coverage-cli.test.ts']),
 ('harness',['bun','test','scripts/coverage/harness.test.ts']),
 ('evidence',['bun','test','scripts/coverage/evidence.test.ts']),
 ('fmt',['cargo','fmt','--all','--','--check']),
 ('clippy',['cargo','clippy','--workspace','--all-targets','--','-D','warnings']),
 ('workspace-tests',['cargo','test','--workspace']),
]
for label,cmd in commands:
 start=time.time()
 proc=subprocess.run(cmd,capture_output=True,text=True,timeout=120)
 data=dict(command=cmd,cwd=str(root),started=start,ended=time.time(),exit=proc.returncode,stdout=proc.stdout,stderr=proc.stderr)
 Path(f'.arc-verify/rust-coverage/tooling/{label}.json').write_text(json.dumps(data,indent=2)+'\n')
 print(label,proc.returncode,flush=True)
