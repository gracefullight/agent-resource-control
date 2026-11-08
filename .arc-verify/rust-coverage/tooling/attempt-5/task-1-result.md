# Task-1 recovery result

Status: partial. The coverage implementation and first real measurement are delivered. Full acceptance remains incomplete because the native workspace test reproduced the process-owned schema fixture failure and the unchanged harness timed out before coverage. Independent oma-qa remains assigned to the separate reviewer.

Accepted HEAD: `fbe76a5f15101ad415f355ef06f1deeb1f97d568`. Current source/configuration SHA-256: `23db85747b5e61c3637d93684e84aba95fd6c8bd5d924135541bc3c875859399`. The actual worktree HEAD still matches accepted HEAD; new work is staged, with no commit or mission-state mutation. Global Git status reported `av.yaml: Operation not permitted`; accessible scoped status is in `inspection.json`. Serena was absent from the available tool inventory; inspection used known paths and scoped Git metadata.

## Delivered implementation

The private root package keeps `test:coverage` exactly `bun scripts/coverage/run.ts`. Rust remains 1.95.0, matching llvm-tools-preview is configured, Bun is pinned to 1.4.4 and cargo-llvm-cov to 0.6.21. Separate explicit setup uses locked upstream Cargo dependencies. The gate installs nothing, rejects missing/mismatched tools, uses locked offline workspace dependencies, records fresh process-specific profiles and rejects old artifacts. No JavaScript dependencies, lockfile, frontend, Vitest or browser tooling was added.

Reused all 17 retained script bodies rather than regenerating or replaying the prior patch. The original retained worktree path now returns ENOENT; the current bodies were reconciled against the latest attempt-4 inventory and actual staged contents. This attempt changed five coverage scripts deliberately: `accounting.ts`, `accounting.test.ts`, `gate.ts`, `run.ts` and `evidence.test.ts`; the other twelve bodies retain their prior hashes. `delivery-comparison.json` verifies every nonempty script against worktree, index, delivery tree and reconstructed actual patch body, with byte lengths and SHA-256. Every coverage code file remains below 500 lines. The existing narrow `.gitignore` exceptions are preserved.

Two real-run defects were repaired. The pinned tool rejects `--no-clean` with `--no-report`; removing the redundant flag keeps the fresh target isolated. LLVM 22.1.2 also emits LF/LH from function-group summaries while DA represents file-wide line data. Rather than ignoring that discrepancy, the real gate requires a second native JSON export, verifies LF/LH against its summaries and every DA line/count against its ordered segments, and checks the exact file sets. Both raw exports remain untouched. Original standalone malformed-summary assertions remain strict and unchanged. The pinned LLVM source analysis and actual 74-record discrepancy are documented in `llvm-format-investigation.json` and the accounting contract.

## Source-bound measurement

Latest successful real run: `.arc-verify/rust-coverage/baseline/runs/2026-11-07T23-42-57-893Z-acb0fb6d-78cf-4df8-bfdc-c971e85992db`. Its instrumented workspace/all-targets suite passed 247 tests in 19 result groups, with zero failed, ignored or filtered tests. Unit, integration and real CLI/resident-daemon test paths are included. Stable-toolchain doctest coverage is not collected. The separate CLI acceptance invoked the exact real root command and passed.

| Crate | Covered/total production lines | Coverage |
| --- | ---: | ---: |
| arc-core | 64/64 | 100.00% |
| arc-detector | 572/578 | 98.96% |
| arc-storage | 214/227 | 94.27% |
| arc-ipc | 178/215 | 82.79% |
| arc-daemon | 1501/1613 | 93.06% |
| arc-cli | 200/213 | 93.90% |

Each crate independently passes 80% by integer counts. This is a new engineering target, not an SRS threshold or an earlier measured baseline. The first successful run is separately retained by `baseline/first-measurement.json`; daemon coverage varies by one hit line between successful runs without denominator changes. No production path was excluded to reach the target.

