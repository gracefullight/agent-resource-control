# Task-7 integration checkpoint, attempt 11

Status: implementation gates completed on native macOS. This is the backend handoff; task-8 independent native workspace QA and task-9 Chief acceptance remain required. Lifecycle management is implemented and tested. Scheduling, desktop, native Windows and commercial outcomes are not accepted by this report.

Skill used: oma-backend. The existing six-crate Rust 2021 workspace, Clap, Tokio, authenticated arc-ipc and bounded rusqlite worker remain the implementation stack. No dependency, protocol shape, storage transition, scheduler or terminal topology was changed in this attempt.

## Resumed scope and changed paths

The requested recent-status correction was already present when attempt 11 resumed. `crates/arc-cli/src/job.rs` prints labelled Updated (Unix ms) and the serialized static Reason for each recent entry. The existing `scenarios::status_discovery_labels_and_missing_id_never_launch` scenario retains empty history, by-ID, missing-ID, static parse/spawn diagnostics and row-count/no-launch assertions, and adds controlled valid distinct registration/update times, typed response order and exact populated CLI output associated with each opaque ID.

No product source was edited in this attempt. The accepted task-6-terminal resume7 handoff was preserved byte-for-byte. Its six source/test paths were staged for index-based fresh-checkout verification: `crates/arc-cli/src/exec/signals.rs`, `crates/arc-cli/src/exec/signals/tests.rs`, `crates/arc-cli/tests/fixtures/child.rs`, `crates/arc-cli/tests/fixtures/pipeline.rs`, `crates/arc-daemon/tests/managed_exec/pipeline.py` and `crates/arc-daemon/tests/managed_exec/pipeline.rs`. Shared manifests/lockfile, migration 0001, accepted contracts, preservation baselines/accounting, reviewer artifacts and Chief outputs were not edited.

Maintained files changed under this implementation directory are `report.md`, `inputs.json`, `audit.json`, the 15 current receipts below and explicitly promoted `positive-execution.json`. `resume-attempt11.json` is the new checkpoint/resource/archive index. Raw execution logs, isolation checkouts and scripts stay in ignored runs/target paths. Earlier maintained implementation bytes were copied intact to `runs/task7-attempt11-original/` before updates; the checkpoint records their SHA-256 digests. Prior reviewer bytes remain at their original paths.

## TDD_EVIDENCE and superseded scope

Exact regression command: `cargo test -p arc-daemon --test managed_exec scenarios::status_discovery_labels_and_missing_id_never_launch -- --exact --nocapture`.

The retained actual RED in `recent-red.json` has source hash `2866cefe1575df2fd7dd2921d4054ffc99fadd58fa3ec03ccd4793aefa96f0fe`, exit101 and one failing test. Its hash-verified native assertion output shows both records missing Updated and Reason. The expected values pair Registered100 with Updated4000000000350 and Registered200 with Updated4000000000475, with REGISTERED_NOT_CLAIMED and OBSERVED_SPAWN_FAILURE respectively. This is historical pre-fix evidence, not a new failure or a fabricated replay.

Current GREEN in `recent-green.json` executes the same exact command against `a9f6451fc3242aab2d555e56d8ee59caa293cf42481315e23a4c4b4485ce7137`: one passed, zero failed/ignored. The fresh full workspace execution independently includes the same scenario. Earlier integration-completeness claims did not cover the confirmed omission; earlier recent-status and terminal reviews remain valid only for their recorded source/gate scope. The new separately owned `integration-resume11-independent-review.md/.json` supplies the renewed task-7 reproduction and final-source evidence verdict. This backend report grants no independent verdict.

## Native command receipts

Managed source SHA-256: `a9f6451fc3242aab2d555e56d8ee59caa293cf42481315e23a4c4b4485ce7137`. `inputs.json` binds all 177 actual file hashes/modes, current Git source diff and native host. Host: Darwin 27.2.0, arm64, Eunkwangui-MacBookPro.local. Each receipt records exact argv/cwd, UTC times, native PID, source hash, actual exit, 3600000-ms allowance and stdout/stderr hashes pointing to retained ignored raw files. All 12 mandatory command identities are present; no result was restamped or promoted from a stale source.

