"""Refresh only task-4's evidence index; never edits product or mission state."""
import datetime
import hashlib
import json
from pathlib import Path
import re
import subprocess
import record

ROOT, OUT = record.ROOT, record.OUT
INDEX = OUT.parent / "evidence.json"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def save(name, value):
    (OUT / name).write_text(json.dumps(value, indent=2) + "\n")


def collect():
    before = json.loads((OUT / "before-boundary.json").read_text())
    for item in before:
        path = ROOT / item["path"]
        assert sha(path.read_bytes()) == item["sha256"], item["path"]
        assert oct(path.stat().st_mode & 0o777) == item["mode"], item["path"]
    save("boundary-check.json", {"productFilesChanged": [],
         "checkedFiles": len(before), "sourceDocsHarnessUnchanged": True})
    fingerprint = record.fingerprint()
    checks = {}
    for name in ("focused", "fmt", "clippy", "workspace-test", "harness", "diff-check"):
        receipt = json.loads((OUT / (name + ".json")).read_text())
        assert receipt["exitCode"] == (1 if name == "harness" else 0), name
        assert receipt["processExitObserved"] and not receipt["timedOut"], name
        assert receipt["sourceBefore"] == receipt["sourceAfter"] == fingerprint["sha256"], name
        assert sha((OUT / receipt["output"]).read_bytes()) == receipt["outputSha256"], name
        checks[name] = {"receipt": str((OUT / (name + ".json")).relative_to(ROOT)),
                        "argv": receipt["argv"], "exitCode": receipt["exitCode"]}
    previous = json.loads((OUT / "previous-evidence.json").read_text())
    historical = []
    replacements = {str((OUT.parent / name).relative_to(ROOT)):
                    str((OUT / ("previous-" + name)).relative_to(ROOT))
                    for name in ("delivery.md", "claim.json", "evidence.json")}
    for item in previous["sourceHashes"] + previous["artifacts"]:
        retained = dict(item)
        retained["path"] = replacements.get(item["path"], item["path"])
        if retained["path"] != item["path"]:
            retained["originalPath"] = item["path"]
        assert sha((ROOT / retained["path"]).read_bytes()) == item["sha256"], item["path"]
        historical.append(retained)
    paths = {p for p in OUT.iterdir() if p.is_file() and p.name not in
             ("collection-check.json", "collection-check.txt")}
    paths.update(OUT.parent / name for name in ("delivery.md", "claim.json"))
    result = ROOT / ".agents/results/result-backend-task-4-attempt-4-b137e253-def6-46ef-a75d-25ab414784ea.md"
    paths.add(result)
    artifacts = [{"path": str(p.relative_to(ROOT)), "sha256": sha(p.read_bytes()),
                  "bytes": p.stat().st_size} for p in sorted(paths)]
    evidence = {"version": 2, "recordedAt": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "status": "ready-for-independent-task-review", "actorId": "backend", "taskId": "task-4",
        "attempt": 4, "skillsUsed": ["oma-backend"],
        "baselineHead": "42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0",
        "changedProductPaths": [], "sourceFileCount": len(fingerprint["files"]),
        "sourceFingerprint": fingerprint["sha256"], "sourceHashes": fingerprint["files"],
        "finalChecks": checks, "historicalArtifacts": historical,
        "authoritativeReportChannel": "ARC_RUNTIME_REPORT", "reportMaxBytes": 4096,
        "stdoutSubstitutionSuperseded": True, "migrationUnchanged": True,
        "harnessUnchanged": True, "harnessExitCode": 1,
        "unresolvedHarnessFailures": ["resident.rs: 901 lines", "arc-ipc/src/lib.rs: 551 lines",
                                      "missing test:coverage"],
        "independentReviewComplete": False, "postCollectionVerified": False,
        "artifacts": artifacts}
    INDEX.write_text(json.dumps(evidence, indent=2) + "\n")
    print(json.dumps({"sourceFiles": evidence["sourceFileCount"],
                      "artifacts": len(artifacts), "historicalEntries": len(historical),
                      "sourceFingerprint": evidence["sourceFingerprint"]}))


if __name__ == "__main__":
    collect()
