# Traffic task 7, attempt 9

Status: **partial**. The repair and focused checks passed; the full harness had
not completed at handoff, and its live coverage-evidence check could not yet
run. Used `oma-backend` and `oma-dev-workflow`.

## Repair and scope

The cached Cargo fixture now launches through the same `BuildChild` guard used
for the other real-Cargo fixtures. The queued cancellation and notice fixtures
also use that guard because a failure could otherwise allow their real Cargo
commands to start. Nested and bypassed real Cargo launches already used the
guard. The regression warms a project, holds Cargo's build-directory lock, and
interrupts the cached launch while its real Cargo process and an observed child
are alive. It verifies native identities for the wrapper, Cargo, and child, and
requires `Missing` or `Mismatch` after the interruption; indeterminate native
observations do not count as termination. A separate identity-checked cleanup
guard remains in place if an assertion fails.

Changed paths, relative to the mission worktree:

- `crates/arc-daemon/tests/traffic_cli/recovery.rs`
- `crates/arc-daemon/tests/traffic_cli/cleanup.rs`
- `crates/arc-daemon/tests/traffic_cli/diagnostics.rs`
- `crates/arc-daemon/tests/traffic_cli/robustness.rs`
- This report.

The changes affect test fixtures only; product protocol and execution interfaces
are unchanged. The accepted witness, recovery, and status policies were not
changed.

## Completed checks

| Command | Actual outcome |
| --- | --- |
| `cargo test -p arc-daemon --test traffic_cli interrupted_cached_launch_reaps_verified_cargo_tree -- --nocapture` | Passed, 1 test. The intentionally caught panic marks the interruption; the wrapper, Cargo, and one observed descendant terminated. |
| `cargo test -p arc-daemon --test traffic_cli cleanup:: -- --nocapture` | Passed, 2 tests. |
| `cargo test -p arc-daemon --test traffic_cli cached_cargo_launch_still_executes_one_native_child -- --nocapture` | Passed, 1 test. |
| `cargo test -p arc-daemon --test traffic_cli -- --nocapture` | Passed, 32 tests, 0 ignored. |
| `cargo fmt --all -- --check` | Passed. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed. |
| `git diff --check -- crates/arc-daemon/tests/traffic_cli.rs crates/arc-daemon/tests/traffic_cli` | Passed using the separate intent-to-add index. |

The native CLI suite's measurement recorded independent directories, bypassed
peak 2 and scheduled peak 1, one invocation per job, queued before release,
start after release, 122 ms queue wait, and two `EXITED:0` outcomes. This is
the test log's observation, not the downstream task-9 receipt.

## Source identity and raw logs

HEAD was `4d2f71c1a1482925f6c9918296d7ceb3435b7592`. The existing coverage
validator's `inputIdentity` for the current source was
`54cbc59992f44190bac4726f449c775e927664b801894f0533ae3e5ca592bc5a`.
The full input manifest is `target/traffic-task7-attempt9/source-identity.json`.

All raw paths below are under `target/traffic-task7-attempt9/`; values are
SHA-256:

| File | SHA-256 |
| --- | --- |
| `cached-cleanup.log` | `a2b07371a1750876acf642697d6cdd8677efe7d7f3cf413f19e2172965d39e58` |
| `cleanup.log` | `808d2784e4e442da2887adb97763be675dd8f8db8a648fb6e9f2cf3d7a174806` |
| `cached-launch.log` | `444108634eb790382793af101ce165aea6978138659225ec802c3bf3f8d9047c` |
| `traffic-cli.log` | `277e7ece5bc8f48d6787805eecc24f9028ae783710ab94fba152e5ae97ade936` |
| `fmt.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `clippy.log` | `ea9c51a5dabb87aa19eadd3cd86c053868e016b105a363775d16aa6a1fdd7db1` |
| `source-identity.json` | `ebb344feab5632526dfdc8321fb5f53a1e4f54032a3eabac5d325180cffba616` |
| `harness-pending.log` | `883c415c22ebe8e5fd48f611f571f10390e245ec56ee376ea770acae9aa187d6` |

The harness uses `target/traffic-task7-attempt9/harness.index`, copied from the
shared index with 71 untracked paths added as intent-to-add. The shared index
was not edited. No short wrapper timeout was applied.
The pending harness snapshot records four passing structural checks and the
start of coverage, not a completed coverage or harness result.

At 2026-11-10 16:52 UTC, the harness was still running its instrumented
workspace test command, approximately 20 minutes after collection began. The
previous successful collection's test command took about three minutes, but
that comparison does not establish the cause or final result. The current
collector has not written `06-command.json` or a new `latest.json`. Process
inspection with `ps` was denied, so the active test and any descendants are
unverified. The execution session was `39327`; the harness was not stopped or
given a shorter timeout. Its continuing availability after this handoff has not
been verified. Once the final collection exits, the outstanding command is
`bun test scripts/coverage/evidence.test.ts`; its result must not be inferred
from the previous run's receipt.

The Traffic evidence directory contains 15 files. Counting it together with
the Traffic-related crate reports and coverage contract gives 24 files and
approximately 250 KB, within the 30-file and 5 MB limit. Raw logs remain under
`target`.

## Limits

Git reported `av.yaml: Operation not permitted`; its state is not verified.
Serena was unavailable, so the assigned paths were read directly. A read-only
attempt to run `ps` was denied with `Operation not permitted`; no alternate
process inspection was used. Task 9 owns integrated measurement and gate
receipts, task 10 owns independent QA, and task 11 owns final verification.
No configured-target inventory or health conclusion follows from a no-policy
result. Desktop, native Windows, vendor hooks, broader detection, and remaining
scheduling policy remain outstanding.
