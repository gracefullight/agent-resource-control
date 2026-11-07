# Task-4 integration delivery

Status: completed assigned implementation and focused verification; ready for independent review. This is not a mission-wide completion claim. Actor: backend / IPC Engineer; task-4, attempt 1. Skill used: oma-backend. Read the supplied backend-engineer definition, backend execution protocol/checklist, shared execution/result policies, backend rule, and accepted bounded IPC contract and dependency deliveries. Existing Rust/Tokio/rusqlite stack retained.

## Changes and ownership

| Changed product path | Result |
| --- | --- |
| `crates/arc-daemon/src/main.rs` | Registers delivered modules and the new lifecycle module; selects exactly two Tokio runtime workers. Per-user instance admission still precedes SQLite open, and its binding remains held across server shutdown. |
| `crates/arc-daemon/src/server.rs` | Replaces blocking connection threads/shared SQLite mutex with validated limits, fixed metrics, one DbWorker and the shared supervisor. Retains `serve(Connection, PathBuf)` and adds the contracted cfg(test) `serve_with_worker(PathBuf, RuntimeLimits, Arc<RuntimeMetrics>, DbWorker)`. |
| `crates/arc-daemon/src/lifecycle.rs` | New supervisor: signal-first select, continuous JoinSet reaping, nonwaiting authenticated admission, async connection handlers and typed worker adapter. One shutdown deadline covers cancellation/draining, actual worker close/join, owned socket removal and accounting. |
| `crates/arc-daemon/src/server_tests.rs` | Moves all ten original server tests; adapts the two framing cases to real async handlers and the real worker while retaining their assertions. Eight filesystem cases retain their statements, with one rustfmt line wrap. |
| `crates/arc-daemon/src/lifecycle_tests.rs` | Adds two owned signal subprocess cases, a cfg(test) child entry and bounded fixture helpers. Exercises real CLI overload mapping, health independence, stalled header/body, actual joins, lock lifetime and reopen integrity. |
| `crates/arc-daemon/src/connection.rs` | Replaces the temporary local DB_BUSY string with the public protocol constant; handler behavior unchanged. |
| `crates/arc-ipc/src/lib.rs` | Adds public `ERROR_DB_BUSY` only; framing, dispatch and synchronous CLI interfaces unchanged. |

No database-owned file was edited. `boundary-check.json` verifies all four accepted worker source/test hashes, shared limits/metrics, the accepted transport/admission/platform code and dependency evidence. The sole changed delivered transport file is connection.rs for its planned public constant import. Shared Cargo manifests and Cargo.lock match the accepted task-3 product snapshot. Platform.rs retains task-2's already accepted generic socket authentication adaptation unchanged. No ownership handoff was needed.

