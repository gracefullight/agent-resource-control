# Schema compatibility final decision

Task-8 delivery is complete. Product requirements and the three required Cargo gates passed on the reviewed sources. Independent oma-qa found no implementation blocker. Full validation is incomplete: the unchanged harness exited **1** because `test:coverage` is missing. Coverage did not execute. This delivery retains that failure and does not claim a clean harness or completed coverage qualification.

## Scope and decision

The original supplied goalBrief, verification contract, acceptance criteria, mission graph and Actor assignments remain unchanged. This task consumed the frozen assignment and preceding deliverables; it changed only `final.md` and `final.json`. No product repair, shared-manifest handoff or new mission was needed. HEAD remains the accepted base `bf28a35b594f5ae83ac97c216bd3b09e60a2dfa1`; implementation is integrated as working-tree changes and new files, rather than a claimed implementation commit.

Choose automatic atomic startup migration with a compiled ordered registry and explicit doctor compatibility. This preserves the existing start/inspect journey. The [technical contract](../../docs/schema-compatibility-contract.md) and [Technical Director report](technical/report.md) support reusing Rust, rusqlite, sha2, Tokio, Serde and Clap. No new dependency, runtime migration discovery, ORM, generator, second SQLite worker or production migration 0002 was needed. A separate migration command would add an operator step; an interactive wizard would add prompts and scripting complexity. Neither was required by this mission.

The [Design Director contract](design/diagnostic-contract.md) informs plain terminal status, explicit rejection reasons and recovery guidance. Successful doctor reports versions and verified history. Legacy responses identify unavailable compatibility detail. Rejected startup reports its cause on daemon stderr before IPC readiness; doctor cannot query that failed daemon. Health remains a lightweight, database-independent operation subject to transport admission. No interaction study was performed.

The [Marketing Director assessment](marketing/value-scope.md) confines value to preserving records in tested upgrades and making incompatibility diagnosable. Support savings, task-time improvements, retention, willingness to pay and revenue remain hypotheses or unknowns. Paid assessments, promotion, channels and outreach are deferred; they add no acceptance requirement here.

## Delivered implementation and interface compatibility

- [Storage handoff](storage/report.md): [migration.rs](../../crates/arc-storage/src/migration.rs) compiles the production registry; [history.rs](../../crates/arc-storage/src/history.rs) validates ordered IDs, checksums and version witness. `BEGIN IMMEDIATE` precedes authoritative history validation. Pending SQL, history inserts and `user_version` commit together; failure rolls the batch back. Existing public open signatures remain available. Alternate registries and synchronization hooks are private test inputs.
- [Backend handoff](backend/report.md): compatibility assessment stays on the existing bounded single SQLite worker. [IPC schema](../../crates/arc-ipc/src/schema.rs) and [CLI doctor](../../crates/arc-cli/src/doctor.rs) share explicit modern validation. Optional schema details preserve protocol-1 decoding and truthful legacy fields. Error envelopes retain diagnostic details; malformed modern reports cannot inherit legacy success. [Recovery documentation](../../docs/bounded-ipc-recovery.md) describes current behavior.
- Production remains schema **1**, with `0001_phase0.sql` preserved byte-for-byte. Two-version evolution is exercised through isolated compiled storage fixtures using the same engine. CLI version-2 wire fixtures separately verify rendering and validation.
- Baseline request/decoder/count-validation adapters accept the actual new production daemon. New CLI tests consume legacy response fixtures and label absent details without claiming checksum verification. Historical binaries were not separately compiled. Old count-based clients still reject count 2; protocol 1 does not remove that limitation, and counts were not falsified.
- A lower registry refuses an actually upgraded fixture without changing state. **Downgrade rejection is verified; downgrade execution is unsupported.**

No ownership or interface conflict remains in the supplied implementation and independent assessment. Complete changed-path inventories are in the storage handoff, backend handoff and `backend/boundary.json`; this task preserves all of them.

## Source identity and immutable requirements

Final source fingerprint: `3ff8691bc3d81511f30ec68bca9a62e7bd74dabd13a3fd686a955d189d302ca5`. This is SHA-256 of the exact [source-manifest.json](verification/source-manifest.json) bytes, matching both the [verification summary](verification/summary.json) and [independent assessment](review/assessment.json). It binds 92 relevant source/configuration inputs, including every crate Rust/SQL/TOML input, root Cargo manifest/lock/toolchain, harness and consumed interface/recovery documentation. It is a scoped fingerprint, not a whole-worktree hash.

