"""Task-2 evidence-only repair. Never writes product files or Git refs."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import resource
import subprocess
import sys
import time

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[3]
REVIEW = ROOT.parents[3]
BASE = "42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0"
OLD = "3816bafaa684c8099dad78be2d0cb0e474c5f08b"
SNAP = "eac1ffc1739f3cc3da254376b8243c7e05012087"
INPUTS = ["crates", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "scripts/harness/validate.sh"]
ENV = dict(os.environ, CARGO_NET_OFFLINE="true", CARGO_TARGET_DIR=str(REVIEW / "target"))


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def save(name, data):
    (OUT / name).write_text(json.dumps(data, indent=2) + "\n")


def command(argv, cwd=ROOT):
    started = now()
    p = subprocess.run(argv, cwd=cwd, env=ENV, capture_output=True, timeout=20)
    return dict(argv=argv, cwd=str(cwd), startedAt=started, finishedAt=now(),
                exitCode=p.returncode, stdout=p.stdout.decode(errors="replace"),
                stderr=p.stderr.decode(errors="replace"))


def fingerprint(cwd=ROOT):
    result = command(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z", "--", *INPUTS], cwd)
    if result["exitCode"]:
        raise RuntimeError(result)
    files = []
    for path in sorted(set(result["stdout"].split("\0")) - {""}):
        p = cwd / path
        files.append(dict(path=path, sha256=sha(p.read_bytes()), mode=oct(p.stat().st_mode & 0o777)))
    return dict(algorithm="sha256 of json.dumps(files, sort_keys=True)", scope=INPUTS,
                files=files, sha256=sha(json.dumps(files, sort_keys=True).encode()))


def prepare():
    prior = json.loads((OUT.parent / "evidence.json").read_text())
    files = [e["path"] for e in prior["sourceHashes"]]
    context = dict(recordedAt=now(), actorId="backend", taskId="task-2", attempt=4,
                   missionId="b137e253-def6-46ef-a75d-25ab414784ea",
                   sessionId=os.environ.get("CODEX_SESSION_ID"), omaRunId=None, injectedClaimPath=None,
                   identityNote="No OMA run ID or claim path was injected. Absence is preserved.",
                   environment={k: ENV[k] for k in ("PATH", "TMPDIR", "CARGO_HOME", "CARGO_TARGET_DIR", "CARGO_NET_OFFLINE", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN", "RUSTFLAGS", "AGENT_VALLEY_MANAGED_RUN") if k in ENV},
                   toolchain=[command(a) for a in (["rustc", "-Vv"], ["cargo", "-Vv"], ["rustup", "show", "active-toolchain"], ["uname", "-a"])],
                   budget=dict(missionMaxDurationSec=86400, missionMaxParallel=3, missionMaxRuns=200,
                               missionMaxRetries=3, actorAttempt=4, commandTimeoutSec=120,
                               concurrentCargoCommands=1, absoluteMissionDeadline=None,
                               note="Existing mission budgets retained; exact start/deadline not injected. No mission retry, run, or state reset."),
                   hostLimits={k: resource.getrlimit(getattr(resource, k)) for k in ("RLIMIT_NOFILE", "RLIMIT_NPROC", "RLIMIT_AS", "RLIMIT_CPU")})
    save("execution-context.json", context)
    refs = [BASE, OLD, SNAP, "HEAD"]
    for attempt in (1, 2, 4):
        refs += [f"refs/agent-valley/task-evidence/d6c6b45f4a6c3769-78686d2704d4d690-{attempt}/{suffix}" for suffix in ("snapshot", "worker")]
    receipts = []
    for cwd in (ROOT, REVIEW):
        for ref in refs:
            for args in (["rev-parse", "--verify", ref], ["rev-parse", "--verify", ref + "^{tree}"], ["merge-base", "--is-ancestor", BASE, ref]):
                receipts.append(command(["git", *args], cwd))
        for ref in (BASE, OLD, SNAP, "HEAD"):
            receipts.append(command(["git", "ls-tree", "-r", ref, "--", *files], cwd))
        for args in (["status", "--porcelain=v1", "--untracked-files=all"],
                     ["status", "--porcelain=v1", "--untracked-files=all", "--", *INPUTS],
                     ["diff", "--stat", BASE, "--", *INPUTS],
                     ["diff", "--check", "--", *INPUTS]):
            receipts.append(command(["git", *args], cwd))
    hashes = []
    for e in prior["sourceHashes"]:
        p = e["path"]
        current = (ROOT / p).read_bytes()
        snap = command(["git", "show", SNAP + ":" + p])
        hashes.append(dict(path=p, sha256=sha(current), originalSha256=e["originalSha256"],
                           matchesOriginal=sha(current) == e["originalSha256"],
                           matchesInputSnapshot=snap["exitCode"] == 0 and snap["stdout"].encode() == current,
                           reviewSha256=sha((REVIEW / p).read_bytes()), matchesReview=(REVIEW / p).read_bytes() == current))
    frozen = []
    for p in ("crates/arc-storage/migrations/0001_phase0.sql", "scripts/harness/validate.sh"):
        b = command(["git", "show", BASE + ":" + p])
        frozen.append(dict(path=p, baselineRead=b, baselineSha256=sha(b["stdout"].encode()), currentSha256=sha((ROOT / p).read_bytes()), byteIdentical=b["exitCode"] == 0 and b["stdout"].encode() == (ROOT / p).read_bytes()))
    originals = []
    for p in sorted(OUT.parent.rglob("*")):
        if p.is_file() and OUT not in p.parents and "__pycache__" not in p.parts:
            originals.append(dict(path=str(p.relative_to(ROOT)), sha256=sha(p.read_bytes())))
    save("provenance-before.json", dict(recordedAt=now(), acceptedBaseline=BASE,
         expectedBaselineTree="c802f71ac4d91ed6b8a602d5c83bf9cf57116b76",
         injectedSnapshot=SNAP, expectedSnapshotTree="4214462f01a3c059d825ca6ae7ed90dc156ed086",
         gitReceipts=receipts, sourceHashes=hashes, preservedArtifacts=originals, frozenFiles=frozen,
         actorFingerprint=fingerprint(), reviewFingerprint=fingerprint(REVIEW)))
    print(json.dumps(dict(prepared=True, sourceHashes=hashes, frozenFiles=[{k:v for k,v in e.items() if k != "baselineRead"} for e in frozen])))


def gate(name):
    commands = {"fmt": ["cargo", "fmt", "--all", "--", "--check"],
                "clippy": ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
                "workspace-test": ["cargo", "test", "--workspace"]}
    argv = commands[name]
    before = fingerprint()
    started, tick = now(), time.monotonic()
    timed_out = False
    with (OUT / (name + ".txt")).open("xb") as log:
        p = subprocess.Popen(argv, cwd=ROOT, env=ENV, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        save(name + ".running.json", dict(argv=argv, cwd=str(ROOT), startedAt=started, pid=p.pid, processGroup=p.pid, timeoutSec=120))
        try:
            code = p.wait(timeout=120)
        except subprocess.TimeoutExpired:
            import signal
            timed_out = True
            os.killpg(p.pid, signal.SIGTERM)
            try:
                code = p.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(p.pid, signal.SIGKILL)
                code = p.wait(timeout=5)
    after = fingerprint()
    save(name + ".json", dict(argv=argv, cwd=str(ROOT), startedAt=started, finishedAt=now(), elapsedSeconds=time.monotonic()-tick,
         exitCode=code, timedOut=timed_out, pid=p.pid, processExitObserved=True, context="execution-context.json",
         output=name + ".txt", outputIncludes="full combined stdout/stderr", outputSha256=sha((OUT / (name + ".txt")).read_bytes()),
         sourceBefore=before["sha256"], sourceAfter=after["sha256"], productInputsUnchanged=before == after))
    print((OUT / (name + ".json")).read_text())


if __name__ == "__main__":
    if sys.argv[1] == "prepare":
        prepare()
    else:
        gate(sys.argv[1])
