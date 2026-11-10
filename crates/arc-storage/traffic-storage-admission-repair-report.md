# Traffic storage admission repair

Status: the assigned storage repair is implemented and its storage acceptance checks pass. Daemon integration and mission-wide verification remain open.

## Source and scope

The accepted committed baseline is `4d2f71c1a1482925f6c9918296d7ceb3435b7592`, tree `4455896fdf976244313a70a9fa38b730b344b411`. This Actor's inspected initial snapshot and local HEAD are `bdaefc12aac20874e14ae5d94c643be0a82fa6ab`, tree `5e7a6eae625def95da07ae2e3e12ab648ee4935d`. That snapshot already contains the earlier Traffic storage and dependency deliverables. The changes described here are uncommitted differences from that snapshot, not changes claimed in the accepted baseline. The supplied snapshot and worker refs did not resolve in this Actor checkout. Full Git status reported `av.yaml: Operation not permitted`; status excluding that path showed only the listed storage changes. Serena was unavailable in the tool inventory, so inspection used known paths.

Changed in this attempt:

- `crates/arc-storage/src/jobs/traffic/admission.rs`
- `crates/arc-storage/src/jobs/traffic/tests.rs`
- `crates/arc-storage/src/jobs/traffic/tests/admission.rs`
- `crates/arc-storage/src/jobs/traffic/tests/lineage.rs`
- `crates/arc-storage/src/jobs/traffic/tests/identity.rs` (new and untracked in this Actor checkout)
- `crates/arc-storage/traffic-storage-admission-repair-report.md` (this report)

No migration, manifest, IPC, daemon or CLI file was changed in this attempt. Preserve the earlier storage handoffs and their unresolved integration notes.

## Behavior and transaction boundary

Both enqueue and poll now route queued decisions through one admission function. It verifies that the owner observation matches the stored complete identity and is `VerifiedMatch` before returning a queued decision or granting. Missing, mismatched and indeterminate observations cannot create a permit. An already committed grant or terminal result remains an idempotent reply. A failed queued request does not automatically delete the entry: the existing `queue_head` and `reconcile_queued` flow can abandon a confirmed missing or mismatched owner; indeterminate evidence leaves it queued.

A queued enqueue retry can supply a newly verified lineage proof. The same SQLite `IMMEDIATE` transaction checks the persisted inherited context, active root permit, anchor identity, revision and generation, then changes the existing queue entry to a reused reservation and claims the job. It does not create another permit. A stale or forged proof rolls back; retries of an already reserved descendant return the existing result. The root permit stays active until the root and all recorded descendants finish.

The SQLite writer serializes these read/decision/write transactions; its commit or rollback is the atomic durability boundary. Existing uniqueness, referential and state constraints remain unchanged. Process launch, authenticated peer checks and native ancestry verification remain application responsibilities. Native owner evidence is an observation made before the transaction; the caller must gather it for each request and bound the delay before the writer consumes it. An owner can still exit after a successful OS probe, and these storage tests do not establish native ancestry or socket-peer authentication.

## Required handoff to the daemon/CLI owner

1. The Rust storage API is source-incompatible: `traffic::poll` now requires `&OwnerEvidence` immediately after `&JobRequest`. On every poll and enqueue, authenticate the socket peer, gather fresh evidence for the claimed observer outside the writer and pass it to storage. Run queue-head reconciliation when a queued owner is confirmed missing or mismatched. No protocol-1 wire field changed.
2. `JobRequest::Poll` does not carry an inherited context or lineage proof. If ancestry becomes verifiable after a child has queued, repeat the original `Enqueue` with the same job ID, observer, classification and inherited context, supplying a fresh daemon-verified `LineageProof`; a context alone must not grant reuse. If integration requires a different route, coordinate an interface change with the IPC owner.
3. The earlier `confirm_cancel_unspawned` handoff remains: the current IPC cancel request and retained result do not themselves express an explicit no-spawn confirmation after a permanently lost grant. Coordinate a compatible operation before invoking that storage function, and serialize it against the CLI spawn decision. Do not infer no-spawn from an unknown generation.
4. The final workspace test below reproduces the schema-version expectation conflict in daemon-owned paths: `crates/arc-daemon/src/db_worker/tests.rs:35` and `:219`, `crates/arc-daemon/src/database_failure_tests.rs:351`, and `crates/arc-daemon/src/server_tests.rs:82`. Migration 3 returns version 3 while those tests expect 2. The earlier handoff also identifies `crates/arc-daemon/tests/support/runtime.rs` for the same integration review. Fix under the daemon owner's assignment and rerun the workspace checks.

