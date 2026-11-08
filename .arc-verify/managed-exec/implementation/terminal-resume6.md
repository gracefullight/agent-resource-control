# Terminal repair resumed checkpoint 6

Status: PASS for the assigned terminal repair checkpoint. Skill: oma-backend. Final lifecycle acceptance remains downstream. The retained repair already satisfies the inherited SIGCHLD requirement; this attempt verified it without changing product sources. A new independent oma-qa review owns its separate verdict in terminal-resume6-independent-review.md/json. Framework checkpoint review remains required.

Source identity: `64156543efd3b1d26ed4d1fd7d0ea607f3495735c5a9b82db5e7ae1cc376f06b` across 177 byte/mode bindings. Native host: Darwin 27.2.0 arm64. HEAD remains 19d7a507044cbf83918f58cd41056905c275c8d4. The exact source inputs are in the JSON companion; every fresh command started and ended with those inputs unchanged.

## Delivered implementation and compatibility

The delivered exec/signals implementation saves the original SIGCHLD action, installs a private wait-compatible action even when inherited SIG_IGN, and excludes SA_NOCLDWAIT from that private action. The actual production Command pre_exec path restores the caller action in the executed target using sigaction and propagates preparation errors through spawn failure. It leaves the inherited mask unchanged. Guard drop restores wrapper actions on completion, early return and spawn failure. Other ignored signals retain their behavior. The consuming native wait clears target ownership before unmasking; post-reap signals cannot be forwarded to a recycled PID.

Ordinary controlling-terminal jobs keep the caller process group, including background-to-foreground jobs. There is no process_group(0), setpgid or tcsetpgrp on that path. Terminal/group INT and QUIT are not forwarded again. Wrapper-directed TERM/HUP/USR1/USR2 target only the owned unreaped PID. Nonterminal launches retain their separate child group. WUNTRACED/WCONTINUED events and pending continuation reconcile stale stops. The same single spawn site receives original OsString argv outside IPC retries with inherited descriptors, environment and cwd. Storage, IPC, reservations, diagnostics and accepted contracts were preserved.

## Current native verification

| Actual command | Result |
| --- | --- |
| `cargo test -p arc-cli --bin arcctl exec::signals::tests -- --nocapture` | 6 passed; 5 substantive cases and receiver helper |
| `cargo test -p arc-daemon --test managed_exec pipeline::native_foreground_pipeline_input_signals_stop_resume_and_exit17 -- --exact --nocapture` | 1 passed; 13 direct/managed native scenarios |
| `cargo test -p arc-daemon --test managed_exec pipeline::native_inherited_ignored_sigchld_preserves_target_state_and_completion -- --exact --nocapture` | 1 passed; all 4 default/ignored and blocked/unblocked cases |
| `python3 target/terminal-resume6-5fabf9d6/inherited-exact.py` | Original blocked-SIGCHLD body passes with IGN and blocked CHLD established before wrapper exec |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| exact `cargo test --workspace` | 339 passed in 32 harnesses; 0 failed, 0 ignored |
| `bun test scripts/harness/managed-exec.test.ts` | 12 passed; 234 assertions |
| `bun test scripts/coverage/preservation.test.ts scripts/coverage/hygiene-preservation.test.ts` | 4 passed; 23 assertions |
| `bun test scripts/harness/equivalence.test.ts` | 2 passed; 10 assertions, in the isolated delivered-input checkout |
| assigned-path `git diff --check` | PASS |

The pipeline covers producer/peer terminal input, one intended INT per recipient, external job-group delivery without duplication, wrapper-only TERM without peer delivery, whole-job Ctrl-Z, background continuation/SIGTTIN, foreground resume, subsequent input, stale-stop reconciliation, [17,0] outcomes and terminal/foreground restoration. All recorded native process identities are absent after cleanup. The real CLI test verifies target disposition/mask, one invocation, observed exit 17, persistent completion 17 with the same target identity, and wrapper/target identity absence for all four cases. The isolated original-body probe additionally asserts the actual exec'd target IGN and blocked mask, consumes exit 17, clears CHILD, rejects post-reap forwarding, and reports wrapper cleanup.

