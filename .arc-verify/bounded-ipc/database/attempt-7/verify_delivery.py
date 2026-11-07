"""Read-only verification in the Actor or collected review checkout."""
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[4]
DB = ROOT / ".arc-verify/bounded-ipc/database"


def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()


def verify():
    evidence = json.loads((DB / "evidence.json").read_text())
    checked = []
    for item in evidence["sourceHashes"] + evidence["artifacts"]:
        p = ROOT / item["path"]
        assert p.is_file(), str(p)
        assert sha(p) == item["sha256"], str(p)
        checked.append(item["path"])
    records = []
    for item in evidence["productManifest"]:
        p = ROOT / item["path"]
        records.append(dict(path=item["path"], sha256=sha(p), mode=oct(p.stat().st_mode & 0o777)))
    fingerprint = hashlib.sha256(json.dumps(records, sort_keys=True).encode()).hexdigest()
    assert fingerprint == evidence["sourceFingerprint"]
    for item in evidence["checks"]:
        receipt = json.loads((ROOT / item["receipt"]).read_text())
        assert receipt["argv"] == item["argv"]
        assert receipt["exitCode"] == 0 and receipt["processExitObserved"] and not receipt["timedOut"]
        assert receipt["sourceBefore"] == receipt["sourceAfter"] == fingerprint
        assert receipt["productInputsUnchanged"]
        assert sha((ROOT / item["receipt"]).parent / receipt["output"]) == receipt["outputSha256"]
    for key in ["migration", "harness"]:
        record = evidence[key]
        p = ROOT / record["path"]
        baseline = subprocess.check_output(["git", "show", evidence["baselineHead"] + ":" + record["path"]], cwd=ROOT)
        assert baseline == p.read_bytes()
        assert sha(p) == record["currentSha256"]
    retained = evidence["historicalHarness"]
    assert sha(ROOT / retained["receipt"]) == retained["receiptSha256"]
    assert sha(ROOT / retained["output"]) == retained["outputSha256"]
    receipt = json.loads((ROOT / retained["receipt"]).read_text())
    assert receipt["argv"] == ["./scripts/harness/validate.sh"] and receipt["exitCode"] == 1
    assert receipt["sourceBefore"] == receipt["sourceAfter"] == fingerprint
    lifecycle = json.loads((DB / "attempt-7/accounting-summary.json").read_text())
    assert lifecycle["lifecycleCount"] == 43
    for snapshot in lifecycle["snapshots"]:
        assert snapshot["worker_started"] == snapshot["worker_joined"] == 1
        for value in snapshot.values():
            if isinstance(value, dict) and "current" in value:
                assert value["current"] == 0
        for name, cap in [("db_queued", snapshot["caps"]["db_queue"]), ("db_executing", 1),
                          ("db_outstanding", snapshot["caps"]["db_queue"] + 1)]:
            assert snapshot[name]["high_water"] <= cap
    return dict(cwd=str(ROOT), indexedPathsChecked=len(checked), productFilesChecked=len(records),
                successfulReceiptsVerified=len(evidence["checks"]), lifecycleCount=43,
                sourceFingerprint=fingerprint, pathsAndHashesMatch=True,
                note="Receipt/content verification only; independent acceptance remains a separate task review.")


if __name__ == "__main__":
    print(json.dumps(verify(), indent=2))
