# Task-4 storage handoff

Status: completed for the assigned storage scope. Skill used: oma-db, including its execution protocol, checklist and application-ownership review. No delegation, mission-state changes, manifests, lockfile or harness edits were made. Existing task-1/2/3 artifacts were preserved.

## Delivered implementation

- `crates/arc-storage/src/lib.rs`: retains `StorageResult`, `open_default` and `open_and_migrate`, local path checks, foreign keys, WAL and the five-second startup busy timeout. Reexports the task-1 public contract.
- `crates/arc-storage/src/migration.rs`: implements `Migration`, compiled `MIGRATIONS`, `MIGRATION_ID`, `SchemaStatus`, `SchemaCompatibility`, `MigrationError` and `assess_schema`. Registry validation checks nonempty SQL, contiguous versions, canonical IDs and uniqueness. Exact UTF-8 SQL bytes are SHA-256 hashed. Typed SQLite failures retain `source()`.
- `crates/arc-storage/src/history.rs`: validates every stored ID, timestamp type and checksum in numeric version order; detects recognizable ARC tables/indexes, missing/trailing/gapped history, unsupported versions and inconsistent witnesses. Read-only assessment owns one deferred snapshot or reuses its caller's transaction.
- `crates/arc-storage/src/migration/tests.rs` and `tests/{acceptance,concurrency,rejection}.rs`: private unit-test helpers, full database snapshots, synchronization and storage acceptance cases.
- `crates/arc-storage/tests/fixtures/{0001_fixture,0002_additive}.sql`: isolated compiled test fixtures. Fixture registries and synchronization hooks exist only under `cfg(test)`; production has no override, runtime SQL discovery or fixture configuration.
- `.arc-verify/schema-compatibility/storage/`: source manifest, immutable-input proof, command receipts, complete command logs and this report.

