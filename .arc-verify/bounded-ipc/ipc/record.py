"""Retain task-2 evidence without mutating product files or Git refs."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import resource
import shutil
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / ".arc-verify/bounded-ipc/ipc"
BASE = "42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0"
OLD = "3816bafaa684c8099dad78be2d0cb0e474c5f08b"
FILES = ["crates/arc-daemon/" + p for p in (
    "src/admission.rs", "src/connection.rs", "src/limits.rs", "src/platform.rs",
    "src/runtime_metrics.rs", "src/transport.rs", "src/transport_test_support.rs",
    "src/transport_tests.rs", "tests/transport_unit.rs",
)]
INPUTS = ["crates", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]
ENV = dict(os.environ, CARGO_NET_OFFLINE="true")


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def save(name, value):
    (OUT / name).write_text(json.dumps(value, indent=2) + "\n")


def command(argv, cwd=ROOT):
    result = subprocess.run(argv, cwd=cwd, env=ENV, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, timeout=20)
    return {"argv": argv, "cwd": str(cwd), "exitCode": result.returncode,
            "stdout": result.stdout.decode(errors="replace"),
            "stderr": result.stderr.decode(errors="replace")}


def fingerprint():
    listed = command(["git", "ls-files", "--cached", "--others",
                      "--exclude-standard", "-z", "--", *INPUTS])
    if listed["exitCode"]:
        raise RuntimeError(listed)
    entries = []
    for path in sorted(set(listed["stdout"].split("\0")) - {""}):
        file = ROOT / path
        entries.append({"path": path, "sha256": sha(file.read_bytes()),
                        "mode": oct(file.stat().st_mode & 0o777)})
    return {"algorithm": "sha256 canonical JSON of sorted path/content/mode records",
            "scope": INPUTS, "sha256": sha(json.dumps(entries, sort_keys=True).encode()),
            "files": entries}


def prepare():
    source = ROOT.parent / "d6c6b45f4a6c3769-78686d2704d4d690-1/.agents/results/result-backend-task-2-attempt-1-b137e253-def6-46ef-a75d-25ab414784ea.md"
    shutil.copyfile(source, OUT / "original-delivery.md")
    import re
    expected = dict((path, digest) for digest, path in re.findall(
        r"^([0-9a-f]{64})  (crates/arc-daemon/[^\n]+)$", source.read_text(), re.M))
    hashes = [{"path": p, "sha256": sha((ROOT / p).read_bytes()),
               "originalSha256": expected[p],
               "matchesOriginal": sha((ROOT / p).read_bytes()) == expected[p]}
              for p in FILES]
    refs = [BASE, OLD, "HEAD"]
    for attempt in (1, 2):
        for suffix in ("snapshot", "worker"):
            refs.append("refs/agent-valley/task-evidence/d6c6b45f4a6c3769-78686d2704d4d690-" + str(attempt) + "/" + suffix)
    git = []
    for ref in refs:
        git.append(command(["git", "rev-parse", ref, ref + "^{tree}"]))
        git.append(command(["git", "merge-base", "--is-ancestor", BASE, ref]))
    for ref in (BASE, OLD, "HEAD"):
        git.append(command(["git", "ls-tree", "-r", ref, "--", *FILES]))
    for args in (
        ["status", "--porcelain=v1", "--untracked-files=all"],
        ["status", "--porcelain=v1", "--untracked-files=all", "--", *INPUTS],
        ["diff", "--stat", BASE, "--", *INPUTS],
        ["diff", "--exit-code", "HEAD", "--", *INPUTS],
        ["diff", "--check"],
        ["check-ignore", "--", ".arc-verify/bounded-ipc/ipc/delivery.md",
         ".arc-verify/bounded-ipc/ipc/evidence.json"],
    ):
        git.append(command(["git", *args]))
    migration = "crates/arc-storage/migrations/0001_phase0.sql"
    baseline = subprocess.run(["git", "show", BASE + ":" + migration], cwd=ROOT,
                              capture_output=True, check=True).stdout
    current = (ROOT / migration).read_bytes()
    provenance = {"recordedAt": now(), "cwd": str(ROOT), "baselineHead": BASE,
        "baselineTree": "c802f71ac4d91ed6b8a602d5c83bf9cf57116b76",
        "originalSnapshotCommit": OLD, "gitReceipts": git, "sourceHashes": hashes,
        "originalReport": {"source": str(source), "retainedCopy": "original-delivery.md",
                           "sha256": sha(source.read_bytes())},
        "migration": {"path": migration, "argv": ["git", "show", BASE + ":" + migration],
                      "baselineSha256": sha(baseline), "currentSha256": sha(current),
                      "byteIdentical": baseline == current},
        "productFingerprint": fingerprint()}
    save("provenance-before.json", provenance)
    tools = [command(args) for args in (["rustc", "-Vv"], ["cargo", "-Vv"],
                                       ["rustup", "show", "active-toolchain"], ["uname", "-a"])]
    safe_env = {k: ENV[k] for k in ("PATH", "HOME", "TMPDIR", "CARGO_HOME", "CARGO_TARGET_DIR",
        "CARGO_NET_OFFLINE", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN", "RUSTFLAGS", "CODEX_SESSION_ID",
        "AGENT_VALLEY_MANAGED_RUN") if k in ENV}
    save("execution-context.json", {"recordedAt": now(), "tools": tools, "environment": safe_env,
        "cachePaths": {"cargoHome": ENV.get("CARGO_HOME", str(Path.home() / ".cargo")),
                       "target": ENV.get("CARGO_TARGET_DIR", str(ROOT / "target"))},
        "identities": {"actorId": "backend", "taskId": "task-2", "attempt": 2,
            "missionId": "b137e253-def6-46ef-a75d-25ab414784ea",
            "omaRunId": None, "injectedClaimPath": None,
            "note": "No OMA run ID or claim path injected; preserve absence, do not invent one."},
        "budget": {"missionMaxDurationSeconds": 86400, "missionMaxParallel": 3,
            "missionMaxRuns": 200, "missionMaxRetries": 3, "actorAttempt": 2,
            "concurrentCargoCommands": 1, "commandTimeoutSeconds": 120,
            "note": "No mission budgets reset; no new agents, retries, installs or builds."},
        "hostLimits": {k: resource.getrlimit(getattr(resource, k)) for k in
                       ("RLIMIT_NOFILE", "RLIMIT_NPROC", "RLIMIT_AS", "RLIMIT_CPU")}})
    assert all(item["matchesOriginal"] for item in hashes)
    assert baseline == current
    print("Prepared provenance: all nine original source hashes and migration bytes match.")


def gate(name, argv):
    before = fingerprint()
    started, clock = now(), time.monotonic()
    timed_out = False
    with (OUT / (name + ".txt")).open("wb") as output:
        process = subprocess.Popen(argv, cwd=ROOT, env=ENV, stdout=output, stderr=subprocess.STDOUT)
        save(name + ".running.json", {"argv": argv, "cwd": str(ROOT), "pid": process.pid,
                                     "startedAt": started, "timeoutSeconds": 120})
        try:
            status = process.wait(timeout=120)
        except subprocess.TimeoutExpired:
            timed_out = True
            process.terminate()
            try:
                status = process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                status = process.wait(timeout=5)
    after = fingerprint()
    receipt = {"argv": argv, "cwd": str(ROOT), "startedAt": started, "finishedAt": now(),
        "elapsedSeconds": time.monotonic() - clock, "exitCode": status, "timedOut": timed_out,
        "pid": process.pid, "processExitObserved": True,
        "output": name + ".txt", "outputSha256": sha((OUT / (name + ".txt")).read_bytes()),
        "outputIncludes": "full combined stdout and stderr",
        "context": "execution-context.json", "sourceBefore": before["sha256"],
        "sourceAfter": after["sha256"], "productInputsUnchanged": before == after}
    save(name + ".json", receipt)
    print(json.dumps(receipt))


def finish():
    save("provenance-after.json", {"recordedAt": now(), "productFingerprint": fingerprint(),
        "gitStatus": command(["git", "status", "--porcelain=v1", "--untracked-files=all", "--",
                              *INPUTS, ".arc-verify/bounded-ipc/ipc"]),
        "gitDiffCheck": command(["git", "diff", "--check"])})


if __name__ == "__main__":
    OUT.mkdir(parents=True, exist_ok=True)
    action = sys.argv[1]
    if action == "prepare":
        prepare()
    elif action == "finish":
        finish()
    else:
        gates = {"fmt": ["cargo", "fmt", "--all", "--", "--check"],
                 "clippy": ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
                 "workspace-test": ["cargo", "test", "--workspace"]}
        gate(action, gates[action])
