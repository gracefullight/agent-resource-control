# Task-6 TypeScript recognition repair

Status: completed for task-6 repair; current-source workspace checks passed. Owner: backend-execution, task-6 attempt 2. Skill: oma-backend, including the shared execution policy, backend execution protocol and checklist. This report supplements the material source/tests and preserves the preceding README and canonical mission evidence unchanged.

## Source scope

Accepted committed source baseline: `5578e69fc63ada7a73190f5681e7960da17969f7`; product tree: `888ddd6cbe9d02611b87ed98c8529882d8a0778b`.

The inspected Actor input snapshot and local HEAD are `13545024be1d25f2e205965e866a45d493910194`, tree `063a41532fa36ec20ee0e3d09dd6676a7cf31815`. Both objects were read back successfully. The supplied snapshot ref `refs/agent-valley/task-evidence/69a59d52a042fc13-98c4ca9fa7207111-2/snapshot` and worker ref `refs/agent-valley/task-evidence/69a59d52a042fc13-98c4ca9fa7207111-2/worker` are unavailable in this Actor checkout (`git rev-parse --verify ...^{commit}` exits 128). No ref was created, substituted or inferred. The repairs below are Actor-local uncommitted changes relative to that snapshot; they are not in the accepted committed baseline. Reviewers must inspect their integrated files and the delivery receipt.

The initial accessible index/worktree was clean. Unscoped status/diff inspection reported `av.yaml: Operation not permitted`; that path was excluded from subsequent checks and never read through another route. Serena discovery found no callable search/navigation tool. Inspection used scoped reads of the assigned and directly referenced files, plus Git change metadata; no substitute project code search or provider installation ran.

## Repair and interface compatibility

P1: the pure TypeScript grammar now rejects every response-file argument beginning with `@`, including option values and arguments forwarded through nested Bun scripts. Response files can change watch/noEmit behavior outside the bounded package snapshot. Analysis does not open them or execute a compiler. Pre/post lifecycle occurrences also make the entire graph UNKNOWN/OBSERVE.

P2: TypeScript has its own sequential option parser instead of the shared other-tool `--key=value` parser. It consumes optional separated boolean values and required separated option values, requires enabled `--noEmit`, and rejects unsupported spellings, missing/empty values and any explicit `--noEmit false`. Valid `--noEmit true`, `--strict false`, `-p tsconfig.json`, `--project tsconfig.json` and the actual desktop two-project chain remain supported. The preceding positive test for invalid equals forms is replaced by valid separated syntax; dedicated negative regressions retain the invalid forms.

The public detector/metadata APIs, frozen limits, classification/wire vocabulary, CLI execution owner and admission interfaces are unchanged. Only the detector production grammar changes. CLI production code, shared types/manifests/lockfiles, daemon/storage/desktop files, migrations, frozen contract, traceability and canonical reports are untouched. No ownership handoff or shared-file repair is needed.

## Acceptance checklist

- [x] Response-file bodies, quoted arguments, nested forwarding, Bun delimiters, option values and both lifecycle positions yield UNKNOWN/OBSERVE, with no script admission eligibility.
- [x] Invalid equals forms and missing option arguments bypass; separated boolean/project arguments remain TYPECHECK. Existing four-class, desktop delegation, graph-limit, cycle and lifecycle tests pass in the detector suite.
- [x] The CLI regression peer sees only UNKNOWN/OBSERVE `job.start` registrations for unsupported forms, no capability/admission request, and no additional request after exit. The original native argument bytes, including delimiters, are preserved; each fixture starts exactly once and returns 17.
- [x] The negotiated four-class fixture still preserves cwd, environment, stdio, signal/exit behavior and one original native child. Its TYPECHECK case now also covers separated boolean values and both project option spellings. Existing lost-ACK/unsupported-peer uncertainty, cancellation and bounded-outage cases pass in the focused CLI suite.
- [x] Required workspace checks complete on unchanged repaired inputs.
- [x] All five deliverables are explicitly staged, non-ignored and checked for whitespace.

## Check evidence

These are task-local checks, not the exclusive final native gate. Each subprocess check wrapper allows 7200 seconds, retaining the operator's per-check allowance. Only one owned Cargo check runs at a time; no subagents were dispatched. The framework's existing absolute mission deadline is retained (not exposed in the assignment), with the supplied maxDurationSec=172800, maxParallel=3, maxRuns=400 and maxRetries=3. Host observation: Darwin, 8 CPUs, 16 GiB RAM, approximately 64 GiB available storage; Rust/Cargo 1.95.0 and Bun 1.4.4. No tools/dependencies were installed.

