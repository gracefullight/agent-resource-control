# Rust coverage gate: final decision and handoff

Status: **completed for task-8; local acceptance passes**. The unchanged harness wrapper passed on this attempt: **246.466 seconds**, exit **0**. The coverage gate, same-source command receipts and independent task-7 QA meet the implementation requirements. Supervising framework verification runs after this handoff; its execution and final approval are not claimed here.

## Source and ownership

Accepted and current HEAD: `fbe76a5f15101ad415f355ef06f1deeb1f97d568`. Measured working-source/configuration SHA-256: `cf31468e23ac45fee3e5f04704e19d4b89f9b56365ef516b9daa9ebc6c9833d8`, across **114 inputs** in [the source manifest](integration/repair-4/source.json). This identifies uncommitted implementation bytes, rather than treating the accepted commit as containing the repair. Every manifest hash was checked against current files; live `inputIdentity` reproduced the aggregate. The harness binding confirms unchanged inputs before and after execution.

Task-8 changes only [final.md](final.md) and [final-summary.json](final-summary.json). The required test also generates its ordinary acceptance receipt, coverage run and mutable `baseline/latest.json`; these are verification outputs, not implementation edits. Preceding implementation, Director reports, independent QA and failed-attempt evidence remain intact. Production behavior, protocol, permissions and existing assertions remain compatible.

Skills: **oma-coordination** and **oma-docs**, using the installed `.agents/agents/docs-curator.md` definition. The existing graph, ownership and completed dependencies were reused. No new plan or recursive AV operation was started. Implementation used oma-dev-workflow; task-7 independent review used oma-qa.

## Acceptance evidence

| Success criterion | Result and evidence |
| --- | --- |
| Root Bun entry point, pinned tools, six independent 80% targets, unit/integration/CLI execution | **PASS.** Private package exposes `test:coverage` as `bun scripts/coverage/run.ts`; standalone and harness measurements below include instrumented child binaries. |
| Failed tests, missing tools, missing/stale reports and below-target results exit nonzero | **PASS.** Fresh gate/accounting tests and [18 independent driver probes](qa/driver-probes.json), each with an older passing run present. Malformed reports, missing crates/files and source drift also fail. Exact 80% passes; rounded display and a high workspace average cannot conceal a failing crate. |
| Preserve production behavior, existing assertions, migration and harness | **PASS.** Fresh preservation test, direct accepted-Git byte comparisons and [independent review](qa/review.md) of 77 pre-existing Rust files, production tokens and ordered assertions. |
| `cargo fmt --all -- --check` | **PASS.** Same-source exact-command receipt, revalidated. |
| `cargo clippy --workspace --all-targets -- -D warnings` | **PASS.** Same-source exact-command receipt, revalidated. |
| `cargo test --workspace` | **PASS.** Same-source exact-command receipt, revalidated. |
| Unchanged harness, zero violations and no skip flags | **PASS.** This attempt executed the exact saved `bun test scripts/coverage/harness.test.ts` wrapper, whose child ran the unchanged `./scripts/harness/validate.sh`. It asserted zero violations, coverage success, all six crate results and no skip diagnostics. |
| Source-bound measurements, uncovered lines, commands, versions, boundaries and independent review | **PASS for delivery.** Linked artifacts, current source checks and task-7 PASS/completed verdict are retained. The supervising framework replay remains the next stage. |

## Production-source coverage

**80% per crate is a new engineering target, not a measured baseline or SRS threshold.** Both retained required measurements report:

| Crate | Covered / production lines | Coverage | Result |
| --- | ---: | ---: | --- |
| arc-core | 64 / 64 | 100.00% | PASS |
| arc-detector | 577 / 578 | 99.83% | PASS |
| arc-storage | 226 / 227 | 99.56% | PASS |
| arc-ipc | 215 / 215 | 100.00% | PASS |
| arc-daemon | 1507 / 1613 | 93.43% | PASS |
| arc-cli | 203 / 213 | 95.31% | PASS |