Production remains schema 1. `0001_phase0.sql` is 3,917 bytes and matches accepted base `bf28a35b594f5ae83ac97c216bd3b09e60a2dfa1` byte-for-byte. SHA-256: `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. No production 0002 was added. All storage Rust sources/tests are below 500 lines; see `source-manifest.json`.

## Transaction and schema boundaries

The external interface adds reusable compatibility assessment without changing existing open signatures. The conceptual model is an ordered registry and an exact committed history prefix, with `PRAGMA user_version` as an independent witness. Internally, `schema_migrations(id TEXT PRIMARY KEY, applied_at INTEGER NOT NULL, checksum TEXT NOT NULL)` and the production application schema remain unchanged.

Startup validates the compiled registry, acquires `BEGIN IMMEDIATE`, then takes the authoritative history/witness snapshot. All pending SQL, parameterized history inserts and the final witness commit in one transaction. Rejections perform no application/history/witness writes. SQL errors explicitly roll back; observable rollback errors propagate as typed SQLite failures. WAL recovery handles process death before commit. This preserves SQLite ACID behavior and existing integrity constraints. The bounded worker and all daemon/IPC/CLI ownership remain outside this crate.

The original valid production history with witness 0 is accepted read-only and stamped during successful startup. A fixture with later history and witness 0 is rejected. A lower supported registry refuses an upgraded fixture; there is no DOWN SQL or downgrade execution.

Application ownership review: migration ordering and compatibility policy remain explicit Rust code. No triggers, stored procedures, implicit business workflow, ORM hooks or external effects were introduced. Existing PK/FK/unique constraints remain in SQLite and their existing acceptance tests pass. No application data model, capacity policy or backup product was added.

## Acceptance checklist and case mapping

Names below are under `migration::tests::` unless stated otherwise. `storage-test.txt` contains their actual passing results. Snapshots capture every application table's complete rows, all history tuples, schema definitions and the witness; rejected assessment and migration attempts are both compared against the baseline.

| Criterion | Executed case and evidence |
| --- | --- |
| Fresh initialization, schema objects, WAL, constraints, original checksum | Existing `nfr010_wal_migration_survives_close_and_reopen_without_cleanup` and `foreign_keys_and_unique_resource_identity_are_enforced` integration tests; original tests unchanged |
| Existing-database reopen, additive v1-to-v2 upgrade, record preservation, exact `(id, applied_at, checksum)` history, repeat-open idempotency | `acceptance::fixture_initialization_additive_upgrade_reopen_and_idempotency`; closes/reopens v1, preserves original vendor fields, checks new default/backfill rows, preserves the exact v1 tuple, verifies the v2 checksum/timestamp and three repeat opens |
| Original production legacy history and witness | `acceptance::production_legacy_history_is_read_only_then_stamped_atomically`; assessment preserves all state and startup changes only witness 0 to 1 |
| Concurrent-upgrader serialization and authoritative reread | `concurrency::concurrent_upgraders_serialize_and_reread_after_lock_acquisition`; A signals after acquiring the immediate lock; B's SQLite busy handler signals actual contention; parent permits A to commit before allowing B to retry; B's acquired-lock hook sees both committed history rows; final complete snapshots match |
| Failed additive migration rollback | `acceptance::sql_failure_rolls_back_schema_application_data_history_and_witness`; pending ALTER/UPDATE/CREATE followed by invalid SQL leaves the complete v1 snapshot unchanged; subsequent valid upgrade succeeds |
| Failed initial migration and multi-entry atomicity | `acceptance::initial_sql_failure_rolls_back_every_pending_object` and `acceptance::later_failure_rolls_back_prior_migration_history_in_same_batch`; failure in entry 2 removes entry 1's already-executed DDL/history too |
| Process interruption rollback and recovery | `concurrency::process_interruption_before_commit_rolls_back_and_recovers`; child `concurrency::interrupted_child` confirms pending v2 DDL/data/history/witness before flushing a stdout handshake; parent sees the old snapshot, kills/waits for the child, reopens and checks exact baseline equality, then upgrades and checks idempotency |
| Every known checksum, including the final migration | `rejection::checksum_defects_in_every_known_entry_leave_all_state_unchanged` |
| Missing history table, all rows, interior gap and deleted trailing row | `rejection::missing_history_table_rows_gaps_and_trailing_entry_are_rejected`; trailing deletion detected through witness |
| Empty ARC tables/indexes/history or nonzero witness cannot be adopted as fresh | `rejection::empty_arc_objects_or_history_and_nonzero_witness_are_never_fresh`; includes ASCII case-insensitive ARC object recognition |
| Newer history, newer witness and explicit downgrade rejection | `rejection::unsupported_newer_history_and_witness_and_downgrade_are_rejected`; a v2 fixture opened with a v1 registry is refused unchanged, then future v3 history/witness cases are refused unchanged |
| Malformed IDs, timestamps, checksum types, duplicate numeric versions, inconsistent/negative witness | `rejection::invalid_history_ids_types_duplicates_and_witness_are_rejected` |
| Deterministic defect precedence | `rejection::known_prefix_defect_takes_precedence_over_future_history` |
| History ordered numerically, independently of insertion order/timestamps | `rejection::numeric_version_order_ignores_history_insertion_and_timestamp_order` |
| Safely unreadable layouts remain typed SQL errors | `rejection::unreadable_layout_stays_a_typed_sql_error`; state unchanged |
| Read-only assessment within an existing transaction | `rejection::assessment_reuses_existing_transaction_without_committing_it`; caller's uncommitted write remains uncommitted and rolls back |
| Malformed registry rejected before SQL | `acceptance::malformed_registry_is_rejected_before_any_sql`; empty, reversed, skipped, duplicate, wrong/empty/noncanonical ID, empty SQL and overflow cases |

Concurrency and interruption tests use channels, SQLite busy-handler notification and child-process I/O handshakes. They contain no timing sleeps. Ten-second receive deadlines bound failed handshakes; they do not determine when to release a lock or kill the child. Child ownership is guarded, and the child is killed and reaped on normal completion or parent assertion failure.

## Commands and actual outcomes

Each recorded check was run from the mission worktree using `python3 .arc-verify/schema-compatibility/storage/record.py LABEL COMMAND...`. Each receipt includes actual argv, exit, elapsed time and before/after input hashes; its `.txt` file is the complete combined command log. Final receipts match the current storage source hashes.

| Command | Outcome | Receipt/log basename |
| --- | --- | --- |
| `cargo test --offline -p arc-storage` | Exit 0; 18 unit tests and 4 integration tests passed; 0 failed | `storage-test` |
| `cargo clippy --offline -p arc-storage --all-targets -- -D warnings` | Exit 0 | `storage-clippy-final` |
| `cargo fmt --all -- --check` | Exit 0 | `fmt-final` |
| `git diff --check -- crates/arc-storage` | Exit 0 | `diff-check` |
| `cargo fetch --offline --target aarch64-apple-darwin` | Exit 0; workspace dependencies for the current host are cached | `offline-host-dependencies` |
| `cargo fmt --all -- --check` before formatting | Exit 1; storage formatting differences, corrected with `rustfmt --edition 2021 crates/arc-storage/src/lib.rs` | `fmt-initial` |
| `cargo fetch --offline` across all target platforms | Exit 101; uncached Windows-only `anstyle-wincon v3.0.11`; no download attempted | `offline-dependencies` |

`storage-test-initial` and `storage-clippy` retain passing checks before the final rollback/case-recognition additions; they are superseded by the final receipts above. Rust is 1.95.0, host `aarch64-apple-darwin`. No dependency, compiler or unrelated tooling was installed. Mise emitted an existing permission warning while trying to register the worktree config; all required focused checks still exited successfully.

## Chief/backend handoff and remaining work

Task-1 revision 1's public types, variant names, signatures and production registry contract are implemented. Storage has no dependency on IPC or Serde. There is no ownership conflict or needed manifest change.

Concrete backend integration requirement: `crates/arc-daemon/tests/startup.rs:78` still expects the original checksum-error sentence. Under the new agreed error contract, the daemon's existing formatting yields `Database startup failed: migration checksum mismatch; preserve the database and investigate against a trusted copy.` plus newline. The backend owner should update that assertion and implement/test the new assessment mapping. This was identified by reading the actual file; the daemon integration suite was not run by this storage task.

The Chief retains final `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, the unchanged `./scripts/harness/validate.sh`, integrated protocol/CLI/runtime checks and independent oma-qa. No current harness result or coverage success is claimed here. Host offline dependency readiness does not claim that uncached cross-platform targets work. Fixture downgrade refusal proves preservation, not downgrade execution. Full-source hashes and scope are in `source-manifest.json` and `summary.json`.
