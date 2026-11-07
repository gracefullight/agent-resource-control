"""Refresh the delivery pointer and append current, source-bound evidence."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
DB = OUT.parent
BASE = "42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0"
SNAP = "8571d9f990d8d521fee007e0131d07bb383bd66f"


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load(path):
    return json.loads(path.read_text())


def main():
    before = load(OUT / "provenance-before.json")
    after = load(OUT / "provenance-after.json")
    assert before["productFingerprint"] == after["productFingerprint"]
    manifest = after["productFingerprint"]
    assert len(manifest["files"]) == 50
    accounting = load(OUT / "accounting-summary.json")
    workspace = load(OUT / "workspace-summary.json")
    assert workspace["passed"] == 173
    assert any("tests/db_worker_unit.rs" in x["target"] and x["passed"] == 16 for x in workspace["suites"])
    assert any("tests/transport_unit.rs" in x["target"] and x["passed"] == 18 for x in workspace["suites"])
    original = load(OUT / "previous-evidence.json")
    checks = []
    for name in ["worker-test", "fmt", "clippy", "workspace-test", "worker-accounting"]:
        p = OUT / (name + ".json")
        r = load(p)
        assert r["exitCode"] == 0 and r["processExitObserved"] and not r["timedOut"]
        assert r["productInputsUnchanged"]
        assert r["sourceBefore"] == r["sourceAfter"] == manifest["sha256"]
        assert sha(OUT / r["output"]) == r["outputSha256"]
        checks.append(dict(receipt=str(p.relative_to(ROOT)), **r))
    assert after["migration"]["byteIdentical"] and after["harness"]["byteIdentical"]
    assert after["migration"]["currentSha256"] == "046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384"
    harness_receipt_path = ROOT / ".arc-verify/bounded-ipc/qa-review/harness.json"
    harness_receipt = load(harness_receipt_path)
    harness_log = ROOT / ".arc-verify/bounded-ipc/qa-review/harness.txt"
    assert sha(harness_log) == harness_receipt["outputSha256"]
    assert harness_receipt["exitCode"] == 1
    assert harness_receipt["sourceBefore"] == harness_receipt["sourceAfter"] == manifest["sha256"]
    session = os.environ.get("CODEX_SESSION_ID")
    report = ROOT / f".agents/results/result-database-task-3-attempt-7-{session}.md"
    recorded = datetime.datetime.now(datetime.timezone.utc).isoformat()
    text = f"""# SQLite worker task-3, attempt 7

Status: existing implementation preserved; current checks passed; ready for independent task review.
Recorded {recorded}. Skills used: oma-db and oma-backend; db-engineer definition, execution policies and relevant checklists were read. Actual session: `{session}`. No OMA run ID or claim path was injected. No identity, routing repair, retry reset or review outcome was invented.

This attempt resumed completed work. The single SQLite worker, its cancellation code, real-metrics tests and discovered `db_worker_unit` target were already present in the supplied isolated snapshot. No product, test, manifest, lockfile, migration, harness, contract, transport, metrics or limits file changed. Changed paths are only `.arc-verify/bounded-ipc/database/delivery.md`, `evidence.json`, new `attempt-7/` evidence and the supplemental `.agents/results/` report. The 50-file product manifest has the same before/after fingerprint, `{manifest['sha256']}`. `attempt-7/provenance-before.json` and `provenance-after.json` retain the exact paths, modes, revisions, dirty boundaries and accepted-baseline diff.

## Acceptance evidence

- [x] Unit tests prove queue and executing-work bounds, immediate full-queue rejection and cancellation accounting.
- [x] Tests exercise active SQL interruption, lock waits and cancellation/start races, proving worker exit and successful reopen/integrity checks.
- [x] The delivered worker implements the contract and has no hello/health dependency.
- [x] Material database evidence includes actual lifecycle results and the unchanged migration comparison.

