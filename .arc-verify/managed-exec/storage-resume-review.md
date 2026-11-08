## Review Result: FAIL

Status: partial for the mandatory task-6 checkpoint. Independent task-6 implementation verification is complete, with no confirmed defect in the assigned core/detector/IPC/storage/schema-guard changes. The required workspace check fails in task-7 pipeline execution. This report does not approve that failure or declare task or mission acceptance.

Reviewer: `/root/reviewer`; skill: `oma-qa`; session: `01a1bb90-9cda-7b22-98c0-ad418f42dc94`. No framework claim path was injected. The coordinator retains mission/run/deadline accounting; this review creates no replacement mission run.

### CRITICAL

None.

### HIGH

- `crates/arc-cli/src/exec.rs:98` — Existing task-7 code gives the managed child a private process group. `exec/terminal.rs:59` repeats that assignment and line 67 transfers the terminal to it. This splits an existing foreground producer/peer pipeline: the managed fixture loses peer input, terminal INT misses a peer, and the complete pipeline does not stop/resume. Both current parent workspace commands reproduce `pipeline::native_foreground_pipeline_input_signals_stop_resume_and_exit17`, failing at `crates/arc-daemon/tests/managed_exec/pipeline.rs:23` with `native pipeline comparison failed`; direct comparison modes pass. This is outside the assigned task-6 repair. Task 7 must preserve the inherited pipeline group, update terminal resume and signal forwarding together, and rerun the unchanged native comparison. The group-handling correction starts with the following code; it is a remediation sketch, not an implemented or verified complete signal-handling fix:

  ```rust
  let caller_group = unsafe { libc::getpgrp() };
  command.args(&argv[1..]).process_group(caller_group);
  // Terminal::configure must retain this group instead of process_group(0).
  // Terminal::resume must restore the caller group, not the child's PID.
  set_foreground(self.file.as_raw_fd(), self.original)?;
  ```

  The owner must also handle terminal/group signal delivery without forwarding a duplicate to the child. No task-7 source was edited or its acceptance weakened by this review.

### MEDIUM

None in the assigned task-6 scope.

### LOW

None.

### Reviewed behavior

Lifecycle requests use typed protocol-1 payloads, reject unknown fields, bound payloads to 8192 bytes and return static validation/storage errors. Native identity distinguishes match, missing, mismatch and indeterminate; child zombie identity does not admit a dead observer. Direct argv classification keeps OsString boundaries and input/work/token budgets; unsupported native bytes remain UNKNOWN. No third-party dependency was added by this repair. JavaScript/Python package audits and browser accessibility tools do not apply to this Rust storage review; there is no changed UI surface.

Storage uses parameterized IMMEDIATE transactions and allowlisted metadata. Register/claim/attach/completion retries preserve original revisions/times; conflicting mutations roll back. Eight concurrent contenders produce one registration, claim and completion winner. Terminal outcomes win stale observations; missing or uncertain identity does not invent an exit. Native probes run outside SQLite transactions. RECENT materializes at most 51 indexed lifecycle candidates before joining jobs, returns at most 50 summaries and makes no per-row native probes. Existing tests verify scan work is unaffected by newer legacy rows and check privacy sentinels, rollback, reopen, fast exits and strict outcome decoding.

