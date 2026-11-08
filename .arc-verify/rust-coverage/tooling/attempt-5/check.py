from pathlib import Path
import subprocess,json,time,datetime,sys,os
out=Path(__file__).parent
name=sys.argv[1];argv=sys.argv[2:]
t=time.monotonic();started=datetime.datetime.now(datetime.timezone.utc).isoformat()
env={**os.environ,"RUSTUP_TOOLCHAIN":"1.95.0","CARGO_NET_OFFLINE":"true"}
try:
 p=subprocess.run(argv,capture_output=True,text=True,env=env,timeout=120)
 r=dict(exit=p.returncode,stdout=p.stdout,stderr=p.stderr)
except subprocess.TimeoutExpired as e:r=dict(exit=124,stdout=str(e.stdout),stderr=str(e.stderr),timedOut=True)
except OSError as e:r=dict(exit=127,stdout="",stderr=str(e))
r.update(argv=argv,cwd=str(Path.cwd()),started=started,elapsedSec=time.monotonic()-t,environment={"RUSTUP_TOOLCHAIN":"1.95.0","CARGO_NET_OFFLINE":"true"})
(out/(name+".json")).write_text(json.dumps(r,indent=2)+"\n")
print(json.dumps(dict(name=name,exit=r["exit"],elapsedSec=r["elapsedSec"],tail=(r["stdout"]+r["stderr"])[-3500:])),flush=True)
