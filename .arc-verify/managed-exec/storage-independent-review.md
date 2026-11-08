## Review Result: PASS

Status: completed for task 6 attempt 4. No confirmed CRITICAL, HIGH, MEDIUM or LOW defects remain in the reviewed scope. This is an independent task-6 checkpoint verdict; final mission acceptance remains downstream.

### CRITICAL

None.

### HIGH

None.

### MEDIUM

None.

### LOW

None.

## Summary and scope

Reviewed the changes against immutable baseline `19d7a507044cbf83918f58cd41056905c275c8d4`, the saved contract/decision and the operator-authorized schema repair. Scope covers typed lifecycle payloads, shared native process identity, direct OsString argv classification, allowlisted transactional SQLite lifecycle storage, migration 0002 and the 12 assigned regression/guard repair paths. Daemon production integration and CLI execution remain task 7.

Used `oma-qa` and its execution protocol/checklist. Serena was unavailable; inspection used direct known-path reads and Git diffs. No product source, test, manifest, instruction, mission state or accepted preservation-review file was modified. No installation, browser, GUI, Playwright, standalone build, collector, publication or agent dispatch ran. No OMA claim/run identity was injected; no synthetic identity or second orchestration was created. The supervising framework retains cumulative deadline/run/retry accounting.

## Review evidence

Security: lifecycle requests deny unknown fields and enforce bounded payloads and static validation errors. Native identity evidence distinguishes match, missing, mismatch and indeterminate; a zombie identity can preserve child evidence without admitting a dead observer. On native macOS, immediate-exit identity capture and equal live/zombie tokens passed. Classifier input remains original OsString argv boundaries and uses existing budgets, without shell reconstruction. Storage writes use parameterized IMMEDIATE transactions and typed allowlisted metadata. No raw argv, environment, cwd, executable path, output, command digest or resource reservation is added. SQLite error details are reduced to static errors.

Correctness and performance: stable-key registration/claim/attachment and identical completion preserve durable revisions/timestamps; conflicting mutation rolls back. Terminal outcomes win reconciliation races; missing or uncertain identity never becomes an invented exit. Native probes occur outside SQLite transactions. RECENT bounds an indexed lifecycle candidate set to 51 before joining summaries; tests prove newer legacy jobs do not increase scan work. Atomic rollback, concurrent one-winner claims, reopening, outcome constraints and strict decoding passed.

Preservation: `schema-adaptations.ts` names exact baseline paths, functions, replacement text and occurrence counts. Only immutable baseline content is adapted, before the common item/span/declaration/reachability comparisons. Candidate tokens receive no number/SQL normalization or exemption. The fixed schema-1 registry retains Compatible/witness-1 behavior; current-registry cases assert schema 2 and future schema 3. Complete schema inventories, both migration records/checksums, WAL, foreign keys, integrity and rejection snapshots remain checked. Guard negatives reject wrong counts/inventories/checksums, legacy-witness changes, assertion removal/shadowing, early returns, ignores/cfg bypasses, helper redirection, snapshot removal, migration edits and coverage/harness weakening. Existing accounting/harness/evidence tooling remains unchanged from the accepted task-5 review, apart from the authorized live-guard extension and tests.

Accessibility has no changed UI surface in this scope. Dependency audit tools for JavaScript/Python/UI do not apply; the repair adds no third-party dependency. Formatting and warning-denying Clippy results were audited before finalizing the manual review.

## Verification

