# Contributor coverage documentation and interface review

Status: completed for task-5 documentation. Overall mission acceptance remains incomplete. Skill used: oma-docs with the installed docs-curator definition. No subagent was dispatched.

## Scope and provenance

Owned deliverables are [the contributor guide](../../../docs/rust-coverage.md), [the root coverage link](../../../README.md) and this report. README.md and the guide did not exist at intake. No gate code, Rust source, tests, manifests, lockfiles, protected files or mission instructions were edited.

Accepted committed baseline: `fbe76a5f15101ad415f355ef06f1deeb1f97d568`; product tree: `8db02719b5bd06d888729ca337f9bf8887991515`. This Actor inspected local snapshot/HEAD `c891e004d07bb563d6e3cd705b7630f914a16971`, tree `23e3dd57a4929d7c30e17472121d015b3ea95f83`. The dependency implementation is present in this snapshot; it is not thereby part of the accepted baseline or proof of its integration into another checkout. The three documentation files are new Actor-local worktree changes after that snapshot. No commit was made.

Inspected implementation paths: root package.json, mise.toml and rust-toolchain.toml; scripts/coverage/config.ts, provision.sh, run.ts, commands.ts, gate.ts, accounting.ts, source.ts, gate.test.ts, accounting.test.ts, preservation.test.ts, evidence.test.ts and coverage-cli.test.ts. Serena was not available in the tool inventory; these were known-path reads, not repository code searches. Scoped Git status showed no preceding changes in the three owned paths. Global Git status reported `av.yaml: Operation not permitted`; global cleanliness is unverified, and the denied file was not accessed by another route.

The implementation input fingerprint recomputed in this worktree is `23db85747b5e61c3637d93684e84aba95fd6c8bd5d924135541bc3c875859399` across 109 files. It matches the supplied measurement's input hash. Documentation is outside that fingerprint. This equality establishes matching implementation bytes, not path portability or a fresh local coverage run.

## Delivered contributor journey

The guide gives the explicit `mise run coverage:provision` setup, its four actual underlying commands and exact Bun 1.4.4, Rust/Cargo 1.95.0, cargo-llvm-cov 0.6.21 and matching LLVM 22.1.2 pins. It explains that setup is separate from measurement, installation uses locked dependencies, and coverage uses offline Cargo operations. It records the first-failure behavior of the provisioning script rather than implying later setup commands ran after a mise failure.

Repeat use is: run `bun run test:coverage`; read the six named results and covered/total counts; open the printed run directory's uncovered-line report; repair and rerun. The root link makes that journey discoverable. The guide distinguishes unavailable measurement from valid below-target results, and exact integer acceptance from rounded display. Eighty percent is labeled a new engineering target, not a measured baseline or SRS threshold.

The report inventory covers native raw exports, source/configuration hashes, per-file maps, raw profiles and child PIDs, exact command receipts, tool versions and failed-attempt receipts. Canonical file/line union, test-only omission, missing-file rejection, zero-hit retention and native JSON corroboration match the delivered accounting contract. No interim percentage is presented as a final project result.

Platform text identifies the measured macOS host and native target, the gate's Linux admission versus unmeasured Linux execution, inactive source branches, Unix-socket limits and uncollected stable-toolchain doctest coverage. The three incomplete obligation groups remain explicit below.

## Actual text interface

Inspected behavior and historical observations:

- The exact root entry point is `bun scripts/coverage/run.ts`, exposed as `bun run test:coverage` by the private package. The source prints three stages, then six crate labels in stable order with covered/total production lines and two-decimal percentages, followed by a report directory. There is no wizard, prompt, spinner or color-only verdict in this driver.
- The supplied [successful command receipt](../tooling/attempt-5/coverage-final.json) records that actual root command, exit 0, the three stages, all six `PASS` labels and a report path. This was a prior run in the dependency's parent workspace, not a new command execution in this Actor's isolated worktree. The [task-1 report](../tooling/attempt-5/task-1-result.md) separately records real integration/CLI acceptance. The mutable [latest pointer](../baseline/latest.json) identifies a later successful dependency run; its directory and native reports were inspected here.
- The run source distinguishes `MISSING_TOOL`, `TOOL_VERSION`, `TEST_FAILURE`, missing/stale reports and `BELOW_THRESHOLD`, while the catch prints a common failure heading and evidence path. Before a valid measurement, all six crates say `UNAVAILABLE`; after valid below-threshold accounting, actual counts and `FAIL` labels remain visible. Exact threshold decisions use BigInt counts, not printed rounding.
- Deterministic subprocess fixtures executed here check missing tools, failed tests, absent/stale/malformed reports, source drift, missing files/crates and below-threshold results with an older passing report present. They assert specific diagnostics and nonzero exits. These are synthetic process/report checks, not six-crate production measurements or observed contributor sessions.
- Test output is captured in numbered JSON receipts. The terminal does not stream each test result or print an individual remediation sentence for every failure. The guide supplies the diagnostic-to-action mapping and explains where captured output lives. The driver supplies an initial setup hint and failure evidence path; a contextual rerun reminder is documented, not claimed as newly implemented terminal behavior.

## Alternatives and usability hypotheses

| Approach | Assessment against the actual implementation |
| --- | --- |
| Plain terminal results plus files | Delivered. Stable labels, counts and a report path support the required unattended one-command journey. Hypothesis: contributors can locate failed crates and repair actions with fewer unnecessary commands. No task-time improvement has been measured. |
| HTML coverage explorer | Could make long uncovered-line lists easier to browse. That is an untested hypothesis; no explorer was added or required. Existing per-file text and JSON reports satisfy this assignment. |
| Interactive setup/remediation wizard | Could explain prerequisites incrementally, but would add choices to repeat use and complicate automation. These are design hypotheses. No wizard or prompt was introduced. |
| Neutral failure text with contextual recovery | The guide maps each actual diagnostic to a next action and a rerun. Hypothesis: this reduces mistaken success judgments while preserving direct exit. The driver was not changed. |
| Guilt or streak-loss retention text | Not introduced. More invocations would not establish correct repair, usability or retention. No conversion, churn or retention experiment was run. |

