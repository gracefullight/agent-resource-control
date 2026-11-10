# Traffic storage recovery handoff

Status: the assigned storage recovery repair is implemented and the storage tests pass. Daemon integration and mission-wide verification remain open.

## Provenance and changed paths

The accepted committed baseline is `4d2f71c1a1482925f6c9918296d7ceb3435b7592`, tree `4455896fdf976244313a70a9fa38b730b344b411`. This actor's initial local HEAD and snapshot commit are `184e81ac724fe50071f3c41d128801f84c9e7552`, tree `904152e6f52dc1ada266f6087ae82055af841e33`. That snapshot includes earlier storage and dependency work; these observations do not attribute that work to the accepted baseline. Both supplied evidence refs failed to resolve in this actor checkout. The changes below are uncommitted differences from the actor snapshot:

- `crates/arc-storage/src/jobs/reconcile.rs`
- `crates/arc-storage/src/jobs/traffic.rs`
- `crates/arc-storage/src/jobs/traffic/lifecycle.rs`
- `crates/arc-storage/src/jobs/traffic/tests/recovery.rs`
- `crates/arc-storage/traffic-storage-recovery-report.md` (this report)

SHA-256 of the four source and test files in the same order: `939531ae70bfe1137ace20aea0b1e5a637edf3a2a11e6b96eade4a249ee8c756`, `745ed62a1f251055fbde0388d2f03c67c8e3e611589f3e0c98d1f285101f6d05`, `93c66fbf7a8510144864bed24d3a1bddd2d33f772adcf651f0046728d60a8b4f`, and `bd21f68a9c74f2338f2deb3a5ce7a525e5cde679fd3f59788a59dfd42a2aba59`. The report is untracked in this actor checkout.

Git status excluding `av.yaml` was clean before this repair. Full status printed `av.yaml: Operation not permitted`, so the full worktree status was not established; the file was not changed. Serena was not present in the available tool inventory, and inspection used the supplied known paths. No manifest, IPC, daemon, CLI or migration file was modified.

## Recovery behavior

`jobs::observe` gathers the complete observer and attached-child identities outside the SQLite writer. `jobs::apply_observation` rechecks the stored tuples and lifecycle revision under an SQLite `IMMEDIATE` transaction. When both the observer and the previously verified attached child are missing or have mismatched identities, that transaction records `OUTCOME_UNKNOWN` with no exit code, marks the Traffic entry `ABANDONED`, and releases the root permit only when every recorded lineage member is terminal. A live or indeterminate observer or child, and an uncertain grant with no verified attachment, retain the reservation. A stale observation cannot replace a committed outcome.

The lifecycle update, Traffic transition, and conditional permit release commit or roll back together. Job and permit uniqueness, the existing FIFO sequence, and resource accounting are unchanged. A late completion or fallback request from the authenticated owner can refine a recovered unknown outcome without reacquiring the old permit or releasing a newer job's permit. The daemon remains responsible for authenticating that request and for process launch; SQLite does not replay a child.

## Required daemon handoff

1. `traffic::recovery_candidates(connection, after_sequence, limit)` returns a bounded page of attached active lineage members. On restart and while admission is waiting, the daemon must walk all pages and gather `jobs::observe` outside the database worker. Send observations for which `traffic_recovery_ready()` is true back through the single writer to `jobs::apply_observation`; skip the others to avoid churning live jobs' lifecycle revisions. Repeat from cursor zero on a later pass; a single page is insufficient when more than 64 entries exist. Storage only performs recovery when that observation is applied, so this actor's storage test alone does not establish automatic daemon recovery.
2. Traffic protocol 1 currently has an `ABANDONED` state but no distinct unknown-outcome state. The existing lifecycle snapshot is `OUTCOME_UNKNOWN` with `outcome: None`; use that snapshot when a consumer needs to distinguish a recovered child from an unstarted abandoned queue entry. The wire contracts and existing storage function signatures remain unchanged; `recovery_candidates` and `traffic_recovery_ready` are additive Rust APIs.
3. Previous handoffs about `confirm_cancel_unspawned`, authenticated no-spawn confirmation, and `lineage_snapshots` / native ancestry verification still apply. A grant without verified attachment remains reserved until reliable evidence or explicit authenticated no-spawn confirmation resolves it.
4. The final workspace run failed in daemon-owned schema expectations. `crates/arc-daemon/src/db_worker/tests.rs:35` and `:219`, `crates/arc-daemon/src/database_failure_tests.rs:351`, and `crates/arc-daemon/src/server_tests.rs:82` expect schema version 2 while migration 3 reports version 3. The earlier handoff also identified `crates/arc-daemon/tests/support/runtime.rs` for review. Coordinate these changes with the daemon owner, then rerun the integrated gates.
5. A preceding workspace run failed in `crates/arc-cli/src/exec/signals/tests.rs:450`: `shared_job_terminal_signals_between_spawn_and_attach_arrive_once` did not observe `signals.interrupted()`. The same test passed in the next workspace run, which then reached the daemon failures above. The cause was not established; the CLI owner should assess this intermittent observation.

