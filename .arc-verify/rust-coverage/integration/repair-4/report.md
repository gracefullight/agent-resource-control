# Task-6 repair attempt 4: completed for independent QA

The timeout repair is complete. Fresh source-bound receipts pass all five exact required commands and all coverage verification tests, including the real unchanged harness. Independent final QA and mission approval remain with task-7 and the Chief Director. Earlier failed attempts remain retained.

Accepted HEAD: `fbe76a5f15101ad415f355ef06f1deeb1f97d568`.
Source/configuration SHA-256: `cf31468e23ac45fee3e5f04704e19d4b89f9b56365ef516b9daa9ebc6c9833d8` (114 inputs).

## Change and ownership

The operator note recorded at 2026-11-08T00:42:45.775Z explicitly identifies the two timers as mission-owned constants and directs task-6/task-1 to raise them to at least 900000 ms. `scripts/coverage/acceptance.ts` now allows 900000 ms for its child, and `scripts/coverage/harness.test.ts` allows 960000 ms for the test. The receipt runner allows 1020000 ms. The 60-second outer margin permits child exit, receipt creation and test assertions before the outer test expires. No command, assertion, threshold or failure condition changed.

`source-change.json` proves these are the only changes among the 114 preceding integrated inputs. Task-3, task-4 and task-5 deliverables remain integrated in plan order; original authorship, delivery hashes and isolated-run evidence remain in the preceding integration records. Their production/test/documentation inputs are unchanged. No source-test or documentation ownership handoff was needed. Runtime dependencies, manifests, lockfiles and public interfaces are unchanged.

## Fresh command evidence

Commands below ran in the mission worktree, with stable before/after input hashes, no skip flags, no environment bypass and no test/report substitution. The source change invalidated earlier coverage receipts, so coverage was measured again. The standalone entry point and the harness each performed a separate actual workspace instrumentation run.

| Command | Exit | Duration | Receipt |
| --- | ---: | ---: | --- |
| `bun run test:coverage` | 0 | 80.164 s | `.arc-verify/rust-coverage/tooling/acceptance/coverage-cli-f8af459c-a9cf-49ed-9c31-34138816c24e/00-command.json` |
| `cargo fmt --all -- --check` | 0 | 1.116 s | `.arc-verify/rust-coverage/integration/runs/repair-4-fmt-2026-11-08T00-45-17-805Z` |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 0.376 s | `.arc-verify/rust-coverage/integration/runs/repair-4-clippy-2026-11-08T00-45-18-928Z` |
| `cargo test --workspace` | 0 | 40.064 s | `.arc-verify/rust-coverage/integration/runs/repair-4-workspace-tests-2026-11-08T00-45-22-579Z` |
| `./scripts/harness/validate.sh` | 0 | 225.973 s | `.arc-verify/rust-coverage/tooling/acceptance/harness-c4da9be1-0b54-46d4-ba48-5c247c503f88/00-command.json` |

Additional current wrapper checks:

| Check | Exit | Duration |
| --- | ---: | ---: |
| `cargo fetch --locked --offline` | 0 | 0.202 s |
| `bun test scripts/coverage/coverage-cli.test.ts` | 0 | 80.542 s |
| `cargo fmt --all -- --check` | 0 | 1.116 s |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 0.376 s |
| `cargo test --workspace` | 0 | 40.064 s |
| `bun test scripts/coverage/gate.test.ts scripts/coverage/accounting.test.ts scripts/coverage/preservation.test.ts scripts/coverage/evidence.test.ts` | 0 | 3.071 s |
| `bun test scripts/coverage/harness.test.ts` | 0 | 227.290 s |
| `bun test scripts/coverage/evidence.test.ts` | 0 | 0.785 s |

The real CLI test passed its 9 assertions. The real harness test passed all 10 assertions, including six crate verdicts, coverage completion, zero violations, preservation and no skip diagnostic. The controlled/accounting/preservation/evidence invocation passed 28 tests and 2667 assertions. Native workspace execution passed 275 tests, zero failed and zero ignored. Fixtures proved nonzero exits for failed tests, missing/mismatched tools, missing/stale/malformed reports, changed inputs, missing files/crates, below-threshold crates despite a passing average, and rounded-below-80% results. Fixtures assert the intended failure so unrelated prerequisites do not rescue their verdicts.

`commands.json` in this directory contains the current commands, relevant inherited environment, duration, process identity, source binding and tool versions. The parent `../commands.json` retains earlier rows and appends current wrapper and actual nested-command receipts. Actual command output, native reports, profiles and command receipts stay in separate run directories. No older passing report replaced a current failure.

