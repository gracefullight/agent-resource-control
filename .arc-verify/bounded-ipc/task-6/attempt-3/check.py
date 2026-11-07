"""Task-local documentation evidence; does not mutate product or mission state."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import resource
import subprocess
import sys
import time
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
DOCS = ["docs/runtime-identity-usage.md", "docs/bounded-ipc-recovery.md"]


def utc():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(name, value):
    (OUT / name).write_text(json.dumps(value, indent=2) + "\n")


def git(*args):
    result = subprocess.run(["git", *args], cwd=ROOT, capture_output=True)
    return {"argv": ["git", *args], "exitCode": result.returncode,
            "stdout": result.stdout.decode().strip(), "stderr": result.stderr.decode().strip()}


def tracked():
    result = subprocess.run(["git", "ls-files", "-z"], cwd=ROOT, check=True, capture_output=True)
    # The host denies av.yaml access. Preserve that limit rather than bypassing it.
    return [x.decode() for x in result.stdout.split(b"\0") if x and x != b"av.yaml"]


def manifest(paths):
    result = {}
    for p in paths:
        path = ROOT / p
        if path.is_symlink():
            result[p] = {"symlink": os.readlink(path), "mode": oct(path.lstat().st_mode & 0o777)}
        else:
            result[p] = {"sha256": digest(path), "mode": oct(path.stat().st_mode & 0o777)}
    return result


def product():
    return manifest([p for p in tracked() if p.startswith("crates/") or p in
                     ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml")])


def preflight():
    deadline = "2026-11-08T10:52:35.601000+00:00"
    now = datetime.datetime.now(datetime.timezone.utc)
    assert now < datetime.datetime.fromisoformat(deadline), "Saved mission deadline elapsed"
    prefix = "refs/agent-valley/task-evidence/d6c6b45f4a6c3769-98c4ca9fa7207111-2/"
    save("preflight.json", {
        "recordedAt": utc(), "savedDeadline": deadline,
        "remainingSeconds": (datetime.datetime.fromisoformat(deadline) - now).total_seconds(),
        "maxParallel": 3, "maxRuns": 200, "recordedMissionRunsAtDispatch": 63,
        "maxRetries": 3, "taskAttempt": 3,
        "budgetSource": "Supplied saved mission; limits and elapsed budget not reset.",
        "platform": platform.platform(), "machine": platform.machine(),
        "limits": {name: resource.getrlimit(getattr(resource, name)) for name in
                   ("RLIMIT_NOFILE", "RLIMIT_NPROC", "RLIMIT_AS", "RLIMIT_CPU")},
        "head": git("rev-parse", "HEAD"), "tree": git("rev-parse", "HEAD^{tree}"),
        "baselineTree": git("rev-parse", "42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0^{tree}"),
        "snapshotRef": git("rev-parse", "--verify", prefix + "snapshot"),
        "workerRef": git("rev-parse", "--verify", prefix + "worker"),
        "claimPath": None, "omaRunId": None, "serenaAvailable": False,
        "scope": "Only two assigned docs and task-specific evidence/report outputs.",
        "boundaryExclusions": {"av.yaml": "Host denied read with Operation not permitted; unverified."}
    })
    save("before.json", manifest(tracked()))


def run():
    before = product()
    save("source-before.json", before)
    env = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / "target"), CARGO_NET_OFFLINE="true")
    argv = ["cargo", "test", "--workspace"]
    started = utc()
    clock = time.monotonic()
    timed_out = False
    with (OUT / "workspace-test.txt").open("wb") as log:
        child = subprocess.Popen(argv, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
        save("workspace-test.running.json", {"argv": argv, "cwd": str(ROOT),
             "pid": child.pid, "startedAt": started, "timeoutSeconds": 120})
        try:
            code = child.wait(timeout=120)
        except subprocess.TimeoutExpired:
            timed_out = True
            child.terminate()
            try:
                code = child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                code = child.wait(timeout=5)
    after = product()
    save("source-after.json", after)
    log = (OUT / "workspace-test.txt").read_text()
    suites = re.findall(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out", log)
    totals = [sum(int(row[i]) for row in suites) for i in range(5)]
    save("workspace-test.json", {"argv": argv, "cwd": str(ROOT), "pid": child.pid,
         "startedAt": started, "finishedAt": utc(), "elapsedSeconds": time.monotonic() - clock,
         "exitCode": code, "timedOut": timed_out, "processExitObserved": True,
         "environment": {k: env[k] for k in ("CARGO_TARGET_DIR", "CARGO_NET_OFFLINE")},
         "output": "workspace-test.txt", "outputSha256": digest(OUT / "workspace-test.txt"),
         "sourceFiles": len(before), "sourceBeforeSha256": digest(OUT / "source-before.json"),
         "sourceAfterSha256": digest(OUT / "source-after.json"),
         "productInputsUnchanged": before == after,
         "suites": len(suites), "totals": dict(zip(["passed", "failed", "ignored", "measured", "filtered"], totals))})
    print(json.dumps({"exitCode": code, "timedOut": timed_out, "suites": len(suites), "totals": totals}))
    assert code == 0 and not timed_out and before == after


def references():
    links = []
    for doc in DOCS:
        source = (ROOT / doc).read_text()
        for target in re.findall(r"\[[^\]]*\]\(([^)]+)\)", source):
            if re.match(r"(?:https?:|mailto:)", target):
                links.append({"doc": doc, "target": target, "scope": "external-unverified"})
                continue
            path, _, fragment = unquote(target.strip("<>")).partition("#")
            resolved = ((ROOT / doc).parent / path).resolve() if path else ROOT / doc
            exists = resolved.exists()
            fragment_ok = True
            if exists and fragment and resolved.suffix == ".md":
                headings = re.findall(r"^#{1,6}\s+(.+)$", resolved.read_text(), re.M)
                slugs = [re.sub(r"[^\w\- ]", "", h.lower()).replace(" ", "-") for h in headings]
                fragment_ok = fragment in slugs
            links.append({"doc": doc, "target": target, "exists": exists, "fragmentValid": fragment_ok})
    broken = [r for r in links if not r.get("exists", True) or not r.get("fragmentValid", True)]
    return {"method": "manual inspection — oma docs verify did not run; deterministic local path/heading resolution",
            "links": links, "localLinkCount": sum("exists" in r for r in links), "broken": broken}


def finish():
    refs = references()
    save("references-after.json", refs)
    before = json.loads((OUT / "before.json").read_text())
    changed = [p for p, value in before.items() if not (ROOT / p).exists() or
               manifest([p])[p] != value]
    diff = git("diff", "--check", "--", *DOCS)
    save("boundary.json", {"trackedPathsChecked": len(before), "changedTrackedPaths": changed,
         "onlyOwnedDocsChanged": set(changed) <= set(DOCS), "diffCheck": diff,
         "referencesBroken": len(refs["broken"]), "documentHashes": manifest(DOCS),
         "postCollectionVerified": False})
    print(json.dumps({"changedTrackedPaths": changed, "brokenReferences": refs["broken"], "diffCheckExit": diff["exitCode"]}))
    assert set(changed) <= set(DOCS) and not refs["broken"] and diff["exitCode"] == 0


if __name__ == "__main__":
    {"preflight": preflight, "run": run, "finish": finish,
     "references-before": lambda: save("references-before.json", references())}[sys.argv[1]]()
