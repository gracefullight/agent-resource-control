"""Assemble the repaired task-3 handoff without changing prior receipts."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
DB = OUT.parent
BASE = "42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0"


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load(path):
    return json.loads(path.read_text())


def main():
    final = load(OUT / "provenance-after.json")
    original = load(OUT / "original-evidence.json")
    accounting = load(OUT / "accounting-summary.json")
    comparison = load(OUT / "source-comparison.json")
    fingerprint = final["productFingerprint"]["sha256"]
    assert final["migration"]["byteIdentical"]
    assert final["migration"]["currentSha256"] == "046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384"
    assert comparison["allOriginalFunctionsRetained"] and comparison["allAcceptedTransportHashesMatch"]
    worker = (OUT / "worker-test-final.txt").read_text()
    workspace = (OUT / "workspace-test.txt").read_text()
    assert "test result: ok. 16 passed; 0 failed; 0 ignored;" in worker
    sections = {}
    for match in re.finditer(r"Running ([^\n]+)\n(.*?)(?=\n\s*Running |\n\s*Doc-tests |\Z)", workspace, re.S):
        summary = re.search(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;", match[2])
        if summary:
            sections[match[1]] = dict(zip(["passed", "failed", "ignored"], map(int, summary.groups())))
    assert any("tests/db_worker_unit.rs" in k and v["passed"] == 16 for k, v in sections.items())
    assert any("tests/transport_unit.rs" in k and v["passed"] == 18 for k, v in sections.items())
    checks = []
    for name in ["worker-test-final", "fmt-final", "clippy", "workspace-test", "worker-test-accounting"]:
        receipt = load(OUT / (name + ".json"))
        assert receipt["exitCode"] == 0 and receipt["processExitObserved"] and not receipt["timedOut"]
        assert receipt["sourceBefore"] == receipt["sourceAfter"] == fingerprint
        assert sha(OUT / receipt["output"]) == receipt["outputSha256"]
        checks.append({"receipt": str((OUT / (name + ".json")).relative_to(ROOT)), **receipt})
    old_driver = []
    for entry in original["sourceFiles"]:
        if "/driver/" in entry["path"]:
            assert sha(ROOT / entry["path"]) == entry["sha256"]
            old_driver.append(entry)
    harness = "scripts/harness/validate.sh"
    harness_baseline = subprocess.check_output(["git", "show", BASE + ":" + harness], cwd=ROOT)
    assert harness_baseline == (ROOT / harness).read_bytes()
    session = os.environ["CODEX_SESSION_ID"]
    report = ROOT / f".agents/results/result-database-task-3-attempt-6-{session}.md"
    text = f"""# SQLite worker task-3 repair handoff

Status: implementation and workspace evidence repaired; ready for independent task review.
Correction dated {datetime.datetime.now(datetime.timezone.utc).isoformat()}. Actor: database; task: task-3; assigned attempt: 6; actual Codex session: `{session}`. No OMA run ID, claim path or current collection snapshot was injected. None was invented. The existing managed execution route worked; no AV dispatch, routing repair, mission-state edit, retry reset, credential change, GUI workaround or subagent was used.

## Changed paths and ownership

- Repaired `crates/arc-daemon/src/db_worker/tests.rs` to use the actual `RuntimeMetrics::new(&RuntimeLimits)` constructor. The helper derives queue capacity, execution timeout and busy timeout from the tested `DbConfig`, preserves the other established limits, and validates the resulting limits.
- Added `crates/arc-daemon/tests/db_worker_unit.rs`, registering the real worker, limits and metrics modules with `#[path]`. Its `dead_code` allowance is confined to this temporary integration test target. No production lint suppression was added.
- Reused `crates/arc-daemon/src/db_worker.rs` and `src/db_worker/execution.rs` unchanged. Their hashes still match the original task-3 evidence.
- Wrote this directory's evidence and `{report.relative_to(ROOT)}`. Preserved prior database receipts and standalone-driver files. `repair-6/original-evidence.json` is the original evidence file, byte-for-byte. `original-handoff.md` and `original-handoff.json` are copies of the original Actor handoff from its actual retained workspace; no original report or receipt was rewritten.

All nine accepted transport source hashes match `.arc-verify/bounded-ipc/ipc/evidence.json`, recorded in `repair-6/source-comparison.json`. Transport, metrics, limits, main.rs, server.rs, manifests, lockfiles, storage sources, migrations and harness scripts were not edited. Importing the reused evidence recorder briefly generated a Python bytecode file in the IPC evidence directory; only that newly generated file was removed and bytecode writes disabled. No accepted IPC artifact was changed.

