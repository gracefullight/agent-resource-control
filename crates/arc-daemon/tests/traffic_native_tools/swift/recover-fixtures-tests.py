import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

HERE = Path(__file__).parent
SCRIPT = HERE / "recover-fixtures.py"
SPEC = importlib.util.spec_from_file_location("swift_recovery", SCRIPT)
RECOVERY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RECOVERY)
MANIFEST = (HERE / "sources/Package.swift.txt").read_bytes()


class SwiftRecoveryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve()
        self.project = self.root / "target/traffic-bun-scripts/native/daemon-swift-semantics-case/direct-swift-build-false"
        (self.project / "package path").mkdir(parents=True)
        (self.project / "package path/Package.swift").write_bytes(MANIFEST)
        (self.project / "form").write_text("swift-build")
        (self.project / "sdk-path").write_text("sdk")
        self.output = self.project / "scratch path/plugins/outputs/package/evidence"
        self.output.mkdir(parents=True)
        (self.output / "entry.json").write_bytes(b"native entry")
        (self.output / "not-evidence").write_bytes(b"compiler output")
        (self.project / "module-cache").mkdir()
        (self.project / "module-cache/data").write_bytes(b"cache" * 1000)
        (self.project / "direct-stderr").write_bytes(b"diagnostic")

    def tearDown(self):
        self.temp.cleanup()

    def test_discovery_and_apply_preserve_evidence_and_unrelated_files(self):
        unrelated = self.root / "target/traffic-bun-scripts/native/unrelated"
        unrelated.mkdir()
        (unrelated / "sentinel").write_bytes(b"keep")
        command = [sys.executable, str(SCRIPT), "--workspace", str(self.root)]
        discovery = subprocess.run(command, capture_output=True, text=True, timeout=60, check=True)
        plan = json.loads(discovery.stdout)
        self.assertEqual(plan["projects"], [str(self.project)])
        plan_file = self.root / "plan.json"
        plan_file.write_text(discovery.stdout)
        result = subprocess.run(command + ["--apply-file", str(plan_file)], capture_output=True, text=True, timeout=60, check=True)
        receipt = json.loads(result.stdout)
        saved = self.project / "failure-plugin-outputs/package/evidence/entry.json"
        self.assertEqual(saved.read_bytes(), b"native entry")
        self.assertEqual(receipt["saved"][0]["path"], str(saved))
        self.assertFalse((self.project / "scratch path").exists())
        self.assertFalse((self.project / "module-cache").exists())
        self.assertEqual((self.project / "direct-stderr").read_bytes(), b"diagnostic")
        self.assertEqual((unrelated / "sentinel").read_bytes(), b"keep")

    def test_symlinked_evidence_and_outside_paths_are_rejected_before_deletion(self):
        outside = self.root / "outside"
        outside.mkdir()
        (outside / "sentinel").write_bytes(b"keep")
        with self.assertRaises(ValueError):
            RECOVERY.validate(self.root, outside, MANIFEST)
        plugins = self.project / "scratch path/plugins"
        plugins.rename(self.project / "scratch path/original-plugins")
        plugins.symlink_to(outside, target_is_directory=True)
        with self.assertRaises(ValueError):
            RECOVERY.compact(self.project)
        self.assertTrue((self.project / "module-cache/data").is_file())
        self.assertEqual((outside / "sentinel").read_bytes(), b"keep")


if __name__ == "__main__":
    unittest.main()
