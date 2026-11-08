# Terminal repair attempt 5

Status: implementation and assigned native gates passed for task-6-terminal. Renewed independent oma-qa executions found no defect; its final verdict belongs to `terminal-repair5-independent-review.md` and `.json`. This frozen implementation handoff is a repair checkpoint consumed by tasks 6 and 7. It does not claim final lifecycle acceptance or replace framework review.

## Repair and interface compatibility

The inherited SIGCHLD=SIG_IGN defect is reproduced and fixed. `Signals::install` now saves and overrides SIGCHLD even when the caller ignored it. Its private action permits native waiting and clears automatic-reaping flags, including SA_NOCLDWAIT. Other inherited ignored signals retain their prior behavior. `Signals::prepare_child` restores the saved SIGCHLD action using sigaction in the real Command pre_exec path. The callback uses only native async-signal-safe operations and errno inspection; a preparation error returns through Command spawn and the existing static spawn-failure/lifecycle-completion path. The caller's signal mask is unchanged in the target. Normal completion, early return and spawn failure drop the guard and restore the wrapper's saved action and mask.

The child remains the single CLI-owned Command target. Original OsString argv, cwd, environment and descriptors reach that child directly, outside every IPC retry. Observed exit 17 is preserved and recorded as completion 17. No storage/IPC contract, classifier, migration, reservation, admission or shutdown implementation changed.

The operator-directed job-control repair remains intact: a controlling terminal means the ordinary caller job group is inherited, including background jobs and a terminal session leader. There is no child process-group or foreground-terminal transfer on that path. Terminal and external-group INT are not forwarded again. Wrapper-directed nonterminal signals target only its owned unreaped PID; the private nonterminal child-group behavior remains. Native WUNTRACED/WCONTINUED consumption, stale-stop reconciliation, foreground/background continuation and inherited ignored INT/USR2 behavior remain present. Ownership is cleared within the consuming wait before handlers can forward another signal.

## RED to GREEN and native evidence

Fresh default control: the original exact inherited-blocked-SIGCHLD test passed before the repair. An isolated process then set SIGCHLD=SIG_IGN and blocked SIGCHLD before executing that same test body via ARC_SIGNAL_CASE. That variant timed out after five seconds. The owned wrapper PID/start token was verified before SIGKILL, it was reaped, and its identity was absent afterwards. `ignore-red.json` records exit 124 rather than a false pass. The same bounded probe passes after repair (`ignore-green.json`, exit 0), preserving its original command and process setup.

The new isolated unit requires inherited IGN before wrapper startup, verifies IGN in the actual executed receiver, preserves blocked CHLD in that receiver, releases exit only after an empty WNOHANG wait, observes exit 17, proves target identity absence and CHILD=-1, and raises a wrapper TERM after reap without queuing or forwarding it. A second isolated unit verifies that wrapper SA_NOCLDWAIT is cleared privately and that the saved ignored action/flags are restored after early guard drop and executable-not-found spawn failure. All six signal-unit identities pass; one is the native receiver helper, five are substantive checks. Both production and unit receiver use the same preparation method.

The real CLI integration adds four cases: default/ignored SIGCHLD crossed with blocked/unblocked CHLD. Setup occurs in pre_exec before the real wrapper starts. The native target reports the disposition and mask through its existing barrier. Each wrapper observes exit 17, the daemon stores completion 17 with the exact captured target identity, the invocation marker has one line, and target plus wrapper identities are absent after reap. The test also supplies SA_NOCLDWAIT during setup. These are actual native executions, not simulations.

The original pipeline RED remains preserved and raw-hash verified through terminal-repair4's `red` command (exit 101, direct 6/6, managed 0/5 at its original source identity). The current pipeline passed before this narrow SIGCHLD edit and passes after it. Its 13 direct/managed scenarios retain peer input, terminal and external-group INT counts, wrapper-only TERM without a peer signal, whole-job Ctrl-Z, background SIGTTIN, foreground resume, stale-stop handling, input after resume, [17,0] outcomes, terminal-mode/foreground restoration and absence of all recorded process identities. No provenance/group candidate experiment was rerun.

| Actual delivered-source command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS, exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS, exit 0 |
| exact `cargo test --workspace` | PASS, 339 identities, 32 harnesses, 0 ignored; includes all 20 managed-exec identities |
| exact native foreground-pipeline command | PASS, 1 identity, all 13 scenarios |
| exact inherited-ignore real CLI command | PASS, 1 identity, all four disposition/mask cases |
| original exact blocked-CHLD body under inherited IGN | PASS, bounded isolated native probe |
| `bun test scripts/harness/managed-exec.test.ts` | PASS, 12 tests |
| historical preservation and hygiene suites | PASS, 4 tests; historical contracts retained |
| `bun test scripts/harness/equivalence.test.ts` | PASS, 2 tests in an isolated copy containing all 177 delivered inputs |
| scoped `git diff --check` | PASS, exit 0 |

