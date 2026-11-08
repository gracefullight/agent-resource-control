# Terminal repair attempt 4

Status: implemented and verified repair checkpoint for task-6-terminal. This is the handoff to task-6 and task-7, not final lifecycle acceptance. The independent reviewer reported PASS after native reproduction and a separate unfiltered workspace run. Its final report and receipt are `terminal-repair4-independent-review.md` and `.json`; they own the independent verdict. They are deliberately not hashed here, so the reviewer can bind this frozen handoff without a hash cycle.

The repaired sources have managed source identity `2132331368d21281806b33bc265a2183fb1bd1702a09144c9c3b95386ec08864`, with 177 input byte/mode bindings. HEAD remains `19d7a507044cbf83918f58cd41056905c275c8d4`. Existing staged/unstaged work and earlier failures remain in place. Skill used: `oma-backend`; separate review uses `oma-qa` under the assigned reviewer, not the implementation writer.

## Implementation and interfaces

An invocation with a controlling terminal inherits the caller's job group, whether foreground, background, or a terminal session leader. ARC never calls tcsetpgrp or creates a private child group on this path. The OS delivers terminal INT, QUIT and TSTP to the job; ARC does not forward another copy of INT or QUIT. Nonterminal invocations create a separate child group. A session ID by itself does not prove detachment.

The wrapper forwards TERM, HUP, USR1 and USR2 only to its one attached, unreaped target PID. Nonterminal INT/QUIT forwarding remains supported. Ignored dispositions stay ignored. A bit set retains each distinct supported preattachment signal; repeated standard-signal counts may coalesce under POSIX. Signal masking synchronizes attachment, pending delivery, native reaping and detachment on the single-threaded CLI path. Handlers use native async-signal-safe calls and lock-free atomics and preserve errno.

Consuming waitpid handles WUNTRACED and WCONTINUED. Available continuation events supersede older stop events; a pending CONT also suppresses an earlier stop. Confirmed target stops can stop the wrapper. Normal shell continuation remains the shell's responsibility. Target-only SIGSTOP and detached-child stops receive PID-only continuation after wrapper resume. No peer group is signaled by ARC. The waiter's private suspend mask unblocks its CHLD/CONT notifications even when the caller blocked them, while child exec and return preserve the inherited mask. A private wait callback supplies a deterministic unit barrier after an empty native wait; production passes an empty callback.

One CLI-owned Command still receives original OsString argv directly, outside every IPC retry. Existing cwd, environment, inherited descriptors/streams, native signal result propagation and exit 17 remain intact. Storage, IPC, classification, admission, shutdown, diagnostics and public lifecycle contracts were not changed by this repair. Compatible native signal termination remains observable to the parent and shells report 128+signal. Lifecycle bookkeeping failure still cannot replace a child outcome or launch another child.

## Native evidence

The exact fresh RED command was run after the operator note: `cargo test -p arc-daemon --test managed_exec pipeline::native_foreground_pipeline_input_signals_stop_resume_and_exit17 -- --exact --nocapture`. It exited 101: direct 6/6 passed, managed 0/5 failed. Its unique receipts are under `target/terminal-repair4-01a1bbc4/red.*`. Prior RED artifacts and independent reviews were not rewritten.

The preserved pipeline regression now requires target and peer to retain the caller group. It verifies producer and peer terminal input, one terminal/group INT per recipient, wrapper-only TERM reaching the target without the peer, whole-job Ctrl-Z, background continuation/SIGTTIN, foreground input after resume, outcomes [17,0], terminal modes/foreground restoration and verified cleanup. A new barrier freezes the wrapper, stops and continues the target, confirms target activity, then resumes the wrapper; subsequent input and exit prove that stale stop evidence does not suspend it again. Independent final reproduction passes all 13 direct/managed scenarios.

The standalone fixture's background controller now resumes its job group rather than only the wrapper PID. It still checks the original outcomes and terminal restoration. The private-group assertion was replaced with caller-group membership; input, signals, native statuses and modes assertions remain. A new exec-after-setsid/TIOCSCTTY case reproduced loss of foreground input (exit 101) before the session-leader detection fix, then passed on delivered sources. The retained nonterminal session-leader case explicitly checks that the target owns its separate group.