## Measurements and platform boundaries

| Crate | Covered / production lines | Coverage |
| --- | ---: | ---: |
| arc-core | 64 / 64 | 100.00% |
| arc-detector | 577 / 578 | 99.83% |
| arc-storage | 226 / 227 | 99.56% |
| arc-ipc | 215 / 215 | 100.00% |
| arc-daemon | 1507 / 1613 | 93.43% |
| arc-cli | 203 / 213 | 95.31% |

Eighty percent per crate remains a new engineering target, not a measured baseline or SRS threshold. Production accounting unions canonical LLVM-mapped lines across repeated compilation instances; test-only spans are removed, every mapped zero-hit production line remains, and no difficult production path was excluded. All 29 production files are present with nonzero mapped denominators. Native LLVM JSON corroborates the LCOV line map. `production-inventory.json` records per-file covered/uncovered lines, mapped lines, omitted test spans, profile counts, child proof and platform boundaries.

Latest passing measurement: `.arc-verify/rust-coverage/baseline/runs/2026-11-08T00-48-46-903Z-669089ee-9bd4-42bc-910e-4e1b9f574228`. Its `raw.lcov`, `raw.json`, `summary.json`, `source-inventory.json`, `profiles.json`, `inputs.json`, `versions.json`, `receipt.json` and `uncovered.md` provide the complete accounting and command trail. Raw `.profraw` files remain under the recorded run-specific target path, with every size and SHA-256 checked by the evidence test. Both fresh measurements retain distinct explicit CLI and daemon PID profiles in the actual merge input list, alongside integration-suite child execution.

Host/target: `aarch64-apple-darwin`. Bun 1.4.4, Rust/Cargo 1.95.0, cargo-llvm-cov 0.6.21 and matching LLVM 22.1.2 tools passed current checks; see `versions.json`. `cargo fetch --locked --offline` passed before the native checks. No installation or download was needed. The existing setup and reproduction instructions remain in `docs/rust-coverage.md`.

Inactive platform code remains inventoried and unmeasured: arc-core non-macOS runtime/database paths, Windows LOCALAPPDATA, Linux daemon SO_PEERCRED and /proc parsing, and unsupported-platform fallbacks. These compiler-inactive paths are not counted as covered and have no new exclusion rule. The host measurement does not establish native Windows or desktop completion.

## Preservation and execution limits

`preservation.json` and the current preservation test confirm byte identity of `scripts/harness/validate.sh` and `crates/arc-storage/migrations/0001_phase0.sql` against accepted HEAD, ordered retention of original tokens/assertions in all 77 existing Rust files, and unchanged production tokens. `preservation-audit.md` describes the exact two-line repair and retained dependency audits. No production behavior, permission, protocol or interface changes were made.

`harness-scope.json` and `harness-tracked-paths.txt` record the complete 3807-file tracked scope before the real run. No alternate Git index, narrowed path selection, skip flag or shell replacement was used. Evidence generated after that scan is retained separately; the source/configuration hash remained unchanged throughout all checks.

The saved verification contract supplied to this Actor still lists an enclosing `timeoutMs: 120000` for `bun test scripts/coverage/harness.test.ts`. This task does not edit frozen mission/verification metadata. The successful real child and wrapper durations above exceed that value; the Chief should account for the actual runtime when the framework executes the unchanged command. This is a handoff about the enclosing runner, not a changed verification command or a claim that a 120-second process completed.

Original duration/run/retry/parallelism limits remain in force, without resetting mission state. Limits and host resources are recorded in `limits.json`. Checks ran in one task without new Actors; brief formatting/Clippy process-handle overlap did not overlap Cargo tests or instrumentation. Owned child exits are recorded by actual subprocess handles. Monetary execution cost is unknown.

Serena was not exposed in the tool inventory; inspection used supplied-path reads and Git metadata, with no native repository code search. Global Git status reports `av.yaml: Operation not permitted`; no bypass or global-cleanliness claim is made. No browser, screen, GUI, Playwright, build command, deployment, external delivery or mission-state operation was performed. Normal tests/lints compiled their own inputs.

Skill used: **oma-dev-workflow**, with shared execution policy, result contract, i18n guidance and repeated-run validation guidance. No OMA claim/run/session path was injected; none was invented. This implementer does not claim independent oma-qa; that remains task-7, followed by the Chief final judgment.

- desktop: incomplete
- native Windows: incomplete
- remaining Phase 0: incomplete
