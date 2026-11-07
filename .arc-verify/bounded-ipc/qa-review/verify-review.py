import hashlib,json,pathlib,sys
sys.dont_write_bytecode=True
import run
ROOT,OUT=run.ROOT,run.OUT
read=lambda p:json.loads(p.read_text())
index=read(OUT/"evidence.json")
for item in index["artifacts"]:
 p=ROOT/item["path"]
 assert p.is_file() and p.stat().st_size==item["bytes"] and run.sha(p.read_bytes())==item["sha256"],item["path"]
assert run.fingerprint()==read(OUT/"context.json")["source"]
for n,c in [("fmt",0),("clippy",0),("workspace",0),("harness",1),("inherited-receipts",0)]:
 r=read(OUT/(n+".json")); assert r["exitCode"]==c and not r["timedOut"] and r["productUnchanged"]
 assert r["sourceBefore"]==r["sourceAfter"]==index["sourceFingerprint"]
 assert run.sha((OUT/(n+".txt")).read_bytes())==r["outputSha256"]
qa=read(OUT.parent/"qa.json"); perf=read(OUT.parent/"performance.json")
for k in ["behavioralContractVerified","compatibilityVerified","migrationUnchanged","cargoFmtPassed","cargoClippyPassed","cargoTestsPassed","harnessExecuted","harnessUnchanged","unresolvedHarnessFailuresRetained"]: assert qa[k] is True,k
assert qa["harnessPassed"] is False and qa["harnessExitCode"]==1 and len(qa["findings"])==3
assert perf["nativeMeasurementsRecorded"] and perf["srsComparisonComplete"]
assert len(read(OUT/"workspace-raw/small-message-samples.json")["samples_ns"])==10000
assert sorted(read(OUT/"workspace-raw/small-message-samples.json")["samples_ns"])[9499]==perf["small_message"]["p95_ns"]
assert (OUT.parent/"harness.txt").read_bytes()==(OUT/"harness.txt").read_bytes()
print(json.dumps({"status":"verified","artifacts":len(index["artifacts"]),"sourceFiles":len(run.fingerprint()["files"]),"sourceFingerprint":index["sourceFingerprint"],"harnessExit":1,"remaining":"harness conformance; av.yaml/container access; Phase 0/Linux/Windows; Chief final report"}))
