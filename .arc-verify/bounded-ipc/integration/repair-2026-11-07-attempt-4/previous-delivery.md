# Task-4 repaired integration delivery

Status: assigned repair implemented and locally verified; ready for collection and independent task review. Mission completion remains open. Actor: backend / IPC Engineer, task-4, attempt 3, 2026-11-07 UTC. Skill: oma-backend; backend-engineer role, execution policy, result contract, backend checklist/rule and the accepted contract/dependency deliveries were used. The existing Rust, Tokio and rusqlite stack is retained.

The earlier unconditional `arc-daemon runtime: ...` stdout substitution was rejected and is superseded. **The authoritative accounting channel is the optional `ARC_RUNTIME_REPORT` file**, as required by `docs/bounded-ipc-contract.md`. Task-5 and final QA must read that file. No consumer contract change or ownership handoff is needed. The previous delivery, index and claim are preserved byte-for-byte in `repair-2026-11-07-attempt-3/previous-*`; original receipts, logs, collector and verifier remain historical evidence and must not be rerun as current collectors. The repaired collection verifier is `repair-2026-11-07-attempt-3/verify.py`.

## Owned changes

This repair changes only these product files relative to the resumed worktree:

| Path under `crates/arc-daemon/src/` | Change |
| --- | --- |
| `main.rs` | Registers accept recovery and runtime report modules; instance admission, SQLite startup order and lock lifetime unchanged. |
| `lifecycle.rs` | Recoverable accept failures use one bounded backoff in the supervisor select. Startup failures explicitly stop/join the worker. Final file export follows cancellation, handler reaping, worker close/join and socket cleanup under the same shutdown deadline. |
| `accept_recovery.rs` | Fixed-size saturating supervisor counters, error classification and capped retry policy; cfg(test)-only accept-result injection and bounded watch synchronization. |
| `runtime_report.rs` | Validated parent open, directory-relative create-new/no-follow 0600 file creation, retained descriptor, fixed 4,096-byte serializer and final write/deadline/error handling. |
| `lifecycle_tests.rs` | Reads the contracted report; retains existing signal, slow-worker, CLI DB_BUSY, health, lock-lifetime and integrity assertions. Adds common schema/accounting assertions. |
| `accept_recovery_tests.rs` | Owned subprocess regressions with real CLI status/doctor recovery, reaping during backoff, signal responsiveness and retained fatal OS cause. |
| `runtime_report_tests.rs` | Real-file permissions/refusal/descriptor tests, exact size boundary, expired deadline and actual EBADF write failure. |
| `report_lifecycle_tests.rs` | Owned subprocess export/disabled/startup-failure tests and actual write-failure cleanup/accounting proof. |

No worker/storage, shared limits/metrics/admission/transport/platform, manifests, lockfiles, migrations, harness, independent evidence or saved mission/task definitions were edited. `boundary-check.json` checks all preexisting product bytes: only the three owned existing paths changed and the five listed modules were added. Accepted database and IPC input hashes, shared manifests and all baseline filesystem/identity/CLI files still match. The previous integration wiring in server.rs, server_tests.rs, connection.rs and arc-ipc is preserved. No preceding test was removed, ignored or weakened.

Migration `crates/arc-storage/migrations/0001_phase0.sql` remains byte-identical to accepted HEAD `42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0`, SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. Harness bytes match that HEAD too. All protocol-1 framing, successful CLI text/exit behavior, same-user authentication, stable TMPDIR-independent identity, single-daemon admission and directory/symlink protections remain covered by the active preservation suites.

## Accept recovery policy

A successful listener acceptance resets consecutive failures and the retry deadline, including an acceptance subsequently rejected by admission/authentication. Failed accepts create no task, log entry or retained history. There is one cancel-safe inline sleep/accept future in the existing signal-first supervisor select; handlers continue running and their JoinSet entries are reaped during backoff. No retry queue or spawned retry task exists.

- Transient: EINTR, EAGAIN (including its EWOULDBLOCK alias), ECONNABORTED, EPROTO and ECONNRESET.
- Resource pressure: EMFILE, ENFILE, ENOBUFS and ENOMEM.
- Fatal invalid-listener state: EBADF, EINVAL, ENOTSOCK and EOPNOTSUPP. Its original io::Error is returned after accounted shutdown; its numeric OS cause is retained in the final report.
- Other errors are counted and retried conservatively.

Each nonfatal failure schedules 5, 10, 20, 40, 80, then 100 ms; all subsequent delays stay at 100 ms. There is no failure-count termination threshold. Saturating u64 counters record category totals, scheduled retries, current/high-water consecutive errors and maximum backoff. `accept_retries` counts scheduled retries, including a pending retry canceled by a signal. Signals and reaping do not restart the absolute retry deadline. Test injection and observation exist only under cfg(test), with a one-value watch channel; there is no production SQL or test-control environment hook.

## Report producer/consumer interface

