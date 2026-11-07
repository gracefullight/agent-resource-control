"""Record serial workspace checks; preserve prior worker receipts and Git refs."""
import datetime
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import resource
import subprocess
import sys

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
BASE = "42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0"
spec = importlib.util.spec_from_file_location("ipc_receipts", ROOT / ".arc-verify/bounded-ipc/ipc/record.py")
rec = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rec)
rec.ROOT = ROOT
rec.OUT = OUT
rec.ENV = dict(os.environ, CARGO_NET_OFFLINE="true")


def save(name, value):
    (OUT / name).write_text(json.dumps(value, indent=2) + "\n")


def hashes():
    paths = [
        "crates/arc-daemon/src/db_worker.rs",
        "crates/arc-daemon/src/db_worker/execution.rs",
        "crates/arc-daemon/src/db_worker/tests.rs",
        "crates/arc-daemon/tests/db_worker_unit.rs",
        "crates/arc-daemon/src/limits.rs",
        "crates/arc-daemon/src/runtime_metrics.rs",
        "docs/bounded-ipc-contract.md",
        ".arc-verify/bounded-ipc/ipc/evidence.json",
        "scripts/harness/validate.sh",
    ]
    return [{"path": p, "sha256": rec.sha((ROOT / p).read_bytes())} for p in paths]


def provenance():
    path = "crates/arc-storage/migrations/0001_phase0.sql"
    baseline = subprocess.check_output(["git", "show", BASE + ":" + path], cwd=ROOT)
    actual = (ROOT / path).read_bytes()
    return {
        "recordedAt": rec.now(), "cwd": str(ROOT), "baselineHead": BASE,
        "sourceHashes": hashes(), "productFingerprint": rec.fingerprint(),
        "migration": {"path": path, "baselineSha256": rec.sha(baseline),
                      "currentSha256": rec.sha(actual), "byteIdentical": baseline == actual,
                      "baselineReadArgv": ["git", "show", BASE + ":" + path], "baselineReadExitCode": 0},
        "git": [rec.command(args) for args in [
            ["git", "rev-parse", "HEAD", "HEAD^{tree}"],
            ["git", "status", "--porcelain=v1", "--untracked-files=all", "--", *rec.INPUTS],
            ["git", "for-each-ref", "--format=%(refname) %(objectname)", "refs/agent-valley/task-evidence"],
            ["git", "ls-tree", "-r", "HEAD", "--", "crates/arc-daemon/src/db_worker.rs",
             "crates/arc-daemon/src/db_worker", "crates/arc-daemon/tests/db_worker_unit.rs"],
            ["git", "diff", "--check"],
            ["git", "rev-parse", "9f6532ba89de234ae11895a6f8591d1fc6195d22",
             "9f6532ba89de234ae11895a6f8591d1fc6195d22^{tree}"],
            ["git", "merge-base", "--is-ancestor", BASE, "9f6532ba89de234ae11895a6f8591d1fc6195d22"],
            ["git", "ls-tree", "-r", "9f6532ba89de234ae11895a6f8591d1fc6195d22", "--",
             "crates/arc-daemon/src/db_worker.rs", "crates/arc-daemon/src/db_worker"],
        ]],
    }


if __name__ == "__main__":
    action = sys.argv[1]
    if action == "prepare":
        save("provenance-before.json", provenance())
        save("execution-context.json", {
            "recordedAt": rec.now(),
            "identity": {"actorId": "database", "taskId": "task-3", "attempt": 6,
                         "codexSessionId": os.environ.get("CODEX_SESSION_ID"),
                         "omaRunId": None, "injectedClaimPath": None,
                         "note": "No OMA run or claim identity injected; none manufactured."},
            "skillsUsed": ["oma-db", "oma-backend"],
            "tools": [rec.command(a) for a in [["rustc", "-Vv"], ["cargo", "-Vv"],
                       ["rustup", "show", "active-toolchain"], ["uname", "-a"]]],
            "environment": {k: rec.ENV[k] for k in ["CARGO_HOME", "CARGO_TARGET_DIR", "CARGO_NET_OFFLINE",
                            "RUSTUP_HOME", "RUSTUP_TOOLCHAIN", "RUSTFLAGS", "TMPDIR", "AGENT_VALLEY_MANAGED_RUN"]
                            if k in rec.ENV},
            "budget": {"missionMaxDurationSeconds": 86400, "missionMaxParallel": 3,
                       "missionMaxRuns": 200, "missionMaxRetries": 3, "attempt": 6,
                       "concurrentCargoCommands": 1, "commandTimeoutSeconds": 120,
                       "note": "Existing spent limits retained; no dispatch, routing mutation or counter reset."},
            "hostLimits": {k: resource.getrlimit(getattr(resource, k))
                           for k in ["RLIMIT_NOFILE", "RLIMIT_NPROC", "RLIMIT_AS", "RLIMIT_CPU"]},
            "preflight": {"argv": ["pwd"], "exitCode": 0,
                          "result": str(ROOT), "noninteractiveExecutionSucceeded": True,
                          "serenaAvailable": False, "inspection": "Known-path direct reads only."},
        })
        print("Prepared execution context and original-source provenance.")
    elif action == "finish":
        save("provenance-after.json", provenance())
        print("Recorded final provenance.")
    else:
        commands = {
            "format-owned": ["rustfmt", "--edition", "2021", "crates/arc-daemon/src/db_worker/tests.rs"],
            "registered-constructor-failure": ["cargo", "test", "-p", "arc-daemon", "--test", "db_worker_unit", "--", "--nocapture"],
            "worker-test": ["cargo", "test", "-p", "arc-daemon", "--test", "db_worker_unit", "--", "--nocapture"],
            "worker-test-final": ["cargo", "test", "-p", "arc-daemon", "--test", "db_worker_unit", "--", "--nocapture"],
            "worker-test-accounting": ["cargo", "test", "-p", "arc-daemon", "--test", "db_worker_unit", "--", "--nocapture"],
            "fmt": ["cargo", "fmt", "--all", "--", "--check"],
            "fmt-final": ["cargo", "fmt", "--all", "--", "--check"],
            "clippy": ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
            "workspace-test": ["cargo", "test", "--workspace"],
        }
        if action == "worker-test-accounting":
            rec.ENV["RUST_TEST_THREADS"] = "1"
            context = json.loads((OUT / "execution-context.json").read_text())
            context["recordedAt"] = rec.now()
            context["environment"]["RUST_TEST_THREADS"] = "1"
            context["reason"] = "Avoid libtest stdout interleaving with JSON on combined stderr; concurrent producer fixture remains unchanged."
            save("accounting-execution-context.json", context)
        rec.gate(action, commands[action])
        if action == "worker-test-accounting":
            receipt = json.loads((OUT / (action + ".json")).read_text())
            receipt["context"] = "accounting-execution-context.json"
            receipt["environmentOverrides"] = {"RUST_TEST_THREADS": "1"}
            save(action + ".json", receipt)
