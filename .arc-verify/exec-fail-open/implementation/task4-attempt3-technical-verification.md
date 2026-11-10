# Task 4 technical verification — attempt 3

Status: blocked. The integrated final schedule cannot complete under the current filesystem permissions. `implementation/gates.json` remains `passed: false`. This delivery reports fresh failures; it does not claim native or coverage acceptance.

Actual collector: technical-director. Skill used: oma-architecture, for the ownership/interface diagnosis. Only assigned evidence/report files changed. No product source, tests, manifests, lockfiles, validators, mission state or instructions changed.

## Current checks

All commands below ran serially in the integrated mission checkout on 2026-11-08 at 23:04 UTC, after reviewing both operator notes and prior findings. Each direct gate allowed 3,600,000 ms. Exact cwd, argv, PID, timestamps, elapsed time and output are in `implementation/task4-attempt3-commands.json`; prerequisite failures are in `implementation/task4-attempt3-preflight.json`.

| Command | Exit | Elapsed | Outcome |
| --- | ---: | ---: | --- |
| `git add -- .arc-verify/exec-fail-open/technical-verification.md` | 128 | 11.677 ms | Cannot create real `.git/index.lock`: Operation not permitted. Index unchanged. |
| `bun scripts/harness/exec-fail-open-receipts.ts task4-attempt3-integrated-fmt cargo fmt --all -- --check` | 1 | 383.694 ms | Cannot create normal native receipt output directory: EPERM. Cargo never launched through this wrapper. |
| `cargo fmt --all -- --check` | 0 | 402.578 ms | Formatting passes directly against integrated source. This is a Technical Director observation, not a backend receipt. |
| `cargo clippy --workspace --all-targets -- -D warnings` | 101 | 183.742 ms | Cannot open integrated `target/debug/.cargo-lock`: Operation not permitted. No Clippy pass claimed. |

The Clippy failure confirms that denial affects the normal Cargo path as well as receipt output. No alternate target, index, native-output location, GUI or privileged retry was used. Repeating collectors under these permissions would not produce the required integrated evidence. No background worker remains.

## Source provenance and preservation inventory

`implementation/task4-attempt3-source.json` records fresh source observations. The accepted committed baseline and integrated HEAD are `00773c69a8e5e7fd417fd5cc78d03e222364f609`, tree `b98f996caa04fe13eeeeab1072c3c528f6d18c65`. Task-3 changes remain an uncommitted overlay there, with an empty staged diff. Its source-scope baseline diff hash is `c24d89c44843e61a76edb25c08908187edd58c983d1d202f7b9219e0ad100d53`.

This Actor snapshot/HEAD is `2b07c2b4b81236f69c1b5ad52204cbfb17892a5f`, tree `385bb15ac5db16de9a48728285745299627c9b44`. Its source-scope baseline diff hash is `3e82e9dd060c1a29dcc9cb28523c4eb8d30e178916fb9c89f44a7c62392cc053`. Actor-local tracked files do not imply integrated tracking. The gate now records the actual integrated Git observation rather than substituting the Actor HEAD.

All 212 contract inputs match between the checkouts, source hash `3d51ee23c34f602cf6bc5bb4ca6faf7d626aa7f5bcfc7347210e338dd3a0df17`. Prior execution receipts were not rebound to those observations. Their raw absolute paths and results remain historical.

