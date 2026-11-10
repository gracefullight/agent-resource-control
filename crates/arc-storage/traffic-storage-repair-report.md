# Traffic storage repair handoff

Status: the two reported defects have storage-level repairs and regression tests. Daemon/CLI integration and the mission-wide gates remain incomplete.

## Scope and provenance

The accepted committed baseline is `4d2f71c1a1482925f6c9918296d7ceb3435b7592`, tree `4455896fdf976244313a70a9fa38b730b344b411`. This Actor's inspected local HEAD and initial snapshot are `c8e8f3ee47978122703d429e1531eeb6ce67fe8b`, tree `df5fd3f3f6f9ec77747dddc1ea28fb0e3387246c`; that snapshot already contains the previous Traffic storage implementation and other input overlays. The repair below is an uncommitted diff against that local snapshot, not a claim about the accepted baseline or the integrated workspace. The supplied snapshot and worker refs did not resolve in this Actor checkout; the snapshot commit and tree did resolve. Full `git status` reported `av.yaml: Operation not permitted`. Storage-scoped status was clean at entry. Serena was not exposed in the available tool inventory; inspection used known paths.

Changed in this repair:

- `crates/arc-storage/src/jobs/traffic.rs`
- `crates/arc-storage/src/jobs/traffic/admission.rs`
- `crates/arc-storage/src/jobs/traffic/lifecycle.rs`
- `crates/arc-storage/src/jobs/traffic/tests/lineage.rs`
- `crates/arc-storage/src/jobs/traffic/tests/recovery.rs`
- `crates/arc-storage/traffic-storage-repair-report.md`

The existing `crates/arc-storage/traffic-storage-report.md` is preserved. No manifest, migration, daemon, CLI or IPC file was modified in this repair.

## Recovery and transaction contract

An ordinary cancellation without a generation still retains an uncertain grant. A subsequent authenticated owner cancellation with the matching generation now changes an unspawned `RETAINED` entry to `CANCELLED` and releases its permit at most once. The new `confirm_cancel_unspawned` storage operation handles a permanently lost generation after the owner explicitly confirms no spawn. It requires matching live owner evidence and the stored complete PID/start/boot identity. A recorded child prevents this release. An unknown-generation cancellation alone never supplies that confirmation. The terminal entry prevents delayed enqueue, poll or attach from starting the same job again.

The lineage lookup now returns all active anchor candidates under the still-active root permit, including descendants of a completed or cancelled root. The root permit remains the accounting owner. `LineageProof` binds the selected candidate, its revision and anchor identity to that root and generation; storage rechecks them against the opaque context and active permit inside the admission transaction. A context without a trusted ancestry proof follows ordinary queue admission. The last active descendant releases the root permit when the root is terminal.

The conceptual model remains one root reservation, zero or more reused descendants and owner-bound terminal records. The existing SQLite `IMMEDIATE` transactions serialize each check and mutation; release and cancellation commit atomically. No schema or migration change was needed for this repair. The existing unique job/permit keys and FIFO sequence remain unchanged. SQLite is the durable source of truth; the application owns process launch, proof gathering and the no-spawn assertion.

## Required integration handoff