The intended reader is a repository contributor or reviewer, inferred from the assignment and command contract. No recruited users, field tests, screen checks or simulated participant outcomes were used. A later authorized formative study could ask actual contributors to locate the command, interpret a below-target result and recover from a stale report. Completion, time to first correct action, unnecessary commands and mistaken success judgments would test the hypotheses; this proposal adds no acceptance gate.

## Discrepancies and handoff to the Chief

1. **Observed local evidence portability failure; tooling owner action required.** `bun test scripts/coverage/evidence.test.ts` exits 1 in this isolated worktree at accounting.ts:63, called by evidence.test.ts:19, with `UNMAPPED_FILE: ../../../../crates/arc-cli/src/classify.rs`. The copied raw LCOV/JSON reports name absolute source paths in the parent mission workspace, while the checker resolves against this Actor worktree. Input hashes and raw hashes pass the preceding assertions; recomputation stops at the foreign root. The affected copied run is `2026-11-07T23-42-57-893Z-acb0fb6d-78cf-4df8-bfdc-c971e85992db` under the coverage baseline runs directory. This does not establish that the original parent's evidence check fails. The Chief should have the tooling owner verify in the integrated workspace or produce a run bound to that workspace. No raw export, source hash, acceptance command or checker was rewritten here.
2. **Historical prerequisite issue, not a current denial claim.** Dependency attempt-5 receipts show matching LLVM tools, pinned coverage installation and Cargo fetch succeeded after the operator's environment note. Its mise installation still exited 1 on a denied macOS cache write. Current direct version checks here found Bun 1.4.4 and cargo-llvm-cov 0.6.21. No installation was rerun for this documentation task; the guide describes the delivered setup and its actual stop-on-error behavior.
3. **Historical downstream acceptance remains unapproved.** The dependency report records `cargo test --workspace` exiting 101 at the process-owned schema fixture, and the fixed-budget unchanged harness timing out before coverage. Those commands were not rerun here, and this report does not claim those defects remain unchanged in another Actor's concurrent work. They require current downstream receipts and independent oma-qa before overall completion.
4. **Unrelated documentation drift.** The global baseline scan found 226 broken references, outside the three owned paths. The sync command proposed six historical evidence documents, including the marketing report and prior task-1 reports. All were skipped: they are outside ownership and several are immutable historical receipts. No unrelated reference repair or shared registry edit was attempted. Generated ignored documentation indexes are CLI outputs, not product deliverables.

## Checks and acceptance

| Command/check | Outcome and scope |
| --- | --- |
| Scoped Git status/diff and `git rev-parse HEAD` / tree | Intake complete; actual snapshot identifiers above. Global status denied access to av.yaml. |
| `oma docs verify --json` before edits | Exit 1; 670 docs, 4,307 refs, 356 skipped, 226 broken references; none in assigned files, which did not yet exist. |
| `oma docs sync fbe76a5f15101ad415f355ef06f1deeb1f97d568..HEAD --json` | Exit 0; six proposals, all outside assigned edits; preserved. |
| `bun test scripts/coverage/gate.test.ts scripts/coverage/accounting.test.ts scripts/coverage/preservation.test.ts` | Exit 0; 27 tests, 87 assertions. Checks synthetic failure/accounting behavior and accepted-baseline preservation. |
| `bun test scripts/coverage/evidence.test.ts` | Exit 1; 0 pass, 1 fail, seven preceding assertions; copied absolute-path report mismatch described above. Repeated standalone to capture its exit independently. |
| `bun -e` importing `inputIdentity` | Exit 0; matching implementation hash and 109-file inventory. No measurement rerun. |
| `bun --version`; `rustup run 1.95.0 cargo llvm-cov --version` | Exit 0; 1.4.4 and cargo-llvm-cov 0.6.21. |
| `oma docs verify docs/rust-coverage.md --json` | Exit 0; one doc, ten references, zero broken or skipped. |
| Final `oma docs verify --json` | Exit 1; 673 docs, 4,339 refs, 356 skipped, the same 226 out-of-scope broken references. No new in-scope broken references. |
| `oma docs verify README.md --json`; `oma docs verify docs/rust-coverage.md --json`; `oma docs verify .arc-verify/rust-coverage/directors/design.md --json` | Each exits 0: respectively 1, 10 and 21 references; zero broken or skipped. |
| Manual relative Markdown-link and whitespace check across the three owned files | 13 relative links resolve; zero missing targets or trailing-whitespace lines. |

Acceptance checklist:

- [x] Commands, pins, prerequisite setup and one-command repeat journey match delivered tooling; root link and actionable report instructions exist.
- [x] Actual terminal source and prior command receipts are assessed separately from fresh synthetic checks and untested usability hypotheses.
- [x] Canonical accounting, unrounded 80% target, unavailable measurements and host/inactive-platform limitations are documented.
- [x] Desktop, native Windows and remaining Phase 0 stay incomplete.
- [x] No HTML explorer, wizard, retention change, invented field test, browser/GUI check, Playwright installation or external delivery was introduced.

Existing production, CLI/IPC/database and gate interfaces remain compatible because only documentation was added. The passing preservation check compares production tokens, existing assertions and protected bytes with accepted HEAD. This task is not the independent oma-qa review and does not supply a mission-wide PASS.

- desktop: incomplete
- native Windows: incomplete
- remaining Phase 0: incomplete
