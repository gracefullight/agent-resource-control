"""Read-only collection verification for task-4 integration artifacts."""
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
sha = lambda data: hashlib.sha256(data).hexdigest()

def verify():
    evidence = json.loads((OUT / "evidence.json").read_text())
    for item in evidence["sourceHashes"] + evidence["artifacts"]:
        assert sha((ROOT / item["path"]).read_bytes()) == item["sha256"], item["path"]
    for name in evidence["finalChecks"]:
        receipt = json.loads((OUT / (name + ".json")).read_text())
        assert receipt["exitCode"] == (1 if name == "harness" else 0), name
        assert receipt["processExitObserved"] and not receipt["timedOut"], name
        assert receipt["sourceBefore"] == receipt["sourceAfter"] == evidence["sourceFingerprint"], name
        assert sha((OUT / receipt["output"]).read_bytes()) == receipt["outputSha256"], name
    boundary = json.loads((OUT / "boundary-check.json").read_text())
    for path, record in boundary["baselineFiles"].items():
        baseline = subprocess.check_output(["git", "show", evidence["baselineHead"] + ":" + path], cwd=ROOT)
        assert baseline == (ROOT / path).read_bytes(), path
        assert sha(baseline) == record["baselineSha256"], path
    for path, record in boundary["acceptedInputs"].items():
        assert sha((ROOT / path).read_bytes()) == record["currentSha256"], path
    for path, record in boundary["sharedManifestAndLockfiles"].items():
        assert sha((ROOT / path).read_bytes()) == record["sha256"], path
    paths = subprocess.check_output(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z", "--", "crates", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"], cwd=ROOT).decode().split("\0")
    files = [{"path":p, "sha256":sha((ROOT/p).read_bytes()), "mode":oct((ROOT/p).stat().st_mode & 0o777)} for p in sorted(set(paths)-{""})]
    assert sha(json.dumps(files, sort_keys=True).encode()) == evidence["sourceFingerprint"]
    assert evidence["independentReviewComplete"] is False
    print(json.dumps({"sourceFiles":len(files), "artifactsVerified":len(evidence["artifacts"]), "checksVerified":len(evidence["finalChecks"]), "fingerprint":evidence["sourceFingerprint"], "status":"matches-current-checkout"}))

if __name__ == "__main__":
    verify()
