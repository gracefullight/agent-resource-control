## Review Result: PASS

Status: completed independent task-6-terminal attempt-5 repair checkpoint. Verification completeness: completed for this checkpoint. No CRITICAL, HIGH, MEDIUM or LOW finding remains. This supersedes the attempt-4 PASS, which missed inherited ignored SIGCHLD. It does not accept the complete managed lifecycle feature or downstream coverage/final QA work.

The reviewed source identity is `e828477b00cd7b02b40fa0deea1425321e8b0b5b63b081dfaa7b31b48485ca05`, from the unchanged `managedInputs` inventory of 177 byte/mode bindings. Every independent executable check started and ended with these bindings. The exact backend `cargo test --workspace` passed 339 identities in 32 harnesses, with 0 failures and 0 ignored; managed-exec passed 20 identities. I audited that current-source execution and independently reproduced the focused native cases below.

### CRITICAL

None.

### HIGH

None.

### MEDIUM

None.

### LOW

None.

### Source review

Compared all 177 preceding bindings and read verified attempt-4 source bytes from its retained isolated checkout. Exactly five paths changed: `crates/arc-cli/src/exec.rs`, `crates/arc-cli/src/exec/signals.rs`, `crates/arc-cli/src/exec/signals/tests.rs`, `crates/arc-cli/tests/fixtures/child.rs`, and `crates/arc-daemon/tests/managed_exec/pipeline.rs`. The remaining 172 inputs are unchanged. The reviewed unified diff is bound in the companion JSON.

`Signals::install` now saves and replaces SIGCHLD even when inherited as SIG_IGN. Its private action uses SA_RESTART without auto-reap flags. `prepare_child` copies the original action before fork; its pre-exec closure performs only native sigaction and errno-based error construction, returning failures through Command::spawn. It does not alter the inherited mask. The hook is installed at the one production spawn site and at the isolated native target helper. Executed programs therefore retain ignored SIGCHLD while ARC can consume its child's native result.

The signal guard restores each saved action under a blocked mask on normal drop, early return and spawn failure. Installation errors unwind the partially installed guard. The native guard regression verifies ignored disposition and original action flags after early return and a real missing-program spawn failure. Normal inherited-ignore execution verifies disposition restoration after waiting; the inherited mask remains blocked after return. Consuming native wait clears target ownership before unmasking, so pending signals cannot reach a reaped/recycled target. The existing stop/continued handling and caller-job topology are unchanged.

Signal handlers retain native calls and lock-free atomics, preserve errno, and address only the attached unreaped target PID. Shared-job INT/QUIT are suppressed in ARC because the terminal/group already reaches each participant. TERM/HUP/USR1/USR2 use the PID-only forwarding path. Ignored non-CHLD dispositions remain ignored. The wait path sleeps atomically with its private CHLD/CONT wake mask rather than continuously polling. No web UI, permission, storage, IPC, schema, or public interface change appears in this repair.

### Independent native execution

Native host: Darwin 27.2.0, arm64, Eunkwangui-MacBookPro.local. Actual outer cwd is the assigned workspace. Every check supervisor allowed 3,600,000 ms. Fixtures use bounded barriers. The JSON companion records exact argv/cwd, timestamps, host, owned PID, source bindings, raw output hashes, and executed identities/counts.

| Exact execution | Result |
| --- | --- |
| `cargo test -p arc-daemon --test managed_exec pipeline::native_foreground_pipeline_input_signals_stop_resume_and_exit17 -- --exact --nocapture` | 1 passed; 13 native scenarios |
| `cargo test -p arc-cli --bin arcctl exec::signals::tests -- --nocapture` | 6 passed, 0 ignored; 5 substantive cases plus receiver helper |
| `python3 target/reviewer-terminal-repair5-6db167ca-1e5b-445f-99a8-c8910d4bdbda/inherited-exact.py` | Original exact inherited-blocked case passes with inherited SIG_IGN and blocked CHLD before test-wrapper exec |
| `cargo test -p arc-daemon --test managed_exec pipeline::native_inherited_ignored_sigchld_preserves_target_state_and_completion -- --exact --nocapture` | 1 passed; all 4 native disposition/mask cases |
| `cargo test -p arc-daemon --test managed_exec terminal:: -- --nocapture` | 4 passed, 0 ignored |
| `cargo test -p arc-daemon --test managed_exec pipeline::native_session_leader_without_terminal_preserves_execution_and_exit17 -- --exact --nocapture` | 1 passed, 0 ignored |

The independent inherited-ignore probe executes `exec::signals::tests::inherited_blocked_sigchld_wakes_after_exit_and_preserves_native_child_mask` with ARC_SIGNAL_CASE set to that exact identity. A native pre-exec setup sets SIGCHLD=SIG_IGN and blocks CHLD before the test wrapper starts. ARC_EXPECT_CHLD_IGNORED makes the actual exec'd receiver assert its disposition; its existing assertion also verifies the blocked mask. The production preparation hook is used. A gate releases exit only after WNOHANG has returned no event. The waiter consumes exit 17, clears ownership, preserves the caller mask and suppresses post-reap TERM. Native identity checks inside the test and separate reviewer probes show both wrapper and target absent after completion. The probe records the exact test binary SHA-256 and native PID/start-token identities.