| Check | Independent result or audited owner result |
| --- | --- |
| Exact `cargo test --workspace` | Independently PASS: 306 tests, 30 harness summaries, zero failed/ignored/filtered |
| `cargo test -p arc-storage --no-fail-fast` | Independently PASS: 48 tests, 5 harness summaries, zero failed/ignored/filtered |
| `bun test scripts/harness/managed-exec.test.ts` | Independently PASS: 12 tests, 234 assertions, zero failures |
| `cargo fmt --all -- --check` | Owner PASS, final-source receipt/output audited |
| `cargo clippy --workspace --all-targets -- -D warnings` | Owner PASS, final-source receipt/output audited |
| Coverage-verification inventory and five noncollecting suites | Owner PASS, receipt/output and detailed inventory audited; no collection claimed |
| Selected evidence validator/integration cases | Owner PASS: 9 tests, 95 assertions; final acceptance case deliberately remains downstream |
| Actual exact Cargo positive execution | Independently validated promoted receipt: 1 executed/passed test; producer is backend invocation with the existing technical-director label |
| Source, metadata, migration, evidence and output audits | PASS: 153 owner/independent code inputs; 160-input managed manifest; 24 referenced artifact hashes; 9 owner and 3 independent final command receipts |

Independent logs and receipts are under `.arc-verify/managed-exec/runs/qa-task6-attempt4-final-4e51ade8742a46b5a119511a6b9cecc5/`. Its receipts record actual argv, cwd, native macOS host, start/end times, exits, combined output hashes, source hashes and a 3,600,000 ms per-command timeout. Sources remained unchanged during and after all three checks. Cargo counts were cross-checked against individual case lines, summaries and the existing execution parser.

The backend source-manifest file SHA-256 is `336a50efee2c86e0f98d890e36fe7e205d7fcf43987a148ec9c44be6c6cfe752`; the broader managed-input fingerprint is `220f0484ad88facde4b65229b450cab642d67b4b23f5ddf82e1991186880c691`. The machine-readable review includes the full managed manifest, reviewed reports/receipts and all relevant output bindings.

Migration `0001_phase0.sql` equals the immutable baseline bytes, SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. Maximum Rust/SQL source length is 474 lines. The managed-exec evidence inventory is 29 nonignored tracked-plus-untracked files including these two review files; native logs remain ignored.

Verified the explicit positive-execution promotion occurred after execution, outside the test: generated and promoted bytes both hash to `3e8c63c3de5c6883d01cedbbfaf3a9a0710ca2c7781231d4bd36b836181ef7f2`. The previous promotion and preservation report are archived; the current preservation report retains its complete previous bytes as a prefix. Accepted `preservation-independent-review.md` and `.json` retain their original hashes and bytes.

The first independent guard run failed because two reference snippets omitted rustfmt-added trailing commas; its source changed during the ongoing guard suite and that run is retained as historical evidence only. The backend repaired the exact recipe strings, without changing Rust or normalizing candidates. The fresh final-source guard passed. Earlier backend failed executions remain retained and are not relabeled as successes.

## Acceptance criteria

- [x] Typed protocol-1 requests/results and validation reviewed and exercised.
- [x] Native macOS identity and direct argv behavior exercised.
- [x] Allowlisted lifecycle storage, monotonic/idempotent transitions, rollback/reopen, reconciliation and bounded reads reviewed and exercised.
- [x] Immutable 0001, additive 0002, schema-2 expectations and fixed schema-1/witness-1 regression preserved.
- [x] Exact immutable-reference adaptations and all guard positive/negative tests pass.
- [x] Exact independent workspace and focused storage commands pass without skipped cases.
- [x] Final-source hashes, output hashes, host/times/exits/counts, promotion and accepted-review preservation verified.
- [x] Task-6 delivery boundary and downstream requirements accurately separated.

## Downstream acceptance

Task 7 must integrate bounded daemon worker/dispatch/reconciliation and CLI execution/status. Full lifecycle, terminal/PTY, retry/one-launch, stream/signal/exit behavior and daemon-loss survival still require native macOS acceptance. Full coverage collection, each crate's 80% production-line floor, separate clean-checkout/coverage-CLI/harness/live-evidence checks, the full no-skip harness and final independent QA remain required. Desktop, native Windows and Phase 1 scheduling remain outstanding. This review sets none of those final acceptance flags.

## Files changed

Only `storage-independent-review.md`, `storage-independent-review.json` and unique ignored review logs/audits under `runs/`.