All 832 raw profiles were listed in the native merge input. Explicit probes identify actual CLI PID 29122 and daemon PID 29127, with nonempty matching profiles included in that merge. The daemon startup-rejection probe deliberately exits 1; existing integration tests separately exercise resident behavior. `profiles.json` contains exact names, sizes and hashes. The native reports, six crate summaries, canonical per-file line maps and `uncovered.md` are in the linked run.

Measured tools: Bun 1.4.4; rustc 1.95.0; Cargo 1.95.0 (f2d3ce0bd 2026-03-21); cargo-llvm-cov 0.6.21; matching LLVM 22.1.2. Host: aarch64-apple-darwin. Native tool checks, commands and outputs are retained in each run and `tool-versions.json`.

## Checks and command evidence

| Command/check | Exit | Seconds | Receipt |
| --- | ---: | ---: | --- |
| `mise install` (mise-install) | 1 | 0.648 | `mise-install.json` |
| `mise --verbose install rust@1.95.0 bun@1.4.4` (mise-diagnostic) | 1 | 1.832 | `mise-diagnostic.json` |
| `rustup component add llvm-tools-preview --toolchain 1.95.0` (llvm-tools) | 0 | 0.078 | `llvm-tools.json` |
| `cargo +1.95.0 install cargo-llvm-cov --version 0.6.21 --locked` (cargo-llvm-cov) | 0 | 64.506 | `cargo-llvm-cov.json` |
| `cargo +1.95.0 fetch --locked` (fetch) | 0 | 2.560 | `fetch.json` |
| `cargo +1.95.0 fetch --locked --offline` (offline-fetch) | 0 | 0.071 | `offline-fetch.json` |
| `cargo fmt --all -- --check` (fmt) | 0 | 0.346 | `fmt.json` |
| `cargo clippy --workspace --all-targets -- -D warnings` (clippy) | 0 | 0.271 | `clippy.json` |
| `bun test scripts/coverage/accounting.test.ts` (llvm-regression-red) | 1 | 0.169 | `llvm-regression-red.json` |
| `bun test scripts/coverage/gate.test.ts scripts/coverage/accounting.test.ts scripts/coverage/preservation.test.ts` (llvm-regression-green) | 0 | 1.993 | `llvm-regression-green.json` |
| `bun run test:coverage` (coverage) | 1 | 2.982 | `coverage.json` |
| `bun run test:coverage` (coverage-compatible) | 1 | 77.253 | `coverage-compatible.json` |
| `bun run test:coverage` (coverage-final) | 0 | 76.905 | `coverage-final.json` |
| `bun test scripts/coverage/coverage-cli.test.ts` (coverage-cli) | 0 | 78.149 | `coverage-cli.json` |
| `cargo test --workspace` (workspace-tests) | 101 | 6.485 | `workspace-tests.json` |
| `bun test scripts/coverage/gate.test.ts scripts/coverage/accounting.test.ts scripts/coverage/preservation.test.ts` (fixtures-final) | 0 | 1.956 | `fixtures-final.json` |
| `bun test scripts/coverage/evidence.test.ts` (evidence) | 0 | 0.197 | `evidence.json` |
| `bun test scripts/coverage/harness.test.ts` (harness) | 1 | 116.271 | `harness.json` |
| `bun test scripts/coverage/harness.test.ts` (harness-stable) | 1 | 116.821 | `harness-stable.json` |
| `bun test scripts/coverage/harness.test.ts` (harness-isolated) | 1 | 116.385 | `harness-isolated.json` |

The final gate/accounting/preservation run passed 27 tests and 87 assertions. Controlled subprocess fixtures distinguish failed tests, absent/mismatched tools, absent/stale/malformed reports, changed source, missing files/crates, per-crate failure despite a high average, exact 80% and rounded below-80% failure. The added LLVM fixture proves raw-summary differences need native corroboration and rejects missing zero-hit lines, changed DA hits, changed LF and invalid or missing native data. Evidence validation independently recomputed the current counts and raw hashes and passed 17 assertions. These implementation checks are not the separate independent oma-qa review.

