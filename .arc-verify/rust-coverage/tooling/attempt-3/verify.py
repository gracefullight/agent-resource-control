from pathlib import Path
import subprocess, json, time, datetime, os
out=Path(__file__).parent
checks=[
 ("coverage",["bun","run","test:coverage"]),
 ("coverage-cli",["bun","test","scripts/coverage/coverage-cli.test.ts"]),
 ("fmt",["cargo","fmt","--all","--","--check"]),
 ("clippy",["cargo","clippy","--workspace","--all-targets","--","-D","warnings"]),
 ("schema",["cargo","test","-p","arc-cli","--test","schema"]),
 ("workspace-tests",["cargo","test","--workspace"]),
 ("harness",["bun","test","scripts/coverage/harness.test.ts"]),
 ("evidence",["bun","test","scripts/coverage/evidence.test.ts"]),
]
env={**os.environ,"RUSTUP_TOOLCHAIN":"1.95.0","CARGO_NET_OFFLINE":"true"}
for name,argv in checks:
 t=time.monotonic(); started=datetime.datetime.now(datetime.timezone.utc).isoformat()
 try:
  p=subprocess.run(argv,capture_output=True,text=True,env=env,timeout=120)
  r={"exit":p.returncode,"stdout":p.stdout,"stderr":p.stderr}
 except subprocess.TimeoutExpired as e:
  r={"exit":124,"stdout":str(e.stdout),"stderr":str(e.stderr),"timedOut":True}
 except OSError as e: r={"exit":127,"stdout":"","stderr":str(e)}
 r.update(argv=argv,cwd=str(Path.cwd()),started=started,elapsedSec=time.monotonic()-t,environment={"RUSTUP_TOOLCHAIN":"1.95.0","CARGO_NET_OFFLINE":"true"})
 (out/(name+".json")).write_text(json.dumps(r,indent=2)+"\n")
 print(json.dumps({"name":name,"exit":r["exit"],"elapsedSec":r["elapsedSec"],"tail":(r["stdout"]+r["stderr"])[-1200:]}),flush=True)
