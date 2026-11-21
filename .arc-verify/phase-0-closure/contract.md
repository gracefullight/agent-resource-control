# Phase 0 closure contract

The accepted product tree is `b7622ba1c876b8cc4680358acbd4857ac304cce8`.
The first task numbers the SRS-derived cases in
[`closing.md`](../../docs/requirements/closing.md). `HOST` cases require a
named passing test on the production path; `EXTERNAL` is reserved for an
actual unavailable external environment, never unimplemented or untested HOST
work. A selected row is Complete only when its HOST cases pass and no external
items remain; it is Blocked only for genuinely external remaining items. Keep
linked acceptance tests independent: AT-TRF-12 needs stage-by-stage control,
and AT-SWP-20 needs both remote-context switch and named-volume protection.

The installed hook test is simulated agent execution using a temporary home,
real local ARC daemon and offline gh stub, not live vendor/GitHub acceptance.
While a separate local Build permit is held, correlate the event, returned
command, single child invocation and REMOTE_CI daemon record, and assert no
new local queue entry or reservation. Preserve command semantics, existing
build rewrites and vendor permission evaluation. Test capture deadlines with
an injected clock and observed child/daemon state; test startup-affected nested
bash as UNKNOWN/OBSERVE through both routes. Preserve the 42 simulated cases;
report their denominator separately from named production-path cases, with
recognized-remote, UNKNOWN and false-local counts on each route. False-local
must be zero, and overlapping cases or the two routes are never added twice.

## Sole writers per wave

| File or seam | Wave 1 | Wave 2 | Wave 3 |
| --- | --- | --- | --- |
| `docs/requirements/closing.md`, this contract | task 1 | read only | integration |
| `crates/arc-detector/src/parser.rs`, `lexer.rs`, `rules.rs`, `argv.rs`, `lib.rs`, shell-parser tests and `scripts/**` | task 1 for the guard | shell lane | integration for repairs |
| `crates/arc-detector/src/locality.rs`, `locality/**`, `remote_ci.rs`, `remote_endpoints.rs` and their locality tests | task 1 for the guard/test | locality lane | integration for repairs |
| `crates/arc-cli/src/exec/capture.rs`, `ci_context/**`, `endpoint_context/**` and locality-specific tests | task 1 only if needed | locality lane | integration for repairs |
| `crates/arc-cli/src/hooks.rs`, hook tests, `src/classify/**`, `src/exec/entry.rs`, `run.rs`, `preparation.rs`, `metadata.rs` and their exec tests | task 1 for compatibility test | hook/exec lane | integration for repairs |
| `crates/arc-daemon/tests/managed_exec/capture_integration.rs` | task 1 | read only | integration |
| `capture_integration/closing_locality.rs` | task 1 baseline | locality lane | integration for repairs |
| `capture_integration/closing_exec.rs` | task 1 baseline | hook/exec lane | integration for repairs |
| `capture_integration/closing_shell.rs` | task 1 baseline | shell lane | integration for repairs |
| `crates/arc-daemon/tests/{support/instrumentation.rs,support/runtime.rs,managed_exec/shared_execution_contract.rs,classification.rs}` | task 1 | read only | integration for repairs |
| shared manifests and `Cargo.lock` | task 1 if needed | locality lane if needed | integration if needed |
| `docs/requirements/traceability.md` | read only | read only | integration |
| `.arc-verify/phase-0-closure/qa.md` | read only | read only | independent QA once |

The three `capture_integration/closing_*.rs` entries above are under
`crates/arc-daemon/tests/managed_exec/`. Existing shared registration,
fixture and instrumentation seams are working code, not replacement
placeholders. Wave 2 runs the three lanes concurrently; wave 3 runs integration
and independent QA together. Lanes extend their own registered module and consume shared
helpers without editing them. A needed ownership handoff requires an actual
artifact dependency; otherwise report a conflict. Integration owns the final
wiring, remaining Phase 0 lists, status/count reconciliation and repairs for
runner-reported integration failures in any file. QA reviews its lane snapshot
once, alone writes `qa.md`, and never rewrites it after later deliveries.

## Verification and reviews

Independent task reviews and final verification are required. The first
review names every blocking defect; other findings are follow-ups. Later task
rejection is limited to an unresolved earlier mission finding, a defect in
lines changed since that task's prior review, or a failed runner step caused
by a file it owns. Record a full-run failure that passes alone; reject it only
if it fails in two consecutive checks or fails again alone. A passing runner
integration check is complete execution evidence. QA rejection is limited to
a false statement about its snapshot, omission of an earlier mission review's
production-path gap, or a size/path violation. `contract.md` and `qa.md` are
each at most 8,000 bytes; tracked cycle evidence is limited to those files,
`closing.md` and `traceability.md`.

The runner owns desktop checks, coverage of at least 80% per crate,
traceability and the architecture harness. Actors run focused named checks in
their owned crates; they do not run workspace tests, coverage, the architecture
harness, SwiftPM-backed tests, browser E2E or screen checks. Tests launching
built binaries clear the environment, use temporary HOME and explicit fixture
configuration, and do not read user configuration, host names, account names
or network interfaces. Forward `LLVM_PROFILE_FILE` through cleared child
environments and use the shared instrumentation helper for exact environment
comparisons. Use observed state or injected clocks and diagnostic child
watchdogs of at least 60 seconds. Do not ignore tests. The runner's separate
per-check timeout is 7,200,000 ms; command estimates do not terminate checks.

The fixed runner verification commands are exactly:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
bun test apps/desktop/tests/bridge-client.test.ts apps/desktop/tests/contracts.test.ts apps/desktop/tests/credits.test.ts apps/desktop/tests/dashboard.test.ts apps/desktop/tests/governor-contract.test.ts apps/desktop/tests/governor.test.ts apps/desktop/tests/lifecycle.test.ts apps/desktop/tests/meter.test.ts apps/desktop/tests/traffic-scripts-contract.test.ts apps/desktop/tests/traffic-scripts.test.ts scripts/coverage/accounting.test.ts scripts/coverage/desktop-foundation.test.ts scripts/coverage/evidence.test.ts scripts/coverage/gate.test.ts scripts/coverage/hygiene.test.ts scripts/coverage/terminal-outcomes.test.ts scripts/coverage/traffic-accounting.test.ts scripts/harness/coverage-reuse.test.ts scripts/harness/equivalence.test.ts scripts/harness/traffic-targets.test.ts scripts/requirements/traceability-parser.test.ts scripts/requirements/traceability.test.ts
```
