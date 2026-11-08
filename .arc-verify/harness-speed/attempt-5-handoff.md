# Harness Engineer — attempt 5

Status: partial. The retained optimization is staged. The inherited evidence-hygiene requirements remain failed and prevent mission acceptance. No product repair is authorized by the latest finding: it confirms the inherited corpus problem and explicitly prohibits deleting historical evidence, filtering its audit or relaxing assertions in this assignment.

Skills used: oma-dev-workflow for task inspection and command verification; oma-refactor for source-bound behavior preservation. No subagent, browser, installation, build, external delivery or mission-state operation was used. Serena tools were unavailable; inspection used supplied known paths without native project code search. `mise tasks --all` succeeded and lists only `coverage:provision` and `test:coverage`; the frozen Bun verification commands have no corresponding mise tasks.

## Working tree and ownership

Inspection found twelve staged product paths and no unstaged product changes:

- `scripts/harness/validate.sh`
- `scripts/harness/benchmark.ts`
- `scripts/harness/coverage-verification.test.ts`
- `scripts/harness/equivalence.test.ts`
- `scripts/harness/evidence-budget.test.ts`
- `scripts/harness/fixtures.ts`
- `scripts/harness/performance.test.ts`
- `scripts/harness/test-support.ts`
- `scripts/coverage/config.ts`
- `scripts/coverage/hygiene.ts`
- `scripts/coverage/preservation.ts`
- `scripts/coverage/hygiene-preservation.test.ts`

This attempt changes no product file or index entry. It adds this report, `attempt-5-checks.json` and a diagnostic runner under ignored `target/harness-speed/`. Original task-1/task-2 reports and all prior failed-attempt evidence are preserved. The existing implementation report remains at `implementation.md`; this file supplies the current handoff without rewriting its prior contents.

Candidate SHA-256: `0e66d222504f469cc9be910e1c7a6e33544440f017809e25cc255f6dc90f29a8`. Working and staged implementation bytes agree. Cargo manifests/lockfile, package manifest, mise configuration and crate sources/migrations match HEAD. Scoped cached whitespace checks pass. `checkPreservation` and `checkExactPreservation` both pass, including all 84 exact protected entries. No Rust/migration, coverage-accounting or six-crate threshold amendment is made.

## Inherited failure and required coordination

The current audit reproduces **932** indexed `.arc-verify/rust-coverage/` entries and **457** prohibited indexed artifacts. Accepted HEAD `f85ae0430038cc3d7a6125f650872dbfce6b5529` contains the same 932 entries. Both canonical path/mode/OID manifests have SHA-256 `220c4fe94bc7183e56701454e9b8bae4d7d2dd3edbb0616816e12cd0e74ee934`.

`git diff --cached --exit-code f85ae0430038cc3d7a6125f650872dbfce6b5529 -- .arc-verify` exits 0. These indexed inputs predate the optimization. The complete unfiltered audit is retained in scratch, with its hash and location in `attempt-5-checks.json`.

The failed assertion is `scripts/coverage/hygiene.ts:70`: `rust-coverage count 932 must be <150`. The audit also records 457 prohibited entries that remain disallowed even if the count issue is resolved. Affected checks are:

- `scripts/coverage/hygiene.test.ts:11`, reached by `scripts/harness/coverage-verification.test.ts`.
- `scripts/coverage/clean-checkout.test.ts:15`, which stops before fresh-checkout collection assertions execute.
- `scripts/harness/evidence-budget.test.ts:36`, after the harness-speed budget and current receipt-binding checks.

Chief prerequisite: coordinate ownership and specific authorization to reconcile the inherited indexed evidence with the existing hygiene contract. This Actor cannot delete historical deliverables or change audit accounting under the saved assignment. After that coordinated change, rerun these unchanged checks and the required subsequent verification. A passing speed run or root coverage collection cannot clear these failures.

## Retained performance and interface evidence

