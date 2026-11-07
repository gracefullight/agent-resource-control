# Task-4 classification repair, attempt 7

Status: assigned repair completed; independent technical-director review pending. Overall mission completion remains open. Skills used: oma-backend and oma-db, with the installed backend-engineer definition. The application adapter owns protocol classification; worker execution, SQLite storage and dispatch validation retain their existing responsibilities.

## Changed files and interface policy

Product changes are confined to:

- `crates/arc-daemon/src/lifecycle.rs`: `database_failure` now recognizes only SQLite `DatabaseCorrupt` (SQLITE_CORRUPT) and `NotADatabase` (SQLITE_NOTADB) as positive corruption evidence. All other SQL, rusqlite and worker failures produce the existing Busy outcome. The same guard handles extended corruption codes through their primary ErrorCode.
- `crates/arc-daemon/src/database_failure_tests.rs`: five deterministic regression tests, registered only with cfg(test) in lifecycle.rs.

SQLITE_IOERR (including IOERR_READ), NOMEM, FULL, CANTOPEN, READONLY, PERM, AUTH, BUSY, LOCKED and INTERRUPT produce unavailable. Unexpected SQL errors, including ERROR, INTERNAL and CONSTRAINT, and representative non-SqliteFailure errors also default to unavailable. The existing connection handler emits protocol-1 DB_BUSY; the unchanged CLI maps it to IPC_ERROR. CORRUPT and NOTADB still emit INTEGRITY_FAILED. Failed integrity_check results and invalid migration/schema reports still pass through the existing dispatch validation and produce INTEGRITY_FAILED.

No wire codes, retries, commands, SQL interfaces, production fault controls or CLI mappings were added. The accepted report channel remains ARC_RUNTIME_REPORT with its 4,096-byte limit. Transport, admission, metrics, limits, worker/storage files, manifests, lockfiles, migration, harness, CLI and independent QA artifacts were preserved. `boundary-check.json` compares every incoming product file with the final tree and permits only the two paths above. The incoming tracked and untracked changes remain intact.

## TDD_EVIDENCE

The offline prerequisite was established with `cargo test --offline -p arc-cli --test binary`: exit 0, two tests passed; no standalone build or download was run.

RED, before changing the production adapter:

1. `cargo test --offline -p arc-daemon --bin arc-daemon lifecycle::failure_tests -- --nocapture`: exit 101, three failures and two passes. IOERR was observed as INTEGRITY_FAILED instead of DB_BUSY; non-SQLiteFailure errors were also misclassified. Positive corruption and integrity/schema-report checks passed.
2. `cargo test --offline -p arc-daemon --bin arc-daemon lifecycle::failure_tests::typed_worker_errors_reach_protocol_and_actual_cli_then_recover -- --exact --nocapture`: exit 101. The actual arcctl doctor process printed INTEGRITY_FAILED on stderr instead of IPC_ERROR. This second RED deliberately checks the user-facing assertion before the wire-code assertion; neither expectation was weakened. The preceding adapter and exact second-RED test source are preserved as `lifecycle.rs.red.txt` and `database_failure_tests.rs.red.txt`.

GREEN: `cargo test --offline -p arc-daemon --bin arc-daemon lifecycle:: -- --nocapture`: exit 0, six adapter tests passed, including all five new regressions and the retained contention/cancellation test.

The actual-CLI fixture binds an owned private socket before starting clients. It uses production Handlers for same-user admission and bounded tracking, production handle_connection, and production database_failure with constructed typed DbError::Sql values. The test code supplies IOERR, IOERR_READ, NOMEM, CORRUPT and NOTADB; it never supplies a canned Response or DoctorOutcome::Busy. For each case, it asserts a protocol-1 error and invokes the real arcctl doctor subprocess: nonzero exit, empty stdout, and exactly IPC_ERROR or INTEGRITY_FAILED plus a newline. After clearing the typed fault, actual arcctl status and doctor both succeed with their existing exact text; healthy doctor uses the real single worker and production doctor adapter. The fixture explicitly reaps its bounded handlers, joins the worker, verifies zero gauges, then reopens the database and checks integrity successfully. Child processes have a three-second deadline and owned cleanup; handler/worker shutdown shares a two-second fixture deadline. Fault injection is confined to test compilation.

## Current-tree verification

