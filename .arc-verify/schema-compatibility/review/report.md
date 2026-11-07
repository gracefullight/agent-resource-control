# Independent schema compatibility review

## Review Result: PASS for product requirements

Task: task-7. Actor: independent-reviewer. Skill: oma-qa, including its execution protocol and checklist. No unresolved implementation blocker was found. The recorded Cargo gates pass on the inspected sources. The unchanged harness exited 1 because `test:coverage` is missing; coverage remains unresolved. This is not a passing harness verdict.

This review inspected the existing changes first, then the storage engine, fixture SQL and assertions, daemon/IPC/CLI integration, relevant runtime ownership paths, baseline client code and executable evidence. Director advice and preceding verdicts were not used as proof. No product files, preceding artifacts, task instructions or mission state were changed. All new files are under the assigned `review/` directory.

## Findings

### CRITICAL

None.

### HIGH

None.

### MEDIUM

No product defect identified.

### LOW

No product defect identified.

The outstanding coverage gate is recorded separately below as a verification failure. No repair to product code is requested. The Chief Director retains coordination of the existing harness prerequisite and final delivery.

## Source and evidence binding

Reviewed source fingerprint: `3ff8691bc3d81511f30ec68bca9a62e7bd74dabd13a3fd686a955d189d302ca5`, defined as SHA-256 of the exact bytes of `../verification/source-manifest.json`. The manifest contains 92 relevant source/configuration inputs, including every crate Rust, SQL and TOML input. It is a scoped source fingerprint, not a whole-worktree hash. HEAD remains accepted base `bf28a35b594f5ae83ac97c216bd3b09e60a2dfa1`; implementation is present in working-tree changes and new files.

The independent [audit](audit.json) recomputed every source entry, verified complete crate-input coverage, checked all four required command receipts against current files and their before/after manifests, and checked their complete log hashes. All 28 entries in the verification artifact manifest also match. The audit checked 68 acceptance-test references across 26 groups against actual log lines and independently counted 247 passing invocations. Shared test modules and child helpers mean this is not a claim of 247 distinct behavior tests.

The receipt recorder was inspected: it executes the recorded argv from this worktree, captures combined output, records actual process exits, bounds execution and hashes sources before/after. It is local evidence, not independent cryptographic attestation of a remote run. Inspection of the actual logs and source assertions supports reuse under the task's explicit receipt-reuse instruction. No changed inputs or concrete executable gap required repeating the Cargo suites.

## Implementation assessment

`crates/arc-storage/src/migration.rs:6` defines the production registry using `include_str!` for `0001_phase0.sql`. Registry validation requires consecutive versions from 1, matching canonical numeric ID prefixes, nonempty SQL and unique IDs. Public `open_and_migrate` and `assess_schema` always use this registry; alternate-registry operations are crate-private. Fixture SQL, observers and synchronization helpers enter through the `cfg(test)` module only. No environment setting, runtime SQL discovery, public registry override, production migration 0002 or dependency was added.

`migration.rs:147` acquires `BEGIN IMMEDIATE` before assessing history. Pending SQL, parameterized history inserts and the `user_version` witness execute inside that transaction. Failure explicitly rolls back, including earlier migrations within the batch. A competing upgrader assesses authoritative history after obtaining its lock. Compiled SQL is trusted source input; migration identifiers/checksums are bound parameters rather than concatenated external input.

`crates/arc-storage/src/history.rs:79` checks stored value types, canonical IDs, numeric order, duplicates, prefix gaps, exact known IDs and checksums. The version witness detects a deleted trailing entry. Recognizable ARC objects without history are rejected, including empty objects; they are not treated as a fresh database. Known-prefix checksum defects take precedence over future history. A higher stored version or witness rejects opening without downgrade SQL. The original production version-1 history with witness 0 remains accepted and is stamped atomically. Read-only assessment uses a transaction snapshot, rolls it back, and respects an existing caller-owned transaction.

Daemon assessment runs on the existing single SQLite worker in `crates/arc-daemon/src/db_worker/execution.rs`. It retains the progress/busy handlers and checks cancellation/deadline/shutdown around assessment. No pool, diagnostic connection, detached migrator, arbitrary SQL endpoint or additional worker was introduced. `connection.rs` still routes hello/health independently; `lifecycle.rs` still owns admission, one worker, handler retention, a shared shutdown deadline, close/join and owned socket cleanup. Instance admission still precedes opening SQLite in `main.rs`.