A read-only Python audit in task-8 recomputed all 92 entries, checked complete crate-input coverage, all 28 [verification artifact hashes](verification/artifact-manifest.json), independent report/audit hashes, command argv/cwd/exits, before/after source manifests, log hashes, 68 named-test references across 26 groups and immutable Git-object bytes. All assertions passed, exit 0. No review or verification artifact was rewritten. Cargo suites were not repeated because sources and evidence match and no concrete gap was found. The final JSON records the audit and consumed-artifact hashes.

| Immutable obligation | Evidence and result |
| --- | --- |
| Ordered compiled production inputs; unchanged original SQL | Registry inspection and independent review pass. `0001_phase0.sql`: 3,917 bytes, SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`, equal to accepted-base bytes. |
| Protocol 1; source files within 500 lines | Protocol constant and compatibility tests pass. All 248 audited source-length inputs match; maximum 474 lines. Untracked Rust additions are included. |
| Deterministic migration, preservation, compatibility and runtime matrix | All 26 requirement groups pass; full named-test mapping and limits are retained below and in the acceptance matrix. |
| Exact Cargo gates | All three exit 0 on unchanged integrated sources. |
| Unchanged harness execution and actual-result retention | Fulfilled: exact command ran, exit 1 retained. Harness SHA-256 `93140173df8b8bd28b067aff68301134b0398b643240b0b1e6a13da4487ebe33` matches accepted-base bytes. Coverage qualification remains unresolved. |
| Source-bound evidence, independent oma-qa and final delivery | Verification, review and task-8 outputs are present and source-matched. Product review passes; it does not waive the harness failure. |

The saved acceptance obligations are met, including retaining the required failing harness result. The separate coverage gate has **not** passed. No verification command, test or acceptance criterion was weakened, skipped through a workaround or replaced.

## Named acceptance evidence

The [acceptance matrix](verification/acceptance-matrix.md), [machine-readable cases](verification/acceptance-cases.json) and [test index](verification/test-index.json) map every required group to exact test-log lines. The [independent review](review/report.md) assesses the actual assertions and preservation snapshots.

| Requirement | Representative passing test and source |
| --- | --- |
| Fresh initialization and production reopen | `nfr010_wal_migration_survives_close_and_reopen_without_cleanup`, [nfr010.rs](../../crates/arc-storage/tests/nfr010.rs) |
| Two-version additive upgrade, preservation and repeat-open idempotency | `fixture_initialization_additive_upgrade_reopen_and_idempotency`, [acceptance.rs](../../crates/arc-storage/src/migration/tests/acceptance.rs) |
| Existing legacy history with zero witness | `production_legacy_history_is_read_only_then_stamped_atomically`, same acceptance module |
| Concurrent-upgrader serialization | `concurrent_upgraders_serialize_and_reread_after_lock_acquisition`, [concurrency.rs](../../crates/arc-storage/src/migration/tests/concurrency.rs); channels and SQLite busy notification establish lock ordering |
| SQL-failure rollback, including an earlier migration in the batch | `sql_failure_rolls_back_schema_application_data_history_and_witness` and `later_failure_rolls_back_prior_migration_history_in_same_batch`, acceptance module |
| Interrupted execution rollback/recovery | `process_interruption_before_commit_rolls_back_and_recovers`, concurrency module; flushed pre-commit handshake, reader isolation, child kill/reap, reopen snapshot and valid retry |
| Altered checksum and missing history rejection | `checksum_defects_in_every_known_entry_leave_all_state_unchanged` and `missing_history_table_rows_gaps_and_trailing_entry_are_rejected`, [rejection.rs](../../crates/arc-storage/src/migration/tests/rejection.rs) |
| Unsupported newer schema and downgrade rejection | `unsupported_newer_history_and_witness_and_downgrade_are_rejected`, same rejection module |
| Actual daemon/CLI diagnostics and preserved records/history | `live_schema_failures_use_bounded_worker_reach_cli_and_preserve_records_and_history`, [resident/schema.rs](../../crates/arc-daemon/tests/resident/schema.rs); actual startup rejection is additionally checked in [startup.rs](../../crates/arc-daemon/tests/startup.rs) |
| Older-client compatibility and legacy limitations | `baseline_client_adapter_accepts_actual_new_daemon_production_report`, resident schema module; `modern_version_two_succeeds_but_legacy_count_check_rejects_without_falsifying_count`, [IPC schema tests](../../crates/arc-ipc/src/tests/schema.rs); `new_client_old_daemon_labels_unavailable_details_without_claiming_verification`, [CLI tests](../../crates/arc-cli/tests/schema.rs) |
| Single bounded worker, cancellation and shutdown ownership | `schema_assessment_cancellation_deadline_and_shutdown_keep_worker_owned`, [schema_tests.rs](../../crates/arc-daemon/src/db_worker/schema_tests.rs), plus existing 100,000-attempt admission/queue tests |
| Health, identity and shutdown regression guarantees | `health_completes_while_schema_assessment_is_deliberately_stalled` and saturated SIGINT/SIGTERM cases in [worker_runtime.rs](../../crates/arc-daemon/tests/worker_runtime.rs); distinct-socket identity/restart cases in [identity.rs](../../crates/arc-daemon/tests/resident/identity.rs); join/reap assertions in [lifecycle_tests.rs](../../crates/arc-daemon/src/lifecycle_tests.rs) |

Preservation snapshots include application rows, complete `(id, applied_at, checksum)` history tuples, schema definitions and `user_version`. Rejected, failed and interrupted attempts require exact equality. Successful additive upgrades retain original fields/history while adding the expected new data. No timing sleep substitutes for migration lock/interruption coordination. The default child-helper invocation alone is not counted as interruption proof.

## Commands and actual outcomes

Task-6 executed these commands from the mission worktree on native `aarch64-apple-darwin`, using cached dependencies with `CARGO_NET_OFFLINE=true`. Source-bound receipts and full combined output are linked here. Task-7 independently inspected them; task-8 revalidated their current hashes and results.

| Command | Actual result | Receipt / complete log |
| --- | --- | --- |
| `cargo fetch --offline --target aarch64-apple-darwin` | Exit 0; cached host prerequisites | [receipt](verification/prepare-host.json), [log](verification/prepare-host.txt) |
| `cargo fmt --all -- --check` | Exit 0 | [receipt](verification/fmt.json), [log](verification/fmt.txt) |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0 | [receipt](verification/clippy.json), [log](verification/clippy.txt) |
| `cargo test --workspace` | Exit 0; 247 passing invocations, zero failed/ignored/filtered, 18 nonempty suites | [receipt](verification/workspace-test.json), [log](verification/workspace-test.txt) |
| `./scripts/harness/validate.sh` | **Exit 1**, not timed out; checks 1–4 passed; coverage failed to start | [receipt](verification/harness.json), [log](verification/harness.txt) |

247 is an invocation count, not a distinct-behavior count; shared modules run in multiple targets. Harness stderr contains `error: Script not found "test:coverage"`. No coverage percentage was measured. `SKIP_COVERAGE` was unset; no dummy script, substitute tooling or harness change was used.

Resolved earlier attempts remain in their owners' logs: storage formatting initially exited 1 and was corrected; all-target offline fetch exited 101 for uncached Windows-only `anstyle-wincon`, then host-only offline preparation passed without a download. Backend preparation initially exited 101 for missing new report initializers; a focused test attempt exited 101 for private deadline access. Owners corrected these within their scopes before passing focused and final workspace checks. Those earlier results are not presented as current failures or erased.

Task-8 ran scoped Git status/diff/HEAD inspection, known-path artifact reads and the read-only Python source/receipt audit (exit 0). Final JSON, links, ownership and fingerprint checks are recorded in `final.json`. Skills used here: oma-coordination and oma-pm to consume the existing handoff, assess unchanged acceptance obligations and deliver the decision; no plan or task graph was regenerated. Implementation owners recorded oma-db and oma-backend, and independent review recorded oma-qa.

## Limits, lessons and next actions

Coverage is the remaining verification prerequisite. Preserve this result. A separately assigned repair of the missing coverage entry point must retain the exact harness command and be followed by refreshed source-bound evidence and affected review; this final-report task does not own that shared tooling. No product-code remediation is requested by independent QA.

The tests establish native Darwin behavior in the stated fixtures. They do not establish other OS qualification, field usability or commercial results. Serena was unavailable, so this task used supplied paths and evidence/hash inventories without native repository code discovery. `av.yaml` is inaccessible in recorded inspection; no whole-worktree cleanliness claim is made. Mise emitted a tracking-symlink permission warning during Python/tool execution, while the audit itself exited 0. No denied action was retried through GUI.

The useful engineering lessons are to validate history after taking the write lock, keep the whole migration batch atomic, use a version witness to detect trailing-history loss, and separate transport compatibility from schema semantics. Handshakes and whole-state snapshots make concurrency, interruption and rejection claims reviewable. Reusing source-matched receipts avoids repeating settled checks; any later product-input change requires refreshed evidence.

No external deployment, operational restart, outreach, publication, spending action or distribution occurred in the reported mission work. Test-owned daemons were started/stopped/restarted for deterministic checks. Provider/API cost remains unknown; no zero-cost claim follows. No browser suite, Playwright installation, ComputerUse, GUI or screen-based verification was performed. Visual verification is not relevant to this terminal-only change.
