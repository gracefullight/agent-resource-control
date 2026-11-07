# Task-5 backend delivery

Status: completed within assigned ownership. Skill used: oma-backend, including its execution protocol and verification checklist. No subagents were dispatched. No mission state, instructions, acceptance criteria, shared registry or Actor assignment was modified.

Daemon doctor now assesses schema compatibility with task-4's reusable storage API on the existing bounded single SQLite worker. IPC preserves protocol 1, framing, request defaults, legacy envelope/capability fields and actual migration counts. Arcctl reports verified schema versions, identifies unavailable legacy details and retains schema details from error responses. Startup uses the storage error's actionable cause and rejects before IPC readiness. No downgrade execution is implemented or claimed.

## Inputs and interface decisions

Consumed `docs/schema-compatibility-contract.md`, `.arc-verify/schema-compatibility/design/diagnostic-contract.md` and the delivered storage API in `crates/arc-storage/src/{lib,migration,history}.rs`. Existing changes were inspected first: the storage implementation and schema evidence were present; backend product files were unchanged. Storage changes were preserved.

The technical contract binds exact numeric versions, the `schema` object, `SCHEMA_INCOMPATIBLE` and `SCHEMA_DETAILS_UNAVAILABLE`. The design artifact permits binding its proposed names to the shared backend/storage interface. Accordingly, success appends `Schema compatible. Version 1. Migration history verified.` rather than the design's illustrative migration-ID labels. Failure text uses the shared error code plus a static cause and applied/supported versions. This resolves the proposal/interface difference without changing either input artifact.

The daemon exhaustively maps every storage `SchemaStatus` to the independent IPC enum; storage does not import IPC. `DoctorReport.schema` has a default of None and is omitted when serializing None. A present schema requires status, supported version and history verification; unknown status decodes to Unknown and fails validation. Compatible success requires verified history, equal nonzero applied/supported versions and the actual count agreeing with the contiguous version count. Legacy count 1 remains basic diagnostic success, explicitly labeled as lacking schema details; other legacy counts fail as unavailable detail. Malformed present details never fall back to legacy success.

The error envelope retains the diagnostic result for schema failures. Old clients therefore cannot accept an incompatible count-1 schema solely because integrity is ok. New doctor handling consumes this result before generic error mapping. Existing protocol, integrity, busy, unavailable and classification error categories remain covered by tests.

The storage error Display already supplies checksum/missing/invalid/newer causes and recovery guidance to unchanged daemon startup formatting. No redundant startup adapter was added. Rejected startup has empty stdout, nonzero exit and no socket; subsequent doctor cannot reach that daemon. Newer-schema guidance names the advertised/supported versions, asks for a supporting binary and states that downgrade execution is unsupported. History failures ask to preserve the database and investigate against a trusted copy.

## Changed product paths

- `crates/arc-ipc/src/lib.rs`: optional schema field, exported types and shared doctor dispatch.
- `crates/arc-ipc/src/schema.rs`: wire types, validation and diagnostic envelope construction.
- `crates/arc-ipc/src/tests/{mod,dispatch,schema}.rs`: legacy and modern interoperability, invalid details, error precedence.
- `crates/arc-cli/src/main.rs`: doctor routing/help and protocol/request identity checks while retaining schema error results.
- `crates/arc-cli/src/doctor.rs`: validation, success rendering, unavailable detail and static recovery reasons.
- `crates/arc-cli/tests/schema.rs`: actual CLI against protocol-1 fixtures, success/failure/help coverage.
- `crates/arc-daemon/src/db_worker.rs`: typed assessment failure; no admission, thread or shutdown changes.
- `crates/arc-daemon/src/db_worker/execution.rs`: existing worker calls read-only storage assessment with sticky cancellation/deadline checks, preserving SQLite busy/interruption/corruption mapping.
- `crates/arc-daemon/src/db_worker/{tests,schema_tests}.rs`: assessment-specific cancellation, absolute deadline, shutdown, recovery and join evidence.
- `crates/arc-daemon/src/database_failure_tests.rs`: intentional schema-details validation update and current success text; existing typed-failure obligations retained.
- `crates/arc-daemon/tests/resident.rs` and `resident/schema.rs`: actual daemon interoperability, live schema errors, preservation, status and shutdown.
- `crates/arc-daemon/tests/resident/lifecycle.rs` and `tests/support/runtime.rs`: updated exact success text while retaining existing guarantees.
- `crates/arc-daemon/tests/startup.rs`: actionable startup checks plus unchanged-state rejection snapshots.
- `crates/arc-daemon/tests/worker_runtime.rs`: existing controlled native authorizer fixture extended to schema assessment; health/status, recovery, one worker and cleanup checked.
- `docs/bounded-ipc-recovery.md`: current outputs, compatibility matrix, error/recovery guidance and explicit historical-evidence boundary.