`crates/arc-ipc/src/schema.rs` validates explicit compatibility details instead of requiring a modern migration count of 1. `DoctorReport.schema` is optional with Serde defaults and omitted when absent. The protocol remains 1 and legacy fields remain truthful. Error responses retain schema diagnostics. Unknown, malformed, unverified or internally inconsistent modern details cannot claim compatibility. Integrity failure keeps precedence. `crates/arc-cli/src/doctor.rs` preserves existing capability disclosures and plain terminal output, explains missing legacy details, and reports schema reasons/versions through failure stderr. Startup rejects before serving IPC; its cause is on daemon stderr, with no claim that doctor can query a daemon that never became ready.

## Acceptance assessment

All test outcomes below refer to the source-matched `cargo test --workspace` log in `../verification/workspace-test.txt`. The full 26-group mapping remains in the inspected [acceptance matrix](../verification/acceptance-matrix.md). These conclusions come from the actual test bodies and output, not their names alone.

| Criterion | Inspected assertions and executable evidence | Assessment |
| --- | --- | --- |
| Fresh initialization, reopen, WAL and constraints | `tests/nfr010.rs` creates the actual production schema, pins its checksum, writes a vendor, closes/reopens, compares the history tuple and retains the record. WAL, foreign keys and uniqueness are asserted. Log lines 358–364. | Pass |
| Actual two-version additive upgrade | `migration/tests/acceptance.rs` invokes the same `migrate` engine with two compiled isolated SQL fixtures. Version 2 adds a column/table and backfills the original vendor into the new table. Original ID/name and original history tuple remain exact; the new history ID/checksum is asserted. Log line 344. | Pass |
| Repeat-open idempotency | That test closes/reopens three times and compares complete schema, all table rows, history and witness snapshots; compatibility reports version/count 2. Existing version-1 reopen is also compared before upgrading. Log line 344. | Pass |
| Legacy database reopen | `production_legacy_history_is_read_only_then_stamped_atomically` checks a production database with witness 0: assessment changes nothing, migration changes only the witness, and reopen preserves the result. Log line 347. | Pass |
| Concurrent serialization | `migration/tests/concurrency.rs:23` gates A after acquiring the write transaction. SQLite's busy callback proves B attempted the held lock. A commits before B retries. B then reads two committed history tuples and produces the exact same final snapshot. Ordering does not depend on a sleep. Log line 341. | Pass |
| SQL-failure rollback | `acceptance.rs` tests initial SQL failure, failure after altering/updating an existing database, and failure after a prior migration in one batch. Each compares the full pre-attempt snapshot; a valid retry succeeds. Log lines 337, 338, 340. | Pass |
| Interrupted execution rollback | `concurrency.rs:82` starts a child that asserts pending v2 history, witness and backfill before flushing `ARC_PENDING_COMMIT` at the engine's pre-commit hook. Parent observes reader isolation, kills and reaps the child, closes/reopens and compares the complete pre-upgrade snapshot. A subsequent upgrade and repeat migration succeed. Log line 342. The default no-op child helper at line 335 supplies no interruption evidence by itself. | Pass |
| Altered checksums | `rejection.rs` corrupts each of the two known history entries separately. `reject_unchanged` checks read-only assessment, typed rejection, autocommit restoration and exact schema/all-row/history/witness equality. Log line 346. | Pass |
| Missing history | Tests remove the table, all rows, the first row and trailing row, and test empty recognizable ARC objects and nonzero witnesses. The same complete snapshots remain unchanged through assessment and rejection. Log lines 350–351. | Pass |
| Newer schema and downgrade rejection | A lower registry rejects the actually upgraded two-version fixture unchanged. Separate future-history and future-witness cases also reject. Applied version 2/3 is checked. Log line 349. No downgrade execution is implemented or claimed. | Pass |
| Invalid registry/history and read-only assessment | Malformed/reordered/gapped/duplicate/empty registries reject before SQL. Invalid IDs/types/duplicate versions/witnesses reject; history insertion/time order does not define migration order; unreadable layouts stay typed SQL errors; nested assessment does not commit caller data. Log lines 336, 339, 343, 345, 348, 352. | Pass |
| Real startup rejection preserves data/history | `tests/startup.rs` runs the daemon against checksum, missing, invalid and future histories; checks nonzero exit, empty stdout, exact recovery stderr, absent socket, and vendor/history/schema/witness snapshots. Log lines 191–192. | Pass |
| Live diagnostics and actual CLI | `tests/resident/schema.rs` externally changes history only as a fixture after daemon readiness. Doctor returns explicit checksum/missing/invalid/newer statuses, accurate counts and false history verification. Actual CLI fails with the reason, status remains usable, and repeated diagnostics/shutdown preserve snapshots. Log line 184. | Pass |
| Successful CLI versions and failure text | `arc-cli/tests/schema.rs` runs actual arcctl against version-1/version-2 wire fixtures and checks exact success text. Every schema rejection, malformed/unknown detail, inconsistent success, legacy missing detail, help and existing error category is checked. Log lines 32–38. These wire fixtures verify rendering/validation; storage tests establish upgrade execution. | Pass |
| Older-client compatibility | Literal baseline protocol-1 requests and baseline decoder/count checks accept the actual new production daemon; the baseline CLI source at the accepted Git base was compared to the adapter. New-client/old-daemon fixtures label absent/null schema details without asserting verified history. Modern count 2 succeeds while the legacy count==1 validator honestly rejects it. Log lines 33, 183, 313–317. Historical binaries were not separately compiled. | Pass |
| One bounded worker and cancellation/deadline/shutdown | `db_worker/schema_tests.rs` gates preparation of the actual `user_version` query. Cancellation, deadline and shutdown cannot escape as success; recovery, high-water execution count 1 and close/join are asserted. Existing finite/concurrent queue and 100,000-attempt tests still execute. Log lines 67–69, 90, 228, 232, 235, 251. | Pass |
| Health responsiveness | `worker_runtime.rs` proves a real schema preparation stall is active, then actual status completes within the asserted bound; releasing the gate lets doctor complete and recover. Existing SQL stall and saturation tests remain exercised. Log lines 230, 272–273. This does not promise health admission under complete transport saturation. | Pass |
| Runtime identity and shutdown guarantees | Unchanged instance/identity paths retain per-user admission and native runtime identity. Tests cover distinct socket overrides, death/restart, TMPDIR independence, real SIGINT/SIGTERM under saturation, stalled-handler reaping and worker join/handle retention. Log lines 117–118, 178–179, 236, 274–275. | Pass |

