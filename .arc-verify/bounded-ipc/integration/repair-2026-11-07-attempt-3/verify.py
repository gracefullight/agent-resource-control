"""Read-only checker of the current task-4 repair and collected receipts."""
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[4]
REPAIR = Path(__file__).resolve().parent
OUT = REPAIR.parent
sha = lambda b: hashlib.sha256(b).hexdigest()

def verify():
    evidence = json.loads((OUT / "evidence.json").read_text())
    for item in evidence["sourceHashes"] + evidence["artifacts"]:
        p = ROOT / item["path"]
        assert sha(p.read_bytes()) == item["sha256"], item["path"]
        if "bytes" in item: assert p.stat().st_size == item["bytes"], item["path"]
    for name in evidence["finalChecks"]:
        receipt = json.loads((REPAIR / (name + ".json")).read_text())
        assert receipt["exitCode"] == (1 if name == "harness" else 0), name
        assert receipt["processExitObserved"] and not receipt["timedOut"], name
        assert receipt["sourceBefore"] == receipt["sourceAfter"] == evidence["sourceFingerprint"], name
        assert sha((REPAIR / receipt["output"]).read_bytes()) == receipt["outputSha256"], name
        assert receipt["toolchain"] and receipt["environment"]["CARGO_NET_OFFLINE"] == "true", name
    for receipt_path in REPAIR.glob("*.json"):
        receipt = json.loads(receipt_path.read_text())
        if isinstance(receipt, dict) and "outputSha256" in receipt:
            assert sha((REPAIR / receipt["output"]).read_bytes()) == receipt["outputSha256"], receipt_path.name
            assert receipt["processExitObserved"] and not receipt["timedOut"], receipt_path.name
    boundary = json.loads((REPAIR / "boundary-check.json").read_text())
    for path, expected in boundary["allCurrentHashes"].items():
        assert sha((ROOT/path).read_bytes()) == expected, path
    assert set(boundary["changed"] + boundary["added"]) == set(evidence["changedProductPaths"])
    baseline = boundary["baselinePreservationChecks"]
    for path, record in baseline["baselineFiles"].items():
        accepted = subprocess.check_output(["git","show",evidence["baselineHead"]+":"+path],cwd=ROOT)
        assert accepted == (ROOT/path).read_bytes(), path
        assert sha(accepted) == record["baselineSha256"], path
    for path, record in baseline["acceptedInputs"].items():
        assert sha((ROOT/path).read_bytes()) == record["currentSha256"], path
    for path, record in baseline["sharedManifestAndLockfiles"].items():
        assert sha((ROOT/path).read_bytes()) == record["sha256"], path
    paths = subprocess.check_output(["git","ls-files","--cached","--others","--exclude-standard","-z","--","crates","Cargo.toml","Cargo.lock","rust-toolchain.toml"],cwd=ROOT).decode().split("\0")
    files = [{"path":p,"sha256":sha((ROOT/p).read_bytes()),"mode":oct((ROOT/p).stat().st_mode & 0o777)} for p in sorted(set(paths)-{""})]
    assert sha(json.dumps(files,sort_keys=True).encode()) == evidence["sourceFingerprint"]
    assert evidence["independentReviewComplete"] is False
    assert evidence["authoritativeReportChannel"] == "ARC_RUNTIME_REPORT"
    return {"status":"matches-current-checkout","sourceFiles":len(files),"artifactsVerified":len(evidence["artifacts"]),"checksVerified":len(evidence["finalChecks"]),"fingerprint":evidence["sourceFingerprint"]}

if __name__ == "__main__": print(json.dumps(verify(),indent=2))