| Exact command | Exit | Receipt | UTC start / end |
| --- | --- | --- | --- |
| `cargo fmt --all -- --check` | 0 | `fmt.json` | 2026-11-08T19:07:44.217Z / 2026-11-08T19:07:44.776Z |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | `clippy.json` | 2026-11-08T19:07:52.057Z / 2026-11-08T19:07:52.446Z |
| `cargo test --workspace` | 0 | `workspace.json` | 2026-11-08T19:07:57.955Z / 2026-11-08T19:08:41.949Z |
| `bun test scripts/harness/managed-exec.test.ts` | 0 | `preservation.json` | 2026-11-08T19:08:31.646Z / 2026-11-08T19:09:38.011Z |
| `bun test scripts/harness/equivalence.test.ts` | 0 | `equivalence.json` | 2026-11-08T19:08:58.122Z / 2026-11-08T19:10:52.862Z |
| `bun test scripts/harness/coverage-verification.test.ts` | 0 | `coverage-verification.json` | 2026-11-08T19:10:08.622Z / 2026-11-08T19:10:25.087Z |
| `bun test scripts/coverage/clean-checkout.test.ts` | 0 | `clean-checkout-final.json` | 2026-11-08T19:11:02.067Z / 2026-11-08T19:12:42.429Z |
| `bun test scripts/coverage/coverage-cli.test.ts` | 0 | `coverage-cli.json` | 2026-11-08T19:13:26.033Z / 2026-11-08T19:15:01.962Z |
| `bun run test:coverage` | 0 | `coverage-root.json` | 2026-11-08T19:13:26.319Z / 2026-11-08T19:15:01.949Z |
| `bun test scripts/coverage/harness.test.ts` | 0 | `harness.json` | 2026-11-08T19:15:36.574Z / 2026-11-08T19:17:19.417Z |
| `./scripts/harness/validate.sh` | 0 | `harness-root.json` | 2026-11-08T19:15:38.044Z / 2026-11-08T19:17:19.400Z |
| `bun test scripts/coverage/evidence.test.ts` | 0 | `evidence.json` | 2026-11-08T19:17:53.154Z / 2026-11-08T19:17:54.299Z |
| `python3 .arc-verify/managed-exec/implementation/privacy.py` | 0 | `privacy.json` | 2026-11-08T19:09:45.805Z / 2026-11-08T19:09:45.934Z |
| `bun test scripts/harness/managed-exec-evidence.test.ts -t real deterministic Cargo execution binds positive identity and actual output` | 0 | `positive-execution-test.json` | 2026-11-08T19:09:37.828Z / 2026-11-08T19:09:38.320Z |
| `cargo test -p arc-daemon --test managed_exec scenarios::status_discovery_labels_and_missing_id_never_launch -- --exact --nocapture` | 0 | `recent-green.json` | 2026-11-08T19:09:19.523Z / 2026-11-08T19:09:20.461Z |

Workspace result: 342 passed, 0 failed, 0 ignored across 32 libtest harnesses. Managed preservation: 12 passed/234 assertions. Equivalence: two passed/10 assertions. The noncollecting inventory executed its five verification suites and retained the four separately required gates. Equivalence and the noncollecting inventory ran in isolated local checkouts whose complete 177 file hashes/modes match the mission checkout; their receipts include actual cwd, source bindings and generated artifact hashes. This keeps frozen root harness-speed evidence unchanged.

Fresh-checkout coverage, root coverage CLI and full harness ran serially. `coverage-root.json` and `harness-root.json` were extracted from the actual successful nested `bun run test:coverage` and `./scripts/harness/validate.sh` executions, including command/binding/outer hashes. The full harness ran without skip flags and reported zero violations. `evidence.json` starts after the last collector ends and verifies current LCOV/LLVM accounting, profile hashes/merge membership and child PID proof. `positive-execution.json` is the exact actual Cargo validator output explicitly promoted from the successful fresh positive-execution test; its emitter role is implementation evidence, never independent QA.

The native privacy check observed exit17, one lifecycle row, zero permits, a pinned nonempty WAL and no synthetic argv/environment sentinel in SQLite rows/files or ARC diagnostics. Both owned native identities were absent after cleanup. Existing fixture cleanup and identity checks remained in the full suite.

## Requirement to test mapping

All names below are actual passing tests parsed from the unfiltered current workspace receipt. Existing status/doctor/classify and bounded-runtime regression suites also ran unfiltered.

- Exactly one invocation through commit/reply loss, disconnect and retries; concurrent duplicate start/complete:

  - `retries::cli_has_one_child_under_before_after_commit_reply_loss_and_retains_exit17`
  - `protocol::retries_response_loss_concurrent_claim_and_completion_are_idempotent`

- Native argv bytes, cwd/environment/streams and exit17; executable with spaces and literal closed descriptors:

  - `cli::exact_native_argv_environment_cwd_streams_and_exit17_match_direct_execution`
  - `scenarios::executable_with_spaces_closed_descriptors_and_redirected_streams_match_direct`
  - `scenarios::native_non_rust_child_observes_literal_closed_standard_descriptors`

- Foreground pipeline, terminal signals, stop/resume, inherited signal dispositions and session-leader execution:

  - `pipeline::native_foreground_pipeline_input_signals_stop_resume_and_exit17`
  - `pipeline::native_inherited_sigcont_target_stop_wrapper_continue_and_completion`
  - `pipeline::native_nonterminal_inherited_sigcont_target_stop_wrapper_continue_and_completion`
  - `pipeline::native_inherited_ignored_sigchld_preserves_target_state_and_completion`
  - `pipeline::native_session_leader_without_terminal_preserves_execution_and_exit17`

- PTY input/modes, foreground restoration, background resume and controlling-terminal session leader:

  - `terminal::native_background_terminal_read_can_be_foregrounded_and_resumed`
  - `terminal::native_pty_foreground_input_modes_and_terminal_signals_match_direct_child`
  - `terminal::native_pty_spawn_failure_restores_foreground_without_child_output`
  - `terminal::native_session_leader_with_controlling_terminal_keeps_foreground_input`
  - `exec::signals::tests::private_sigcont_preserves_target_disposition_mask_and_restores_guard_on_every_exit`