`timing.json` remains the passing original-plus-three-candidate full measurement receipt. Its current candidate, test-support, benchmark and performance-test bindings match. The only current source difference in that older broad inventory is `equivalence.test.ts`, whose later trace-counter correction is documented in the retained implementation report. No performance-relevant source changed in this attempt, so the measurements were not repeated to investigate an unrelated inherited audit failure.

| Run | Checks 1–4 s | Check 5 s | Total s | Exit |
|---|---:|---:|---:|---:|
| original | 159.002406 | 94.632839 | 254.360494 | 0 |
| candidate-1 | 14.476503 | 79.759842 | 95.024124 | 0 |
| candidate-2 | 14.485945 | 80.953849 | 95.962326 | 0 |
| candidate-3 | 14.733389 | 80.745241 | 96.020242 | 0 |

Those serial full runs used real coverage without skip flags, fresh accepted snapshots and scan-boundary corpus checks. Each candidate meets the strict 20-second scan requirement. Startup, conservative start-to-Check-5 and finalization are recorded separately in the receipt.

The production interface remains the self-contained Bash 3.2 harness and native matching engine. No new dependency is introduced. The final preserved implementation retains ordered parent replay, separate force-push detection/reporting, original domain filtering and native newline counting. Independent oma-qa remains assigned to the existing reviewer; this report is an implementation handoff, not an independent review or completion claim.

## Current checks

Every child used the 3,600,000 ms allowance bounded by this attempt's absolute deadline. Commands ran serially, without a competing benchmark or coverage collection.

| Command | Exit | Seconds | Result |
|---|---:|---:|---|
| `bun test scripts/harness/equivalence.test.ts` | 0 | 116.204 | 79 equivalent cases; native invocation scaling passes |
| `bun test scripts/harness/coverage-verification.test.ts` | 1 | 16.970 | inherited hygiene failure; every listed child executed |
| `bun test scripts/coverage/clean-checkout.test.ts` | 1 | 1.275 | inherited hygiene failure before fresh collection |
| `bun test scripts/coverage/evidence.test.ts` | 0 | 0.681 | 2,580 assertions verify current native coverage evidence |
| `bun test scripts/harness/evidence-budget.test.ts` | 1 | 1.311 | 113 assertions reached; inherited hygiene audit fails |

Individual non-collecting coverage results: accounting **12 pass**; gate **14 pass**; hygiene-preservation **3 pass**; preservation **1 pass**; hygiene **3 pass / 1 fail**. The complete nine-file inventory is checked by the aggregate, with the four collecting/evidence files separately required. No check was omitted to obtain the aggregate outcome.

Fresh native grep traces show original/candidate calls of **38/20** for 2 files and **1,520/20** for 80 files. All 79 fixture cases pass separate byte comparisons for stdout/stderr, exact exits and coverage-call equality. Current receipts are `equivalence.json`, `invocations.json` and `coverage-verification.json`; their prior bytes were preserved in the scratch location bound by `attempt-5-checks.json`.

The earlier source-bound Cargo formatting, Clippy, workspace-test, coverage-CLI and full unskipped harness passes remain recorded in `option-repair-checks.json` and `option-repair-finalchecks.json`. They were not rerun in this attempt because their inputs did not change and the latest finding concerns only inherited hygiene. Fresh `evidence.test.ts` still verifies the current retained native collection. The fresh-clean-checkout assertions remain unexecuted and required; they are not represented by the root evidence pass.

Final evidence count: **26 files**, including untracked and nested harness-speed evidence, below 150. The separate inherited Rust coverage evidence budget still fails. Product source identity is unchanged across this attempt. All owned command sessions exited, and scratch logs/audits remain retained. The diagnostic runner's successful bookkeeping exit does not override its three recorded failed checks.

Readability and performance verdict: the retained batching implementation is unchanged; prior behavior and measured speed evidence remain applicable, with fresh equivalence and preservation results above. The new deliverable makes the unresolved prerequisite explicit. Hand off to the Chief and the existing independent oma-qa task with **partial acceptance**, without changing mission state or requesting an operator implementation choice.
