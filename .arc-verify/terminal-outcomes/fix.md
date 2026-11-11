# Terminal outcomes implementation handoff

Status: task-1 implementation and focused macOS checks pass. Independent QA, the full workspace test, the full coverage gate, the complete harness, and container verification remain for the Chief Director and dependent tasks. The preservation-validator failure below requires coordination outside task-1 ownership. Historical sections after the attempt 7 record describe earlier attempts; their build-prohibition and unexecuted-test statements are no longer current.

## Attempt 7: diagnosis and repair

The operator explicitly authorized compilation. At `6b5218cf63b174b7976e52d81d56c7e3bf81112d` on Darwin arm64, both regressions were reproduced before repair:

- A `DB_BUSY` response from the recent list's Traffic lookup mapped a validated lifecycle `SIGNALED` or `SPAWN_FAILED` summary to `unknown`. Recent status now retains the phase's known signal or spawn-failure label when that lookup is unavailable. It still stops on the first busy reply for each row, uses the existing shared 250 ms deadline, and leaves later rows available. The proxy regression produces a transient busy reply for each affected row and verifies both labels and the later exit-0 and exit-17 rows.
- A historical `COMPLETED` row with no exit code was decoded to an `OUTCOME_UNKNOWN` snapshot with invented `IDENTITY_UNAVAILABLE` reasons. It now receives neutral `NONE` reasons in both by-ID and recent results, retaining the recorded `CHILD` / `VERIFIED_MATCH` observation. Both missing-kind and `EXITED`-without-code forms are covered; the tests validate protocol-1 decoding and CLI output and compare every stored job and lifecycle column before and after reads and reconciliation.
- By-ID fallback for unavailable Traffic retains validated recorded exits, signals and spawn failures, and reports the actual exit code. The compatibility regression covers exit 0, exit 17, signal, spawn failure and a nonterminal unknown.
- A first focused run exposed a macOS proxy race: an accepted nonblocking Unix socket could return `WouldBlock` before the request arrived. The new status proxies now switch accepted streams to blocking mode with a bounded read timeout. The restart fixture retains the earlier repair that archives the first fixture-owned daemon's runtime report before starting the second daemon.
- The new integration cases were split into `status_compat_terminal.rs` and `terminal_outcomes_historical.rs`; all changed Rust files are within the existing 500-line cap. An initial split omitted a `JobRequest` import and failed compilation; restoring it made the subsequent tests and Clippy pass.

The existing implementation writes `SUCCEEDED` for validated exit 0 and `FAILED` for validated nonzero exits through the shared storage codec; signal, spawn failure, cancellation and unknown remain distinct. It keeps protocol-1 `COMPLETED` on the wire and keeps the SQLite migrations unchanged. The old-client fixture and a proxy emulating a legacy daemon passed. The accepted-HEAD daemon's old closed phase decoder rejects the newly stored database states; the same-database downgrade risk is established by source and the phase-decoder unit test, but an old daemon executable was not run against the new database. No migration, dependency or IPC field was added. The pre-existing task change adds the public Rust `JobError::UnsupportedStatus` variant, which exhaustive downstream Rust matches must handle.

## Current verification

| Exact command | Result |
| --- | --- |
| `cargo build --workspace --bins` | PASS, exit 0 on final source. |
| `cargo test -p arc-daemon --test traffic_cli terminal_outcomes` | PASS, 10 tests. Both named exit-17 tests each run one fixture invocation, assert one marker, exact stderr and wrapper exit, restart their fixture-owned daemon, query SQLite and check both status views. Exit 0, historical rows, 50-row deadline, retry and permit release cases also passed. |
| `cargo test -p arc-daemon --test traffic_cli status_compat` | PASS, 7 tests, including transient DB_BUSY and Traffic-unavailable regressions and protocol peer fixtures. |
| `cargo test -p arc-storage` | PASS: 80 unit and 16 integration tests. |
| `cargo test -p arc-cli` | PASS: 53 unit and 17 integration tests. |
| `cargo test -p arc-daemon --test managed_exec` | PASS: 59 tests, including fail-open, child streams, retries and signal semantics. The later source change only split the separate Traffic test modules and added a comment in the codec. |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS, exit 0. |
| `cargo fmt --all -- --check` | PASS, exit 0. |
| `CARGO_LLVM_COV_TARGET_DIR=$PWD/target/terminal-outcomes/coverage-target LLVM_PROFILE_FILE_NAME=arc-%p-%m.profraw cargo llvm-cov --workspace --all-targets --locked --offline --no-report --no-cfg-coverage --no-cfg-coverage-nightly -- terminal_outcomes` | PASS: the 10 matching tests, including both restart tests and the 50-row deadline case, under instrumentation. This is a filtered robustness check, not a coverage measurement or gate. |
| Same instrumented command with `status_compat` as the filter | PASS: 7 matching tests. An initial command combined `--no-report` with incompatible `--no-clean`; its exit 1 and the corrected run are retained below. |
| `bun test scripts/harness/managed-exec.test.ts` | FAIL: 2 passed, 10 failed. See ownership handoff below. |
| Scoped `git diff --check`; byte comparison of all three migrations against HEAD | PASS. Migration SHA-256 values are in the source-identity raw log. |