A machine-readable exact path list is in `boundary.json`. New backend artifacts: this report, `check.py`, command receipts/full logs, `source-manifest.json`, `boundary.json` and the inserted documentation fragment `documentation-addition.md`.

## Acceptance mapping

| Requirement | Executable evidence in passing focused run |
| --- | --- |
| Explicit doctor compatibility through existing worker | Resident `schema::live_schema_failures_use_bounded_worker_reach_cli_and_preserve_records_and_history` checks actual daemon worker reports, every storage incompatibility used in production, actual counts, history_verified=false, applied/supported values, CLI errors and liveness. `resident_contract_holds_until_sigterm` checks verified success. |
| Old-client/new-daemon production interoperability | IPC `legacy_request_and_decoder_accept_new_production_report_with_truthful_count` uses a literal baseline request, framing and a derived baseline decoder that ignores additive schema fields. Resident `baseline_client_adapter_accepts_actual_new_daemon_production_report` sends the literal original request to the actual daemon and executes original protocol/status/integrity/count checks. |
| New-client/old-daemon fallback | Actual CLI `new_client_old_daemon_labels_unavailable_details_without_claiming_verification` checks absent/null schema, legacy integrity/count success and exact unavailable suffix. IPC `legacy_absence_and_null_decode_but_missing_present_fields_are_invalid` proves decoding/omission/default behavior. |
| Honest old-client fixture-version-2 limitation | IPC `modern_version_two_succeeds_but_legacy_count_check_rejects_without_falsifying_count` proves count 2, modern success and baseline count-only rejection. Actual CLI `actual_cli_reports_verified_production_and_two_version_fixture_success` accepts version 1 and 2 reports with true counts. These protocol fixtures are reports, not a new production migration or proof of executed upgrade; storage owns actual additive-upgrade execution. |
| Unavailable/malformed/unknown details | IPC `present_but_inconsistent_details_never_fall_back_to_legacy_success` plus `legacy_absence_and_null_decode_but_missing_present_fields_are_invalid`; actual CLI `unavailable_and_malformed_present_details_fail_without_legacy_fallback` and `inconsistent_or_unknown_success_envelopes_cannot_claim_schema_compatibility`. No success stdout precedes validation. |
| Failure reasons and safe guidance | CLI `every_schema_failure_survives_error_envelope_with_actionable_reason` exercises all six statuses plus unknown, exact error category, nonzero exit, empty stdout, versions and recovery. `doctor_help_explains_schema_scope_and_prestartup_stderr` checks discoverable plain-text scope and pre-startup advice. |
| Startup checksum/history/newer rejection and downgrade refusal | `checksum_rejection_exits_nonzero_with_recovery_guidance` and `missing_invalid_and_newer_schema_startup_reject_without_data_or_history_changes` assert exact stderr, no readiness/socket, existing application record preservation, complete history tuples, schema objects and user_version unchanged. The newer fixture advertises history/witness version 2 to the production version-1 binary; it is explicit rejection, not downgrade execution. Storage's separate isolated registries prove executed upgrade/lower-registry rejection. |
| Live diagnostics preserve data/history | Resident live-schema fixture snapshots all vendor values, complete migration tuples, sqlite_schema objects and witness before and after doctor/CLI/shutdown. Four cases cover altered checksum, deleted history, changed known ID and advertised newer history/witness. External fixture mutation occurs before the snapshot; production diagnostics perform no repair. |
| Protocol mismatch before DB and integrity separate from schema | IPC `protocol_mismatch_precedes_doctor_and_integrity_failure_precedes_schema`, existing hello/health dispatch checks, daemon typed-worker failure suites and CLI `integrity_protocol_busy_and_absent_endpoint_keep_existing_error_categories`. |
| Health independence during changed DB work | `health_completes_while_schema_assessment_is_deliberately_stalled` gates preparation of the real user_version read through SQLite's native authorizer, fills the existing DB queue, checks health/status with available transport slots, releases via handshake, checks all real responses and CLI recovery, signals shutdown and verifies runtime report/reopen. The original integrity-preparation test still passes. |
| Cancellation/deadline/shutdown of assessment | Worker `schema_assessment_cancellation_deadline_and_shutdown_keep_worker_owned` gates user_version preparation and tests ticket cancellation, the admitted absolute request deadline and stopping. Recovery reports compatible, high-water executing=1, all bounds hold, connection closes and worker joins. Gate callbacks are test-only, bounded and never added to production. |
| Existing worker/admission/identity/shutdown guarantees | Existing 100,000-attempt request/DB/connection cases; real busy callback and queue cancellation/deadline tests; stale/live identity and socket ownership tests; lifecycle SIGTERM/SIGINT cleanup, saturated native signal tests and restart/reopen; stalled frame/nonreading peer recovery; zero-gauge/reap/join accounting. All remain enabled and passed. |