The exact workspace run also passes standalone foreground and background-read/resume, ignored-signal and signal-count, stopped-child, closed-descriptor, spawn-failure, wrapper-only termination, detached/controlling-terminal session-leader and daemon shutdown/crash survival regressions. The independent reviewer separately reproduced focused units, all four standalone terminal identities, detached session leader, the exact pipeline and inherited-ignore through the production path. Its report lists its own executions.

The JSON companion records actual argv/cwd, Darwin arm64 host, UTC timestamps, exit codes, 3,600,000-ms check allowances, native PIDs, source hashes and raw output hashes. Cargo identities/counts were parsed from actual libtest output. Equivalence records the actual isolated cwd and exact inner Bun argv; this is not a claim that the suite ran in the original checkout. Original harness-speed receipts remained byte-identical. Fixture bounds such as the five-second RED probe do not shorten required gate allowances.

A development audit failed because it compared storage's digest strings with byte/mode objects. Its original script and recorded failing replay remain under the unique ignored directory. A separate corrected audit compares the actual SHA-256 fields and passes; no product or protection assertion was changed for that tooling error. The first receipt assembler referenced the supervisor receipt where it needed the JSON audit data; that data was retained in stdout. The separate corrected assembler binds those actual retained bytes. No product check was replayed or relabeled for this assembly error. Native reviewer dispatch initially failed because full-history context could not be loaded; the supported no-history dispatch succeeded with explicit scope. Neither failure is counted as a product pass.

## Source and ownership receipts

Delivered managed source identity: `e828477b00cd7b02b40fa0deea1425321e8b0b5b63b081dfaa7b31b48485ca05`, 177 byte/mode bindings. All 177 matched terminal-repair4 at entry; only five assigned paths differ now, with 172 unchanged:

- `crates/arc-cli/src/exec.rs`
- `crates/arc-cli/src/exec/signals.rs`
- `crates/arc-cli/src/exec/signals/tests.rs`
- `crates/arc-cli/tests/fixtures/child.rs`
- `crates/arc-daemon/tests/managed_exec/pipeline.rs`

The storage receipt's 168 bindings were rechecked; its seven mismatches are exactly those already recorded by terminal-repair4, with no new storage/IPC mismatch. All prior required contract/review artifacts and all 97 frozen retained evidence files remain byte-identical. Index bytes remained unchanged across gate/audit execution; no staging or reset occurred. Existing preceding work remains in place. No accepted contract, migration, manifest, lockfile, baseline, coverage accounting, .agents definition or reviewer artifact was edited. The earlier independent PASS is retained in its original scope and is not used to clear the later rejection.

Migration 0001 is byte-identical to accepted HEAD, SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. Maximum Rust/SQL source length is 474 lines. Before the new implementation/review pairs there were 64 indexed and 97 retained managed-exec files; both pairs bring retained count to 101, below 150 even if tracked. No prior evidence was deleted. The positive-execution promotion artifact remains frozen in its task-5 scope; these terminal checks write unique execution evidence and do not require its promotion or claim final-summary validation.

Own deliverables are this report and `terminal-repair5.json`. Raw command receipts, outputs, supervisors, exact identity inventories, copied-equivalence receipts and audit data live under `target/terminal-repair5-ca4a6f6d/`. The reviewer separately owns its new pair. Skill used: oma-backend. Serena was not exposed; named paths were read directly without project-native discovery searches. No cancellation API, injected absolute deadline or authoritative remaining cumulative allowance was exposed. No unavailable budget was invented or reset; mission attempts, graph and abandoned collector state were not mutated.

## Acceptance checklist and remaining scope

- [x] Preserve original pipeline RED and pass its direct-versus-managed native regression.
- [x] Verify terminal input, signal counts, job stop/resume, background/foreground continuation, restoration and exit 17.
- [x] Fix inherited ignored/blocked SIGCHLD through real preparation; verify disposition/mask, completion, ownership cleanup and guard restoration.
- [x] Keep and pass standalone, ignored-signal, closed-descriptor, spawn-failure and daemon-loss regressions.
- [x] Pass exact workspace, formatting, warning-denying Clippy and affected protection/equivalence checks on delivered sources.
- [x] Freeze source-bound handoff for renewed independent oma-qa and framework review; final verdict belongs to the separate reviewer pair.

This is lifecycle management, not scheduling capability. Task-6 still consumes a refreshed passing workspace/storage checkpoint; task-7 owes current-source coverage collection, each crate's 80% production-line floor, the full verification inventory, full harness without skips and final live evidence. Task-8 final QA, task-9 Chief acceptance and the framework's independent checkpoint remain required. Desktop, native Windows and Phase 1 scheduling remain outstanding. No final lifecycle acceptance flags were created. No browser/GUI/Playwright, installation/download, separate build, publication, push, merge, outreach, spending or mission-state mutation occurred.
