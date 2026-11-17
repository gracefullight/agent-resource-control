# Independent agent-hook QA

Verdict: **FAIL; verification partial.** Coverage reuse fails, a test lacks a
child watchdog, and production hooks pass through: active control is incomplete.

## Inputs and provenance

Reviewed Actor HEAD/snapshot `8e84705654e9200452d72d7a94e7b9cade3f8b1f`, tree
`30ef347d790afc38a016ead82a382b7ed31ec5ce`. Its overlays are absent from the
accepted committed baseline `7f9987c1c26ff72fb010c773e3cfc4995a8daacc`, tree
`0b3e6582cba0eb228e7b6c7f2c621a11ab29bd4f`. Snapshot/worker refs were unavailable. Mission HEAD is the baseline with
staged/untracked overlays; 12 selected inputs matched, not the entire tree.
`qa.md` was staged there and `contract.md` untracked; task 8 must carry it. Only this report changed here.

SHA-256: `contract.md` = `29dbfd5fee5fa3dfae5f2d29dc787c39e60c11e3da0789f4aab006357579e216`;
`docs/requirements/traceability.md` = `4634ad621b1d4a2b88050846a35f6c478bb07eeb87d0a2c571cb29851ca10821`.

## Blocking findings

1. **HIGH — coverage reuse rejects the recorded collection. Owners: task-2
   coverage and task-5 integration; task 8 coordinates shared files.**
   `scripts/coverage/coverage-receipts.ts:49-55` rejects any ignored test. Mission-workspace receipt
   `target/av-runner/coverage/runs/2026-11-17T18-14-21-498Z-2f3c50b3-b7ee-46ce-9b49-24900995f378/receipt.json`
   has `--no-report` stdout that `parseCargoExecution` in
   `scripts/archive/harness/managed-exec-test-output.ts:10` parses as **1,836
   passed, 23 ignored, 84 harnesses**. `crates/arc-cli/tests/binary.rs:136-184`
   launches an ignored worker from its parent. Runner log
   `target/av-runner/gates/harness.2026-11-17T18-40-51-625Z.stderr.log`
   reports `COVERAGE: incomplete native tests`; exit 1, no recollection. Make
   worker execution and reuse compatible, proving required workers ran without
   broadly accepting skips; rerun final-input reuse.

2. **HIGH — unbounded direct child waits. Owner: task-5 integration; task 8
   coordinates overlap.** `crates/arc-cli/tests/binary/hook_scheduled.rs:46-76`
   reads pipes to EOF and waits without a watchdog, including in `Drop`. The
   test at `:303-326` has no enclosing watchdog; a stalled child or retained
   pipe can hang it, including during unwind. Supervise output and the direct
   child/group with a diagnostic watchdog of at least 60 seconds; bound cleanup.
   No hang was injected. Nested-helper cleanup on watchdog failure is deferred.

## Reviewed behavior

- `crates/arc-cli/src/agent_hooks/claude.rs:10-13` and `cursor.rs:25-28`
  supply no `VerifiedSurface`; `hooks.rs:235-252` requires one, so production
  never rewrites. Claude's fixture copies input fields without granting
  permission; Cursor's candidate fails its schema projection
  (`cursor/response_tests.rs:37-51`). Live approval and shell remain unverified.
- `hooks/event.rs:46-61` rejects duplicate keys and inexact numbers;
  `hooks.rs:235-252` only rewrites the two literals when executable/PATH checks
  and surface proof pass. Fixtures cover disabled/unavailable adapters,
  unsupported input, no hook-side execution and no double wrapping.
- Claude ownership/comparison: `claude/installer.rs:159-180`,
  `claude/transaction.rs:171-189`; Cursor: `cursor.rs:184-223,307-346`,
  `cursor/snapshot.rs:83-109`. Fixtures cover scope, order, idempotence,
  ambiguity, metadata and edits. No further blocker reproduced; focused runs
  are Actor-reported.
- `traffic_bun_scripts/agent_hooks.rs:309-399` specifies both fixture orders
  with the production daemon: zero second starts before release, high-water
  one, one execution each and no final permits. `mod.rs:262-320` reconciles
  SQLite, CLI and a supplied read; desktop `traffic_bun_scripts.rs:26-65`
  supplies the registered bridge. Execution is runner-owned.
