# Task 4 technical verification — attempt 2

Status: blocked. The unchanged final acceptance schedule and integrated native matrix remain uncollected. `implementation/gates.json` records `passed: false`; this report does not claim implementation acceptance.

Actual Actor: technical-director. Skill used: oma-architecture, for the collector ownership/interface assessment. Only assigned evidence/report paths changed. Product code, tests, manifests, lockfiles, validators, saved instructions and mission state were preserved.

## Fresh source and prerequisite evidence

Rechecked at 2026-11-08T23:00 UTC after reading both operator notes and the task-4 repair findings. Supported shell checks still fail in the integrated collection checkout. No historical permission failure was substituted for a current attempt.

| Current command in integrated checkout | Exit | Observation |
| --- | ---: | --- |
| `git add -- .arc-verify/exec-fail-open/technical-verification.md` | 128 | Cannot create `.git/index.lock`: Operation not permitted. Index unchanged. |
| `bun scripts/harness/exec-fail-open-receipts.ts task4-attempt2-integrated-fmt cargo fmt --all -- --check` | 1 | EPERM creating `target/exec-fail-open/commands/task4-attempt2-integrated-fmt-…`; fails before `spawn`, so Cargo did not run. |
| Maintained `context(process.cwd())` read through Bun | 0 | Current integrated source/configuration fingerprint refreshed; no acceptance execution. |

Exact argv, cwd, UTC timestamps, elapsed measurements and stdout/stderr for prerequisite commands are retained in `implementation/task4-attempt2-preflight.json`. The failed wrapper lasted 235.066 ms; that is a prerequisite failure duration, not command-invocation latency. No GUI, alternate Git index, redirected native output location or privileged retry was used.

Accepted committed baseline is `00773c69a8e5e7fd417fd5cc78d03e222364f609`, tree `b98f996caa04fe13eeeeab1072c3c528f6d18c65`. The integrated checkout remains at that HEAD with task-3 source edits in its worktree and an empty staged diff. New execution files and evidence are untracked there. Its baseline-diff digest is `c24d89c44843e61a76edb25c08908187edd58c983d1d202f7b9219e0ad100d53`.

This attempt's isolated Actor HEAD is `c72beda4a3e04c69919fa7725cda4786fa77fd6e`, tree `5e937df1445035e27f7d2011e2b16c36d09911c8`. It includes the initial source overlay. This is not the accepted baseline or proof of integrated index tracking. `implementation/task4-attempt2-source.json` separately records the actual Git observations for both checkouts.

Both current checkouts have identical 212-file contract input bytes and source fingerprint `3d51ee23c34f602cf6bc5bb4ca6faf7d626aa7f5bcfc7347210e338dd3a0df17`. Only `claims-review.md` differs from attempt 1's manifest: its current SHA-256 is `1e5acbb46ab202e963c0560e54c9ce7b265d7c85755b653d651c48d9f066d8b3`. Old command/native receipt hashes remain unchanged and scoped to their original snapshot; refreshing source observations does not refresh those executions.

## Collector interface conflict and Chief handoff

The saved assignment names technical-director. `scripts/harness/exec-fail-open-gates.ts:21` requires the successful gate Actor to be `backend`; `exec-fail-open-inputs.ts:72-73` requires its actual Actor and `oma-backend` skill. `exec-fail-open-receipts.ts:63-80` and the assembler also default to or emit backend identity. This Actor cannot truthfully use those labels for its own collection. No validator, target or receipt identity was changed to produce a pass.

Chief recovery request: arrange supported write access to the integrated checkout's real index and native output directories, and coordinate collection by the actual backend owner under the existing interface. That preserves the fixed contract and costs less than repeating native/coverage runs in an isolated checkout whose absolute paths cannot serve integrated acceptance. Editing validators or relabeling this Actor's execution would breach the assignment and receipt contract. No new implementation choice or operator approval is requested by this report.

After those prerequisites are resolved, freeze and stage the intended integrated inputs, then run all eleven prerequisite commands in the contract's serial schedule, assemble their genuine predecessor receipts, and obtain independent oma-qa. Source changes require recollection of affected receipts. Do not reuse attempt-1 native absolute paths as integrated acceptance.

## Required schedule: still pending

No final command below ran in the integrated checkout during attempt 2. There is no successful no-skip acceptance cycle or current integrated timing, historical, preservation or coverage receipt.

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace`
4. `bun run test:coverage`
5. `bun test scripts/harness/managed-exec.test.ts scripts/harness/coverage-verification.test.ts scripts/harness/equivalence.test.ts scripts/harness/exec-fail-open-preservation.test.ts`
6. `bun test scripts/coverage/clean-checkout.test.ts`
7. `bun test scripts/coverage/coverage-cli.test.ts`
8. `bun test scripts/coverage/harness.test.ts`
9. `./scripts/harness/validate.sh`
10. `bun test scripts/coverage/evidence.test.ts` after every collector above
11. `bun test scripts/harness/exec-fail-open-docs.test.ts`

The two terminal validator commands remain downstream, unchanged. All nine coverage tests, historical fixed-fixture assertions, accounting rejection cases and six independent 80% floors remain required. No LCOV, LLVM export or profile bytes were collected or recomputed here, and no average or stale summary is used. The complete current native outage/compatibility/exclusion/signal/pipeline/recovery matrix, including committed-but-unacknowledged requests and the two-second measurement, remains unverified. Prior snapshot results are retained only as historical observations.

## Resources and bounded checks

Current host probes confirm Darwin 27.2.0 arm64, 8 CPUs, 16 GiB RAM and 43 GiB available storage; Bun 1.4.4, Cargo/Rust 1.95.0, LLVM 22.1.2 and cargo-llvm-cov 0.6.21 are installed. The Rust LLVM component is listed installed. No downloads, installs or dependency edits occurred. Installed tools do not establish offline collector readiness or a coverage pass.

The saved mission limits remain 86,400 seconds, concurrency 3, maxRuns 200, maxRetries 3, retry delay 1,000 ms and 3,600,000 ms per check. Actual probes ran serially. An absolute mission start/deadline was not supplied or present in relevant environment keys; no replacement deadline was invented. Actual shell CPU, address-space, process and file limits are in the fresh preflight. No long-running collector was launched and no owned child worker remains.

Migration `crates/arc-storage/migrations/0001_phase0.sql` hashes to `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384` in both checkouts' working trees, indexes and accepted baseline objects. The read-only contract inventory checked 188 `.rs`/`.ts`/`.sh` files: maximum 474 lines, none over 500. This inventory does not substitute for executable preservation guards. Before delivery, the isolated index contained 71 evidence files while the integrated index contained zero; all present new evidence remains below 150. Final index equality and sealed inventory remain pending.

Attempt 1's decision and report are preserved unchanged as `implementation/task4-attempt1-gates.json` and `implementation/task4-attempt1-technical-verification.md`; its other command, source and native files remain untouched. Attempt 2 adds fresh preflight/source observations and updates the owned blocked gate/report. Delivery checks and final evidence counts are in `implementation/task4-attempt2-validation.json`.

Remaining risks: integrated native behavior, exact coverage floors, full no-skip harness, clean-checkout preservation, independent QA and final acceptance are unverified. Desktop, native Windows, vendor hooks and scheduling remain outstanding. User and commercial outcomes are unmeasured.
