## Review Result: FAIL

Status: failed delivered-source checkpoint; independent review execution completed. Delivered source identity is `51b7a5a6ba911d26e58d00e40f74485e54a935097f0f9207b6964b1ec6027411` (176 inputs), independently recomputed and stable across the final native check. The hybrid experiment was rejected and restored; its passing checks remain separately bound to its candidate identity. This is task-6-terminal review, not final lifecycle acceptance.

### CRITICAL

None confirmed by this checkpoint.

### HIGH

- `crates/arc-cli/src/exec.rs:98` (delivered product), with `exec/terminal.rs:59` and `:67` — The private child group and foreground handoff still split the caller pipeline. The final independently executed exact native check fails: direct 6/6, managed 0/5. Managed peers lose input with SIGTTIN; terminal SIGINT reaches producer once and peer zero times; Ctrl-Z records only the wrapper stopping. This blocks the accepted terminal behavior. Fix inherited caller-job topology and exact forwarding together; simply removing the private group while retaining unconditional forwarding is insufficient, as the independent duplicate-delivery probe shows.

```rust
// Necessary topology change, not a complete verified signal repair:
command.args(&argv[1..]); // retain inherited caller process group
// Remove process_group(0) and the private-group tcsetpgrp handoff.
// Keep wrapper-only forwarding scoped to the unreaped target PID;
// add a proven mechanism to avoid forwarding already delivered group signals.
```

- `crates/arc-cli/src/exec/relay.rs:175` (frozen hybrid candidate) — The sessionleader fallback fails a supported launch with a controlling terminal and an existing caller-group peer. The independent actual-product probe records direct terminal input and exit 17; the managed case reports `SPAWN_FAILED`, exits 1, and never connects its target fixture. Read-only SQLite inspection confirms `SPAWN_FAILED`, `PERMISSION_DENIED`, `os_code = 1`, with null child identity, under job `469ec8aac1827b2c73ab975b0606507d`. The same owned healthy daemon successfully serves the no-terminal managed case. This is a confirmed launch failure, not a daemon prerequisite failure. The exact failing native call inside spawn was not instrumented; no stronger root-cause claim is made.

  Fix: replace the blanket private-target sessionleader path with a native mechanism preserving the original caller job and direct CLI-owned Command child. Retain the no-terminal regression and add this controlling-terminal/peer comparison as a required regression. Removing only the failing terminal operation still leaves the privately grouped target outside the foreground caller job; foregrounding that private group instead reproduces the independently verified peer-input defect. No complete drop-in source repair satisfying all fixed requirements was established here; do not promote the hybrid.

```python
# Regression requirements implemented by the retained actual session probe:
assert direct_ctty_peer["pass"] and direct_ctty_peer["leaderOutcome"] == 17
assert managed_ctty_peer["pass"] and managed_ctty_peer["leaderOutcome"] == 17
assert managed_ctty_peer["allRecordedIdentitiesAbsent"]
```

The snippet is concrete remediation test code, not a claimed complete product fix.

### MEDIUM

None separately confirmed.

### LOW

None separately confirmed.

### Delivered-source binding and final failed checkpoint

After rejecting the hybrid, the owner restored five candidate product/fixture paths from preserved initial bytes and removed the new relay module. Only the new no-terminal sessionleader regression is intentionally retained in product source, at `pipeline.rs` SHA-256 `ca6a9637046a24e327797499409fab2ddd4fb87516e5a3c7003f102e1b7f9754`. Independent comparison of the 177 initial input bindings with the current 176 confirms exactly two differences: this regression and the missing preexisting generated pycache described below. No other original input binding differs; the experimental relay is absent.

The owner disclosed inadvertently removing preexisting `crates/arc-daemon/tests/managed_exec/__pycache__/pipeline.cpython-314.pyc`, whose original SHA-256 was `a8b9b17bdcaaa4e797a00175a2192878f0a0272b6e36dbb353f87255422d5739`. Exact-byte recovery failed. This cache was part of the original input inventory even though generated, so full 177-input restoration is **not** claimed. `independent-restore-comparison.json` records the discrepancy, and the delivered identity includes its absence. The reviewer did not alter the cache or reconstruct replacement bytes. No source definition or acceptance rule was changed to hide the discrepancy.

The reviewer independently reran the exact focused pipeline command on delivered sources at **2026-11-08 15:09:25.784–15:09:39.625 UTC**. Exit **101**; libtest **0 passed, 1 failed, 17 filtered**; native scenarios **direct 6/6, managed 0/5**. All eleven scenarios report controller exit 0, all recorded identities absent and no remaining identities. Delivered hash `51b7a5a6ba911d26e58d00e40f74485e54a935097f0f9207b6964b1ec6027411` and 176-input inventory stayed unchanged before/after. Raw receipt, stdout/stderr, compact native scenario copies and inspected source copies are `restored-focused.*`, `restored-focused-scenarios.json`, `restored-inputs.json` and `restored-source/` in the reviewer raw directory. The supervisor exited normally while retaining Cargo exit101; it does not turn the failed check into a pass.