The earliest storage and busy-status regression runs each failed for the intended reason before the fix (exit 101); their raw logs are listed below. The first complete terminal selection failed because of the proxy socket race; after the socket fix and module split the final run passed. The filtered runs are not the full workspace or coverage gates; no passing coverage percentages are claimed. Source inspection used scoped reads of known paths because the configured Serena tools were unavailable. Global Git status reported `av.yaml: Operation not permitted`; the complete worktree status remains unverified.

## Validator and ownership handoff

`bun test scripts/harness/managed-exec.test.ts` stops its live preservation check at `REGRESSION: existing test/helper item changed or unreachable: crates/arc-daemon/src/db_worker/tests.rs`. That file, `scripts/coverage/current-preservation.ts`, and `scripts/harness/managed-exec.test.ts` all match accepted HEAD byte-for-byte. The validator compares against `19d7a507044cbf83918f58cd41056905c275c8d4`, while the accepted HEAD includes earlier schema-3 and cancellation-test changes. The validator's historical-fixture tests also fail because the old checkout has no `crates/arc-traffic/src` while the current inventory expects seven crates, and because some fixture assertions still expect schema-2 adaptations. The first failure may hide additional live-check findings. The raw log records all 10 failed test names and diagnostics. A tooling owner must reconcile the immutable baseline and historical fixture handling without weakening protected checks, then rerun the validator and full mission gates. This task did not edit those scripts or revert accepted-HEAD tests.

## Source identity and handoff files

These hashes describe the task-owned source files to install for dependent tasks; the current report is the additional task-owned artifact. New source files are currently untracked. No commit or mission-state mutation was made.

| Path | SHA-256 |
| --- | --- |
| `crates/arc-cli/src/exec/transport.rs` | `8f7395f6e5f8569bc803cb2d035ab62d31cb2851356d4c65e1c3eef970b82c8e` |
| `crates/arc-cli/src/exec/transport/live_tests.rs` | `2daa04ef00f3575681d98602c03496773a88af9230892a0fd9c3cc9e929eff7b` |
| `crates/arc-cli/src/job.rs` | `c2024ede50b9f4073cb7979f5f6292b04365c2a9cba5ac98721240eed7e93ccb` |
| `crates/arc-cli/tests/fixtures/child.rs` | `42200fb9843827d6e6cc30f6f8aa781472e9da2de86600a7632e74d781276ed0` |
| `crates/arc-daemon/tests/managed_exec/cli.rs` | `806ef1d91cba74e31bef58572dc1fc85f7f6028c5cf726f6eedab5633651c7d5` |
| `crates/arc-daemon/tests/managed_exec/retries.rs` | `890c0346d06e5eee0da81c83fc8212158db40e4324df1b46c0618a9ff064c448` |
| `crates/arc-daemon/tests/traffic_cli.rs` | `37c6824feb0c6af95894f203d9f0cde9fa4ad061d6f9e80ee219ee743735d647` |
| `crates/arc-daemon/tests/traffic_cli/status_compat.rs` | `b8a4f0222b96b233bba8c996de4c67394f0669dbd213f4cf4dc523782dc020d6` |
| `crates/arc-daemon/tests/traffic_cli/status_compat_terminal.rs` | `510c3d874e5da90436431ef3768890a3ac7e46a006e94778ea14bbb569bdf3a6` |
| `crates/arc-daemon/tests/traffic_cli/terminal_outcomes.rs` | `642a9b3f90ab57d5c8942279b974054bb1ed63d21527568ba0958cc429b94656` |
| `crates/arc-daemon/tests/traffic_cli/terminal_outcomes_historical.rs` | `75f80c0b75324ff266a8a16edb23b80d71b8b191bd52b9254c26a027126cad99` |
| `crates/arc-storage/src/jobs.rs` | `7c6201ab8bdcc2be57a856e5ac08cc4381ea46d16d5174fcf2707375f820989e` |
| `crates/arc-storage/src/jobs/codec.rs` | `979fec1eab53b53694dae6420dc3fffb00ab38455d56748f694a37d9f32c0f6c` |
| `crates/arc-storage/src/jobs/reconcile.rs` | `56904b85a9e2ac21483b4d54a2fd0fade0396d44376bbffd5dbf13549548090e` |
| `crates/arc-storage/src/jobs/tests.rs` | `7e21ed867beecb4d52807432e5f9cf8b03dbddb0621cdeda701b2ff53d861229` |
| `crates/arc-storage/src/jobs/tests/terminal_outcomes.rs` | `bf1ddf8fc0a35cebdaa21b7243df328ce529b61c16c86526b85a7818ebfca237` |
| `crates/arc-storage/src/jobs/traffic/store.rs` | `cb3fe1155e02c15ed30c1c567aa941a2a4e646fbe9eee6e5d2e82f10b0cdc24a` |
| `crates/arc-storage/src/jobs/traffic/tests/admission.rs` | `16ff9e2b716a0c31061c913c92b16d046b8244189fbd3aeaf5f8664656c72111` |

