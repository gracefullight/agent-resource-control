## Review Result: FAIL

Status: failed checkpoint. Independent review execution completed. This is task-7 checkpoint QA, not task-8 final QA or final acceptance.

The exact native regression reproduced the product defect on macOS 27.2 (26B123), Darwin 27.2.0, arm64, Eunkwangui-MacBookPro.local. Six direct scenarios passed; all five managed scenarios failed. All eleven scenarios reported no remaining recorded PID/start-token identities and controller exit 0. Inspected source/test hashes remained unchanged during the final run.

### CRITICAL

None confirmed in this scope.

### HIGH

- `crates/arc-cli/src/exec.rs:98` — The unconditional private child group, combined with `exec/terminal.rs:59` and `:67` foreground transfer, splits the caller pipeline. A peer in the original job group loses terminal input and triggers SIGTTIN; terminal Ctrl-C reaches the managed producer but misses the peer; Ctrl-Z stops the wrapper while leaving the peer running. The direct baseline passes each behavior. Fix the inherited process-group topology and solve the forwarding interface conflict below together. The following is a necessary topology repair, **not a complete or verified forwarding fix**:

```rust
command.args(&argv[1..]); // inherit caller job process group
// Remove process_group(0) in both launch paths.
// Remove the child's pre_exec tcsetpgrp handoff.
// Do not replace PID-only wrapper forwarding with kill(-caller_pgid, signal).
```

### MEDIUM

None separately confirmed. Remaining evidence gaps are below.

### LOW

None separately confirmed.

### Native execution evidence

```text
cargo test -p arc-daemon --test managed_exec pipeline::native_foreground_pipeline_input_signals_stop_resume_and_exit17 -- --exact --nocapture
```

Cwd is the assigned workspace. Final run: 2026-11-08T13:46:59.440321Z to 13:47:15.907573Z, cargo PID 97287, exit 101, timeout allowance 3,600,000 ms. Libtest: 0 passed, 1 failed, 0 ignored, 16 filtered out. The subprocess supervisor itself completed normally; cargo's failure is retained explicitly. Raw files: `target/pipeline-independent-review-final/stdout.txt`, `stderr.txt`, and `receipt.json`. The JSON companion embeds command, timestamps, host, hashes and compact scenario outcomes. An earlier independent run before the provenance subcase also failed, direct 5/5 and managed 0/5; its raw files remain under `target/pipeline-independent-review/`.

| Scenario | Direct | Managed |
| --- | --- | --- |
| Pipeline peer terminal input | PASS, exit [17,0] | FAIL: peer SIGTTIN (21); foreground moved to child PG 97322, caller PG 97320 |
| Terminal SIGINT | PASS, counts [1,1] | FAIL: counts [1,0], peer missed Ctrl-C |
| External group SIGINT | PASS, counts [1,1] | Counts [1,1], then FAIL on peer SIGTTIN |
| Wrapper-only SIGTERM | PASS, counts [1,0] | Counts [1,0], then FAIL on peer SIGTTIN |
| Ctrl-Z, background continuation, foreground resume | PASS, exit [17,0] | FAIL before continuation: only wrapper 97528 stopped (SIGSTOP 17); peer 97529 did not stop |
| Same-sender PID versus group SIGINT provenance | PASS, sameNativeProvenance=true | Direct-only diagnostic |

The fixture uses a real OS producer/peer pipe, a non-orphaned caller job group, terminal reads, barrier control sockets, and a continuously drained PTY. Native controller cleanup checks recorded PID/start-token identities rather than matching process names. The daemon is stopped by the Rust fixture before its failing assertion. `ps` was denied; no alternate access was attempted, and this report does not claim an independent whole-host process inventory.

### Signal forwarding interface assessment

The retained provenance subcase sends SIGINT to the producer PID and then its group from the **same native controller 97316**, within the same job. Producer counts advance 1 to 2; peer counts advance 0 to 1. Both producer receipts have `code=0` and `sender=97316`. `sameNativeProvenance=true` is asserted by the test. The extra count establishes that these are two observed deliveries; sender/code cannot distinguish the two target forms.

With wrapper and Command child in the inherited caller group, a PID-only wrapper signal must be forwarded to the child, while an external group signal must not be forwarded because the child already receives it. The current handler (`exec/signals.rs:10-24`) has no native target identifier or child-delivery acknowledgment for that choice. Foreground membership only establishes terminal delivery eligibility; it does not identify the origin of a particular received signal. Sender-PG inference, timing windows, and child pending-mask polling are not exact delivery proofs. Removing the private group while retaining unconditional `kill(child, signal)` can therefore introduce duplicate external group delivery.

No validated drop-in strategy was found under the unchanged selected CLI-owns-one-Command-child model. This is an interface conflict for that model and receipt-based forwarding, not a claim that every possible native architecture is impossible. Exec-in-place with a separate observer would avoid this forwarding choice but changes selected ownership and native wait semantics. Moving the wrapper to a helper-created group avoids shared receipt delivery but introduces another process and breaks ordinary shell group stop/continue routing unless an additional proven protocol repairs it. Neither alternative is implemented, authorized here as a contract change, or accepted as a solution.

`exec/signals.rs:47-49` preserves inherited ignored dispositions; `:65-71` attaches a child and sends the pending slot; `:80-88` observes without reaping and `:127-128` disables forwarding before reap. Preserve these safeguards in any repair. The pending slot stores only one signal; this checkpoint does not establish preservation of multiple distinct preattach signals. `:92-117` consumes a stop, stops the wrapper, then resumes the child; stale stop versus intervening continuation needs explicit verification. Do not forward wrapper-only events to unrelated caller-group peers.

The earlier native PTY regression at `crates/arc-daemon/tests/managed_exec/terminal.rs:146` asserts managed `pgid == pid`. It encodes the private-child-group design that the real pipeline disproves. Backend must reconcile that assertion with inherited caller PG behavior while preserving its input, mode, wait-status and restoration checks. It is not a reason to weaken the new pipeline regression.

### Acceptance criteria checklist

- [x] Independently ran the exact native check with at least 3,600,000 ms allowance.
- [x] Recorded native host, exact argv/cwd, exit, raw output digests and inspected source/test hashes.
- [x] Confirmed direct 6/6 baseline and same-sender PID/group provenance.
- [x] Confirmed cleanup of all recorded identities in all eleven scenarios.
- [ ] Managed child inherits caller job PG and preserves peer terminal input.
- [ ] Terminal signals reach the whole job once.
- [ ] Same-group repair preserves exact external group and wrapper-only forwarding.
- [ ] Managed whole-job stop, background continuation, foreground resume and exit17 complete.
- [ ] Ignored dispositions, pending preattach signals, native signal outcomes, restoration and stale-stop continuation are fully reverified after a coherent repair.

### Files changed and limits

Reviewer-owned deliverables are only `pipeline-independent-review.md` and `.json` in this implementation directory. Raw diagnostic output is retained under ignored `target/`. No product source, tests, contracts, baselines, prior Actor reports, mission state, backend verification receipts or inputs were changed. No collector ran.

This scoped Rust/native checkpoint does not need npm audit, bandit, Lighthouse or browser checks. No browser/GUI/Playwright, download, separate build command, publication or external messaging was used. Explicitly authorized cargo test compiled its test target as needed. Broader lint/workspace/preservation receipts remain backend-owned and are not claimed here.

Serena tools were absent; named files were read directly without repository code discovery. No injected claim/run/session identity was supplied; named-artifact ownership forbade creating OMA state or generic reports. No relevant memory evidence was used. No final acceptance is claimed.
