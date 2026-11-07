"""Check the assigned report/evidence contract without mutating product files."""
import hashlib
import json
from pathlib import Path
import re
import runpy
import subprocess

ROOT = Path.cwd()
OUT = ROOT / ".arc-verify/runtime-identity/qa"
report = ROOT / "docs/runtime-identity-qa.md"
evidence = json.loads((ROOT / "docs/runtime-identity-evidence.json").read_text())
expected = {
    "fmt": ["cargo", "fmt", "--all", "--", "--check"],
    "clippy": ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
    "test": ["cargo", "test", "--workspace"],
}
assert report.stat().st_size >= 500
assert all(text in report.read_text() for text in ["Acceptance matrix", "Sandbox limits"])
assert evidence["baseline"] == "073802555b639aa2fec183d93f748d9437cbbcad"
assert evidence["platform"] == "macos"
assert evidence["migrationSha256"] == "046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384"
assert all(key in evidence for key in ["sourceFingerprint", "fixtureRoots", "requiredCases", "limits", "commands"])
assert all(evidence["commands"][key]["argv"] == argv and evidence["commands"][key]["exitCode"] == 0 for key, argv in expected.items())
assert all(case["executed"] and case["passed"] for case in evidence["requiredCases"])
current = runpy.run_path(str(OUT / "run_checks.py"))["source"]()
recorded = json.loads((OUT / "source.json").read_text())
assert current["sourceFiles"] == recorded["sourceFiles"]
assert current["sourceFingerprint"] == evidence["sourceFingerprint"]
assert all(hashlib.sha256((ROOT / path).read_bytes()).hexdigest() == digest for path, digest in evidence["artifactSha256"].items())
links = re.findall(r"\]\(([^)]+)\)", report.read_text())
assert all((report.parent / link).exists() for link in links)
ignored = subprocess.run(["git", "check-ignore", ".arc-verify/runtime-identity/qa/target/debug/arcctl"], capture_output=True, text=True)
assert ignored.returncode == 0
result = {"passed": True, "report": str(report.relative_to(ROOT)),
          "requiredCaseGroups": len(evidence["requiredCases"]), "reportLinksResolved": len(links),
          "sourceUnchangedAtDelivery": True, "sourceFingerprint": current["sourceFingerprint"],
          "generatedTargetIgnored": True, "requiredFindings": [],
          "additionalHarnessExitCode": evidence["additionalHarness"]["exitCode"]}
(OUT / "final-validation.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result))
