# Traffic policy and IPC repair handoff

Status: task-4's reported envelope redaction defect is repaired and its focused
checks pass. This is not evidence that the complete Traffic scheduler is
integrated or that the mission-wide gates have passed. Skill used: `oma-backend`.

## Source scope

The accepted committed baseline is `4d2f71c1a1482925f6c9918296d7ceb3435b7592`,
tree `4455896fdf976244313a70a9fa38b730b344b411`. This repair started at Actor
snapshot and local HEAD `ecf013b675db1c8501d4498db11273cf57f5b0b6`, tree
`8cf0658e06ba807bf9dcc7bb43d1aa7fc2ba6b18`. That snapshot contains the earlier
Traffic policy and IPC delivery; it is not the accepted baseline. The repair
changes and this report are uncommitted in this Actor workspace. The supplied
snapshot and worker refs did not resolve here; the supervising collector must
verify the delivered files in the integrated worktree.

The task-1 architecture input at `.arc-verify/traffic/architecture.md` has
SHA-256 `4bc2a0eeae324e5582c64b7f1b747ac5ab768b6ae46f373bf2589f485aa28c82`.
The previous `implementation-report.md` describes the preceding Actor attempt
and was preserved. Initial Git status excluding `av.yaml` was clean; full status
reported `av.yaml: Operation not permitted`, so the full initial worktree state
was not established. Serena tools were not exposed; the known assigned paths
were read directly.

## Delivered repair and consumer contract

- `crates/arc-ipc/src/lib.rs`: `Request` and `Response` use borrowed recursive
  debug formatting that hides `lineage_context` and classification `command`
  fields, including in nested objects and lists. Other diagnostic fields remain
  visible. Serialization and deserialization are unchanged.
- `crates/arc-ipc/tests/traffic.rs`: complete typed ENQUEUE and GRANTED envelopes
  are checked with both normal and alternate `Debug`; actual framed round trips
  retain the token. A separate case covers nested values and command redaction.
- `crates/arc-traffic/README.md`: documents the envelope redaction and unchanged
  wire behavior.
- `crates/arc-traffic/repair-report.md`: this handoff and the associated checks.

At this snapshot the workspace and lockfile already contain `arc-traffic`, and
the policy exposes `decide`, `Policy`, `Active`, `Decision`, reservations and
wait/bypass reasons. Its focused tests exercise class windows, shared CPU and
memory, FIFO and remote/UNKNOWN bypass. Storage must obtain FIFO head and active
reservations in the grant transaction. The daemon must independently verify
peer identity and ancestry before reusing a lineage, and must advertise
`traffic.v1` only after implementing the complete extension. Existing protocol-1
lifecycle shapes and the capability probe remain unchanged. This repair does not
implement the durable queue, daemon handlers, or CLI scheduling.

## Task acceptance

- [x] The Actor snapshot's `arc-traffic` crate, workspace membership, lockfile
  and typed interfaces compile in the focused and workspace test runs.
- [x] Focused policy tests cover class windows, shared CPU and memory, FIFO and
  bypass. IPC tests cover the legacy lifecycle shape and capability negotiation.
- [x] The README records the storage and daemon integration points and the
  assumptions that must hold before advertising the capability.
- [x] Complete ENQUEUE and GRANTED envelope debug output redacts the lineage
  context while framed serialization retains it.

The uncommitted repair files inspected here have these SHA-256 values:

| Path | SHA-256 |
| --- | --- |
| `crates/arc-ipc/src/lib.rs` | `9944843d6d56e22110e4a8f94b99882943f47a10b759d789578673a39056c4ee` |
| `crates/arc-ipc/tests/traffic.rs` | `725910315587ad27edf2933c5fd9bd2bf65993bc8f51dc1ff687885325f7f260` |
| `crates/arc-traffic/README.md` | `0ccb3df50d4455733f5b4be72c82f68b03f3b7e1c2ee47e80f2fb3e9dc43122f` |

## Checks in this Actor workspace

Cargo ran on Darwin with `CARGO_NET_OFFLINE=true` and `CARGO_BUILD_JOBS=2`.
The raw logs below are ignored Actor-local files under
`target/traffic-task-4-repair/`; their hashes do not establish their presence in
the integrated workspace.

| Command or check | Outcome | Log SHA-256 |
| --- | --- | --- |
| `cargo test --offline -p arc-ipc --test traffic` before the repair | Expected failure: both new redaction regressions failed; five prior tests passed. | `red-tests.log`: `f34790d29577c47dc01b179438212f2463bcd89e4e52dc36c031e8ee5adb1711` |
| `cargo test --offline -p arc-traffic -p arc-ipc` | Passed: 49 tests, none ignored. | `focused-final.log`: `b477a8b105134044231a859afb626f6fdcaba6b6427f0ba2510405c54fa3f36b` |
| `cargo fmt --all -- --check` | Passed after correcting formatting in the new test. | `fmt-final.log`: `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed. | `clippy.log`: `cbd6f12c9dba3a1d83cdcd0f859d2227838032ed71af095516c4332dfb2da9d4` |
| `cargo test --workspace`, first run | Failed on the existing daemon cancellation fixture described below. | `workspace-tests.log`: `da41b576130fd4839e5024b553d931ab0a8b23e1e6721db977e82af087a96629` |
| Exact isolated daemon cancellation test | Passed once and in 20 further isolated runs. | `daemon-repro.log`: `0a6a41944d37eda5c7e5e6dadb567b3ed879b410c0acb32e74088f5a5c3821b5`; `daemon-repro-repeat.log`: `1fb882aba36cea10554fe2d1acc1849402b785136f2feb293be384fb56a308bb` |
| `cargo test --workspace`, retry | Passed: 424 summed tests in 36 result blocks, none failed or ignored. | `workspace-tests-retry.log`: `6aea49cff696baa2b4f2fd275f9fd2ffbdaea88308d07bdd587803abd0f7b862` |
| `cargo llvm-cov -p arc-ipc -p arc-traffic -p arc-detector --tests --offline --locked --no-cfg-coverage --no-cfg-coverage-nightly --fail-under-lines 80` | Passed its aggregate check: 85.52% lines. `arc-ipc/src/lib.rs` was 204/215 lines and `arc-traffic/src/lib.rs` was 113/113. This is not the full per-crate coverage gate. | `focused-coverage.log`: `d4bc53ac08bd48c8adb0618f8bdff16d15341f11077810d3f867da9401a580cc` |

## Daemon test failure for Chief coordination

The first workspace run failed at `crates/arc-daemon/src/db_worker/tests.rs:296`:
`release.try_send(()).unwrap()` received `TrySendError::Disconnected`. The
same test subsequently passed in isolation, in 20 repetitions, and in the
workspace retry. The daemon test and `db_worker/execution.rs` are unchanged
between the accepted baseline and this Actor snapshot.

The test drops the ticket immediately before sending the release. The worker's
`Hooks::before_sql` checks cancellation in its loop, so it can return and drop
the release receiver before that send. This is a possible race consistent with
the observed error; the failing run did not record the actual interleaving.
The daemon owner should make this fixture handle the cancellation-completed
case while retaining the recovery and no-SQL-progress assertions. No daemon
files were changed by this repair.

## Remaining mission work

Durable queue and permit transitions, direct-execution integration, real macOS
two-directory Cargo measurements, a live full coverage gate, full no-skip
harness, target inventory, and independent QA remain for the assigned Actors
and Chief. The passing workspace run covers the snapshot's existing execution
tests; it is not the requested Traffic concurrency experiment. Desktop, native
Windows, vendor hooks, broader detection and remaining scheduling policy remain
outstanding.
