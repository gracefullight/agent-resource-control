"""Index finished task-4 artifacts without touching product or mission state."""
import datetime
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
sha = lambda data: hashlib.sha256(data).hexdigest()
changed = ["crates/arc-daemon/src/" + p for p in ["main.rs", "server.rs", "connection.rs", "lifecycle.rs", "lifecycle_tests.rs", "server_tests.rs"]] + ["crates/arc-ipc/src/lib.rs"]
report = ROOT / ".agents/results/result-backend-task-4-attempt-1-b137e253-def6-46ef-a75d-25ab414784ea.md"
report.parent.mkdir(parents=True, exist_ok=True)
report.write_bytes((OUT / "delivery.md").read_bytes())
claim = {"status":"completed", "changedFiles":changed,
    "unresolved":["Independent task review and mission-wide saturation/performance/final QA remain later tasks.", "Unchanged harness exits 1 with three retained violations."],
    "artifacts":[str((OUT/"delivery.md").relative_to(ROOT)), str((OUT/"evidence.json").relative_to(ROOT)), str(report.relative_to(ROOT))],
    "identity":{"actorId":"backend", "taskId":"task-4", "attempt":1, "injectedClaimPath":None, "omaRunId":None}}
(OUT / "claim.json").write_text(json.dumps(claim, indent=2) + "\n")
fp = json.loads((OUT / "fmt-final-source.json").read_text())
artifacts = [{"path":str(p.relative_to(ROOT)), "sha256":sha(p.read_bytes())} for p in sorted(OUT.iterdir()) if p.is_file() and p.name != "evidence.json"]
artifacts.append({"path":str(report.relative_to(ROOT)), "sha256":sha(report.read_bytes())})
evidence = {"version":1, "recordedAt":datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "status":"completed-assigned-task-ready-for-independent-review",
    "baselineHead":subprocess.check_output(["git","rev-parse","HEAD"], cwd=ROOT).decode().strip(),
    "skillsUsed":["oma-backend"], "changedProductPaths":changed,
    "sourceHashes":[{"path":p, "sha256":sha((ROOT/p).read_bytes())} for p in changed],
    "sourceFingerprint":fp["sha256"], "sourceFileCount":len(fp["files"]),
    "finalChecks":["fmt-final","clippy-final","workspace-test","lifecycle-final","harness"],
    "migrationUnchanged":True, "databaseOwnedFilesUnchanged":True, "sharedManifestsUnchanged":True,
    "independentReviewComplete":False,
    "harness":{"exitCode":1, "unchanged":True, "unresolved":["resident.rs:901 lines", "arc-ipc/src/lib.rs:551 lines", "missing test:coverage script"]},
    "remaining":["Task-5 mission-wide saturation/recovery/adversarial/native performance fixtures", "Independent oma-qa and final mission evidence", "Remaining Phase 0, Linux qualification and Windows runtime obligations"],
    "artifacts":artifacts, "artifactIndexScope":"All task-4 integration files except evidence.json itself, plus the supplemental result report."}
(OUT / "evidence.json").write_text(json.dumps(evidence, indent=2) + "\n")
print(json.dumps({"sourceFileCount":len(fp["files"]), "sourceFingerprint":fp["sha256"], "artifactCount":len(artifacts), "changedProductPaths":changed}))
