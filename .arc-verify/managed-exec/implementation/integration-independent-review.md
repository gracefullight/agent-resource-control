## Review Result: PASS

Status: completed for task-7 integration review. Verification completeness: completed within the assigned scope. Skill: `oma-qa`. No outstanding CRITICAL, HIGH, MEDIUM or LOW finding. This checkpoint does not accept task-8 final QA or task-9 Chief delivery.

The current implementation and executed gates match managed source SHA-256 `e828477b00cd7b02b40fa0deea1425321e8b0b5b63b081dfaa7b31b48485ca05`: 177 complete file/mode bindings. Native execution host is Darwin 27.2.0 arm64, Eunkwangui-MacBookPro.local. The common native coverage identity is separately `d307816fd004d7a0470c8137c329ebbefaba552d07e508e89325c9281c3ea915`.

### CRITICAL

None.

### HIGH

None.

### MEDIUM

None.

### LOW

None outstanding. Resolved during review: `docs/managed-exec.md:14` described the superseded foreground-group transfer. The backend owner replaced it with the delivered behavior: preserve the caller's process group with a controlling terminal, avoid foreground transfer and duplicate terminal forwarding, use a private group without a controlling terminal, and forward wrapper-directed nonterminal signals. The current paragraph agrees with `exec.rs`, `exec/terminal.rs` and `exec/signals.rs`. Corrected documentation SHA-256 is `76a634416a406038e605ce9aa09f4f672a829b6a724444a47482c2c35b05e2f5`; it is bound separately from managed source inputs. No source fix is requested.

### Implementation inspection

The CLI has one direct `Command::spawn` site outside metadata retries. Original `OsString` argument boundaries reach that command; shell metacharacters are not reconstructed or interpreted. Classification failure yields UNKNOWN while execution retains native bytes. The observer and child have separate boot/PID/start-token identities. Attach retains an unreaped fast-exit identity without asserting that a zombie is running. The wrapper waits for the same child after daemon loss and preserves the observed outcome.

Lifecycle payloads use strict typed validation, static errors and an 8,192-byte bound within protocol 1. Same-user authentication precedes handler creation and finite admission. Raw payload data is cleared before worker await. Lifecycle and diagnostic operations share the existing bounded SQLite worker and shutdown deadline; no long-lived child is owned by a daemon handler. SQL uses bound parameters and immediate transactions. Conflicting owner/outcome changes reject atomically; identical retries preserve durable identity and terminal observations. Revision-bound status reconciliation cannot launch work or invent an exit. Recent status uses a bounded indexed scan rather than per-row native probes. Remote/UNKNOWN work obtains no resource reservation.

The scope has no web interface; browser accessibility and browser performance checks are inapplicable. Native terminal controls and output behavior are covered by the delivered tests. No npm/Python web audit applies to this Rust integration; current workspace Clippy and the full native harness supply the applicable automated checks. No separate dependency vulnerability scanner was run or claimed.

### Independent native execution

All six reviewer-owned commands ran serially after the coordinator reported collection complete. Every command received 3,600,000 ms. Exact argv, cwd, UTC times, PID, host, source bindings, raw stdout/stderr digests and canonical parsed libtest names/counts are in the JSON companion and ignored raw directory.

| Command selector | Result |
| --- | --- |
| `cargo test -p arc-daemon --test managed_exec retries:: -- --nocapture` | 2 passed |
| `cargo test -p arc-daemon --test managed_exec protocol:: -- --nocapture` | 4 passed |
| `cargo test -p arc-daemon --test managed_exec cli::daemon_shutdown_and_crash_preserve_child_and_restart_reconciles_without_replay -- --exact --nocapture` | 1 passed |
| `cargo test -p arc-daemon --test managed_exec pipeline::native_foreground_pipeline_input_signals_stop_resume_and_exit17 -- --exact --nocapture` | 1 passed; 13 native scenarios |
| `cargo test -p arc-daemon --test managed_exec pipeline::native_inherited_ignored_sigchld_preserves_target_state_and_completion -- --exact --nocapture` | 1 passed; four disposition/mask combinations |
| `cargo test -p arc-cli --bin arcctl exec::signals::tests -- --nocapture` | 6 passed, including the receiver helper |

Total: 15 executed tests, 0 failed, 0 ignored. The pipeline's 13 JSON observations all report pass, outcomes [17,0], controller exit 0, restored terminal/foreground cleanup and no remaining recorded native identities. Default/ignored and blocked/unblocked CHLD cases each report target state, one exit 17, persisted completion 17 and verified cleanup. `parseCargoExecution`, `validateCargoExecution` and `validateCommandReceipt` independently validated the normalized reviewer commands and actual selectors.

### Gate and retained-evidence audit