The independent real CLI regression crosses DFL/IGN with unblocked/blocked CHLD. Each target reports its actual native disposition/mask, receives one barrier release, exits 17 exactly once, and produces persisted lifecycle completion 17 with the same target identity. Both target and wrapper identities are absent afterward. These are independent executions of delivered product tests; the backend RED/GREEN probe receipts are context, not substituted proof.

The 13 foreground pipeline scenarios comprise 7 direct and 6 managed cases. Target and peer keep the caller group; both can read terminal input. Terminal and external-group INT arrive once per recipient. Wrapper-only TERM reaches the owned target without the peer. Ctrl-Z, background SIGTTIN, foreground continuation and resumed input work. The target-continued-before-wrapper barrier does not cause a stale second stop. All scenarios yield [17,0], restore terminal settings/foreground ownership, and report no remaining recorded native identities. Standalone PTY tests also cover ignored INT, native signal outcomes, target-only stop/continue, controlling-terminal session leaders, background reads and spawn failure. The detached session-leader check still requires a private child group.

### Gate and protection audit

Read and verified the frozen backend handoff: `terminal-repair5.md` SHA-256 `604319388333abe4f0b73ccde9a2203ff93c2df1f963545e6d756f4336804b33`; `terminal-repair5.json` SHA-256 `3340d94d96f1e7fb707121ae06622614ed8b34c0ab4afac2b26e5dc6af30c2ed`. Its 17 command records, referenced artifact bytes and raw command-output hashes were verified. The handoff matches the reviewed implementation and keeps failed probes/assembly attempts separate from passing gates. Audited current-source gates pass: formatting, workspace Clippy with -D warnings, exact unfiltered workspace tests (339), managed preservation (12), historical preservation/hygiene (4), and isolated equivalence (2). The source and native-host metadata, actual argv/cwd, allowance, status, timing and stdout/stderr digests match their raw receipts. `validateCommandReceipt`, `parseCargoExecution` and `validateCargoExecution` checked the normalized receipts and Cargo identities/selectors. The exact workspace run is backend-owned; this checkpoint does not claim a second reviewer-owned unfiltered execution.

The isolated equivalence run used its recorded temporary checkout and exact inner `bun test scripts/harness/equivalence.test.ts` argv. I read all 177 copied input bytes/modes, checked the copied invocation/equivalence artifact hashes, and verified 2 pass/0 fail in its raw output. The equivalence suite ran in the copied checkout and did not regenerate original-checkout harness-speed receipts.

All 97 previously retained evidence artifacts match their frozen hashes. Migration `crates/arc-storage/migrations/0001_phase0.sql` remains byte-identical to HEAD, SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. Maximum Rust/SQL source length remains 474 lines. There were 64 indexed and 97 eligible retained evidence files before the two new report pairs; with the pairs, 101 remains below 150. Historical ignored raw run files are recorded separately and excluded from the retained/index budget. No assertion, floor, accepted contract, manifest, lockfile, coverage accounting, baseline, or storage/IPC source changed.

An initial reviewer audit mistakenly counted ignored raw run files against the retained-evidence limit and failed its own assertion. The failure remains in `source-audit.*`; `source-audit-v2.*` applies the existing index/nonignored scope and passes. This was a review-tool counting error, not a product failure. No failed execution is presented as a pass.

### Acceptance criteria checklist

- [x] Read oma-qa and retained attempt-4 implementation/review evidence.
- [x] Inspect the exact five-file repair diff against verified prior source bytes.
- [x] Independently execute the original exact inherited-blocked case under SIG_IGN plus blocked CHLD before test-wrapper exec.
- [x] Verify actual target disposition/mask, native exit 17, lifecycle completion 17, and native cleanup.
- [x] Verify guard restoration after normal wait, early return and spawn failure; verify post-reap ownership clearing.
- [x] Reproduce ordinary caller-group terminal behavior, standalone/session cases, and native stopped/continued consumption.
- [x] Audit current-source formatting, Clippy, unfiltered workspace, preservation and isolated equivalence receipts.
- [x] Verify source scope, migration, source-length bound, frozen prior evidence and retained-evidence budget.
- [x] Freeze this separate report and companion without accepting downstream lifecycle work.

Reviewer-owned retained files changed are only `terminal-repair5-independent-review.md` and `.json`. Unique ignored raw evidence is under `target/reviewer-terminal-repair5-6db167ca-1e5b-445f-99a8-c8910d4bdbda`. No product, fixture, previous evidence, skill definition, mission state or OMA plan was edited. No injected OMA claim/run/session identity was available; none was invented. Serena tools were unavailable, so review used scoped reads of named source files and ignored retained source copies; no native project-code search substituted for Serena. No relevant memory evidence was used.

No cancellation API, injected absolute deadline, or remaining cumulative allowance was exposed. No attempt/deadline/budget was reset. No coverage collection, AV operation, browser/GUI, Playwright, install, download, separate build, publication, push, merge, outreach or spending occurred. Task-7 collecting coverage, every crate's production-line floor, full harness/final live evidence, task-8 independent final QA and Chief acceptance remain downstream. Desktop, native Windows and Phase 1 scheduling are outside this checkpoint.
