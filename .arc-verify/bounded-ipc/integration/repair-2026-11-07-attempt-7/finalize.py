"""Refresh task-4 delivery only; preserve earlier reports, receipts and indices."""
import datetime
import difflib
import hashlib
import json
from pathlib import Path
import re
import subprocess

from record import fingerprint

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[3]
BASE = OUT.parent
ACCEPTED = "42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0"
OWNED = ["crates/arc-daemon/src/database_failure_tests.rs", "crates/arc-daemon/src/lifecycle.rs"]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


current = fingerprint()
initial = json.loads((OUT / "execution-context.json").read_text())["productFingerprint"]
before = {r["path"]: r for r in initial["files"]}
after = {r["path"]: r for r in current["files"]}
changed = sorted(p for p in before.keys() | after.keys() if before.get(p) != after.get(p))
assert changed == OWNED, changed
baseline = {}
for path in ["crates/arc-storage/migrations/0001_phase0.sql", "scripts/harness/validate.sh"]:
    original = subprocess.check_output(["git", "show", ACCEPTED + ":" + path], cwd=ROOT)
    assert original == (ROOT / path).read_bytes(), path
    baseline[path] = {"sha256": sha(original), "bytes": len(original), "byteIdentical": True}
save(OUT / "boundary-check.json", {"acceptedBaselineFiles": baseline,
     "changedProductPaths": changed, "allOtherProductInputsPreserved": True,
     "initialFingerprint": initial["sha256"], "finalFingerprint": current["sha256"]})
old = (OUT / "lifecycle-before.rs.txt").read_text().splitlines(keepends=True)
new = (ROOT / OWNED[1]).read_text().splitlines(keepends=True)
(OUT / "lifecycle-change.patch").write_text("".join(difflib.unified_diff(old, new, fromfile="incoming/lifecycle.rs", tofile="repaired/lifecycle.rs")))
save(OUT / "current-source.json", current)
tests = (OUT / "workspace-test.txt").read_text()
counts = [int(n) for n in re.findall(r"test result: ok\. (\d+) passed;", tests)]
signals = []
for line in (OUT / "lifecycle.txt").read_text().splitlines():
    if line.startswith("integration-signal-receipt: "):
        signals.append(json.loads(line.removeprefix("integration-signal-receipt: ")))
save(OUT / "acceptance-summary.json", {"workspacePassedInvocations": sum(counts),
     "workspaceTestBatchPassedCounts": counts, "newRegressionTests": 5,
     "focusedAdapterTestsPassed": 6, "existingLifecycleTestsPassed": 13,
     "signals": signals, "migrationUnchanged": True, "harnessUnchanged": True,
     "classificationPolicy": "Only SQLITE_CORRUPT and SQLITE_NOTADB indicate corruption; other typed errors are unavailable. Integrity/schema report validation is unchanged.",
     "missionAcceptanceClaimed": False})
cli = ROOT / "target/debug/arcctl"
save(OUT / "cli-binary-binding.json", {"path": str(cli), "sha256": sha(cli.read_bytes()),
     "bytes": cli.stat().st_size, "source": after["crates/arc-cli/src/main.rs"],
     "prerequisiteReceipt": "cli-prerequisite.json", "note": "Offline CLI test setup; actual process invoked by the typed-error fixture; binary is not a delivery artifact."})
checks = {}
expected_exits = {"cli-prerequisite": 0, "regression-red": 101, "actual-cli-red": 101,
                  "format-owned": 0, "regression-green": 0, "lifecycle": 0,
                  "fmt": 0, "clippy": 0, "workspace-test": 0, "harness": 1,
                  "nonignored-deliverables": 0}
for name, expected_exit in expected_exits.items():
    receipt = json.loads((OUT / (name + ".json")).read_text())
    assert receipt["exitCode"] == expected_exit, name
    checks[name] = {"argv": receipt["argv"], "exitCode": receipt["exitCode"],
                    "receipt": str((OUT / (name + ".json")).relative_to(ROOT))}
claim = {"status": "completed", "scope": "Task-4 classification repair; independent review pending",
         "changedFiles": OWNED, "artifacts": [str((BASE / "delivery.md").relative_to(ROOT)), str((BASE / "evidence.json").relative_to(ROOT))],
         "skillsUsed": ["oma-backend", "oma-db"], "unresolved": [
             "Independent technical-director review and current-tree task-8/task-9 acceptance remain open.",
             "Unchanged harness exits 1: two oversized files and missing test:coverage.",
             "Task-6 collection, configured-target applicability and remaining Phase 0/Linux/Windows obligations remain with their owners."],
         "identity": {"actorId": "backend", "taskId": "task-4", "attempt": 7, "injectedClaimPath": None, "omaRunId": None}}
save(OUT / "claim.json", claim)
index = {"version": 4, "recordedAt": datetime.datetime.now(datetime.timezone.utc).isoformat(),
         "actorId": "backend", "taskId": "task-4", "attempt": 7,
         "status": "ready-for-independent-technical-director-review", "skillsUsed": ["oma-backend", "oma-db"],
         "baselineHead": ACCEPTED, "actualHead": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT).decode().strip(),
         "sourceWorkspace": str(ROOT), "receiptScope": "Direct receiving mission checkout; no new commits, refs or mission-state changes",
         "sourceFileCount": len(current["files"]), "sourceFingerprint": current["sha256"],
         "sourceHashes": current["files"], "changedProductPaths": changed, "checks": checks,
         "currentTreeChecks": ["regression-green", "lifecycle", "fmt", "clippy", "workspace-test", "harness"],
         "previousIndex": str((OUT / "previous-evidence.json").relative_to(ROOT)),
         "authoritativeReportChannel": "ARC_RUNTIME_REPORT", "reportMaxBytes": 4096,
         "migrationUnchanged": True, "harnessUnchanged": True, "harnessExitCode": 1,
         "unresolvedHarnessFailures": ["resident.rs: 901 lines", "arc-ipc/src/lib.rs: 551 lines", "missing test:coverage"],
         "independentReviewComplete": False, "postCollectionVerified": False, "missionCompletionClaimed": False}
index["artifacts"] = [{"path": str(p.relative_to(ROOT)), "sha256": sha(p.read_bytes()), "bytes": p.stat().st_size}
                      for p in sorted(BASE.rglob("*")) if p.is_file() and p != BASE / "evidence.json" and "__pycache__" not in p.parts]
save(BASE / "evidence.json", index)
print(json.dumps({"artifactsIndexed": len(index["artifacts"]), "sourceFiles": len(current["files"]), "fingerprint": current["sha256"], "changedProductPaths": changed}))