The unfiltered workspace run includes all 20 managed-exec tests: standalone foreground/background resume, ignored-signal/count, target-stop, closed-descriptor, spawn-failure, session-leader and daemon shutdown/crash survival remain present and pass. Exact identities/counts were parsed with the unchanged repository parseCargoExecution helper. Independent QA separately reran focused native cases; its final report owns those observations.

The original pipeline RED (exit 101) and inherited-ignore RED (exit 124) remain preserved in terminal-repair4 and terminal-repair5. This attempt revalidated their raw output hashes and records them with their historical source identity in retained-red-verification.json. Fresh results are GREEN on the already-repaired sources. No failing implementation was restored merely to reproduce an old failure.

Each fresh supervisor allows 3,600,000 ms and records actual argv, absolute cwd, host, UTC timestamps, PID, exit, before/after source bindings, stdout/stderr hashes and byte lengths. The five-second isolated failure bound belongs to the fixture. Equivalence ran in its recorded temporary checkout with all 177 current source bindings copied and hash-checked; original harness-speed receipts stayed unchanged. Its inner exact argv and copied receipt hashes are separately retained.

## Ownership and retained evidence

No source file changed in this attempt. All ten terminal implementation/fixture inputs remain byte/mode-identical to terminal-repair5. The five differences from that handoff are preceding work: contract.json, decision.md, arc-cli/src/job.rs, and daemon managed_exec/cli.rs and scenarios.rs. The current storage.json has 177 bindings and the same five historical differences. The older storage-resume-review has 168 bindings with ten now-changed paths; its FAIL and historical scope are preserved, not promoted into a current passing storage checkpoint. Fresh workspace success here is available for task-6 to consume under its ownership.

All 130 preexisting retained evidence files and the Git index remain byte-identical to entry. Migration 0001_phase0.sql equals accepted HEAD, SHA-256 046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384. Maximum Rust/SQL source length is 474 lines. The index still has 127 managed-exec files; 130 indexed/nonignored files existed at entry, and the new implementation/review pairs yield at most 134 if all are tracked, below 150. No prior evidence was deleted or staged. Historical preservation, regression assertions, coverage accounting and harness protections remain intact and their affected checks pass. The task-5 positive-execution artifact remains frozen: no product inputs changed here, so no promotion was required.

Owned retained outputs are terminal-resume6.md and terminal-resume6.json; reviewer owns its separate pair. Raw receipts, outputs, identity inventories, audit and copied-equivalence artifacts remain under target/terminal-resume6-5fabf9d6/. There were no new failed product attempts. Initial reads of nonexistent YAML config/run.py paths were corrected using the installed CUE config and existing named probe files; no prerequisite was bypassed. Serena tools were not exposed; named files were read directly without native project-code discovery searches. No relevant memory entry was used.

No cancellation API, injected absolute deadline or authoritative remaining cumulative allowance was exposed. Supplied attempt 6, maxParallel 3, 86,400-second mission bound, 200-run limit and spent budgets remain unchanged under framework accounting. No budget or mission state was reset. No concurrent task-6/task-7 implementation was dispatched. No collector, AV operation, GUI/browser, Playwright, installation/download, separate build, publication, push, merge, outreach or spending ran.

## Acceptance and downstream work

- [x] Preserved original native pipeline RED and current direct-versus-managed GREEN.
- [x] Verified native input, single intended signals, stop/resume, background/foreground, restoration and exit 17.
- [x] Verified inherited SIGCHLD ignore/mask through production preparation, native completion, ownership cleanup and guard restoration.
- [x] Existing standalone, ignored-signal, closed-descriptor, spawn-failure and daemon-loss regressions pass.
- [x] Exact workspace, fmt, Clippy and affected protection/equivalence gates pass on current sources.
- [x] Material source-bound handoff prepared for separately owned independent oma-qa and framework checkpoint review.

This is a terminal lifecycle repair checkpoint, not scheduling capability or final lifecycle acceptance. Task-6 still owns refreshed workspace/storage checkpoint integration. Task-7 owes current-source coverage verification, every crate's 80% production-line floor, the complete full harness without skips and final live evidence. Task-8 final independent QA and task-9 Chief acceptance remain required. Desktop, native Windows and Phase 1 scheduling remain outstanding. The abandoned collector state and final-summary acceptance flags were not altered.