## Acceptance evidence

- [x] Storage tests cover FIFO and resource reservations, duplicate and failure transitions, owner loss, live and indeterminate processes, PID mismatch, repeated and stale recovery, reopen, lineage retention, admission after release, and rollback of the whole recovery transaction. The native macOS fixture uses two real processes and checks that a surviving child retains its permit; identity and lineage edge cases also use explicit simulated observations.
- [x] Existing migration tests pass for version-2-to-3 upgrade, row and checksum preservation, reopen, failed-migration rollback, and checksum rejection. Compared to the accepted baseline, migration 0001 remains SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384` and migration 0002 remains `464a1c88270f01a56ccab6610f87eff7648e0640f276ebce53b8a97026c50814`. Existing 0003 remains `1f4522e3f27c124aea7127d92434c7bd22be36168fef0a4bf665f8cf16debe90`.
- [x] The unchanged IPC interface and the additive Rust integration requirement are described above.
- [ ] Automatic daemon recovery, native ancestry integration, CLI acceptance, coverage, full harness and independent QA require the later integrated work.

## Commands and raw evidence

Tests ran on Darwin arm64 with `CARGO_NET_OFFLINE=true` and `CARGO_BUILD_JOBS=2`. Raw logs are under `target/`.

| Command | Outcome | Log and SHA-256 |
| --- | --- | --- |
| `cargo test -p arc-storage lost_owner_and_exited_attached_child_release_on_observation_after_reopen` before repair | Failed as expected: one permit remained active after owner and child exit. | `target/traffic-storage-recovery-before.log` `c61275fd98a00b3f01cdf0cd84f7cd593840b92196e7ff12431ceb5fab15ea8f` |
| `cargo test -p arc-storage late_` after the first recovery change | Failed as expected: a late observed completion was rejected or returned abandoned; both cases were repaired. | `target/traffic-storage-recovery-late-before.log` `27326752896cc794372f1dfb8bfb545fe599a11364956db1893567b1195cf38d` |
| `cargo test -p arc-storage` | Passed: 66 unit and 16 integration tests, none ignored. | `target/traffic-storage-recovery-storage-tests-final.log` `4dfa924a389adcd522ff7c61dac4e7972353d78df84ad39a43d91f1202004df2` |
| `cargo fmt --all -- --check` | Passed. | `target/traffic-storage-recovery-fmt-final.log` `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed. | `target/traffic-storage-recovery-clippy-final.log` `f10d416b84a3f11b255c980b6f1de0598f8069aa46f2628114d1632d10ff68ff` |
| `cargo test --workspace` | An initial run failed in one CLI signal test; that test passed on the next run, which failed with 45 daemon tests passed and 16 schema-version failures. Later workspace binaries were not run. | `target/traffic-storage-recovery-workspace-signal-failure.log` `e4f6f890ea371d421620dbe0251b1a17ba80cc8fa57d4645b7169d59c9bea171`; `target/traffic-storage-recovery-workspace-final.log` `a440eadb8c1e352547e7bbc97dba85385377d7b35c4e103bebd8da4b16a6f6d0` |
| `git diff --check -- crates/arc-storage` | Passed for the tracked changes. | No output. |

Skill used: `oma-db`; the database rule, shared execution policy, execution protocol and checklist were read. Desktop, native Windows, vendor hooks, broader detection and remaining scheduling policy remain outside this repair.