Set `ARC_RUNTIME_REPORT` to a **new file** in an existing, owned 0700 directory. The daemon reuses `server::open_private_directory`, then `openat(O_CREAT|O_EXCL|O_NOFOLLOW|O_CLOEXEC|O_NONBLOCK)` relative to its validated descriptor, sets/verifies mode 0600 and verifies regular-file type, UID and single link. It never overwrites an existing entry or follows a link. The descriptor stays open across readiness/service/shutdown and is never reopened by pathname. An absent setting creates no report and emits no replacement runtime stdout record. An empty setting is invalid. Readiness text is unchanged.

The requested report is opened **before readiness**. Creation/validation failure refuses startup, leaves existing contents and links unchanged, exposes no listener/readiness, explicitly closes/joins the worker and returns failure. A newly created empty report may remain after a later startup failure; it is not a successful final receipt. Unknown IPC profiles still fail through the existing profile parser.

At shutdown the daemon serializes the complete JSON into a fixed `[u8; 4096]` before writing anything, then writes once through the retained descriptor. There is no truncation, newline requirement, periodic export or stdout fallback. A complete report is <=4,096 bytes. Both serialization overflow and actual OS write failures return errors. An OS write error can leave a partial file; consumers must require parseable complete JSON **and successful observed process exit**, never infer success from existence alone.

Top-level keys:

| Key | Type / meaning |
| --- | --- |
| `event` | `"ipc_shutdown"` |
| `reason` | `"SIGTERM"`, `"SIGINT"` or `"accept_error"` |
| `profiles` | `{ "ipc": "default" | "test" }`, selected environment profile; absent/empty profile selects default. cfg(test) supplied limits can differ; `limits` always describes effective values. |
| `limits` | connections, requests, db_queue, db_executing (=1), max_frame_bytes and header_timeout_ms, frame_timeout_ms, write_timeout_ms, db_response_timeout_ms, db_execution_timeout_ms, db_busy_timeout_ms, shutdown_timeout_ms. |
| `metrics` | The unchanged complete Snapshot JSON described below. |
| `supervisor` | Separate fixed-size accept/retry/supervisor accounting described below. |
| `handlers_remaining` | Remaining retained JoinSet entries; zero for successful shutdown. |
| `handlers_reaped`, `worker_joined`, `connection_closed`, `gauges_zero`, `socket_removed`, `accounted`, `deadline_met` | Boolean accounting/cleanup results. `connection_closed` refers to SQLite; handler/socket lifetimes are covered by reaping/cleanup. |
| `shutdown_elapsed_ms` | Shutdown elapsed time sampled before final serialization/write. The final return separately checks the same absolute deadline after writing. |
| `accept_error_os_code` | Fatal accept errno or null. |

`metrics` preserves every existing current/high_water gauge: connections, requests, request_body_bytes, response_bytes, db_queued, db_executing, db_outstanding, rejection_work, plus tracked_tasks. It includes all counters: accepted, connection_rejected, request_rejected, db_rejected, frames_completed, header_timeouts, frame_timeouts, write_timeouts, db_timeouts, worker_started, worker_joined, handlers_spawned, handlers_reaped. Its `caps` contains connections, requests, db_queue, db_executing=1, supervisor=1. No metric was renamed or dropped.

`supervisor` contains accept_errors, transient_errors, resource_pressure_errors, other_errors, fatal_errors, accept_retries, consecutive_errors, consecutive_errors_high_water, backoff_high_water_ms, shutdown_during_backoff, retry_tasks (=0), and supervisors `{current:1, high_water:1}`. The one supervisor is executing this final export; its completion is established by observed process exit, separately from the zero handler/DB gauges. No Tokio private-task measurement is claimed.

On either signal, the existing S+3,500 ms absolute deadline covers admission stop, SQL interruption, handler abort/drain, actual worker close/join, socket cleanup, report serialization/write and final checks. Report failure cannot bypass resource cleanup: tests inject a real read-only-descriptor EBADF and prove worker join, zero gauges, equal handler spawned/reaped counts, socket removal and actual database reopen/integrity success. Regular-file I/O and native SQLite close can still stall inside the OS; no uninterruptible kernel call is claimed to be forcibly bounded. Deadline and I/O errors are returned honestly, without detached report-writing tasks.

## Acceptance and verification evidence