Owner-recorded delivered checks: fmt exit0, Clippy with `-D warnings` exit0, and `cargo test --workspace` exit101 with the managed-exec target **17 passed / 1 failed**. The new no-terminal regression passes in that run. Their receipt and output digests were independently checked, but these are owner executions, distinct from the independent focused reproduction. Receipt paths are `target/terminal-repair2-8cee663f-d445-43c4-a34a-4539e40d1d04/restored-{fmt,clippy,workspace}.json`; `owner-restored-receipts.json` records verified digests. The owner restoration receipt is `restore.json`, SHA-256 `dac49661ffe9915013087e5f1f86aa70a913a47f8029f184c555eee0930582a0`. The second rejected patch is `.arc-verify/managed-exec/implementation/terminal-repair2-candidate.patch`, SHA-256 `dbae7bbefd16a74095f36718ebc5c831d28990d4ea179b6f49dc7c7abc0e0bcc`.

No whole-workspace pass, final coverage, full harness, or final lifecycle acceptance is established. Required behavior still needs a conforming implementation; distinct preattach signals and arbitrary stop/CONT interleavings remain unproved. No additional security, performance or accessibility defect was confirmed in this scoped native CLI review.

### Independently executed frozen-candidate checks

The reviewer independently recomputed `managedInputs` using the repository function: SHA-256 `6bef1e3b89e1bdaaa504795d42f12e78b81b86c3b1427da9b32b8dc1413d14b5`, **177 inputs**. The coordinator initially stated 178; the recorded function output is authoritative for these receipts. `frozen-inputs.json` retains all per-file digests/modes. The snapshot remained identical before and after each command. Relevant candidate source copies and the exact product-probe script are retained under `frozen-source/` and `frozen-session-probe.py`.

| Command | UTC interval | Result |
| --- | --- | --- |
| `cargo test -p arc-daemon --test managed_exec pipeline::native_foreground_pipeline_input_signals_stop_resume_and_exit17 -- --exact --nocapture` | 2026-11-08 15:06:05.665–15:06:10.664 | exit 0; 1 passed, 17 filtered |
| `cargo test -p arc-daemon --test managed_exec pipeline::native_session_leader_without_terminal_preserves_execution_and_exit17 -- --exact --nocapture` | 15:06:10.734–15:06:11.441 | exit 0; 1 passed, 17 filtered |
| Coordinator-owned `session-probe.py` with reviewer-owned unique raw output argument | 15:06:11.513–15:06:19.729 | exit 1; confirmed controlling-terminal managed launch failure |

All commands ran in the assigned workspace with a 3,600,000 ms allowance. `focused.receipt.json`, `session-leader.receipt.json` and `actual-session.receipt.json` retain full argv, cwd, host, times, PID, status, source identity and output hashes. Arcctl binary SHA-256 was `5a82ebc217b2804e8be5f068c0c9dd7e1a8847242318b3f8d8a256e7ff9d96b9`; arc-daemon was `8def2088a3ec30e52a888d8f893f92eb1ef7d84eed66223e5aa9069bd167e4a4`.

The pipeline passes six direct and five managed scenarios. Each ends with producer/peer exits `[17, 0]`, controller exit 0, and all recorded identities absent. Managed terminal/group INT counts are `[1, 1]`; wrapper-only TERM counts are `[1, 0]`. The anchor identity is included in exercised cleanup checks. These are candidate results, not evidence for later restored product. The direct-only provenance diagnostic again records identical sender/code for PID versus group delivery.

The actual session probe passes direct and managed no-terminal exit17, and direct controlling-terminal peer input/exit17. Only the managed controlling-terminal peer case fails. The fixture's target accept timeout follows the CLI's observed spawn failure; it is not the sole failure evidence. Managed observer PID 26773/start 1794150371663361 exits 1; peer PID 26774/start 1794150371704602 is absent after cleanup. Owned daemon PID 26767/start 1794150371566007 exits 0 and is absent. Both controlling-terminal cases report all recorded identities absent. The script sets `sys.dont_write_bytecode=True`; it did not regenerate project pycache. SQL rows copied through read-only SQLite access are retained in `actual-session-jobs.json` and `actual-session-job_lifecycle.json`.

### Native alternatives assessment