TDD_EVIDENCE: command `bun test scripts/coverage/accounting.test.ts`; RED `llvm-regression-red.json` exited 1 at the valid LLVM-format fixture; GREEN `llvm-regression-green.json` exited 0 after the accounting repair. The original malformed-report fixtures continued to pass. The real incompatible command failure and corrected passing root command are retained separately.

## Remaining acceptance and ownership

The exact native `cargo test --workspace` returned 101: two tests failed at `crates/arc-cli/tests/schema.rs:57:67` with WouldBlock, then at line 73 joining the failed server thread. This is the product test defect named in the operator note, reproduced in the changed environment. Task-1 owns no Rust files and did not change or weaken any assertion. `chief-blockers.md` provides the process Actor the bounded-listener/accepted-stream-mode diagnostic lead. Do not treat the instrumented successes as a repair of this native failure.

The final isolated real harness invocation was `./scripts/harness/validate.sh`, without flags or coverage skips. Its source hash was unchanged and equals the current delivered source hash. It scanned 3,369 tracked paths and remained in check 1 at the fixed 115-second child limit. Actual harness exit: 143; wrapper exit: 1 after 116.385 seconds. It never reached coverage and did not establish zero violations. Receipt: `.arc-verify/rust-coverage/tooling/acceptance/harness-9ac75ca7-29f9-4b8e-84c1-5493ef5edb31`. The earlier concurrent stable-source attempt also timed out; the implementation-time source-drift attempt remains failure history. No harness bytes, timeout, operator command or acceptance requirement changed. No further identical retry is proposed. The harness parent was reaped according to its command receipt; no independent descendant process-tree claim is made.

The operator prerequisite note was honored: exact LLVM component, pinned locked Cargo installation, locked fetch and offline fetch now succeed. `mise install` still exits 1 on denied macOS cache writes under `/Users/gracefullight/Library/Caches/mise/`; installed required runtimes and separately provisioned tools enabled real measurement. The denied path was not relocated or bypassed.

Independent oma-qa, the process Actor repair, final native workspace success and a completed zero-violation fixed-budget harness remain for the Chief. The six explicit verification test paths exist and execute real commands where required; harness acceptance accurately fails. No mission-final report or QA PASS was fabricated in another Actor's owned paths.

## Preservation and interfaces

Production Rust, existing test assertions, Cargo.toml and Cargo.lock are unchanged. Protected harness SHA-256: `93140173df8b8bd28b067aff68301134b0398b643240b0b1e6a13da4487ebe33`. Migration SHA-256: `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. Both match accepted HEAD byte-for-byte. `preservation.json` and the passing preservation test provide evidence. Existing Rust APIs, CLI/IPC behavior and database interfaces remain compatible.

Changed paths are the owned root tooling files, the narrow retained ignore exception, `scripts/coverage/`, and owned `contract/`, `baseline/` and `tooling/` evidence under `.arc-verify/rust-coverage/`. Full path inventory is in `task-1-result.json` and the staged delivery comparison. Other Actors' deliverables and saved task/mission state were preserved.

Coverage is limited to active macOS compiler cfg. The contract inventories inactive Linux credentials and non-macOS/Windows runtime branches; Linux requires a separate native measurement. No platform source was removed.

- desktop: incomplete
- native Windows: incomplete
- remaining Phase 0: incomplete

Skills used: oma-dev-workflow and oma-backend with the installed backend-engineer definition. API/auth/schema-migration checklist items are inapplicable because those interfaces were not changed. No subagent, browser/screen check, Playwright installation, GUI workaround, publication, outreach or external delivery was used. Saved duration/run/retry/parallel budgets were retained rather than reset.
