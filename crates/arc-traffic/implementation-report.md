# Traffic policy handoff

Status: task-4 implemented and locally checked. The complete Traffic mission is
not yet implemented or verified. Skill used: `oma-backend`.

## Source scope

The accepted committed baseline is `4d2f71c1a1482925f6c9918296d7ceb3435b7592`,
tree `4455896fdf976244313a70a9fa38b730b344b411`. This actor checked out snapshot
`d60709fa026357f9b64a6fa297c1508511effbfe`, tree
`004408dcbd3195a93b5dcd9317bf70fcff233602`. The task changes below are
uncommitted in that actor worktree; they are not claimed to be in the accepted
baseline or the snapshot commit. The supplied snapshot and worker refs did not
resolve in this checkout when inspected. Recheck the delivery receipt and
integrated files after collection.

The input `.arc-verify/traffic/architecture.md` in this snapshot has SHA-256
`4bc2a0eeae324e5582c64b7f1b747ac5ab768b6ae46f373bf2589f485aa28c82`.
Git status and diff excluding `av.yaml` were initially empty. Full status and
diff reported `av.yaml: Operation not permitted`, so the complete initial
worktree state could not be established; that file was not modified.

## Delivered interfaces

- `Cargo.toml`, `Cargo.lock`, `crates/arc-traffic/Cargo.toml`,
  `crates/arc-ipc/Cargo.toml`, `crates/arc-storage/Cargo.toml` and
  `crates/arc-daemon/Cargo.toml`: workspace membership and local dependency
  wiring. No new third-party dependency was added.
- `crates/arc-traffic/src/lib.rs`, `crates/arc-traffic/tests/policy.rs` and
  `crates/arc-traffic/README.md`: pure admission decision, validated class
  windows and integer reservations, deterministic tests and integration contract.
- `crates/arc-ipc/src/lib.rs`, `crates/arc-ipc/src/traffic.rs` and
  `crates/arc-ipc/tests/traffic.rs`: additive typed protocol-1 capability and
  Traffic job contracts, validated payloads and results, redacted context debug,
  and legacy-shape/probe tests.
- `crates/arc-traffic/implementation-report.md`: this handoff.

Acceptance for this task is covered by the focused policy and IPC tests, the
workspace test run and the integration contract in the README. The existing
legacy dispatcher still returns `UNSUPPORTED_CAPABILITY` for the new probe.
The new daemon must only advertise it after the entire implementation exists.
The tests do not claim that an actual new-daemon/old-client pairing, durable
queue, native lineage, or CLI scheduling already works.

The effective defaults are one slot per class; build reserves 2,000 millicores
and 2 GiB, test and lint each reserve 1,000 millicores and 1 GiB, and the shared
budget is 2,000 millicores and 4 GiB. The storage worker must derive the FIFO
head and `Active` snapshot in the same transaction as the permit grant and must
persist the permit generation used by attach, completion and cancellation.
The daemon must authenticate and verify peer identity and ancestry; syntactic
context validity does not authorize reuse. The CLI must classify the exact
native argv and make one spawn decision. Full recovery and compatibility duties
are recorded in `README.md` for the consumers.

Selected SHA-256 values of the uncommitted actor files at handoff:

| Path | SHA-256 |
| --- | --- |
| `Cargo.toml` | `79eaf16e2fa7d5263b974a6980012115f1c6b11906e1f785dd52fe8c549a3380` |
| `Cargo.lock` | `424e3a77d5c517f2616655c4c1a2c4ce1e2485ce4be2117020271ab92887dd7e` |
| `crates/arc-traffic/src/lib.rs` | `9146c289b25c4d1760132922db02dafdfbd2bcc5bfb561a6e522973ea5b70d9b` |
| `crates/arc-traffic/tests/policy.rs` | `c5ddf00dfcc3553da026d32789af76883ee405adefb3a73673d9fa8f513ecd32` |
| `crates/arc-traffic/README.md` | `68593b27c39b64d78e710c3be8a01694c3c031d3031c28326ffa9b128c4e1422` |
| `crates/arc-ipc/src/traffic.rs` | `8ccf7c4d18ab72d2d6caa3f4b76f993b285966a8237a94aeebe72fd94982f127` |
| `crates/arc-ipc/tests/traffic.rs` | `99aa2e897757a1f4ea4e5e28dd5c62fb44c621d4987abf35bfdca4ee3edb7c13` |