The real registered target imports `db_worker.rs`, `limits.rs` and `runtime_metrics.rs` with path declarations. Its temporary dead-code allowance remains confined to the test target. The metrics helper derives queue capacity and database budgets from each DbConfig and calls `RuntimeMetrics::new(&RuntimeLimits)` without a double. All 14 worker tests remain (13 lifecycle tests plus one execution-race test), alongside two separately counted shared-module tests. All 18 transport tests also ran in the full workspace suite.

The synchronized queue test attained Q=2 and one executing job. All 100,000 additional calls immediately returned Busy; the production DbRejected counter matched 100,000. In the fresh serial accounting log this loop took 20,495 us. Cancelled queued jobs continue owning their queue/outstanding tokens until the worker removes them, so dropping tickets cannot admit unlimited retained jobs. Recovery then succeeded. This is a worker-only admission test, not a substitute for mission-wide subprocess/CLI load evidence or a performance benchmark.

`assert_bounds` checks current and high-water DbQueued<=Q, DbExecuting<=1 and DbOutstanding<=Q+1 on real snapshots. `stop` checks native JoinHandle removal after actual join, explicit SQLite close, every final gauge and tracked-task count at zero, WorkerStarted=1 and WorkerJoined=1, then reopens the database and requires integrity_check=ok and one schema migration. The accounting run contains {accounting['lifecycleCount']} matching snapshots and successful join/close/reopen receipts; maxima are queued=2, executing=1, outstanding=3. Largest observed join/close/reopen interval was {accounting['largestJoinCloseReopenUs']} us, under the existing 1,500 ms assertion. These timings do not prove every operating-system filesystem stall is bounded.

Preserved fixtures synchronize on real VM progress, actual busy callbacks and bounded channels. They cover active SQL interruption, ticket cancellation without stopping the worker, execution and queued expiry, early-interrupt/start races, eight concurrent cancelling producers, enqueue/stop between empty check and park, a full queue during shutdown, lock contention, repeated idle lifecycles and retention of an unfinished native handle after a shutdown deadline. No arbitrary production SQL, sleep operation, pool, per-job task/thread or explicit WAL checkpoint was added.

## Commands and outcomes

Commands ran serially from this assigned worktree using the prepared native offline cache, Rust/Cargo 1.95.0 on aarch64-apple-darwin. No dependency installation/download, standalone build, browser suite, GUI action, AV order/resume or mission-state mutation occurred. Test compilation is included in the requested Cargo test commands. Complete logs and structured receipts under `attempt-7/` retain exact argv, cwd, UTC start/end, PID, observed exit, environment/toolchain, source-before/after fingerprints and output hashes.

| Command | Outcome | Receipt |
| --- | --- | --- |
| `cargo test -p arc-daemon --test db_worker_unit -- --nocapture` | Exit 0; 16 passed, none ignored/filtered | worker-test.json / .txt |
| `cargo fmt --all -- --check` | Exit 0 | fmt.json / .txt |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0 | clippy.json / .txt |
| `cargo test --workspace` | Exit 0; 173 passed, none failed/ignored/filtered, including subprocess and CLI fixtures | workspace-test.json / .txt |
| Same focused worker command, RUST_TEST_THREADS=1 | Exit 0; 16 passed; all 43 lifecycle records extracted | worker-accounting.json / .txt |

The extra serial worker run prevents libtest stdout/stderr from interleaving JSON; the explicit eight-producer concurrency remains unchanged. Default-concurrency focused and workspace runs passed separately. The existing unchanged harness execution was not repeated for this source-preserving task: `.arc-verify/bounded-ipc/qa-review/harness.json` records the exact command at its original cwd/time, exit 1, with the same current 50-file product fingerprint and matching full-output hash. Its unresolved failures remain resident.rs 901 lines, arc-ipc/src/lib.rs 551 lines and missing test:coverage. The older database-local harness receipts retain their older four-failure scope; neither receipt was rewritten or relabeled as current execution.