## Acceptance evidence

- [x] Unit tests prove queue and executing-work bounds, immediate full-queue rejection and cancellation accounting.
- [x] Tests exercise active SQL interruption, lock waits and cancellation/start races, proving worker exit and successful reopen/integrity checks.
- [x] The delivered worker implements the contract and has no hello/health dependency.
- [x] Material database evidence includes actual lifecycle results and the unchanged migration comparison.

These are implementation/unit-evidence results awaiting the required independent task acceptance.

1. Queue bounds, immediate rejection and cancellation: all 14 original worker tests remain and run in Cargo's discovered target. The synchronized full-queue test reaches queued=2, executing=1 and outstanding=3, asserts all 100,000 attempts return Busy immediately, checks the real DbRejected counter equals 100,000, cancels tickets and proves doctor recovery. `assert_bounds` checks real snapshot current/high-water values for queued<=Q, executing<=1 and outstanding<=Q+1. The eight-producer cancellation test remains unchanged in its concurrency and attempt count.
2. Cancellation and actual termination: tests retain active VM interruption, cancelled active SQL, execution expiry before response expiry, expired queued work, a full queue at shutdown, busy-lock expiry and shutdown cancellation, early-interrupt/start races, enqueue/stop between empty-check and park, repeated idle lifecycles and timeout retention of the native join handle. Each completed worker lifecycle asserts the handle is absent only after actual join, SQLite closed, all eight current gauges and tracked tasks are zero, WorkerStarted=1 and WorkerJoined=1, then reopens the database and checks `PRAGMA integrity_check == ok` and one schema migration.
3. Contract and fast-path separation: worker APIs and production worker source are unchanged. Production accepts only fixed doctor diagnostics; it imports no transport, hello or health path and exposes no arbitrary SQL or sleep operation. Test-only gates, fixed recursive SQL and fixed lock probes remain behind cfg(test). The one thread owns SQLite through close. Nonwaiting semaphore plus try_reserve admission retains cancelled queued work within Q slots; a job retains its outstanding gauge until discarded or completed. Sticky stopping/cancellation/deadline checks, the 1,000-instruction progress callback, interrupt handle and bounded busy callback cover early-interrupt races and lock waits. Stop wakes park independently of queue capacity; shutdown retains its handle on deadline failure until a later successful is_finished+join.
4. Integrity and lifecycle material: the final accounting run contains 43 complete snapshots and 43 successful native-join/SQLite-close/reopen/integrity receipts. Observed high-water maxima: queued=2, executing=1, outstanding=3. Every final gauge is zero; every worker started and joined exactly once. Largest observed join+close+reopen interval: {accounting['largestJoinCloseReopenUs']} us, within each test's existing 1,500 ms assertion. This is worker-unit evidence, not a daemon shutdown or performance benchmark. Migration bytes were compared directly using git show of accepted HEAD; SHA-256 remains `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`.

## Commands and retained failures

All Cargo commands ran serially from the assigned mission workspace with the existing offline cache (`CARGO_NET_OFFLINE=true`), Rust/Cargo 1.95.0 on aarch64-apple-darwin. No dependency install, download, standalone build or browser suite was run. Complete combined output and JSON receipts retain argv, cwd, UTC times, process PID, observed child status, timeout state, toolchain/environment reference, source-before/source-after fingerprints and output hashes.

| Command | Outcome / evidence under repair-6 |
| --- | --- |
| `cargo test -p arc-daemon --test db_worker_unit -- --nocapture` before the repair | Exit 101, actual E0599 missing RuntimeMetrics::default, `registered-constructor-failure.*` |
| Same focused command after repair | Exit 0, 16 passed = 14 worker + 2 shared module tests; `worker-test.*` and post-format `worker-test-final.*` |
| `cargo fmt --all -- --check` initial attempt | Exit 1, line wrapping in the edited owned test file; `fmt.*` retained |
| `rustfmt --edition 2021 crates/arc-daemon/src/db_worker/tests.rs` | Exit 0, only the owned test file formatted; `format-owned.*` |
| `cargo fmt --all -- --check` final | Exit 0; `fmt-final.*` |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0; `clippy.*` |
| `cargo test --workspace` | Exit 0; worker target 16 passed and transport target 18 passed, all other existing CLI/core/daemon/resident/startup/IPC/storage tests passed; `workspace-test.*` |
| Same focused command with RUST_TEST_THREADS=1 | Exit 0, 16 passed; `worker-test-accounting.*`, `accounting-execution-context.json` and `accounting-summary.json` |

