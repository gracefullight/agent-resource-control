"""Record task-3 resumption checks without changing product inputs or refs."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import resource
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
BASE = "42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0"
SNAP = "8571d9f990d8d521fee007e0131d07bb383bd66f"
PREFIX = "refs/agent-valley/task-evidence/d6c6b45f4a6c3769-1724980aa084bbaa-7/"
INPUTS = ["crates", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]
ENV = dict(os.environ, CARGO_NET_OFFLINE="true")


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def save(name, value):
    (OUT / name).write_text(json.dumps(value, indent=2) + "\n")


def command(argv):
    p = subprocess.run(argv, cwd=ROOT, env=ENV, capture_output=True, timeout=20)
    return dict(argv=argv, cwd=str(ROOT), exitCode=p.returncode,
                stdout=p.stdout.decode(errors="replace"), stderr=p.stderr.decode(errors="replace"))


def fingerprint():
    result = command(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z", "--", *INPUTS])
    assert result["exitCode"] == 0, result
    files = []
    for name in sorted(set(result["stdout"].split("\0")) - {""}):
        p = ROOT / name
        files.append(dict(path=name, sha256=sha(p.read_bytes()), mode=oct(p.stat().st_mode & 0o777)))
    return dict(scope=INPUTS, files=files, sha256=sha(json.dumps(files, sort_keys=True).encode()))


def preservation(path):
    p = subprocess.run(["git", "show", BASE + ":" + path], cwd=ROOT, capture_output=True)
    assert p.returncode == 0, p.stderr
    actual = (ROOT / path).read_bytes()
    return dict(path=path, argv=["git", "show", BASE + ":" + path], exitCode=p.returncode,
                baselineSha256=sha(p.stdout), currentSha256=sha(actual), byteIdentical=p.stdout == actual)


def provenance():
    return dict(recordedAt=now(), productFingerprint=fingerprint(),
        git=[command(a) for a in [
            ["git", "rev-parse", "HEAD", "HEAD^{tree}"],
            ["git", "rev-parse", BASE + "^{tree}"],
            ["git", "rev-parse", SNAP + "^{tree}"],
            ["git", "rev-parse", PREFIX + "snapshot"],
            ["git", "rev-parse", PREFIX + "worker"],
            ["git", "status", "--porcelain=v1", "--untracked-files=all", "--", *INPUTS],
            ["git", "diff", "--check"],
            ["git", "diff", "--stat", BASE, "--", *INPUTS],
            ["git", "ls-tree", "-r", SNAP, "--", "crates/arc-daemon/src/db_worker.rs",
             "crates/arc-daemon/src/db_worker", "crates/arc-daemon/tests/db_worker_unit.rs"],
        ]], migration=preservation("crates/arc-storage/migrations/0001_phase0.sql"),
        harness=preservation("scripts/harness/validate.sh"))


def gate(name, argv):
    before = fingerprint()
    started, clock = now(), time.monotonic()
    env = dict(ENV)
    if name == "worker-accounting":
        env["RUST_TEST_THREADS"] = "1"
    timed_out = False
    with (OUT / (name + ".txt")).open("wb") as log:
        p = subprocess.Popen(argv, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
        save(name + ".running.json", dict(argv=argv, cwd=str(ROOT), pid=p.pid, startedAt=started, timeoutSeconds=120))
        try:
            status = p.wait(timeout=120)
        except subprocess.TimeoutExpired:
            timed_out = True
            p.terminate()
            try:
                status = p.wait(timeout=5)
            except subprocess.TimeoutExpired:
                p.kill()
                status = p.wait(timeout=5)
    after = fingerprint()
    receipt = dict(argv=argv, cwd=str(ROOT), startedAt=started, finishedAt=now(),
        elapsedSeconds=time.monotonic()-clock, pid=p.pid, exitCode=status,
        timedOut=timed_out, processExitObserved=True, context="execution-context.json",
        environmentOverrides={"CARGO_NET_OFFLINE":"true", **({"RUST_TEST_THREADS":"1"} if name == "worker-accounting" else {})},
        output=name + ".txt", outputSha256=sha((OUT / (name + ".txt")).read_bytes()),
        outputIncludes="complete combined stdout and stderr", sourceBefore=before["sha256"],
        sourceAfter=after["sha256"], productInputsUnchanged=before == after)
    save(name + ".json", receipt)
    print(json.dumps(receipt))


if __name__ == "__main__":
    action = sys.argv[1]
    if action == "prepare":
        save("provenance-before.json", provenance())
        save("execution-context.json", dict(recordedAt=now(),
            identity=dict(actorId="database", taskId="task-3", attempt=7,
                codexSessionId=os.environ.get("CODEX_SESSION_ID"), omaRunId=None, injectedClaimPath=None),
            skillsUsed=["oma-db", "oma-backend"],
            tools=[command(a) for a in [["rustc", "-Vv"], ["cargo", "-Vv"], ["rustup", "show", "active-toolchain"], ["uname", "-a"]]],
            environment={k:ENV[k] for k in ["CARGO_HOME", "CARGO_TARGET_DIR", "CARGO_NET_OFFLINE", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN", "TMPDIR", "AGENT_VALLEY_MANAGED_RUN"] if k in ENV},
            budgets=dict(missionMaxDurationSeconds=86400, maxParallel=3, maxRuns=200, maxRetries=3,
                concurrentCargoCommands=1, commandTimeoutSeconds=120,
                note="Existing spent budgets retained; no dispatch or counter reset. No new mission deadline inferred."),
            hostLimits={k:resource.getrlimit(getattr(resource,k)) for k in ["RLIMIT_NOFILE", "RLIMIT_NPROC", "RLIMIT_AS", "RLIMIT_CPU"]},
            preflight=dict(noninteractiveExecutionSucceeded=True, serenaAvailable=False,
                inspection="Known-path direct reads; no code discovery fallback search.")))
        print("Prepared source provenance, toolchain and budget records.")
    elif action == "finish":
        save("provenance-after.json", provenance())
        print("Recorded final provenance.")
    else:
        gates = {
            "worker-test": ["cargo", "test", "-p", "arc-daemon", "--test", "db_worker_unit", "--", "--nocapture"],
            "fmt": ["cargo", "fmt", "--all", "--", "--check"],
            "clippy": ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
            "workspace-test": ["cargo", "test", "--workspace"],
            "worker-accounting": ["cargo", "test", "-p", "arc-daemon", "--test", "db_worker_unit", "--", "--nocapture"],
        }
        gate(action, gates[action])
