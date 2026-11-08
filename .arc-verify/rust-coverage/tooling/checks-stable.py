import subprocess,json,time
from pathlib import Path
commands=[
 ('coverage-stable',['bun','run','test:coverage']),
 ('coverage-cli-stable',['bun','test','scripts/coverage/coverage-cli.test.ts']),
 ('harness-stable',['bun','test','scripts/coverage/harness.test.ts']),
 ('evidence-stable',['bun','test','scripts/coverage/evidence.test.ts']),
]
for label,cmd in commands:
 start=time.time();p=subprocess.run(cmd,capture_output=True,text=True,timeout=120)
 data=dict(command=cmd,cwd=str(Path.cwd()),started=start,ended=time.time(),exit=p.returncode,stdout=p.stdout,stderr=p.stderr)
 Path(f'.arc-verify/rust-coverage/tooling/{label}.json').write_text(json.dumps(data,indent=2)+'\n')
 print(label,p.returncode,flush=True)
