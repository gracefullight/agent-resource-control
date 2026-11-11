# Independent terminal-outcomes review

Review Result: FAIL (confirmed verification-tooling blocker V1). Verification completeness: partial.

The reviewed terminal-outcome implementation compiled and all independently executed Cargo checks passed. No reproducible product defect was found in the task-1 source artifact. The existing preservation validator fails, and the full mission coverage, workspace, harness and operational evidence is not yet available. This report does not grant final mission sign-off.

Reviewer: independent-reviewer, task-4, attempt 2, mission `97195cbe-3041-4f04-986d-54cce01c0f14`. Skill used: `oma-qa`, including its execution protocol and applicable checklist. Review and checks performed on 2026-11-11 UTC, after the operator's explicit build authorization.

## Reviewed input and provenance

- Accepted committed baseline: `6b5218cf63b174b7976e52d81d56c7e3bf81112d`; verified product tree `8e68a263c1393aca92a5b1d435b5e37ba1d56ada`.
- This isolated review checkout's actual HEAD is the supplied snapshot commit `56f176362270729ac5b88145abc4a3ad3670ad66`; its verified tree is `be56e98a2524b2d7fbdfa5d307db9d5f1ed19184`. It contains the task-1 overlay. These implementation observations refer to that snapshot and the inspected worktree bytes, not to implementation already committed in the accepted baseline.
- All **18 source SHA-256 values** in `.arc-verify/terminal-outcomes/fix.md` matched the local files before checks and again after checks. The implementation handoff report SHA-256 and full path/hash inventory are recorded in the hashed `source-evidence.json` receipt below. The accepted-baseline-to-snapshot scoped diff contains those 18 source paths and `fix.md`; no source, test, manifest or lockfile was edited during this review.
- All **18 current raw-log hashes** in the handoff's `Raw logs` section matched files read at the supplied source workspace: `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/97195cbe-3041-4f04-986d-54cce01c0f14/workspace/target/terminal-outcomes/`. This isolated checkout initially contained none of those ignored logs. Historical attempts 1–6 were not treated as current verification. Fresh independent logs were produced locally below.
- The supplied snapshot and worker **ref names do not resolve in this isolated repository** (`git rev-parse --verify`, exit 128). The supplied snapshot commit itself resolves and equals the actual inspected HEAD/tree; no replacement worker revision was invented. No delivery receipt proves later installation into the integrated worktree; task-5 owns installation and must compare installed bytes.
- The complete `crates/arc-ipc` subtree is identical to the accepted baseline, tree `3336010bc9d190d8b74539201d84bb0de02b547c`. The migration subtree is also identical, tree `52a3080c7e284d2cd259c86c57795e103bc601a1`.

Migration bytes were individually compared to the accepted commit:

| Path | SHA-256 | Result |
| --- | --- | --- |
| `crates/arc-storage/migrations/0001_phase0.sql` | `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384` | MATCH |
| `crates/arc-storage/migrations/0002_managed_exec.sql` | `464a1c88270f01a56ccab6610f87eff7648e0640f276ebce53b8a97026c50814` | MATCH |
| `crates/arc-storage/migrations/0003_traffic.sql` | `1f4522e3f27c124aea7127d92434c7bd22be36168fef0a4bf665f8cf16debe90` | MATCH |

## Executed checks

Host: Darwin 27.2 arm64; Cargo 1.95.0, Bun 1.4.4. Observed resources: 8 CPUs, 16 GiB RAM and approximately 50 GiB free disk. Cargo checks ran serially without source edits or repair retries. Each check wrapper allowed 7,200 seconds, matching `mission.verificationCheckTimeoutMs`, rather than the shorter planning estimates. The runner recorded the supplied 172,800-second mission cap against the earliest supplied mission run, process IDs, commands, durations and log hashes under `target`.

| Exact command | Independent result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS, exit 0 |
| `cargo build --workspace --bins` | PASS, exit 0 |
| `cargo test -p arc-daemon --test traffic_cli terminal_outcomes` | PASS, exit 0; 10 passed, 0 failed, 0 ignored, 65 unrelated cases filtered |
| `cargo test -p arc-daemon --test traffic_cli status_compat` | PASS, exit 0; 7 passed, 0 failed, 0 ignored, 68 unrelated cases filtered |
| `cargo test -p arc-storage` | PASS, exit 0; 80 unit and 16 integration tests; 0 failed or ignored |
| `cargo test -p arc-cli` | PASS, exit 0; 53 unit and 17 integration tests; 0 failed or ignored |
| `cargo test -p arc-daemon --test managed_exec` | PASS, exit 0; 59 passed, 0 failed, 0 ignored; 124.46 seconds of test execution |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS, exit 0 |
| `bun test scripts/harness/managed-exec.test.ts` | FAIL, exit 1; 2 passed, 10 failed; V1 below |
| `git diff --check 6b5218cf63b174b7976e52d81d56c7e3bf81112d HEAD -- crates .arc-verify/terminal-outcomes` | PASS, exit 0 |