## Checks in this actor worktree

All Cargo commands below used `CARGO_NET_OFFLINE=true` and
`CARGO_BUILD_JOBS=2`. Raw logs are actor-local, ignored files under
`target/traffic-task-4/`; their hashes bind the observations but do not imply
that the files will be delivered to another workspace.

| Command | Result | Raw log SHA-256 |
| --- | --- | --- |
| `cargo test --offline -p arc-traffic -p arc-ipc` | Passed: 47 tests. | `focused-tests.log`: `da07cd655a1895a549d9801ed9912095167494e6b7ad915ce4ae720098aefc49` |
| `cargo fmt --all -- --check` | Passed. | `fmt.log`: `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed. | `clippy.log`: `2a248bb2842865011641499ab0308811f4d47ea6ffbdedda556d260074b9f8c1` |
| `cargo test --workspace` | Passed: 422 summed test successes, zero failures or ignored tests across 36 result blocks. | `workspace-tests.log`: `07556a683c73ff620993cb0f8a58319ad2bbdb1cc2d12d9d9eb7588b31f05906` |
| `cargo llvm-cov -p arc-traffic -p arc-detector --tests --offline --locked --no-cfg-coverage --no-cfg-coverage-nightly --fail-under-lines 80` | Passed. The Traffic production file measured 113/113 lines in this focused run. | `policy-detector-coverage.log`: `9ed3f6783dea52a6da7ea1ce04e5d833266f69cdaad5218341ddc102d8f2a374` |
| `cargo llvm-cov -p arc-ipc -p arc-traffic -p arc-detector --tests --offline --locked --no-cfg-coverage --no-cfg-coverage-nightly --fail-under-lines 80` | Passed overall; the new IPC Traffic file measured 147/150 lines. This is not the full per-crate coverage gate. | `ipc-coverage.log`: `8daa53a64463bb50aa811d654e0fd925c964f2cdb314ad54225d463f1120f815` |

`git diff --check -- . ':(exclude)av.yaml'` passed. Two earlier focused
coverage commands failed their aggregate 80% threshold because they executed
only Traffic tests while also reporting the detector. The attempted
`--exclude-from-report arc-detector` did not remove those detector files. Both
Traffic test runs passed and each reported 113/113 Traffic lines; the later
combined run exercised both crates. Their logs are `policy-coverage.log`
(`9cd217ca9d58b97dc0fc8daf5aa0d0528c8f9e016685fffb028e4afeb2c7fde2`)
and `policy-only-coverage.log`
(`8636552b6f1e918f70b628674fadcb8ac903502f0f3a61a1ccbc738adb888e3b`).

## Remaining work and coordination

The storage and daemon/CLI consumers must implement and test durable FIFO,
atomic reservation/release, stale and duplicate requests, cancellation,
fallback reconciliation, restart and verified nested ancestry. The macOS
two-directory Cargo experiment, remote/unknown bypass under a full queue,
integrated compatibility tests, full seven-crate coverage gate, no-skip harness,
live evidence check, configured container target checks and independent oma-qa
remain for the Chief and assigned actors. Desktop, native Windows, vendor
hooks, broader detection and remaining scheduling policy remain outside this
slice. A `ps` diagnostic attempt was denied by the environment; no GUI or
privilege workaround was used.

For the verification owner: in this snapshot `scripts/coverage/run.ts` passes a
3,600,000 ms timeout to its workspace coverage child and
`scripts/coverage/commands.ts` terminates the child at that timeout. The mission
specifies a 7,200,000 ms framework timeout for verification-contract commands;
assess the collector's inner timeout when coordinating the final checks without
changing the operator's verification commands.
