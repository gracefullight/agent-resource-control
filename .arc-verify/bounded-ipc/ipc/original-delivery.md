# IPC Engineer task-2 delivery

Status: completed for task-2's module and unit-test scope. Production wiring is reserved for task-4 and has not been performed. No claim is made that the resident binary is already resource-bounded.

Actor: backend / IPC Engineer. Task: task-2, attempt 1. Mission: b137e253-def6-46ef-a75d-25ab414784ea. Skill used: oma-backend, with its execution protocol/checklist and shared execution/result policy. No injected OMA run ID or claim path was present; this report does not invent a receipt or modify mission state. No agents were spawned.

## Source scope

Accepted committed baseline: `42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0`, tree `c802f71ac4d91ed6b8a602d5c83bf9cf57116b76`. Actor-local snapshot and unchanged local HEAD: `3816bafaa684c8099dad78be2d0cb0e474c5f08b`, tree `e79ff4ed85c94059275bbb4992579a2d55f51eb3`. The snapshot contains the task-1 input overlay, including contract/manifests; it is not the accepted committed baseline. Initial scoped status was empty. All implementation listed below is an uncommitted Actor-local diff/new file and must be collected and verified in the integrated worktree. No commit/ref, task instructions, graph or assignment was changed.

Serena tools were not available in the enabled tool inventory. Inspection used direct reads of supplied known paths and Git metadata/diffs, without native code searches. A lightweight external memory query had no relevant ARC result and supplied no implementation evidence.

## Changed files

Paths below are relative to this Actor worktree, under `crates/arc-daemon/`.

| Path | Change |
| --- | --- |
| `src/admission.rs` | New `Handlers` owning one JoinSet, connection semaphore, pre-spawn same-user authentication, nonwaiting rejection, completion reaping and bounded abort/drain. New shared request semaphore/RAII admission. |
| `src/connection.rs` | New sequential request handler: header before request admission/body allocation, payload/raw-body release before database await, existing protocol dispatch, bounded errors and one overflow fallback. |
| `src/limits.rs` | New validated default/test profiles with positive finite caps and budgets. Rejects unknown profiles, arithmetic overflow and invalid deadline relationships. |
| `src/runtime_metrics.rs` | New fixed arrays of gauges/high waters and saturating counters, RAII guards, retained-task accounting, fixed-key JSON snapshot and test-only lossless change notification. |
| `src/transport.rs` | New Tokio readiness I/O with absolute deadlines; existing FrameError/size policy; capped direct JSON writer with checked exact reservation, prefix/body sharing one write deadline. |
| `src/platform.rs` | Generalizes `ensure_same_user` to `&(impl AsRawFd + ?Sized)`. Existing Linux SO_PEERCRED/macOS getpeereid UID comparisons and unsupported-platform policy are unchanged. |
| `src/transport_tests.rs` | Deterministic transport/admission tests, 474 lines. |
| `src/transport_test_support.rs` | Test-only Unix-pair/frame helpers. |
| `tests/transport_unit.rs` | Includes owned modules as one integration test target before task-4 module declarations. The harness permits unused functions because server wiring is intentionally absent; product source has no dead-code suppression. |

Only platform.rs is a modification of existing tracked product code. The other eight files are new, currently untracked. Shared manifests/lockfile, server.rs, main.rs, arc-ipc and CLI files were not edited. No SQL/worker implementation or database mutex was added.

## Acceptance evidence

- [x] Finite admission and accounting: tests attain C=4, retain four completed entries before reap, reject 100 additional accepted sockets without invoking a handler factory, then reap before replacement. Spawned/reaped totals match. Rejection work high water is one and tracked-task high water is four. A separate two-worker test performs 16,000 competing request admissions and asserts the request high water never exceeds R; normal, unwind and cancellation paths return gauges to zero.
- [x] No body allocation at request saturation: valid, zero and oversized declared lengths all close immediately while all request permits are held; body high water remains zero. Oversized body API calls defensively reject before allocation.
- [x] Absolute reads: paused-time Unix-pair tests cover idle headers, trickled header bytes, partial/trickled bodies, and a header completed just before its deadline followed by expiry at the original whole-frame deadline. Partial progress cannot renew either deadline.
- [x] Absolute writes: native SO_SNDBUF is reduced and a 512 KiB response is proven pending before time advances. The non-reading peer times out at the original write budget. A handler-level case verifies request/response reservations return to zero after blocked-write expiry.
- [x] Protocol/error compatibility: malformed JSON receives INVALID_REQUEST and continues; version 99 doctor is rejected before invoking the database callback; a following hello succeeds. Zero/oversize prefixes receive existing empty-ID errors and close. An exactly full legal request producing an oversized echoed response receives one small empty-ID FRAME_TOO_LARGE fallback and closes. Oversized serialization writes no prefix before failing. Typed busy and genuine-integrity outcomes remain distinct.
- [x] Same-user authentication: native Tokio Unix pairs pass; a non-socket descriptor fails the same native credential call. Existing UID-comparison branches were preserved byte-for-byte apart from import/signature adaptation. This does not claim a new foreign-UID subprocess test.
- [x] Material deliverable: this report, source hashes below and executable test target document implementation, checks and integration prerequisites.

