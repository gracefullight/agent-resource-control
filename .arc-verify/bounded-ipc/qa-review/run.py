import datetime, hashlib, json, os, pathlib, resource, signal, subprocess, sys, time
ROOT=pathlib.Path(__file__).resolve().parents[3]
OUT=pathlib.Path(__file__).resolve().parent
ENV=dict(os.environ,CARGO_NET_OFFLINE="true")
def now(): return datetime.datetime.now(datetime.timezone.utc).isoformat()
def sha(b): return hashlib.sha256(b).hexdigest()
def save(n,v): (OUT/n).write_text(json.dumps(v,indent=2)+"\n")
def fingerprint():
 paths=subprocess.check_output(["git","ls-files","--cached","--others","--exclude-standard","-z","--","crates","Cargo.toml","Cargo.lock","rust-toolchain.toml"],cwd=ROOT).decode().split("\0")
 files=[{"path":p,"sha256":sha((ROOT/p).read_bytes()),"mode":oct((ROOT/p).stat().st_mode&0o777)} for p in sorted(set(paths)-{""})]
 return {"sha256":sha(json.dumps(files,sort_keys=True).encode()),"files":files}
def run(name,args):
 before=fingerprint(); save(name+"-source.json",before)
 started=now(); t=time.monotonic(); env=dict(ENV,ARC_TEST_EVIDENCE_DIR=str(OUT/(name+"-raw")))
 timeout=False
 with (OUT/(name+".txt")).open("wb") as log:
  p=subprocess.Popen(args,cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
  save(name+"-running.json",{"pid":p.pid,"processGroup":p.pid,"argv":args,"startedAt":started,"timeoutSeconds":120})
  try: code=p.wait(timeout=120)
  except subprocess.TimeoutExpired:
   timeout=True; os.killpg(p.pid,signal.SIGTERM)
   try: code=p.wait(timeout=5)
   except subprocess.TimeoutExpired: os.killpg(p.pid,signal.SIGKILL); code=p.wait(timeout=5)
 after=fingerprint()
 r={"argv":args,"cwd":str(ROOT),"startedAt":started,"finishedAt":now(),"elapsedSeconds":time.monotonic()-t,"exitCode":code,"timedOut":timeout,"sourceBefore":before["sha256"],"sourceAfter":after["sha256"],"productUnchanged":before==after,"outputSha256":sha((OUT/(name+".txt")).read_bytes()),"pid":p.pid,"processExitObserved":True,"environment":{"CARGO_NET_OFFLINE":"true","ARC_TEST_EVIDENCE_DIR":env["ARC_TEST_EVIDENCE_DIR"]}}
 save(name+".json",r); print(json.dumps(r)); print((OUT/(name+".txt")).read_text()[-2000:]); return code
if __name__=="__main__":
 if sys.argv[1]=="context":
  results=[]
  for args in [["git","rev-parse","HEAD","HEAD^{tree}"],["rustc","-Vv"],["cargo","-Vv"],["uname","-a"],["sysctl","-n","hw.memsize","hw.ncpu","machdep.cpu.brand_string"]]:
   r=subprocess.run(args,cwd=ROOT,env=ENV,capture_output=True); results.append({"argv":args,"exitCode":r.returncode,"stdout":r.stdout.decode(),"stderr":r.stderr.decode()})
  save("context.json",{"recordedAt":now(),"tools":results,"identity":{"actor":"reviewer","task":"task-8","attempt":1,"session":os.environ.get("CODEX_SESSION_ID"),"injectedClaimPath":None,"omaRunId":None},"budget":{"missionMaxDurationSec":86400,"missionMaxRuns":200,"missionMaxRetries":3,"missionMaxParallel":3,"parallelCommands":1,"commandTimeoutSec":120,"missionStartOrDeadlineNotInjected":True,"note":"No mission limits reset; no delegation, installs, standalone builds or mission-state writes."},"hostLimits":{k:resource.getrlimit(getattr(resource,k)) for k in ["RLIMIT_NOFILE","RLIMIT_NPROC","RLIMIT_AS","RLIMIT_CPU"]},"source":fingerprint()})
 else: sys.exit(run(sys.argv[1],sys.argv[2:]))
