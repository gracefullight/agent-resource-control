# Traffic storage handoff

Status: storage implementation and its focused tests are complete; daemon integration and mission-wide verification remain incomplete.

## Scope and provenance

The Actor worktree's inspected HEAD is `1c6e98590303b4ae697dd0cfcb00fa430fdfb6cb`, tree `2109a44083d1e89a9c4cd5dfdb78d9126a732488`. The changes listed below are uncommitted in this worktree; they are not claimed as part of that commit. The supplied accepted baseline is `4d2f71c1a1482925f6c9918296d7ceb3435b7592`, whose tree was verified locally as `4455896fdf976244313a70a9fa38b730b344b411`. The supplied snapshot and worker refs did not resolve in this checkout. Full Git status/diff reported `av.yaml: Operation not permitted`; status restricted to accessible paths was otherwise clean at entry. Serena was not available in the tool inventory; inspection used known source paths. No other Actor's files were edited.

## Changed files

- `crates/arc-storage/migrations/0003_traffic.sql`: forward-only queue, tombstone and exact permit-accounting fields.
- `crates/arc-storage/src/migration.rs`: registers migration 3.
- `crates/arc-storage/src/jobs.rs` and `crates/arc-storage/src/jobs/transitions.rs`: expose the Traffic boundary and reject legacy lifecycle mutations for Traffic-owned IDs.
- `crates/arc-storage/src/jobs/traffic.rs`, `traffic/admission.rs`, `traffic/lifecycle.rs`, `traffic/store.rs`: transactional storage operations, reads and evidence interfaces.
- `crates/arc-storage/src/jobs/traffic/tests.rs` and `traffic/tests/{admission,lineage,recovery}.rs`: storage regression tests.
- `crates/arc-storage/src/jobs/tests/migration.rs`, `src/migration/tests/acceptance.rs`, `tests/managed_exec_migration.rs`, and `tests/nfr010.rs`: version-3 migration expectations and upgrade, reopen, rollback, checksum and preserved-schema checks.
- `crates/arc-storage/traffic-storage-report.md`: this handoff.

## Database model and transaction contract

The external interface is `arc_storage::jobs::traffic`, using the delivered `arc_ipc::traffic::{JobRequest, JobResult, StatusResult}` and `arc_traffic::Policy`. The conceptual entities are an existing lifecycle job, a durable queue entry, at most one root permit, optional reused descendants, and owner-bound tombstones for requests that terminate before enqueue commits. The internal schema uses an `AUTOINCREMENT` sequence for FIFO order, a unique queue job ID, foreign keys, domain checks, and the existing unique `permits.job_id`. Exact CPU millicores and memory bytes are stored as integers; the old real-valued CPU field remains an estimate for existing consumers. Existing permits and both historical migration files remain intact.

Each mutation uses a short SQLite `IMMEDIATE` transaction. FIFO-head selection, active-permit counts, aggregate resource totals, policy decision, permit insertion and lifecycle claim commit together. SQLite's serialized writer and atomic rollback provide the consistency boundary; callers must continue to use the existing daemon worker. All active permits are counted. An older active permit lacking an exact reservation, or containing an unrecognized class, causes eligible admission to fail closed instead of silently ignoring capacity. Such a row needs reliable reconciliation by its owner; bypass requests do not reserve or inspect capacity.

A queued cancellation or fail-open operation is terminal. If either precedes enqueue, a tombstone preserves the first terminal decision and owner tuple across restart; delayed enqueue and poll cannot revive it. An uncertain committed grant remains reserved until an authenticated completion or other safe evidence is supplied. Completion or confirmed pre-spawn cancellation releases at most once, and a completed parent retains its permit while a recorded reused descendant is active. The storage layer never starts or replays a child. An owner observation can tombstone a still-queued entry only if the stored identity and lifecycle revision still match; stale or indeterminate observations do not release a grant.

## Backend integration requirements

- The daemon must authenticate each socket peer and bind it to the request's native PID/start/boot identity. `OwnerEvidence::gather` and `ChildEvidence::gather` collect process evidence outside the database transaction; storage compares owner tuples and verifies the evidence used to register or attach.
- Mint an unpredictable unique 256-bit `LineageContext` for a new grant. `lineage_snapshot` provides the current root snapshot and generation without returning a context. The daemon must independently verify native socket peer identity and ancestry before constructing `LineageProof::verified`; that constructor is an assertion by the trusted daemon, not an OS ancestry verifier. Storage rechecks context, root state, anchor identity, revision and generation inside its transaction. A stale proof returns `IdentityUnavailable` and should be rechecked; a context without proof follows ordinary admission. The nested tests supply a test proof and do not prove native ancestry.
- Call `queue_head`, gather `QueueObservation` outside the writer, then call `reconcile_queued` to make progress past a verified missing or mismatched unreserved head. Do not release a permit based only on a missing owner or an indeterminate child.
- A `CANCEL` with a generation asserts that the authenticated owner has confirmed no child was spawned. A cancellation without that proof retains an uncertain grant. Repeated `Granted` or `Reused` replies are stable retry results, not additional permission for the CLI to spawn again; the backend/CLI must serialize the launch and fallback decision.
- Dispatch `enqueue`, `poll`, `attach`, `complete`, `cancel`, `fallback`, and `status` through the single SQLite worker. Generate fresh grant contexts outside the transaction. Legacy protocol-1 operations and IPC payload definitions were not changed; bypass jobs can still follow the legacy lifecycle path because no Traffic row is inserted.

