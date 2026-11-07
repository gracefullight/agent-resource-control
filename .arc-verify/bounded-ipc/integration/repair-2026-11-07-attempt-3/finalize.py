"""Collect the task-4 repair without Git/ref/mission-state mutations."""
import datetime
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[4]
REPAIR = Path(__file__).resolve().parent
OUT = REPAIR.parent
sha = lambda b: hashlib.sha256(b).hexdigest()
owned = ["crates/arc-daemon/src/" + p for p in [
    "main.rs", "lifecycle.rs", "lifecycle_tests.rs", "accept_recovery.rs",
    "accept_recovery_tests.rs", "runtime_report.rs", "runtime_report_tests.rs", "report_lifecycle_tests.rs"]]
cumulative = sorted(set(owned + [x["path"] for x in json.loads((REPAIR / "previous-evidence.json").read_text())["sourceHashes"]]))
report = ROOT / ".agents/results/result-backend-task-4-attempt-3-b137e253-def6-46ef-a75d-25ab414784ea.md"
report.parent.mkdir(parents=True, exist_ok=True)
report.write_bytes((OUT / "delivery.md").read_bytes())
claim = {
    "status": "completed", "changedFiles": owned,
    "unresolved": ["Independent task review required before downstream execution.",
        "Unchanged harness exits 1 with three retained failures.",
        "Tasks 5-9 and all mission-wide saturation/native measurement/final QA/platform obligations remain open."],
    "artifacts": [str((OUT / "delivery.md").relative_to(ROOT)), str((OUT / "evidence.json").relative_to(ROOT)), str(report.relative_to(ROOT))],
    "identity": {"actorId":"backend", "taskId":"task-4", "attempt":3, "injectedClaimPath":None, "omaRunId":None},
}
(OUT / "claim.json").write_text(json.dumps(claim, indent=2) + "\n")
checks = ["focused-final", "fmt-final", "clippy-final", "workspace-test", "harness", "diff-check"]
fp = json.loads((REPAIR / "fmt-final-source.json").read_text())
assert len(fp["files"]) == 43
artifacts = [{"path":str(p.relative_to(ROOT)), "sha256":sha(p.read_bytes()), "bytes":p.stat().st_size}
    for p in sorted(OUT.rglob("*")) if p.is_file() and p != OUT / "evidence.json" and p != REPAIR / "collection-check.json"]
artifacts.append({"path":str(report.relative_to(ROOT)), "sha256":sha(report.read_bytes()), "bytes":report.stat().st_size})
evidence = {
    "version":2, "recordedAt":datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "status":"completed-assigned-repair-ready-for-independent-review",
    "baselineHead":subprocess.check_output(["git","rev-parse","HEAD"], cwd=ROOT).decode().strip(),
    "skillsUsed":["oma-backend"], "changedProductPaths":owned, "cumulativeTask4Paths":cumulative,
    "sourceHashes":[{"path":p,"sha256":sha((ROOT/p).read_bytes())} for p in cumulative],
    "sourceFingerprint":fp["sha256"], "sourceFileCount":len(fp["files"]),
    "receiptDirectory":str(REPAIR.relative_to(ROOT)), "finalChecks":checks,
    "migrationUnchanged":True,"databaseOwnedFilesUnchanged":True,"sharedManifestsUnchanged":True,
    "authoritativeReportChannel":"ARC_RUNTIME_REPORT", "reportMaxBytes":4096,
    "stdoutSubstitutionSuperseded":True,"independentReviewComplete":False,
    "harness":{"exitCode":1,"unchanged":True,"unresolved":["resident.rs:901 lines","arc-ipc/src/lib.rs:551 lines","missing test:coverage script"]},
    "remaining":claim["unresolved"], "artifacts":artifacts,
    "artifactIndexScope":"All integration files recursively except this index and derived collection-check.json, plus the supplemental result report.",
    "historicalEvidence":"Original delivery/index/claim copied to previous-* under the dated repair directory; all original command receipts/logs remain untouched.",
}
(OUT / "evidence.json").write_text(json.dumps(evidence, indent=2) + "\n")
print(json.dumps({"files":len(fp["files"]),"artifacts":len(artifacts),"fingerprint":fp["sha256"]}))
