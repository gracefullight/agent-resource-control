# Task-4 resumed integration handoff

Status: implemented integration retained and freshly verified; ready for collection and independent task review. Actor backend, task-4, attempt 4, 2026-11-07 UTC. Skill used: oma-backend, with the installed backend-engineer definition, execution policy, result contract and backend checklist/rule. No product changes were needed in this attempt.

## Ownership and resumption

The resumed worktree already contains the repaired supervisor and report exporter from attempt 3. All 43 product files in that delivery still match its recorded hashes and modes. Seven downstream test/helper files expand the current product manifest to 50 files; none of the previous files changed. The old verifier reached its aggregate fingerprint assertion after passing individual source, artifact, receipt and boundary checks, then exited 1 because it expected the earlier 43-file manifest. That actual failure is retained in initial-verifier-observation.json. historical-binding.json records the exact additions and verifies all 124 preceding source/artifact entries. No old receipt was promoted to a fresh run.

This attempt adds only repair-2026-11-07-attempt-4/ evidence, refreshes integration/delivery.md, evidence.json and claim.json, and writes the supplemental .agents/results report. The previous three top-level files are preserved byte-for-byte as previous-delivery.md, previous-evidence.json and previous-claim.json. All earlier logs, scripts and independent evidence are retained. No product, test, documentation outside the assigned report, worker/storage module, shared module, manifest, lockfile, migration, harness, mission state, commit or ref changed. before-boundary.json and boundary-check.json cover 120 product/document/harness files.

The retained production integration consists of main.rs/server.rs registration, lifecycle.rs supervision, accept_recovery.rs policy and runtime_report.rs export. The retained regressions are lifecycle_tests.rs, accept_recovery_tests.rs, runtime_report_tests.rs and report_lifecycle_tests.rs. The worker remains its accepted single dedicated thread and bounded queue. Only doctor uses its client; hello/health remain independent. Instance admission precedes storage open, and the instance lock stays held through handler cancellation/reaping, actual worker close/join and socket cleanup.

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

## Acceptance evidence

- [x] Bounded tracked handlers and exactly one worker remain wired; hello/health bypass SQL. Accepted IPC/DB source hashes are unchanged.
- [x] The historical RED receipt (attempt-3/red-accept.json, exit 101) and source/log hashes remain retained: injected ECONNABORTED terminated the old supervisor and actual CLI status returned DAEMON_UNAVAILABLE. Fresh GREEN focused tests recover actual status and doctor after isolated ECONNABORTED and nine EMFILE failures, reset consecutive state, retain caps, and create zero retry tasks. The 100,000-error paused-time policy unit remains active; it is separate from request-load/performance acceptance.
- [x] Synchronized shutdown while backing off observes six errors, a completed hello and a reaped handler before SIGINT. Fatal EBADF preserves its cause after complete shutdown.
- [x] Real-file/subprocess tests cover disabled export, before-readiness creation, full schema/permissions, existing-file preservation, symlink/unsafe-parent refusal, retained-descriptor replacement, exact 4,096-byte acceptance and 4,097-byte refusal, expired deadline and actual EBADF write failure with cleanup/worker join/reopen integrity.
- [x] Both stalled-client/held-worker signal cases retain actual CLI DB_BUSY to IPC_ERROR, empty stdout/nonzero exit, SQL-independent health, partial header/body, instance-lock lifetime, zero final gauges, socket removal and reopen integrity. Fresh SIGINT signal-to-observed-exit was 7,631 us; SIGTERM was 7,589 us. Both reaped all eight handlers, joined exactly one worker and verified integrity=ok. These are native macOS fixture observations, not performance/SRS or other-platform qualification.
- [x] Protocol-1, same-user authentication, stable identity independent of TMPDIR, single-daemon admission, filesystem protections and successful CLI text/exit behavior remain covered by active unchanged suites. No test was removed, ignored or weakened.

## Commands and receipts

All commands ran from the mission worktree, offline, against the existing worktree-local target cache. Rust/Cargo 1.95.0, aarch64-apple-darwin. Required Cargo commands ran serially. Receipts include exact argv/cwd, UTC, observed PID/exit, timeout state, environment/toolchain, full log hashes and source-before/source-after. All six fresh checks bind the current 50-file fingerprint f9c6c5baeb3e95c48603717f08cf2abb88baf85f334250db739eb903c44b4999.

| Command | Observed outcome |
| --- | --- |
| cargo test -p arc-daemon --bin arc-daemon -- --nocapture --test-threads=1 | Exit 0; 52 passed, zero ignored/filtered. |
| cargo fmt --all -- --check | Exit 0. |
| cargo clippy --workspace --all-targets -- -D warnings | Exit 0. |
| cargo test --workspace | Exit 0; 173 passed across 17 suites, zero failed/ignored/measured/filtered. |
| ./scripts/harness/validate.sh | Exit 1; resident.rs 901 lines, arc-ipc/src/lib.rs 551 lines, missing test:coverage. Full output retained unchanged. |
| git diff --check | Exit 0; av.yaml Operation not permitted remains in the log. |

Migration bytes match accepted HEAD 42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0, SHA-256 046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384. Harness bytes also match HEAD, SHA-256 93140173df8b8bd28b067aff68301134b0398b643240b0b1e6a13da4487ebe33. No harness skip, dummy coverage tooling, dependency download, installation, standalone build or GUI/browser operation occurred. Mise tracking-config permission warnings did not prevent these checks. Serena tools were unavailable; only supplied known-path source reads and scoped Git/hash comparisons were used.

## Chief handoff

Use verify.py in this repair directory and the refreshed integration/evidence.json. Collection checks recompute source/artifact/log hashes and unchanged boundaries; they do not claim AV post-collection or independent review. No OMA run ID or claim path was injected, and none was invented. The supplied native route worked; no repeat routing probe, policy change or mission-state mutation was needed. preflight.json retains the original 2026-11-08T10:52:35.601Z deadline and existing limits without resetting spent budgets.

Independent task review remains required before downstream work resumes. Final oma-qa, mission-wide subprocess/CLI saturation/deadline/accounting assessment, native RSS/thread/task/P95 measurement and SRS comparison, operator verification and tasks 5-9 remain Chief-managed obligations; this task does not claim their completion. Retain all three harness failures. Remaining Phase 0 features, Linux qualification and Windows Named Pipe/runtime/tray/measurement obligations stay open. av.yaml remains inaccessible, so configured external target applicability is unverified; no service health, delivery or credentials were invented. No out-of-scope ownership change is requested.
