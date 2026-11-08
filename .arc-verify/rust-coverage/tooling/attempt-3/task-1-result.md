# Task-1 attempt 3 result

Status: partial / blocked. The malformed-LCOV false pass is repaired and actual-content delivery capture is verified. The first real measurement remains unavailable. No mission completion or independent QA approval is claimed.

Accepted HEAD remains `fbe76a5f15101ad415f355ef06f1deeb1f97d568`. Current source/configuration hash: `ecd38f7d15137942dbdee81dc996386cb4f8347ca8644cdcf62c1de9b2b28027`. `source-binding.json` records actual input hashes and the source inventory; the fresh failed run in `baseline/latest.json` has that binding. The workspace retains Rust/Bun, with no JavaScript dependencies or new lockfile.

## Changed files and interfaces

This attempt changed `scripts/coverage/accounting.ts`, `accounting.test.ts`, `gate.test.ts` and `fixtures.ts`, and synchronized `contract/accounting.md` and `contract/accounting.json`. Each LCOV record now requires exactly one nonnegative safe-integer LF/LH summary matching its unique DA count and positive-hit count. Duplicate DA entries, duplicate/missing summaries, nonnumeric/unsafe values, inconsistent counts and summaries outside a record fail before canonical union, including test-only records. A later valid instance cannot rescue a malformed earlier denominator. Valid repeated-instance union, zero-hit retention, production/test boundaries and integer 80% comparisons remain intact.

The retained private root package still maps test:coverage to exactly `bun scripts/coverage/run.ts`. Pins remain Rust 1.95.0, Bun 1.4.4, cargo-llvm-cov 0.6.21 and matching llvm-tools-preview. Separate provisioning retains locked dependencies. The root gate never installs tools or recursively invokes its real CLI/harness acceptance tests. All six crate identities and acceptance interfaces remain compatible; valid synthetic LCOV now includes LF/LH summaries. Real tooling compatibility is still unverified.

`reconciliation.json` confirmed all 17 script bodies matched both the retained workspace and prior recorded inventory before editing. `delivery-comparison.json` records current byte lengths and SHA-256 values for worktree, index and delivery tree; it also reconstructs every script's actual added-file body from `delivery.patch`. All 17 are nonempty and match actual index/tree/patch contents. Four script bodies changed deliberately; thirteen retain their original hashes. Every coverage code file is within 500 lines. No intent-to-add capture is used. The existing narrow `.gitignore` exception remains; no other ignore rules changed this attempt.

## Commands and outcomes

| Command | Exit | Duration | Receipt |
| --- | ---: | ---: | --- |
| `rustup run 1.95.0 cargo llvm-cov --version` | 101 | 0.078s | `tool-availability.json` |
| `rustup component list --toolchain 1.95.0 --installed` | 0 | 0.030s | `components.json` |
| `rustup run 1.95.0 cargo fetch --locked --offline` | 101 | 0.120s | `offline-fetch.json` |
| `bun test scripts/coverage/gate.test.ts scripts/coverage/accounting.test.ts scripts/coverage/preservation.test.ts` | 0 | 1.959s | `fixtures-green.json` |
| `bun run test:coverage` | 1 | 0.434s | `coverage.json` |
| `bun test scripts/coverage/coverage-cli.test.ts` | 1 | 0.228s | `coverage-cli.json` |
| `cargo fmt --all -- --check` | 0 | 0.389s | `fmt.json` |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 0.366s | `clippy.json` |
| `cargo test -p arc-cli --test schema` | 101 | 1.106s | `schema.json` |
| `cargo test --workspace` | 0 | 50.170s | `workspace-tests.json` |
| `bun test scripts/coverage/harness.test.ts` | 1 | 116.448s | `harness.json` |
| `bun test scripts/coverage/evidence.test.ts` | 1 | 0.034s | `evidence.json` |

The new regression command `bun test scripts/coverage/gate.test.ts scripts/coverage/accounting.test.ts` first returned 1 against the old implementation, with both new regression cases failing. Its controlled child falsely exited zero for inconsistent LF/LH; `regression-red.json` preserves the proof. After repair, gate/accounting/preservation passed 26 tests and 78 assertions. The controlled cases distinguish failed subprocess tests, absent/mismatched tools, absent/stale/malformed reports, source drift, missing crates/files, exact 80%, rounded below-80% and one under-target crate despite a high workspace average. These are synthetic fixtures, not product measurements.

