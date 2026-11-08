# Terminal repair checkpoint

Status: failed task-6-terminal repair attempt. No product change was promoted. The original native pipeline defect remains. This report and the rejected candidate are handoff evidence for the Chief Director; they are not task-6 acceptance, task-7 acceptance or final lifecycle acceptance.

Skill used: `oma-backend`. Independent reviewer used `oma-qa`. The review is in [terminal-repair-independent-review.md](terminal-repair-independent-review.md) and its JSON companion. No mission graph, instructions, acceptance criteria, assignments, budgets or operator verification command changed.

## Result and unresolved prerequisite

One concrete mechanism passed the required pipeline behaviors on native macOS but introduced a confirmed execution failure for a session-leader wrapper. It therefore does not satisfy the execution contract.

The candidate keeps the direct `Command` target in its original caller group. A continuation anchor remains in that group. A transient bootstrap creates a separate group for the CLI observer, which still owns and waits for the target. Caller-group termination signals reach target and peers directly; PID-only observer signals are forwarded only to its unreaped target. The anchor relays continuation with a shared epoch and acknowledgement so a continuation cannot simply disappear before observer suspension. It never forwards termination to peers. The child arguments remain `OsString` values passed directly to `Command`, outside metadata retries.

The original and anchored Python prototypes each passed six direct and five managed native pipeline scenarios. The anchored prototype also delayed peer creation until its anchor/group setup barrier completed. The Rust candidate passed the preserved pipeline regression and all 17 managed-exec tests independently. Those checks cover peer terminal input, terminal and external group signal counts, wrapper-only termination, stop/background-continue/foreground-resume, exit 17 and terminal restoration. The extended candidate fixture recorded the continuation anchor's native identity and verified its absence after each scenario.

Independent QA then demonstrated the blocker with a healthy owned daemon: direct execution using `start_new_session=True` exited 17; ordinary managed execution exited 17; managed execution using `start_new_session=True` exited 1 with `NOT_STARTED`. A separate native syscall experiment confirmed `setpgid` returns `EPERM` when the session leader attempts to join the live bootstrap group. The failed managed record remained in `starting` state.

This mechanism depends on relocating the observer. Session leaders cannot perform that relocation in the demonstrated native experiment. Keeping observer and target together reintroduces the already demonstrated indistinguishable PID-versus-group signal receipts. Moving the target to a private foreground group restores the original pipeline defect. No conforming replacement was demonstrated under the fixed CLI-owned direct child/wait boundary. This is a failure of the tested mechanism, not proof that every native architecture is impossible. The Chief needs a validated mechanism satisfying both invocation contexts before implementation continuation. This task does not authorize changing the accepted ownership contract or dropping a behavior.

## Preserved work and material deliverables

- [terminal-repair-candidate.patch](terminal-repair-candidate.patch): rejected Rust mechanism and fixture changes, 18,399 bytes, SHA-256 `dadd2f2570ad22f86f91ebbcdb953106090af170b0964a986134df9b0972b1fd`. Applying it to copied initial files reconstructed all six candidate file hashes exactly. Do not treat it as an accepted patch.
- [terminal-repair.json](terminal-repair.json): native host, required input bindings, exact commands/cwd/timestamps/statuses, test identities/counts, output hashes, candidate/restored source identities and restore proof.
- Reviewer-owned `terminal-repair-independent-review.md` and `.json`: FAIL checkpoint, independent native test receipts and the session-leader counterexample.

The candidate touched `crates/arc-cli/src/exec.rs`, `exec/signals.rs`, `exec/terminal.rs`, a new `exec/relay.rs`, `crates/arc-daemon/tests/managed_exec/terminal.rs` and `pipeline.py`. Those edits were all removed after preserving the patch. Five existing paths were restored from verified initial bytes; only the new file created by this attempt was deleted. Every one of the 175 initial bindings matches again. No reset, staging, index update or edit to another Actor's work was used.

Current restored managed-input identity is `d90dd726c3108b6e7468ce332dd88fd4f1b601928c95e2dda7c91e4b1b47bb30`. Rejected candidate identity is `10f3b6354c14f1b78c316d2e061f3c025403eda4f15a84bfc8bf8e6e5bd5c010`. The reviewer independently recorded candidate per-file hashes. The first candidate workspace run lacks a canonical pre-command aggregate snapshot; its actual execution is retained, but it is not offered as a reusable final acceptance receipt. Independent focused/managed runs and later parent checks bind their inspected sources explicitly.

Migration `0001_phase0.sql` remains byte-identical to accepted HEAD, SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. The 168 storage receipt bindings matched before reuse. Accepted contracts, manifests, lockfiles, preservation baselines, coverage accounting, previous reviewer artifacts and `.agents` definitions were not edited. The candidate's largest changed source file had 382 lines; restored workspace source limits remain protected by the passing current preservation suite.

## Actual command results

Commands ran in the mission worktree unless a fixture cwd is stated. Wrappers allowed 3,600,000 ms per check. Host: Darwin 27.2.0, arm64, Eunkwangui-MacBookPro.local. Exact UTC times and command argument arrays are in the JSON receipt.