The two filtered Traffic commands select overlapping cases and are not the full Traffic suite or workspace gate. Fresh LLVM coverage was not measured by this reviewer. The implementation's filtered instrumented-run receipts were verified by hash; those are robustness evidence, not an 80% coverage measurement.

## Runtime and source review

| Requirement | Inspected evidence and independent result |
| --- | --- |
| Scheduled and lifecycle exit 17, one launch | `crates/arc-daemon/tests/traffic_cli/terminal_outcomes.rs:54` invokes the wrapper once per case. It requires wrapper exit 17, exact `fixture-stderr\n`, and exactly one launch-marker line. Both `scheduled_exit_17_persists_failed_across_restart` and `lifecycle_exit_17_persists_failed_across_restart` executed and passed. The fixture is not first run directly in these tests. |
| Persistence and restart | The same helper releases/checks permits, stops only its fixture-owned daemon, archives its first runtime report at `terminal_outcomes.rs:45`, and starts a second daemon. Its independent SQLite query checks `jobs.state`, `jobs.exit_code` and `job_lifecycle.outcome_kind` after restart. Assertions require `FAILED`, 17 and `EXITED`, then check recent and by-ID CLI output and the detailed exit code. PASS. The runtime-report collision did not recur. |
| Exit 0 and atomic storage mapping | The restart helper also tests both execution paths with code 0, requiring `SUCCEEDED` and both `succeeded` labels. The existing lifecycle and Traffic transactions both call the shared codec; `crates/arc-storage/src/jobs/codec.rs:186` maps only a validated completed exit to `SUCCEEDED` or `FAILED`. Storage tests also reopen and repeat completion. PASS. |
| Historical known and unavailable outcomes | Storage `jobs/tests/terminal_outcomes.rs:172` covers `COMPLETED` with known zero/nonzero exits, a missing kind with a recorded code, both unavailable-code forms, and inconsistent new states. CLI historical tests cover both execution paths after restart. Unknown cases never receive an invented success/failure or exit code. PASS. |
| Historical record preservation and identity reasons | Storage tests at `jobs/tests/terminal_outcomes.rs:24` and `:84` compare every joined job/lifecycle column across reads, reconciliation and rejected late attachment. `traffic_cli/terminal_outcomes_historical.rs:86` also verifies protocol-1 `CHILD` / `VERIFIED_MATCH`, neutral `NONE` reasons, CLI identity wording and unchanged rows for both unavailable forms. PASS. |
| Transient DB_BUSY and later rows | `traffic_cli/status_compat_terminal.rs:170` injects a first busy Traffic response for recorded signal and spawn-failure rows, requires their correct labels, and requires later zero/nonzero rows to remain visible. Its lookup inventory proves no busy retry consumes the later rows' budget. The legacy-unknown case at `:55` also passes. |
| By-ID recorded outcome with unavailable Traffic | `traffic_cli/status_compat_terminal.rs:261` forces Traffic protocol failure and requires consistent `succeeded`, `failed`, signal and spawn-failure labels with the actual recorded detail; the nonterminal case remains unknown without an exit code. PASS. |
| Bounded recent lookups | `RECENT_SQL` retains its materialized 51-candidate bound, 50-row output and indexed order. The existing VM-step regression with 10,000 unrelated legacy rows passed in storage tests. Both the 50-terminal-row CLI case and the delayed lookup case passed; the latter observes one lookup within a shared 250 ms enrichment deadline. Unknown is used after the deadline. |
| Retries and permit release | `traffic_cli/terminal_outcomes.rs:288` drops a completion response, retries, and requires one released permit, no active permit, `FAILED` and code 17. `managed_exec/retries.rs:105` exercises before/after-commit reply loss, one launch, exact stderr and persisted failure; completion-unconfirmed cases retain unknown outcomes. PASS. |
| Signals, spawn failure, cancellation and unknown | Dedicated storage cases keep `SIGNALED` and `SPAWN_FAILED` distinct. The executed status proxy cases retain their labels under failure. Storage Traffic cancellation/unknown cases and managed-exec cancellation, directed signal, child-death and restart cases passed. New storage state validation does not convert missing outcomes to exits. |
| Protocol-1 peers | IPC source/types are unchanged. Both restart cases assert wire phase `COMPLETED`. The executed frozen Python old-client fixture checks exact field sets and closed protocol shapes against the new daemon; the current-client proxy rejects `STATUS_V2` as a legacy daemon would and forwards protocol-1 lifecycle responses. It checks both status views. PASS for these peer fixtures, with the downgrade limit below. |
| Fail-open, process semantics and redaction | The 59-case managed-exec run includes outage/one-invocation/exit-17, unchanged argv/environment/cwd/streams, cancellation, denied connection, missing/replaced endpoints, signals, descriptors, terminal behavior, no replay and secret-absence checks. All passed. The new no-busy-retry transport is used for list enrichment; existing execution exchanges retain busy retries and continuity handling. Errors remain fixed local categories, and new SQL uses bound parameters. |

