"""Recheck lifecycle child profiles against the full merge and export them alone."""
import hashlib
import json
from pathlib import Path
import subprocess
import time

ROOT = Path.cwd().resolve()
OUT = ROOT / ".arc-verify/rust-coverage/process"
latest = json.loads((OUT / "measurement.json").read_text())
assert latest["pass"] is True
run = ROOT / latest["directory"]
receipt = json.loads((run / "receipt.json").read_text())
assert receipt["pass"] is True
proof = json.loads((OUT / "instrumented/coverage-lifecycle.json").read_text())
profiles = json.loads((run / "profiles.json").read_text())
template = proof["inheritedProfileTemplate"]
if isinstance(template, dict):
    # serde preserves OsString as native Unix bytes rather than assuming UTF-8.
    template = bytes(template["Unix"]).decode("utf-8")
assert "%p" in template and "%m" in template
assert len({child["pid"] for child in proof["children"]}) == 5
inventory = {item["path"]: item for item in json.loads((run / "source-inventory.json").read_text())}
raw_inventory = {item["path"]: item for item in profiles["profiles"]}
inputs = []
for child in proof["children"]:
    assert child["reaped"] is True and child["profiles"]
    for name in child["profiles"]:
        path = Path(name)
        assert f"arc-{child['pid']}-" in path.name
        assert name in profiles["merge"]
        assert hashlib.sha256(path.read_bytes()).hexdigest() == raw_inventory[path.name]["sha256"]
        inputs.append(name)
assert len(set(inputs)) == len(inputs)

versions = json.loads((run / "versions.json").read_text())
sysroot = subprocess.check_output(["rustup", "run", "1.95.0", "rustc", "--print", "sysroot"], text=True).strip()
llvm = Path(sysroot) / "lib/rustlib" / versions["host"] / "bin"
merged = OUT / "child-only.profdata"
raw = OUT / "child-only.lcov"
commands = []


def execute(argv, output=None):
    started = time.time()
    result = subprocess.run([str(arg) for arg in argv], capture_output=True, timeout=60)
    commands.append({"argv": [str(arg) for arg in argv], "exit": result.returncode,
                     "elapsedSeconds": time.time() - started, "stderr": result.stderr.decode()})
    if output:
        output.write_bytes(result.stdout)
    (OUT / "child-only-commands.json").write_text(json.dumps(commands, indent=2) + "\n")
    assert result.returncode == 0, result.stderr.decode()


execute([llvm / "llvm-profdata", "merge", "-sparse", "-o", merged, *inputs])
target = Path(profiles["target"]) / "debug"
execute([llvm / "llvm-cov", "export", "--format=lcov", f"--instr-profile={merged}",
         target / "arc-daemon", "--object", target / "arcctl"], raw)


def covered_lines(path):
    result = set()
    source = None
    for line in path.read_text().splitlines():
        if line.startswith("SF:"):
            try:
                source = str(Path(line[3:]).resolve().relative_to(ROOT))
            except ValueError:
                source = None
        elif line.startswith("DA:") and source in inventory:
            item = inventory[source]
            number, hits = map(int, line[3:].split(",")[:2])
            if not item["testOnly"] and number not in item["omitted"] and hits > 0:
                result.add((source, number))
    return result


child_lines = covered_lines(raw)
full_lines = covered_lines(run / "raw.lcov")
assert child_lines and child_lines <= full_lines
files = {}
for source, number in sorted(child_lines):
    if source.startswith(("crates/arc-cli/", "crates/arc-daemon/")):
        files.setdefault(source, []).append(number)
assert any(source.startswith("crates/arc-cli/") for source in files)
assert any(source.startswith("crates/arc-daemon/") for source in files)
result = {"pass": True, "runId": latest["runId"], "sourceHash": latest["sourceHash"],
          "measurementDirectory": latest["directory"], "childCount": len(proof["children"]),
          "profileCount": len(inputs), "distinctInheritedDestinations": True,
          "allChildrenReaped": True, "allChildProfilesInFullMerge": True,
          "childOnlyCoveredProductionLines": len(child_lines),
          "allChildCoveredLinesInCanonicalFullReport": True,
          "daemonAndCliChildCoveredLines": files,
          "artifacts": {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest()
                        for path in [raw, merged, run / "raw.lcov", run / "raw.json",
                                     OUT / "instrumented/coverage-lifecycle.json"]}}
(OUT / "child-profile-proof.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps({key: value for key, value in result.items()
                  if key not in ["daemonAndCliChildCoveredLines", "artifacts"]}, indent=2))
