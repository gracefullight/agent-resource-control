# Test-performance contract

Test/support only: preserve all scenarios, assertions, registered desktop
bridges, product behavior and `.arc-verify/agent-hooks/` files. This directory
holds only `contract.md` and `qa.md` (each at most 8,000 bytes); runner records,
receipts, hashes and command results stay out of Git.

## Immutable verification contract

Agent Valley runs these four checks once per integration or final verification.
Bun names 22 active files; fixtures and archives are not standalone tests.
Discovery mismatches block acceptance. `timeoutMs` is an estimate; runner and
replay wrappers allow at least 7,200,000 ms per check.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
bun test apps/desktop/tests/bridge-client.test.ts apps/desktop/tests/contracts.test.ts apps/desktop/tests/credits.test.ts apps/desktop/tests/dashboard.test.ts apps/desktop/tests/governor-contract.test.ts apps/desktop/tests/governor.test.ts apps/desktop/tests/lifecycle.test.ts apps/desktop/tests/meter.test.ts apps/desktop/tests/traffic-scripts-contract.test.ts apps/desktop/tests/traffic-scripts.test.ts scripts/coverage/accounting.test.ts scripts/coverage/desktop-foundation.test.ts scripts/coverage/evidence.test.ts scripts/coverage/gate.test.ts scripts/coverage/hygiene.test.ts scripts/coverage/terminal-outcomes.test.ts scripts/coverage/traffic-accounting.test.ts scripts/harness/coverage-reuse.test.ts scripts/harness/equivalence.test.ts scripts/harness/traffic-targets.test.ts scripts/requirements/traceability-parser.test.ts scripts/requirements/traceability.test.ts
```

Runner owns integration, desktop typecheck/check, SwiftPM tests, coverage and
the architecture harness. Collect coverage once per input identity; reuse a
matching valid receipt and retain all eleven 80% per-crate floors and checks.
The operator waived the original 360-second limit only. Require two accepted
runner-recorded full workspace passes below 900 seconds each on the eight-core
Apple M1 Pro with unchanged final implementation and test inputs. A failed run
is never a pass. Runner keeps failure logs and repeats a failed check
once. Amendment 20: record a full-run failure that passes alone in `qa.md`; reject
for that flake only after failures in two consecutive checks or both isolated
runs. QA identifies its reviewed snapshot.

## Fixture and correctness obligations

Per binary, prepare offline Go, SwiftPM and CMake fixtures and build trees once
from a clean checkout, using real tools and private writable state. Prove the
relocated CMake tree executes. Prepare SwiftPM trees in place by form and lease
them exclusively; check fixed preparation counts and same-path reacquisition.
Parallelize independent cases; retain
per-user daemon-anchor coordination, ordered pairs, native outcomes, CLI/SQLite
reconciliation and registered desktop routes and assertions.

Observe readiness, release and process state; inject product clocks. Never use
sleep or elapsed time as success. Child waits need diagnostic watchdogs of at
least 60 seconds and bounded owned-child/descendant cleanup; Bun subprocess
tests need explicit timeouts. Passing fixtures remove their directories; failures
retain only theirs. Validated coverage removes its instrumented build. Keep the
500 MB allocated-block limit outside `target/debug`, excluding runner-owned
`target/av-runner`. Forward `LLVM_PROFILE_FILE` through cleared environments;
child entry tests return early unless selected and are never `#[ignore]`. Remove
owned helper files and symlinks; preserve archived validator bytes and receipt
validation.

## Concurrent ownership and compatibility

Writers are exclusive; reliability exceptions override native directories.
Keep `Read`, `RunnerTools`, `Case`, `Run`, `Prepared`, `Form`, `Pair`,
`NATIVE_FORMS`, `ORDERED_PAIRS`, witness types/functions, family
`semantics(Read)`/`pairs(Read)` and direct/registered callbacks source-compatible.
Private helpers are allowed; no lane may require a concurrent lane's interface
or wiring change. Report needed seams to Chief; integrate after an actual
artifact dependency.