All commands ran serially in the receiving mission workspace with Rust/Cargo 1.95.0 on aarch64-apple-darwin, existing local caches, CARGO_NET_OFFLINE=true and CARGO_TARGET_DIR set to this workspace's target directory. Each command has a JSON receipt with exact argv, cwd, UTC start/end, PID and observed exit, full output and SHA-256, toolchain/cache context, and source-before/source-after fingerprints. No subprocess timed out. An auxiliary `git diff --check` returned exit 1 because av.yaml access is denied; its exact shell invocation and observed output are preserved in auxiliary-git-failure.json. The chained ignore check did not execute. This permission failure was not bypassed and does not change the recorded Cargo outcomes. The source fingerprint for every final check is `9e3335b98f73d9ef964809c8fbb08fb1530661adb1d6e2c1adac492d28543970` across 51 product files.

| Command | Actual outcome |
| --- | --- |
| `rustfmt --edition 2021 crates/arc-daemon/src/lifecycle.rs crates/arc-daemon/src/database_failure_tests.rs` | Exit 0; owned formatting only, recorded source transition |
| `cargo test --offline -p arc-daemon --bin arc-daemon lifecycle_tests -- --nocapture` | Exit 0; all 13 retained lifecycle tests pass |
| `cargo fmt --all -- --check` | Exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0 |
| `cargo test --workspace` | Exit 0; 183 passing test invocations, zero failures/ignored tests; repeated module registration is counted as invocations rather than distinct tests |
| `./scripts/harness/validate.sh` | Exit 1; exactly the three retained conformance failures below |

The retained lifecycle run recorded SIGINT signal-to-exit at 8,162 microseconds and SIGTERM at 7,270 microseconds. Both cases assert the five-second bound, all handler gauges zero, spawned handlers equal reaped handlers, exactly one worker started/joined, connection closed, owned socket removed, instance lock held through join, and successful reopen/integrity. The same current-tree workspace run retains the saturation, deadline, slow-database health, identity, filesystem and protocol suites. Their passing test invocations do not replace the independent task-8 evidence assessment or required native performance measurements.

The unchanged harness reports resident.rs at 901 lines, arc-ipc/src/lib.rs at 551 lines, and missing test:coverage. These remain unresolved. The script and `0001_phase0.sql` were directly compared byte-for-byte with accepted HEAD `42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0`. Migration SHA-256 remains `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`.

## Evidence and consumer handoff

New receipts, RED sources, full logs, summaries, boundary checks and the read-only collection verifier are under `repair-2026-11-07-attempt-7/`. This directory and delivery.md are non-ignored material deliverables. Earlier delivery.md and evidence.json are preserved byte-for-byte as previous-delivery.md and previous-evidence.json. Other historical reports and receipts were not rewritten as current evidence. No commits, refs, saved task instructions, mission state or assignments were changed. No injected OMA claim path/run ID was supplied; the scoped claim is in the new evidence directory rather than an uncollected ignored supplemental report.

The refreshed integration/evidence.json binds current product sources and material files. Run `python3 -B .arc-verify/bounded-ipc/integration/repair-2026-11-07-attempt-7/verify.py` in the receiving checkout after normal collection; it verifies every indexed artifact, current fingerprints, exact receipts, ownership boundary, and accepted migration/harness bytes without changing files or refs. The present execution works directly in the receiving mission workspace; it does not claim a separate delivery-tree commit or external collection event. Historical standalone verifiers remain attached to their earlier indices and are not current-tree acceptance commands.

Chief Director handoff: obtain the retained independent technical-director review of DbError -> production adapter -> production connection handler -> protocol -> actual CLI, including genuine corruption. Then task-8 must bind its independent oma-qa checks and native RSS/thread/task/P95 measurements to the repaired current inputs, and task-9 must assess that evidence. Older passing booleans and performance receipts are historical until explicitly rebound. No independent-review or overall acceptance boolean was set here.

The missing task-6 report `.agents/results/result-design-director-task-6-attempt-3-local.md` remains with its owner and supported collector. Configured-target applicability remains unverified while av.yaml access is denied; no access restriction was bypassed and owned subprocesses are not deployed-target evidence. Remaining Phase 0/tray/vendor/classification/Sweeper, Linux qualification and Windows runtime obligations remain open. No remaining 100,000-attempt/cap/deadline/recovery, native SRS-performance, platform or final-verification obligation is waived by this focused repair.
