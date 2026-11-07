# Task-2 transport evidence repair

Status: repair artifacts prepared; independent review and normal collection verification are pending. All three required Cargo commands passed in the repair checkout, including all 18 existing transport tests. No product input changed during this repair. This report does not claim production integration or mission acceptance.

Actor: backend / IPC Engineer; task-2, attempt 2; mission `b137e253-def6-46ef-a75d-25ab414784ea`; Codex session `01a1b5e0-6fe3-7bc2-976c-efdfb968d9b1`. Skill used: oma-backend and its execution protocol/checklist, shared execution policy, result contract and code-intelligence contract. No OMA run ID or claim path was injected. None was invented; no mission state, assignment, evidence ref or commit was changed. No agents were dispatched.

## Changed paths and preservation

Every new file from this repair is under `.arc-verify/bounded-ipc/ipc/`: this report, `evidence.json`, the byte-preserved `original-delivery.md`, `record.py`, command logs/receipts, execution context, provenance and review-checkout observations. `evidence.json` indexes all files and their hashes. Existing receipts and the original Actor-local report were preserved.

The nine source files are unchanged from task-2's original delivery: `crates/arc-daemon/src/{admission,connection,limits,platform,runtime_metrics,transport,transport_test_support,transport_tests}.rs` and `crates/arc-daemon/tests/transport_unit.rs`. All nine SHA-256 values match the original report and the current review checkout, as recorded in `provenance-before.json` and `review-source-comparison.json`. No manifest, lockfile, worker, server, main, IPC, CLI or migration edit was made. Product fingerprints over crates, manifests, lockfile and toolchain are identical before and after each gate: `eabc0c7347d1d52d428f45512191d5931df608092b409836c873c94561d4adea`.

`original-delivery.md` is an exact copy of the attempt-1 report from the prior Actor's ignored `.agents/results/` location. It retains the original implementation descriptions, failed attempts, hashes, handoff and limitations; its historical gate claims are not used as fresh command receipts.

## Provenance scopes

| Scope actually inspected | Commit | Product tree |
| --- | --- | --- |
| Accepted committed baseline; also review-checkout HEAD at observation | `42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0` | `c802f71ac4d91ed6b8a602d5c83bf9cf57116b76` |
| Attempt-1 Actor snapshot and worker refs, resolved in review checkout | `3816bafaa684c8099dad78be2d0cb0e474c5f08b` | `e79ff4ed85c94059275bbb4992579a2d55f51eb3` |
| Attempt-2 repair snapshot and local HEAD | `eee0c01750bcf852d691395774215b18628e89d7` | `90ce687dd5407eff881a03c666ebe89d79ca66da` |

The accepted baseline is an ancestor of both snapshot commits, verified with `git merge-base --is-ancestor`. The attempt-1 snapshot contains task-1's input overlay; its tree does not contain the eight new task-2 files and still has the original platform blob. Those nine delivered contents were uncommitted task-2 changes in attempt 1. They are tracked input in the attempt-2 isolated snapshot, alongside the delivered database sources, but remain uncommitted/untracked overlay in the review checkout at accepted HEAD. Tracking in the repair snapshot does not mean tracking in the review checkout or inclusion in the accepted baseline.

`ref-scopes.json` records read-only resolutions in both the original Actor and review repositories. The `refs/agent-valley/task-evidence/d6c6b45f4a6c3769-78686d2704d4d690-1/{snapshot,worker}` refs both resolve to `3816baf...` in the review repository. The attempt-2 `/snapshot` resolves there to `eee0c017...`. Those refs are absent in the isolated Actor clones; those failed resolutions are retained in `provenance-before.json`, not silently replaced with local HEAD. Attempt-2 `/worker` is absent at observation before framework collection; no revision is invented for it.

Migration verification compared current bytes directly with `git show 42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0:crates/arc-storage/migrations/0001_phase0.sql`. Both SHA-256 values are `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`; byte comparison is equal.

## Fresh checks

Commands below ran serially in the attempt-2 repair checkout, with their exact argv, UTC start/end, actual exit status, PID, full combined stdout/stderr, toolchain/context reference and pre/post source fingerprints retained in the corresponding JSON/TXT pairs. `CARGO_NET_OFFLINE=true` prevented downloads; cached Rust/Cargo 1.95.0 on aarch64-apple-darwin was used. `execution-context.json` records cache paths, environment, host limits and saved mission limits. Each Cargo child was observed through process exit.