The actual historical constructor failure remains in `repair-6/registered-constructor-failure.json` and `.txt` (exit 101, E0599 at RuntimeMetrics::default). It was already repaired in the supplied snapshot; this attempt did not break the tests again to manufacture RED. Original driver files, failures, reports and receipts were preserved. `attempt-7/previous-delivery.md` and `previous-evidence.json` archive the preceding handoff byte-for-byte before updating these current pointers. The entry audit verified all 9 prior source hashes and all 59 prior artifact hashes. The standalone driver and its metrics-double results remain superseded and excluded from acceptance. Its missing serde_json edge was not repaired and no dependency was added to sustain it. The old 35-file collection checker is historical; use the new 50-file `attempt-7/verify_delivery.py` for this delivery.

## Provenance and collection boundary

Accepted committed baseline: `{BASE}`, tree `c802f71ac4d91ed6b8a602d5c83bf9cf57116b76`. Actual Actor HEAD equals supplied snapshotCommit `{SNAP}`, tree `4214462f01a3c059d825ca6ae7ed90dc156ed086`. The worker and target are tracked at this isolated input snapshot and unchanged in this attempt. This does not claim that they exist in the accepted baseline or are tracked in the review checkout. The snapshot/worker ref names supplied by sourceEvidence both fail local rev-parse with exit 128; the full errors are retained. The actual snapshot commit/tree exists and was read directly. No ref or commit was created or altered, and no other checkout's HEAD was substituted.

