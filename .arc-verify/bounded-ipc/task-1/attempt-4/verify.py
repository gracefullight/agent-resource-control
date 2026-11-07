"""Task-1 preservation and dependency checks; never writes product inputs."""
from pathlib import Path
import datetime
import hashlib
import json
import os
import platform
import stat
import subprocess
import time
import tomllib

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
BASE = "42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0"


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def save(name, value):
    (OUT / name).write_text(json.dumps(value, indent=2) + "\n")


def run(name, argv):
    started, tick = now(), time.monotonic()
    result = subprocess.run(argv, cwd=ROOT, capture_output=True, timeout=120)
    (OUT / (name + ".stdout")).write_bytes(result.stdout)
    (OUT / (name + ".stderr")).write_bytes(result.stderr)
    save(name + ".json", dict(argv=argv, cwd=str(ROOT), startedAt=started,
         finishedAt=now(), elapsedSeconds=time.monotonic()-tick,
         exitCode=result.returncode, stdoutSha256=sha(result.stdout),
         stderrSha256=sha(result.stderr)))
    return result


os.chdir(ROOT)
qa_dir = Path(".arc-verify/bounded-ipc/qa-review/attempt-2")
inherited = json.loads((qa_dir / "workspace-source.json").read_text())
paths = sorted({item["path"] for item in inherited["files"]} | {
    "docs/bounded-ipc-contract.md", "docs/bounded-ipc-targets.md",
    "docs/bounded-ipc-final.md", "docs/bounded-ipc-recovery.md",
    "docs/bounded-ipc-value-scope.md", "docs/runtime-identity-usage.md",
    "scripts/harness/validate.sh",
})


def snapshot():
    return [dict(path=p, sha256=sha(Path(p).read_bytes()),
                 mode=oct(stat.S_IMODE(Path(p).stat().st_mode))) for p in paths]


before = snapshot()
save("source-before.json", before)
save("execution-context.json", dict(
    recordedAt=now(), actor="technical-director", task="task-1", attempt=4,
    managedRun=os.environ.get("AGENT_VALLEY_MANAGED_RUN"),
    suppliedMaxDurationSec=86400, suppliedMaxRuns=200, suppliedMaxRetries=3,
    concurrentChildProcesses=1, agentsSpawned=0,
    budget="Inherited mission allocations unchanged. No new mission, run allocation or deadline reset; original start/deadline not injected.",
    checks="Serial read-only offline dependency and formatting checks; 120-second timeout per process; no build or runtime workload replay.",
    host=dict(system=platform.system(), release=platform.release(), machine=platform.machine()),
    sourceSearch="Serena configured, no exposed tools; known-path reads and Git only for source."))
assert run("head", ["git", "rev-parse", "HEAD"]).stdout.decode().strip() == BASE
assert run("dirty-before", ["git", "status", "--short"]).returncode == 0
assert run("manifest-diff", ["git", "diff", BASE, "--", "Cargo.toml", "Cargo.lock", "crates/arc-daemon/Cargo.toml"]).returncode == 0

access = dict(recordedAt=now(), path="av.yaml", method="Path.read_text: direct supported filesystem read")
try:
    config_text = Path("av.yaml").read_text()
except OSError as error:
    access.update(outcome="failed", errorType=type(error).__name__, errno=error.errno,
                  error=str(error), configuredTargetApplicabilityVerified=False,
                  containerHealthVerified=False, bypassAttempted=False, servicesRestarted=False)
else:
    access.update(outcome="readable", sha256=sha(config_text.encode()),
                  configuredTargetApplicabilityVerified=False,
                  note="Configuration became accessible; target assessment is required.")
save("configuration-access.json", access)

for name, argv in [
    ("cargo-version", ["cargo", "--version"]),
    ("rustc-version", ["rustc", "--version"]),
    ("components", ["rustup", "component", "list", "--installed"]),
    ("native-metadata", ["cargo", "metadata", "--offline", "--locked", "--filter-platform", "aarch64-apple-darwin", "--format-version", "1"]),
    ("native-fetch", ["cargo", "fetch", "--offline", "--locked", "--target", "aarch64-apple-darwin"]),
    ("fmt", ["cargo", "fmt", "--all", "--", "--check"]),
    ("diff-check", ["git", "diff", "--check", "--", "Cargo.toml", "Cargo.lock", "crates", "docs"]),
]:
    assert run(name, argv).returncode == 0, name

preserved = {}
for name in ["crates/arc-storage/migrations/0001_phase0.sql", "scripts/harness/validate.sh"]:
    frozen = subprocess.check_output(["git", "show", BASE + ":" + name])
    assert frozen == Path(name).read_bytes(), name
    preserved[name] = dict(byteIdentical=True, sha256=sha(frozen))
