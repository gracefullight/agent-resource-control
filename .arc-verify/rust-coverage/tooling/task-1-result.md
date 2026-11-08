# Task 1 handoff to Chief Director

Status: partial. The assigned tooling, accounting contract and executable check paths are implemented. Real Rust instrumentation is blocked by unavailable prerequisites and provisioning failures. No six-crate production coverage values, instrumented-child profile proof or per-file uncovered measurements exist yet. This is not mission completion or independent QA approval.

## Source and ownership

Accepted baseline: `fbe76a5f15101ad415f355ef06f1deeb1f97d568`; product tree: `8db02719b5bd06d888729ca337f9bf8887991515`. Actor snapshot/local HEAD: `d3c62cb034707d8cdbb30ba3ab51a392e92a6cce`, same initial product tree. All task edits are Actor-local and uncommitted. They are not in accepted HEAD. The integrated worktree must verify delivered bytes.

Changed owned paths are `package.json`, `mise.toml`, `rust-toolchain.toml`, `scripts/coverage/`, and `.arc-verify/rust-coverage/{contract,baseline,tooling}/`. No production Rust file, existing test/assertion, Cargo.toml, Cargo.lock, protected migration or harness was changed. Scoped accepted-baseline Git diff for crates, Cargo manifests/lock and harness is empty. Global status emitted `av.yaml: Operation not permitted`; it was not bypassed. Serena was not exposed, so known-path reads and Git metadata were used.

The existing `coverage/` ignore pattern also ignores `scripts/coverage/`. The new owned scripts were force-added with intent-to-add so scoped Git diff/collection can see them. No commit was made, and that local index state does not prove that a downstream review checkout has the scripts. The Chief must include every script from the delivery inventory when integrating. No ownership change or shared-manifest conflict occurred.

## Delivered interfaces

The private root package entry is exactly `"test:coverage": "bun scripts/coverage/run.ts"`. There are no JavaScript dependencies or new lockfile. Existing Rust 1.95.0 and Cargo dependency choices remain. mise pins Bun 1.4.4 and exposes separate `coverage:provision` and `test:coverage` tasks. `provision.sh` requests matching llvm-tools-preview and cargo-llvm-cov 0.6.21 with locked dependencies, then prepares the Cargo cache. The gate does not call provisioning. The selected release's compatibility with Rust 1.95.0 remains unverified until actual instrumentation succeeds.

The machine-readable and human-readable accounting contracts are `contract/accounting.json` and `contract/accounting.md`. They define six mandatory crates, the new 80% per-crate target, realpath source merging, production/test-only boundaries, zero-hit retention, explicit missing-data failures, platform limits and source/configuration hashes. `contract/source-inventory.json` classifies the current source files; `contract/preservation-references.json` binds protected bytes and existing Rust file/token hashes to accepted HEAD.

The root gate is wired for a fresh workspace/all-targets instrumented pass, existing integration/CLI subprocesses, PID-identifiable real binary probes and exact profile-merge input verification. Cargo-llvm-cov's broad default filename filters are disabled so `tests/../src` paths cannot hide production modules. Only exact dependency/compiler roots are filtered. These real-instrumentation branches have not run in this environment.

The six required test entry paths exist: `gate.test.ts`, `accounting.test.ts`, `coverage-cli.test.ts`, `preservation.test.ts`, `harness.test.ts`, and `evidence.test.ts`. The root gate does not recursively invoke acceptance tests. CLI acceptance executes the actual `bun run test:coverage`; harness acceptance executes the actual unchanged `./scripts/harness/validate.sh` with no skip flag. Native Cargo checks remain separate.

## Verified evidence

The controlled Bun gate/accounting/preservation suite passed 23 tests and 59 assertions. Fixtures use synthetic LCOV and subprocesses; they are not product coverage. They distinguish failed tests, absent/mismatched tools, missing/stale/malformed reports, source drift, missing crates/files and one failing crate despite an above-target workspace average. Exact 80% passes; 80000/100001 fails despite displaying 80.00%. Repeated compilation is unioned without denominator duplication, and zero-hit production lines remain. The final executable receipt is `tooling/fixtures-delivery.txt`.

