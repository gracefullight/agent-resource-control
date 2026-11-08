# Independent QA: managed execution and evidence identity

## Review Result: PASS

Status: completed. Native macOS product verdict: PASS. Review completeness: completed for this assignment. No confirmed CRITICAL, HIGH, MEDIUM or LOW defects remain. Chief final mission acceptance remains pending.

This review applies oma-qa and binds 179 file/content/mode inputs to `c1728da557260d92269fa306ed5cb79392530d2865917b04244256f4cb76676f`, against accepted HEAD `19d7a507044cbf83918f58cd41056905c275c8d4`. Native host is Darwin 27.2.0, arm64, Eunkwangui-MacBookPro.local. The separately verified coverage input identity is `6f0fbcb95fb05bfc5133d5255ac9cfa1c9afb79a80c47b8cc8060392cdcb05d8`. Serena was unavailable; known-file reads, Git diffs and retained executable output supplied the review evidence. No browser, GUI, Playwright, installation, publication or new agent dispatch occurred.

## Independent command evidence

Fresh command argv, cwd, PID, native host, times, source identity, output paths/digests and 3,600,000 ms allowances are retained in qa/commands.json. The 13 task9-recovery6 commands all exited 0, with unchanged product inputs. Actual Cargo output was parsed and matched to execution counts and named tests; help, list, no-run, skip-all and unmatched-selector outputs cannot count as execution.

| Command | Actual result |
| --- | --- |
| `cargo test --workspace` | 342 passed, 0 failed, 0 ignored; 32 harnesses; unfiltered |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed |
| `cargo test -p arc-daemon --test managed_exec -- --nocapture` | 22 passed, 0 ignored |
| Native identity fault fixture via offline Cargo | 2 passed, 0 ignored; actual current arc-core |
| Independent native Python privacy/comparison probe | Passed |
| Original by-ID signal/core contract regression | 1 passed |
| `bun test scripts/harness/managed-exec.test.ts` | 12 passed; 234 assertions |
| `bun test scripts/harness/managed-exec-immutability.test.ts` | 1 passed; 13 assertions |
| `bun test scripts/harness/managed-exec-coverage-identity.test.ts` | 4 passed; 66 assertions |
| Saved original validator reproduction | Original failure reproduced; corrected validator passed |
| `bun test scripts/coverage/evidence.test.ts` | Passed after all Backend collectors |
| Ignored independent dual-run audit runner | Passed; exact runner path/digest in qa/audit.json |

## Reproduced and corrected gate defects

The original validator compared the latest collection's crate counts with the reviewed final-summary collection. I independently loaded the saved original module with only its imports resolved to current paths. Both validators accepted the same 8/10 fixture. Advancing latest to a separately passing 9/10 fixture made the original throw `native crate accounting differs`; the corrected validator passed. The saved actual failure also contains 9 passing tests and 1 failure at that comparison. Actual retained runs independently recompute successfully but differ in arc-cli coverage:the reviewed run has 674/774 and the later run 677/774. Both retain the same current coverage input identity. The reproduction source, original-module digest and results are preserved in this review's ignored run directory.

An intermediate correction allowed substitution of another valid collection without matching QA. Backend added whole `qa.coverage === final-summary.coverage` equality. The independent 4-test regression run verifies that substituting a fully valid passing collection fails, alongside count/file/inventory/source/raw/profile/summary/run/path mutations, stale recorded or latest inputs, and a genuine 7/10 below-floor latest collection. Same-run exact equality remains mandatory. The effective correction is:

```ts
requireValue(equal(recomputed.crates, report.crates) &&
  equal(recomputed.files, report.files), "native crate/file accounting differs");
requireValue(equal(recorded.crates, report.crates), "recorded measurements differ");
nativeCollection(root, latest.runId);
requireValue(equal(qa.coverage, recorded), "coverage not bound to independent review");
```

These are resolved historical defects, not current findings. This QA authored no product or tooling repair.

## Product behavior and requirement coverage

qa.json preserves the full 27-row requirement matrix. Every named Cargo test in it matches this fresh 342-test workspace output. Protocol 1 strict typed start/complete/status requests reject unknown and secret-bearing fields. Storage uses allowlisted metadata, parameterized SQL and immediate transactions for atomic summary/detail updates, idempotent retries, conflict rollback and terminal completion. The CLI has one native spawn site outside bounded metadata retries; exact argv/cwd/inherited environment/descriptors/streams and exit 17 match direct execution. Spawn failure, immediate exit, daemon graceful shutdown/crash survival, restart without replay, PID/start-token/boot matching and terminal-outcome reconciliation races passed.

Fresh focused stderr contains 15 native pipeline scenarios:seven direct controls, six ordinary managed cases and two shared-job continuation cases. Every result has `[17, 0]`, preserved terminal behavior and all recorded identities absent after cleanup. Producer and peer retain terminal input; eligible terminal/group INT reaches each once; wrapper-only TERM reaches the producer once without affecting its peer. Whole-job stop/background/foreground continuation, default/ignored CONT, target-only stop, and inherited ignored/blocked CHLD cases passed. Native fault interposition exercised EPERM and short reads as Indeterminate, ESRCH as Missing, start/boot mismatch as Mismatch, and matching identity as VerifiedMatch; native call-count assertions prove injection ran.