For preservation cases, the central storage snapshot helper reads all table columns/rows, full `(id, applied_at, checksum)` history tuples, schema definitions and `user_version`. The additive upgrade intentionally adds/backfills fields while retaining old record values; rejected, failed and interrupted attempts require exact whole-snapshot equality. The real daemon startup/live tests additionally exercise the production schema and process boundary.

## Immutable inputs and size constraint

Independent Git-object comparison establishes `0001_phase0.sql` remains exactly 3,917 bytes with SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. The harness, root Cargo manifest/lockfile and toolchain also match the accepted base byte-for-byte. All 248 recorded source-length inputs were rehashed and recounted; the maximum is 474 lines. All crate Rust inputs, including untracked additions, fit the 500-line limit. Protocol constant inspection and the independent audit confirm version 1.

## Commands and actual outcomes

| Command | Evidence used | Outcome |
| --- | --- | --- |
| `cargo fmt --all -- --check` | Reused source-matched `../verification/fmt.json` and complete empty output log | Exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | Reused source-matched receipt and log; no accepted warnings | Exit 0 |
| `cargo test --workspace` | Reused source-matched receipt and complete log; independent count and source assertion inspection | Exit 0; 247 passing invocations, zero failed/ignored |
| `./scripts/harness/validate.sh` | Reused unchanged-script receipt and complete log; `SKIP_COVERAGE` unset | Exit 1; checks 1–4 pass; coverage cannot execute because `test:coverage` is absent |
| `python3 .arc-verify/schema-compatibility/review/audit.py` | Executed during this review and repeated after writing the assessment; [audit.json](audit.json), [captured output](audit-output.txt) | Both runs exit 0; all source, log, artifact, preservation, line-count and reference assertions pass |

Read-only shell commands also included scoped Git status/diff, `git show` for baseline comparisons, known-path file reads and compact JSON inspections. Git status reported `av.yaml: Operation not permitted`; it was not read or modified, and whole-worktree cleanliness is not claimed. Serena tools were not exposed in the available tool catalog; code review used already supplied paths, diff contents and evidence manifests, without substituting native repository source searches. The memory quick pass found no relevant prior ARC entry and supplied no review evidence.

## Remaining risk and handoff

The unresolved verification issue is the missing `test:coverage` script, recorded at `../verification/harness.txt` and `harness.json`. It is not a product regression established by this review, a measured coverage percentage, or a passing gate. Product acceptance and the requirement to retain the harness's actual failure are satisfied; coverage qualification itself is incomplete.

Execution evidence is native aarch64-apple-darwin. Legacy compatibility is established through baseline behavior adapters and response fixtures rather than separately built historical executables. Version-2 schema evolution is exercised with isolated storage fixtures; production remains version 1. Downgrade rejection is verified, and downgrade execution remains unsupported. No deployment, business result, field usability result, browser suite or screen check is claimed.

Deliverables: `review/report.md`, `review/assessment.json`, `review/audit.py`, `review/audit.json`, `review/audit-output.txt`. Ownership stayed within task-7. No interface conflict or product repair is outstanding; any subsequent product-source change requires refreshed affected receipts and independent review.