- Standalone: [summary](baseline/runs/2026-11-08T00-43-49-396Z-e559836e-73d9-4c33-b202-cd7a23cb44cb/summary.json), [per-file uncovered lines](baseline/runs/2026-11-08T00-43-49-396Z-e559836e-73d9-4c33-b202-cd7a23cb44cb/uncovered.md).
- Harness: [summary](baseline/runs/2026-11-08T00-48-46-903Z-669089ee-9bd4-42bc-910e-4e1b9f574228/summary.json), [per-file uncovered lines](baseline/runs/2026-11-08T00-48-46-903Z-669089ee-9bd4-42bc-910e-4e1b9f574228/uncovered.md).
- This attempt: [fresh harness coverage summary](baseline/runs/2026-11-08T01-36-21-343Z-9776ab27-0462-4156-bab7-94aa7a4c7160/summary.json), [uncovered lines](baseline/runs/2026-11-08T01-36-21-343Z-9776ab27-0462-4156-bab7-94aa7a4c7160/uncovered.md), [actual child command receipt](tooling/acceptance/harness-124c2bfe-3335-4277-9d88-18ef99281fa6/00-command.json), [source binding](tooling/acceptance/harness-124c2bfe-3335-4277-9d88-18ef99281fa6/binding.json). Its per-crate targets also pass.

The two independently reviewed measurements cover **29 production files** and report **275 passing tests, zero failed and zero ignored**. Each retains native LCOV/LLVM JSON, source/input manifests, command receipts and 851 raw profiles. [Independent reconciliation](qa/reconciliation.json) checked all 1702 raw-profile hashes and merge inputs. Canonical file/line identities are unioned across repeated compilation; no binary averages or duplicate denominators are used. Zero-hit production paths remain. The omission audit covers 20 test-only source files and 54 spans proven inactive without `test`; platform predicates are not blanket exclusions.

CLI and daemon child profiles contribute actual production counters. Independent instrumented execution reaped five children; a child-only merge/export confirms positive daemon and CLI counters: [execution](qa/receipts/real-children.json), [merge/export](qa/receipts/real-child-merge.json). Test counts alone are not the evidence for subprocess collection. The recorded dependency-only Tokio profile warning did not cause a workspace coverage mismatch.

## Commands and tools

All five exact commands have zero-exit receipts bound to the current source. Task-7 independently checked them, and this attempt revalidated their hashes and contents:

| Exact command | Exit | Retained duration | Receipt |
| --- | ---: | ---: | --- |
| `bun run test:coverage` | 0 | 80.164 s | [receipt](tooling/acceptance/coverage-cli-f8af459c-a9cf-49ed-9c31-34138816c24e/00-command.json) |
| `cargo fmt --all -- --check` | 0 | 1.116 s | [receipt](integration/runs/repair-4-fmt-2026-11-08T00-45-17-805Z/00-command.json) |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 0.376 s | [receipt](integration/runs/repair-4-clippy-2026-11-08T00-45-18-928Z/00-command.json) |
| `cargo test --workspace` | 0 | 40.064 s | [receipt](integration/runs/repair-4-workspace-tests-2026-11-08T00-45-22-579Z/00-command.json) |
| `./scripts/harness/validate.sh` | 0 | 225.973 s | [receipt](tooling/acceptance/harness-c4da9be1-0b54-46d4-ba48-5c247c503f88/00-command.json) |

Fresh attempt-4 commands and outcomes are embedded under `final-summary.json#/attempt4`:

- `bun test scripts/coverage/harness.test.ts`: exit 0, **246.466 seconds**, 1000000 ms wrapper timeout. Child harness exits 0 with zero violations; no skip flags were supplied.
- `bun test scripts/coverage/gate.test.ts scripts/coverage/accounting.test.ts scripts/coverage/preservation.test.ts scripts/coverage/evidence.test.ts`: exit 0, **28 tests, 2667 assertions**, zero failures.
- Read-only `bun -e` evaluation of `inputIdentity`/`sourceInventory`, Python manifest/report/receipt/protected-byte checks and version commands: exit 0. All 114 inputs match.
- `oma docs verify --help`: exit 0. Manual local-reference/JSON/content checks and scoped `git diff --check` pass; the index-writing docs command was not run.

Directly reconfirmed: **Bun 1.4.4; Rust/Cargo 1.95.0; cargo-llvm-cov 0.6.21; matching LLVM 22.1.2**, with the matching rustup LLVM component installed. [Version evidence](integration/repair-4/versions.json). Provisioning is separate: `mise run coverage:provision`, including `cargo install cargo-llvm-cov --version 0.6.21 --locked`. Measurement uses locked/offline Cargo operations. Missing tools fail rather than trigger implicit installation. Existing Cargo dependencies/lockfile remain; no Bun dependency or dependency lockfile was added.

