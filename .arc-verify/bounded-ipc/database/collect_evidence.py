"""Collect hashes and observed worker outcomes; no performance extrapolation."""

import datetime
import hashlib
import json
from pathlib import Path
import platform
import re
import subprocess

root = Path(__file__).resolve().parents[3]
evidence = Path(__file__).resolve().parent
baseline = "42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0"
snapshot = "9f6532ba89de234ae11895a6f8591d1fc6195d22"
migration = "crates/arc-storage/migrations/0001_phase0.sql"

def git(*args):
    return subprocess.check_output(["git", *args], cwd=root)

def sha(data):
    return hashlib.sha256(data).hexdigest()

files = ["crates/arc-daemon/src/db_worker.rs",
         "crates/arc-daemon/src/db_worker/execution.rs",
         "crates/arc-daemon/src/db_worker/tests.rs",
         ".arc-verify/bounded-ipc/database/driver/Cargo.toml",
         ".arc-verify/bounded-ipc/database/driver/Cargo.lock",
         ".arc-verify/bounded-ipc/database/driver/src/lib.rs"]
current = (root / migration).read_bytes()
accepted = git("show", f"{baseline}:{migration}")
log = (evidence / "unit-tests.txt").read_text()
tests = re.search(r"test result: ok\. (\d+) passed; 0 failed", log)
lifecycle = [int(value) for value in re.findall(r"worker-lifecycle: .*?elapsed_us=(\d+)", log)]
loads = re.findall(r"worker-load: attempts=100000 rejected=100000 queued=2 executing=1 elapsed_us=(\d+)", log)
accounting = re.findall(r"worker-accounting: queued_high=(\d+) executing_high=(\d+) outstanding_high=(\d+) started=1 joined=1 final_gauges=0", log)
record = {
    "scope": "Actor-local uncommitted task-3 files; isolated worker driver with metrics interface test double",
    "collectedAt": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "host": platform.platform(),
    "baselineHead": baseline,
    "baselineTree": git("rev-parse", f"{baseline}^{{tree}}").decode().strip(),
    "snapshotCommit": snapshot,
    "snapshotTree": git("rev-parse", f"{snapshot}^{{tree}}").decode().strip(),
    "inspectedLocalHead": git("rev-parse", "HEAD").decode().strip(),
    "inspectedLocalTree": git("rev-parse", "HEAD^{tree}").decode().strip(),
    "branch": git("branch", "--show-current").decode().strip(),
    "sourceFiles": [{"path": path, "sha256": sha((root / path).read_bytes()),
                     "inSnapshot": bool(git("ls-tree", snapshot, "--", path).strip()),
                     "scope": "local uncommitted file"} for path in files],
    "migration": {"path": migration, "unchanged": current == accepted,
                  "baselineSha256": sha(accepted), "localSha256": sha(current)},
    "harness": {"path": "scripts/harness/validate.sh",
                "unchanged": (root / "scripts/harness/validate.sh").read_bytes() == git("show", f"{baseline}:scripts/harness/validate.sh")},
    "workerTestsPassed": int(tests.group(1)) if tests else 0,
    "workerAccountingScope": "test-double metrics assertions at every gauge transition, real SQLite worker and native join",
    "saturation": {"attempts": 100000 if loads else 0,
                   "rejected": 100000 if loads else 0,
                   "queuedCap": 2, "executingCap": 1, "outstandingCap": 3,
                   "elapsedUs": int(loads[0]) if loads else None},
    "observedMaxima": {"queued": max(int(x[0]) for x in accounting),
                       "executing": max(int(x[1]) for x in accounting),
                       "outstanding": max(int(x[2]) for x in accounting)},
    "workerAccountingReceipts": len(accounting),
    "lifecycleReopenReceipts": len(lifecycle),
    "largestRecordedJoinCloseReopenUs": max(lifecycle) if lifecycle else None,
    "integrationVerified": False,
    "nativePerformanceMeasured": False,
    "remaining": ["Production daemon module/metrics integration by IPC Actor",
                  "Integrated worker, health, subprocess and CLI tests by assigned Actors",
                  "Final integrated gates and independent review",
                  "Mission native performance, Phase 0, Linux and Windows runtime obligations"]}
(evidence / "evidence.json").write_text(json.dumps(record, indent=2) + "\n")
print(json.dumps(record, indent=2))
