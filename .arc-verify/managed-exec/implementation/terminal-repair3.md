# Terminal repair attempt 3

Status: **failed repair checkpoint**. No product change was promoted. The exact native macOS pipeline regression and `cargo test --workspace` still fail. Independent `oma-qa` reproduced the failure. This report is the task-6-terminal handoff to the Chief Director; it does not accept task-6, task-7 or the lifecycle slice.

## Delivered files and ownership

Backend wrote `terminal-repair3.md`, `terminal-repair3.json` and `terminal-repair3-probe.py` in this directory. The assigned reviewer wrote the separate `terminal-repair3-independent-review.md` and `.json`. Raw command receipts, output, supervisor code and probe versions are retained under `target/terminal-repair3-8e0645a0-1776-4bfd-9e04-09b9925e9ac2/`. Short Unix-socket fixture roots are retained under `/tmp/arc-sig3-*`; their names are bound in the receipts.

No production code, test, migration, manifest, lockfile, accepted contract, preservation baseline, coverage accounting, prior receipt or mission state was changed. Existing work remains in place. Skill used: `oma-backend`, with its execution protocol and checklist; independent review used `oma-qa`. Serena was absent, so inspection used the supplied named paths directly. No relevant memory evidence was used.

The recomputed managed source identity is `51b7a5a6ba911d26e58d00e40f74485e54a935097f0f9207b6964b1ec6027411`: all 176 input bytes and modes match retained `terminal-repair2.json`. The affected storage dependency binding was checked before reuse: 167 of its 168 entries match, with only `crates/arc-daemon/tests/managed_exec/pipeline.rs` differing because of the preceding attempt's retained session-leader regression. That file matches the newer 176-input delivered inventory. This attempt changed neither binding nor source.

## Fresh native results

Host: Darwin 27.2.0, arm64, Eunkwangui-MacBookPro.local. Every executable-check supervisor allowed 3,600,000 ms. The JSON companion records actual argv, cwd, UTC interval, native host, exit status, test identities/counts and verifiable output hashes. Source inputs stayed unchanged across each command.

| Command | Result |
| --- | --- |
| `cargo test -p arc-daemon --test managed_exec pipeline::native_foreground_pipeline_input_signals_stop_resume_and_exit17 -- --exact --nocapture` | FAIL, exit 101; direct 6/6, managed 0/5; libtest 0 passed, 1 failed |
| `cargo fmt --all -- --check` | PASS, exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS, exit 0 |
| `cargo test --workspace` | FAIL, exit 101; managed-exec target 17 passed, 1 failed |
| `bun test scripts/harness/managed-exec.test.ts` | PASS, 12 tests |
| `bun test scripts/coverage/preservation.test.ts scripts/coverage/hygiene-preservation.test.ts` | PASS, 4 tests |

The workspace run passed the retained standalone foreground/input/modes/signals, background read/foreground resume, ignored disposition, stopped-child, spawn-failure, closed-descriptor, wrapper-directed termination, daemon shutdown/crash, retry and session-leader-without-terminal cases. The pipeline regression remains the failing test. Cargo stopped at that target; later workspace targets are not claimed as executed by this run.

The pipeline failure is unchanged: the private child foreground group takes terminal access from the peer; terminal SIGINT reaches the child once but the peer zero times; Ctrl-Z stops only the wrapper. Direct cases preserve input, whole-job suspension/continuation, foreground restoration and outcomes `[17, 0]`. All eleven fresh scenarios reported controller exit 0 and no remaining recorded PID/start-token identities. Cleanup success does not turn the managed scenarios into passes.

Independent QA ran the exact regression again: exit 101, direct 6/6, managed 0/5, with all 176 bound inputs unchanged. See [independent review](terminal-repair3-independent-review.md) and its JSON companion. Its verdict is **FAIL**.

## Native receipt experiment

The concrete candidate was to retain caller-group membership and use Darwin `EVFILT_PROC / NOTE_SIGNAL` notifications as child delivery receipts for a forwarding decision. Before editing product code, [the retained probe](terminal-repair3-probe.py) tested the necessary receipt property against the existing native pipeline fixture and controller. Probe SHA-256: `66575a9265ba6e3622e7708d15b48ebb988b56d178263841c9e4ea47e8e715b0`.

