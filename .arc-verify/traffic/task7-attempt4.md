# Task 7, attempt 4 handoff

Status: **partial**. The lineage race, by-ID status and orphaned diagnostic-helper defects are repaired and covered by focused tests. The unattached-grant recovery gap still needs a storage interface and its acceptance test. The full harness remains red on a storage-owned file-length violation. Task 9 receipts and independent QA are not complete.

## Changes in this attempt

- `crates/arc-daemon/src/db_worker/traffic.rs`: stale lineage proofs cause a fresh snapshot and native ancestry check, with bounded retries using the same request. By-ID Traffic status first reconciles lifecycle identity and then reads the Traffic state; a retained verified child is reported running, and an unverified running claim is not returned as confirmed running.
- `crates/arc-cli/src/exec/traffic.rs`: a queued invocation with inherited context idempotently resubmits its enqueue, allowing fresh ancestry verification on each wait. Its notice helper arms its own one-second SIGALRM deadline before a potentially blocking write; the wrapper's existing 50 ms wait and kill behavior remains in place.
- `crates/arc-daemon/src/db_worker/traffic_tests.rs`: forces parent attachment after a real process ancestry proof and before the worker transaction, then verifies proof refresh, one permit and idempotent reuse. This is a daemon worker test with native process identities; it supplies the peer PID directly rather than through a socket.
- `crates/arc-daemon/tests/traffic_cli.rs` and `crates/arc-daemon/tests/traffic_cli/{recovery,robustness,status}.rs`: exercise a real queued CLI rechecking lineage, a full notice pipe after wrapper SIGKILL, status recovery after owner and child exit without a new admission, and the status of an attached retained child.

The new CLI behavior uses the already-defined protocol-1 Traffic enqueue and status operations; no shared IPC type, schema, manifest or lockfile was edited in this attempt. An inherited capability still requires native ancestry verification. The storage and validator actors' files were not modified.

## Acceptance evidence

- `cargo test -p arc-daemon --test traffic_cli -- --nocapture --test-threads=1`: **18 passed, none ignored or filtered**. The macOS test used independent Cargo projects and targets, observed peak builds of 2 bypassed and 1 scheduled, one invocation per job, a queued notice before the first release, the second start after release, a 250 ms queue wait, and two `EXITED:0` outcomes. Remote and UNKNOWN fixtures bypassed while the queue was full without changing reservations. This is a test observation, not task 9's final receipt.
- `cargo test -p arc-daemon attachment_between_snapshot_and_transaction_refreshes_lineage_proof -- --nocapture --test-threads=1`: **passed** in all three test targets that include the worker module. `cargo test -p arc-cli --tests`: **passed**. The separate queued-lineage CLI test passed.
- Before the repair, the new status tests failed on both stale-running and retained-running assertions, and the blocked-pipe test failed because the killed wrapper left the helper alive. After the repair, both status tests and the blocked-pipe test passed. The first attempted green blocked-pipe run used the preexisting stale `arcctl` binary and failed; rebuilding the CLI through its requested tests produced the passing run. The first lineage test compilation failed because a result variable shadowed its temporary directory path; the renamed variable compiled and passed.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and scoped `git diff --check -- crates/arc-cli crates/arc-daemon`: **passed**. The Git whitespace check covers tracked paths; Rust formatting covers the untracked Rust files.
- `cargo test --workspace`: **failed twice**, each time with 55/56 `managed_exec` tests passing and macOS `EINVAL` at `crates/arc-daemon/tests/managed_exec/fault.rs:93` while setting a read timeout. The first failure was in `fail_open::outages_preserve_native_bytes_exit17_one_invocation_budget_and_no_replay`; the retry passed that case and failed `fallback_descriptors::fallback_preserves_closed_standard_and_extra_inherited_descriptors` at the same helper. The cause is not established. This helper is hash-pinned by `scripts/harness/exec-fail-open-preservation.ts`; a fix to it needs verification-owner coordination.
- `bun run test:coverage`: **failed**, with the same `EINVAL` in the instrumented outage test, so that invocation produced no valid coverage percentages.
- `./scripts/harness/validate.sh`: **failed** on the 1,088-line storage recovery test, using an isolated index that included untracked source and tests. No checks were skipped. The first three structural checks passed, and its independent instrumented coverage run passed all seven crate floors: arc-core 96.48%, arc-detector 99.84%, arc-storage 95.35%, arc-ipc 98.92%, arc-daemon 92.98%, arc-cli 85.15%, and arc-traffic 100.00%.
- `bun test scripts/coverage/evidence.test.ts`: **1 passed**, validating the successful harness coverage collection and current source identity. `bun test scripts/harness/traffic-targets.test.ts`: **8 passed**, with the saved mission reporting no configured observation policy as not applicable. `bun test scripts/harness/traffic-evidence.test.ts`: **8 fixture tests passed, 1 live test failed** because `.arc-verify/traffic/gate-summary.json` is absent; it is a task 9 deliverable.
- A direct `checkCurrentPreservation(process.cwd())` invocation **failed** on `crates/arc-daemon/src/database_failure_tests.rs`: the current test correctly expects migration version 3, but the validator's immutable-baseline recipe in `scripts/coverage/schema-adaptations.ts` only adapts this daemon test to version 2. The check stopped there, so this run does not establish whether later preservation checks pass.
- Migrations `0001_phase0.sql` and `0002_managed_exec.sql` match accepted HEAD byte-for-byte by SHA-256. The accepted HEAD remains `4d2f71c1a1482925f6c9918296d7ceb3435b7592`.

