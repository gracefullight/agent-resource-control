"""Read-only collection verification; run from any collected checkout."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
EVIDENCE = ROOT / ".arc-verify/bounded-ipc/database/evidence.json"


def verify():
    data = json.loads(EVIDENCE.read_text())
    checked = []
    for item in data["sourceHashes"] + data["artifacts"]:
        path = ROOT / item["path"]
        actual = hashlib.sha256(path.read_bytes()).hexdigest()
        assert actual == item["sha256"], (item["path"], actual, item["sha256"])
        checked.append(item["path"])
    for item in data["checks"]:
        receipt = json.loads((ROOT / item["receipt"]).read_text())
        output = (ROOT / item["receipt"]).parent / receipt["output"]
        assert hashlib.sha256(output.read_bytes()).hexdigest() == receipt["outputSha256"]
        assert receipt["exitCode"] == 0 and receipt["processExitObserved"]
        assert not receipt["timedOut"] and receipt["productInputsUnchanged"]
        assert receipt["sourceBefore"] == receipt["sourceAfter"] == data["sourceFingerprint"]
    before = json.loads((EVIDENCE.parent / "repair-6/provenance-after.json").read_text())["productFingerprint"]
    records = []
    for item in before["files"]:
        path = ROOT / item["path"]
        records.append({"path": item["path"], "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                        "mode": oct(path.stat().st_mode & 0o777)})
    assert hashlib.sha256(json.dumps(records, sort_keys=True).encode()).hexdigest() == data["sourceFingerprint"]
    return {"cwd": str(ROOT), "evidence": str(EVIDENCE), "pathsChecked": len(checked),
            "allIndexedPathsResolveAndMatch": True, "successfulReceiptsVerified": len(data["checks"]),
            "sourceFingerprintMatches": True,
            "note": "Hash verification in this checkout; independent acceptance is a separate review."}


if __name__ == "__main__":
    print(json.dumps(verify(), indent=2))
