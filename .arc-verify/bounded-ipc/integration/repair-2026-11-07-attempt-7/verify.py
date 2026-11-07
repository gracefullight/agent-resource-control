"""Read-only verification in the receiving mission checkout; no Git writes."""
import hashlib
import json
from pathlib import Path
import subprocess

from record import fingerprint

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[3]
INDEX = OUT.parent / "evidence.json"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def verify():
    index = json.loads(INDEX.read_text())
    assert index["attempt"] == 7
    for item in index["artifacts"] + index["sourceHashes"]:
        path = ROOT / item["path"]
        assert path.is_file(), item["path"]
        assert sha(path.read_bytes()) == item["sha256"], item["path"]
    current = fingerprint()
    assert current["sha256"] == index["sourceFingerprint"]
    assert len(current["files"]) == index["sourceFileCount"]
    for name, expected in index["checks"].items():
        receipt = json.loads((OUT / (name + ".json")).read_text())
        assert receipt["exitCode"] == expected["exitCode"], name
        assert receipt["argv"] == expected["argv"], name
        assert receipt["cwd"] == str(ROOT), name
        assert receipt["processExitObserved"] and not receipt["timedOut"], name
        assert sha((OUT / receipt["output"]).read_bytes()) == receipt["outputSha256"], name
        before = json.loads((OUT / (name + "-source.json")).read_text())
        after = json.loads((OUT / (name + "-source-after.json")).read_text())
        assert before["sha256"] == receipt["sourceBefore"]
        assert after["sha256"] == receipt["sourceAfter"]
        if name in index["currentTreeChecks"]:
            assert receipt["sourceBefore"] == receipt["sourceAfter"] == current["sha256"]
    boundary = json.loads((OUT / "boundary-check.json").read_text())
    for path, saved in boundary["acceptedBaselineFiles"].items():
        baseline = subprocess.check_output(["git", "show", index["baselineHead"] + ":" + path], cwd=ROOT)
        assert baseline == (ROOT / path).read_bytes()
        assert sha(baseline) == saved["sha256"]
    initial = json.loads((OUT / "execution-context.json").read_text())["productFingerprint"]
    before = {item["path"]: item for item in initial["files"]}
    after = {item["path"]: item for item in current["files"]}
    changed = sorted(p for p in before.keys() | after.keys() if before.get(p) != after.get(p))
    assert changed == index["changedProductPaths"]
    assert index["independentReviewComplete"] is False
    assert index["missionCompletionClaimed"] is False
    return {"status": "matches-receiving-mission-checkout", "cwd": str(ROOT),
            "artifactsVerified": len(index["artifacts"]), "sourceFiles": len(current["files"]),
            "sourceFingerprint": current["sha256"], "checksVerified": len(index["checks"]),
            "changedProductPaths": changed, "migrationAndHarnessMatchAcceptedHead": True}


if __name__ == "__main__":
    print(json.dumps(verify(), indent=2))