## Coordination required

1. **Storage and backend:** `crates/arc-storage/src/jobs/traffic.rs:54-76` excludes grants without a verified child, while `crates/arc-cli/src/exec.rs:179-205` and `:233-242` use bounded post-spawn metadata calls. If both calls miss an outage, a surviving or subsequently exited child has no durable identity or terminal evidence in storage. An authenticated durable handoff and storage reconciliation contract must bridge that gap before the backend can prove daemon loss after grant and before attachment for both live and exited children. Do not release a permit merely because its owner disappeared. This gap is established by the current source; this attempt did not claim a new end-to-end reproduction or repair.
2. **Storage:** split `crates/arc-storage/src/jobs/traffic/tests/recovery.rs` below the harness's 500-line limit without removing tests. In `crates/arc-storage/src/migration/tests/acceptance.rs:97-115`, assert original permit unique constraints on the actual upgraded database; coordinate the pinned preservation recipe with the verification owner.
3. **Documentation owner:** `docs/managed-exec.md` still describes Phase 0 only, no reservations, one shared preparation deadline and no fallback post-spawn metadata calls. Update it against the implemented behavior and the unresolved recovery boundary.
4. **Verification owner and Chief:** add a narrowly scoped schema-3 preservation recipe for the daemon's existing version assertions, preserving their other tokens and reachability; the first observed mismatch is `crates/arc-daemon/src/database_failure_tests.rs:351` versus `scripts/coverage/schema-adaptations.ts`. Re-run the validator to expose any later mismatches. Investigate the pinned native fault proxy's intermittent `EINVAL` with the existing preservation contract; rerun the exact workspace command, coverage and no-skip harness after the required fixes. Produce task 9's actual measurement and gate receipts, and have independent `oma-qa` review the integrated result. Do not reuse this report as a final receipt.

Desktop, native Windows, vendor hooks, broader detection and remaining scheduling policy are outstanding. The available target check reported not applicable; no container or service health was observed. Full `git status` printed `av.yaml: Operation not permitted`, so that file's state is unverified and it was preserved. Serena was absent from the tool inventory; source inspection used supplied and directly related known paths. Used the `oma-backend` skill. No browser or Playwright tooling was used.

## Raw log references

Raw logs are under `target/traffic-task7-attempt4/`; values below are SHA-256:

| File | SHA-256 |
| --- | --- |
| `traffic-cli.log` | `d40f014b04e2c19015cad90b49ef5439cc402bcb3a2268bddb59e3180d51edda` |
| `lineage-unit-2.log` | `9838ece7a80ca3d39c76cb8b5f8b8a12bbb2325527ff3bf3dc8231c2d2c3c95d` |
| `lineage-cli.log` | `e9f1e9f74e34b3cfd84fb96321626a988cfd75b0302612a6b2188d09e41ae56d` |
| `notice-red.log` | `143caf1c706414782419fd348315349dc7ec4e93d9de8f0070bee92ad16dd004` |
| `notice-green2.log` | `2f2195c052c4cc695cd0815097f0327e482fad086ebdd06711e6f23fcfa88fb5` |
| `status-red.log` | `f3b8fb7e76c7914549902b862d66ef9e1f46ae28457d0b0d6276705574e3be4b` |
| `status-green.log` | `128b56602577d6735dadaeb75b8121f1cc49312bcd0c7a584c20b6dded4ffb6e` |
| `cli-final.log` | `a7a41bbd9967da6b0f65efaf41d04fc49d5ceeb97b0e09e4e985d17db0137e9b` |
| `fmt.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `clippy.log` | `fab191378e158cdffa8fca1b04550fa107468bb8cef8cfacfae0398945c9ba45` |
| `workspace-test.log` | `3d32b696398203d0e84eb488a30451994e2ebdb4b27b317a56c95b3dbfae11cf` |
| `workspace-retry.log` | `520e7f6691ce8316dd399b48db4bda13720e343cd80289a7475c4220c865b5b4` |
| `coverage.log` | `2f5460403c4cddeee96bf1006ec0647348c0762449332bad8d4fe70b394945de` |
| `harness.log` | `c07f856f4d272211d91db08e76e600e8c345a7fb20839a81d00ffc4244565eed` |
| `coverage-evidence.log` | `bf724df9390f1f3be3cfa204d2c092931bd0dbad0ffeb1c5b05f0f8b06c46dd4` |
| `traffic-evidence.log` | `524c0485cf60a486b3c5b98914dbfd7463488f73211bb1142a5df94a120502ef` |
| `targets.log` | `09d835abaf710601675a1dbba3ef5e8c802041ccb51b7df66ae6ef9a29dba4c1` |
| `current-preservation.log` | `095f525cac7aef3af0479c7b7741b4d1a9de3740d1798666cda8ca6d0fe81dc4` |
