# Terminal repair checkpoint 7

Status: PASS for this repair checkpoint. Implementation owner: Technical Director. Skill: oma-backend. Final lifecycle acceptance remains downstream. Independent oma-qa owns its separate terminal-resume7-independent-review.md/json verdict; this report does not author that verdict. Framework review remains required.

The ignored-SIGCONT defect is repaired. A wrapper now installs its private SIGCONT handler even when the caller ignored SIGCONT. It saves that original action and restores it in the executed target through the same production Signals::prepare_child used by exec. SIGCHLD restoration remains intact. Both actions are copied before fork; the callback uses sigaction and errno inspection only. Target masks remain inherited. The existing guard restores wrapper actions and mask on completion, early return, spawn failure and partial installation failure. Other ignored signals retain their behavior.

No launch, IPC or lifecycle interface changed. exec.rs still has one direct OsString Command spawn outside retries. Controlling-terminal jobs retain their caller group and peers, without setpgid or tcsetpgrp. Terminal/group INT and QUIT are not forwarded again. Wrapper-directed TERM/HUP/USR1/USR2 still reach only the owned unreaped target. The existing consuming waiter, stale-stop reconciliation, PID-only target continuation and ownership clearing remain unchanged. No storage, classifier, contract, migration, dependency, manifest, lockfile, coverage accounting or preservation-baseline file was edited.

## Reproduction and regressions

The fresh shared-job native RED includes a default-disposition control using the same independent controller and fixture. The default case passed. With SIGCONT=SIG_IGN established before actual arcctl startup, a target-only SIGSTOP suspended the target and then its wrapper. SIGCONT sent only to the wrapper resumed the wrapper, while the target remained natively stopped through the bounded control timeout. The failure-state inventory records the wrapper running and target stopped, and every recorded identity is absent after fixture cleanup. cont-red-states.json binds this exit101 and output hashes. The isolated unit RED also fails the missing-private-handler assertion. Original pipeline RED evidence remains preserved at its earlier sources; it was not relabeled as a current failure.

GREEN uses the same production child-preparation path. Default and ignored SIGCONT cases pass in both a real shared terminal pipeline and a no-terminal session. Controllers verify identities, explicit target stop, mirrored wrapper stop, wrapper-only continuation, continued supervision and the same target, one invocation, exit17, matching persistent completion17 and cleanup. Shared jobs retain peer terminal input and terminal/foreground restoration. The nonterminal controller additionally observes native WCONTINUED for its wrapper before releasing the target exit barrier. Its failure guard kills only a verified target, resumes its verified wrapper for reap and retains bounded cleanup.

The new isolated unit case checks default/ignored target disposition and blocked CONT mask, private wrapper installation, native exit17, ownership clearing and restoration after normal completion, early return and failed spawn in shared and nonterminal modes. Existing inherited ignored/blocked SIGCHLD, ordinary ignored signals, preattachment delivery and post-reap ownership assertions remain present.

## Required checkpoint checks

| Command | Result |
| --- | --- |
| `cargo test -p arc-daemon --test managed_exec inherited_sigcont -- --nocapture` | 2 passed; both default and ignored controls, shared and nonterminal |
| `cargo test -p arc-cli --bin arcctl exec::signals::tests -- --nocapture` | 7 passed, including receiver helper |
| `cargo test -p arc-daemon --test managed_exec pipeline::native_foreground_pipeline_input_signals_stop_resume_and_exit17 -- --exact --nocapture` | 1 passed; all 13 preserved direct/managed scenarios |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| exact `cargo test --workspace` | 342 passed across 32 harnesses; 0 failed, 0 ignored |
| `bun test scripts/harness/managed-exec.test.ts` | 12 passed; 234 assertions |
| `bun test scripts/coverage/preservation.test.ts scripts/coverage/hygiene-preservation.test.ts` | 4 passed; 23 assertions |
| `bun test scripts/harness/equivalence.test.ts` | 2 passed in an isolated delivered-source checkout |
| scoped `git diff --check` | PASS |

All final gate receipts bind native Darwin 27.2.0 arm64 and the same 177 source/mode inputs, SHA-256 `a9f6451fc3242aab2d555e56d8ee59caa293cf42481315e23a4c4b4485ce7137`. Each records actual argv, absolute cwd, UTC timestamps, owned command PID, exit status, at least 3,600,000-ms allowance and verifiable stdout/stderr hashes. Exact test identities/counts were parsed with the existing repository parser. Raw receipts and outputs remain under ignored target/terminal-resume7/. The separate equivalence record binds its actual isolated cwd, exact inner Bun argv, all copied input hashes/modes and generated receipt hashes; retained harness-speed artifacts were not overwritten.

The unfiltered workspace run includes all 22 managed-exec identities: the new CONT regressions, four inherited CHLD combinations, standalone foreground/background read/resume, ignored-signal/count, target stop, closed descriptors, spawn failure, wrapper-only termination, session leaders, IPC retries, child survival through fixture daemon shutdown/crash and restart reconciliation. These are actual passing executions. Independent QA separately executes the focused native cases and audits these gate receipts; its report distinguishes those two forms of evidence.

## Ownership and evidence boundary

Six owned source/fixture paths changed; their final hashes and full input manifest are in the companion JSON. No new crate or dependency was introduced. All 177 terminal-resume6 input bindings matched entry before reuse. The reviewed storage handoff's 177 bindings have five preceding differences at entry (contract.json, decision.md, job.rs, managed_exec/cli.rs and scenarios.rs), preserved in source-audit.json. Storage/core/IPC production inputs were unchanged; fresh exact workspace success supplies a current execution checkpoint rather than promoting historical storage reviews.

All 134 retained entry artifacts and the Git index remain byte-identical. Migration 0001_phase0.sql equals accepted HEAD, SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. Maximum checked source length is 474 lines; changed files range from 108 to 427 lines. The two new implementation outputs and separate review pair bring the retained prospective count to 138, below150; no earlier evidence was deleted or staged. Accepted contracts, frozen positive-execution/reviewer bytes and the abandoned collector record remain unchanged. No final evidence promotion is claimed; downstream final-source integration/rebinding remains necessary.

No authoritative cancellation API, injected absolute deadline, OMA claim identity or remaining cumulative allowance was exposed. Supplied attempt7 and all spent budgets remain under framework accounting; none were reset. No concurrent task6/task7 implementation was dispatched. The first native reviewer spawn failed to load its inherited thread context; a fresh-context native reviewer dispatch succeeded without changing models or mission routing. Serena tools were unavailable; inspection used named direct reads. No relevant memory evidence was used.

## Handoff

Owned outputs are terminal-resume7.md/json and the six changed source/fixture files. The reviewer owns terminal-resume7-independent-review.md/json. Independent review must bind the completed report and final-source gate receipts; its verdict is not written here by the implementation owner.

This delivers the terminal lifecycle repair checkpoint. Task6 still owns refreshed storage/workspace integration. Task7 owes complete current-source coverage verification with each crate's80% production-line floor, the full harness without skips and final live evidence. Task8 final independent QA and task9 Chief acceptance remain required. Lifecycle management is separate from scheduling. Desktop, native Windows and Phase1 scheduling remain outstanding; commercial benefits are unmeasured. No collector, AV operation, GUI/browser, Playwright, separate build, publication, push, merge, outreach or spending ran.
