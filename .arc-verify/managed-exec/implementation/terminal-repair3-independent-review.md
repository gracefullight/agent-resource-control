## Review Result: FAIL

Status: failed product checkpoint; assigned independent reproduction and mechanism review completed. This is task-6-terminal attempt 3, not final lifecycle acceptance. No conforming repair was demonstrated under the fixed one CLI-owned direct `Command` child boundary.

### CRITICAL

None confirmed in this scope.

### HIGH

- `crates/arc-cli/src/exec.rs:98`, with `crates/arc-cli/src/exec/terminal.rs:59` and `:67` — A private child process group and foreground handoff split the caller pipeline. The independent exact native test exits **101**: direct **6/6**, managed **0/5**. Managed peer terminal reads stop with SIGTTIN; terminal SIGINT reaches the producer once and peer zero times; Ctrl-Z records only the wrapper stopping. The accepted caller-job behavior remains broken. Remediation must preserve the caller job while retaining exact wrapper-only forwarding and native wait ownership. The following topology change is necessary but insufficient; it must not be promoted alone because current unconditional forwarding would duplicate group delivery.

```rust
// Required topology direction; not a verified complete repair.
command.args(&argv[1..]); // inherit the caller process group
// Remove process_group(0) and the private-group tcsetpgrp handoff.
// Retain direct Command-child ownership, unreaped-PID identity and native wait.
// Implement a proven receipt/provenance mechanism before forwarding signals.
```

Concrete regression requirements, preserving the existing comparisons:

```python
assert peer_info["pgid"] == producer_info["pgid"] == job["group"]
assert terminal_int_counts == group_int_counts == [1, 1]
assert wrapper_pid_term_counts == [1, 0]
assert outcomes == [17, 0]
assert all_recorded_identities_absent
```

No complete replacement mechanism was validated. The NOTE_SIGNAL experiment below does not supply the missing mechanism.

### MEDIUM

None separately confirmed.

### LOW

None separately confirmed.

### Independent execution and source binding

The exact command was run from the assigned mission workspace:

```text
cargo test -p arc-daemon --test managed_exec pipeline::native_foreground_pipeline_input_signals_stop_resume_and_exit17 -- --exact --nocapture
```

UTC interval: **2026-11-08T15:18:21.927228+00:00–15:18:36.063140+00:00**. Actual Cargo PID **53778**, exit **101**, libtest **0 passed / 1 failed / 17 filtered**. The supervisor allowance was **3,600,000 ms**; no timeout occurred. Supervisor success only means that it recorded the Cargo result.

Native host: **Darwin 27.2.0, arm64, Eunkwangui-MacBookPro.local**, kernel `xnu-13432.41.4~25/RELEASE_ARM64_T6000`; Python **3.14.10**. Full actual argv, cwd, host, times, process ID, exit status and stdout/stderr/input hashes are in `focused.receipt.json`.

All **176** `sourceInputs` entries in retained `terminal-repair2.json` matched bytes and modes before and after both independent commands. Their retained combined source identity is `51b7a5a6ba911d26e58d00e40f74485e54a935097f0f9207b6964b1ec6027411`; this reviewer verified every bound entry rather than asserting a newly recomputed combined digest. The retained receipt itself hashes to `0afd1a7bdef2e97ab891856fc15492f1782ba35d1594d2ff94a49cace26da8d3`.

All eleven focused scenarios report controller exit **0**, all recorded identities absent and no remaining identities. Direct cases exit `[17, 0]`. Failed managed cases remain failures even though their cleanup completed. Parsed full scenario records are retained separately from the untouched raw command output.

### NOTE_SIGNAL mechanism assessment

The backend supplied the frozen native Python probe `target/terminal-repair3-8e0645a0-1776-4bfd-9e04-09b9925e9ac2/signal-receipts.py`, SHA-256 `66575a9265ba6e3622e7708d15b48ebb988b56d178263841c9e4ea47e8e715b0`. The reviewer inspected it and independently replayed it with a fresh `/tmp/arc-sig3-review-e482dee1` root and the existing absolute fixture/arcctl paths. Exact actual argv is in `signal-receipts.receipt.json`.

The replay ran **2026-11-08T15:19:14.341400+00:00–15:19:14.668085+00:00**, PID **53922**, exit **0**, with a **3,600,000 ms** allowance. No timeout occurred. All four direct fixture cases passed and reported controller exit 0, all recorded identities absent and no remaining identities. This exit proves the probe's assertions and baseline prerequisites, not product repair acceptance.

