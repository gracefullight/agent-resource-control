import importlib.util,sys,pathlib,json,os,resource,datetime
sys.dont_write_bytecode=True
p=pathlib.Path(__file__).resolve()
s=importlib.util.spec_from_file_location("prior_runner",p.parent.parent/"run.py")
r=importlib.util.module_from_spec(s);s.loader.exec_module(r);r.OUT=p.parent
if sys.argv[1]=="context":
 r.save("context.json",{"recordedAt":r.now(),"actor":"reviewer","task":"task-8","attempt":2,"session":os.environ.get("CODEX_SESSION_ID"),"injectedClaimPath":None,"source":r.fingerprint(),"budget":{"missionMaxDurationSec":86400,"missionMaxRuns":200,"missionMaxRetries":3,"missionMaxParallel":3,"parallelCommands":1,"commandTimeoutSec":120,"deadlineNotInjected":True,"note":"Retain spent mission limits; no mission state mutated; no installs or standalone builds."},"hostLimits":{k:resource.getrlimit(getattr(resource,k)) for k in ["RLIMIT_NOFILE","RLIMIT_NPROC","RLIMIT_AS","RLIMIT_CPU"]}})
 print(json.dumps({"source":r.fingerprint()["sha256"],"files":len(r.fingerprint()["files"])}))
else:sys.exit(r.run(sys.argv[1],sys.argv[2:]))
