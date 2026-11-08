# Harness Engineer — attempt 6

Status: task-3 recovery passed; ready for the saved independent review. Mission-wide completion remains with the Chief and tasks 4–6. No independent verdict, qa.md, final.json or mission-state change is written by this Actor.

Skills used: oma-dev-workflow for recovery and exact serial command execution; oma-refactor for source and behavior preservation. Serena tools were unavailable; all source inspection used supplied known paths. No browser, subagent, package installation, production build or external delivery was used. The required Bun commands have no matching mise task; mise tasks --all confirms the existing two-task configuration.

## Changes and authorization

The operator authorization recorded at 2026-11-08T07:55Z supersedes attempt-5's coordination request. The actual enumerated index had 932 Rust coverage paths and 457 prohibited artifacts. The whitelist was derived from retained[].destination in .arc-verify/evidence-hygiene/retention.json, with 24 unique destinations verified present.

Nine bounded explicit git rm --cached --ignore-unmatch -- invocations untracked exactly 908 non-whitelisted paths. All prohibited paths were among those removals; none outside rust-coverage required a further removal. No file was deleted from disk. The exact removed path/mode/OID manifest is retained at target/harness-speed/attempt-6/removed-manifest.json with SHA-256 5a400edd202c5b2e554a0745ede26e8bcfab7ea4641c713b0a5cfe6c2ba7dbcc. Its full identity and before/after audits are in attempt-6-recovery.json.

After verification, all 908 removed local files still match their pre-operation sizes, modes and SHA-256 hashes. All 24 retained index entries and local files are unchanged. Comparing the full index to the saved pre-operation index minus exactly the enumerated removals confirms unrelated index changes were preserved. The final audit reports 24 Rust coverage files, 475,720 bytes and zero prohibited indexed artifacts. Exact Rust/migration preservation and harness structure checks pass for all 84 protected entries.

The immutable accepted commit f85ae0430038cc3d7a6125f650872dbfce6b5529 and its 4,044-entry corpus remain unchanged (Git manifest SHA-256 cc9e627ec4cc9f2ab8fa41b8be66e6ca285f64cf48729600d7aac587da955344). Index cleanup affects delivery only; no benchmark was rerun against a smaller corpus. Historical receipts and accepted task-1/task-2 deliverables were not edited.

This attempt changes only the evidence index and recovery documentation. The twelve existing staged implementation files remain unchanged:

- scripts/harness/validate.sh
- scripts/harness/benchmark.ts
- scripts/harness/coverage-verification.test.ts
- scripts/harness/equivalence.test.ts
- scripts/harness/evidence-budget.test.ts
- scripts/harness/fixtures.ts
- scripts/harness/performance.test.ts
- scripts/harness/test-support.ts
- scripts/coverage/config.ts
- scripts/coverage/hygiene.ts
- scripts/coverage/preservation.ts
- scripts/coverage/hygiene-preservation.test.ts

Working/index bytes agree for all twelve. New durable outputs are attempt-6-recovery.json, attempt-6-checks.json, attempt-6-handoff.md and the generated budget.json; implementation.md is appended with this recovery. Prior report/coverage-verification receipt bytes and full manifests/diagnostics remain in target/harness-speed/attempt-6/. Previous failed receipts and attempt-5-handoff.md remain intact.

## Current command evidence

Each wrapper permits 3,600,000 ms per check. The following commands ran serially, with no benchmark or competing coverage collection:

| Command | Exit | Seconds |
|---|---:|---:|
| `bun test scripts/coverage/hygiene.test.ts` | 0 | 0.815 |
| `bun test scripts/harness/coverage-verification.test.ts` | 0 | 15.080 |
| `bun test scripts/coverage/clean-checkout.test.ts` | 0 | 86.445 |
| `bun test scripts/harness/evidence-budget.test.ts` | 0 | 0.725 |

The hygiene suite passed 4 tests and 214 assertions. The non-collecting aggregate passed and ran every listed child: accounting, gate, hygiene-preservation, hygiene and preservation. Its nine-file inventory still includes the four separately required coverage CLI, clean-checkout, integrated harness and native-evidence files. The budget test passed 119 assertions.

