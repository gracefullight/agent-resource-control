#!/usr/bin/env python3
"""Validate the baseline handoff against Git bytes and lossless receipts."""
import datetime as dt
import json
from pathlib import Path
import re
import runpy

HERE = Path(__file__).resolve().parent
api = runpy.run_path(str(HERE / "capture.py"))
git, sha = api["git"], api["sha"]
load = lambda name: json.loads((HERE / name).read_text())
dump = api["write_json"]
baseline = load("baseline.json")
revision = baseline["acceptedRevision"]
assert git("rev-parse", "HEAD").decode().strip() == revision
sources = load("source-files.json")
for item in sources:
    original = git("show", f"{revision}:{item['path']}")
    current = (api["ROOT"] / item["path"]).read_bytes()
    assert sha(original) == item["sha256"] == sha(current), item["path"]
    assert original.count(b"\n") == item["lines"]
assert all(t["lines"] > 500 for t in baseline["targets"])
assert api["source_fingerprint"]() == baseline["fingerprint"]["value"]

inventory = load("inventory.json")
assert len({f["id"] for f in inventory}) == len(inventory)
for path, first in [(api["RESIDENT"], 1), (api["IPC"], 355), (api["SUPPORT"], 1)]:
    functions, assertions, attributes = api["inventory"](path, first)
    assert functions == [f for f in inventory if f["path"] == path]
    assert assertions == [a for a in load("assertions.json") if a["path"] == path]
    assert attributes == [a for a in load("attributes.json") if a["path"] == path]
    # Independent direct attribute count catches missing test entries.
    text = git("show", f"{revision}:{path}").decode().splitlines()
    test_attributes = sum(line.strip() == "#[test]" for line in text[first - 1:])
    assert test_attributes == sum(f["kind"] == "test" for f in functions)

def output(receipt):
    chunks = []
    for part in receipt["outputParts"]:
        data = (HERE / part["path"]).read_bytes()
        assert sha(data) == part["sha256"]
        assert data.count(b"\n") == part["lines"]
        chunks.append(data)
    data = b"".join(chunks)
    assert sha(data) == receipt["outputSha256"]
    assert len(data) == receipt["outputBytes"]
    return data.decode()


names = ["global-status", "scoped-worktree-diff", "scoped-staged-diff", "scoped-untracked",
         "toolchain", "cargo-version", "components", "cargo-metadata", "ipc-discovery",
         "cli-discovery", "resident-discovery", "ipc-baseline", "cli-baseline", "resident-baseline"]
receipts = []
for name in names:
    receipt = load(name + ".json")
    text = output(receipt)
    assert receipt["exitCode"] == 0 and not receipt["timedOut"], name
    if "sourceFingerprintBefore" in receipt:
        assert receipt["sourceFingerprintBefore"] == baseline["fingerprint"]["value"]
        assert receipt["sourceFingerprintAfter"] == baseline["fingerprint"]["value"]
        assert receipt["acceptedRevision"] == revision
        assert receipt["processReaped"]
    if name.startswith("scoped-"):
        assert text == "", name
    receipts.append({"name": name, "receipt": name + ".json", "command": receipt["command"],
                     "exitCode": receipt["exitCode"], "outputSha256": receipt["outputSha256"],
                     "sourceBinding": "per-command-before-and-after" if "sourceFingerprintBefore" in receipt
                     else "accepted-capture-and-unchanged-final-audit"})
assert "av.yaml: Operation not permitted" in output(load("global-status.json"))
for target, receipt_name in [(api["IPC"], "ipc-discovery"), (api["RESIDENT"], "resident-discovery")]:
    discovered = re.findall(r"^(.+): test$", output(load(receipt_name + ".json")), re.MULTILINE)
    expected = [f["qualifiedName"] for f in inventory if f["path"] == target and f["kind"] == "test"]
    assert sorted(discovered) == sorted(expected), receipt_name
for name, count in [("ipc-baseline", 13), ("cli-baseline", 2), ("resident-baseline", 11)]:
    text = output(load(name + ".json"))
    assert f"test result: ok. {count} passed; 0 failed; 0 ignored" in text
    assert "0 filtered out" in text
api["write_rows"]("commands.json", receipts)
baseline["baselineChecks"] = {"ipcPassed": 13, "cliPassed": 2, "residentPassed": 11,
                             "discoveryNamesVerified": True, "receiptIndex": "commands.json",
                             "fullWorkspaceAndHarness": "not run in this baseline task"}
baseline["finalSourceAudit"] = {"allCapturedFilesByteIdentical": True, "capturedFiles": len(sources),
                               "sourceFingerprint": api["source_fingerprint"](),
                               "globalCleanlinessVerified": False}
dump("baseline.json", baseline)
files = sorted(p for p in HERE.iterdir() if p.is_file())
for file in files:
    data = file.read_bytes()
    assert data.count(b"\n") <= 500 and len(data.splitlines()) <= 500, file.name
result = {"status": "completed", "task": "task-1", "acceptedRevision": revision,
          "checkedAt": dt.datetime.now(dt.timezone.utc).isoformat(),
          "sourceFingerprint": baseline["fingerprint"]["value"], "sourceFilesByteIdentical": len(sources),
          "testInventoryCount": sum(f["kind"] == "test" for f in inventory),
          "helperInventoryCount": sum(f["kind"] == "helper" for f in inventory),
          "assertionSites": len(load("assertions.json")), "attributes": len(load("attributes.json")),
          "receiptCount": len(receipts), "allReceiptOutputsVerified": True,
          "focusedTestsPassed": 26, "productFilesChanged": [],
          "limitations": ["av.yaml access denied; global cleanliness unverified",
                          "Serena unavailable; known-path fallback",
                          "No coverage, mutation or flakiness measurement",
                          "No injected OMA claim/run/session identity"],
          "command": ["python3", ".arc-verify/module-conformance/baseline/audit.py"],
          "exitCode": 0}
dump("audit-result.json", result)
manifest = []
for file in sorted(HERE.iterdir()):
    if not file.is_file() or file.name == "artifacts.json":
        continue
    data = file.read_bytes()
    assert data.count(b"\n") <= 500 and len(data.splitlines()) <= 500, file.name
    manifest.append({"path": str(file.relative_to(api["ROOT"])), "bytes": len(data),
                     "lines": data.count(b"\n"), "logicalLines": len(data.splitlines()), "sha256": sha(data)})
api["write_rows"]("artifacts.json", manifest)
assert len(manifest) + 2 <= 500
print(json.dumps(result, indent=2))