## Raw logs

All raw logs are under `target/terminal-outcomes`; the SHA-256 values below identify their exact bytes. Earlier attempt logs remain referenced in the historical sections.

| Evidence | Path | SHA-256 |
| --- | --- | --- |
| RED: verified-child historical reason | `target/terminal-outcomes/attempt-7-red-storage.log` | `5d44b75857691876e43617d8c403e8c39eca1543a3125ecc65e77351d8d5055e` |
| RED: busy terminal summaries | `target/terminal-outcomes/attempt-7-red-busy.log` | `e26a2957c8f66557a4e65054634ad03ee347c580fe68bdab14661635f9710ffb` |
| Initial terminal suite: proxy race | `target/terminal-outcomes/attempt-7-terminal.log` | `c9ee6c54a523798ce8be9c7932a4243c0de4986e84c7cebb8e76c920f76e94ea` |
| Initial module split: missing import | `target/terminal-outcomes/attempt-7-split-terminal.log` | `eecdec7a9c36c06dc43b7b900eaa25857433263a7e861a7bed112353947da590` |
| Preservation validator failure | `target/terminal-outcomes/attempt-7-final-preservation.log` | `5c22f5e1ebcdd9a6b739b013212270c75532c308c3c3e6a107671e90bb2cf738` |
| Workspace binary build | `target/terminal-outcomes/attempt-7-verified-build.log` | `732186322402325172940531d8d6d7cfac85dc902585b2bef6715ad6839dacfd` |
| Terminal-outcome suite | `target/terminal-outcomes/attempt-7-verified-terminal.log` | `e2fb99d5d092911d2ca374259dfbb043e38fab53c37b8cedaf81e165d650f9e8` |
| Status compatibility suite | `target/terminal-outcomes/attempt-7-verified-status-compat.log` | `4cf2b49eb67d3074ef7abe05f40be448e175eab63a7cb6b04dfb23b26be337fc` |
| Storage tests | `target/terminal-outcomes/attempt-7-verified-storage.log` | `516cb2dc9503464ee3d206af1920cf2358a9f2ea93f26457772a3f33cbca8241` |
| CLI tests | `target/terminal-outcomes/attempt-7-verified-cli.log` | `2b32702079a10b8271599546542ccd454107d6872f4d6d0b9df999d25b347b51` |
| Managed-exec integration tests | `target/terminal-outcomes/attempt-7-managed-exec.log` | `e241538f9b7b459f979e854512e5280f54800caccd5cde9831c3114b9b6537a6` |
| Workspace Clippy | `target/terminal-outcomes/attempt-7-verified-clippy.log` | `379e208f31a739b7c442288a7c96c6722f28f2660c36598dd01488d05f4de04a` |
| Cargo formatting check | `target/terminal-outcomes/attempt-7-verified-format.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| Instrumented terminal-outcome tests | `target/terminal-outcomes/attempt-7-coverage-terminal.log` | `8a4757843d0c5afefb13386d18d6e5d413bd8b5ab2dbca9cc7ba3fd6c86eda83` |
| Rejected incompatible coverage flags | `target/terminal-outcomes/attempt-7-coverage-status-compat.log` | `654b3cd1e504e076f9fd79a2e7ab5a8240cbe4f9e6d543d6d83d8d764525da11` |
| Instrumented status compatibility tests | `target/terminal-outcomes/attempt-7-coverage-status-compat-retry.log` | `3216076f5ad6dd030c84c2e7bd9f2c5501a17a988ce30dd972a6e5d1a3025979` |
| HEAD, source and migration hashes | `target/terminal-outcomes/attempt-7-source-identity.log` | `c43d3293d99dbd04e254ddb7a0768c9fa2a956835378e7ebe16b047c7efc4ddf` |

| Source/log hash and evidence inventory check | `target/terminal-outcomes/attempt-7-final-audit.log` | `d72f425d1f261a04319fcc30c8aebc4dfded984d3e00f56a9a60954aee3f2da3` |

## Historical handoffs: attempts 1–6

Changed paths:

- `crates/arc-storage/src/jobs.rs`, `crates/arc-storage/src/jobs/codec.rs`, `crates/arc-storage/src/jobs/reconcile.rs`, `crates/arc-storage/src/jobs/tests.rs`, `crates/arc-storage/src/jobs/tests/terminal_outcomes.rs`, `crates/arc-storage/src/jobs/traffic/store.rs`, `crates/arc-storage/src/jobs/traffic/tests/admission.rs`
- `crates/arc-cli/src/job.rs`, `crates/arc-cli/src/exec/transport.rs`, `crates/arc-cli/src/exec/transport/live_tests.rs`, `crates/arc-cli/tests/fixtures/child.rs`
- `crates/arc-daemon/tests/traffic_cli.rs`, `crates/arc-daemon/tests/traffic_cli/terminal_outcomes.rs`, `crates/arc-daemon/tests/traffic_cli/status_compat.rs`, `crates/arc-daemon/tests/managed_exec/cli.rs`, `crates/arc-daemon/tests/managed_exec/retries.rs`
- `.arc-verify/terminal-outcomes/fix.md` (this report)

## Baseline and diagnosis

- `git rev-parse HEAD` returned `6b5218cf63b174b7976e52d81d56c7e3bf81112d`. The first attempt reported an empty staged diff and accessible scoped unstaged diff before its changes. Attempt 3 began with those task-1 changes already present. Global `git status --short` printed `av.yaml: Operation not permitted`, so the complete worktree state cannot be established.
- The configured Serena tools were not exposed. The affected paths named in the assignment and adjacent named files were read directly; no native project source search was substituted. This limits the completeness of impact discovery.
- Both `crates/arc-storage/src/jobs/transitions.rs` and `crates/arc-storage/src/jobs/traffic/lifecycle.rs` set protocol `Phase::Completed` for every normal exit. The shared codec formerly serialized that phase directly into `jobs.state`. Recent protocol-1 summaries omit exit codes; CLI rendering used `completed` for both codes. These source observations are the diagnostic evidence; a compiled RED reproduction was not authorized.

## Implementation and interfaces

- The storage codec writes `SUCCEEDED` only for an observed exit code 0 and `FAILED` for an observed nonzero exit, in the same transactions already used by both execution paths. New state/exit-kind/code mismatches are rejected. Signal and spawn-failure states remain separate; Traffic cancellation and unknown handling are unchanged.
- Protocol-1 phase values and payload shapes remain unchanged: a valid stored success or failure decodes to wire `COMPLETED` with its existing outcome. Recent summaries keep their 50-row bound; completed rows use the existing lifecycle by-ID request with one shared 250 ms deadline to obtain the actual outcome. Nonterminal rows retain existing Traffic status handling. By-ID status and recent status render recorded exits as `succeeded` or `failed`; unavailable outcomes are not guessed.
- Historical `COMPLETED` rows with a known exit code are decoded using that code, including rows without an outcome-kind marker. A historical row without an exit code is presented as outcome unknown. No migration or dependency was added or modified.
- For a historical `COMPLETED` row without an exit code and with either a missing kind or an `EXITED` marker, the protocol snapshot cannot carry its stored completion timestamp without violating protocol-1 validation. Reconciliation now checks the stored row inside its write transaction and returns the synthetic unknown snapshot without updating the row. The codec also rejects attempts to write a synthetic no-outcome snapshot over that historical row, including a delayed attachment. A lifecycle-only completion with a genuinely observed outcome remains eligible to be recorded; the Traffic completion path still rejects an already-completed Traffic entry. This preserves the original stored state, completion timestamp, outcome-kind marker, and other metadata for status reads.
- Traffic's closed protocol-1 `COMPLETED` response requires an outcome. For the two historical no-exit-code forms only, Traffic status returns the existing `UNSUPPORTED_CAPABILITY` error rather than retryable `DB_BUSY`. The current and accepted-HEAD CLI interpret that error for `STATUS_V2` as a signal to use the lifecycle status, which reports `OUTCOME_UNKNOWN`. Legacy `STATUS` receives the same existing protocol-1 error envelope. No successful response shape or wire enum is changed, and malformed new states still produce storage errors. `JobError::UnsupportedStatus` is a new public Rust enum variant within `arc-storage`; downstream Rust code with exhaustive matches must account for it. No client or daemon compatibility test has been executed in this attempt.
- The separate Traffic owner-reply path had the same missing-outcome assumption. For the same historical predicate, it now returns the existing invalid-transition error instead of `DB_BUSY`; it does not fabricate a completion or change reservation state. Storage regression source covers poll, cancel and fallback retries as well as status.
- Same-database downgrade limitation: the accepted HEAD daemon decodes `jobs.state` directly through the closed protocol `Phase` enum; it has no `SUCCEEDED` or `FAILED` variant. Thus an old daemon reading newly written terminal rows cannot decode those rows. This follows from the old codec and enum source; the downgrade has not been exercised with an old executable. Old-client/new-daemon and new-client/old-daemon source fixtures are included, but have not been executed.

## Regression sources

- `crates/arc-storage/src/jobs/tests/terminal_outcomes.rs` covers normal exit states, repeat completion, reopen, protocol-1 phase values, historical known and unavailable outcomes, mismatched new state/outcome pairs, signal and spawn-failure states, and the old phase decoder's rejection of the new database values.
- `crates/arc-daemon/tests/traffic_cli/terminal_outcomes.rs` is registered in `traffic_cli.rs`. The two named exit-17 tests each run the fixture once, require one launch marker, exact `fixture-stderr\n` and wrapper exit 17, restart the fixture-owned daemon, independently query SQLite for `FAILED`, exit code and outcome kind, and check both status views. Additional cases cover exit 0 on both paths, historical status, 50 recent rows, a delayed lookup sharing the deadline across rows, lost completion reply/retry and permit release.
- The historical storage regression now checks both missing-kind and `EXITED`-without-code rows, compares every `jobs` and `job_lifecycle` column before and after simulated reconciliation, and checks that a delayed attachment is rejected without changing the row. The macOS CLI regression compares those complete rows before and after by-ID status for both kinds, and checks the original completion timestamp and marker exist. The macOS test module now imports `arc_ipc::traffic::JobRequest`, fixing the reported unresolved name in source. None of these tests has been compiled or executed.
- Attempt 4 adds a Traffic storage regression for both historical no-code forms and rejection of an invalid new state, plus a scheduled CLI regression. The latter fixes registration ordering in the fixture so that both unknown rows precede known exit-17 and exit-0 rows, checks recent and by-ID labels, checks legacy and V2 Traffic error envelopes, and compares the entire historical rows before and after status calls. These regression tests are source evidence only.
- Attempt 5 adds a transport regression and two CLI proxy regressions in `traffic_cli/status_compat.rs`. One proxy reproduces the accepted-HEAD recent `COMPLETED` summary and `DB_BUSY` by-ID response for an outcome-less historical row, places a second busy Traffic row before known exit-17 and exit-0 rows, and checks one lookup per row and all four labels. The other proxy makes Traffic status unavailable while returning valid lifecycle results for exit 0, exit 17, signal, spawn failure and a nonterminal record; it checks the scheduling-unavailable message and each expected label. These are test sources, not executed evidence.
- Existing protocol-1 compatibility, retry and expected status tests were extended in `traffic_cli/status_compat.rs`, `managed_exec/retries.rs` and `managed_exec/cli.rs`. Existing fail-open, command-stream, redaction and cancellation tests were retained. These are test sources, not pass evidence.

## Earlier attempt checks

The following were recorded by the previous attempt. Only the checks listed again in the attempt 3 section were rechecked against the current bytes; the other entries are prior-attempt evidence, not fresh execution evidence.

| Command or check | Result |
| --- | --- |
| `uname -s`, `rustc --version`, `cargo --version`, `rustfmt --version` | Darwin; Rust/Cargo 1.95.0; rustfmt 1.9.0-stable. |
| `rustup show active-toolchain` | `1.95.0-aarch64-apple-darwin`, selected by `RUSTUP_TOOLCHAIN`; `rust-toolchain.toml` pins 1.95.0. |
| `cargo fetch --locked --offline` | PASS, exit 0; non-building offline dependency preparation. |
| `rustfmt --edition 2021 --config skip_children=true` on changed Rust files | PASS, exit 0; parsed and formatted those files. |
| `cargo fmt --all -- --check` | PASS, exit 0. |
| `git diff --check` over the owned crate paths | PASS, exit 0. |
| `git show HEAD:<migration> | shasum -a 256` compared with worktree `shasum -a 256` for all three migrations | PASS: each HEAD/worktree pair matched. An initial shell loop used zsh's special `path` variable and failed to find commands; the corrected `migration_file` loop passed. |
| `python3` in-memory SQLite check of the exact `RECENT_SQL` read from `reconcile.rs`, using migrations 0001 and 0002 | PASS on SQLite 3.53.1: one lifecycle row required 43 VM steps before and after 5,000 unrelated jobs; 60 lifecycle rows returned 51 candidates in 1,242 steps before and after 10,000 unrelated jobs. The query plan used the recent index and indexed job lookup. This is a SQL-only check, not a Rust or daemon test. |

The earlier attempt reported no raw logs. No `gates.json`, `audit.json`, or QA PASS artifact was written.

## Attempt 3 repair checks

| Command or check | Result |
| --- | --- |
| `git rev-parse HEAD` | `6b5218cf63b174b7976e52d81d56c7e3bf81112d`, the accepted baseline. |
| `git status --short` | Existing task-1 changes were present; Git again reported `av.yaml: Operation not permitted`. The global worktree state remains unverified. |
| `rustfmt --edition 2021 --config skip_children=true` on the four repair source/test files | PASS, exit 0; syntax parsed and files formatted. This is not type checking. |
| `cargo fmt --all -- --check` | PASS, exit 0 on current bytes. |
| `cargo fetch --locked --offline` | PASS, exit 0; no build or network download. |
| `rustup show active-toolchain`, `uname -s` | `1.95.0-aarch64-apple-darwin` selected by `RUSTUP_TOOLCHAIN`; Darwin. |
| `git diff --check -- crates/arc-cli crates/arc-daemon crates/arc-ipc crates/arc-storage` | PASS, exit 0 for tracked diffs. |
| Python SQLite predicate check using the exact SQL extracted from `codec.rs` | PASS, nine positive and negative cases including both historical forms, known exit codes, live unknown, signal, new states and an absent ID. This verifies only the SQL predicate, not the Rust call path. |
| `git show HEAD:<migration>` byte comparison for all three migration files | PASS; each matches HEAD. Hashes are recorded in the raw log. |

Raw check output: `target/terminal-outcomes/task-1-attempt-3-static.log`, SHA-256 `38c9d1cfade3683712e3f71920bb735e1474cbdfdf10c3f91efd2022529ea118`.

The attempt 3 inventory of `.arc-verify/terminal-outcomes` found one file, `fix.md`, at 10,528 bytes before adding this sentence. This is an inventory, not the independent mission evidence audit.

## Attempt 4 repair and checks

The previous review identified a second status path: `traffic::store::status` required an outcome for every completed Traffic entry. The codec intentionally decodes a historical `COMPLETED` row with no exit code as an unknown outcome; the Traffic path then converted that absence to `StorageFailure`, sent as `DB_BUSY`. The CLI's retry loop could spend the shared recent-list deadline on this single row and hide later known outcomes. The repair recognizes only the existing `legacy_unknown` predicate and uses the protocol's non-retryable unsupported response, leaving the lifecycle record and unrelated states unchanged. The analogous owner-reply branch now rejects a transition instead of presenting this row as a retryable database failure.

| Command or check | Result |
| --- | --- |
| `git rev-parse HEAD` and `git status --short` | HEAD is the accepted `6b5218cf63b174b7976e52d81d56c7e3bf81112d`; prior task-1 changes were present. Git again reported `av.yaml: Operation not permitted`, so the global worktree state remains unverified. |
| Python in-memory SQLite with all three migrations and the exact `legacy_unknown` predicate | Both `COMPLETED` Traffic row forms are accepted by the schema and match the predicate. Pre-fix source inspection showed both paths reached `outcome.ok_or(StorageFailure)`; this is not a Rust RED execution. |
| Python in-memory SQLite with all three migrations and the exact `RECENT_SQL` | Both historical unknown rows precede the known exit-17 and exit-0 rows; the exact historical predicate matches the unknown rows and not the known rows. Separate source checks trace the new unsupported response into the existing CLI fallback. This does not execute the Rust code. |
| `rustfmt --edition 2021 --config skip_children=true` on the four repair Rust files | PASS, exit 0; syntax parsed and files formatted, but not type-checked. |
| `cargo fmt --all -- --check` | PASS, exit 0 after the repair. |
| `cargo fetch --locked --offline` | PASS, exit 0; no build or network download. |
| `git diff --check` and `git diff --cached --check` on owned crate paths | PASS, exit 0. |
| HEAD/worktree byte comparison of all three migration files | PASS; SHA-256 hashes are in the check log. |
| Mission evidence inventory | One file under `.arc-verify/terminal-outcomes`, 10,775 bytes before this report update; not an independent evidence audit. |
| Final `cargo fmt --all -- --check`, scoped staged and unstaged `git diff --check`, and HEAD/worktree migration byte comparison after the owner-reply change | PASS, exit 0. Static checks found guards for the historical predicate in both reply paths and source assertions for poll, cancel and fallback. These do not execute or type-check Rust. |

Raw logs: `target/terminal-outcomes/task-1-attempt-4-reproduction.log` SHA-256 `d607641882bd6f471ca22cc8fdaedaf2f8683a0d3b55ea41718a095ee18d22bd`; `target/terminal-outcomes/task-1-attempt-4-static.log` SHA-256 `1cd4f2c5eab7247d4d5c58cda9cecd1259561f8a68eb70948bf3edeb9daae940`; `target/terminal-outcomes/task-1-attempt-4-checks.log` SHA-256 `d6f9fb3ca74d489ecb45deb9273ed7829cc3684294fa6370f303d07e9bc7e88b`; `target/terminal-outcomes/task-1-attempt-4-final-checks.log` SHA-256 `b327c46dd7239571836876d27291beab575d50358262a1ca18592665a71b593e`.

The final pre-handoff inventory found one mission evidence file, 15,911 bytes before this paragraph; all five referenced raw log hashes, including the attempt 3 log, matched. This is not the independent evidence audit.

## Attempt 5 repair and checks

The review found that the previous storage-side repair could not help an accepted-HEAD daemon: it still returns a `COMPLETED` recent summary and a retryable `DB_BUSY` for an old row without an exit code. The CLI's per-row lifecycle lookup retried that response until the one shared 250 ms recent-list deadline was gone. The same starvation was possible for a busy per-row Traffic lookup. A new crate-private read-only transport path returns on the first `DB_BUSY` for these recent-row requests. Both still use the original shared deadline, the 50-row limit is unchanged, and the ordinary transport path, detailed by-ID requests, and mutation retries retain the prior busy-retry behavior. Other exchange failures still use their existing bounded handling; a silent peer can exhaust the shared deadline and make remaining rows unknown.

The second finding was in the detailed status renderer: it replaced even a validated recorded terminal outcome with `unknown` whenever Traffic was unavailable. It now uses the lifecycle outcome for a validated exit, signal or spawn failure while still printing `Scheduling: status unavailable`. A lifecycle with no outcome keeps the existing `unknown` behavior. No IPC payload, protocol version, wire enum, migration, dependency, or externally visible Rust interface was changed in attempt 5. The earlier `JobError::UnsupportedStatus` public-variant change and same-database downgrade limitation still apply.

The causal evidence is source tracing and the new test cases. The Rust regressions could not be run under this assignment's build prohibition, so their RED/GREEN outcomes and runtime compatibility remain unverified. One patch application initially failed because `rustfmt` had changed the expected context; it was reapplied successfully after inspecting the file.

| Command or check | Result on the current source bytes |
| --- | --- |
| `git rev-parse HEAD`; `uname -s`; `rustup show active-toolchain` | Accepted `6b5218cf63b174b7976e52d81d56c7e3bf81112d`; Darwin; selected Rust `1.95.0-aarch64-apple-darwin`. |
| `rustfmt --edition 2021 --config skip_children=true` on `job.rs`, `transport.rs`, `transport/live_tests.rs`, and `traffic_cli/status_compat.rs` | PASS; the four files parse for formatting. This is not type checking. |
| `cargo fmt --all -- --check` | PASS, exit 0. |
| `cargo fetch --locked --offline` | PASS, exit 0; no build or network download. |
| Staged and unstaged `git diff --check` on owned crate paths | PASS, exit 0. Untracked source files are outside Git's diff check. |
| `git show HEAD:<migration>` compared byte-for-byte to each of the three worktree migration files | PASS; all match. |
| `git status --short --untracked-files=all` | Prior implementation changes and these repair files are present. `av.yaml: Operation not permitted` remains; full worktree state is not verified. |

Raw output: `target/terminal-outcomes/task-1-attempt-5-static.log`, SHA-256 `396f50571d48a181bcb6e47700c89684d7300ed25540d14dbd4e47aa9f31e839`.

A local inventory after the report update found one mission evidence file (`fix.md`, 20,216 bytes before this paragraph), currently untracked. All six raw logs referenced above were present under `target/terminal-outcomes` and matched their recorded hashes. This is an inventory, not the independent mission evidence audit.

## Attempt 6 restart-fixture repair

The review found that each second daemon used the first daemon's `runtime.json` path. The fixture sets that path for every spawn, and the daemon opens it with `O_CREAT | O_EXCL`; startup therefore fails while the first report remains in place. The required exit-17 and exit-0 restart cases and both historical restart cases could not reach their post-restart assertions.

`crates/arc-daemon/tests/traffic_cli/terminal_outcomes.rs` now uses a local `restart_daemon` helper at all three restart sites. It renames `runtime.json` to `prior-runtime.json` before starting the second fixture-owned daemon, matching the existing managed-exec restart pattern. Each first shutdown report is validated before the rename, including the historical lifecycle case; each second report is still validated after shutdown. Each helper invocation uses a fresh fixture directory and performs one restart. This repair changes no production code, protocol, schema, dependency, or public interface.

| Command or check | Result on attempt 6 bytes |
| --- | --- |
| `git rev-parse HEAD`; `uname -s`; `rustup show active-toolchain` | Accepted `6b5218cf63b174b7976e52d81d56c7e3bf81112d`; Darwin; `1.95.0-aarch64-apple-darwin`. |
| `cargo fetch --locked --offline` | PASS, exit 0; no build or network download. |
| `rustfmt --edition 2021 --config skip_children=true --check crates/arc-daemon/tests/traffic_cli/terminal_outcomes.rs` | PASS; formatting parses this file, but does not type-check it. |
| `cargo fmt --all -- --check` | PASS, exit 0. |
| Staged and unstaged `git diff --check` on the owned crate paths | PASS, exit 0 for tracked diffs. The untracked terminal-outcomes test was separately checked for trailing whitespace. |
| Comparison of all three migration files against `git show HEAD:<path>` | PASS, byte-for-byte. Hashes are in the raw log. |
| Python filesystem probe using the daemon report's exclusive-create flags | Reused `runtime.json` produced `EEXIST`; renaming it allowed a second exclusive create while preserving both reports. This probes filesystem behavior, not the Rust daemon. |
| Source-order check of all three restart paths | Each validates the first shutdown report before `restart_daemon` and validates the second report after shutdown. This is a source check, not executable Rust test evidence. |
| `git status --short --untracked-files=all` | Prior task-1 changes remain present. Git reports `av.yaml: Operation not permitted`; global worktree state is unverified. |

Raw output: `target/terminal-outcomes/task-1-attempt-6-static.log`, SHA-256 `f308a55b7c3c12f35eceaeff12b60eb060378362ebcb386da6284bdd50d8416a`. Before this report update, the mission evidence directory contained one untracked file, `fix.md`, totaling 20,711 bytes; that inventory is not the independent evidence audit.

After the repair report update, a local inventory still found one untracked mission evidence file below the file and byte limits. All seven raw logs referenced in this report were present under `target/terminal-outcomes` and matched their recorded SHA-256 values. Independent audit remains outstanding.

The daemon, CLI and fixture binaries were absent at `target/debug/arc-daemon`, `target/debug/arcctl` and `target/debug/arc-exec-fixture`. No Rust test or daemon was compiled or executed in this attempt. In particular, the restart persistence and status assertions, protocol compatibility, exit behavior, 50-row deadline and coverage under instrumentation are still unverified.

Task-1 handoff status: **partial**.

- [x] Source and regression cases are present for the assigned outcomes and restart paths; their compilation and behavior are not verified.
- [ ] Protocol compatibility, preserved execution semantics, and the other runtime acceptance criteria require executable checks. The documented same-database downgrade limitation remains.
- [x] This report records the root cause, available evidence, interface implications and unverified checks.

## Required follow-up and coordination

- The assigned build restriction prevents running `cargo test -p arc-storage historical_completed_without_exit_has_a_non_retryable_status`, `cargo test -p arc-storage terminal_outcomes`, `cargo test -p arc-daemon --test traffic_cli terminal_outcomes`, `cargo test -p arc-daemon --test traffic_cli status_compat`, `cargo test -p arc-daemon --test managed_exec`, `cargo test -p arc-cli`, `cargo test -p arc-ipc`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `bun run test:coverage`, and `./scripts/harness/validate.sh`: these compile or invoke compilation. They remain unverified, including both new proxy cases, the 80% per-crate floor and the healthy-daemon 50-row deadline under coverage. The full harness also runs coverage and includes fixture builds. Do not treat a source test as a passing gate.
- Replays of verification-contract commands must allow at least 7,200,000 ms per check, as specified by the assignment's `verificationCheckTimeoutMs`.
- Independent `oma-qa`, evidence-budget audit, and container target checks belong to the supervising mission and have not been performed by this task. The explicit build prerequisite must be resolved before the compilation-dependent checks can be run; any failure should be returned to the implementation owner.
- Ownership handoff: `docs/managed-exec.md` under “Status and cancellation” still describes normal exit status as `completed` and says Traffic unavailability always forces an unknown lifecycle state. It should describe `succeeded` and `failed`, and the availability of validated recorded terminal outcomes when Traffic status is unavailable. This task does not own that file. The old-daemon/same-database limitation above also needs an explicit release or operational decision by the Chief.

Skills used: `oma-debug` and `oma-backend`. The debug record is this assigned report rather than an additional out-of-scope `.agents/results/bugs/` artifact.