| Mechanism | Independently observed behavior | Consequence |
| --- | --- | --- |
| Session leader joins live helper group | `setpgid(0, helper)` returns EPERM; self-group setpgid and setsid also return EPERM | Mandatory observer relocation still cannot support this invocation |
| Helper changes parent group | `setpgid(parent, helper)` returns ESRCH | Delegating the same operation to this helper does not repair relocation |
| Session leader retains caller PG; target gets private foreground PG | Native PTY direct peer reads `peer-input\n`; private-group peer read returns EIO (errno 5) | Blanket sessionleader fallback to the original terminal topology still breaks a preexisting peer |
| Wrapper and target share caller PG; no forwarding | Group SIGINT yields target count 1; PID-only wrapper SIGINT yields target count 0 | Wrapper-only forwarding requirement remains unmet |
| Wrapper and target share caller PG; unconditional forwarding | Group SIGINT yields target count 2; PID-only wrapper SIGINT yields target count 1 | Exact group delivery requirement remains unmet |

The delivery test blocks SIGINT only in the wrapper, keeps the target unblocked, and queries the target before releasing the wrapper. A group event is confirmed at the target before the wrapper can forward it. Releasing the wrapper then causes the second observed delivery. Socket query/release barriers establish order; no delay or pending-mask inference selects whether forwarding is needed. This is a native model of the proposed same-group forwarding fallback, not a product instrumentation test.

The PTY test uses `pty.fork`: the wrapper is a session leader with a controlling terminal, the peer retains that caller group, and only the target is moved to a private foreground group. The original caller group is orphaned in this fixture, so the background terminal read returns EIO rather than a SIGTTIN stop. Direct terminal input succeeds under otherwise matching conditions. Both test targets exit 17; peers and wrappers exit 0 and are reaped. The relocation helper and wrappers are also reaped. This establishes cleanup for these owned native processes, not a whole-host inventory.

These examples exclude the tested mechanisms. They do not establish that every native implementation is impossible. Exec-in-place plus a different observer would change the fixed original CLI-owned direct Command-child/wait boundary. No such contract change is accepted here. Tracing, injected target code, or platform-specific interception was neither implemented nor verified and cannot be claimed as a repair. A no-controlling-terminal private-target branch may avoid this particular peer-input failure; that alone does not prove terminal/session, exact signal, or whole-job stop/CONT compatibility.

### Evidence and verification limits

Raw directory: `target/terminal-repair2-review-9393ba54-5a6c-4e82-8802-ba7946f50a99/`. `probe.py` and `session-tty.py` are the independent native probes. `receipt.json` and `session-tty.receipt.json` record exact Python argv, assigned workspace cwd, host, UTC interval, source/probe hashes, exit status and output hashes. Both supervisor allowances are 3,600,000 ms; each probe exited 0 with its assertions satisfied. `stdout.json` and `session-tty.stdout.json` retain process identities, results and wait statuses; stderr files are empty.

Host: Darwin 27.2.0, arm64, Eunkwangui-MacBookPro.local; Python 3.14.10. Native checks ran at 2026-11-08 15:02:45.677–15:02:45.752 UTC and 15:03:16.053–15:03:16.110 UTC. Recorded contract/prior-review/source hashes stayed stable across each probe. Candidate edits by the implementation owner may occur between checks; these records do not freeze or accept that evolving product.

The accepted technical and CLI contracts, prior pipeline review, rejected-candidate review and retained rejected patch were read without editing. The retained patch SHA-256 is `dadd2f2570ad22f86f91ebbcdb953106090af170b0964a986134df9b0972b1fd`. Earlier product failures and candidate passes are retained owner/reviewer evidence, not new executions by this checkpoint. Cargo ran only after the coordinator explicitly froze and authorized the candidate checks above; broader workspace, fmt and Clippy checks are not independently claimed here. npm audit, Bandit and Lighthouse do not apply to this Rust/native CLI scope. No browser, GUI, Playwright, install, download, collector or standalone build command ran.

Serena tools were absent; named source files were read directly. The memory registry had no matching context; no memory evidence was used. No injected OMA claim/run/session identity was supplied, so no identity or mission state was fabricated. This JSON companion is the explicit coordinator handoff.

### Acceptance criteria checklist

- [x] Read retained execution and CLI requirements without changing them.
- [x] Test helper/sessionleader relocation natively.
- [x] Test the controlling-terminal peer counterexample natively.
- [x] Test same-group no-forward and unconditional-forward signal outcomes with barriers.
- [x] Record native host, argv/cwd, times, source/output hashes and owned wait statuses.
- [x] Independently execute the frozen product candidate and reproduce its sessionleader failure.
- [x] Independently bind and reproduce the delivered-source failed pipeline checkpoint.
- [x] Record the exact restoration discrepancy without changing retained evidence.
- [ ] Establish a repair preserving all fixed ownership, sessionleader, signal and terminal requirements.
- [ ] Complete final task-8 QA and lifecycle acceptance (outside this checkpoint).

### Files changed

Retained reviewer changes are only `terminal-repair2-independent-review.md` and `.json` under `.arc-verify/managed-exec/implementation/`, plus unique ignored native raw artifacts under the directory above. Product source, contracts, previous evidence and mission state were not edited by this reviewer.