The read-only preservation inventory records migration `crates/arc-storage/migrations/0001_phase0.sql` SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384` in both working trees, both indexes and the accepted baseline. Mode is 100644. Among 188 contract `.rs`/`.ts`/`.sh` sources, the maximum is 474 lines; none exceeds 500. These observations do not replace executable historical, accounting or tamper-rejection checks.

## Collector interface and Chief handoff

The fixed validator requires backend identity at `scripts/harness/exec-fail-open-gates.ts:21` and oma-backend at `exec-fail-open-inputs.ts:72-73`; this assignment names technical-director/oma-architecture. No successful backend receipt can truthfully be assembled from this Actor's executions. The maintained capture defaults and assembler labels were not used to disguise that conflict.

Chief action needed: arrange supported collection access to the integrated real index and target directories, then coordinate genuine backend/oma-backend collection under the unchanged interface. Current access denial blocks that collection regardless of Actor identity. The alternative of running everything again in this isolated snapshot would duplicate expensive work and would still fail the required integrated absolute-path evidence contract. No operator implementation choice is requested.

After access is resolved, stage only the intended integrated inputs, freeze them, run the exact schedule below, then assemble genuine receipts. Preserve the failed attempts and obtain independent oma-qa. Source changes require recollecting affected commands. No validator, target, coverage denominator or acceptance criterion may be weakened.

## Required schedule

Only the direct formatting command passed; Clippy failed as recorded. Remaining commands did not run during attempt 3 because integrated writes are blocked. Missing results are pending, never passes or skipped successes.

1. `cargo fmt --all -- --check` — passed.
2. `cargo clippy --workspace --all-targets -- -D warnings` — failed.
3. `cargo test --workspace` — not-run-prerequisite-blocked.
4. `bun run test:coverage` — not-run-prerequisite-blocked.
5. `./scripts/harness/validate.sh` — not-run-prerequisite-blocked.
6. `bun test scripts/harness/managed-exec.test.ts scripts/harness/coverage-verification.test.ts scripts/harness/equivalence.test.ts scripts/harness/exec-fail-open-preservation.test.ts` — not-run-prerequisite-blocked.
7. `bun test scripts/coverage/clean-checkout.test.ts` — not-run-prerequisite-blocked.
8. `bun test scripts/coverage/coverage-cli.test.ts` — not-run-prerequisite-blocked.
9. `bun test scripts/coverage/harness.test.ts` — not-run-prerequisite-blocked.
10. `bun test scripts/coverage/evidence.test.ts` — not-run-prerequisite-blocked.
11. `bun test scripts/harness/exec-fail-open-docs.test.ts` — not-run-prerequisite-blocked.

The live evidence check must follow the final collector. The two terminal validator commands remain downstream. All nine coverage files, fixed historical assertions, no-skip harness, preservation and accounting rejection tests remain mandatory. There are no current integrated LCOV, LLVM exports or profile bytes from this attempt, no independent integer recomputation, and no proved per-crate 80% floor. No average or retained summary substitutes for them.

The full integrated native outage, compatibility, exclusion, signal, foreground-pipeline and recovery matrix remains uncollected, including committed REGISTER/CLAIM acknowledgement loss. Metadata elapsed time, invocation elapsed time and scheduling overhead are unavailable. The format/Clippy elapsed values above are command durations, not native invocation timings. Prior isolated measurements are preserved but do not satisfy this assignment.

## Resources and retained failures

Darwin 27.2.0, arm64, 8 logical CPUs, 16 GiB RAM and approximately 43 GiB free disk. Installed probes pass: Bun 1.4.4, Cargo/Rust 1.95.0, cargo-llvm-cov 0.6.21, LLVM 22.1.2 and the Rust LLVM component. Installed versions do not establish locked offline dependency readiness. No provisioning or downloads were needed or attempted.

Saved mission limits remain 86,400 seconds, concurrency 3, 200 runs, three retries, 1,000 ms retry delay and 3,600,000 ms per check. Checks here ran serially. The absolute mission start/deadline is not supplied in the assignment or relevant environment; no replacement deadline was invented. Host resource limits and exact free bytes are retained in the command report.

Attempt 2 gate/report bytes are preserved as `implementation/task4-attempt2-gates.json` and `implementation/task4-attempt2-technical-verification.md`. Attempt 1 artifacts and prior command/native reports remain untouched. Current delivery inventory and expected validator rejection are recorded in `implementation/task4-attempt3-validation.json`; the present evidence set remains below 150 files. The integrated evidence index still has zero files and is not sealed.

Remaining risks: all blocked technical gates, native timing/compatibility/recovery, exact coverage floors, independent QA and final acceptance. Desktop, native Windows, vendor hooks and scheduling remain outstanding. User and commercial outcomes are unmeasured.
