"""Task-4 command receipts; never changes product, Git refs or mission state."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import resource
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
ENV = dict(os.environ, CARGO_NET_OFFLINE="true", CARGO_TARGET_DIR=str(ROOT / "target"))
INPUTS = ["crates", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]

def sha(data):
    return hashlib.sha256(data).hexdigest()

def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()

def save(name, data):
    (OUT / name).write_text(json.dumps(data, indent=2) + "\n")

def fingerprint():
    paths = subprocess.check_output(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z", "--", *INPUTS], cwd=ROOT).decode().split("\0")
    files = [{"path": p, "sha256": sha((ROOT / p).read_bytes()), "mode": oct((ROOT / p).stat().st_mode & 0o777)} for p in sorted(set(paths) - {""})]
    return {"sha256": sha(json.dumps(files, sort_keys=True).encode()), "scope": INPUTS, "files": files}

def gate(name, argv):
    before = fingerprint()
    save(name + "-source.json", before)
    started, clock = now(), time.monotonic()
    timed_out = False
    with (OUT / (name + ".txt")).open("wb") as output:
        process = subprocess.Popen(argv, cwd=ROOT, env=ENV, stdout=output, stderr=subprocess.STDOUT, start_new_session=True)
        save(name + ".running.json", {"argv": argv, "cwd": str(ROOT), "pid": process.pid, "processGroup": process.pid, "startedAt": started, "timeoutSeconds": 120})
        try:
            status = process.wait(timeout=120)
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(process.pid, signal.SIGTERM)
            try:
                status = process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                status = process.wait(timeout=5)
    after = fingerprint()
    receipt = {"argv": argv, "cwd": str(ROOT), "startedAt": started, "finishedAt": now(), "elapsedSeconds": time.monotonic() - clock,
        "exitCode": status, "timedOut": timed_out, "pid": process.pid, "processExitObserved": True,
        "output": name + ".txt", "outputSha256": sha((OUT / (name + ".txt")).read_bytes()),
        "sourceBefore": before["sha256"], "sourceAfter": after["sha256"], "productInputsUnchanged": before == after,
        "environment": {k: ENV[k] for k in ("CARGO_HOME", "CARGO_TARGET_DIR", "CARGO_NET_OFFLINE", "RUSTUP_TOOLCHAIN", "AGENT_VALLEY_MANAGED_RUN") if k in ENV},
        "toolchain": subprocess.check_output(["rustc", "-Vv"], cwd=ROOT, env=ENV).decode(),
        "cargo": subprocess.check_output(["cargo", "-Vv"], cwd=ROOT, env=ENV).decode()}
    save(name + ".json", receipt)
    print(json.dumps(receipt))
    print((OUT / (name + ".txt")).read_text()[-5000:])
    return status

if __name__ == "__main__":
    if sys.argv[1] == "context":
        results = []
        for argv in [["git", "rev-parse", "HEAD", "HEAD^{tree}"], ["rustc", "-Vv"], ["cargo", "-Vv"], ["uname", "-a"]]:
            result = subprocess.run(argv, cwd=ROOT, env=ENV, capture_output=True)
            results.append({"argv": argv, "exitCode": result.returncode, "stdout":result.stdout.decode(), "stderr":result.stderr.decode()})
        save("execution-context.json", {"recordedAt": now(), "tools": results, "environment": {k:ENV[k] for k in ("CARGO_HOME","CARGO_TARGET_DIR","CARGO_NET_OFFLINE","RUSTUP_TOOLCHAIN","AGENT_VALLEY_MANAGED_RUN") if k in ENV},
            "identity":{"actor":"backend","task":"task-4","attempt":5,"injectedClaimPath":None,"omaRunId":None},
            "budget":{"missionMaxDurationSec":86400,"missionMaxParallel":3,"missionMaxRuns":200,"missionMaxRetries":3,"parallelCargoCommands":1,"commandTimeoutSec":120,"note":"Existing mission limits retained; no agents, installs, standalone builds or mission-state writes."},
            "hostLimits":{k:resource.getrlimit(getattr(resource,k)) for k in ("RLIMIT_NOFILE","RLIMIT_NPROC","RLIMIT_AS","RLIMIT_CPU")},
            "productFingerprint":fingerprint()})
    else:
        sys.exit(gate(sys.argv[1], sys.argv[2:]))