The new target has 18 passing tests (including local limits/counter units). Resource structural bounds are not RSS or latency measurements. The 16,000 request-permit race and 100 admission rejections do not substitute for the mission's later 100,000 real request-attempt workload.

## Task-4 integration handoff

Add production module declarations for `admission`, `connection`, `limits`, `runtime_metrics`, `transport`, and the SQLite Engineer's `db_worker`. Do not declare the test support/tests in production. Keep the existing serve(Connection, PathBuf) entry point and instance-lock/startup ordering. Configure the main Tokio runtime with two workers. Read `ARC_IPC_PROFILE` with var_os and pass to RuntimeLimits::from_profile; validate before readiness.

Construct one `Arc::new(RuntimeMetrics::new(&limits))`, one `Requests::new(&limits, metrics.clone())` and one `Handlers::new(&limits, metrics.clone())`. Requests must be shared across handlers, not constructed per connection. Only the supervisor owns Handlers. Select signals first, then `handlers.join_next()` when `handlers.len() != 0`, alongside listener.accept; try_spawn also reaps ready entries before every admission. It invokes its factory only for an authenticated admitted socket. There is no reject task, task waiting for a permit or per-request task.

`connection::handle_connection(stream, limits, requests, metrics, doctor)` accepts one doctor adapter `Fn(std::time::Instant) -> Future<Output = DoctorOutcome>`. This added local callback boundary keeps SQLite types outside transport and consumes the declared worker API without changing it. The callback must synchronously call `DbClient::try_doctor(deadline)` and return a future that owns its `Result<DbTicket, DbError>` and, on success, awaits `DbTicket::receive()`. Dropping that future must drop the ticket. No detached timeout watcher is needed. The handler fixes the real monotonic queue-to-reply deadline and also bounds the future with timeout_at. It invokes this callback only for decoded protocol-1 doctor, never hello/health, unknown operations or incompatible versions.

Map a successful report to `DoctorOutcome::Ready(report)`. Map Busy, Stopping, Cancelled, Deadline, WorkerLost, unavailable I/O and SQLite DatabaseBusy/DatabaseLocked/interrupt errors to Busy; map genuine integrity/SQL failures to IntegrityFailed. Keep this DbError/SQLite classification in the task-4 worker adapter, not in transport. Worker-side DbRejected/DbTimeouts accounting remains with the worker; the transport counts an outer response deadline itself. No raw database error string enters a response or per-request log.

The saved assignment does not assign arc-ipc writes to task-2. Needed shared-path change for the Chief/task-4 writer: add `pub const ERROR_DB_BUSY: &str = "DB_BUSY";` to `crates/arc-ipc/src/lib.rs` and replace the identical temporary connection-local constant with that import. Wire behavior is already DB_BUSY; the synchronous protocol API remains unchanged. The existing CLI maps this error to IPC_ERROR without a CLI change. This report is the written producer/consumer handoff for the callback/outcome addition; no existing task-1 signature was changed.

At shutdown, stop listener/admission and call worker.begin_shutdown before awaiting handlers. Handlers::shutdown(deadline) sets sticky stopping, aborts all tasks and reaps all retained entries; subsequent try_spawn calls reject. Supply the contract's handler-stage deadline within the one signal-observation budget. Then join/close the actual worker under that same overall deadline. A handler-drain timeout returns an error with entries retained, never a success claim. Keep Handlers/worker/instance lock alive until accounted for. RuntimeMetrics::snapshot exposes gauge(g), counter(c), tracked_tasks, caps and json() for the task-4 secure optional final-report writer; there is no report-path I/O in these modules.

## Commands and outcomes

All checks were native on the provided macOS worktree using Rust 1.95.0 and cached dependencies. Tests may compile their own test targets; no standalone build, package or browser tooling was run. Saved mission limits were preserved (max parallel 3, 86,400 seconds, 200 runs, 3 retries); this Actor spawned no extra workers/agents. Owned cargo sessions were observed through process completion.