Acceptance checklist: all five task-5 criteria are satisfied by the focused executable checks above. No criterion was weakened, skipped or replaced with a synthetic success. No standalone build, browser/GUI check, dependency installation or external delivery occurred.

## Commands and actual outcomes

Commands ran from the assigned worktree on Darwin. `check.py` enforces a 120-second subprocess wait with bounded termination/reaping on expiry. Commands used the existing cached toolchain/dependencies and default worktree-local Cargo target. They ran serially; no additional Actors were started.

| Command | Result | Evidence |
| --- | --- | --- |
| `cargo test --offline -p arc-ipc -p arc-cli -p arc-daemon --no-run` | First attempt exit 101: pre-existing DoctorReport initializers needed the new optional field; corrected. Second attempt exit 0. These preparation outcomes are recorded from tool output, not substituted for execution tests. | Session tool output; no separate full log claimed. |
| `cargo test --offline -p arc-ipc -p arc-cli -p arc-daemon` | First recorded execution exit 101: new test tried to access private Active.deadline. Corrected by using its own admitted request deadline; production visibility was not relaxed. | `focused-test-1.json`, `focused-test-1.txt` |
| `cargo test --offline -p arc-ipc -p arc-cli -p arc-daemon` | Final execution exit 0, 210 passing test invocations across 13 nonempty suites plus two empty suites/doc targets; zero failures/ignored/filtered. Some shared modules run in several targets, so 210 is not a distinct-test count. Elapsed 50.54 seconds. | `focused-test-2.json`, `focused-test-2.txt` |
| `cargo fmt --all -- --check` | Exit 0, final inputs unchanged. Owned changed files were formatted with rustfmt first; no storage formatting edits. | `fmt.json`, `fmt.txt` |
| `cargo clippy --offline -p arc-ipc -p arc-cli -p arc-daemon --all-targets -- -D warnings` | Exit 0, final inputs unchanged. | `clippy.json`, `clippy.txt` |
| Scoped `git diff --check` | Exit 0. | `boundary.json` |
| Original SQL and harness baseline byte comparison; source line limits and ownership | Pass. SQL SHA-256 remains `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. Owned Rust maximum is 474 lines. Manifests/lockfile/harness unchanged. | `boundary.json`, `source-manifest.json` |

Final source binding: all runtime, test, migration and manifest inputs match the passing test receipt exactly. Only the owned recovery Markdown was updated after that test run; final fmt/Clippy receipts and source manifest include those final documentation bytes. `boundary.json` records this difference explicitly. Full logs and JSON receipts include command argv, cwd, process ID, times, exits, output hashes and before/after source hashes. The final manifest binds every Rust/TOML/SQL input under crates plus root manifests, harness and consumed contract/documentation. Reports are supplemental evidence, not proof by their existence alone.

## Limits and Chief handoff

No ownership conflict or required API deviation remains. Serena discovery tools are unavailable; inspection used known paths supplied by the assignment/contracts and compiler diagnostics. Some guessed known-path reads initially returned missing-file errors; actual module paths were resolved from supplied modules without broad native code search. Mise emitted tracking-symlink permission warnings; native checks completed. No denied command was rerouted through GUI or another surface.

Mission-wide `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, the unchanged `./scripts/harness/validate.sh`, final-source evidence and independent oma-qa review remain assigned to Chief/QA. This task did not run the harness or claim that its historical missing `test:coverage` failure has cleared. Preserve the actual result in final mission evidence. Linux/Windows qualification and human usability are not established by these Darwin native fixtures.

Production schema remains version 1, so baseline client interoperability is preserved there. The known count-2 old-client limitation is documented and tested; it is not fixed by lying about counts. Startup migration precedes IPC, so doctor cannot diagnose a rejected startup. No unsupported downgrade execution is claimed.