- [x] Bounded tracked handlers and exactly one worker remain integrated; hello/health stay outside SQL. Accepted dependency files were not edited.
- [x] The new accept regression was RED against prior behavior: injected ECONNABORTED ended service and actual arcctl status returned `DAEMON_UNAVAILABLE`. `red-accept.json/txt` and the red lifecycle/injection sources retain it. Repaired isolated ECONNABORTED and nine consecutive EMFILE cases both recover actual status and doctor, with reset consecutive state, bounded cap snapshots, zero retry tasks and no per-error logging. A paused-time policy unit checks 100,000 errors without a fatal threshold and saturating counters; this is not the later mission-wide 100,000-request subprocess proof.
- [x] Shutdown during backoff is synchronized by cfg(test) watch observation after at least six errors, with an actual hello completed and handler reaped through the real supervisor. SIGINT returns a report with shutdown_during_backoff=true, no surviving handlers/worker and successful exit. Fatal EBADF retains its cause after full accounting.
- [x] Disabled export, valid permissions/full schema, existing-file refusal, symlink/unsafe/linked-parent refusal, retained-descriptor path replacement, exact 4,096-byte success / 4,097-byte refusal, expired deadline and actual EBADF write failure pass. Owned subprocess startup-refusal cases also prove worker join and reopen integrity. Maximum numeric widths in the delivered schema fit the cap.
- [x] Both existing stalled-client/held-worker signal cases retain all substantive assertions: real CLI DB_BUSY -> empty stdout, exactly `IPC_ERROR\n`, nonzero exit; database-independent health; partial header/body; instance lock held after joins; zero final gauges; eight handlers spawned/reaped; exactly one worker started/joined; socket removal; reopen integrity=ok. Final focused signal-to-observed-exit: SIGINT 8,013 us, SIGTERM 7,568 us, each strictly below five seconds. These are native macOS fixtures, not RSS/P95 benchmarks or Windows qualification.
- [x] Exact serial required Cargo checks pass; unchanged harness run retains all three failures. Full workspace suite has zero failed/ignored/filtered tests. All ten original server tests, 11 resident, 18 transport-target, 16 worker-target and 13 IPC cases remain active.

All new receipts/logs are under `repair-2026-11-07-attempt-3/`. Commands ran offline from the mission worktree with Rust/Cargo 1.95.0 aarch64-apple-darwin, one Cargo process at a time. No download/install, standalone build, browser/GUI check, agent dispatch, commit/reset/ref edit, mission-state mutation or deployed-service operation occurred. Saved policy/deadline and host limits are in preflight.json; the existing execution began 2026-11-07T10:52:35.601Z with a 2026-11-08T10:52:35.601Z deadline, maxParallel=3/maxRuns=200/maxRetries=3 preserved. The supplied route already dispatched this Actor and supported noninteractive shell commands succeeded; no additional routing probe or policy alteration was needed. No OMA claim path/run ID was injected; none was invented.

| Command | Observed result |
| --- | --- |
| `cargo test -p arc-daemon --bin arc-daemon isolated_transient_accept_error_recovers_cli -- --nocapture --test-threads=1` | Expected RED exit 101 against prior fatal behavior. |
| `cargo test -p arc-daemon --bin arc-daemon -- --nocapture --test-threads=1` | First repair exit 101 because two new tests incorrectly expected four completed frames; real CLI sends two. Corrected that new expectation, retained failure. Final focused run exit 0, 52 passed. |
| `rustfmt --edition 2021 --config skip_children=true` with only owned paths | Exit 0; formatting receipts retained. |
| `cargo fmt --all -- --check` | Exit 0. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0. |
| `cargo test --workspace` | Exit 0; all suites pass, no ignored/filtered tests. |
| `./scripts/harness/validate.sh` | Exit 1: resident.rs 901 lines; arc-ipc/src/lib.rs 551 lines; missing test:coverage. No harness/threshold/tooling changes. |
| `git diff --check` | Exit 0; also reports `av.yaml: Operation not permitted`, retained without bypass. |

Final focused/fmt/Clippy/workspace-test/harness/diff receipts bind the same 43-file product fingerprint `49ab9bbd71a6545018377a840f901e11e8bef69fd5fbf9d6c48389dd18e307a8`. Receipts retain argv/cwd, UTC, process PID and observed exit, timeout state, source-before/source-after and complete-log SHA-256. New final receipts also embed toolchain/environment; the first two commands' minimal original receipts remain unchanged, with enclosing execution-context.json documenting the unchanged offline toolchain/environment. This context was sampled later, not falsely represented as a second contemporaneous version probe. Initial evidence extraction included non-test helper names and returned a false original-server-test flag; its initial summary and explicit correction are retained. Correct extraction confirms all ten original server tests passed; no product test changed for that comparison.

## Chief handoff and remaining work

Use this repaired file producer contract for downstream fixtures after independent task review. The new verifier checks current source fingerprint, all indexed artifact hashes, full log hashes/receipts, migration bytes, ownership boundaries and unchanged dependency inputs. Prior receipts remain history; they are not promoted to current evidence. Task-5/task-6 must still deliver mission-wide 100,000-attempt subprocess/CLI accounting, all saturation/deadline/recovery proofs and native RSS/thread/task/small-message P95 measurements. `docs/bounded-ipc-targets.md` supplies the previously located ARC SRS targets; this repair makes no performance/SRS-comparison claim. Structural caps and the retry-policy unit are not native measurements.

Independent task review and final oma-qa verification, mission QA/performance artifacts, final operator verification and tasks 5–9 remain open. Keep the harness failures unresolved in final evidence. Remaining Phase 0 features (tray, exec, managed jobs, vendor/classification/Sweeper and broader platform integration), native Linux qualification and Windows Named Pipe/runtime/tray/measurement obligations are not completed. av.yaml remains inaccessible, so no real configured container/service health, availability or recent-error proof was invented. No out-of-scope owner change was required by this repair.
