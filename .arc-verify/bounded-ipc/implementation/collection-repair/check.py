"""Read-only collection check. Run with python3 -B after inspecting this file.

No Cargo, harness, benchmark, filesystem write, staging, or mission-state operation.
The original verifier remains unchanged and is expected to reject task-6's docs.
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
ROOT = OUT.parents[3]
OLD = OUT.parent / "resume-attempt-3"
REPORT = ".agents/results/result-test-engineer-task-5-uninjected-attempt-3-b137e253-def6-46ef-a75d-25ab414784ea.md"
REPORT_SHA = "43c6657a2ddb223d78e1fa4299b719499a8527d90d741e843d66a6878fce991d"
BASELINE = "42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0"
DOCS = {"docs/bounded-ipc-recovery.md", "docs/runtime-identity-usage.md"}
HISTORICALLY_OWNED = {"crates/arc-daemon/tests/resident.rs", "crates/arc-daemon/tests/support/mod.rs"}


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def load(path):
    return json.loads(path.read_text())


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT)


def listed_paths():
    return set(git("ls-files", "--cached", "--others", "--exclude-standard", "-z").decode().split("\0")) - {""}


def metadata(path):
    link = path.is_symlink()
    raw = os.readlink(path).encode() if link else path.read_bytes()
    return {"kind": "symlink" if link else "file", "bytes": len(raw),
            "sha256": digest(raw), "mode": oct(path.lstat().st_mode & 0o777)}


def main():
    before = load(OUT / "before.json")
    # This verifies every readable pre-repair tracked or nonignored input, including
    # historical logs/manifests/receipts and the original verifier. Symlinks are
    # compared without traversing their targets. av.yaml remains inaccessible.
    for item in before["files"]:
        actual = metadata(ROOT / item["path"])
        require(actual == {k: item[k] for k in actual}, "repair changed input: " + item["path"])
    require({x["path"] for x in before["unreadable"]} == {"av.yaml"}, "unexpected unreadable boundary")
    before_paths = {x["path"] for x in before["files"] + before["unreadable"]}
    after_paths = listed_paths()
    require(before_paths <= after_paths, "pre-repair path disappeared")
    added = after_paths - before_paths
    correction_prefix = str(OUT.relative_to(ROOT)) + "/"
    require(all(p == REPORT or p.startswith(correction_prefix) for p in added), "unowned added path")
    require(REPORT in added, "restored report is not visible to normal Git collection")

    index = load(OLD / "evidence.json")
    require(len(index["artifacts"]) == 107, "index must retain all 107 entries")
    require(len({x["path"] for x in index["artifacts"]}) == 107, "duplicate indexed entry")
    for item in index["artifacts"]:
        path = ROOT / item["path"]
        require(path.is_file() and not path.is_symlink(), "missing/nonregular indexed file: " + item["path"])
        raw = path.read_bytes()
        require(len(raw) == item["bytes"] and digest(raw) == item["sha256"], "indexed mismatch: " + item["path"])
    report = (ROOT / REPORT).read_bytes()
    require(len(report) == 8417 and digest(report) == REPORT_SHA, "restored report mismatch")
    require(report == (OLD / "delivery.md").read_bytes(), "report differs from retained delivery")
    provenance = load(OUT / "restoration.json")
    require(provenance["destination"] == REPORT and provenance["sha256"] == REPORT_SHA,
            "restoration provenance mismatch")
    require(provenance["source"] == ".agent-valley/parallel/workspaces/d6c6b45f4a6c3769-e7c08de777760acd-2/" + REPORT,
            "unexpected retained Actor source")
    collection = load(OUT / "collection.json")
    require(collection["argv"] == ["git", "add", "-f", "--", REPORT] and collection["exitCode"] == 0,
            "collection operation mismatch")
    require(collection["stagedPaths"] == [REPORT] and collection["stagedReportBytes"] == len(report)
            and collection["stagedReportSha256"] == REPORT_SHA, "collection index binding")
    delta = load(OUT / "changeSet.json")
    require(delta["addedPaths"] == sorted(added) and delta["modifiedPaths"] == []
            and delta["deletedPaths"] == [], "correction changeSet mismatch")

    # Reproduce record.py's original algorithm, including modes, sorted paths,
    # deduplication and json.dumps(sort_keys=True) with its default separators.
    expected = load(OLD / "final-source.json")
    paths = sorted(set(git("ls-files", "--cached", "--others", "--exclude-standard", "-z", "--",
                           *expected["scope"]).decode().split("\0")) - {""})
    files = [{"path": p, "sha256": digest((ROOT / p).read_bytes()),
              "mode": oct((ROOT / p).stat().st_mode & 0o777)} for p in paths]
    current = {"sha256": digest(json.dumps(files, sort_keys=True).encode()), "scope": expected["scope"], "files": files}
    require(current == expected and current["sha256"] == index["sourceFingerprint"], "product fingerprint mismatch")

    task6 = load(ROOT / ".arc-verify/bounded-ipc/task-6/attempt-3/delivery.json")
    differences = []
    for item in load(OLD / "before-boundary.json")["files"]:
        path = ROOT / item["path"]
        require(oct(path.stat().st_mode & 0o777) == item["mode"], "historical boundary mode: " + item["path"])
        raw = path.read_bytes()
        if digest(raw) != item["sha256"]:
            differences.append(item["path"])
            if item["path"] in DOCS:
                owned = task6["files"][item["path"]]
                require(len(raw) == owned["bytes"] and digest(raw) == owned["sha256"], "task-6 attribution mismatch")
            else:
                require(item["path"] in HISTORICALLY_OWNED, "unexplained historical boundary: " + item["path"])
    require(set(differences) == DOCS | HISTORICALLY_OWNED, "unexpected historical differences")

    # Import only inspected helper functions; do not call original main() or any
    # gate/collector. Its whole-boundary assertion is retained, never patched.
    sys.path.insert(0, str(OLD))
    spec = importlib.util.spec_from_file_location("retained_verify", OLD / "verify.py")
    original = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(original)
    require(original.record.record.fingerprint() == current, "original algorithm disagrees")
    gates = [
        ("fmt-final", ["cargo", "fmt", "--all", "--", "--check"], 0),
        ("clippy-final", ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"], 0),
        ("workspace-final", ["cargo", "test", "--workspace"], 0),
        ("focused", ["cargo", "test", "-p", "arc-daemon", "--test", "bounded", "--test", "worker_runtime", "--", "--nocapture"], 0),
        ("harness", ["./scripts/harness/validate.sh"], 1),
    ]
    for name, argv, code in gates:
        receipt = load(OLD / (name + ".json"))
        require(receipt["argv"] == argv and receipt["exitCode"] == code and not receipt["timedOut"], "receipt: " + name)
        require(receipt["processExitObserved"] and receipt["productInputsUnchanged"], "receipt process/source: " + name)
        require(receipt["sourceBefore"] == receipt["sourceAfter"] == current["sha256"], "receipt binding: " + name)
        require(digest((OLD / receipt["output"]).read_bytes()) == receipt["outputSha256"], "receipt output: " + name)
    for name, count in [("workspace-final", 173), ("focused", 51)]:
        results = re.findall(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out", (OLD / (name + ".txt")).read_text())
        require(sum(int(row[0]) for row in results) == count, "test count: " + name)
        require(all(all(int(x) == 0 for x in row[1:]) for row in results), "nonzero test result: " + name)
    preservation = {}
    for path, value in load(OLD / "preservation-targets.json")["frozen"].items():
        raw = (ROOT / path).read_bytes()
        accepted = git("show", BASELINE + ":" + path)
        require(raw == accepted, "accepted HEAD byte mismatch: " + path)
        require(digest(raw) == value["actualSha256"] == value["acceptedBaselineSha256"], "preservation hash: " + path)
        preservation[path] = {"bytes": len(raw), "sha256": digest(raw), "acceptedHeadByteIdentical": True}

    for folder in ["workspace-final-raw", "focused-raw"]:
        raw = OLD / folder
        for path in raw.glob("*.json"):
            data = load(path)
            for key in ["report", "restart_report"]:
                if key in data:
                    original.check_report(data[key])
        for name in ["SIGTERM", "SIGINT"]:
            data = load(raw / (name + ".json"))
            require(data["signal_to_exit_us"] <= 5_000_000 and data["reopen_integrity"] == "ok", folder + name)
        for name, admitted in [("connection", 8), ("request", 4), ("db", 9)]:
            data = load(raw / (name + "-100000.json"))
            require(data["attempted"] == 100_000 and data["admitted"] == admitted,
                    "attempt/admission count: " + name)
            require(data["rejected"] + admitted == data["attempted"], "attempt reconciliation: " + name)
            metrics = data["final"]
            original.check_gauges(metrics, metrics["caps"])
            require(metrics["handlers_spawned"] == metrics["handlers_reaped"], "handler accounting: " + name)
            require(metrics["worker_started"] == metrics["worker_joined"] == (1 if name == "db" else 0), "worker accounting: " + name)
            terminal = sum(data.get(key, 0) for key in ["completed", "timed_out", "shutdown_cancelled", "released_without_dispatch", "client_cancelled"])
            require(terminal == admitted, "terminal outcomes: " + name)
        performance = load(raw / "native-measurements.json")
        require(performance["attempts"] == {"attempted": 100000, "admitted": 100000, "completed": 100000,
                "rejected": 0, "timed_out": 0, "shutdown_cancelled": 0}, "native exchange accounting")
        samples = load(raw / "small-message-samples.json")
        require(len(samples["samples_ns"]) == 10000 and samples["failed"] == 0, "latency sample count")
        require(sorted(samples["samples_ns"])[9499] == performance["small_message"]["p95_ns"], "P95 recomputation")
        observations = load(raw / "native-observations.json")
        require(len(observations["idle"]) == 100 and observations["load"], "native observations")
        require(max(row["rss_bytes"] for row in observations["idle"]) == performance["idle"]["max_rss_bytes"], "RSS recomputation")
        require(performance["idle"]["max_rss_bytes"] < 50_000_000, "retained RSS target")
        require(performance["small_message"]["p95_ns"] <= 20_000_000, "retained P95 target")
        require(performance["lifetime_peak_rss_bytes"] > 0 and performance["sampled_native_thread_peak"] > 0, "native peak evidence")

    return {"status": "verified-current-checkout-collection", "observedHead": git("rev-parse", "HEAD").decode().strip(),
            "sourceFiles": len(files), "sourceFingerprint": current["sha256"], "indexedArtifactsVerified": 107,
            "restoredReport": {"path": REPORT, "bytes": len(report), "sha256": digest(report)},
            "preRepairInputsUnchanged": len(before["files"]), "unreadableInput": "av.yaml",
            "task6BoundaryAttribution": sorted(DOCS), "historicalTask5TestChanges": sorted(HISTORICALLY_OWNED),
            "repairAddedPaths": sorted(added), "repairModifiedExistingPaths": [], "preservation": preservation,
            "historicalReceiptsVerified": [x[0] for x in gates], "freshRuntimeChecks": False,
            "independentReviewRequired": True, "configuredTargetApplicabilityVerified": False}


if __name__ == "__main__":
    print(json.dumps(main(), indent=2))
