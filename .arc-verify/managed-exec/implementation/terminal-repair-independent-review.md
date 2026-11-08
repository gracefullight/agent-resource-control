## Review Result: FAIL

Status: failed candidate checkpoint; independent review execution completed. Task `task-6-terminal`, not task-8 independent QA or final lifecycle acceptance. The candidate passes the requested pipeline and managed execution checks but introduces a confirmed session-leader execution failure. It is not accepted for promotion.

This review binds the frozen candidate, not any later restored product. Coordinator-provided candidate aggregate source hash: `10f3b6354c14f1b78c316d2e061f3c025403eda4f15a84bfc8bf8e6e5bd5c010`. Independent per-file hashes and copies of the CLI execution sources are retained in `target/terminal-repair-qa-143d377a-3df9-4b88-b9b9-d2b2e6fe75bb/candidate-source.json` and `candidate-source/`. The aggregate recipe was not independently recomputed. Source and contract hashes remained unchanged across both independently executed Cargo checks.

### CRITICAL

None confirmed in this scope.

### HIGH

- `crates/arc-cli/src/exec/relay.rs:132` (frozen candidate) — Every managed invocation must join the transient bootstrap's observer-only group. A session leader cannot perform this operation: the native probe returns `EPERM` (errno 1). `exec.rs:96` converts that failure into `NOT_STARTED` after REGISTER/CLAIM, without starting the target. A direct `start_new_session=True` launch exits 17; an ordinary managed launch also exits 17; a managed session-leader launch exits 1 with empty stdout and the daemon-connection diagnostic, despite the same healthy daemon. This violates the accepted caller-session and execution-transparency requirements. The retained last-known job remains `starting`.

  Remediation: replace mandatory observer group relocation with a session-leader-compatible native mechanism that also preserves the existing caller job, exact signal delivery, and the original CLI-owned direct `Command` child/wait boundary. Preserve the regression below alongside the existing pipeline checks. Checking `getsid(0) == getpid()` and returning a clearer error would still reject a supported launch; leaving the observer in the caller group restores the original signal ambiguity. Moving the target to a private group loses the demonstrated pipeline behavior. No complete implementation patch satisfying all these requirements was established here; the unresolved mechanism must return to the implementation owner. This is a conflict in this candidate, not a claim that every native mechanism is impossible.

```python
# With the same healthy daemon and target that exits 17:
ordinary = subprocess.run([arcctl, "exec", "--", *target], env=env,
                          capture_output=True, timeout=30)
session = subprocess.run([arcctl, "exec", "--", *target], env=env,
                         start_new_session=True, capture_output=True, timeout=30)
assert ordinary.returncode == session.returncode == 17
assert ordinary.stderr == session.stderr == b""
```

The snippet is concrete regression code, not a claimed drop-in product fix. The retained `session-probe.py` supplies executable target/daemon setup and cleanup.

### MEDIUM

None separately confirmed.

### LOW

None separately confirmed.

### Independently executed native checks

Native host: Darwin 27.2.0, arm64, `Eunkwangui-MacBookPro.local`. Cwd: the assigned workspace. Checks ran serially, each with a 3,600,000 ms subprocess allowance.

| Check | UTC interval | Result |
| --- | --- | --- |
| `cargo test -p arc-daemon --test managed_exec pipeline::native_foreground_pipeline_input_signals_stop_resume_and_exit17 -- --exact --nocapture` | 2026-11-08 14:50:47.806–14:50:50.013 | exit 0; 1 passed, 16 filtered |
| `cargo test -p arc-daemon --test managed_exec -- --nocapture` | 2026-11-08 14:50:50.015–14:50:53.852 | exit 0; 17 passed, 0 failed |

Raw `focused.stdout`, `focused.stderr`, `focused.json`, `managed.stdout`, `managed.stderr` and `managed.json` are under the reviewer raw directory above. Receipts include argv, cwd, Cargo PID, timestamps, output digests and before/after input hashes.

The focused pipeline check independently passed six direct and five managed scenarios. Input, terminal INT, group INT, wrapper-only TERM and stop/background-continue/foreground-resume each ended with producer/peer outcomes `[17, 0]`. Signal observations were `[1, 1]` for terminal/group INT and `[1, 0]` for wrapper-only TERM. The direct provenance diagnostic again reported identical native sender/code for PID and group delivery. All eleven cases reported controller exit 0 and no remaining recorded PID/start-token identities. Each managed case inventories exactly one live continuation anchor after target readiness; its identity is included in the absence checks. This supports helper cleanup in these exercised paths, not a whole-host process inventory or exhaustive failure-injection proof.

The broader managed check also exercises ignored INT disposition, native termination results, foreground modes/input, stop handling, background terminal reads, spawn failure, exact argv/environment/streams/closed descriptors, retry behavior and daemon loss. It does not prove every stop/CONT interleaving or distinct multiple-signal preattach delivery.

### Session-leader evidence

Probe: `python3 target/terminal-repair-qa-143d377a-3df9-4b88-b9b9-d2b2e6fe75bb/session-probe.py`, 2026-11-08 14:51:34.947–14:51:35.086 UTC, exit 0. A successful probe process means its comparisons were recorded; it does not make the failed managed case pass.

| Case | Native identity / topology | Observed result |
| --- | --- | --- |
| Direct session leader | PID/PGID/SID 99805; start token 1794149494974336 | exit 17, native identity JSON, empty stderr |
| Ordinary managed | observer 99808 / start 1794149495001741; target 99813 in caller PG 99775 | exit 17, native identity JSON, empty stderr |
| Managed session leader | observer 99817 / start 1794149495055402 | exit 1, no target output, `NOT_STARTED` daemon-connection diagnostic |

