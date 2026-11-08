## Review Result: PASS

Status: completed independent task-6-terminal attempt-4 repair review. Verification completeness: completed for this checkpoint. No unresolved CRITICAL, HIGH, MEDIUM or LOW finding remains. This review does not accept task-7, task-8 or the complete lifecycle feature.

The delivered source identity is `2132331368d21281806b33bc265a2183fb1bd1702a09144c9c3b95386ec08864`, covering all 177 current input bytes/modes from the unmodified `managedInputs(root)` inventory. Every final reviewer command began and ended on that identity. The independent unfiltered `cargo test --workspace` passed 336 tests across 32 harnesses, with 0 failed and 0 ignored; its managed-exec target passed all 19 tests. Final native focused checks and both reviewer-found defect reproductions also passed.

### CRITICAL

None.

### HIGH

None unresolved.

### MEDIUM

None.

### LOW

None.

### Resolved during this review

1. **HIGH — `crates/arc-cli/src/exec/signals.rs:176` at source `3949154e9ef4aaaf6d13ae40b9e2e05b9accb5109d37eea8630c9a3e9917ccf5`: inherited blocked SIGCHLD prevented the native wait from waking after a later child exit.** The existing isolated native test passed normally, then failed with a 15-second native isolation timeout when its parent blocked SIGCHLD before exec. `blocked-chld-red.json` preserves exit 101. The implementation owner corrected the private sleep mask while preserving the caller mask for child exec and return:

```rust
let mut wake_mask = mask.0;
unsafe {
    libc::sigdelset(&mut wake_mask, libc::SIGCHLD);
    libc::sigdelset(&mut wake_mask, libc::SIGCONT);
}
// After consuming available native child events:
unsafe { libc::sigsuspend(&wake_mask) };
```

The new native regression releases child exit only after WNOHANG reports no event, verifies the child's inherited blocked mask, and checks the original mask after return. A waitid(WNOWAIT) development barrier was removed because it consumed the notification being tested. All 4 final signal tests and the unchanged reviewer reproduction now pass.

2. **HIGH — `crates/arc-cli/src/exec/terminal.rs:5` before repair: session-leader identity overrode an actual controlling terminal.** The corrected independent probe launched each target after setsid/TIOCSCTTY. Direct execution had foreground access and exited 17. The managed child had a private background group, lost foreground access, and stopped the wrapper. `session-tty3-red.*` preserves that reproduction. Signal-only edits had occurred before this probe, so its current-source snapshot is not claimed as the compiled binary identity; the retained `session-probe-bindings.json` binds the existing executable bytes. The implementation owner removed the early getsid/PID exclusion. Detection now checks terminal ownership directly:

```rust
if [0, 1, 2].into_iter().any(|fd| unsafe { libc::tcgetpgrp(fd) } > 0) {
    return true;
}
// Fall back to opening /dev/tty; a real controlling terminal shares the job.
```

The added Rust PTY regression and the corrected independent direct/managed replay now pass with foreground access and exit 17. The separate nonterminal session-leader regression still proves that a detached target receives its own group.

### Native execution and receipt verification

Native host: Darwin 27.2.0, arm64, Eunkwangui-MacBookPro.local. Cwd for each outer command was the assigned workspace (`.` in compact receipts). Each supervisor allowed 3,600,000 ms. Times below are actual UTC timestamps. Every command exited 0 with unchanged 177-input identity.

| Exact command | Executed result | UTC interval |
| --- | --- | --- |
| `cargo test -p arc-daemon --test managed_exec pipeline::native_foreground_pipeline_input_signals_stop_resume_and_exit17 -- --exact --nocapture` | 1 passed, 0 ignored | 2026-11-08T15:41:20.391863+00:00 to 2026-11-08T15:41:22.172304+00:00 |
| `cargo test -p arc-cli --bin arcctl exec::signals::tests -- --nocapture` | 4 passed, 0 ignored | 2026-11-08T15:41:32.154532+00:00 to 2026-11-08T15:41:32.324382+00:00 |
| `cargo test -p arc-daemon --test managed_exec terminal:: -- --nocapture` | 4 passed, 0 ignored | 2026-11-08T15:41:35.477217+00:00 to 2026-11-08T15:41:36.903684+00:00 |
| `cargo test -p arc-daemon --test managed_exec pipeline::native_session_leader_without_terminal_preserves_execution_and_exit17 -- --exact --nocapture` | 1 passed, 0 ignored | 2026-11-08T15:41:43.228303+00:00 to 2026-11-08T15:41:43.970666+00:00 |
| `python3 target/reviewer-terminal-repair4-1f167f8e-3f3e-4d88-bff8-753dce8d44ed/blocked-chld.py` | 1 passed, 0 ignored | 2026-11-08T15:41:49.359701+00:00 to 2026-11-08T15:41:49.476163+00:00 |
| `python3 target/reviewer-terminal-repair4-1f167f8e-3f3e-4d88-bff8-753dce8d44ed/session-tty4.py` | 2 native direct/managed cases passed | 2026-11-08T15:41:49.618149+00:00 to 2026-11-08T15:41:49.790340+00:00 |
| `cargo test --workspace` | 336 passed, 0 ignored | 2026-11-08T15:41:53.277460+00:00 to 2026-11-08T15:42:35.441807+00:00 |

