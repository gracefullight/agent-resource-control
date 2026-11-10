# Task 7 repair handoff

Status: **partial**. This repair addresses transient database contention, the shared-stderr flag defect, native Cargo launch observation, and missing scheduled test/lint process coverage. It does not resolve the unattached-grant recovery defect. A storage-owner handoff is required before task 7 can satisfy its recovery acceptance criterion.

## Changes in this repair

- `crates/arc-cli/src/exec/transport.rs` retries responsive `DB_BUSY` replies with a 20 ms backoff until the existing absolute exchange deadline; endpoint failures retain their three-attempt bound. `crates/arc-cli/src/exec/transport/live_tests.rs` covers six busy replies followed by success. The pinned historical `transport/tests.rs` was restored to Git blob `41902733a5c1b741ac2097dcccb1ad7fd439e9ca`.
- `crates/arc-cli/src/exec/traffic.rs` writes each queue or cleanup notice in a short-lived, signal-blocked helper process. The parent waits for delivery or kills and reaps the helper after 50 ms. It never changes the inherited stderr open-file-description flags; the helper cannot write a stale notice after the parent returns. The wrapped command remains a single native spawn.
- `crates/arc-daemon/tests/traffic_cli.rs` and `crates/arc-daemon/tests/traffic_cli/{measurement,protocol,recovery,robustness,semantics}.rs` split the earlier 909-line file into six files below 500 lines. The tests cover an actual SQLite write-lock contention burst while a real Cargo build holds the permit, queued child execution with full and broken stderr, a blocked sibling writer, native scheduled test and lint streams, cwd, environment, process group and exit status, and scheduled signal status.
- The Cargo measurement fixture logs every launch in a temporary `cargo` observer that replaces itself with the real Cargo executable. The held process is independently checked as Cargo, the logged PID is matched to the CLI's child, cached launches are counted independently of build scripts, and peak counts are derived from observed overlapping or non-overlapping process intervals. Committed permit release and second grant are checked before the second start. The measurement output comes from those observations, not literal peak or invocation values.

The initial task-7 implementation already changed the daemon and CLI paths listed in `git diff --stat -- crates/arc-cli crates/arc-daemon`; the new untracked Traffic modules and split tests are not shown in that stat. This repair did not edit the shared manifests, lockfile, storage implementation, saved task, or mission state.

## Acceptance evidence

- `cargo test -p arc-daemon --test traffic_cli -- --nocapture --test-threads=1`: 14 passed, none ignored or filtered. On macOS, the two independent project and target directories measured peaks 2 bypassed and 1 scheduled, launch counts `[1,1]` for each arm, a 123 ms scheduled queue wait, and two recorded `EXITED:0` outcomes. The test witnessed the second notice while the first permit was active, then the committed release, second grant and second Cargo start. Remote and UNKNOWN fixtures ran while the queue was full without changing the active reservation totals. Raw `target/traffic-task7-repair/traffic-cli-final.log`: SHA-256 `27bc0b428b27949cda0f20d22d8368cccfb9900b8112ae90400c8c919b530972`.
- `cargo test -p arc-cli --bin arcctl exec::transport::live_tests:: -- --test-threads=1`: 6 passed, including the busy burst.
- `cargo fmt --all -- --check`: passed. Raw `target/traffic-task7-repair/fmt.log`: SHA-256 `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed. Raw `target/traffic-task7-repair/clippy.log`: SHA-256 `96ebe4196b5ca9385619d455e4e136eb63bacdd0c9ce119be188bebc48b8897e`.
- `cargo test --workspace`: passed, including all 56 managed-execution tests and 14 Traffic CLI tests. Raw `target/traffic-task7-repair/workspace-test.log`: SHA-256 `4f5fc0c9a45bf39c15eea7f40fc9d47771b1aceac27240d30b210ffc97e6c120`.
- `git diff --check -- crates/arc-cli crates/arc-daemon`: passed for tracked files. The untracked files are included in Rust formatting and tests; the full harness is run using an isolated intent-to-add index so they also enter its structural scan.
- Coverage, harness, and source-bound evidence outcomes are recorded in the gate update below.

## Required coordination and limits

1. `crates/arc-storage/src/jobs/traffic.rs` excludes an active grant lacking a verified child from recovery candidates, and `crates/arc-storage/src/jobs/traffic/tests/recovery.rs` explicitly retains that grant when the owner disappears. `crates/arc-cli/src/exec.rs` makes bounded attachment and completion attempts; if the daemon is unavailable for both attempts and remains unavailable until the owner exits, restart has no verified child identity or terminal outcome to reconcile. Releasing solely because the owner exited would be unsafe when the child survives. The storage owner and Chief must provide an authenticated, durable way to preserve child identity or terminal evidence across the gap, then add the requested real-CLI regression for daemon loss after grant and before attachment, covering both a surviving and an exited child. This Actor did not alter storage-owned files or claim that this scenario passed.
2. `crates/arc-storage/src/jobs/traffic/tests/recovery.rs` is 1,088 lines and remains untracked in the integrated worktree. The harness's 500-line cap applies when that file is included. The storage owner must split it without removing or weakening its tests. The isolated harness index at `target/traffic-task7-repair/harness.index` includes all 40 untracked crate/script paths by intent-to-add without changing the shared index.
3. The macOS measurement above is an Actor test log, not the task-9 source-bound `mac-acceptance.json` receipt. Independent QA, task-9 measurement collection, final mission evidence, and desktop, native Windows, vendor hooks, broader detection and remaining scheduling policy are still outstanding.
4. The requested Serena provider was not present in the tool inventory; inspection was confined to supplied and already located paths. `git status --short` reported `av.yaml: Operation not permitted`; the file was preserved. A diagnostic `ps` command was also denied by the host and was not worked around.

## Attempts and environment

The first split-test compilation failed because the integration-test root needed explicit `#[path]` module locations; these were added. The first full-stderr check compared every `F_GETFL` bit and failed when Darwin exposed transient kernel-internal bit `0x10000` while a sibling write slept. The test now checks user-settable status flags, including `O_NONBLOCK`, and separately verifies the sibling write succeeds after draining; the corrected full-stderr test and the full Traffic suite passed. No user or acceptance criterion was changed.

