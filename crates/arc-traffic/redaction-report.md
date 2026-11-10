# Task-4 IPC redaction repair handoff

Status: completed for the shared policy and contract task. The Traffic scheduler
is not integrated in this Actor snapshot. Skill used: `oma-backend`.

## Source provenance

The accepted committed baseline is `4d2f71c1a1482925f6c9918296d7ceb3435b7592`,
tree `4455896fdf976244313a70a9fa38b730b344b411`. The Actor-local starting HEAD
is snapshot `c62802faa2774c521efc828c3c8781c1f87a8ba4`, tree
`c55914e11bdc73e117808d785da0064f16d7cb32`. Its tree contains previous policy,
IPC, and verification deliveries that are absent from the accepted baseline.
The repair changes listed below are uncommitted in this Actor worktree and
must be checked in the integrated worktree. Both supplied task evidence refs were
unavailable in this checkout; their resolution at collection is the collector's
responsibility. No other checkout's HEAD was substituted.

The task-1 architecture input `.arc-verify/traffic/architecture.md` matched
SHA-256 `4bc2a0eeae324e5582c64b7f1b747ac5ab768b6ae46f373bf2589f485aa28c82`.
Initial status excluding `av.yaml` was clean; full status printed
`av.yaml: Operation not permitted`, so the complete worktree status could not
be established. Serena tools were not exposed in the available tool inventory;
the assigned source files and known IPC module paths were read directly.

## Repair and compatibility

`crates/arc-ipc/src/lib.rs` now formats `Request` as its ID, operation, and a
fixed payload marker; `Response` formats its ID, status, and a fixed result
marker. The formatter never receives the JSON value, and does not format the
error body. Serialization, deserialization, and all wire fields remain
unchanged. The inspected IPC modules contain these two untyped JSON envelopes.

`crates/arc-ipc/tests/traffic.rs` tests each envelope's placeholder and absence
of payload text. It also checks framed ENQUEUE and GRANTED round trips retain
`lineage_context`; legacy lifecycle and capability-probe tests remain in the
same suite. `crates/arc-traffic/README.md` documents the new Debug contract.

The uncommitted repair files had these SHA-256 values after the checks:

| Path | SHA-256 |
| --- | --- |
| `crates/arc-ipc/src/lib.rs` | `17ff544a163bf7c831b8f3c84092c03047eb01498a9696c2004fe888229bdf46` |
| `crates/arc-ipc/tests/traffic.rs` | `9450b00c1c9d6ace4786573852a0a5ba6278e767ec8429f8954a5ba4b530b093` |
| `crates/arc-traffic/README.md` | `3adc7b5fe063fd450f3ee7c8633ec981ccc3d1b124b2d834adb01eaa61ff3c02` |

## Task acceptance

- [x] The snapshot's `arc-traffic` crate, workspace membership, lockfile and
  typed interfaces compile in focused and workspace tests.
- [x] Policy tests cover independent class windows, shared CPU and memory
  budgets, FIFO decisions, and remote/UNKNOWN bypass. IPC tests cover protocol-1
  lifecycle shape, explicit capability negotiation, and wire round trips.
- [x] `crates/arc-traffic/README.md` states storage and daemon integration
  points, including atomic FIFO selection and reservation, authenticated peer
  and ancestry verification, and the conditions for capability advertisement.
- [x] Request and Response Debug use a fixed placeholder, with a regression
  test for each and a separate lineage-context wire test.

## Checks

Checks ran locally on Darwin with `CARGO_BUILD_JOBS=2` and
`CARGO_NET_OFFLINE=true` for Cargo test and Clippy commands. Raw logs are local
ignored files under `target/traffic-task-4-redaction-20261110/`; their hashes
do not prove that the files will be present in the integrated workspace.

| Command | Outcome | Raw log SHA-256 |
| --- | --- | --- |
| `cargo test --offline --locked -p arc-ipc --test traffic` before implementation | Expected RED: the two new redaction tests failed; the wire test and five other tests passed. | `red.log`: `207bb8cf5bb07edc4014d9e3df01737a98ce5657ea7ae9d3e8118373cb7e3091` |
| `cargo test --offline --locked -p arc-ipc -p arc-traffic` | Passed: 50 tests, none failed or ignored. | `focused-final.log`: `46c85657d0f38e5c5989f97292c9c027a7fe426e772cea5f3f67d82f109894cc` |
| `cargo fmt --all -- --check` | Passed. | `fmt-final.log`: `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed. | `clippy.log`: `f4a733c99f13b7acc8ee376639060ce897c8d87e787f1ce29761d06a3618c63c` |
| `cargo test --workspace` | Passed: 425 summed successes across 36 result blocks, none failed or ignored. | `workspace.log`: `6696d3c35675744d80a115158be826d309554216631259f378adc42c57bba4fa` |
| `git diff --check -- . ':(exclude)av.yaml'` | Passed. | No log. |

The first focused GREEN run preceded a test-only strengthening; the reported
focused result and both workspace-wide checks were run after that change.

## Integration handoff and remaining work

Storage must make FIFO head selection, active resource totals, grant, and
release atomic and idempotent. The daemon must validate requests, authenticate
the peer and verify native ancestry before lineage reuse; it must advertise
`traffic.v1` only after the complete extension is available. The CLI remains
the sole child launcher, and capability negotiation preserves legacy protocol-1
operations. No changes outside task ownership were made in this repair.

Durable scheduling, the macOS two-directory acceptance experiment, integrated
coverage and full harness, configured target health, and independent QA remain
for the assigned Actors and Chief. This workspace test run is not proof of those
mission requirements. The previous Actor's `repair-report.md` records a daemon
cancellation fixture race; this run passed, but did not establish that the
fixture race was fixed. Desktop, native Windows, vendor hooks, broader detection,
and remaining scheduling policy are outside this slice.