Migration bytes were compared directly with `git show {BASE}:crates/arc-storage/migrations/0001_phase0.sql`; both hashes equal `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. Harness bytes likewise match accepted HEAD (`93140173df8b8bd28b067aff68301134b0398b643240b0b1e6a13da4487ebe33`). Native noninteractive execution worked. Git reported av.yaml: Operation not permitted; mise also emitted tracked-config symlink warnings. Neither observation establishes an authentication failure or configured-target absence. No workaround was attempted. Serena tools were unavailable; source inspection used supplied known paths.

Run `python3 .arc-verify/bounded-ipc/database/attempt-7/verify_delivery.py` in the collected review checkout. It checks delivered paths and hashes, the actual product manifest, every fresh receipt/output, baseline migration/harness bytes and all 43 accounting records. This Actor can verify its own delivered files but cannot claim a future collection or independent acceptance; the Chief must retain the independent review. No review was waived or invented.

## Chief handoff

Interfaces remain unchanged: DbWorker::start(Connection, DbConfig, Arc<RuntimeMetrics>), client(), begin_shutdown(), shutdown(tokio::time::Instant), DbClient::try_doctor(std::time::Instant), and DbTicket::receive(). One dedicated thread owns SQLite, a finite channel and queue permits bound retention, and ticket drop sets sticky cancellation. Stop sets a sticky flag, interrupts and unparks independently of queue capacity; progress and busy callbacks check stop/cancellation/absolute deadlines. Shutdown explicitly closes SQLite and waits for is_finished plus native join, retaining the handle on deadline failure. No hello or health dependency exists in the worker. Production data access is limited to fixed read-only integrity/schema diagnostics; no schema, data model, business-policy trigger or transaction behavior changed.

Task-4 retains production wiring, public ERROR_DB_BUSY, CLI mapping and production-lint ownership. Its supervisor must preserve admission before SQLite open, hold the instance lock through handler/worker termination, stop the worker before draining handlers, cancel/drop tickets, and use one shared absolute deadline through actual close/join. Existing integrated source and passing workspace tests were preserved; this handoff neither rewrites those deliverables nor grants new ownership. No shared-file/interface change is requested.

Independent task-3 acceptance remains required. The reviewer should inspect the real registration, absence of the double, cap/high-water/final-zero/start/join assertions and native cancellation/lock/park fixtures; compare receipts and hashes in the collected checkout. The broader mission still requires its independent integrated assessment, 100,000-attempt subprocess/CLI evidence, both five-second signals and native RSS/thread/task/P95 comparison with SRS. This task supplies no new performance or deployed-service measurements. The reported missing task-6 receipts and denied configured-target evidence remain Chief/documentation-owner closure items, outside this assignment. Retain harness failures, remaining Phase 0, Linux qualification and Windows Named Pipe/runtime/tray obligations. No mission completion is claimed.
"""
    (DB / "delivery.md").write_text(text)
    report.parent.mkdir(parents=True, exist_ok=True)
    report.write_text(text)
    paths = [x["path"] for x in manifest["files"]]
    paths.extend(["docs/bounded-ipc-contract.md", ".arc-verify/bounded-ipc/ipc/evidence.json", "scripts/harness/validate.sh"])
    data = dict(version=3, recordedAt=recorded, status="ready-for-independent-review",
        identity=dict(actorId="database", taskId="task-3", attempt=7, codexSessionId=session, omaRunId=None, injectedClaimPath=None),
        skillsUsed=["oma-db", "oma-backend"], delivery=str((DB / "delivery.md").relative_to(ROOT)),
        baselineHead=BASE, baselineTree="c802f71ac4d91ed6b8a602d5c83bf9cf57116b76",
        actorSnapshotCommit=SNAP, actorSnapshotTree="4214462f01a3c059d825ca6ae7ed90dc156ed086",
        actorObservedHead=SNAP, snapshotAndWorkerRefsLocallyResolvable=False,
        provenanceScope="Actor-local snapshot and fresh receipts, not accepted-baseline implementation or future review checkout.",
        sourceHashes=[dict(path=p,sha256=sha(ROOT / p)) for p in sorted(set(paths))],
        sourceFingerprint=manifest["sha256"], productManifest=manifest["files"], changedProductPaths=[],
        migration=after["migration"], harness=after["harness"], checks=checks,
        workerUnit=dict(workerTestsPassed=14, sharedTestsPassed=2, totalPassed=16, failed=0, ignored=0, filtered=0),
        transportUnit=dict(totalPassed=18, failed=0, ignored=0, filtered=0), workspaceResults=workspace,
        lifecycle={k:v for k,v in accounting.items() if k!="snapshots"},
        historicalDriver=dict(supersededForAcceptance=True, workspaceMember=False,
            originalHandoff=["original-handoff.md", "original-handoff.json"],
            previousEvidence="attempt-7/previous-evidence.json",previousDelivery="attempt-7/previous-delivery.md",
            unchangedDriverFiles=[x for x in original["artifacts"] if "/driver/" in x["path"]]),
        historicalHarness=dict(rerunThisAttempt=False, receipt=str(harness_receipt_path.relative_to(ROOT)),
            output=str(harness_log.relative_to(ROOT)), exitCode=1,
            receiptSha256=sha(harness_receipt_path), outputSha256=sha(harness_log), sourceFingerprint=manifest["sha256"],
            unresolved=["resident.rs 901 lines", "arc-ipc/src/lib.rs 551 lines", "missing test:coverage"]),
        scope=dict(workerUnitCompatibilityVerified=True, independentReviewExecuted=False,
            nativePerformanceMeasuredThisAttempt=False, missionAcceptanceComplete=False,
            futureCollectedReviewCheckoutVerified=False),
        remaining=["Independent task-3 review and collected checkout hash verification", "Unresolved harness failures",
            "Chief/documentation-owner task-6 receipt recovery", "Supported configured-target evidence",
            "Independent integrated mission assessment", "Remaining Phase 0, Linux, Windows runtime obligations"])
    data["artifacts"] = [dict(path=str(p.relative_to(ROOT)),bytes=p.stat().st_size,sha256=sha(p))
        for p in sorted(DB.rglob("*")) if p.is_file() and p != DB / "evidence.json"
        and p != OUT / "collection-check.json" and "__pycache__" not in p.parts]
    data["artifactIndexScope"] = "All database evidence files except current evidence.json (self), later attempt-7/collection-check.json and generated bytecode. Historical reports/receipts retain original scope."
    (DB / "evidence.json").write_text(json.dumps(data,indent=2)+"\n")
    print(json.dumps(dict(delivery=data["delivery"], sourceFingerprint=manifest["sha256"], artifacts=len(data["artifacts"]), report=str(report.relative_to(ROOT)))))


if __name__ == "__main__":
    main()
