# Task 2, attempt 9: Cargo witness continuity repair

Status: the operator's final task-2 repair and its focused checks passed in this Actor worktree. Skill used: `oma-backend`. Integrated measurement, independent `oma-qa`, workspace-wide gates, coverage and the full harness remain assigned downstream.

## Source identity and scope

- Accepted committed baseline: `16772fe2aa350413225f3fb2bb9227d14ae5a6c4`, tree `1834e0cccb50a12fea299618f9a360f4ca10e03f`.
- Actor-local starting and current HEAD: snapshot commit `3c5fb7a7ec70becc8c6e5366efcb1feec6c106d9`, tree `6ef5cc2082861cce73c02b5496748162ab5ff6ad`; its parent is the accepted baseline. The snapshot contains the retained implementation and other input overlays. It is not the accepted baseline. The three source/test changes below are working-tree edits relative to this snapshot. The supplied snapshot and worker refs did not resolve in this checkout's ref namespace; no collection-time worker revision is inferred.
- Only these owned paths and this report were changed. No migration, manifest, lockfile, measurement-owned path, mission instruction, graph or Actor assignment was edited. Full `git status` still prints `av.yaml: Operation not permitted`; scoped status and diff excluding that path do not establish the whole worktree state. Serena tools were unavailable, so named source paths were read directly.
- Host: Darwin arm64, Cargo/Rust 1.95.0 and Bun 1.4.4, 16 GiB RAM and 8 CPUs, with 72 GiB available at the initial disk check. Checks ran serially with `CARGO_BUILD_JOBS=4`; none was given a timeout shorter than the mission's 7,200,000 ms per-check allowance.

## Behavior and compatibility

Cargo lifecycle requests now record the same authenticated peer continuity as Bun requests. Cargo attachment, cancellation and completion require confirmed continuity before treating a reply as proof of original-store cleanup. An uncertain Cargo fallback may remove its witness only when the reply is terminal and continuity is confirmed. A bypass or not-found response after a possibly committed Cargo request remains uncertain. Bun's existing conservative rule for uncertain fallback is unchanged.

Continuity is based on the authenticated daemon process identity, not a persistent database identity. A changed or indeterminate peer therefore cannot prove that it uses the original store, including after a restart against the same database. In that case the CLI retains the witness for recovery; it may report unconfirmed tracking even if the replacement eventually used the same database. Protocol-1 request and status shapes, native arguments and execution ownership were not changed by this repair.

The deterministic two-store regression uses a fault peer that checks the actual wrapper PID and commits the real `CARGO_TEST` enqueue through storage, then drops the acknowledgement. The wrapper enters fail-open while the socket is absent. A separately launched current daemon then serves an empty replacement store. The test verifies the replacement's `ABANDONED` tombstone, the original active permit and retained witness, one observed native Cargo launch, one successful real Cargo test-body execution, and release of the original permit after its daemon is restored. The initial fault peer is a labeled test fixture, not an accepted old daemon.

The retained mixed-version tests use the explicitly labeled frozen old-client fixture and accepted-style claim-lookup emulation documented in `task2-attempt7.md`; they are not a run of the accepted old daemon binary. The caller-supplied lineage limitation recorded there still applies to the combined old-daemon/caller-value case. The operator accepts the specified separate compatibility cases; documentation and QA should not claim universal old-peer permit reuse from these results.

## Acceptance

- [x] Cargo records peer continuity and checks it on cancellation, attachment, completion and uncertain fallback cleanup.
- [x] The real Cargo two-store lost-enqueue test demonstrates witness retention, successful single execution and eventual original-store permit recovery.
- [x] Fresh focused diagnostics, recovery, semantics, status compatibility, CLI, detector, Traffic policy and IPC checks passed. The restart fixture still waits for both the durable `QUEUED` state and visible notice before checking that the child has not started.
- [x] Existing migration bytes match the accepted committed baseline. No owned-path conflict was encountered.
- [ ] Integrated four-case macOS measurement, independent review, full workspace verification, 80% per-crate coverage and full harness remain downstream; this report does not claim their result.

## Checks and raw log hashes