assert preserved["crates/arc-storage/migrations/0001_phase0.sql"]["sha256"] == "046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384"
old = tomllib.loads(subprocess.check_output(["git", "show", BASE + ":Cargo.lock"]).decode())
new = tomllib.loads(Path("Cargo.lock").read_text())
oldpk = {p["name"]: p for p in old["package"]}
newpk = {p["name"]: p for p in new["package"]}
assert len(oldpk) == len(old["package"]) == len(newpk) == len(new["package"])
assert oldpk.keys() == newpk.keys()
assert [n for n in oldpk if oldpk[n] != newpk[n]] == ["arc-daemon"]
expected = dict(oldpk["arc-daemon"])
expected["dependencies"] = sorted(expected["dependencies"] + ["serde_json"])
assert expected == newpk["arc-daemon"]
meta = json.loads((OUT / "native-metadata.stdout").read_text())
nodes = {n["id"].split("#")[-1]: n for n in meta["resolve"]["nodes"]}
assert len(meta["workspace_members"]) == 5
assert {"sync", "time", "test-util"} <= set(nodes["tokio@1.54.1"]["features"])
assert not {"io-util", "full"} & set(nodes["tokio@1.54.1"]["features"])
assert "hooks" in nodes["rusqlite@0.41.1"]["features"]
save("preservation.json", dict(baseline=BASE, files=preserved,
     packageVersionsAndChecksumsUnchanged=True, lockPackagesAdded=[],
     lockPackagesRemoved=[], onlyLockChange="arc-daemon -> existing serde_json",
     nativeDependenciesReady=True, workspaceMembers=5))

srs = Path("/Users/gracefullight/Downloads/Agent_Resource_Control_ARC_Requirements_SRS_2026-11-06.md")
srs_bytes = srs.read_bytes()
assert sha(srs_bytes) == "a8ef945251f9f3fd7c3e337d4efc77ff9914f95114212ca9e6c90ae2ce3e3d9e"
lines = srs_bytes.decode().splitlines()
assert "NFR-002" in lines[2425] and "50 MB" in lines[2425]
assert "NFR-004" in lines[2427] and "20ms" in lines[2427]
save("srs-source.json", dict(path=str(srs), sha256=sha(srs_bytes),
     copiedIntoWorktree=False, targetsRevalidated=True,
     idleRss=dict(requirement="NFR-002", line=2426, target="below 50 MB idle"),
     smallMessageP95=dict(requirement="NFR-004", line=2428, target="within 20 ms local IPC round trip"),
     versionInconsistencyRetained=True, measurementsPerformed=False))

bindings = []
current = {i["path"]: i for i in before}
assert all(current[item["path"]] == item for item in inherited["files"])
for label, code in [("fmt", 0), ("clippy", 0), ("workspace", 0), ("harness", 1)]:
    receipt = json.loads((qa_dir / (label + ".json")).read_text())
    assert sha((qa_dir / (label + ".txt")).read_bytes()) == receipt["outputSha256"]
    assert receipt["sourceBefore"] == receipt["sourceAfter"] == inherited["sha256"]
    assert receipt["exitCode"] == code
    bindings.append(dict(receipt=str(qa_dir / (label + ".json")), argv=receipt["argv"],
         originalStartedAt=receipt["startedAt"], exitCode=code, outputHashMatches=True,
         sourceFingerprint=inherited["sha256"], replayed=False))
save("inherited-gates.json", dict(productFilesMatched=len(inherited["files"]),
     fingerprint=inherited["sha256"], gates=bindings,
     evidenceBoundary="Historical source-matched command receipts; no runtime or benchmark rerun."))

task6_path = ".agents/results/result-design-director-task-6-attempt-3-local.md"
task6_manifest = json.loads(Path(".arc-verify/bounded-ipc/task-6/attempt-3/delivery.json").read_text())
entry = task6_manifest["files"][task6_path]
save("outside-ownership.json", dict(recordedAt=now(), task6MissingPath=task6_path,
     present=Path(task6_path).is_file(), indexedEntry=entry,
     owner="Design Director/task-6, coordinated by Chief Director",
     disposition="No ownership handoff supplied; no restoration or changes to task-6 manifest/report attempted.",
     configuredTargetOwner="Chief Director: supported configuration evidence and, if applicable, authorized health/availability/recent-error observations"))
after = snapshot()
save("source-after.json", after)
assert before == after
save("checks.json", dict(recordedAt=now(), status="passed", sourceFilesChecked=len(paths),
     preservedAllCheckedBytesAndModes=True, nativeOfflineDependenciesReady=True,
     formattingPassed=True, inheritedProductFilesMatched=len(inherited["files"]),
     configuredTargetApplicabilityVerified=False,
     missionCompletionClaimed=False, skills=["oma-architecture", "oma-backend"]))
print(json.dumps(dict(checks="passed", sourceFilesUnchanged=len(paths),
     inheritedProductFilesMatched=len(inherited["files"]), configuration=access["outcome"],
     task6ReportPresent=Path(task6_path).is_file())))