No new security or execution-semantics defect was reproduced. This is a text CLI change; no browser, GUI, screen or Playwright checks were used. A screen-reader study was not performed or implied by the CLI assertions.

## HIGH: V1 — Existing preservation gate cannot validate accepted source and its fixtures

Classification: confirmed verification-tooling defect, outside task-1 product ownership. Owner handoff: Chief Director to the task-5 backend/tooling writer, with Technical Director coordination. This reproduces the implementation owner's disclosed blocker; it is not evidence that the terminal-outcome implementation changed protected baseline tests.

Reproduction: in this exact review snapshot, run `bun test scripts/harness/managed-exec.test.ts`. It exits 1 with 10 failures. The complete raw log and hash are below. The first-pass blocker has three observed causes:

1. `scripts/coverage/current-preservation.ts:9`, `:359` and `:370` use immutable feature baseline `19d7a507044cbf83918f58cd41056905c275c8d4`, then reject `crates/arc-daemon/src/db_worker/tests.rs` with `REGRESSION: existing test/helper item changed or unreachable`. The rejected file is byte-for-byte identical to accepted HEAD `6b5218c`; the accepted commit contains stronger cancellation/drop-receiver tests absent from the reference. The validator and schema adaptation files also match accepted HEAD. Expected: accept the specifically authorized committed repair while still rejecting weakened assertions, disabled tests and changed worktree bytes. Repair direction: add an exact, immutable accepted-reference adaptation for those cancellation items and matching positive/negative fixtures; retain the other pinned comparisons. A suitable pinned input is `gitBytes(root, ["show", "6b5218cf63b174b7976e52d81d56c7e3bf81112d:crates/arc-daemon/src/db_worker/tests.rs"])`; do not derive golden bytes from the candidate or revert the accepted tests.
2. `scripts/harness/managed-exec.test.ts:13` creates a pre-Traffic fixture while `scripts/coverage/current-preservation.ts:359` calls the seven-crate live inventory. Multiple negative fixtures fail with `ENOENT .../crates/arc-traffic/src` instead of their intended regression diagnostic; the authorized-production positive fixture fails too. Expected: build a complete feature fixture with the required accepted seventh-crate inputs, and keep six-crate historical validation separate. The existing fixture-copy pattern can explicitly include `cpSync(resolve(root, "crates/arc-traffic"), resolve(dir, "crates/arc-traffic"), { recursive: true })` as part of a consistent fixture repair. Keep all seven production denominators; do not ignore a missing crate or relax the 80% floor.
3. `scripts/harness/managed-exec.test.ts:56` still requires `assert_eq!(stamped.witness, 2);`, while `scripts/coverage/schema-adaptations.ts:319` deliberately produces witness 3. The mutation list at `managed-exec.test.ts:81` still targets `assert_eq!(count, 2);`, so it fails its own `after != before` guard against the schema-3 fixture. Expected: schema-3 positive and negative fixtures must exercise the actual resulting bytes while retaining fixed schema-1 witness assertions. For example, the current-registry expectation must require `expect(acceptance).toContain("assert_eq!(stamped.witness, 3);")`; corresponding mutation inputs must target the schema-3 counts. These are remediation sketches, not an executed or approved tooling patch.

The 10 failed test names are: live checkout preservation; authorized production/lifecycle addition; exact schema-2 adaptations; schema guards; disabled/early-return schema tests; external macro/nested-module guards; generated macro/wildcard guards; unresolved item macro guards; changed assertion/helper guards; and production-root/enclosing-module guards. Migration byte/mode/symlink and coverage-denominator/harness-bypass tests passed. The first live failure can mask additional validator mismatches; this report does not assert those unexecuted paths pass. Preserve all protected checks and rerun the complete validator after the listed repair.

## Verification gaps and remaining risks

