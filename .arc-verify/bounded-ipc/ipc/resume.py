"""Resume evidence collection; never rewrite preceding command receipts."""
import importlib.util
import json
import os
from pathlib import Path
import resource
import shutil
import sys

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("prior_recorder", HERE / "record.py")
r = importlib.util.module_from_spec(spec)
spec.loader.exec_module(r)
OUT = HERE / "resume-3"
OUT.mkdir(exist_ok=True)
r.OUT = OUT
REVIEW = r.ROOT.parents[3]


def prepare():
    shutil.copyfile(HERE / "delivery.md", OUT / "interrupted-delivery.md")
    original = json.loads((HERE / "provenance-before.json").read_text())
    hashes = []
    for entry in original["sourceHashes"]:
        path = entry["path"]
        digest = r.sha((r.ROOT / path).read_bytes())
        hashes.append(dict(entry, sha256=digest,
                           matchesOriginal=digest == entry["originalSha256"]))
    migration = original["migration"]["path"]
    baseline = r.command(["git", "show", r.BASE + ":" + migration])
    current = (r.ROOT / migration).read_bytes()
    refs = [r.BASE, r.OLD, "eee0c01750bcf852d691395774215b18628e89d7", "HEAD"]
    for attempt in (1, 2):
        for suffix in ("snapshot", "worker"):
            refs.append("refs/agent-valley/task-evidence/d6c6b45f4a6c3769-78686d2704d4d690-"
                        + str(attempt) + "/" + suffix)
    git = []
    for cwd in (r.ROOT, REVIEW):
        for ref in refs:
            git.append(r.command(["git", "rev-parse", "--verify", ref], cwd))
            git.append(r.command(["git", "rev-parse", "--verify", ref + "^{tree}"], cwd))
            git.append(r.command(["git", "merge-base", "--is-ancestor", r.BASE, ref], cwd))
        for ref in (r.BASE, r.OLD, "HEAD"):
            git.append(r.command(["git", "ls-tree", "-r", ref, "--", *r.FILES], cwd))
        git.append(r.command(["git", "status", "--porcelain=v1", "--untracked-files=all",
                              "--", *r.INPUTS, ".arc-verify/bounded-ipc/ipc"], cwd))
        git.append(r.command(["git", "diff", "--stat", r.BASE, "--", *r.INPUTS], cwd))
    r.save("provenance-before.json", {
        "recordedAt": r.now(), "actorWorkspace": str(r.ROOT),
        "reviewWorkspace": str(REVIEW), "sourceHashes": hashes,
        "gitReceipts": git, "productFingerprint": r.fingerprint(),
        "migration": {"path": migration, "baselineRead": baseline,
                      "baselineSha256": r.sha(baseline["stdout"].encode()),
                      "currentSha256": r.sha(current),
                      "byteIdentical": baseline["exitCode"] == 0
                      and baseline["stdout"].encode() == current},
        "reviewSourceHashes": [{"path": p,
                                "sha256": r.sha((REVIEW / p).read_bytes()),
                                "matchesActor": (REVIEW / p).read_bytes()
                                == (r.ROOT / p).read_bytes()} for p in r.FILES],
        "originalReportPreserved": r.sha((HERE / "original-delivery.md").read_bytes())
        == original["originalReport"]["sha256"],
    })
    safe_env = {k: r.ENV[k] for k in ("PATH", "HOME", "TMPDIR", "CARGO_HOME",
        "CARGO_TARGET_DIR", "CARGO_NET_OFFLINE", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN",
        "RUSTFLAGS", "CODEX_SESSION_ID", "AGENT_VALLEY_MANAGED_RUN") if k in r.ENV}
    r.save("execution-context.json", {
        "recordedAt": r.now(), "environment": safe_env,
        "tools": [r.command(a) for a in (["rustc", "-Vv"], ["cargo", "-Vv"],
                  ["rustup", "show", "active-toolchain"], ["uname", "-a"])],
        "cachePaths": {"cargoHome": r.ENV.get("CARGO_HOME", str(Path.home()/".cargo")),
                       "target": r.ENV.get("CARGO_TARGET_DIR", str(r.ROOT/"target"))},
        "identities": {"actorId": "backend", "taskId": "task-2", "attempt": 3,
            "missionId": "b137e253-def6-46ef-a75d-25ab414784ea",
            "omaRunId": None, "injectedClaimPath": None,
            "note": "No run ID/claim path injected; preserve absence, do not invent."},
        "budget": {"missionMaxDurationSeconds": 86400, "missionMaxParallel": 3,
            "missionMaxRuns": 200, "missionMaxRetries": 3, "actorAttempt": 3,
            "concurrentCargoCommands": 1, "commandTimeoutSeconds": 120,
            "note": "Continues interrupted task; no mission budgets or retries reset."},
        "hostLimits": {k: resource.getrlimit(getattr(resource, k)) for k in
                       ("RLIMIT_NOFILE", "RLIMIT_NPROC", "RLIMIT_AS", "RLIMIT_CPU")}})
    assert all(e["matchesOriginal"] for e in hashes)
    assert baseline["exitCode"] == 0 and baseline["stdout"].encode() == current
    print("Preserved earlier receipts; recorded current source/provenance/context.")


if __name__ == "__main__":
    action = sys.argv[1]
    if action == "prepare":
        prepare()
    elif action == "finish":
        r.finish()
    else:
        commands = {"fmt": ["cargo", "fmt", "--all", "--", "--check"],
            "clippy": ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
            "workspace-test": ["cargo", "test", "--workspace"]}
        r.gate(action, commands[action])