All passing commands below exited 0. Raw logs are Actor-local ignored files under `target/traffic-bun/backend-attempt9/`, named below with SHA-256. They must not be treated as integrated-worktree receipts.

| Command | Result | Log and SHA-256 |
| --- | --- | --- |
| `cargo test -p arc-cli --test binary` before and after repair | 4 passed each | `cli-binary.log` `ef98a412d339017bf6d2f65f4c15119fc080577bc2597be76dce4a841ce69bd6`; `cli-binary-green.log` `c52af21e086bbe9823a32a29f5f03524d7221594a5da102188a2f4003637ccbe` |
| `cargo test -p arc-daemon --test traffic_cli lost_cargo_enqueue_then_replacement_fallback_keeps_original_witness -- --nocapture` before repair | First attempt failed because the test queried `traffic_entries` instead of `traffic_tombstones`; after correcting the fixture, the test failed at the intended missing-witness assertion | `cargo-two-store-red.log` `022712afd1270b03cd11974345c72a2053bb2a9bccb23dba9d1d919decea24b1`; `cargo-two-store-red2.log` `4bb624b6e0c4841a6c9488c63106eaec861a7baba3bc02babcf844323078bd41` |
| Same focused command after repair | 1 passed | `cargo-two-store-green.log` `db999eb0943e9d3cf573b06fd241297f534c78fc4de207862df023af03eedb3e` |
| `cargo test -p arc-daemon --test traffic_cli diagnostics:: -- --test-threads=1` | 12 passed, including the new test after final formatting | `diagnostics.log` `350d7d929d88ce4e3046193ea1f9805000f7f7fa87f9f94483f6e6c8881cd3a7` |
| Same command with `recovery::` | 6 passed | `recovery.log` `b8894b224a5a60bdee000ad5b2582f5bc3019ac7fa0d8241a0824d62d8e284a7` |
| Same command with `semantics::` | 11 passed | `semantics.log` `cdd30357524d0ad7e40e602931bd7edfaf44c303501a9cd28077547a03221ecc` |
| Same command with `status_compat::` | 4 passed | `status_compat.log` `48333e218fc0f004b56508a3f50b56845e7de79daa40bcac9209f1ddd8d38f28` |
| `cargo test -p arc-cli` | All package tests passed, none ignored | `cli.log` `f691f59a8724c72dfd2c4363c6d57ae6606370be6c242ed9a962ceffe53d20a9` |
| `cargo test -p arc-detector -p arc-traffic -p arc-ipc` | All package tests passed, none ignored | `policy-ipc.log` `f5f7803c0018ab02fef676493562bb2834ddf838a4e2ecab2572a9bd2b145cfc` |
| `cargo fmt --all -- --check` | Passed | `fmt.log` `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `cargo clippy -p arc-cli -p arc-daemon --all-targets -- -D warnings` | Passed | `clippy.log` `7c9b523a5ae5918c645ba41eb9cbf08c618c0ea5cd0eb01da3d6ef7b1b6f788e` |

`git diff --check` passed for accessible paths. The migrations matched the accepted commit; SHA-256: `0001_phase0.sql` `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`, `0002_managed_exec.sql` `464a1c88270f01a56ccab6610f87eff7648e0640f276ebce53b8a97026c50814`, and `0003_traffic.sql` `1f4522e3f27c124aea7127d92434c7bd22be36168fef0a4bf665f8cf16debe90`.

## Final Actor-local source hashes

These hashes describe the edited files atop snapshot `3c5fb7a7ec70becc8c6e5366efcb1feec6c106d9`, not files committed in the accepted baseline. The binary diff for exactly these paths against that snapshot has SHA-256 `bbe4ee031bb17b6012857fc54f3aa64e446dc9cf564bf6d1679a5915812414f1`.

```text
ab2bf3ed6a7ba824b78bb1cf19a4c533613edc99071fd6ddb07787649b8164d6  crates/arc-cli/src/exec.rs
8d3efbbc83a817f34d7874fe54e74baeef3129f7c6dbf5bb50629f0cd1525238  crates/arc-cli/src/exec/traffic.rs
d47cae83acef72a8d4a3b586dd211496986679b9810cedb6a64067eea8c13043  crates/arc-daemon/tests/traffic_cli/diagnostics/two_store.rs
```