Native `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` returned zero. The latter reported 247 passed, zero failed and zero ignored across 23 target/doc-test summaries, taking about 60 seconds. Exact commands/output/exits are in `tooling/fmt.json`, `clippy.json`, and `workspace-tests.json`. Production/config inputs used by Cargo were unchanged during these commands. This does not establish instrumentation coverage.

The unchanged harness executed against 3,082 tracked files. Its four noncoverage checks passed; coverage failed with one total violation because cargo-llvm-cov is missing. The first acceptance wrapper additionally detected implementation edits during its runtime; its failed receipt is retained, not promoted to a pass. A stable-input rerun is recorded separately. Neither a missing-tool failure nor a successful noncoverage check is a passing harness result.

## Prerequisite failures and recovery dependency

`cargo install cargo-llvm-cov --version 0.6.21 --locked` returned 101: Cargo could not write its registry cache (`Operation not permitted`). `rustup component add llvm-tools-preview --toolchain 1.95.0` returned 1: component download failed and rustup rolled it back. `mise install` returned 1 because it could not write the trusted-config symlink. `cargo fetch --locked --offline` returned 101 for uncached `anstyle-wincon v3.0.11`; online fetch also returned 101 with a cache-write access denial. Exact stderr is retained under `tooling/`. A process-list diagnostic was denied and was not bypassed.

Actual root invocations return 1 with `MISSING_TOOL cargo-llvm-cov` and explicitly report all six crates unavailable. The initial receipt and six null/unavailable summaries are in `baseline/initial-summary.json`; they are not a measured baseline. `baseline/runs/` retains actual prerequisite receipts and current source inventories. CLI acceptance and evidence acceptance consequently fail rather than skip. No raw LCOV or uncovered-line data was fabricated.

Chief recovery dependency: make the pinned prerequisites and locked Cargo cache available through supported provisioning in the integration environment, then rerun the real root command. Confirm tool compatibility and the first complete raw/summary/uncovered/profile artifacts before using measured gaps to drive the library/process Actors. Preserve the exact acceptance commands and threshold. Independent oma-qa review remains assigned to the separate reviewer; no independent-review artifact was manufactured here.

The harness scan itself took about 108 seconds without any Rust coverage execution. The fixed harness acceptance command has a 120-second limit; a real workspace coverage run can exceed the remaining margin. This is a measured scheduling risk for the Chief, not permission to alter the command, skip checks or change the harness. No passing runtime estimate is claimed.

## Acceptance checklist and remaining scope

- Implemented: pinned configuration, separate provisioning, dependency-free root entry, contracts, source inventory and all six test interfaces.
- Passed: controlled failure/accounting/preservation fixtures, Cargo fmt, Cargo clippy and Cargo workspace tests.
- Incomplete: successful prerequisite provisioning, real compatibility validation, first instrumentation pass, six measured crate summaries, per-file uncovered reports and child-profile merge proof.
- Failed as required on this environment: root coverage, CLI acceptance, evidence acceptance and full harness coverage check.
- Pending: library/process test expansion, integrated final acceptance and independent oma-qa review.
- desktop: incomplete
- native Windows: incomplete
- remaining Phase 0: incomplete

Skills used: oma-dev-workflow (tool pins, setup separation, task wiring and verification) and oma-backend with the installed backend-engineer execution protocol (existing Rust stack, bounded implementation and checks). No browser, GUI, Playwright, external delivery, publication, outreach, mission-state mutation or delegation was used.

Delivery source hash: `e1b5926c8a134fd5609dfc7d66524b2583442a3fd48ac3dbac6e5374958360f8`. The final root prerequisite receipt matches this hash. The stable harness receipt binds its own earlier source hash (recorded in its binding JSON); a later one-line LLVM regex syntax correction was downstream of the missing-tool failure and has not been exercised with instrumentation. Native Cargo checks bind unchanged accepted-baseline Rust source/configuration. No failed or earlier receipt is claimed as final integrated success.
