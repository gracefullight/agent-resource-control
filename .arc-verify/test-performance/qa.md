# Independent test-performance QA

Verdict: **NOT ASSESSED; verification partial.** No reproducible blocking
finding was established in the delivered lanes at the reviewed snapshot.

## Scope and review boundary

This review covers the initial isolated task-6 snapshot and the task-2, task-3
and task-4 overlays, not the committed baseline. Eighteen delivery files matched
the source workspace; full input identity was not established. Parallel task-5
is outside this snapshot. The 11:50 review boundary and 12:30 waiver apply.

The SRS requires output/exit preservation, fail-open behavior and queue
reconciliation (`docs/requirements/ARC_SRS_2026-11-06.md:977`). The reviewed
test wiring retains real registered desktop calls for native and Bun scenarios
(`apps/desktop/src-tauri/tests/traffic_native_tools.rs:26` and
`apps/desktop/src-tauri/tests/traffic_bun_scripts.rs:26`). Go publishes a seed
before private copies (`crates/arc-daemon/tests/traffic_native_tools/go/projects.rs:100`);
CMake prepares a seed (`crates/arc-daemon/tests/traffic_native_tools/cmake/project.rs:54`);
Swift leases fixed, in-place trees (`crates/arc-daemon/tests/traffic_native_tools/swift/pool.rs:58`).
The earlier Swift product-byte identity assertion was withdrawn by the operator;
fixed preparation count, isolation and same-path reacquisition remain in
`crates/arc-daemon/tests/traffic_native_tools/swift/relocation_tests.rs:43`.

## Blocking findings

None reproduced under the operator's review boundary.

## Test-support follow-ups

Historical single failures in `.arc-verify/agent-hooks/qa.md` do not establish
final-input failures. The earlier report records isolated passes for the first
three entries and unexpected-release case; the available integrated run passed
them too.

- Go ordered-pair synchronization: `crates/arc-daemon/tests/traffic_native_tools/witnesses.rs:33`
  observes admission and readiness separately; retain both and the pair assertions.
- The interrupted Go fixture previously read an empty PID. Atomic marker rename
  is at `crates/arc-daemon/tests/traffic_native_tools/go/sources/interrupt_test.go.txt:12`;
  descendant identity and cleanup are checked at `crates/arc-daemon/tests/traffic_native_tools/go/cleanup.rs:41`.
- Swift's earlier timeout-kind and unexpected-release cleanup failures are
  covered at `crates/arc-daemon/tests/traffic_native_tools/swift/process_tests.rs:168`
  and `crates/arc-daemon/tests/traffic_native_tools/swift/process_release_tests.rs:111`.
- The historical coverage-reuse default-timeout failure is addressed by the
  explicit watchdog at `scripts/harness/coverage-reuse.test.ts:273` and
  `scripts/harness/coverage-reuse-test-support.ts:14`. Direct and registered
  Swift readiness timeouts were also reported; witnesses separate admission
  from readiness. Not every historical case has an established isolated pass.
- The earlier coverage-reuse failure involved ignored workers. The worker is now
  an ordinary selected test with an execution marker
  (`crates/arc-cli/tests/binary.rs:136`); receipt validation still rejects
  ignored or filtered tests (`scripts/coverage/coverage-receipts.ts:49`).
  Final-input collection and reuse remain unverified here.
- Clock and atomicity follow-ups have focused test coverage: notice-writer
  injected time is at `crates/arc-cli/src/exec/preparation/tests.rs:6`, Meter
  inputs at `crates/arc-meter/src/cpu_tests.rs:15`, and the recent-fifty
  injected clock at `crates/arc-daemon/tests/traffic_cli/terminal_outcomes.rs:228`.
  Legacy preparation: `crates/arc-daemon/tests/legacy_preparation.rs:4`.
  Final receipt, profile and artifact validation remain runner gates.
- Earlier runner checks exceeded the 500 MB artifact limit and isolated retries
  also failed; these were not qualifying isolated-pass flakes. A later
  integrated run passed the hygiene check. Its final post-workspace repetition
  is still required; this review did not establish the earlier cause.