Clean-checkout passed 31 assertions. It materialized index tree f25af4f19afa14a9208ad109795490a6cf0cfaf1, verified no inherited targets/runs/profiles/latest pointer or raw exports, and matched source identity 91b009125526827cd072e922cb1cafcede1ec5d6b2a09ee16dba0ab98dbd0379. It then executed bun run test:coverage and bun test scripts/coverage/evidence.test.ts in the isolated checkout; both exited 0. All six crate results passed their independent exact 80% floors. Current native LCOV/LLVM hashes, crate accounting, child receipts and diagnostics are linked in attempt-6-recovery.json. Fresh checkout: /var/folders/r9/0qvpcx794n385zwgzwfvt55h0000gn/T/arc-clean-coverage-KO8gci. Full child proof: /Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/92ab1985-9682-4e04-a416-1f3ab9a6bc3a/workspace/target/evidence-hygiene/clean-checkout/runs/f59c0ac9-443b-4537-a66a-00b4ee63dc42/result.json.

One scratch report-finalizer invocation exited 1 with a parse error before executing; its extra closing parenthesis was corrected, then the unchanged final checks and report generation completed. No product/index mutation occurred in the failed invocation. Additional checks passed: bash -n scripts/harness/validate.sh; scoped git diff --cached --check; final complete index comparison; local file hash/mode comparison; protected-source preservation; working/index equality; current source binding checks for equivalence.json, invocations.json and timing.json.

## Preserved equivalence and timing evidence

No production/test/helper input changed in this attempt. Current relevant source bindings were verified before reusing the passing 79-case exact stdout/stderr/exit fixture receipt, native grep scaling receipt (38/20 calls at 2 files; 1,520/20 at 80 files) and full accepted-corpus timing receipt. Candidate SHA-256 remains 0e66d222504f469cc9be910e1c7a6e33544440f017809e25cc255f6dc90f29a8.

| Run | Checks 1–4 s | Check 5 s | Total s | Exit |
|---|---:|---:|---:|---:|
| original | 159.002406 | 94.632839 | 254.360494 | 0 |
| candidate-1 | 14.476503 | 79.759842 | 95.024124 | 0 |
| candidate-2 | 14.485945 | 80.953849 | 95.962326 | 0 |
| candidate-3 | 14.733389 | 80.745241 | 96.020242 | 0 |

The three candidate scans remain below 20 seconds. Startup, conservative process-start-to-Check-5 and finalization remain separately recorded in timing.json. Real native coverage ran without skip flags in every measurement. Bash 3.2/native grep interfaces, ordering/counting, original patterns/exclusions, option/error/binary behavior and the separate force-push expressions remain covered by the unchanged matrix. No dependency, Rust source, migration, manifest, lockfile, accounting/collection logic or threshold changed. Earlier Cargo formatting, Clippy, workspace tests, coverage CLI and integrated unskipped harness passes remain in option-repair-checks.json and option-repair-finalchecks.json for their unchanged inputs; they are not represented as freshly rerun here.

## Pre-existing hygiene failures and remaining work

The prior failures were caused by the inherited snapshot's 932 indexed coverage artifacts, including 457 prohibited artifacts. They are preserved as historical failed evidence in earlier receipts and are now resolved by the authorized index removals and fresh passing checks. This attempt reproduced no remaining check failure or ownership/interface conflict.

The evidence directory contains 30 files including untracked and nested evidence, below 150. The standalone budget receipt counts the complete current set. All observed owned command sessions exited.

Readability verdict: the existing batching implementation is unchanged; this recovery adds no scanning complexity. Metric delta for this attempt: indexed Rust coverage evidence 932 → 24; prohibited indexed artifacts 457 → 0; retained local historical files deleted 0.

Chief handoff: task-3's inherited hygiene blocker is resolved. Continue the existing independent oma-qa and tasks 4–6 with their original checks and 3,600,000 ms allowance. Remaining acceptance risk is the unperformed independent/integrated final review; this implementation handoff does not claim that review or mission completion.
