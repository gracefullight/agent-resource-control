"""Read-only collection checker. Inspect before running with python3 -B.

Checks are independent so a current-source mismatch cannot hide collection or
task-6 attribution results. Any failed requirement produces exit 1. No runtime
suite, collector, write, staging, build, or mission command is executed.
"""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import sys

sys.dont_write_bytecode = True
OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[4]
OLD = OUT.parent.parent / "resume-attempt-3"
REPORT = ".agents/results/result-test-engineer-task-5-uninjected-attempt-3-b137e253-def6-46ef-a75d-25ab414784ea.md"
REPORT_SHA = "43c6657a2ddb223d78e1fa4299b719499a8527d90d741e843d66a6878fce991d"
BASELINE = "42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0"
DOCS = {"docs/bounded-ipc-recovery.md", "docs/runtime-identity-usage.md"}
OWNED_TESTS = {"crates/arc-daemon/tests/resident.rs", "crates/arc-daemon/tests/support/mod.rs"}


def load(path):
    return json.loads(path.read_text())


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def require(ok, message):
    if not ok:
        raise AssertionError(message)


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT)


def paths(*scope):
    return set(git("ls-files", "--cached", "--others", "--exclude-standard", "-z", "--", *scope).decode().split("\0")) - {""}


def metadata(path):
    link = path.is_symlink()
    raw = os.readlink(path).encode() if link else path.read_bytes()
    return {"kind": "symlink" if link else "file", "bytes": len(raw),
            "sha256": sha(raw), "mode": oct(path.lstat().st_mode & 0o777)}