| Command | Outcome |
| --- | --- |
| `git status --short`, `git rev-parse HEAD`, manifest/contract/source direct reads | Initial tracked scope clean; local snapshot HEAD confirmed. Git emitted av.yaml permission warnings. |
| Initial `python3` heredoc for platform edit | Failed before execution: zsh could not create a temporary heredoc file, Operation not permitted. No source change from it. Supported apply_patch edits succeeded; no GUI or privilege workaround used. |
| `cargo test --offline --locked -p arc-daemon --test transport_unit` | Initial 12 tests passed. Expanded 17-test run found one incorrect overflow fixture (response remained below cap). Fixture was corrected to an exactly full legal input; 17 tests then passed. No assertion/requirement weakened. |
| `rustfmt --edition 2021` on owned Rust paths | Exit 0; final tests were split to keep each new file under 500 lines. |
| `cargo fmt --all -- --check` | Final exit 0. |
| `cargo clippy --workspace --all-targets -- -D warnings` | First exit 101 for field_reassign_with_default in a new test. Replaced with struct initializers; final exit 0 after the final test-file split. |
| `cargo test --workspace` | Final exit 0: CLI binary 2, core unit 2/runtime 3, daemon unit 17/resident 11/startup 1, new transport 18, IPC 13, storage 4. All doc tests passed. Resident and CLI cases exercise the existing production wiring; they do not prove that task-4 integration has happened. |
| `git diff --check` | Exit 0. |
| `git diff --exit-code 42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0 -- crates/arc-storage/migrations/0001_phase0.sql` | Exit 0. Migration remains byte-identical to accepted committed baseline. |
| `shasum -a 256 crates/arc-storage/migrations/0001_phase0.sql` | `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. |
| `git diff --exit-code HEAD -- Cargo.toml Cargo.lock crates/arc-daemon/Cargo.toml crates/arc-daemon/src/main.rs crates/arc-daemon/src/server.rs crates/arc-ipc/src/lib.rs crates/arc-cli/src/main.rs` | Exit 0; reserved/shared paths unchanged from Actor input snapshot. |

mise repeatedly warned that tracking-config symlinks outside the worktree were not permitted. Required checks nevertheless completed with the exit codes above. No dependencies or browser packages were installed/downloaded. No browser/ComputerUse or screen checks ran.

## Actor-local delivery hashes (SHA-256)

```text
cc80fca15e83d1b73dc20f99d619223e46425016588f9a1e649c81d0bc27b883  crates/arc-daemon/src/admission.rs
90c31e454a00b698dd1390f6dbe18edc9211f27a34b3d5848e43602142ed2195  crates/arc-daemon/src/connection.rs
cd9a9afd80274b2e61f16880395382f73e2dda2a17540557b8188d7be8b47410  crates/arc-daemon/src/limits.rs
20f383e53f94e4180e0e3165c57edf1f4c0d9d5c9f1d4343053efb2cb1b5b7e5  crates/arc-daemon/src/platform.rs
23ac33070705808edd12d0d32d854940faf6184336955746ec53cacf93f28c1d  crates/arc-daemon/src/runtime_metrics.rs
37fd61770d74b57e9b296a697bfe379ee6d1e6be45006b0dadaa7cb3223f7de0  crates/arc-daemon/src/transport.rs
00de7f22e28ea681647edeebb1bbd7c09e85695591231778c86097d1bc56207f  crates/arc-daemon/src/transport_test_support.rs
d75ef33f0d329b982acf2bdb1ff7b89d8dfae4f9ecbb2b353b6ccdb4c8e5307b  crates/arc-daemon/src/transport_tests.rs
43dd4f3d92807d03a97a0f1fa1d1a569e064cf373d1eca4d3d3fd8fb0952fd4b  crates/arc-daemon/tests/transport_unit.rs
```

## Remaining mission work and risks

Task-4 production supervision, secure ARC_RUNTIME_REPORT creation/final accounting, worker adaptation and signal integration remain prerequisites. The SQLite worker is a separate Actor deliverable absent from this isolated input; no queue/cancellation/close claim is made here. No manifests require changes for these modules.

Later tasks must still prove the 100,000 reconciled real request attempts with cap attainment, status/doctor recovery, health during controlled slow SQLite work, SIGTERM and SIGINT below five seconds with stalled clients, all handlers/worker accounted for, and database reopen/integrity. Native peak RSS, native thread/task counts and small-message P95 remain unmeasured by this Actor; compare later actual measurements with the task-1 SRS evidence. No performance result is inferred from semaphore arithmetic or these test times.

The unchanged harness was not run in task-2; the contract assigns its final run to integration/review. Retain and rerun the known conformance failures (existing large Rust files and missing test:coverage) instead of suppressing them. No harness script or acceptance criterion was changed. Independent oma-qa review remains separate. Phase 0 feature obligations, Linux qualification and Windows Named Pipe/runtime/tray work remain open.
