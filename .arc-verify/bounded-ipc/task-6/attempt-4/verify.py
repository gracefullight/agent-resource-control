"""Read-only task-6 delivery verifier; never changes files, refs or mission state."""
import hashlib
import json
import re
import subprocess
from pathlib import Path
from urllib.parse import unquote

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[3]
failures = []

def require(condition, description):
    if not condition:
        failures.append(description)

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

index = json.loads((OUT / "artifacts.json").read_text())
for relative, record in index["files"].items():
    path = ROOT / relative
    require(path.is_file() and not path.is_symlink(), "Missing/nonregular artifact: " + relative)
    if path.is_file() and not path.is_symlink():
        require(sha(path) == record["sha256"], "Artifact hash mismatch: " + relative)
        require(path.stat().st_size == record["bytes"], "Artifact size mismatch: " + relative)

before = json.loads((OUT / "source-before.json").read_text())
after = json.loads((OUT / "source-after.json").read_text())
require(before == after, "Test inputs changed during execution")
for relative, record in after.items():
    path = ROOT / relative
    require(path.is_file() and not path.is_symlink(), "Missing/nonregular source: " + relative)
    if path.is_file() and not path.is_symlink():
        require(sha(path) == record["sha256"], "Receiving source differs: " + relative)
        require(oct(path.stat().st_mode & 0o777) == record["mode"], "Source mode differs: " + relative)

receipt = json.loads((OUT / "workspace-test.json").read_text())
require(receipt["argv"] == ["cargo", "test", "--workspace"], "Unexpected test command")
require(receipt["exitCode"] == 0 and not receipt["timedOut"] and receipt["processExitObserved"], "Tests did not finish successfully")
require(receipt["productInputsUnchanged"], "Receipt lacks unchanged inputs")
require(receipt["sourceFiles"] == len(after) == 51, "Unexpected source scope")
require(receipt["sourceBeforeSha256"] == sha(OUT / "source-before.json"), "Before manifest binding differs")
require(receipt["sourceAfterSha256"] == sha(OUT / "source-after.json"), "After manifest binding differs")
require(receipt["outputSha256"] == sha(OUT / "workspace-test.txt"), "Test log binding differs")
rows = re.findall(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out", (OUT / "workspace-test.txt").read_text())
totals = [sum(int(row[i]) for row in rows) for i in range(5)]
require(totals == [183, 0, 0, 0, 0] and len(rows) == 17, "Test output totals differ")
require(list(receipt["totals"].values()) == totals, "Receipt totals differ from output")

provenance = json.loads((OUT / "source-evidence.json").read_text())
for rev, expected in [(provenance["baselineHead"], provenance["baselineTree"]), (provenance["snapshotCommit"], provenance["snapshotTree"])]:
    result = subprocess.run(["git", "rev-parse", rev + "^{tree}"], cwd=ROOT, capture_output=True, text=True)
    require(result.returncode == 0 and result.stdout.strip() == expected, "Revision/tree unavailable or different: " + rev)
# Receiving HEAD may differ. Missing Actor refs are retained in preflight.json,
# not reconstructed or required as a substitute for exact source/artifact bytes.
for relative, record in json.loads((OUT / "preservation.json").read_text()).items():
    result = subprocess.run(["git", "show", provenance["baselineHead"] + ":" + relative], cwd=ROOT, capture_output=True)
    require(result.returncode == 0, "Cannot read accepted baseline: " + relative)
    if result.returncode == 0:
        require(result.stdout == (ROOT / relative).read_bytes(), "Preservation boundary changed: " + relative)
        require(hashlib.sha256(result.stdout).hexdigest() == record["acceptedBaselineSha256"], "Baseline hash differs: " + relative)

boundary = json.loads((OUT / "boundary.json").read_text())
require(boundary["onlyOwnedDocsChanged"] and boundary["diffCheck"]["exitCode"] == 0, "Document ownership/diff check failed")
refs = json.loads((OUT / "references-after.json").read_text())
require(not refs["broken"], "Recorded broken documentation references")
for link in refs["links"]:
    relative, _, fragment = unquote(link["target"].strip("<>")).partition("#")
    doc = ROOT / link["doc"]
    target = (doc.parent / relative).resolve() if relative else doc
    require(target.exists(), "Missing linked target: " + link["target"])
    if fragment and target.exists() and target.suffix == ".md":
        headings = re.findall(r"^#{1,6}\s+(.+)$", target.read_text(), re.M)
        slugs = [re.sub(r"[^\w\- ]", "", value.lower()).replace(" ", "-") for value in headings]
        require(fragment in slugs, "Missing heading fragment: " + link["target"])

print(json.dumps({"scope": "Actor delivery bytes/current receiving product inputs; no runtime replay or external collection proof", "artifactCount": len(index["files"]), "sourceCount": len(after), "localReferences": refs["localLinkCount"], "failures": failures}, indent=2))
raise SystemExit(1 if failures else 0)