def main():
    results, failures = {}, {}

    def check(name, action):
        try:
            results[name] = action()
        except (AssertionError, OSError, KeyError, ValueError) as error:
            failures[name] = str(error)

    def boundary():
        before = load(OUT / "before.json")
        for item in before["files"]:
            actual = metadata(ROOT / item["path"])
            require(actual == {k: item[k] for k in actual}, "changed pre-repair input: " + item["path"])
        require({x["path"] for x in before["unreadable"]} == {"av.yaml"}, "unexpected unreadable input")
        initial = {x["path"] for x in before["files"] + before["unreadable"]}
        current = paths()
        require(initial <= current, "pre-repair path removed")
        added = current - initial
        prefix = OUT.relative_to(ROOT).as_posix() + "/"
        require(all(p == REPORT or p.startswith(prefix) for p in added), "unowned new file")
        delta = load(OUT / "changeSet.json")
        require(delta["addedPaths"] == sorted(added) and delta["modifiedPaths"] == [] and delta["deletedPaths"] == [], "changeSet differs")
        return {"unchangedReadableInputs": len(before["files"]), "unreadable": "av.yaml", "addedPaths": sorted(added)}

    def collection():
        index = load(OLD / "evidence.json")
        require(len(index["artifacts"]) == len({x["path"] for x in index["artifacts"]}) == 107, "index must retain 107 unique paths")
        for item in index["artifacts"]:
            path = ROOT / item["path"]
            require(path.is_file() and not path.is_symlink(), "missing/nonregular indexed file: " + item["path"])
            raw = path.read_bytes()
            require(len(raw) == item["bytes"] and sha(raw) == item["sha256"], "indexed size/hash: " + item["path"])
        raw = (ROOT / REPORT).read_bytes()
        require(len(raw) == 8417 and sha(raw) == REPORT_SHA and raw == (OLD / "delivery.md").read_bytes(), "restored report bytes")
        receipt = load(OUT / "collection.json")
        require(receipt["argv"] == ["git", "add", "-f", "--", REPORT] and receipt["exitCode"] == 0, "collection method")
        require(receipt["stagedPaths"] == [REPORT] and receipt["stagedReportSha256"] == REPORT_SHA, "collection receipt")
        # In the receiving checkout the collector may have committed the report;
        # require actual Git visibility and index bytes, not the Actor's HEAD.
        require(REPORT in paths(), "report omitted from Git collection")
        require(git("show", ":" + REPORT) == raw, "collected index blob differs")
        return {"indexedArtifactsVerified": 107, "reportBytes": len(raw), "reportSha256": sha(raw), "gitVisible": True}

    expected = load(OLD / "final-source.json")
    files = [{"path": p, "sha256": sha((ROOT / p).read_bytes()), "mode": oct((ROOT / p).stat().st_mode & 0o777)} for p in sorted(paths(*expected["scope"]))]
    current = {"sha256": sha(json.dumps(files, sort_keys=True).encode()), "scope": expected["scope"], "files": files}
    # Preserve original imports/helpers; neither record.gate nor verify.main runs.
    sys.path.insert(0, str(OLD))
    spec = importlib.util.spec_from_file_location("retained_verify", OLD / "verify.py")
    original = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(original)

    def source():
        require(original.record.record.fingerprint() == current, "original fingerprint algorithm differs")
        require(sha(json.dumps(expected["files"], sort_keys=True).encode()) == expected["sha256"] == load(OLD / "evidence.json")["sourceFingerprint"], "historical fingerprint invalid")
        previous = {x["path"]: x for x in expected["files"]}
        actual = {x["path"]: x for x in files}
        differences = [{"path": p, "expected": previous.get(p), "actual": actual.get(p)} for p in sorted(previous.keys() | actual.keys()) if previous.get(p) != actual.get(p)]
        results["sourceComparison"] = {"historicalFingerprint": expected["sha256"], "currentFingerprint": current["sha256"], "currentFiles": len(files), "differences": differences, "originalAlgorithmReproduced": True}
        require(current == expected, "current product differs from final-source.json; see sourceComparison")
        return True

    def task6():
        receipt = load(ROOT / ".arc-verify/bounded-ipc/task-6/attempt-3/delivery.json")
        for p in sorted(DOCS):
            raw = (ROOT / p).read_bytes()
            require({"bytes": len(raw), "sha256": sha(raw)} == receipt["files"][p], "task-6 document differs: " + p)
        return sorted(DOCS)

    def historical_boundary():
        differences = []
        for item in load(OLD / "before-boundary.json")["files"]:
            p = item["path"]
            require(oct((ROOT / p).stat().st_mode & 0o777) == item["mode"], "historical boundary mode: " + p)
            if sha((ROOT / p).read_bytes()) != item["sha256"]:
                differences.append(p)
        extra = sorted(set(differences) - DOCS - OWNED_TESTS)
        results["historicalBoundaryComparison"] = {"differences": differences, "unpermittedDifferences": extra}
        require(set(differences) == DOCS | OWNED_TESTS, "additional historical boundary differences: " + ", ".join(extra))
        return True

    def receipts():
        gates = [("fmt-final", ["cargo", "fmt", "--all", "--", "--check"], 0),
                 ("clippy-final", ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"], 0),
                 ("workspace-final", ["cargo", "test", "--workspace"], 0),
                 ("focused", ["cargo", "test", "-p", "arc-daemon", "--test", "bounded", "--test", "worker_runtime", "--", "--nocapture"], 0),
                 ("harness", ["./scripts/harness/validate.sh"], 1)]
        for name, argv, code in gates:
            receipt = load(OLD / (name + ".json"))
            require(receipt["argv"] == argv and receipt["exitCode"] == code and not receipt["timedOut"], "historical receipt: " + name)
            require(receipt["processExitObserved"] and receipt["productInputsUnchanged"], "historical process/source: " + name)
            require(receipt["sourceBefore"] == receipt["sourceAfter"] == expected["sha256"], "historical source binding: " + name)
            require(sha((OLD / receipt["output"]).read_bytes()) == receipt["outputSha256"], "historical output binding: " + name)
        for name, count in [("workspace-final", 173), ("focused", 51)]:
            rows = re.findall(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out", (OLD / (name + ".txt")).read_text())
            require(sum(int(row[0]) for row in rows) == count and all(all(int(x) == 0 for x in row[1:]) for row in rows), "historical test totals: " + name)
        return {"verifiedHistoricalReceipts": [x[0] for x in gates], "currentSourceBound": current == expected, "freshExecution": False}

    def preservation():
        result = {}
        for p, item in load(OLD / "preservation-targets.json")["frozen"].items():
            raw = (ROOT / p).read_bytes()
            require(raw == git("show", BASELINE + ":" + p), "accepted HEAD byte mismatch: " + p)
            require(sha(raw) == item["actualSha256"] == item["acceptedBaselineSha256"], "frozen hash mismatch: " + p)
            result[p] = {"bytes": len(raw), "sha256": sha(raw), "acceptedHeadByteIdentical": True}
        return result

    for name, action in [("repairBoundary", boundary), ("collection", collection), ("currentProduct", source),
                         ("task6Attribution", task6), ("historicalBoundary", historical_boundary),
                         ("historicalReceipts", receipts), ("preservation", preservation)]:
        check(name, action)
    return {"status": "failed" if failures else "verified-current-checkout", "observedHead": git("rev-parse", "HEAD").decode().strip(),
            "results": results, "failures": failures, "freshRuntimeChecks": False,
            "postCollectionReviewVerified": False, "configuredTargetApplicabilityVerified": False}


if __name__ == "__main__":
    result = main()
    print(json.dumps(result, indent=2))
    sys.exit(1 if result["failures"] else 0)
