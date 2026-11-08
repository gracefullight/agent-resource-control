"""Serial recovery checks. Historical command receipts and measurements are read-only."""
import datetime
import hashlib
import json
from pathlib import Path
import subprocess
import time

ROOT = Path.cwd().resolve()
OUT = ROOT / ".arc-verify/rust-coverage/process"
RECOVERY = json.loads((OUT / "recovery-reconciliation.json").read_text())
OLD = Path(RECOVERY["retainedWorkspace"])
RUN = ROOT / RECOVERY["retainedMeasurement"]["directory"]

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

# Every retained source/config input is still exact in its original workspace.
inputs = json.loads((RUN / "inputs.json").read_text())
for name, expected in inputs["files"].items():
    assert digest(OLD / name) == expected, name
for row in RECOVERY["rustFiles"]:
    for base in [ROOT, OLD]:
        path = base / row["path"]
        assert path.stat().st_size == row["bytes"] and digest(path) == row["sha256"]
for name, expected in RECOVERY["immutableMeasurementFiles"].items():
    for base in [ROOT, OLD]:
        path = base / name
        assert path.stat().st_size == expected["bytes"] and digest(path) == expected["sha256"]
for group in ["preservedPeers", "preservedExistingRuns"]:
    for name, expected in RECOVERY[group].items():
        path = ROOT / name
        assert path.stat().st_size == expected["bytes"] and digest(path) == expected["sha256"]
pointer = ROOT / RECOVERY["sharedPointer"]["path"]
assert digest(pointer) == RECOVERY["sharedPointer"]["sha256"]
assert hashlib.sha256(subprocess.check_output(["git", "show", ":" + str(pointer.relative_to(ROOT))])).hexdigest() == RECOVERY["sharedPointer"]["indexSha256"]
assert subprocess.check_output(["git", "diff", RECOVERY["retainedInputSnapshot"], "--", str(pointer.relative_to(ROOT))], cwd=OLD) == b""

# Recheck actual raw profiles against the full merge, without overwriting proof.
profiles = json.loads((RUN / "profiles.json").read_text())
raw = {row["path"]: row for row in profiles["profiles"]}
for name in profiles["merge"]:
    path = Path(name)
    assert digest(path) == raw[path.name]["sha256"] and path.stat().st_size == raw[path.name]["size"]
proof = json.loads((OUT / "child-profile-proof.json").read_text())
children = json.loads((OUT / "instrumented/coverage-lifecycle.json").read_text())
assert len({child["pid"] for child in children["children"]}) == 5
template = children["inheritedProfileTemplate"]
if isinstance(template, dict):
    template = bytes(template["Unix"]).decode()
assert "%p" in template and "%m" in template
child_paths = []
for child in children["children"]:
    assert child["reaped"] and child["profiles"]
    for name in child["profiles"]:
        assert name in profiles["merge"] and f"arc-{child['pid']}-" in Path(name).name
        child_paths.append(name)
assert len(set(child_paths)) == len(child_paths) == proof["profileCount"] == 5
for name, expected in proof["artifacts"].items():
    assert digest(ROOT / name) == expected, name

inventory = {row["path"]: row for row in json.loads((RUN / "source-inventory.json").read_text())}
def covered(path):
    result = set()
    source = None
    for line in path.read_text().splitlines():
        if line.startswith("SF:"):
            try:
                source = str(Path(line[3:]).resolve().relative_to(OLD.resolve()))
            except ValueError:
                source = None
        elif line.startswith("DA:") and source in inventory:
            row = inventory[source]
            number, hits = map(int, line[3:].split(",")[:2])
            if not row["testOnly"] and number not in row["omitted"] and hits > 0:
                result.add((source, number))
    return result
child_lines = covered(OUT / "child-only.lcov")
full_lines = covered(RUN / "raw.lcov")
assert len(child_lines) == proof["childOnlyCoveredProductionLines"] == 1594
assert child_lines <= full_lines
summary = json.loads((RUN / "summary.json").read_text())
for row in summary["files"]:
    actual = {line for source, line in full_lines if source == row["path"]}
    assert len(actual) == row["covered"], row["path"]
assert len(summary["crates"]) == 6 and all(row["covered"] * 100 >= row["total"] * 80 for row in summary["crates"])
result = {"pass": True, "originalMeasurementSourceHash": inputs["sha256"],
          "retainedInputCount": len(inputs["files"]), "immutableMeasurementFileCount": len(RECOVERY["immutableMeasurementFiles"]),
          "verifiedRawProfileCount": len(raw), "mergeInputCount": len(profiles["merge"]),
          "childProfileCount": len(child_paths), "childOnlyCanonicalCoveredLines": len(child_lines),
          "sharedPointerUnchanged": True, "retainedPointerEqualsOwnInput": True,
          "historicalEvidenceScope": "isolated original workspace; integrated measurement remains task-6", "crates": summary["crates"]}
(OUT / "recovery-proof.json").write_text(json.dumps(result, indent=2) + "\n")

commands = [
    (["bun", "-e", "import { inputIdentity } from './scripts/coverage/source.ts'; import { checkPreservation } from './scripts/coverage/preservation.ts'; console.log(JSON.stringify({identity:inputIdentity(process.cwd()),preservation:checkPreservation(process.cwd())}));"], OLD, "isolated-source-preservation"),
    (["cargo", "test", "-p", "arc-cli"], ROOT, "cli"),
    (["cargo", "test", "-p", "arc-daemon", "--test", "resident", "coverage::", "--", "--nocapture"], ROOT, "resident"),
    (["cargo", "test", "-p", "arc-daemon", "--bin", "arc-daemon", "runtime_report::tests::invalid_report_names"], ROOT, "unit"),
    (["cargo", "fmt", "--all", "--", "--check"], ROOT, "fmt"),
    (["bun", "test", "scripts/coverage/preservation.test.ts", "scripts/coverage/gate.test.ts", "scripts/coverage/accounting.test.ts"], ROOT, "gate-preservation"),
]
receipts = []
for argv, cwd, name in commands:
    started = datetime.datetime.now(datetime.timezone.utc).isoformat()
    begin = time.monotonic()
    child = subprocess.Popen(argv, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    try:
        stdout, _ = child.communicate(timeout=120)
        exit_code = child.returncode
    except subprocess.TimeoutExpired:
        child.kill()
        stdout, _ = child.communicate()
        exit_code = child.returncode
    (OUT / ("recovery-" + name + ".txt")).write_bytes(stdout)
    if name == "isolated-source-preservation" and exit_code == 0:
        measured = json.loads(stdout)
        assert measured["identity"] == inputs
        assert measured["preservation"]["pass"] is True
    receipts.append({"argv": argv, "cwd": str(cwd), "pid": child.pid, "started": started,
                     "ended": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                     "exit": exit_code, "elapsedSeconds": time.monotonic() - begin,
                     "log": ".arc-verify/rust-coverage/process/recovery-" + name + ".txt",
                     "logSha256": hashlib.sha256(stdout).hexdigest()})
    (OUT / "recovery-commands.json").write_text(json.dumps(receipts, indent=2) + "\n")
    print(name, "exit", exit_code, flush=True)
    assert exit_code == 0, stdout.decode(errors="replace")[-6000:]
print(json.dumps(result), flush=True)