Migration 0001 is 3917 bytes and equals both the fixed baseline `19d7a507044cbf83918f58cd41056905c275c8d4` and the current index. SHA-256: `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. Migration 0002 appends through the existing checksummed atomic registry. Genuine schema-1/witness-0 assessment is read-only; current-registry upgrade preserves application rows, original schema objects/history and stamps witness 2. A separate fixed schema-1 registry remains Compatible and stamps witness 1 without upgrading.

The schema adaptation recipes name fixed paths/functions/exact old text/occurrence counts and operate on immutable baseline text only. Candidate code receives no normalization or exemption. Regression items, assertion/helper bodies, attributes, module reachability, macro bindings and test admission remain checked. The positive authorized-feature fixture passes; negatives reject incorrect schema counts/inventories/checksums/witnesses, deleted rejection snapshots, early returns, ignores/cfg disabling, assertion shadowing, migration byte/index/mode/symlink changes, and coverage/harness bypasses. Historical baselines remain fixed. All checked source files stay within 500 lines; the maximum is 474.

### Current execution evidence

| Command | Result |
| --- | --- |
| Independent `cargo test -p arc-core -p arc-detector -p arc-ipc -p arc-storage --no-fail-fast` | PASS: 114 tests, 18 summaries, zero failed/ignored/filtered; core 13, detector 16, IPC 37, storage 48 |
| Independent `bun test scripts/harness/managed-exec.test.ts` | PASS: 12 tests, 234 assertions |
| Parent `cargo test -p arc-storage --no-fail-fast` | PASS: 48 tests |
| Parent `cargo test --workspace --no-fail-fast` | FAIL, exit 101: 329 passed, 1 failed, zero ignored |
| Parent exact `cargo test --workspace` | FAIL, exit 101: 136 passed, 1 failed, zero ignored; stops at the failing integration target |
| Parent formatting and warning-denying workspace Clippy | PASS |
| Parent coverage-verification inventory and five noncollecting suites | PASS; nested suite counts 12/14/3/4/1; all actual output hashes verified |
| Parent affected preservation suites | PASS: 4 tests, 23 assertions |
| Parent scoped whitespace check | PASS |

Independent commands ran sequentially after the parent's checks, on Darwin 27.2.0 arm64. Both used 3,600,000 ms wrappers, with actual argv/cwd/native host/timestamps/exits/output hashes and before/after source manifests retained in the JSON receipts. Individual Cargo case lines and summary totals agree. All 168 source inputs stayed unchanged across both checks and equal the parent's current manifest, SHA-256 `3e3edae6be67c9dd627035de78cc62574b7ac624bc956b5648780d96c1eaae95`.

Independent receipts are under `.arc-verify/managed-exec/runs/qa-task6-storage-resume-e8e2cc92-d654-46a2-abae-811e0fa19e60/`. The binding audit verifies all nine parent output hashes, all five nested coverage stdout/stderr pairs, source/report/archive hashes and the four frozen independent-review artifacts. The refreshed storage report accurately marks task/workspace/final lifecycle acceptance false. Previous storage report/JSON bytes remain in the parent's unique archive with original hashes/times; no old pass was relabeled as current.

### Acceptance criteria

- [x] Protocol-1 validation, native identity and original native argv behavior reviewed and exercised.
- [x] Stable-key concurrency/idempotence/conflicts, rollback/reopen, identity uncertainty, terminal precedence and privacy sentinels pass.
- [x] Immutable 0001, additive migration, read-only schema-1 assessment/upgrade and fixed schema-1 witness remain protected.
- [x] Exact schema adaptations and positive/negative preservation guards pass without exemptions.
- [x] Source bounds and current parent/independent output/source bindings verified.
- [x] Previous storage and preservation independent-review bytes preserved.
- [ ] Mandatory workspace checkpoint passes: blocked by the reproduced task-7 pipeline defect above.
- [ ] Final collecting coverage, every crate's 80% floor, full no-skip harness, final live evidence and native lifecycle acceptance: downstream, not established here.

Desktop, native Windows and Phase 1 scheduling remain outstanding. This is a partial checkpoint with a concrete failing gate, not final acceptance.

### Files changed

Only new reviewer-owned `storage-resume-review.md`, `storage-resume-review.json`, this run's ignored diagnostics/receipts and the matching `.agents/results/` report. No product code, tests, manifests, migrations, mission state, index staging or `.agents/` definitions were changed. Serena tools were unavailable; review used supplied paths and Git diffs. No browser, Playwright, installation, standalone build or coverage collector ran.