| Exact command | UTC interval on 2026-11-07 | Result | Receipt |
| --- | --- | --- | --- |
| `cargo fmt --all -- --check` | 12:00:35.248882–12:00:35.504969 | exit 0 | `fmt.json`, `fmt.txt` |
| `cargo clippy --workspace --all-targets -- -D warnings` | 12:00:37.853111–12:00:50.753726 | exit 0 | `clippy.json`, `clippy.txt` |
| `cargo test --workspace` | 12:01:08.090694–12:01:35.555123 | exit 0 | `workspace-test.json`, `workspace-test.txt` |

The workspace run executes `transport_unit`: 18 passed, zero failed/ignored/filtered. Existing CLI binary, resident, startup, core, IPC and storage checks also pass; the complete output is retained. No database Actor receipt was reused. No standalone build, browser suite, dependency install/download, GUI or screen check ran. Test compilation is part of the explicitly required Cargo commands.

## Acceptance evidence and limits

- [x] Cap accounting/admission: `retained_tasks_reap_and_rejections_do_not_call_factory`, `request_admission_races_and_unwind_never_exceed_cap`, `cancellation_reaps_unpolled_handlers_and_drops_all_guards`, metrics/limits units. Tests cover completed-but-unreaped entries, nonwaiting rejection without a factory/task, bounded gauges and final release.
- [x] Admission before body allocation and bounded errors: `saturated_requests_close_before_allocating_even_for_invalid_prefixes`, `zero_and_oversized_frames_reject_without_body_allocation`, `response_serialization_is_capped_before_any_prefix_is_written`, `oversized_echo_uses_one_small_fallback_then_closes`.
- [x] Absolute deadlines: `headers_and_trickled_bodies_have_absolute_deadlines`, `header_progress_does_not_renew_the_body_budget`, `stalled_header_and_partial_body_release_request_capacity`, `nonreading_peer_expires_under_write_backpressure`, `blocked_handler_write_releases_request_and_response_reservations`. These use paused time/synchronized socket backpressure as described in the preserved implementation report.
- [x] Protocol and authentication: `malformed_json_and_version_errors_continue_without_database`, `typed_worker_outcomes_keep_busy_distinct_from_integrity`, `same_user_authentication_supports_async_sockets_and_rejects_non_sockets`, plus existing IPC compatibility assertions. The native same-UID branches remain unchanged apart from adapting the descriptor signature. This does not claim a newly executed foreign-UID subprocess case.
- [x] Material implementation evidence is now in a non-ignored collection path, with fresh checks and original source hashes.
- [ ] Post-collection artifact verification and independent task review remain framework/Chief steps. `git check-ignore` returns 1 with no matched paths for `delivery.md` and `evidence.json`; final scoped status lists the repair artifacts as non-ignored untracked files. Do not copy evidence manually into another checkout or alter refs to simulate collection. After normal collection, verify the file hashes in `evidence.json` in the review checkout. This Actor cannot observe a collection that occurs after its return.

`transport_unit.rs` currently has a **test-target** `#![allow(dead_code)]` and imports the new source modules by path. `main.rs` still declares only instance/platform/server; the production server still uses its preceding unbounded path. These passing gates prove the standalone module/test scope only. They do not lint the new modules under production visibility/dead-code rules, or prove combined-worker compatibility, bounded resident behavior, shutdown, native performance or mission acceptance. Product source has no blanket dead-code suppression. Task-4 must register and use the modules, then repeat all exact Cargo gates without adding blanket product suppression.

## Task-4 owned handoff

Backend owns task-4 production declarations for admission, connection, limits, runtime_metrics, transport and db_worker; the supervisor/lifecycle integration; worker error adaptation; and the public `ERROR_DB_BUSY` addition to `crates/arc-ipc/src/lib.rs`. Replace the temporary identical connection-local constant with its import. Keep synchronous protocol/CLI APIs unchanged. Require an actual integrated CLI test proving `DB_BUSY` prints `IPC_ERROR` and exits nonzero, together with existing successful output and compatibility assertions. These edits were not performed during evidence repair.