1. `crates/arc-ipc/src/traffic.rs` currently has `JobRequest::Cancel` with only an optional generation, and `JobResult::ReservationRetained` has no generation. Those wire shapes cannot distinguish a retry of an uncertain cancellation from an explicit no-spawn confirmation after the grant response was permanently lost. The backend owner must coordinate a compatible, explicit confirmation operation with the IPC owner, then route it to `confirm_cancel_unspawned` only after authenticating the socket peer against the owner identity. Serialize confirmation with the CLI's spawn decision so no child can be started after confirmation. Do not infer no-spawn from an ordinary `Cancel { permit_generation: None }` or a missing attachment.
2. The daemon should use `lineage_snapshots` and independently check native peer ancestry against every returned candidate's `root.observer` and, where present, `root.child`. Call `LineageProof::verified` only for an actually verified ancestor. The candidate can be a descendant; `permit_job_id` names the actual reservation owner. The existing singular `lineage_snapshot` remains available but returns only the first candidate and is insufficient when the matching ancestor is another active sibling. The `LineageSnapshot` struct gains `permit_job_id`, so any external struct literals need the added field. Storage tests simulate ancestry evidence and do not prove native OS ancestry.
3. `cargo test --workspace` fails in daemon-owned tests because migration 3 is active while those tests still expect version 2. The failures directly identify `crates/arc-daemon/src/db_worker/tests.rs:35`, `:219`, `crates/arc-daemon/src/database_failure_tests.rs:351`, and `crates/arc-daemon/src/server_tests.rs:82`. Update the daemon expectations through its assigned owner and rerun integrated verification. The earlier handoff also identified `crates/arc-daemon/tests/support/runtime.rs` for the version-2 doctor expectation; it was not reached by this workspace run.

## Acceptance evidence

- [x] Storage tests cover FIFO, atomic resource accounting, duplicate and conflicting retries, cancellation, spawn failure, restart, stale owner/PID identity, delayed and permanently lost grants, and one active root permit. The new tests also cover sibling anchors, a completed or cancelled root with active descendants, stale or forged proofs, and retaining a recorded child.
- [x] Migration tests cover version-2 to version-3 upgrade, reopen, transaction rollback and rejection of modified historical checksums. Direct SHA-256 comparison with accepted baseline bytes confirmed `0001_phase0.sql` = `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384` and `0002_managed_exec.sql` = `464a1c88270f01a56ccab6610f87eff7648e0640f276ebce53b8a97026c50814`. The existing `0003_traffic.sql` is `1f4522e3f27c124aea7127d92434c7bd22be36168fef0a4bf665f8cf16debe90`.
- [x] The delivered IPC types remain unchanged; the required no-spawn confirmation interface and native ancestry integration are recorded above.
- [ ] Actual CLI execution, native ancestry, the macOS two-directory experiment, per-crate production coverage, the full harness and independent QA require the integrated mission run.

Checks executed in this Actor worktree:

| Command | Outcome | Raw log and SHA-256 |
| --- | --- | --- |
| `CARGO_NET_OFFLINE=true cargo test -p arc-storage` | Passed: 70 tests, zero ignored; 54 unit and 16 integration tests. | `target/traffic-storage-repair-tests-final.log` `9a7724b70a4c4bc41da519c340c2a300bc4047d43559536e74ae41b70310b9cd` |
| `cargo fmt --all -- --check` | Passed. | `target/traffic-storage-repair-fmt-final.log` `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `CARGO_NET_OFFLINE=true cargo clippy --workspace --all-targets -- -D warnings` | Passed. | `target/traffic-storage-repair-clippy-final.log` `c8c34c33988b4f1fbcd35a9ca01e2902c0c93fcd4fef4a6891b505ef9e08fda3` |
| `CARGO_NET_OFFLINE=true cargo test --workspace` | Failed in the daemon unit binary: 45 passed, 16 failed due to version-2 expectations. This run preceded the final additional storage-only descendant test; production code was unchanged afterward. | `target/traffic-storage-repair-workspace.log` `69ac4749b9cad3a173a0cd85282300a259f7f659d89de9afe6744b75b97b5917` |
| `git diff --check -- crates/arc-storage` | Passed. | No output. |

The first focused regression run produced the two expected failures before the fix: the retained permit was not released on delayed acknowledgement, and no lineage snapshot existed after root completion. Its `tee` could not create a log because `target/` did not yet exist; its output was observed directly and the command exited 1. The repaired focused run passed 18 tests: `target/traffic-storage-repair-focused.log`, SHA-256 `14f53e19c681fcab40e83e59cc61720a152484f453271aae0fd6e5bee794f067`. The final storage run adds one descendant-cancellation regression.

Skill used: `oma-db`; the database rule, shared execution policy, execution protocol and checklist were read. Desktop, native Windows, vendor hooks, broader detection and remaining scheduling policy remain outside this storage repair.