The corrected experiment and independent replay both exit 0 with four direct baseline cases passing. Successful event registration returns an EV_RECEIPT error receipt with `data=0`. After terminal SIGINT, external group SIGINT and direct-producer PID SIGTERM, the observed child event is always `fflags=0x08000000, data=0`: it supplies no signal number or delivery count. In a barrier-controlled interleaving, the target acknowledges two separate SIGINT deliveries, count 1 then 2, before the observer drains the queue. That drain yields one NOTE_SIGNAL event. The first recorded empty event list is deliberate suppression of the first drain, not an observation of an empty queue.

This rejects the proposed lossless child-receipt mechanism. It does not establish that every native strategy is impossible. The experiment's inherited `wrapper-term` label sends TERM to the **direct producer**, not an ARC wrapper. Therefore these passes establish the receipt counterexample and baseline prerequisites, not a repaired wrapper or complete forwarding strategy. No timing window, pending-mask poll, foreground-membership inference or sender/code inference was used to choose whether to forward a signal.

The installed SDK's `sys/event.h` defines the tested flags. Apple's public [XNU process filter](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_event.c#L1108-L1242) masks low hint data and aggregates event flags, consistent with the observed loss of per-delivery detail. That public branch is supporting API evidence, not a claim to have inspected the running kernel's exact source.

Two invalid probe executions remain recorded separately: the first exceeded the native Unix-socket pathname limit before launching participants; the second used a Python `select.KQ_EV_RECEIPT` attribute absent in this installation. The corrected probe uses the installed SDK's literal EV_RECEIPT value `0x0040` and short socket paths. Both the individual-event and coalescence executions are preserved; independent QA replayed the final frozen probe.

## Unresolved implementation prerequisite

The current private-child-group topology cannot preserve caller pipeline behavior. Sharing the caller group fixes that topology but makes unconditional forwarding duplicate external group deliveries. The preceding relocation experiments retain their session-leader/controlling-terminal counterexamples. This attempt supplies no native protocol that resolves those cases while preserving the fixed CLI-owned direct `Command` child and exact signal obligations. NOTE_SIGNAL does not provide the missing lossless receipt stream.

The Chief receives this exact source-bound conflict for coordination. No architecture exception, exec-in-place substitution, unrelated-peer signaling, acceptance weakening or operator implementation choice was introduced. A passing native forwarding/stop-continuation mechanism under the selected boundary remains the prerequisite for promoting a repair. Until then, the existing single pending-signal integer, private-group standalone-test assumption and stop/CONT interleaving gaps remain unresolved. No RED-to-GREEN product result is claimed.

The required check sequence is incomplete as an acceptance sequence. Coverage collectors, full harness, final live evidence and task-8 final QA were not run. The task explicitly defers collectors until the focused repair succeeds. No stabilized candidate changed equivalence inputs; the preceding exact-input equivalence receipts retain only their recorded scope. No mission harness-speed evidence was overwritten.

`0001_phase0.sql` remains byte-identical to accepted HEAD, SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. The maximum Rust/SQL source length is 474 lines. The JSON records tracked and retained evidence counts; neither reaches 150. No coverage floor, accounting rule or regression assertion was changed.

No cancellation was present in the supplied input. The runtime exposes no cancellation API, injected execution deadline or remaining cumulative allowance; none was invented or reset. Attempt 3 and all preceding spent allowances remain recorded. The first full-history reviewer dispatch failed with a missing-rollout error; an explicit-context native dispatch succeeded. No AV mission redispatch occurred. Abandoned collector `14213b3c-fbfb-48c3-867d-4602607bdd05` was left untouched and was not replayed.

Acceptance checklist:

- [x] Freshly reproduce the retained native failure without changing inputs.
- [x] Test and independently replay a concrete native receipt hypothesis before product promotion.
- [x] Retain failed probes, source-bound results and independent review.
- [x] Run formatting, Clippy, exact workspace and preservation checks.
- [ ] Deliver a conforming launch/signal/stop-continuation repair and passing workspace checkpoint.
- [ ] Obtain passing independent repair acceptance.
- [ ] Complete task-7 coverage/full harness/live evidence and task-8 final QA.

This remains lifecycle implementation work, separate from scheduling capability. Native macOS lifecycle acceptance is incomplete. Desktop, native Windows and Phase 1 scheduling remain outstanding. No browser, GUI, Playwright, install, download, separate build, publication, push, merge, outreach or spending occurred.