The JSON companion retains exact argv, cwd, host, timestamps, owned process IDs, allowances, stdout/stderr SHA-256 values, raw output paths and each Cargo test identity/count. Imported `validateCommandReceipt`, `parseCargoExecution` and `validateCargoExecution` checked all five final Cargo receipts, including positive execution counts and matching filters. The two extra Python argv are reviewer-owned native probes, not replacements for required Cargo commands. Their test/native outputs and hashes were verified separately.

The pipeline run contains 13 successful scenarios: 7 direct and 6 managed. Both participants retain the caller group. Producer and peer terminal input work. Terminal and external-group INT are counted once each at producer and peer. Wrapper-only TERM reaches only the target. Ctrl-Z stops the job; background continuation and terminal read produce SIGTTIN; foreground continuation restores input. The target-continued-before-wrapper barrier proves stale stop evidence does not suspend the wrapper again. All scenarios return [17,0], preserve terminal modes, restore controller foreground ownership and report no remaining recorded PID/start-token identities.

Signal units establish distinct pending TERM/HUP/USR1/INT, inherited ignored USR2, no shared-job INT/QUIT forwarding, ownership detachment and pending signal suppression after consuming reap. The native receiver entry is a helper; 3 of the 4 reported unit tests exercise substantive cases. Standalone PTY tests also preserve ignored INT, native INT/QUIT/KILL wait status, target-only stop/continue, input, spawn-failure behavior and terminal restoration. Signal forwarding uses attached unreaped target PIDs; ARC issues no peer-group signal. The wait loop blocks between native notifications instead of polling continuously. Native CLI keyboard behavior is covered; no web/UI surface changed.

The initial normal-mask focused and workspace passes on source `3949154e...` are retained as historical evidence, not delivered-source passes. All final passes use `2132331368d21281806b33bc265a2183fb1bd1702a09144c9c3b95386ec08864`. Two invalid session probe attempts are also retained: startup rejected the review artifact directory's 0755 permissions, then an undrained PTY blocked direct fixture output. The corrected probe uses 0700 storage, a short cwd and a PTY reader. Corrected direct children and daemon were reaped; recorded failed-probe/corrected-probe participant IDs were independently absent via native proc_pidinfo. No process enumeration or ps bypass was used.

### Backend handoff and protection audit

Read the frozen `terminal-repair4.md` and `.json`. Their hashes are respectively `b5c1c21eb931e1750b6a641c09d944909d1dc684c134a48d82fcdfeb50c9d989` and `0049e4b150db28b2e36b1b1b86bc8bdaeb951493894a9c90d505aaa18ee8ad78`. The handoff describes the current topology, wait mechanism, native results and downstream work accurately. Its 27 command records, receipt bytes, raw output digests, input references and source-hash serialization were checked. Failed development commands retain their nonzero outcomes and earlier input scopes.

Audited successful delivered-source backend receipts: formatting; warning-denying workspace Clippy; exact workspace tests; managed-exec protection (12 tests); isolated harness equivalence (2 tests). Each matches the current 177 inputs and native host. Read every copied equivalence input and verified its hash in the isolated checkout, its actual cwd and exact inner `bun test scripts/harness/equivalence.test.ts` argv, and its retained invocation/equivalence artifact hashes. The original harness-speed evidence was not used as a fresh final-source result. The historical preservation/hygiene run passes 4 tests; only its unchanged tooling/fixed-history scope is reused, not its earlier full-source identity.

The implementation owner changed seven existing owned paths and added `crates/arc-cli/src/exec/signals/tests.rs`; this independently matches the preceding 176-input inventory comparison. The JSON lists them. Migration 0001 remains byte-identical, SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. Maximum Rust/SQL length remains 474 lines. No manifest, lockfile, contract, storage/IPC implementation, protection tooling or baseline was changed by this repair. Before writing this independent pair, the handoff audit reported 64 indexed and 95 retained evidence files; the pair raises retained count to 97, below 150.

### Acceptance criteria checklist

- [x] Inspect all eight assigned source/test paths and retained repair-3/contract evidence using oma-qa.
- [x] Independently verify caller-group topology, producer/peer input and exact INT/TERM recipient counts.
- [x] Verify Ctrl-Z, background SIGTTIN, foreground resume, stale stop/continue, exit 17, native signal status and terminal restoration.
- [x] Verify distinct pending signals, ignored dispositions and ownership synchronization across attachment/reap.
- [x] Reproduce and close blocked-SIGCHLD and controlling-terminal session-leader defects with GREEN final-source tests.
- [x] Run exact unfiltered native workspace tests on current inputs and validate raw identities/counts and receipt hashes.
- [x] Audit source-bound backend formatting, Clippy, managed preservation, isolated equivalence and frozen handoff bytes.
- [x] Preserve earlier evidence and source ownership; freeze this report and companion before handoff.

Reviewer-owned retained changes are only this report and its JSON companion; fresh raw evidence is under `target/reviewer-terminal-repair4-1f167f8e-3f3e-4d88-bff8-753dce8d44ed`. No product/test source, previous report, contract, baseline, manifest, mission state or saved OMA plan was edited. No OMA claim/run identity was injected, so no mission/plan mutation or invented run identity was performed. The supplied attempt 4 / round 11 of 16 and 3,600-second Actor allowance were retained; no cancellation API, absolute deadline or remaining cumulative budget was exposed or reset.

Task-7 collecting coverage, per-crate 80% production-line floors, the full harness and final live evidence remain downstream. Task-8 final independent QA and Chief acceptance remain required. No collector, browser/GUI/Playwright, installation, download, separate build, publication, push, merge, outreach or spending occurred. No relevant memory evidence was used.