Migration hashes are `0001_phase0.sql` = `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`, `0002_managed_exec.sql` = `464a1c88270f01a56ccab6610f87eff7648e0640f276ebce53b8a97026c50814`, and new `0003_traffic.sql` = `1f4522e3f27c124aea7127d92434c7bd22be36168fef0a4bf665f8cf16debe90`. The first two hashes match the accepted baseline's migration bytes.

## Checks and unresolved handoffs

- `CARGO_NET_OFFLINE=true cargo test -p arc-storage`: passed, 64 tests, zero ignored. Includes FIFO ties, concurrent pollers, aggregate budgets, bypass, duplicate and conflicting retries, rolled-back grant, cancellation before enqueue and after grant, spawn failure, lost acknowledgements, reopen, uncertain reservations, owner/PID mismatch, and reused descendants. The migration tests cover version-2 upgrade with existing rows, reopen, failure rollback after both ALTERs, and rejection of altered 0001/0002 checksums. Native ancestry and actual child execution are not established by these storage tests.
- `cargo fmt --all -- --check`: passed.
- `CARGO_NET_OFFLINE=true cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `CARGO_NET_OFFLINE=true cargo test --workspace`: failed at the daemon unit-test binary: 45 passed and 16 failed because its expectations still say schema version/count 2. The directly observed failing locations are `crates/arc-daemon/src/db_worker/tests.rs:35`, `:219`, `crates/arc-daemon/src/database_failure_tests.rs:351`, and `crates/arc-daemon/src/server_tests.rs:82`. `crates/arc-daemon/tests/support/runtime.rs` also contains a version-2 doctor expectation. These are backend-owned paths and need a dependency-based handoff, then all workspace tests must be rerun.
- `bun test scripts/harness/managed-exec.test.ts`: 2 passed, 10 failed before the final storage changes. The live preservation check rejects the necessary version-3 changes in `crates/arc-storage/src/migration/tests/acceptance.rs`; historical fixtures also try to scan a nonexistent `arc-traffic/src`. The verification owner must narrowly update `scripts/coverage/schema-adaptations.ts` and historical inventory handling while retaining existing rejection tests and assertions.
- `bun run test:coverage` and `./scripts/harness/validate.sh` ran without skip flags before the final storage changes. Both failed because the instrumented daemon tests still expect schema 2; the gate reported every crate's current coverage as unavailable. The harness scans tracked files, so it did not validate these then-untracked additions. Re-run both on the integrated final source, then run the live evidence check. No 80% gate pass is claimed.
- A separate storage-only `cargo llvm-cov` run before the final changes reported 1,324/1,435 mapped storage source lines (92.26%) by a simple LCOV line count. It includes test-only spans and is not the required production-line accounting or a current gate result. Its first attempt used unsupported `--target-dir` and failed; a fresh `CARGO_TARGET_DIR` run succeeded. An early library test also raced creation of its test module and failed before the file existed; the completed focused run above supersedes it.
- `git diff --check -- crates/arc-storage`: passed for tracked changes; all new Rust files are below the harness's 500-line limit.

Raw logs are local under `target/`; SHA-256:

| Path | SHA-256 |
| --- | --- |
| `target/traffic-storage-tests.log` | `bbd278fe1a632bf9ad1dcae3698678c7b22623977c7570030295fa2d08fe6a67` |
| `target/traffic-storage-clippy.log` | `90b28126e2c84d101c15826f97f3b3683f105fa7dfff99ea48ecda8cf20fc824` |
| `target/traffic-storage-workspace-test-final.log` | `c56d4f48861ef3b38c3cd6e29380909227269b6fb3eee87ba6ec56ca86cff27f` |
| `target/traffic-storage-preservation.log` | `c777e9ae5918b469bc9fcf7e9e69f15bd2449ffb1cc7f5502d7c7131e9cc2edd` |
| `target/traffic-storage-coverage-gate.log` | `cf6e8f858a146e0a0f0bac16b3221aa4f0fe838d9eb5d047bcf69070b4456d62` |
| `target/traffic-storage-harness.log` | `5b29ffee4d6bf92b3c7c2c4bf89972208a255ec167aa6bea8e54f97a17f0d78e` |
| `target/traffic-storage.lcov` | `a21111bfdbb03ac54fe129bdfcd096545fc5cad268b7ef40fc424ae4288c0313` |
| `target/traffic-storage-llvm.log` | `ce8f01a26b489cf5e0ba21f091d5246257e00afff40239710a9e6ce920f580ee` |

Skills used: `oma-db`; the applicable database rule, shared execution policy and DB checklist were read. The material remaining scope includes native daemon/CLI integration and ancestry verification, the real macOS two-directory experiment, independent QA, final coverage/harness and target checks. Desktop, native Windows, vendor hooks, broader detection and remaining scheduling policy are outstanding.