Used the `oma-backend` skill and its execution policy. Host: macOS 27.2.1, Rust/Cargo 1.95.0, Bun 1.4.4, 8 CPUs, 16 GiB RAM, and approximately 64 GiB free at start. No Playwright or browser tooling was installed or used.

## Gate update

The direct `bun run test:coverage` invocation failed in instrumented `fail_open::outages_preserve_native_bytes_exit17_one_invocation_budget_and_no_replay`: the existing fault proxy reported `Invalid argument` while setting a Unix-socket read timeout (`crates/arc-daemon/tests/managed_exec/fault.rs:93`). The managed-execution test result was 55 passed, 1 failed, so no valid per-crate coverage percentages were generated. Running that same instrumented test binary alone passed (1 passed, 55 filtered); the cause of the original failure is not established.

- Raw `target/traffic-task7-repair/coverage.log`: SHA-256 `dad0671d29a3595e3aaa25a08be5e401e588c0299f664cb365c2984b9ca310d0`.
- The gate's captured instrumented-test stdout was copied unchanged to `target/traffic-task7-repair/coverage-instrumented-stdout.log`: SHA-256 `9c56592e655eb876a4ad2870576fdc4094c5f9b29082cf297be19b08fa1b85c4`.
- Raw focused rerun `target/traffic-task7-repair/coverage-repro.log`: SHA-256 `a1fe50b547a18616a75176d6751056646c74cd3896685253257ebb5d45e0070e`.

An unmodified no-skip `./scripts/harness/validate.sh` invocation ran with the isolated index and failed only its file-length check: `crates/arc-storage/src/jobs/traffic/tests/recovery.rs` has 1,088 lines. The first three structural checks and the nested coverage check passed. The isolated index included all 40 untracked crate and script paths, but did not add the untracked Traffic handoff Markdown files to the harness file list.

Both preexisting migration SQL files match accepted HEAD byte-for-byte: `0001_phase0.sql` SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`, and `0002_managed_exec.sql` SHA-256 `464a1c88270f01a56ccab6610f87eff7648e0640f276ebce53b8a97026c50814`.

## Final gate results

- The coverage command executed inside the full harness completed successfully with production-line coverage: arc-core 96.48%, arc-detector 99.84%, arc-storage 95.29%, arc-ipc 98.92%, arc-daemon 92.68%, arc-cli 85.64%, and arc-traffic 100.00%. The preceding standalone coverage attempt remained a failed attempt as described above.
- `bun test scripts/coverage/evidence.test.ts`: 1 passed, 0 failed against the current source-bound collection. `target/traffic-task7-repair/coverage-evidence.log` SHA-256 `13285ed201b516b6a94d49e6aee4469ac1d5522931a059f22d4628950f5e3723`.
- Full harness `target/traffic-task7-repair/harness.log` SHA-256 `92eca409afc51d7c0a800690fb6b8ba8156946ea1d60a9244dc65b6ae59807ea`; it failed the storage length check while its coverage check passed.
- `bun test scripts/harness/traffic-targets.test.ts`: 8 passed. The current saved mission has no observation policy, so the live target check reported not applicable; no target health observation is claimed. `target/traffic-task7-repair/targets-test.log` SHA-256 `debef47555d7dc87e74d9541163fad6c7e3ddd9577ade7ae8bc39d7d7cc3bb71`.
- `bun test scripts/harness/traffic-evidence.test.ts`: 8 fixture checks passed and the live check failed because `.arc-verify/traffic/gate-summary.json` does not yet exist. That receipt is a task-9 deliverable under the binding operator note; this task did not fabricate it. `target/traffic-task7-repair/traffic-evidence.log` SHA-256 `7e93385e41d4b0d6f237a0e866bbe83e563804d37445106b14cddf3de19d5c2b`.
- The workspace test transcript contains 474 passed, 0 failed and 0 ignored across 37 result groups. The initial transport test command used an incorrect filter and selected zero tests; the corrected live-test command selected and passed six. `cargo build -p arc-cli --bin arcctl` passed before the native CLI suite.
- At the last check, `.arc-verify/traffic` contained 11 files totaling under 142 KB.

The diagnostic fix creates a separate short-lived helper process for a queued notice; it does not execute the target command. Independent review should assess that detail against any interpretation of the single-spawn requirement that counts diagnostic helpers as well as target launches. The remaining confirmed recovery gap prevents completion regardless of the passing tests.
