"""Read-only collection check for this attempt's actual files and receipts."""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

sys.dont_write_bytecode = True
import record

ROOT, OUT = record.ROOT, record.OUT


def read(name):
    return json.loads((OUT / name).read_text())


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check_gauges(metrics, limits):
    bounds = {"connections": limits["connections"], "requests": limits["requests"],
              "tracked_tasks": limits["connections"], "db_queued": limits["db_queue"],
              "db_executing": 1, "db_outstanding": limits["db_queue"] + 1,
              "request_body_bytes": limits["requests"] * 1048576,
              "response_bytes": limits["requests"] * 1048576, "rejection_work": 1}
    for key, cap in bounds.items():
        assert metrics[key]["current"] == 0, key
        assert metrics[key]["high_water"] <= cap, key


def check_report(report):
    for key in ["accounted", "handlers_reaped", "worker_joined", "connection_closed",
                "gauges_zero", "socket_removed", "deadline_met"]:
        assert report[key] is True, key
    assert report["handlers_remaining"] == 0
    metrics = report["metrics"]
    check_gauges(metrics, report["limits"])
    assert metrics["handlers_spawned"] == metrics["handlers_reaped"]
    assert metrics["worker_started"] == metrics["worker_joined"] == 1


def main():
    index = read("evidence.json")
    for item in index["artifacts"]:
        path = ROOT / item["path"]
        assert path.is_file() and path.stat().st_size == item["bytes"], item["path"]
        assert sha(path) == item["sha256"], item["path"]
    current = record.record.fingerprint()
    assert current == read("final-source.json")
    assert current["sha256"] == index["sourceFingerprint"]
    owned = {"crates/arc-daemon/tests/resident.rs", "crates/arc-daemon/tests/support/mod.rs"}
    for item in read("before-boundary.json")["files"]:
        path = ROOT / item["path"]
        if item["path"] not in owned:
            assert sha(path) == item["sha256"], item["path"]
        assert oct(path.stat().st_mode & 0o777) == item["mode"], item["path"]
    gates = [
        ("fmt-final", ["cargo", "fmt", "--all", "--", "--check"], 0),
        ("clippy-final", ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"], 0),
        ("workspace-final", ["cargo", "test", "--workspace"], 0),
        ("focused", ["cargo", "test", "-p", "arc-daemon", "--test", "bounded", "--test", "worker_runtime", "--", "--nocapture"], 0),
        ("harness", ["./scripts/harness/validate.sh"], 1),
    ]
    for name, argv, code in gates:
        receipt = read(name + ".json")
        assert receipt["argv"] == argv
        assert receipt["exitCode"] == code and not receipt["timedOut"]
        assert receipt["processExitObserved"] and receipt["productInputsUnchanged"]
        assert receipt["sourceBefore"] == receipt["sourceAfter"] == current["sha256"]
        assert sha(OUT / receipt["output"]) == receipt["outputSha256"]
    for name in ["workspace-final", "focused"]:
        results = re.findall(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out", (OUT / (name + ".txt")).read_text())
        assert sum(int(r[0]) for r in results) == (173 if name == "workspace-final" else 51)
        assert all(all(int(x) == 0 for x in row[1:]) for row in results)
    for path, value in read("preservation-targets.json")["frozen"].items():
        assert sha(ROOT / path) == value["actualSha256"] == value["acceptedBaselineSha256"]
    for folder in ["workspace-final-raw", "focused-raw"]:
        raw = OUT / folder
        for path in raw.glob("*.json"):
            data = json.loads(path.read_text())
            if "report" in data:
                check_report(data["report"])
            if "restart_report" in data:
                check_report(data["restart_report"])
        for name in ["SIGTERM", "SIGINT"]:
            data = json.loads((raw / (name + ".json")).read_text())
            assert data["signal_to_exit_us"] <= 5_000_000
            assert data["reopen_integrity"] == "ok"
        for name, admitted in [("connection", 8), ("request", 4), ("db", 9)]:
            data = json.loads((raw / (name + "-100000.json")).read_text())
            assert data["attempted"] == 100_000
            assert data["admitted"] == admitted
            assert data["rejected"] + data["admitted"] == data["attempted"]
            metrics = data["final"]
            check_gauges(metrics, metrics["caps"])
            assert metrics["handlers_spawned"] == metrics["handlers_reaped"]
            assert metrics["worker_started"] == metrics["worker_joined"] == (1 if name == "db" else 0)
            terminal = sum(data.get(key, 0) for key in ["completed", "timed_out", "shutdown_cancelled", "released_without_dispatch", "client_cancelled"])
            assert terminal == data["admitted"]
        performance = json.loads((raw / "native-measurements.json").read_text())
        assert performance["attempts"] == {"attempted": 100000, "admitted": 100000,
               "completed": 100000, "rejected": 0, "timed_out": 0, "shutdown_cancelled": 0}
        samples = json.loads((raw / "small-message-samples.json").read_text())
        assert len(samples["samples_ns"]) == 10_000 and samples["failed"] == 0
        assert sorted(samples["samples_ns"])[9499] == performance["small_message"]["p95_ns"]
        observations = json.loads((raw / "native-observations.json").read_text())
        assert len(observations["idle"]) == 100 and observations["load"]
        assert max(row["rss_bytes"] for row in observations["idle"]) == performance["idle"]["max_rss_bytes"]
        assert performance["idle"]["max_rss_bytes"] < 50_000_000
        assert performance["small_message"]["p95_ns"] <= 20_000_000
        assert performance["lifetime_peak_rss_bytes"] > 0
        assert performance["sampled_native_thread_peak"] > 0
    return {"status": "verified-current-bytes", "sourceFiles": len(current["files"]),
            "sourceFingerprint": current["sha256"], "artifacts": len(index["artifacts"]),
            "observedReviewHead": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT).decode().strip(),
            "recordedActorHead": "5aa081fb2f73f3d606ef009e30872d32aa94ce6b",
            "baselineHead": "42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0"}


if __name__ == "__main__":
    print(json.dumps(main()))