- Observed signal/core flag, one target, spawn failure and fast exit:

  - `cli::wrapper_directed_signal_preserves_native_signal_status_and_no_second_child`
  - `cli::spawn_failure_quick_exit_help_and_status_have_static_diagnostics`
  - `protocol::unreaped_quick_exit_keeps_identity_but_is_not_reported_running`

- Daemon SIGTERM/SIGKILL survival and identity-verified restart reconciliation without replay or invented exit:

  - `cli::daemon_shutdown_and_crash_preserve_child_and_restart_reconciles_without_replay`
  - `protocol::identity_mismatch_missing_and_restart_never_invent_an_exit`
  - `jobs::tests::recovery::reconciliation_never_replays_or_invents_exit_and_terminal_outcome_wins_races`

- Remote/UNKNOWN pass-through without permits/reservations or scheduler:

  - `retries::path_lookup_remote_and_unknown_commands_execute_without_resource_reservations`

- Secret-free persistence/diagnostics, protocol version-first validation, recent bound/truncation:

  - `protocol::version_first_typed_payload_validation_recent_bound_and_secret_absence`

- Populated recent status labels, distinct timestamps, ID/order/reasons/last-known and prior no-launch diagnostics:

  - `scenarios::status_discovery_labels_and_missing_id_never_launch`

- Finite shared worker, cancellation before commit, bounded admission/shutdown and native observation timestamps:

  - `db_worker::jobs::tests::lifecycle_cancellation_before_commit_releases_shared_capacity_and_never_launches`
  - `db_worker::jobs::tests::worker_records_native_observation_time_instead_of_queue_execution_time`
  - `db_worker::tests::finite_queue_rejects_100000_attempts_and_recovers_after_cancellation`
  - `db_worker::tests::shutdown_timeout_retains_handle_until_native_thread_really_exits`

- Additive migration and persistent transition/idempotence semantics:

  - `jobs::tests::migration::upgrade_from_original_registry_preserves_rows_checksums_and_reopen`
  - `jobs::tests::transitions::register_claim_attach_complete_and_identical_retries_preserve_revision_and_times`
  - `jobs::tests::recovery::concurrent_stable_key_registration_claim_and_completion_have_one_winner`

## Coverage, preservation and evidence limits

| Existing crate | Covered/total production lines | Coverage |
| --- | --- | --- |
| arc-core | 247/256 | 96.48% |
| arc-detector | 609/610 | 99.84% |
| arc-storage | 653/674 | 96.88% |
| arc-ipc | 387/393 | 98.47% |
| arc-daemon | 1628/1746 | 93.24% |
| arc-cli | 674/774 | 87.08% |

`audit.json` binds the final native summary path and digest. Every existing crate meets its 80% floor; no crate was added. Denominators, exclusions, profile/PID accounting and harness protections were preserved. Migration `crates/arc-storage/migrations/0001_phase0.sql` remains byte-identical to accepted HEAD19d7a507044cbf83918f58cd41056905c275c8d4, SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. Historical preservation remains scoped to its original contract, with current migration/regression/accounting/harness rejection checks passing. Maximum relevant source length is 474 lines in `crates/arc-daemon/src/transport_tests.rs`, below500. The retained managed-evidence union at audit is 141 files; this includes prior failures/reviews and stays below150 without deleting evidence.

## Resources, failed attempts and remaining work

Framework limits remain maxDuration86400 seconds, maxRuns200 and maxParallel3; one backend coordinator owns serial collectors. No authoritative cancellation API, absolute deadline, current cumulative remainder or OMA claim identity was injected. No cancellation was received. The prior Chief upper-bound end2026-11-09T08:59:36Z remains an upper bound, not a reset. Initial free disk was18.43GB; resource checks before collection are recorded in `resume-attempt11.json`. Each mandatory command was allowed at least3600000ms.

`ps` inventory was denied EPERM; this was recorded and no GUI workaround or process kill followed. Native libproc inspection found no process paths for the completed commands in the latest incomplete historical collector; the specifically abandoned operator-note collector was not reopened. A native full-context reviewer dispatch failed because the tool had no rollout for this thread; an isolated-context dispatch of the same independent reviewer role succeeded. Supplemental reads of nonexistent convenience paths were unsuccessful and did not run or weaken gates. All actual verification commands in this attempt passed. No toolchain installation/download, browser/GUI/Playwright, separate build, AV operation/state change, push/merge/publication, outreach or spending occurred.

No ownership/interface conflict required product changes. The typed lifecycle, direct OsString argv invocation, stable retry key, existing IPC/worker and persistent storage interfaces remain compatible with accepted tasks1–6 and protocol1. The recorded native tests establish lifecycle behavior; they do not establish scheduling capability.

Chief-owned `final-summary.json`, `final.md` and `receipt-index.json` already exist as prior-source historical artifacts. They were not refreshed or used as current acceptance here. Task-8 final independent QA and task-9 source/evidence rebinding remain required, along with desktop, native Windows and Phase1 scheduling work outside this slice.
