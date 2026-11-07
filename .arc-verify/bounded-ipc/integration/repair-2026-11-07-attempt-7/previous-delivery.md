# Task-4 integration delivery, attempt 5

Status: retained implementation verified on the Actor snapshot; ready for collection and independent review. Skill: oma-backend, installed backend-engineer role. No product edit was needed.

## Scope and provenance

Accepted baseline: 42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0, tree c802f71ac4d91ed6b8a602d5c83bf9cf57116b76. Actor-local HEAD and input snapshot: 2fe61708aeffcd3638c7e9331a3ef3b4f8394ddf, tree 327fef63bace20053064f9662e5b1a5f7a2ea761. The supplied snapshot/worker evidence refs do not resolve locally (rev-parse exit 128); the exact supplied commits and trees resolve and baseline ancestry is verified. No refs were created or changed. Integration observations below describe that isolated snapshot and its actual current files; they do not claim those changes exist in the accepted baseline or receiving checkout.

The retained implementation registers lifecycle.rs, accept_recovery.rs and runtime_report.rs through main.rs/server.rs. The supervisor owns finite task admission, cancellation/reaping, the single worker, owned-socket cleanup and optional final export. Only doctor uses the DB client; hello/health bypass the queue. Instance admission still precedes storage opening. main keeps the instance lock through actual worker join and socket cleanup. Existing protocol, same-user authentication, TMPDIR-independent identity, no-follow and filesystem protections remain active.

All 50 current product inputs bind to fingerprint b3610549adfebffdd626a760741a200c848ec7ee26a88482bcf7e38e563383c4. The only source differences from attempt 4 are preexisting resident.rs and tests/support/mod.rs fixture-lock changes. No worker, storage, shared transport/metrics/admission/limits/platform, manifest, lockfile, migration, harness or test changed in this attempt. before-boundary.json covers 120 product/document/harness files. Existing tests were not removed, ignored or weakened.

This attempt writes repair-2026-11-07-attempt-5/, refreshes this delivery and integration/evidence.json and claim.json, and writes the attempt-5 result report. It also restores the exact indexed attempt-3 and attempt-4 result reports from the retained source workspace (16,368 and 3,967 bytes, respectively). Restored hashes and source paths are recorded. The prior delivery, evidence index and claim are preserved byte-for-byte in previous-* files. The old checker’s exit 1 at resident.rs is retained; old receipts remain historical. No commits, refs, mission state, assignments, routing policy or budgets changed.

## Authoritative producer/consumer interface

**ARC_RUNTIME_REPORT is the authoritative optional accounting file. The earlier stdout substitution was rejected and is superseded.** Task-5 and final QA must consume this file. No stdout fallback, consumer contract relaxation or ownership transfer is needed.

When set, the path must name a new file in an existing owned 0700 directory. Before readiness, server::open_private_directory validates the parent; directory-relative openat uses O_CREAT, O_EXCL, O_NOFOLLOW, O_CLOEXEC and O_NONBLOCK. The descriptor is checked as an owned, single-link, 0600 regular file and retained until the final write. Existing files/links are refused without changes. The pathname is never reopened. Disabled export creates no report and emits no runtime stdout record; readiness and successful CLI text stay unchanged.

Invalid export paths fail startup before a listener/readiness and still stop and join the worker. A later startup failure may leave the newly created empty file. At shutdown, complete serialization uses a fixed 4,096-byte buffer before any file write. Overflow and OS I/O failures return errors, without truncation or success claims. Consumers require complete parseable JSON and an observed successful process exit. A write failure may leave a partial file. Cleanup, cancellation and real join accounting occur before report handling, including when report writing fails.

The complete schema remains:

- event = ipc_shutdown; reason = SIGTERM, SIGINT or accept_error; profiles = {ipc: default|test}; limits records effective connections, requests, db_queue, db_executing, max_frame_bytes and header_timeout_ms, frame_timeout_ms, write_timeout_ms, db_response_timeout_ms, db_execution_timeout_ms, db_busy_timeout_ms, shutdown_timeout_ms.
- metrics retains every existing current/high_water gauge: connections, requests, request_body_bytes, response_bytes, db_queued, db_executing, db_outstanding, rejection_work and tracked_tasks. Counters: accepted, connection_rejected, request_rejected, db_rejected, frames_completed, header_timeouts, frame_timeouts, write_timeouts, db_timeouts, worker_started, worker_joined, handlers_spawned, handlers_reaped. caps: connections, requests, db_queue, db_executing and supervisor.
- supervisor separately contains accept_errors, transient_errors, resource_pressure_errors, other_errors, fatal_errors, accept_retries, consecutive_errors, consecutive_errors_high_water, backoff_high_water_ms, shutdown_during_backoff, retry_tasks (=0), supervisors {current:1, high_water:1}. The supervisor is still executing export; observed process exit establishes its termination.
- handlers_remaining, handlers_reaped, worker_joined, connection_closed, gauges_zero, socket_removed, accounted, deadline_met, shutdown_elapsed_ms and accept_error_os_code retain the existing final-accounting semantics. connection_closed refers to SQLite. shutdown_elapsed_ms is sampled before serialization/write; the final return also checks the same absolute deadline after writing.

An absent/empty IPC profile selects default; the profile field identifies that selection and limits always records actual effective configuration. Test-only supplied limits can differ. The signal shutdown budget remains one absolute 3,500 ms deadline covering cancellation, reaping, worker termination/close/join, socket cleanup and export. No production SQL/test-control environment hook exists. Native filesystem/SQLite kernel stalls cannot be forcibly preempted; deadline and I/O failures remain failures.

## Accept recovery