## Acceptance evidence

- [x] The new lineage regression queues a child first, rejects forged and stale proofs and a missing owner, accepts a fresh verified proof without a second permit, reopens the database, checks an idempotent retry, records queue wait, and releases only after the descendant completes.
- [x] The new owner regression covers missing, PID/start-token mismatch and indeterminate observations on both enqueue and poll, including after reopen. It verifies zero phantom active permits, rejects a different identity, exercises stale-head reconciliation, and permits later live work to proceed.
- [x] Existing storage tests exercise FIFO, concurrent capacity accounting, bypass, duplicate transitions, cancellation, spawn failure, lost acknowledgements and restart. Existing migration tests exercise version-2 upgrade, preserved rows and both historical checksums, reopen, DDL rollback and checksum rejection. The complete storage suite passed 72 tests with none ignored.
- [x] Migration bytes match the accepted baseline: `0001_phase0.sql` SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`; `0002_managed_exec.sql` SHA-256 `464a1c88270f01a56ccab6610f87eff7648e0640f276ebce53b8a97026c50814`. Existing `0003_traffic.sql` SHA-256 is `1f4522e3f27c124aea7127d92434c7bd22be36168fef0a4bf665f8cf16debe90`.
- [ ] Integration with native ancestry, CLI execution and daemon polling, mission-wide coverage, the full harness and independent QA remain for their assigned owners. No coverage-floor or macOS concurrency acceptance result is claimed here.

## Commands and raw evidence

The test commands used `CARGO_NET_OFFLINE=true` and `CARGO_BUILD_JOBS=2`. Raw logs are under `target/` and their SHA-256 values follow.

| Command | Result | Raw log SHA-256 |
| --- | --- | --- |
| `cargo test -p arc-storage queued_` before repair | Expected failure: both new regressions reproduced the reported defects. | `target/traffic-storage-admission-before.log`: `6b645878b8ea120d0adee40ea6418e695d0af955374dfdda1e8471b60db982e8` |
| `cargo test -p arc-storage` | Passed: 56 unit and 16 integration tests, none ignored. | `target/traffic-storage-admission-tests-final.log`: `b37b02bb3e9b6ac4065214b2252edce57ca9ddf67fc2cf9fac3f6cf2d6a531f3` |
| `cargo fmt --all -- --check` | Passed. | `target/traffic-storage-admission-fmt-final.log`: `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed. | `target/traffic-storage-admission-clippy-final.log`: `08867d6da4d0566370f5a64e3a4004ed16c798bd2a03dbd5302ddb05e5f85b35` |
| `cargo test --workspace` | Failed in the daemon unit binary: 45 passed and 16 failed on version-2 expectations. The 59 CLI unit/integration tests reached before that binary passed. | `target/traffic-storage-admission-workspace-final.log`: `b08f0d3e944f3ac22edc01628be68e775f58078509017debe728000b2ea23c1f` |
| `git diff --check -- crates/arc-storage` | Passed for tracked changes; the new test is separately covered by formatting and compilation. | No output. |

The storage test log includes the expected panic from the child process used by the interruption/rollback test; its parent test passed. No coverage gate or full harness was run in this attempt: their mission-level run belongs after daemon integration and the separately assigned validator work. Desktop, native Windows, vendor hooks, broader detection and remaining scheduling policy remain outstanding.

Skill used: `oma-db`. The database rule, shared execution policy, database execution protocol and checklist were read.