The fresh independent probe compares empty, space, leading-dash, literal metacharacter, Unicode and non-UTF-8 argv, full inherited environment, cwd, open descriptors, binary stdin, both output streams and exit 17. A pinned reader keeps the live WAL; rows, raw DB, shm, WAL and daemon diagnostics contain no generated argv/environment sentinels. Permits remain zero. Actual SIGTERM yields parent-visible signal 15, persisted core=false and by-ID status displaying both. Duplicate completion preserves the entire snapshot; changed signal/core returns JOB_CONFLICT. The owned daemon exits 0. Existing diagnostics, protocol compatibility, bounded queues/cancellation/shutdown and unchanged migration 0001 protections remain covered.

## Coverage, receipts and preservation

All 12 required Backend receipts and raw outputs are current and rehashed. Their exact argv identities include formatting, Clippy, workspace tests, root coverage, and full `./scripts/harness/validate.sh` without skips. Nested root receipts match command/PID/cwd/output and unchanged before/after coverage source bindings. All 9 coverage suites are accounted for:five noncollecting suites plus separately required clean-checkout, coverage-cli, harness and evidence suites. The isolated checkout starts without target, prior runs or latest pointer; its current source identity, real collection and live check pass. The implementation repair manifest's 15 archived originals were rehashed. Four nonrequired historical receipts(cli-prerequisite, privacy, positive-execution-test, recent-green) are retained and explicitly excluded from current acceptance; fresh independent execution covers their product behavior.

The reviewed collection remains exactly `2026-11-08T19-15-44-425Z-27759437-90c6-4b49-bd86-e6e011e7f13c/summary.json`, SHA256 `19281d0cebf86cdc88f1398650aee55113932f1bfd96aa13bb336822a54318ac`. qa.coverage is unchanged from that original review. Its latest-pointer hash records the prior observation. The audit separately validates current latest `2026-11-08T19-59-10-665Z-b577e54e-0324-46d1-b979-7da9b2f3139b` and stores its pointer/summary digests under liveCoverageObservation. No cross-run count equality is required and no live pointer/report is rewritten by QA.

Each collection was independently recomputed from unmodified LCOV plus LLVM JSON with exact per-file and per-crate equality, 45 production-file accounting entries, complete source/input inventories, and 1,052 profile hashes. Each manifest has the complete exact merge set and actual CLI/daemon child PID/profile proof. All six crates exceed 80% in both collections. The table reports only the pinned reviewed run:

| Crate | Covered / production lines | Coverage |
| --- | --- | --- |
| arc-core | 247 / 256 | 96.48% |
| arc-detector | 609 / 610 | 99.84% |
| arc-storage | 653 / 674 | 96.88% |
| arc-ipc | 387 / 393 | 98.47% |
| arc-daemon | 1628 / 1746 | 93.24% |
| arc-cli | 674 / 774 | 87.08% |

The accounting/config/source/gate files and accounting contract retain accepted baseline bytes. `0001_phase0.sql` is byte-identical; 0002 is additive. Source maximum is 474 lines. Only the 4 assigned Backend harness files differ between the previous 177-input map and current 179-input map; 175 earlier inputs remain byte/mode-identical. Historical fixed-revision assertions, live schema2 adaptations, assertion reachability, source bounds and negative protection cases all passed.

The evidence immutability test runs the genuine unfiltered evidence suite in an isolated checkout. Its retained bytes, path membership, index and review bindings are identical before/after, while actual Cargo execution occurs. Its final downstream acceptance check remains intentionally unavailable in that checkout; the outer test requires the specific single failure and still verifies preserved bytes. A staged receipt edit is rejected. This is immutability evidence, not final mission acceptance.

## Ownership, acceptance checklist and handoff

Changed only existing qa.json, qa.md, qa/audit.json and qa/commands.json, plus ignored QA diagnostics and the supplemental .agents/results report. Prior QA artifact bytes and command receipts were archived with digests before refresh. Retained qa/audit.ts remains byte-identical and is explicitly historical; the fresh ignored audit runner path/digest is bound in qa/audit.json. No product/test/manifest/tooling, mission-state or .agents definitions changed. Chief explicitly requested staging of these 4 QA artifacts. There are 142 staged managed-exec paths plus 2 preexisting pending candidate patches, 144 total, below 150; none were deleted to alter the count.

- [x] Full 27-row immutable requirement mapping retained and matched to fresh execution.
- [x] Independent native unfiltered workspace, lifecycle 22 and fault 2 checks passed.
- [x] 12 current required Backend receipts, nested full harness/root collection and complete coverage inventory audited.
- [x] Recorded and current collections independently validated; reviewed summary digest/run preserved.
- [x] Old cross-run failure reproduced and correction/tamper regressions passed.
- [x] Evidence bytes/index immutability, 0001 preservation and source bounds passed.
- [x] QA ownership and product/final-acceptance distinction preserved.

Chief owns final-summary/receipt-index/final.md rebinding to these exact frozen QA artifacts and 12 reviewed receipt refs, staging any Chief-owned artifacts, and genuine unfiltered final verification/replay. This QA does not certify the prior final-summary. Native macOS lifecycle implementation is verified. Desktop integration, native Windows and Phase1 scheduling remain outstanding. Commercial effects are unmeasured. No cancellation or absolute deadline was injected; the inherited framework budget and recovery 15/16 remain authoritative, with no reset or new begin/run identity claimed.