New isolated native unit checks cover distinct pending TERM/HUP/USR1 and nonterminal INT, inherited ignored USR2, shared-job INT/QUIT suppression, ownership detachment, pending delivery across reap, and a child exit released only after WNOHANG while CHLD is inherited blocked. The native receiver and post-wait checks verify mask preservation. Independent QA found the inherited-blocked-CHLD and terminal-session-leader defects during this attempt; both have current-source GREEN regressions. Its separate RED probes retain failed startup/drain attempts and owned-process cleanup evidence.

| Delivered-source check | Outcome |
| --- | --- |
| `cargo fmt --all -- --check` | PASS, exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS, exit 0 |
| exact `cargo test --workspace` | PASS, exit 0; 336 identities, 0 ignored, 32 harnesses; managed-exec 19 tests |
| `bun test scripts/harness/managed-exec.test.ts` | PASS, 12 tests, including migration/regression/harness/coverage rejection checks |
| `bun test scripts/harness/equivalence.test.ts` | PASS, 2 tests in an isolated copy containing all 177 delivered inputs; original harness-speed receipts unchanged |
| historical preservation/hygiene suites | PASS, 4 tests against their fixed historical contracts; tested tooling is unchanged by later Rust-only edits |
| separate independent native checks and exact workspace | Reviewer-reported PASS; final report/receipt listed above |

All supervised check allowances were 3,600,000 ms. The JSON companion records actual argv/cwd, native host, timestamps, exit status, source identities and output hashes. The isolated equivalence receipt records its actual copy cwd and exact inner Bun argv; it is not claimed to have run in the original checkout. The workspace identity/count parser checked real raw libtest output. Earlier candidate failures remain separately labeled: stale prebuilt CLI use, a new test's missing fourth signal input, background controller PID-only continuation, wildcard test import rejection, and notification-consuming development barriers. No failed command is presented as a pass. Initial development receipts that omit the new unit file have a 176-input scope; final acceptance receipts bind all 177.

## Ownership and protections

Only eight owned source/fixture paths changed during this task:

- `crates/arc-cli/src/exec.rs`
- `crates/arc-cli/src/exec/signals.rs`
- `crates/arc-cli/src/exec/terminal.rs`
- new `crates/arc-cli/src/exec/signals/tests.rs`
- `crates/arc-cli/tests/fixtures/child.rs`
- `crates/arc-daemon/tests/managed_exec/pipeline.rs`
- `crates/arc-daemon/tests/managed_exec/pipeline.py`
- `crates/arc-daemon/tests/managed_exec/terminal.rs`

Own deliverables are this report and `terminal-repair4.json`, with unique raw outputs/supervisors under `target/terminal-repair4-01a1bbc4/`. Reviewer files are independently owned. The source audit compares all preceding 176 bindings and all 168 storage bindings: only the seven owned existing files differ, plus the new unit file. Contracts, storage/IPC code, manifests, lockfiles, coverage accounting, preservation baselines, prior receipts and reviewer verdicts were not edited. Historical assertions remain scoped to their original contract; no assertion or floor was weakened. No explicit positive-execution promotion was required for this terminal-only handoff; the prior task-5 receipt remains frozen in its original scope.

Migration `0001_phase0.sql` is byte-identical to accepted HEAD, SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. The maximum Rust/SQL source length is 474 lines. The audit found 64 indexed and 93 retained managed-exec files before these two new deliverables and the review pair, below 150 even if all are tracked. No index reset or staging occurred.

Serena was unavailable; named paths were read directly. ps was denied and not bypassed. No cancellation API, injected absolute deadline or remaining cumulative allowance was exposed; no allowance was invented/reset. The supplied attempt/round bounds remain owned by the supervising framework. No other implementation Actor was dispatched. The abandoned collector `14213b3c-fbfb-48c3-867d-4602607bdd05` was left untouched.

## Remaining work

This checkpoint establishes terminal lifecycle behavior on native macOS, not Phase 1 scheduling capability. Task-7 still owes current-source collecting coverage, every crate's 80% production-line floor, the complete coverage inventory, the full harness without skips and final live evidence. Task-8 final independent QA and Chief acceptance remain required. This task creates no final-summary acceptance flags. Desktop, native Windows and Phase 1 scheduling remain outstanding. No browser/GUI/Playwright, installation, separate build, publication, push, merge, outreach, spending or mission-state mutation occurred.