The supervisor retains fixed-size saturating u64 counters, one inline retry future and no retry tasks, queue, per-error logs or history. EINTR/EAGAIN/ECONNABORTED/EPROTO/ECONNRESET are transient; EMFILE/ENFILE/ENOBUFS/ENOMEM are resource pressure. Both retry after 5, 10, 20, 40, 80, then capped 100 ms delays, without a fatal count threshold. Other errors retry conservatively. Only invalid-listener EBADF/EINVAL/ENOTSOCK/EOPNOTSUPP cause accounted termination, preserving the original OS cause. Successful acceptance resets consecutive state. The signal-first select continues reaping handlers during backoff. Injection and bounded watch synchronization exist only under cfg(test).

## Fresh acceptance evidence

- Bounded tracked handlers and one accounted SQLite worker remain wired. All 52 focused daemon tests passed with zero ignored/filtered tests.
- The retained historical RED is attempt-3/red-accept.json (exit 101): isolated ECONNABORTED ended the prior supervisor and actual CLI status returned DAEMON_UNAVAILABLE. Its original source/log hashes are preserved. Fresh GREEN recovers actual status and doctor after one ECONNABORTED and nine EMFILE failures, resets consecutive errors and reports zero retry tasks. The paused-time 100,000-error policy test remains active; it is separate from mission request-load evidence.
- Backoff shutdown synchronizes six EMFILE errors, one completed hello and one reaped handler before SIGINT. Fatal EBADF retains its OS cause after accounted shutdown.
- Real-file/subprocess coverage retains disabled export, safe before-readiness creation, complete schema and 0600/0700 permissions, existing-file preservation, symlink/unsafe-parent refusal, retained-descriptor safety, exact 4,096-byte success and 4,097-byte refusal, expired deadlines and real EBADF write failure with cleanup/join/reopen integrity.
- SIGINT signal-to-observed-exit: 7,217 us; SIGTERM: 9,484 us. Both cases reap all eight handlers, join exactly one worker, close SQLite, remove the socket, leave zero gauges, retain the instance lock through shutdown and pass reopen/integrity=ok. Stalled header/body and held-worker/full-queue fixtures retain actual CLI DB_BUSY -> IPC_ERROR with empty stdout/nonzero exit and health independence. These are native macOS fixture observations, not other-platform or SRS performance qualification.

## Commands and actual outcomes

Commands ran serially from this Actor worktree using Rust/Cargo 1.95.0, aarch64-apple-darwin, CARGO_NET_OFFLINE=true and the existing Cargo source cache. Receipts record exact argv/cwd, UTC, observed PID/exit, environment/toolchain, timeout status, full output SHA-256 and source before/after. Required Cargo gates bind the same 50-file fingerprint.

| Command | Outcome |
| --- | --- |
| cargo test -p arc-daemon --bin arc-daemon -- --nocapture --test-threads=1 | Initial prepared-cache attempt: exit 101, parent target/debug/.cargo-lock Operation not permitted. Retained in focused-cache-denied.*. |
| Same focused command with local target | Exit 101: 48 passed, four failed because local arcctl prerequisite was absent. Retained in focused-missing-cli.*. |
| cargo test -p arc-cli --test binary | Exit 0, two passed; establishes the real local arcctl through a test target. |
| cargo test -p arc-daemon --bin arc-daemon -- --nocapture --test-threads=1 | Fresh final exit 0, all 52 passed. |
| cargo fmt --all -- --check | Exit 0. |
| cargo clippy --workspace --all-targets -- -D warnings | Exit 0. |
| cargo test --workspace | Exit 0; 173 passing invocations across 17 suites, zero failed/ignored/measured/filtered. |
| ./scripts/harness/validate.sh | Exit 1, three violations: resident.rs 901 lines; arc-ipc/src/lib.rs 551 lines; missing test:coverage. |
| git diff --check | Exit 0; av.yaml Operation not permitted is retained in output. |

The denied shared-cache write was not retried. Checks use a writable worktree-local target with existing offline sources. No installation, download, standalone build, browser/GUI operation, deployed-service change or SQL/test-control production hook occurred. Compilation was only through explicitly required test/Clippy commands. Mise tracking-config permission warnings did not prevent these checks. Serena was unavailable; inspection used supplied paths and scoped Git/hash comparisons.

Migration bytes match accepted baseline, SHA-256 046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384. Harness bytes match accepted baseline, SHA-256 93140173df8b8bd28b067aff68301134b0398b643240b0b1e6a13da4487ebe33. Original evidence and failed attempts remain indexed, with historical source bindings distinguished from current ones.

## Chief handoff and remaining work

Run repair-2026-11-07-attempt-5/verify.py after collection and inspect the refreshed index. Local collection validation does not establish receiving-checkout collection or independent review. No OMA run ID or claim path was injected; none was invented. The supplied native route worked. preflight.json preserves the saved deadline 2026-11-08T10:52:35.601Z and mission limits without resetting spent budgets.

Independent task review remains required before task-5/task-6 resume. Task-5 and final QA must consume ARC_RUNTIME_REPORT. The former stdout substitution is rejected and superseded; the existing contract is unchanged and no ownership handoff is requested. Keep mission completion open. Tasks 5-9 retain mission-wide subprocess/CLI accounting, saturation/deadline/recovery assessment, native RSS/thread/task/P95 measurements against SRS targets, final independent oma-qa and operator verification. Running existing workspace tests here does not replace those reports or reviews.

Retain all three actual harness failures: resident.rs at 901 lines, arc-ipc/src/lib.rs at 551 lines and missing test:coverage. No skips or dummy tooling were added. Configured target applicability remains unverified because av.yaml is inaccessible; no external health or delivery proof was invented. Remaining Phase 0 features, Linux qualification and Windows Named Pipe/runtime/tray/measurement obligations remain open.