Below, `D=crates/arc-daemon/tests`, `N=D/traffic_native_tools`,
`T=apps/desktop/src-tauri/tests`, `C=crates/arc-cli`; braces expand filenames.

| Writer | Exclusive files or scope |
| --- | --- |
| Native | `D/traffic_native_tools.rs`, `N/{mod,descriptors,fixtures,scenarios,go,swift,cmake}.rs`, `N/{go,swift,cmake}/**` except reliability files; `T/traffic_native_tools.rs`. Bound the CMake `project.rs` child wait. |
| Performance | `D/traffic_bun_scripts.rs`, `D/traffic_bun_scripts/**` including `mod.rs` and `projects.rs`, `T/traffic_bun_scripts.rs`; other >30 s binaries and otherwise unowned files as below. Preserve shared Bun interfaces. |
| Reliability | `N/witnesses.rs`, `N/go/{cleanup,owned}.rs`, `N/go/sources/interrupt_test.go.txt`, `N/swift/process.rs` and `N/swift/process*_tests.rs`; `D/support/**`, `D/traffic_cli/{process_cleanup,terminal_outcomes}.rs`; `C/tests/binary.rs`, `C/tests/binary/hook_scheduled.rs`, `C/src/exec/launch/tests.rs`; `scripts/harness/coverage-reuse.test.ts` and `scripts/coverage/**`. Other files needed for follow-ups are reliability-owned unless reserved above. |
| Integration | Cargo/Bun manifests and lockfiles, other shared registries not named above, and `docs/requirements/traceability.md`; wire deliveries, repair integration failures in any file and update affected rows/counts after the wave. |
| Independent QA | Review lanes against this contract and the SRS in parallel with integration; write only `qa.md` once. Blocking findings go to the file owner. |

Performance owns `C/tests/governor.rs`, `D/{managed_exec,traffic_cli}.rs` and
`T/daemon_bridge.rs`, subject to the exceptions above.

The runner keeps full-run total/test/build-link and per-binary timings in
`target/av-runner/gates/cargo-test-workspace.binaries.json`. Lanes used this
inventory for ownership; integration and QA consume it. Handoffs need a real
artifact dependency; do not alter saved tasks or graph. Keep timings out of Git.

## Historical test-support follow-ups

Reliability verifies and repairs applicable `.arc-verify/agent-hooks/qa.md`
follow-ups on current inputs, with focused checks:

- Synchronize `go_ordered_pairs` and direct/registered Swift semantics through
  separately observed admission and readiness.
- Repair empty Go PID, descendant registration and atomic markers; Swift direct
  timeout and unexpected-release sibling cleanup, including stopped/zombie and
  permission errors.
- Add coverage-reuse's explicit subprocess timeout; bound pipe/child waits in
  `binary/hook_scheduled.rs`, `exec/launch/tests.rs` and probe descendants.
  Verify owned cleanup on success and failure.
- Verify legacy preparation/pointer/lease atomicity and Rust receipt/artifact
  validation; retain native-profile/source checks and prove ordinary worker
  execution without accepting ignored tests.
- Verify Meter and notice-writer clocks, recent-fifty deadline, fixture locks and
  remaining synchronization/cleanup; retain SIGINT, killed-owner, SIGPIPE,
  owned-tree and calibration obligations using observed state or injected clocks.

Native owns fixtures and the CMake watchdog; reliability verifies without
concurrent edits. Historical passes are not final-input proof. Hook rewrites,
vendor integration and installer hardening are outside this mission.

## Outstanding scope

Passed lanes need no speed work. Agent Valley verifies final inputs, offline
fixtures, Bun discovery, both workspace passes, desktop/SwiftPM, coverage and
harness reuse. QA owns `qa.md`. `av.yaml` is unreadable here; configured target
health remains unverified.