The last focused run resolved an evidence-extraction issue: parallel libtest stdout interleaved five JSON records on combined stderr in `worker-test-final.txt`, so an initial parser raised JSONDecodeError. The original passing test log is retained. A single test-thread run produced all 43 machine-readable records; the explicitly concurrent eight-producer test still uses eight native producers. No assertion or test was removed, ignored, filtered or weakened. The default-concurrency focused and workspace suites both passed independently.

Final product fingerprint: `{fingerprint}`, covering all 35 Cargo/product input files including the new target. The final focused check, formatting, Clippy, workspace suite and additional accounting run all record that same unchanged fingerprint. `repair-6/provenance-before.json` and `provenance-after.json` enumerate content/mode hashes and dirty state. `verify_delivery.py` provides read-only path/hash, receipt/output and source-fingerprint verification for the collected checkout.

## Superseded historical driver and provenance

The original standalone driver under `driver/` and its receipts are historical evidence only and excluded from current acceptance. It used a metrics double with Default and therefore masked this constructor mismatch. It is not a workspace member or imported by db_worker_unit. Its missing serde_json edge for the real shared metrics module was not repaired; no dependency was added to sustain it. Original driver hashes remain unchanged. Current acceptance uses the discovered workspace target with real limits and metrics only.

Current checkout HEAD/tree: `{BASE}` / `c802f71ac4d91ed6b8a602d5c83bf9cf57116b76`. The original task-3 snapshot and worker refs resolve to `9f6532ba89de234ae11895a6f8591d1fc6195d22`, tree `e79ff4ed85c94059275bbb4992579a2d55f51eb3`; accepted HEAD is its ancestor. That snapshot does not contain the worker files: the delivered source was an uncommitted overlay, as the original report states. This repair likewise leaves its two changed product paths as uncommitted overlay files at the mission HEAD. No commits or evidence refs were created or altered. Collection may create later framework provenance; this Actor cannot claim an unobserved snapshot or future reviewer checkout. The self-contained .arc-verify delivery avoids the earlier missing-.agents/results dependency. Actual collection verification must run the supplied checker in the collected review checkout.

## Consumer handoff and remaining work

Task-4 owns production declarations and wiring. Preserve `DbWorker::start(Connection, DbConfig, Arc<RuntimeMetrics>)`, `client`, `begin_shutdown`, `shutdown(deadline)`, `DbClient::try_doctor(std::time::Instant)` and `DbTicket::receive`. Hold DbWorker and its native handle in the supervisor, call begin_shutdown before draining handlers, then use the same absolute shutdown deadline for actual close/join. A DbTicket cancels on drop; its queue slot and outstanding token stay accounted for until the worker removes it. Keep hello/health entirely outside this path. Preserve instance admission before SQLite open, hold the instance lock through worker join, and preserve same-user/filesystem checks.

Task-4 also owns public ERROR_DB_BUSY, error/CLI mapping and production-lint compliance when these modules compile outside cfg(test). The temporary test-target allowance proves no production lint gate. WorkerBusy/Deadline/Cancelled/Stopping/WorkerLost must follow the adopted contract; genuine integrity errors remain distinct. Production wiring, integrated health-during-slow-SQL fixtures, actual CLI DB_BUSY mapping, 100,000-attempt subprocess/CLI admission evidence and both SIGTERM/SIGINT five-second cases are still required. The worker-only 100,000 rejection test does not fulfill that broader load criterion.

Independent review remains required before task-4 admission. Reviewer must inspect the actual target and real constructor, verify the double is excluded, compare the recorded sources/receipts with collected inputs, inspect the real high-water and final-zero/start/join assertions, and run the read-only collection checker. A reviewer lacking functioning tools must report the prerequisite to the Chief for supported independent reassignment. This Actor did not perform or waive independent review.

The prior unchanged harness receipt is retained at `harness.json` / `harness.txt`, exit 1, with four unresolved violations: server.rs 762 lines, resident.rs 901 lines, arc-ipc/src/lib.rs 549 lines, and missing test:coverage. Its script is still byte-identical to accepted HEAD. It was not rerun for this unit-registration repair; later integrated QA must run the unchanged harness and retain unresolved failures. No threshold, test, manifest or script was changed to manufacture conformance.