- Full `cargo test --workspace`, the existing `bun run test:coverage` gate, fresh per-crate production-line percentages, the full harness without skips, and final `gates.json` / `audit.json` evidence remain **unverified here**. Their execution and report installation belong to task-5. V1 is a confirmed prerequisite failure, not a passing alternative to those commands. Do not issue final PASS until every required gate is satisfied or accurately reported unavailable.
- Same-database downgrade is not supported by the accepted daemon's closed storage decoder: it cannot decode newly stored `SUCCEEDED` / `FAILED`. The decoder unit test and accepted source establish this format limitation; an old daemon executable was not run against a new database in this review. The passing old-peer fixtures establish protocol-1 compatibility, not a successful database downgrade. The new public Rust `JobError::UnsupportedStatus` variant also affects exhaustive downstream Rust matches; workspace compilation passed, external consumers were not built.
- Global `git status --short` printed `av.yaml: Operation not permitted` even with exit 0. No complete-worktree-clean or container-configuration/health claim is made. Scoped source status was clean. A diagnostic `ps -p <owned cargo pid> ...` attempt was also denied with `PermissionError: [Errno 1] Operation not permitted`; it was not bypassed. All owned check processes returned through their original runner.
- Serena is configured in `.mcp.json` but no Serena/code-intelligence tools were exposed after discovery. Review used scoped reads of already located task paths and Git byte comparisons, without substituting recursive native project source searches. Broad impact discovery remains limited.
- Named snapshot/worker refs were unavailable locally as recorded above. Provenance is anchored to the actual snapshot commit/tree and matching worktree SHA-256 values. Later integrated bytes and the mission-wide evidence budget must be audited by task-5. This review adds one report; all new raw logs and receipts are under ignored `target`. The review snapshot's mission evidence currently consists of two files totaling under 57 KB, below both mission limits; this is not the later integrated inventory.

## Raw evidence and handoff

All following relative paths are in **this Actor workspace**, not necessarily installed in the integrated checkout. Task-5 must preserve/copy these raw files under integrated `target` with identical hashes if it installs this report; a report-only transfer does not transfer ignored logs.

Prefix: `target/terminal-outcomes/qa-preflight-attempt-2/`.

| File under prefix | SHA-256 |
| --- | --- |
| `format.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `build.log` | `9bfb0cba59323e26a3e5763e2a71e0341a3491d39d413edbebf790db7c9fe14f` |
| `terminal.log` | `f418068874b66e13621441cccb6a7f5ac75eb45fecee3f8c7c8b900ba5530915` |
| `status.log` | `4af7dcedc65fd9860acea9d51d0bb841431a2d4954875b40e236260283bc1b01` |
| `storage.log` | `de1108af62bfe49281b51f02b15a4d7cb28361a33d04aa170701208854f8a229` |
| `cli.log` | `88b20cb18fec458800e849ad11ee03c0e3e578d7eeb3804c756a0767dcece6f6` |
| `managed.log` | `38fa16ff3fc1f8af457edcbff08f6eddd682b6b2c1d47f598743f5e9d3c66ed7` |
| `clippy.log` | `fe0a22c9db421340d6ed2ddff444018eb9a51a507ee38a2ed3d7524150247544` |
| `preservation.log` | `97322360f9eb0d4fead803486f51a8f3c750782402ebff8ac14294dfc27d4caa` |
| `checks.json` | `04e747d9b380a1e51de2f39e459832651304608b7341018a73a47d40338aa8a1` |
| `preservation.json` | `677ff5a2ad657f9de4622cfc74c321cde0ca0876449d8c8ebd6cb2a1b9b49dd3` |
| `source-evidence.json` | `ff6161545ab9a07a42759c781b038b82cbf7c761ba412fe956d5b90fe5913ecd` |
| `tooling-provenance.json` | `a61feacc7f52f42d177ac6359d668191d84a73c5edcff5b339c5d0133f546ab8` |
| `final-source-check.json` | `b281360ecadfd18e5ee67eaf426b405c81b39c5fbb4a9e7fe9cf0adc4ea63d0a` |

Acceptance checklist:

- [x] Reviewed source inputs match the dependency evidence, with baseline/snapshot/worktree scopes distinguished.
- [x] Compiled the reviewed source and independently ran the relevant deterministic macOS, status, storage and execution-semantics tests.
- [x] Recorded commands, outcomes, reproducible blockers, source locations, repair directions and unavailable evidence without claiming final PASS.
- [x] Routed V1 to Chief Director and the backend/tooling owner through this handoff; no new task-1 product defect requires source repair.
- [x] Preserved implementation, tests, manifests, mission state and Actor assignments. Only `.arc-verify/terminal-outcomes/qa-preflight.md` is a product deliverable from this task; raw diagnostic artifacts remain under `target`.