| Native observation | Result |
| --- | --- |
| Registration with EV_RECEIPT | EV_ERROR receipt with `data=0`, confirming successful registration |
| Terminal SIGINT, external group SIGINT, direct PID SIGTERM | One observed child event each: `fflags=0x08000000`, `data=0` |
| Two separately acknowledged SIGINT deliveries before drain | Participant count advances **1 → 2**; subsequent drain returns **one** NOTE_SIGNAL event, `data=0` |
| Probe and fixture/arcctl bytes before/after | Identical hashes |

The first `childEvents: []` in the provenance record is deliberately emitted without reading kqueue. It is **not** an observation that the queue was empty. Deferring the first drain creates the tested coalescence interleaving; acknowledged target counts establish two actual deliveries before the single observed event. An observer can be delayed in ordinary execution, so a lossless receipt mechanism must handle this interleaving.

The `wrapper-term` label is inherited from the direct baseline: this probe sends TERM to the direct producer PID. It does not exercise an ARC wrapper forwarding TERM. No managed case, preattach race, ignored-disposition case or arbitrary stop/CONT interleaving is accepted by this probe.

The result excludes treating this EVFILT_PROC NOTE_SIGNAL interface as a lossless per-signal, per-delivery child acknowledgment stream. It supplies neither the signal number nor a delivery count; unrelated or multiple events can collapse. Its absence at a nonblocking read also cannot prove that a concurrent group signal will never reach the child. No timing-based forwarding decision was implemented or approved.

Apple's archived [kqueue manual](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/EV_SET.2.html) describes aggregation into one kevent and describes NOTE_SIGNAL as notification that a process was sent a signal. The public [XNU process filter](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_event.c#L1108-L1242) masks off low hint data, ORs event bits, and returns the stored hook data; signal count accumulation is absent there. This explains the native result, but the public main branch is not claimed to be the exact running kernel source.

The public [XNU signal filter](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_sig.c#L3051-L3152) restricts EVFILT_SIGNAL attachment to the calling process and counts its own signal events. That is not an external observer's counted child receipt. The installed SDK header confirms NOTE_SIGNAL `0x08000000`, NOTE_PDATAMASK `0x000fffff` and EV_RECEIPT `0x0040`; its digest is retained in `probe-inputs.json`.

These observations reject this receipt strategy. They do **not** prove every kqueue strategy or native implementation impossible. No exec-in-place, second observer/helper ownership, private-target fallback or architecture substitution was introduced. Under the fixed boundary, exact signal discrimination and compatible stop/continue behavior remain unresolved.

### Evidence and verification limits

Reviewer raw directory: `target/terminal-repair3-review-e482dee1-7b1a-4771-89c8-998a48bdb7ca/`. It retains the supervisor source, frozen probe copy, named inspected source copies, actual command receipts, raw outputs, parsed scenarios, all per-input byte/mode bindings and the analysis verification record. The companion JSON lists SHA-256 hashes for every raw artifact and this report. Reports are hashed separately to avoid recursive hashes.

Only the exact focused Cargo test and supplied native Python probe were independently executed. Broader fmt/Clippy/workspace/coverage/harness gates were not rerun by this reviewer in this assignment. No standalone build, browser, GUI, Playwright, installation, download or collector ran. npm audit, Bandit and Lighthouse do not apply to this Rust/native CLI scope. No further security, performance or accessibility defect was confirmed in this scoped review.

The prior review's missing generated pycache discrepancy remains prior evidence; it was not repaired, rewritten or hidden here. Source-input equality is against the retained 176-entry delivered inventory. Serena was absent, so named paths were read directly. No relevant ARC memory evidence was found or used. No OMA claim/task/run/session identity was injected; none was invented and no mission state was changed.

### Acceptance criteria checklist

- [x] Read the QA skill, shared execution policy, retained independent review and named sources.
- [x] Independently execute the exact focused command with at least 3,600,000 ms allowance.
- [x] Verify all retained sourceInputs bytes/modes before and after each command.
- [x] Record native host, actual argv/cwd, times, exit status and output hashes.
- [x] Inspect and independently replay the frozen backend probe and verify its result limits.
- [x] Preserve product source, prior evidence, contracts, baselines, manifests and mission state.
- [ ] Demonstrate a conforming repair for caller-job topology, exact signals and stop/CONT behavior.
- [ ] Complete final lifecycle acceptance and downstream task-8 QA.

### Files changed

Only `.arc-verify/managed-exec/implementation/terminal-repair3-independent-review.md`, its `.json` companion, and the unique raw artifacts listed above were written by this reviewer. The requested unique temporary socket root is retained. Product files and prior artifacts were not edited.