All filesystem preparation, validation and socket-binding functions from the original server are byte-identical. Instance.rs, arc-core identity, CLI source, resident/startup assertions and harness script are byte-identical to accepted HEAD `42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0`. Migration `0001_phase0.sql` remains byte-identical, SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`.

## Integrated interfaces and behavior

Production uses the delivered Handlers/Requests and worker APIs without signature changes. Default limits remain C=64, R=16, Q=8 plus one executing SQL job; explicit ARC_IPC_PROFILE=test selects C=8, R=4, Q=2. Unknown profiles fail. Headers, frames, writes and DB responses retain the accepted absolute deadlines. There are no connection OS threads, per-request tasks or tasks waiting for permits. Same-user checks occur before handler spawning. Hello/health never call the worker adapter. Decoded doctor requests use immediate queue admission and the existing bounded response deadline. Typed busy, stopped/lost worker, cancellation, deadline and SQLite contention/interruption return DB_BUSY. Existing CLI error-first handling produces exactly IPC_ERROR, nonzero, rather than a version or integrity diagnosis. Successful CLI output is unchanged.

Both signals enter the same shutdown path. Drop the listener, set sticky worker shutdown and interrupt SQL, abort/drain every retained handler, then call the worker's actual shutdown/join with the same absolute S+3500 ms deadline. Remove only the recorded owned socket inode. Return failure if shutdown, cleanup, deadline or final accounting fails. The instance lock remains bound in main throughout. Worker Drop's existing last-resort join remains unchanged; an OS filesystem/close stall can exceed the budget and cannot be represented as successful bounded shutdown.

Exactly one fixed-key stdout line is emitted at lifecycle end:

`arc-daemon runtime: {"event":"ipc_shutdown", ...}`

Its top-level fields are event, reason, shutdown_elapsed_ms, deadline_met, handlers_reaped, worker_joined, connection_closed, gauges_zero, socket_removed, accounted and metrics. Metrics uses the accepted fixed Snapshot JSON schema with counters, current/high-water gauges and configured caps. No request IDs, SQL errors, per-attempt log, historical vector, new protocol operation or production test gate was added. Normal readiness text remains unchanged. Accounting is available to bounded subprocess fixtures by parsing this single record; cfg(test) fixtures can also hold Arc<RuntimeMetrics> and use wait_for. An integrated test target importing server.rs must also declare lifecycle.rs alongside admission, connection, db_worker, limits, runtime_metrics, transport and platform; server's cfg(test) tests use the existing transport_test_support module.

## Acceptance evidence

- [x] Integrated bounded tracked handlers and one SQLite worker, with database-independent health. Final workspace tests compile the real production module declarations. Final controlled subprocesses actually reach Q=2, executing=1, outstanding=3. The real arcctl doctor receives exactly `IPC_ERROR\n`, empty stdout and nonzero exit while that queue is full. Admitted daemon.health returns status ok while the worker is held before SQL. No gate release is sent.
- [x] Both signals stop owned fixtures and account for resources. Final focused run: SIGINT signal-to-observed-exit 7,239 us; SIGTERM 7,587 us, each asserted strictly below five seconds. Each fixture has a held worker, two queued doctors, a partial header and a synchronized partial body. Eight handlers are spawned and reaped, WorkerStarted=WorkerJoined=1, every current gauge and tracked_tasks=0, socket removed, SQLite closed and actual native worker joined. Parent reopens/migrates SQLite and verifies PRAGMA integrity_check=ok. The child independently proves that admission is still refused after join/socket cleanup while its instance binding is alive, then reacquires after dropping it. These are native macOS test-executable subprocess observations using the production supervisor, not performance benchmarks or cross-platform results.
- [x] Protocol/CLI/identity/filesystem assertions retained. All ten original server cases remain. The full workspace run passes the existing eleven resident subprocess cases, core identity tests, startup failure test, thirteen arc-ipc cases, CLI binary tests, storage tests, eighteen transport-target cases and sixteen worker-target cases. No tests were ignored, removed or weakened.
- [x] Material report, receipts, source fingerprints and boundary evidence delivered here. The .agents/results copy is supplemental; collection must preserve this self-contained .arc-verify delivery.

## Commands and outcomes

Commands ran serially from the mission worktree, offline with existing caches, Rust/Cargo 1.95.0 aarch64-apple-darwin. No install/download, standalone build, GUI/browser check, agent dispatch, Git reset/commit/ref mutation or mission-state operation was performed. Tests compile their required binaries as part of Cargo test. `record.py` retains complete combined output, argv/cwd, UTC timestamps, PID/observed exit, timeout state, source-before/source-after fingerprint and log hash. Command budget was 120 seconds with owned-process-group cleanup on timeout; no timeout occurred. Saved mission limits were retained (maxParallel=3, maxDurationSec=86400, maxRuns=200, maxRetries=3); this Actor ran one Cargo command at a time.

| Command | Outcome and receipt |
| --- | --- |
| `cargo test -p arc-daemon --bin arc-daemon -- --nocapture` | Exit 0, 37 passed; initial warnings retained in focused-initial.txt. |
| `rustfmt --edition 2021 --config skip_children=true` with the seven owned Rust paths, then lifecycle_tests.rs after its small repair | Exit 0; no foreign module formatted. |
| `cargo fmt --all -- --check` | Exit 0; final receipt fmt-final.json. |
| `cargo clippy --workspace --all-targets -- -D warnings` | First exit 101 for one needless borrow in lifecycle_tests.rs; clippy.json/txt retained. Repaired at its source. Final exit 0, clippy-final.json. No lint suppression added to production. A cfg(test)-only allowance covers unused helpers from the shared test-support module, as in the existing standalone target. |
| `cargo test --workspace` | Exit 0; 37 daemon binary tests, all other suites pass, zero ignored/filtered. workspace-test.json/txt. |
| `cargo test -p arc-daemon --bin arc-daemon lifecycle_tests -- --nocapture --test-threads=1` | Exit 0, three cases pass including two real signal children. Final lifecycle-final.json/txt and extracted signals.json. The child-entry case returns without spawning when unselected. |
| `./scripts/harness/validate.sh` | Exit 1, three unresolved violations; complete unchanged invocation retained in harness.json/txt. |
| `git diff --check` | Exit 0; also reported av.yaml access denial. Scoped product checks and byte/hash comparisons avoid inferring that denied file's state. |

All final Cargo receipts, final lifecycle receipt and harness receipt bind the same unchanged 38-file product fingerprint `e416d6c25ae8131fbc4f6ca093335cd502f6f51c61cf7ae3ef033cabf52a65fe` (see their source snapshots for the exact file count). Initial evidence extraction comparing moved filesystem bodies byte for byte failed because rustfmt moved one line break; test-preservation-initial.txt retains the exact diff. The corrected check confirms identical test names and identical filesystem statements after whitespace normalization. This was a comparison-script failure, not a failed product assertion. The first artifact-index attempt also mixed relative/absolute paths and failed before creating evidence.json; its chained checker consequently could not open that file. evidence-index-initial.txt retains the failure. collect.py fixes the path handling; the read-only checker verifies the resulting index. No product check was bypassed.

Harness failures remain: resident.rs 901 lines; arc-ipc/src/lib.rs 551 lines; missing Bun test:coverage script. The prior server.rs length violation is resolved by the responsibility split (server.rs now 312 lines; adjacent test/lifecycle modules each below 500). Harness bytes and thresholds were not changed; no dummy coverage, JavaScript tooling or skip was introduced. This integration's harness record does not replace later final QA on changed inputs.

## Handoff and limits

Ready for the Chief's collection and independent task review. `verify_delivery.py` verifies this delivery against current files and receipts without running tests. No OMA run ID or claim path was injected, so none was invented and no new managed mission was opened. Reports use the supplied actor/task/attempt and workspace identity. av.yaml remains OS-denied; no service or container state was inferred, no deployed restart/rollback performed.

Task-5 still owns mission-wide deterministic 100,000 application-attempt accounting, cap-attainment saturation stages, production subprocess deadline/recovery cases, extended actual CLI recovery, and native measurement fixtures. The existing worker-only 100,000 rejection test is not substitute evidence. Native peak RSS, native thread/task counts and small-message P95 comparison against the located SRS are not measured by this delivery. Independent oma-qa verification, final QA JSON/performance artifacts and final mission evidence remain later tasks. Remaining Phase 0/tray/vendor/classification/Sweeper behavior, Linux qualification and Windows Named Pipe/runtime/tray obligations remain open. Native filesystem stalls and scheduler behavior beyond these observed fixtures remain material limits.