- Source: `support/native_fixture.rs:75-95` retains failures;
  `scripts/coverage/hygiene.ts:8-30` counts blocks, with a sparse-file test;
  `scripts/coverage/run.ts:111-120` cleans the validated instrumented build;
  `job_meter_acceptance/calibration.rs:144-171` compares a same-run reference;
  `exec/descriptors.rs:92-170` probes SIGPIPE restoration.

## Integration and runner evidence

The three previously leaked `crates/arc-cli/default_*.profraw` files are absent
in both worktrees. `queue_tests.rs:101-146` forwards and checks `LLVM_PROFILE_FILE`;
`scripts/coverage/source.ts:237-260` rejects stray profiles outside root target,
excluding other AV workspaces. Tests cover ignored files and reuse. This is not
repository-wide proof of profile forwarding.

The later receipt reports 11 crate floors above 80% (lowest arc-cli 89.77%)
and digest `8959359416b021b1d8e420ba54ec60663d6752942d6cfa33664bfadd6a7bb64b`.
Three changed source hashes match this snapshot; reuse failed as above. A runner
workspace record reports **1,125.673 s**, above 360 s; speed
is deferred for lane acceptance, not achieved for the mission. Final-input
integration, desktop, coverage reuse, clean workspace, binary timings,
post-workspace Bun/hygiene and evidence sizes remain runner-owned. SIGINT and
killed-owner runtime checks, live vendor lifecycle, attribution, broader syntax
and Windows remain open.
`av.yaml` was readable on retry; no container/service observation policy is
configured in the supplied mission.

## Installer hardening follow-ups

Under 02:35Z, defer post-comparison settings-file, symlink and sidecar races;
different/privileged-user invocation followed by the owner; owner-denying
umask/ACL on installer-created files; nested helper process-group cleanup on
watchdog failure; and vendor matcher/schema forms the installer never writes,
except where the exact-marker rule applies.

## Test-support follow-ups

Under the 08:10Z, 11:45Z and 14:35Z rulings these historical single failures
are follow-ups, not lane rejections; lines are from those runs:

- `go_ordered_pairs`, `traffic_native_tools/witnesses.rs:138`: “native
  Bun-script synchronization timed out”; target later passed isolated.
- `interrupted_go_fixture_reaps_native_descendants`, `go/cleanup.rs:54`:
  `ParseIntError Empty`; passed isolated.
- `swift_direct_timeout_terminates_and_reaps_the_owned_tree`,
  `swift/process_tests.rs:168`: `Other` instead of `TimedOut`; passed isolated.
- `swift_helper_rejects_unexpected_release_byte_and_reaps_siblings`,
  `swift/process_release_tests.rs:111`: `kill group: Operation not permitted`;
  later isolated pass and stopped/zombie repair reported.
- `scripts/harness/coverage-reuse.test.ts:263`, “explicit invalid, missing,
  empty and stale harness receipts fail without hidden recollection”: 5,095 ms
  against default 5,000 ms; add a diagnostic watchdog.
- Earlier `swift_execution_semantics` and `registered_swift_execution_semantics`,
  `witnesses.rs:31`, timed out; synchronization repair was reported. Their
  current runner check remains required under the blocking scope ruling.

Also defer speed, Go descendant registration, legacy preparation/pointer/lease
atomicity, Rust receipt/artifact validation, Meter and notice-writer clocks,
fixture locks and probe descendants holding pipes.
`exec/launch/tests.rs:277-294` needs a watchdog; recent-fifty
(`traffic_cli/terminal_outcomes.rs:142`) still uses a real deadline.

## Review checks

Skill: `oma-qa`. Serena unavailable; scoped reads and Git diffs were used.
`bun test scripts/requirements/traceability-parser.test.ts
scripts/requirements/traceability.test.ts` passed 29/29. `bun
scripts/requirements/traceability.ts` reconciled 316 rows: Requirements
165/13/44/108, Acceptance 47/1/15/31, Catalogs 104/9/38/57
(total/Complete/Partial/Pending), zero Blocked. It checks counts, not narrative.
A read-only Bun parser probe produced finding 1's counts.
`git diff --check` baseline-to-snapshot passed excluding `av.yaml`; archive
had no diff. No native build gates, integration, desktop, workspace, coverage,
harness, SwiftPM or GUI checks were run here. Refresh after input changes.
