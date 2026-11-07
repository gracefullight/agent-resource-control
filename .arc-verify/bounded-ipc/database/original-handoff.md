# SQLite worker delivery

Status: completed within task-3's owned worker scope. Production integration and mission-wide acceptance remain with the assigned Actors and Chief Director.

## Source and ownership

Accepted committed baseline: `42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0`, tree `c802f71ac4d91ed6b8a602d5c83bf9cf57116b76`.

Actor snapshot and inspected local HEAD: `9f6532ba89de234ae11895a6f8591d1fc6195d22`, tree `e79ff4ed85c94059275bbb4992579a2d55f51eb3`, branch `av-task/d6c6b45f4a6c3769-1724980aa084bbaa-1`. These are different revisions from the accepted baseline. The new worker files are local, uncommitted and absent from that snapshot. Their actual SHA-256 hashes and revision scopes are in `.arc-verify/bounded-ipc/database/evidence.json`; they must be verified again in the integrated worktree. No missing delivery revision or receipt is inferred.

Production paths added:

- `crates/arc-daemon/src/db_worker.rs`
- `crates/arc-daemon/src/db_worker/execution.rs`
- `crates/arc-daemon/src/db_worker/tests.rs`

Evidence paths added under `.arc-verify/bounded-ipc/database/`: isolated driver source/manifest/lockfile, successful and failed check output, check receipts, evidence collector and `evidence.json`. The isolated driver's manifest is an evidence fixture, not a change to the shared workspace manifests. No arc-storage source, migration, shared manifest, server, main, metrics, mission state, task instruction or harness was edited. Generated Cargo outputs remain under ignored `target/database-worker-evidence/`.

## Implemented behavior

One named native thread owns the connection from startup configuration through explicit `Connection::close`. It consumes a finite Tokio mpsc queue using `try_recv` and parks when idle. Successful enqueue and shutdown call its `Thread::unpark`; no periodic idle polling, async database tasks, blocking pool, connection mutex or per-job watcher exists.

Admission uses nonwaiting queue-slot acquisition and `try_reserve`, with no ticket/reply allocation on full rejection. An additional semaphore retains each queue slot until its queue gauge drops after dequeue. This avoids a transient metrics overrun when mpsc releases its own capacity before the received job is processed. The configured envelope is Q queued/reserved jobs plus one worker-owned job. Queued, executing and outstanding guards have separate lifetimes; cancellation never creates replacement work outside that envelope.

A ticket carries one reply receiver, its original absolute response deadline and a cancellation flag set on drop. The worker checks cancellation, sticky stop and the minimum of the response deadline and execution budget before statements and afterward. A progress callback runs every 1,000 SQLite VM instructions. The interrupt handle supplements those sticky checks, including the case where interruption arrives before a statement starts. Busy waits use a deadline-limited SQLite callback with at most 1 ms native retry slices and cancellation checks, replacing storage startup's five-second wait before readiness.

Shutdown stops admission, wakes an idle worker, interrupts SQL, closes the receiver, resolves queued work as stopping, closes SQLite and waits for actual native thread completion before joining. An elapsed shutdown deadline returns `ShutdownDeadline` with the handle retained. A test holds the thread after SQLite close and proves that a close notification cannot be mistaken for a joined worker. Drop provides a final blocking join for startup/supervisor error paths; callers must retain/report shutdown failure before destruction. Native filesystem stalls can still exceed a wall-clock budget; this task does not claim an unconditional five-second daemon exit under OS I/O failure.

Production SQL consists only of the existing fixed integrity and migration-count diagnostics. There is no hello/health operation or transport dependency. Expensive SQL, lock probes, gates and synchronization are compiled only under `cfg(test)`; no production arbitrary-SQL, sleep or gate input was added. Existing schema constraints, WAL configuration, IMMEDIATE migration transaction and startup admission-before-open ordering remain unchanged.

## Interface handoff to Chief and IPC Engineer

The shared signatures for `DbConfig`, `DbError`, `DbWorker::{start,client,begin_shutdown,shutdown}`, `DbClient::try_doctor`, `DbTicket::receive`, `DbExit`, and the test-only `start_with_test_hooks` / `WorkerTestHooks` are implemented as specified in task-1. No shared signature changed.

The IPC Engineer owns `mod db_worker;` in main and production `runtime_metrics.rs`. Those files are absent from this Actor's input snapshot, so they were not edited or copied. Add the worker module when integrating. Worker unit tests construct `Arc::new(RuntimeMetrics::default())`; the contract did not specify a constructor, so provide that conventional zero-initialized constructor or adapt only this test helper to the producer's constructor during integration. Production worker code uses only the contracted `enter` and `increment` APIs and gauge/counter variants.

Map `Busy`, `Stopping`, `Cancelled`, `Deadline` and `WorkerLost` to the contracted bounded `DB_BUSY` response. A failed join/close must remain a shutdown failure. SQL errors retain their typed source without exposing SQL/error strings in protocol responses; `Display` messages are fixed. Integration must preserve the existing successful `dispatch` path and must not route hello or health through this queue.

## Acceptance evidence

