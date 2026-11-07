"""Task-1 resume checks. No product writes, service operations or network access."""
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
    receipt = dict(argv=argv, cwd=str(ROOT), startedAt=started, finishedAt=now(),
                   elapsedSeconds=time.monotonic()-tick, exitCode=result.returncode,
                   stdoutSha256=sha(result.stdout), stderrSha256=sha(result.stderr))
    save(name + ".json", receipt)
    return result


os.chdir(ROOT)
inherited = json.loads(Path(".arc-verify/bounded-ipc/qa-review/workspace-source.json").read_text())
owned = ["docs/bounded-ipc-contract.md", "docs/bounded-ipc-targets.md"]
paths = sorted({item["path"] for item in inherited["files"]} | set(owned) |
               {"scripts/harness/validate.sh"})


def snapshot():
    return [dict(path=p, sha256=sha(Path(p).read_bytes()),
                 mode=oct(stat.S_IMODE(Path(p).stat().st_mode))) for p in paths]


before = snapshot()
save("source-before.json", before)
save("execution-context.json", dict(
    recordedAt=now(), actor="technical-director", task="task-1", attempt=3,
    nativeSession=os.environ.get("CODEX_SESSION_ID"), managedRun=os.environ.get("AGENT_VALLEY_MANAGED_RUN"),
    suppliedMaxDurationSec=86400, suppliedMaxRuns=200, suppliedMaxRetries=3,
    parallelismThisAttempt=1, delegation=False,
    budget="Existing mission limits retained; no new mission, deadline, retry or run allocation.",
    checks="Read-only native dependency and formatting checks, each with a 120-second process timeout.",
    host=dict(system=platform.system(), release=platform.release(), machine=platform.machine()),
    sourceSearch="No Serena tools exposed. Known-path reads/Git only for source; non-code documentation search permitted."))
assert run("head", ["git", "rev-parse", "HEAD"]).stdout.decode().strip() == BASE
run("dirty-before", ["git", "status", "--short"])
run("manifest-diff", ["git", "diff", BASE, "--", "Cargo.toml", "Cargo.lock", "crates/arc-daemon/Cargo.toml"])
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

migration = "crates/arc-storage/migrations/0001_phase0.sql"
frozen = subprocess.check_output(["git", "show", BASE + ":" + migration])
assert frozen == Path(migration).read_bytes()
assert sha(frozen) == "046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384"
harness = "scripts/harness/validate.sh"
assert subprocess.check_output(["git", "show", BASE + ":" + harness]) == Path(harness).read_bytes()
old = tomllib.loads(subprocess.check_output(["git", "show", BASE + ":Cargo.lock"]).decode())
new = tomllib.loads(Path("Cargo.lock").read_text())
oldpk = {p["name"]:p for p in old["package"]}
newpk = {p["name"]:p for p in new["package"]}
assert len(oldpk) == len(old["package"]) == len(newpk) == len(new["package"])
assert oldpk.keys() == newpk.keys()
changed_packages = [n for n in oldpk if oldpk[n] != newpk[n]]
assert changed_packages == ["arc-daemon"]
expected = dict(oldpk["arc-daemon"])
expected["dependencies"] = sorted(expected["dependencies"] + ["serde_json"])
assert expected == newpk["arc-daemon"]
meta = json.loads((OUT / "native-metadata.stdout").read_text())
nodes = {n["id"].split("#")[-1]: n for n in meta["resolve"]["nodes"]}
assert len(meta["workspace_members"]) == 5
assert {"sync", "time", "test-util"} <= set(nodes["tokio@1.54.1"]["features"])
assert "io-util" not in nodes["tokio@1.54.1"]["features"]
assert "hooks" in nodes["rusqlite@0.41.1"]["features"]
srs = Path("/Users/gracefullight/Downloads/Agent_Resource_Control_ARC_Requirements_SRS_2026-11-06.md")
srs_hash = sha(srs.read_bytes())
assert srs_hash == "a8ef945251f9f3fd7c3e337d4efc77ff9914f95114212ca9e6c90ae2ce3e3d9e"
save("preservation.json", dict(recordedAt=now(), baseline=BASE,
    migrationSha256=sha(frozen), migrationByteIdentical=True,
    harnessSha256=sha(Path(harness).read_bytes()), harnessByteIdentical=True,
    lockPackagesAdded=[], lockPackagesRemoved=[], lockPackagesChanged=changed_packages,
    packageVersionsAndChecksumsUnchanged=True, onlyLockChange="arc-daemon -> existing serde_json",
    nativeWorkspaceMembers=5, tokioVersion="1.54.1", rusqliteVersion="0.41.1",
    srsPath=str(srs), srsSha256=srs_hash, srsComparedInThisAttempt=False))
access = dict(recordedAt=now(), path="av.yaml", method="Path.read_text (direct supported filesystem read)")
try:
    Path("av.yaml").read_text()
except OSError as error:
    access.update(outcome="failed", errorType=type(error).__name__, errno=error.errno, error=str(error),
                  targetExistenceEstablished=False, containerHealthVerified=False,
                  bypassAttempted=False, servicesRestarted=False)
else:
    access.update(outcome="readable", note="Configuration became readable; requires target assessment before closure.")
save("configuration-access.json", access)
missing = [p for p in [".agents/results/task-6-design/workspace-test.json",
    ".agents/results/task-6-design/workspace-test.txt", ".agents/results/task-6-design/checks.json"] if not Path(p).is_file()]
save("outside-ownership.json", dict(recordedAt=now(), task6ReceiptPathsMissing=missing,
    disposition="Report to Chief for documentation Actor recovery; no task-6 or mission-state edits."))
bindings = []
for label in ["fmt", "clippy", "workspace", "harness"]:
    directory = Path(".arc-verify/bounded-ipc/qa-review")
    receipt = json.loads((directory / (label + ".json")).read_text())
    matches = sha((directory / (label + ".txt")).read_bytes()) == receipt["outputSha256"]
    assert matches
    bindings.append(dict(command=receipt["argv"], originalStartedAt=receipt["startedAt"],
        exitCode=receipt["exitCode"], outputHashMatches=True,
        sourceFingerprint=receipt["sourceAfter"], verification="Receipt/hash inspection only; not re-executed here."))
current_by_path = {i["path"]:i for i in before}
assert all(current_by_path[item["path"]] == item for item in inherited["files"])
save("inherited-gates.json", dict(recordedAt=now(), productFilesMatched=len(inherited["files"]),
    inheritedFingerprint=inherited["sha256"], gates=bindings))
after = snapshot()
save("source-after.json", after)
assert before == after
save("checks.json", dict(recordedAt=now(), status="passed", sourceFilesChecked=len(paths),
    preservedAllCheckedBytesAndModes=True, preservation=True, nativeOfflineDependenciesReady=True,
    formattingPassed=True, contractReview="Known-path source inspection; no runtime or benchmark rerun.",
    configurationObligation="unresolved" if access["outcome"] == "failed" else "assessment required",
    missingOutsideOwnershipReceipts=missing))
print(json.dumps(dict(checks="passed", sourceFilesUnchanged=len(paths),
    nativeOfflineDependencies="ready", configuration=access["outcome"], task6Missing=missing)))