Daemon identity: PID 99803 / start token 1794149494959263. Owned daemon terminated by SIGTERM with exit 0; its socket was removed and its temporary runtime directory was empty. Raw receipt, script, database, shutdown report and daemon logs are retained in the reviewer raw directory. Binary SHA-256: arcctl `113ca0014ab77954adc1b07513abaf96e29d026b582b19792e9279c9c9cb7665`; arc-daemon `8def2088a3ec30e52a888d8f893f92eb1ef7d84eed66223e5aa9069bd167e4a4`.

An additional minimal native `setpgid` experiment is retained in `native-setpgid.json`. Session leader PID/PGID/SID 5233 attempted to join live bootstrap group 5240; it received errno 1, then reaped the bootstrap with status 0. No Cargo invocation or source modification was involved.

The first isolated daemon startup failed because the reviewer-owned database directory was mode 0755 rather than the required 0700. Its `*.startup-failure` records remain retained. Changing only that owned raw directory to 0700 allowed the actual comparison above. This prerequisite failure is not a product defect or a passing managed execution case.

### Source review and verification limits

The candidate retains the target in the original caller PG, isolates the observer, and uses an anchor for CONT only. The bootstrap is reaped before target launch. Signal forwarding uses the unreaped target PID; reaping disables forwarding before PID reuse. The anchor closes inherited descriptors except its control socket and exits on control EOF. Shared lock-free continuation epoch/suspending/acknowledgment state addresses the demonstrated CONT-before-STOP transition. Reviewed signal handlers use native calls and atomics without Rust allocation or locking. No additional reproducible security, performance or accessibility defect was established in this scoped native CLI review.

The initial clone-before-guard resource path was corrected before freeze: the owned stream is moved directly into `Relay` before later fallible setup. It is not reported as an outstanding finding. Allocation/fork/FD failures and arbitrary signal interleavings were not fault-injected. The single pending-signal slot is not evidence of preserving multiple distinct signals between the final interrupted check and attachment; that remains an explicit unverified contract case, not a newly reproduced finding.

The accepted contract and prior pipeline review were read without editing. Scoped source files are all at most 500 lines; candidate `relay.rs` is 226 lines and `pipeline.py` is 382. `0001_phase0.sql` is unchanged against HEAD and has SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. Earlier independent failed pipeline review bytes stayed unchanged during this checkpoint.

Parent-reported candidate workspace tests, fmt and Clippy passed; these are separate owner evidence, not independent executions by this reviewer. The coordinator also reports the candidate managed-exec protection gate at 11 passed / 1 failed, triggered by the conservative macro-binding scanner on `relay.rs`; the proposed `-> !` lexing explanation is not independently verified and the failed gate remains failed. Historical preservation was reported 4 passed; equivalence was still running when this checkpoint was finalized. Broad coverage, full harness, task-8 QA and final lifecycle gates are outside this checkpoint's claimed result. No npm audit, bandit or Lighthouse applies to this scoped Rust/native target. No browser/GUI/Playwright, installation, download, collector, publication or external delivery occurred.

### Acceptance criteria checklist

- [x] Inspect accepted requirements and frozen source without modifying product or contracts.
- [x] Independently run exact pipeline and all managed execution checks with retained receipts.
- [x] Confirm direct 6/6, managed 5/5, signal counts, exit17 and recorded helper cleanup.
- [x] Retain current source hashes, native host, binary hashes and independent failure evidence.
- [x] Reproduce session-leader behavior against a healthy isolated daemon.
- [ ] Preserve session-leader execution and caller-session transparency.
- [ ] Establish a repair preserving session leaders, exact delivery, caller-job compatibility and the unchanged original ownership boundary.
- [ ] Complete distinct preattach-signal and arbitrary stop/CONT interleaving evidence.
- [ ] Final task-8 QA and full lifecycle acceptance (not this checkpoint).

### Files changed and disposition

Only reviewer deliverables `terminal-repair-independent-review.md` and `.json` were written under `.arc-verify/managed-exec/implementation/`; raw output and preserved candidate source copies are under the unique ignored reviewer `target/` directory. Native fixture runtime directories were owned by this probe. Product source, test definitions, contracts, previous reviews, preservation baseline/index and mission state were not edited by this reviewer.

The coordinator rejected candidate promotion and completed restoration at `2026-11-08T14:53:35.861650+00:00`. Its receipt is `target/terminal-repair-7a6b4419-388d-4ddc-a10f-dc456bb6eee5/restore.json`, SHA-256 `45c9cafa3867fecd7ac8594c6e7aeb390092585afe0f6c9351f5d2caa1834ab6`. The inspected receipt records all 175 initial bindings restored, no mismatches, removal of only the new owned relay, and no Git reset, staging or index mutation. These restoration results are owner-recorded; this reviewer did not rerun product checks after restoration. The candidate remains separately retained as `.arc-verify/managed-exec/implementation/terminal-repair-candidate.patch`, 18,399 bytes, SHA-256 `dadd2f2570ad22f86f91ebbcdb953106090af170b0964a986134df9b0972b1fd`; this reviewer verified the patch's recorded digest.

Passing candidate receipts must not be presented as a pass of the restored product. Existing `target/debug` CLI binaries still contain the candidate pending recompilation, so the binary probes above also apply only to the frozen experiment. The earlier retained failed pipeline review remains the evidence for the unresolved original pipeline defect. No fresh restored-product executable result is claimed here.

Serena tools were absent, so assigned named files were read directly. No relevant memory evidence was used. No injected OMA claim/run/session identity was supplied; no synthetic state or generic run claim was created. The JSON companion is this checkpoint's explicit handoff.