- Historical unbounded waits (`.arc-verify/agent-hooks/qa.md:33`): source
  inspection shows `crates/arc-cli/tests/binary/hook_scheduled.rs:29` captures
  to files and uses the 120-second owned-child watchdog in
  `crates/arc-cli/src/hooks/fixture_owned.rs:11`. The release-loss follow-up
  (`.arc-verify/agent-hooks/qa.md:120`) has 60-second receives and socket
  polling at `crates/arc-cli/src/exec/launch/tests.rs:290` and `:300`, using
  `crates/arc-cli/src/exec/launch/release_tests.rs:3`. A focused helper test
  is at `crates/arc-cli/src/exec/launch/tests.rs:49`. This is source inspection;
  this review did not execute the watchdog paths.

Deferred under the operator's boundary, with no reproduced hang here:

- `scripts/coverage/gate.test.ts:21`, `scripts/coverage/hygiene.ts:50`,
  `scripts/coverage/historical.ts:30`, `scripts/coverage/hygiene.test.ts:128`
  and `scripts/coverage/accounting.test.ts:462` have subprocess waits without
  an explicit watchdog. Add bounded ownership and cleanup later.
  `scripts/coverage/commands.ts:9` can await the child or pipes after SIGTERM;
  bound termination and draining too.
- `crates/arc-daemon/tests/support/mod.rs:46` waits on the per-user fixture
  lock without a deadline. Preserve daemon coordination and bound the wait.
- `crates/arc-daemon/tests/support/child_watchdog.rs:70` signals the owned
  process group. Detached sessions and PID reuse remain deferred, not reproduced.
- `crates/arc-daemon/tests/traffic_cli/semantics/output.rs:64` delays reaping,
  but a pipe holder may already be reparented; reproduce using process identity.
- `crates/arc-daemon/tests/traffic_native_tools/go/owned.rs:414` and
  `crates/arc-daemon/tests/traffic_native_tools/go/owned_events.rs:50` are the
  Actor's hypothesized missed-exit-wakeup location. Reproduce the race while
  retaining the pipe-holder assertions; no cause was established here.
- `scripts/coverage/source.ts:258` omits `docs/requirements/traceability.md`
  although `scripts/requirements/traceability.test.ts:6` reads it. The operator
  directed the runner to rerun Bun every check; widen input identity next cycle.
- Fresh-clone offline Go, SwiftPM and CMake preparation remains a runner
  follow-up (`crates/arc-daemon/tests/traffic_native_tools/go/projects.rs:100`,
  `crates/arc-daemon/tests/traffic_native_tools/swift/projects.rs:14`,
  `crates/arc-daemon/tests/traffic_native_tools/cmake/project.rs:54`). The
  operator reports offline runs cleared Swift and Bun trees before each run;
  no fresh-clone exercise is required for this mission.

## Performance follow-ups

The pre-integration runner inventory measured these times; the two native
entries were distinguished using scenario names. They are not customer outcomes:

| Binary | Seconds |
| --- | ---: |
| `crates/arc-daemon/tests/traffic_native_tools.rs` | 139.55 |
| `apps/desktop/src-tauri/tests/traffic_native_tools.rs` | 88.08 |
| `crates/arc-cli/tests/governor.rs` | 52.93 |
| `crates/arc-daemon/tests/managed_exec.rs` | 50.87 |
| `crates/arc-daemon/tests/traffic_cli.rs` | 46.19 |
| `apps/desktop/src-tauri/tests/daemon_bridge.rs` | 40.88 |

The 360-second limit was unmet and waived; two accepted unchanged-final-input
runs under 900 seconds are required as a regression guard.

## Remaining verification gaps

At this snapshot one integrated workspace pass was available; final input
identity, two accepted runs, the four-command contract, SwiftPM and desktop,
post-workspace hygiene, eleven 80% coverage floors, receipt reuse, traceability
and the full harness remain runner gates. Final records govern amendment-20
flakes. Later operator rulings supersede the contract's former 360-second and
Swift relocation requirements.

Review method: `oma-qa`, scoped source and diff inspection and runner evidence.
Serena tools were unavailable; named-file reads do not establish exhaustive
code discovery. The shell reported `Operation not permitted` for `av.yaml`, so
the full worktree state could not be verified from this workspace.