Native formatting and Clippy passed. `cargo test --workspace` passed 247 tests across 23 result groups with no ignored or filtered tests. The separately executed `cargo test -p arc-cli --test schema` failed at `crates/arc-cli/tests/schema.rs:57:67` with WouldBlock (six passed, one failed). The subsequent passing workspace execution does not repair that intermittent failure. The process-test Actor owns this file; task-1 edited no Rust tests or production crates. `chief-blockers.md` gives exact evidence and the accepted-stream-mode diagnostic lead without presenting an unverified fix as proven.

The root command and real root CLI acceptance returned 1 because cargo-llvm-cov is absent. Current component checks also confirm missing llvm-tools-preview; locked offline fetching lacks anstyle-wincon 3.0.11. Prior exact installation receipts remain in `../recovery/`: Cargo-cache access was denied and LLVM component download/cache setup failed. No operator note reports an environment repair. This attempt rechecked current prerequisites and obeyed the ban on repeating unchanged denied installations. It did not relocate caches, change pins, implicitly install tools, bypass permissions or use GUI tools. The Chief must resolve supported provisioning access; see `chief-blockers.md` for exact paths and commands.

The unchanged harness executed exactly `./scripts/harness/validate.sh`, with no skip flags. It scanned 3,206 tracked files, passed checks 1–3 and reached check 4 before the retained 115-second child limit terminated it. Actual harness exit: 143; wrapper exit: 1 after 116.448 seconds. It did not reach coverage or establish zero violations. The real-command receipt and equal before/after source hashes are in `tooling/acceptance/harness-6d5b81a6-77e2-4f73-9bd6-cfc39fb80e11/`. No timeout, verification command, skip behavior or harness bytes changed. A follow-up exact-parent `ps` query was denied (Operation not permitted); `harness-termination.json` records that limit. The command receipt confirms parent exit; independent process-tree/liveness inspection was unavailable, and no workaround was attempted. Completion within the fixed 120-second contract remains unresolved.

Evidence acceptance correctly returns 1 because latest.pass is false. `baseline/attempt-3-summary.json` contains six unavailable/null results rather than invented percentages or 0% measurements. No real raw.lcov, measured per-file uncovered lines or child-profile merge evidence was produced. Keep measurement-driven dependents blocked until required prerequisites and artifacts exist. A real below-target initial measurement remains a failure and can guide the test owners; unavailable data cannot satisfy that acceptance criterion.

## Preservation and acceptance checklist

`inspection.json` confirms no accepted-HEAD diff for Cargo.toml, Cargo.lock, production crates, existing Rust tests or the harness. The passing preservation test verifies production tokens, original Rust tokens/assertions and both protected files. Harness SHA-256: `93140173df8b8bd28b067aff68301134b0398b643240b0b1e6a13da4487ebe33`; migration SHA-256: `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. Both are byte-identical to accepted HEAD. Global status reported `av.yaml: Operation not permitted`; no bypass occurred. Accessible scoped status is recorded. Serena was unavailable, so code inspection used known-path reads and Git metadata rather than code-search fallback.

- Passed: malformed-summary repair, deterministic gate/accounting/preservation checks, actual-content delivery capture, native fmt/Clippy and current workspace tests.
- Retained: exact pins, separate provisioning, full six-crate accounting contract, all six explicit acceptance-test paths, fresh source binding and child-merge validation implementation.
- Incomplete: supported provisioning, offline dependency readiness, real tool compatibility, first production measurement, per-file uncovered lines, child-profile merge evidence, repeatably passing schema fixture, real CLI/evidence acceptance, fixed-budget zero-violation harness, independent oma-qa and final mission verification.
- desktop: incomplete
- native Windows: incomplete
- remaining Phase 0: incomplete

Skills used: oma-dev-workflow for pins, separate setup, recovery, task wiring and command evidence; oma-backend and the installed backend-engineer definition for input validation, Rust boundaries and preservation. API/auth/database checklist items are inapplicable: no API or migration changed. No other Actor deliverable, saved instruction, acceptance criterion, assignment or mission state was edited. No new Actor, browser, screen check, Playwright, external distribution, publication or outreach was used. Saved 86,400-second, 200-run, three-retry and max-parallel-three limits remain; no budget reset occurred. Historical failed receipts remain intact.