| Command | Scope | Actual result |
| --- | --- | --- |
| `cargo test -p arc-daemon --test managed_exec pipeline::native_foreground_pipeline_input_signals_stop_resume_and_exit17 -- --exact --nocapture` | Initial retained sources | Exit 101; 0 passed, 1 failed; direct 6/6, managed 0/5 |
| Same exact focused command | Rejected Rust candidate, after CLI test compilation | Exit 0; 1 passed, 16 filtered |
| `cargo test -p arc-cli` | Candidate CLI compilation and existing regressions | Exit 0; 16 tests passed |
| `cargo test --workspace` | Rejected candidate | Exit 0; 330 passed across 32 completed summaries, 0 failed/ignored |
| `cargo fmt --all -- --check` | Rejected candidate | Exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | Rejected candidate | Exit 0 |
| `bun test scripts/harness/managed-exec.test.ts` | Rejected candidate | Exit 1; 11 passed, 1 failed; conservative assertion-binding guard rejects new `relay.rs` |
| `bun test scripts/coverage/preservation.test.ts scripts/coverage/hygiene-preservation.test.ts` | Rejected candidate | Exit 0; 4 passed |
| `bun test scripts/harness/equivalence.test.ts` | Isolated current-source copy | Exit 0; 2 passed; original retained harness receipts unchanged |
| `cargo test --workspace` | Restored current sources, including rebuilt CLI | Exit 101; 136 passed and 1 failed before Cargo stopped; managed-exec target 16 passed, pipeline 1 failed |
| `bun test scripts/harness/managed-exec.test.ts` | Restored current sources | Exit 0; 12 passed |
| `git diff --check -- <four restored tracked owned paths>` | Restored current sources | Exit 0 |

Independent QA separately ran the exact focused pipeline command and the unfiltered `cargo test -p arc-daemon --test managed_exec`: 1 passed and 17 passed respectively on the frozen candidate. QA's native session-leader probe failed candidate conformance, so its verdict is FAIL despite those passes.

The source guard failure was not bypassed or weakened. The newly introduced never-return syntax appears to trigger its conservative unresolved-macro check; no claimed root-cause repair is part of this rejected candidate. The current restored suite passes unchanged.

## Failed attempts and evidence limits

The first prototype invocation used socket paths exceeding the Unix socket path limit and failed before scenarios. A unique short fixture root resolved that prerequisite. An early focused Rust retry used the old `arcctl` binary because testing only `arc-daemon` does not compile the separate CLI binary. That retry failed and is explicitly excluded from candidate execution proof. `cargo test -p arc-cli`, the candidate workspace run and the independent runs then compiled/exercised the candidate. After restoring sources, the final workspace run rebuilt the CLI and reproduced the original defect; no stale candidate binary pass is claimed for the restored tree.

The initial native reviewer dispatch failed because the tool could not load the parent thread rollout. A context-free dispatch to the same reviewer succeeded. QA's first owned daemon probe encountered its own directory permission prerequisite; QA retained that failure, fixed only its owned directory mode and reran successfully. No credentials or external authorization were invented.

Raw backend artifacts are retained under the unique ignored directory `target/terminal-repair-7a6b4419-388d-4ddc-a10f-dc456bb6eee5/`; reviewer artifacts use their own unique ignored directory. The JSON receipt binds their actual output bytes. Prototype scripts, candidate copies, patch reconstruction and restore evidence are retained there. All recorded native pipeline identities were cleaned up. QA stopped its owned daemon with SIGTERM, reaped it with exit 0 and observed its socket/runtime cleanup. No whole-host process cleanup is claimed.

Serena was absent from the active inventory. Inspection used supplied named-file reads and Git inventories; no replacement code search or GUI workaround was used. There was no browser, Playwright, install, standalone build command, collector, publication, push, merge, outreach or spending. The abandoned collector record was neither awaited nor replayed. Current cumulative mission allowance was not exposed by this Actor dispatch; supplied limits were retained without resetting them. No cancellation instruction arrived during this attempt.

## Acceptance checklist and handoff

- [x] Reproduced the retained native pipeline failure.
- [x] Tested a concrete mechanism against terminal, external group and wrapper-only signals before candidate promotion.
- [x] Preserved the original pipeline regression and recorded independent native review.
- [x] Retained failed results and restored all initial source bindings after rejecting the candidate.
- [ ] Delivered a conforming launch/signal/stop repair for both caller pipelines and session-leader wrappers.
- [ ] Required current-source workspace checkpoint passes.
- [ ] Independent repair checkpoint passes.

The five new material evidence files are the only retained edits from this attempt. At receipt creation, managed-exec had 64 indexed evidence files and 81 nonignored retained files before these last report additions, below 150. No final-summary file or acceptance flags were manufactured.

Lifecycle management remains incomplete. Phase 1 scheduling is not implemented. Desktop and native Windows remain outstanding. Full current-source coverage, the no-skip harness, final live evidence and task-8 final independent QA remain downstream obligations; older or rejected-candidate passes do not satisfy them.