Construct shared `RuntimeMetrics::new(&limits)`, Requests and Handlers once. The supervisor owns one continuously reaped JoinSet and selects shutdown before accept; `try_spawn` authenticates/adopts only admitted sockets and reaps ready entries. Set the production Tokio worker count to two. Preserve instance admission before SQLite open and hold the lock through actual handler/worker shutdown and socket cleanup.

The transport doctor callback accepts a fixed real `std::time::Instant` and returns a future of `DoctorOutcome`. In the task-4 adapter, synchronously call `DbClient::try_doctor(deadline)` and let the future own/receive the ticket; dropping the future drops/cancels it. Hello/health never call this adapter. Map busy/stopping/cancelled/deadline/lost-worker and appropriate SQLite busy/locked/interrupt errors to Busy; genuine integrity failures to IntegrityFailed. Keep raw SQL/errors outside wire responses and logs. See the preserved original report for exact callback and shutdown handoff details; the worker interface is unchanged.

Start worker shutdown before aborting/draining handlers. Drain every JoinSet entry under the shared deadline, then confirm the real SQLite close/native thread join, write the bounded secure optional final report and remove the owned socket. Retain unfinished handles/errors as failures. No detached watcher, per-request task, permit-waiting task or reject task is needed.

## Separate database prerequisite

Task-3 remains unresolved here and is not passed by these receipts. `crates/arc-daemon/src/db_worker/tests.rs::metrics()` still calls nonexistent `RuntimeMetrics::default()` in this snapshot. Its isolated test driver used a divergent metrics double according to the retained independent findings. Database owns repairing that helper with `RuntimeMetrics::new` and limits matching its test configuration, replacing the driver double with the real limits/metrics modules, preserving explicit high-water bounds and final-zero/start/join assertions, refreshing hashes and rerunning affected checks. Its handoff must also be published at a non-ignored integrated evidence path. No database-owned file was edited. Task-4 waits for independent acceptance of both repaired deliverables; current Cargo success cannot establish unregistered worker-test compatibility.

## Execution failures and independent-review prerequisite

Serena tools were absent from the enabled inventory. Inspection used scoped direct reads of supplied known paths and read-only Git metadata, not replacement native code searches. A lightweight external memory query supplied no relevant ARC evidence.

Retained inspection failures: the first Python heredoc failed before execution because zsh could not create its temporary file (Operation not permitted); `.cargo/config.toml` was absent; full Git status/diff emitted `av.yaml: Operation not permitted`; mise emitted tracking-symlink permission warnings. Supported Python `-c`/file execution and apply_patch worked. These did not fail the Cargo gates; no privilege, GUI or shell-restriction workaround was attempted. Missing local evidence-ref resolutions and the still-absent attempt-2 worker ref are retained as failures with their actual scopes.

`review-environment-probe.json` records one `/bin/pwd` probe in the review checkout, with `tty=false`, timeout 10 seconds and exit 0 through this Codex noninteractive tool route. This is **not** evidence that qwen's `run_shell_command` runner recovered from grantpt denial. Before independent review, the Chief/reviewer must run one bounded supported noninteractive probe through the actual reviewer route. If grantpt denial persists, retain it as an unmet execution prerequisite and route to an existing configured reviewer with functioning noninteractive tools. Do not repeat the failing command, waive verification or use GUI/ComputerUse. Independent task review and final oma-qa verification remain required.

## Remaining mission obligations

All later tasks retain their acceptance criteria: 100,000 reconciled subprocess/CLI request attempts with cap attainment and bounded rejections; stalled header/body/non-reading-peer release; status/doctor recovery; health during controlled slow database work; separate SIGTERM/SIGINT exits under five seconds with every handler and worker accounted for and reopen/integrity success. Native peak RSS, native thread/task counts and small-message P95 must still be measured against the located ARC SRS targets (idle RSS <50 MB; P95 <=20 ms). Structural bounds and unit-test runtimes are not performance measurements.

The unchanged harness remains a later required run. Retain unresolved conformance failures, including preceding oversized-file/missing-test:coverage findings, rather than changing thresholds or commands. Phase 0 feature work, Linux qualification and Windows Named Pipe/runtime/tray obligations remain open. This repair makes no deployed-service, performance or full mission-completion claim.