Protected files match accepted HEAD byte-for-byte:

- `scripts/harness/validate.sh`: SHA-256 `93140173df8b8bd28b067aff68301134b0398b643240b0b1e6a13da4487ebe33`.
- `crates/arc-storage/migrations/0001_phase0.sql`: SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`.

## Recovery and final verification

Earlier attempts genuinely timed out at 120 seconds; their receipts remain under `contractReplay` and `attempt3` in the structured report. Attempt 3 returned 124 after 120009 ms. Its attempted task-6 native handoff also failed before startup because the thread-store context was unavailable. Those outcomes are preserved as history.

The operator's 2026-11-08 framework-change note changes the environment: every contract command now has the mission's **1800-second per-check allowance**; frozen `timeoutMs: 120000` values are planning estimates. This attempt reran the affected check with a **1000-second wrapper**, without editing the saved contract, goalBrief, command, harness, tests or mission state. The wrapper passed. The earlier independent QA timeout handoff is therefore superseded by new execution evidence; its report has not been rewritten.

Task-8 marks its evidence/report delivery complete. The supervising framework will execute final verification after this handoff. No host-managed final receipt is available within this Actor turn, and no framework approval is invented. A subsequent actual failure must be handled by the existing owner under the saved budget. No new implementation blocker was found.

## Decisions, review and remaining work

The **Technical Director's supplied consultation** chose the existing Rust/Bun stack, one pinned cargo-llvm-cov tool, canonical unions and tests directed by measured gaps. These were delivered. Vitest, added package dependencies and six separate instrumentation passes were rejected. No numerical cost saving or model superiority was established.

The **Design Director's** [report](directors/design.md) supports plain terminal labels, six explicit counts and actionable [contributor documentation](../../docs/rust-coverage.md). HTML exploration, interactive setup and retention experiments were deferred. Personas are inferred. No contributor field study, diagnosis-time gain or usability improvement was measured. Its isolated-worktree raw-report path failure is historical; integrated-source checks and current evidence tests pass.

The **Marketing Director's** [report](directors/marketing.md) limits product claims to local diagnostics and literal command classification. Paid assessments, partner work, public-channel discovery, content, sponsorship and launch experiments remain deferred. Demand, pricing, conversion, retention, revenue, costs and ROI remain unknown. Unknown costs are not zero. This repair establishes no commercial outcome, broader orchestration capability or Phase 0 completion.

Actual independent task-7 QA is **PASS/completed**: [review](qa/review.md), [structured verdict](qa/review.json). It found no actionable implementation defect and independently assessed accounting, preservation, 18 failure probes and contributing child profiles. The supplied Technical Director review of task-7 also passed. Director advice and scoped reviews are not substituted for final framework execution.

Lessons: bind evidence to working bytes; measure canonical production lines; prove child-profile contribution; preserve failing receipts; rerun affected checks after an operator-reported environment change. Next action: supervising framework verification, followed by separately scoped work on unfinished obligations.

Coverage is for **Darwin arm64 / aarch64-apple-darwin**. Linux tooling admission is not an executed Linux result. Inactive non-macOS runtime/instance-root and Windows/XDG database branches in arc-core, Linux peer-credential/process-start paths in arc-daemon, and unsupported-platform fallbacks are inventoried but unmeasured. Stable Rust doctest coverage is outside this collection. Host percentages do not certify other targets.

Manual inspection — `oma docs verify` did not run because it regenerates the shared `docs/generated/doc-refs.json` outside this assignment. Before/after local-link counts and JSON/content checks are recorded in the summary; CLI drift counts are unavailable. The Design Director's 226 unrelated broken references remain historical and out of scope. Serena was absent; only known-path reads, Git metadata and evidence processing were used. Global Git cleanliness remains unverified because `av.yaml` access was denied; no bypass was attempted.

- desktop: incomplete
- native Windows: incomplete
- remaining Phase 0: incomplete

Acceptance checklist:

- [x] Every criterion maps to source-bound executable evidence and an accurate status.
- [x] Director advice, historical failures, independent QA and pending framework verification are distinguished.
- [x] A fresh passing harness replay resolves the timeout failure without weakening acceptance.
- [x] Incomplete obligations and unmeasured usability/commercial outcomes remain explicit.