TDD_EVIDENCE:

| Exact command | RED before production repair | GREEN after production repair |
| --- | --- | --- |
| `cargo test -p arc-detector --test script_resolution typecheck_` | Exit 101; both new tests fail because response-file and equals-form leaves incorrectly return TYPECHECK. | Covered by the full detector suite below. |
| `cargo test -p arc-cli --test traffic_scripts typecheck_` | Exit 101; both new tests fail because the first request is `traffic.bun-scripts.capabilities`, not observation registration. | Covered by the full CLI script suite below. |
| `cargo test -p arc-detector` | Not a RED selector. | Exit 0; 33 passed, zero failed/ignored; includes all 11 script-resolution tests. |
| `cargo test -p arc-cli --test traffic_scripts` | Not a RED selector. | Exit 0; 13 passed, zero failed/ignored, including the installed-Bun interpreter comparison. |
| `cargo fmt -p arc-detector -p arc-cli` | Formatting action. | Exit 0. |
| `cargo fmt --all -- --check` | Not a RED selector. | Exit 0. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Not a RED selector. | Exit 0; existing ts-rs attribute-parser notices do not fail Clippy. |
| `cargo test --workspace` | Not a RED selector. | Exit 0; 1333 passed, zero failed/ignored across 74 result blocks; elapsed 440.768 seconds. |

The exploratory `apps/desktop/node_modules/.bin/tsc --version` command failed with exit 127 because this isolated checkout has no compiler at that path. This repair relies on the supplied pinned-compiler defect observations and deterministic grammar/CLI assertions; it does not claim a new real TypeScript execution. Native tool prerequisites and real-tool acceptance remain downstream responsibilities.

Ignored logs under `target/task6-repair-*.log` are local diagnostics, not the handoff. Compact completed results and source hashes are retained here. The source hashes below were captured before the three workspace checks and matched again after each check. No production/test inputs changed during those checks. Final task-local receipt summary:

| Check | Exit | Elapsed seconds | Diagnostic log SHA-256 |
| --- | --- | --- | --- |
| Workspace formatting | 0 | 1.347 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| Workspace Clippy | 0 | 11.733 | `d12c4880d5ffa756167c49b4925e70ef00f5f382a2d9cf462f38ae28de982a20` |
| Workspace tests | 0 | 440.768 | `a3d4edda3d549acf46f6e596e951ee748af3c20e46f0e2810ba13904ee054bd6` |

## Repaired source bindings

| Actor-local repaired source/test path | SHA-256 |
| --- | --- |
| `crates/arc-detector/src/scripts/commands.rs` | `3728c3132f8fc7cbb55f0d6f926f1c650fbaea83636b762ab6ee76e50fb4771d` |
| `crates/arc-detector/tests/script_resolution.rs` | `7049dbb799f9b21588d9f852606163579cd0eeed2f7b90c05d1ba477659e8428` |
| `crates/arc-cli/tests/traffic_scripts/compatibility.rs` | `52775a4aed285c0cdac7b6f51f262c3baadd87e93d275f28902a08058b829798` |
| `crates/arc-cli/tests/traffic_scripts/execution.rs` | `dfb4050c25adcf2a423036b8b64466e742bd2438538431d0b5594285d8e8ded5` |

The report's fifth changed path is `crates/arc-cli/tests/traffic_scripts/repair-attempt-2.md`; it is outside the five canonical mission-evidence destinations, whose existing bytes remain unchanged. The preceding report is historical evidence of attempt 1, which was subsequently rejected for P1/P2; this repair does not rewrite its bytes or reuse its test results as current acceptance.

## Remaining integration obligations

Supported Bun-script slice only. No blocker remains within task-6 repair ownership. Real build/test/lint/typecheck output checks, production-daemon Bun-script contention/recovery, registered desktop bridge reconciliation for this new slice, all six exclusive final gates, coverage floors and independent QA remain with their assigned downstream owners. Coverage collections in this repair: zero. No final mission acceptance flag or traceability status is changed here. Package metadata can still change after observation/revalidation as documented in the frozen contract; this repair preserves that boundary and the original invocation.

Delivery consists of the four bound source/test files and this report. No commit, push, external delivery, mission-state mutation, new validator or canonical report update was performed. The canonical evidence present in this snapshot remains 286280 bytes of the 300000-byte budget.
