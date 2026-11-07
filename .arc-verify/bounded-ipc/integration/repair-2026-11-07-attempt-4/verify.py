"""Read-only validation of collected task-4 artifacts and current source bytes."""
import hashlib
import json
from pathlib import Path
import subprocess
import record

ROOT, OUT = record.ROOT, record.OUT
sha = lambda data: hashlib.sha256(data).hexdigest()


def verify():
    evidence = json.loads((OUT.parent / "evidence.json").read_text())
    for item in evidence["sourceHashes"] + evidence["artifacts"] + evidence["historicalArtifacts"]:
        path = ROOT / item["path"]
        assert sha(path.read_bytes()) == item["sha256"], item["path"]
        if "bytes" in item:
            assert path.stat().st_size == item["bytes"], item["path"]
        if "mode" in item:
            assert oct(path.stat().st_mode & 0o777) == item["mode"], item["path"]
    for name, check in evidence["finalChecks"].items():
        receipt = json.loads((ROOT / check["receipt"]).read_text())
        assert receipt["argv"] == check["argv"] and receipt["exitCode"] == check["exitCode"]
        assert receipt["exitCode"] == (1 if name == "harness" else 0)
        assert receipt["processExitObserved"] and not receipt["timedOut"]
        assert receipt["sourceBefore"] == receipt["sourceAfter"] == evidence["sourceFingerprint"]
        assert sha((OUT / receipt["output"]).read_bytes()) == receipt["outputSha256"]
        assert receipt["environment"]["CARGO_NET_OFFLINE"] == "true" and receipt["toolchain"]
        assert receipt["cwd"] == str(ROOT)
    for item in json.loads((OUT / "before-boundary.json").read_text()):
        path = ROOT / item["path"]
        assert sha(path.read_bytes()) == item["sha256"], item["path"]
        assert oct(path.stat().st_mode & 0o777) == item["mode"], item["path"]
    assert record.fingerprint()["sha256"] == evidence["sourceFingerprint"]
    for path, item in json.loads((OUT / "preservation.json").read_text()).items():
        accepted = subprocess.check_output(["git", "show", evidence["baselineHead"] + ":" + path], cwd=ROOT)
        assert accepted == (ROOT / path).read_bytes() and sha(accepted) == item["sha256"]
    assert evidence["authoritativeReportChannel"] == "ARC_RUNTIME_REPORT"
    assert not evidence["independentReviewComplete"] and not evidence["postCollectionVerified"]
    return {"status": "matches-current-checkout", "sourceFiles": evidence["sourceFileCount"],
            "artifactsVerified": len(evidence["artifacts"]),
            "historicalEntriesVerified": len(evidence["historicalArtifacts"]),
            "checksVerified": len(evidence["finalChecks"]),
            "sourceFingerprint": evidence["sourceFingerprint"],
            "independentReviewComplete": False, "postCollectionVerified": False}


if __name__ == "__main__":
    print(json.dumps(verify(), indent=2))