Fifteen current implementation receipts were validated against the current source and native host, including formatting, workspace Clippy, exact unfiltered workspace, managed preservation, equivalence, coverage inventory, clean checkout, coverage CLI and actual nested root, full harness and actual nested root, final live evidence, privacy, positive execution and isolated saturation. Their raw output hashes match. The exact backend workspace run contains 339 passed identities across 32 harnesses, 0 failed and 0 ignored; this is audited backend evidence, not a reviewer-owned unfiltered run. Managed preservation has 12 tests; equivalence has 2. The nested collectors retain their actual inner argv, cwd, PID, times, source binding and output bytes; neither used a skip. Final live evidence starts after the last collector ends.

Native LCOV and LLVM JSON digests match. Recomputed accounting agrees with all six reported production-line floors: core 247/256, detector 609/610, storage 653/674, IPC 387/393, daemon 1628/1746, CLI 661/766, each above the unchanged 80% floor. All 1,037 final native profile sizes/digests, merge membership and arcctl/daemon child PID profile proofs were verified. The retained isolated checkout has the same 177 managed input bytes/modes, no precollection run artifacts, matching native raw report digests and independently recomputed passing floors. Its daemon coverage is 1629/1746; the final root is 1628/1746. These are separate actual runs, both above the floor.

The positive-execution artifact is byte-identical to the explicitly promoted actual test output. Its command is a real named Cargo execution with verified raw identities/digests. The test-emitted `technical-director` label remains intact; the coordinator records backend as actual executor. It is implementation validator evidence and provides no independent final approval.

The six protected migration/accounting/config/source/gate artifacts match HEAD. Accepted repair5 implementation and independent-review pairs remain byte-identical to the retained pre-attempt copies. Maximum Rust/SQL/tooling source length is 474 lines. Eligible managed evidence is 105 files before this pair and 107 afterward, below 150; ignored raw files are separate. The final maintained backend report was read and bound at SHA-256 `fd52317abb9cceffe66858569f12785de3747d82d4e74be2b973b9c1ef2eb336`. No gate receipt or previous review was edited by this reviewer.

### Failed attempts and limits

The first current-source workspace command exited 101. Its stdout records macOS EINVAL from `set_read_timeout` at `crates/arc-daemon/tests/classification/saturation.rs:41` during shutdown load, followed by the scoped-thread panic. The exact fixture bytes equal HEAD. Its isolated rerun and exact workspace rerun pass without source/test changes, as do later native collections. The original failure, UTC times, output digests and exit remain retained. The observed intermittency is a residual test reliability risk; it is not a reproduced managed-exec defect or a successful run.

The first supplemental privacy command exited 1 at an old readiness assertion. Its output omitted startup stderr, so its cause remains unproven. The strengthened assertion and subsequent serial execution pass with live pinned WAL, row and diagnostic sentinel checks, one lifecycle row, zero permits and recorded identity cleanup. The failed original remains separate.

Reviewer tooling initially supplied absolute output paths to the Cargo receipt validator, which requires permitted workspace-relative paths. A separate normalized audit preserved the command/output bytes and passed. A supplemental audit initially passed the macOS temporary-root symlink spelling into `sourceInventory`; the corrected audit uses its canonical real path and passes. These were review-tool path errors, not product failures; original audit scripts and supplemental failed output remain in the raw directory. No product tests were rerun for those corrections.

Task-8 still requires separate independent unfiltered workspace execution and final evidence binding. Task-9 still owns final summary, receipt index, delivery and Chief acceptance. Desktop, native Windows and Phase 1 scheduling remain outside this checkpoint. No injected OMA run/claim/session identity existed; none was invented. The coordinator receives the result claim in final stdout. No absolute cumulative deadline or remaining allowance was exposed; no attempt or deadline was reset. No AV operation, browser/GUI, Playwright, install/download, separate build, source/test edit, mission-state mutation or outreach occurred. Serena was unavailable; inspection used supplied-path reads and scoped Git inventories. The quick memory lookup found no relevant evidence, and no memory-derived fact was used.

### Acceptance criteria checklist

- [x] Read lifecycle contracts and accepted storage/terminal repair without modifying implementation.
- [x] Verify all 177 current input bytes and modes, Git binding, native host and corrected documentation.
- [x] Independently run retries, protocol, daemon survival, pipeline and inherited-signal cases with exact identities/counts.
- [x] Verify current gates, nested collector argv/times/output hashes and postcollection live evidence.
- [x] Recompute six crate floors from native LCOV/LLVM reports and verify profile/child PID evidence.
- [x] Verify clean-checkout source identity, raw native report hashes and recomputed crate floors.
- [x] Verify protected sources, original migration, frozen repair artifacts, source-length and evidence-count bounds.
- [x] Preserve and evaluate original workspace and supplemental privacy failures separately.
- [x] Report task-7 verdict separately from task-8 final QA and task-9 acceptance.

Reviewer-owned retained files changed: `integration-independent-review.md` and `.json` only. Unique ignored raw evidence: `target/managed-exec/reviewer-integration-95236b8f-e483-4edf-94f5-2e589ae687e9`.
