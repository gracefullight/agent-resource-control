# Traffic task 7: cleanup and final harness

Status: completed for the operator's task-7 closing scope. Skill used:
`oma-backend`. The independent review and mission-level verification remain
downstream tasks.

## Changes

- `crates/arc-daemon/tests/traffic_cli.rs`: require an observed wrapper
  identity and surface cleanup-observation errors.
- `crates/arc-daemon/tests/traffic_cli/process_cleanup.rs`: distinguish a
  confirmed disappearance from an unavailable identity or descendant scan;
  verify current identities and parent relationships before signaling;
  retain and report errors during unwinding without a second panic.
- `crates/arc-daemon/tests/traffic_cli/cleanup.rs`: cover live capture,
  disappearance, denied identity and enumeration observations, errors during
  unwinding, and interruption of a cached Cargo launch with a safety guard.

These are native test-fixture changes. No product, IPC, storage, or CLI
interface changed in this attempt. Existing actor changes were preserved.

## Checks

All checks below ran on macOS in the isolated checkout described below.
They completed without a shortened timeout.

| Command | Result |
| --- | --- |
| `cargo test -p arc-cli --test binary --locked --offline` | Passed: 4 tests; established the CLI fixture binary prerequisite. |
| `cargo test -p arc-daemon --test traffic_cli cleanup:: -- --nocapture` | Passed: 7 tests. The interruption and unwind tests intentionally catch a panic. The cached-launch check observed the wrapper, Cargo, and one descendant terminate. |
| `cargo test -p arc-daemon --test traffic_cli cached_cargo_launch_still_executes_one_native_child -- --nocapture` | Passed: 1 test. |
| `cargo test -p arc-daemon --test traffic_cli -- --nocapture` | Passed: 37 tests, none ignored. |
| `cargo fmt --all -- --check` | Passed. |
| `./scripts/harness/validate.sh` | Passed once, without skips, from 2026-11-10 17:01:44 UTC to 17:05:32 UTC. The instrumented workspace command passed 522 tests with zero failures and zero ignored. |
| `bun test scripts/coverage/evidence.test.ts` | Passed after that collection: 1 test, 4,962 assertions. |
| `git diff --check` for the three fixture files in the isolated index | Passed. |

The full CLI suite's native observation recorded peak real Cargo builds of 2
with bypass and 1 with scheduling, one invocation for each job, a queued
notice before release, a second start after release, 370 ms queue wait, and
two `EXITED:0` outcomes. This is the suite observation, not task 9's integrated
acceptance receipt.

The harness measured production-line coverage as follows: arc-core 96.48%,
arc-detector 99.84%, arc-storage 95.57%, arc-ipc 98.92%, arc-daemon 91.18%,
arc-cli 83.41%, and arc-traffic 100.00%. Each exceeded the unchanged 80%
floor.

## Source and isolation

HEAD was `4d2f71c1a1482925f6c9918296d7ceb3435b7592`. The coverage validator's
`inputIdentity` was
`4803e02c0a444268a405a736f7345c829b1381fa6be38a81235ea3c31c5f2e78`
for both the mission tree after the checks and the isolated tree used for
collection. This identity covers the validator's declared inputs, not the
inaccessible `av.yaml` or every possible worktree file.

The prior harness session 39327 returned `Unknown process id` when queried.
Its completion and liveness could not be established. The new run therefore
used `target/traffic-task7-attempt11-checkout`, an independent checkout with
the accessible tracked changes and untracked files overlaid, its own index,
target and coverage directories, and an isolated `WORKSPACE_ROOT`. Its
untracked inputs were marked intent-to-add in that checkout so the harness
could inspect them. No native coverage report was copied between checkouts.
The collector and post-collection evidence test ran in the same checkout.
Keep that checkout and its target files for review.

The changed fixture files have these SHA-256 values:

| File | SHA-256 |
| --- | --- |
| `crates/arc-daemon/tests/traffic_cli.rs` | `04d9254b89358dee8ad153286725c84b8f2cb68dc221d818ee3fcef23f7a2ae7` |
| `crates/arc-daemon/tests/traffic_cli/cleanup.rs` | `7ac0dc090290f319bf37d8b66a346f8a3f73550028623bfc8f4871c576237285` |
| `crates/arc-daemon/tests/traffic_cli/process_cleanup.rs` | `ab8844e0dc72d8177d7adcbceeececbccb0486aacc591a84db28565c751ec5f6` |

## Raw evidence

Paths in this table are relative to
`target/traffic-task7-attempt11-checkout/target/task7-attempt11/`. Values are
SHA-256.

| File | SHA-256 |
| --- | --- |
| `cli-prerequisite.log` | `01abe97c851b8ba8016d0d4d605a887256ed994de7ff566ecbf78aa9dc9658c1` |
| `cleanup.log` | `21ff04bd3b6265fbbdafd6cdc1f942241c1b0390b5576d673d0f06d5bc43977d` |
| `cached-launch.log` | `abad20620eec01d687878048ba097722dbd6c8fe4499655fe8775602c6b16004` |
| `traffic-cli.log` | `c0bda968927ac66a850ce16c91f74f7515d0a4b8322d7b8eb0936cf5fc76c51b` |
| `fmt.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `harness.log` | `48cd7358d0e54f478fbc4707f6877ddecc6d29ce2eea3f3235e523e071065fb5` |
| `coverage-evidence.log` | `2f678f560f96ba4bd144f474b6a3a3b0f5798a6e7a75f243802eaaac1a7ae644` |
| `source-identity.json` | `ce02722ef5566f6e5f79c66c0d6866d7836b7b2b155973ba43843b47afd20697` |

The instrumented workspace command's complete output is in
`target/traffic-task7-attempt11-checkout/.arc-verify/rust-coverage/baseline/runs/2026-11-10T17-01-50-789Z-c29762d1-9914-4593-a0ab-c8bee3aeec04/06-command.json`,
SHA-256 `ada7f52e10f59061cb9b89b4388d268323679fedefacbf7162857a6803dea711`.
The coverage receipt and native reports remain alongside it in the isolated
checkout. `receipt.json` has SHA-256
`984d8d683577b17ba14bac105ae58057de6b6ef2e1d83103498b4a00c5c1cf8d`;
`summary.json` has SHA-256
`8cf80da2075afaf3564ce403429cc282b60b2cffb10ec55e678a7c504973a9be`.

## Remaining scope

Task 9 must produce the integrated measurement and gate receipts; task 10
owns independent `oma-qa` and target observations; task 11 owns final
mission-level verification. The previous inaccessible harness has not been
asserted stopped. Git reported `av.yaml: Operation not permitted` in the
mission tree, and Serena was unavailable; the assigned files were read
directly. Desktop, native Windows, vendor hooks, broader detection and
remaining scheduling policy remain outstanding.