Native RSS/thread/task/P95 measurement against the located SRS, remaining Phase 0 behavior, Linux qualification and Windows Named Pipe/runtime/tray obligations remain open. No measurement, deployed-service evidence or mission-completion claim is supplied here. Native filesystem stalls remain outside what these unit timings prove. `av.yaml` was unreadable in initial Git status; no service/container state was inferred from it. Serena was not available; inspection used supplied known paths. Skills used: oma-db and oma-backend; relevant execution policies, agent definition and checklists were read.
"""
    (DB / "delivery.md").write_text(text)
    report.write_text(text)
    historical_integrity = []
    for entry in original["sourceFiles"]:
        historical_integrity.append({**entry, "currentSha256": sha(ROOT / entry["path"]),
                                     "unchangedInRepair": sha(ROOT / entry["path"]) == entry["sha256"]})
    data = {
        "version": 2, "recordedAt": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "status": "ready-for-independent-review", "identity": {"actorId": "database", "taskId": "task-3",
             "attempt": 6, "codexSessionId": session, "omaRunId": None, "injectedClaimPath": None},
        "skillsUsed": ["oma-db", "oma-backend"], "delivery": str((DB / "delivery.md").relative_to(ROOT)),
        "baselineHead": BASE, "baselineTree": "c802f71ac4d91ed6b8a602d5c83bf9cf57116b76",
        "originalActorSnapshotCommit": original["snapshotCommit"], "originalActorSnapshotTree": original["snapshotTree"],
        "repairSnapshotCommit": None, "repairSnapshotNote": "Uncommitted overlay in mission checkout; no new ref or commit made.",
        "sourceHashes": final["sourceHashes"], "sourceFingerprint": fingerprint,
        "changedProductPaths": ["crates/arc-daemon/src/db_worker/tests.rs", "crates/arc-daemon/tests/db_worker_unit.rs"],
        "unchangedOriginalWorkerProductionSource": all(x["unchangedInRepair"] for x in historical_integrity[:2]),
        "acceptedTransportHashesMatch": True, "migration": final["migration"],
        "checks": checks, "workerUnit": {"workerTestsPassed": 14, "sharedTestsPassed": 2,
              "totalPassed": 16, "failed": 0, "ignored": 0, "filtered": 0},
        "transportUnit": {"totalPassed": 18, "failed": 0, "ignored": 0, "filtered": 0},
        "workspaceResults": sections, "lifecycle": {k:v for k,v in accounting.items() if k != "snapshots"},
        "historicalDriver": {"supersededForAcceptance": True, "workspaceMember": False,
              "filesPreserved": old_driver, "originalEvidence": "repair-6/original-evidence.json",
              "originalHandoff": ["original-handoff.md", "original-handoff.json"], "originalSourceComparison": historical_integrity},
        "harness": {"rerunForThisRepair": False, "scriptUnchanged": True,
              "historicalExitCode": 1, "historicalReceipt": "harness.json", "historicalOutput": "harness.txt",
              "remainingViolations": ["server.rs 762 lines", "resident.rs 901 lines", "arc-ipc/src/lib.rs 549 lines", "missing test:coverage"]},
        "scope": {"workerUnitCompatibilityVerified": True, "productionIntegrationVerified": False,
              "independentReviewExecuted": False, "nativePerformanceMeasured": False, "missionAcceptanceComplete": False,
              "currentMissionCheckoutHashCheck": "repair-6/collection-check.json",
              "futureCollectedReviewCheckoutVerified": False},
        "remaining": ["Independent task-3 review and actual collected-checkout hash validation before task-4",
              "Task-4 production wiring, production lint, DB_BUSY and actual CLI mapping",
              "Integrated 100000-attempt adversarial subprocess/CLI evidence, health independence and both five-second signals",
              "Native RSS/thread/task/P95 versus SRS and exact integrated gates/unchanged harness",
              "Phase 0, Linux and Windows runtime obligations"],
    }
    data["artifacts"] = [{"path": str(p.relative_to(ROOT)), "bytes": p.stat().st_size, "sha256": sha(p)}
                         for p in sorted(DB.rglob("*")) if p.is_file()
                         and p.name not in ["evidence.json", "collection-check.json"]
                         and "__pycache__" not in p.parts]
    data["artifactIndexScope"] = "All database evidence files except evidence.json (self), later collection-check.json and generated bytecode. No self hash."
    (DB / "evidence.json").write_text(json.dumps(data, indent=2) + "\n")
    print(json.dumps({"delivery": str(DB / "delivery.md"), "report": str(report),
                      "sourceFingerprint": fingerprint, "indexedArtifacts": len(data["artifacts"])}))


if __name__ == "__main__":
    main()