- [x] Finite queue/executing/outstanding bounds, immediate full rejection and cancellation recovery: 100,000 explicitly reconciled DB admission attempts were rejected while Q=2 and one executing job were held. Observed high waters were queued=2, executing=1, outstanding=3. A separate eight-producer cancellation case made 40,000 admission attempts while the worker drained work. These are database worker cases, not the mission's full IPC flood proof.
- [x] Active SQL interruption: synchronization occurs in SQLite's actual VM progress callback, followed by shutdown or ticket cancellation. Execution deadlines are tested separately from response deadlines.
- [x] Lock contention: an external SQLite connection holds `BEGIN IMMEDIATE`; the real worker's busy callback signals entry. Tests prove both bounded busy failure and shutdown while that callback is active, then release the lock and reopen successfully.
- [x] Cancellation/start and wake races: tests cover queued expiry, cancellation before SQL, stop after the final precheck but before SQL, early interrupt, enqueue-before-park and shutdown-before-park. Sticky progress checks terminate the expensive statement even when early interrupt has no lasting effect.
- [x] Actual closure and native join: 43 worker accounting receipts each record one start, one actual join and zero final gauges. All 43 lifecycle receipts include successful database reopen and `integrity_check=ok`; the largest recorded join/close/reopen fixture duration was 24,793 microseconds on this macOS arm64 host. This is a debug worker fixture result, not a daemon signal, RSS, thread-count or P95 measurement.
- [x] Migration unchanged: local and accepted-baseline bytes compare equal. SHA-256 remains `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`.

The isolated driver imports the actual worker files and real bundled rusqlite. Its metrics interface test double asserts each gauge bound and zero final ownership, and confirms start/join counters. Fourteen tests pass. This deliberately does not claim production metrics, supervisor, signal, health or CLI integration verification. Once the IPC Actor registers the module, the same source unit tests are available to the workspace test run. Independent oma-qa verification belongs to the assigned reviewer.

## Commands and outcomes

All Cargo verification used the existing offline cache. No dependency, browser, Playwright or browser binary was downloaded. No release build or packaging command ran. No subagents were spawned. The mission's 24-hour / three-concurrent-task budget and saved task/retry limits were not changed. Receipt-producing checks used 120-second limits and owned process groups, with serialized Cargo checks.

| Command | Outcome / evidence |
| --- | --- |
| `cargo test --offline --manifest-path .arc-verify/bounded-ipc/database/driver/Cargo.toml -- --nocapture` | Exit 0; 14 passed; `unit-tests.txt`, `unit-tests.json` |
| `cargo clippy --offline --manifest-path .arc-verify/bounded-ipc/database/driver/Cargo.toml --all-targets -- -D warnings` | Exit 0; `clippy.txt` |
| `rustfmt --edition 2021 --check crates/arc-daemon/src/db_worker.rs .arc-verify/bounded-ipc/database/driver/src/lib.rs` | Exit 0; `scoped-fmt.txt`; follows the explicit private submodule paths |
| `cargo fmt --all -- --check` | Exit 0; `workspace-fmt.txt` |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0; `workspace-clippy.txt` |
| `cargo test --workspace` | Exit 0; `workspace-test.txt`, `workspace-test.json`; includes existing subprocess, CLI and four storage preservation tests |
| `./scripts/harness/validate.sh` | Exit 1, no timeout, 49.91 s; `harness.txt`, `harness.json`; exact script unchanged |
| `python3 .arc-verify/bounded-ipc/database/collect_evidence.py` | Exit 0; actual hashes, revision scopes, lifecycle outcomes and migration comparison in `evidence.json` |
| Scoped `git diff --check` | Exit 0; new source also covered by the explicit rustfmt check |

Workspace gates here check preservation of this Actor snapshot. Because main does not yet register the new worker, their success alone does not verify worker integration. The isolated driver provides the actual worker compilation, Clippy and execution evidence.

Failed attempts retained:

1. Initial direct-path import could not resolve nested modules; rustfmt exited 1 and the first driver test compilation exited 101. Explicit `#[path]` attributes fixed module resolution without editing main.
2. The first executing test run passed 9 and failed 2 park-race tests. Its gate used `recv_timeout`, which can consume the `unpark` token being tested. The private park fixture now uses bounded `try_recv`/sleep without consuming that token. Original output remains in `unit-tests-first-failure.txt`; the final 14-test run passes.
3. Initial driver Clippy exited 101 because its reduced metrics enum had only `Db`-prefixed variants. The test double now includes the contracted non-DB gauge variants. Original output remains in `clippy-first-failure.txt`; Clippy passes without adding a lint suppression for that failure.
4. A shell here-document was denied creation of a temporary file. Supported noninteractive Python script / `-c` invocations were used afterward. No GUI or sandbox bypass was attempted. `mise` emitted nonfatal denied tracking-symlink warnings; `av.yaml` remains unreadable and was not edited. No container configuration evidence is claimed.

## Remaining mission work and risks

The unchanged harness retains four violations: `server.rs` 762 lines, `tests/resident.rs` 901 lines, `arc-ipc/src/lib.rs` 549 lines, and missing `test:coverage`. None was weakened or repaired outside ownership. New worker files are each below the 500-line cap.

Chief integration must verify the real metrics implementation, handler/task accounting, health independence, full 100,000 IPC attempts, status/doctor recovery, both signal-to-exit cases, final reports and native RSS/thread/task/P95 measurements. Production module registration and the test metrics constructor are explicit consumer dependencies. Run the exact gates again on that integrated source and obtain independent review; the current worker evidence is not a substitute.

Phase 0 tray/vendor/classification/Sweeper work, Linux qualification and Windows Named Pipe/runtime/tray obligations remain open. This task makes no completion claim for them or for native daemon performance against the SRS.

Skills used: oma-db and oma-backend, with their execution/checklist references, the installed db-engineer definition, execution policy and i18n rule. Serena tool discovery found no available provider; inspection used known-path reads and scoped Git evidence rather than a replacement repository code search. Memory lookup found no relevant ARC guidance and supplied no implementation fact.
