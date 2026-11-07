# Classification Engineer handoff — task-4

Status: **partial**. The detector, IPC operation, CLI and deterministic regressions are implemented. Final fmt and clippy pass. The exact workspace-test gate fails because this environment denies the mandatory loopback TCP monitor. The unchanged harness exits 1 because `test:coverage` is missing. The Chief also needs to reconcile conflicting human-output and empty/input-limit requirements in the two input artifacts. No successful mission acceptance is claimed.

Skill used: **oma-backend**. Read the local execution policy, backend skill protocol/checklist, backend rule, language rule and result contract. Used the existing Rust 2021 workspace with Serde, Clap, Tokio and SQLite. No new registry dependency, parser service, browser, GUI, Playwright package, standalone build, deployment or external delivery was introduced. The assigned mission was implemented directly without AV delegation or mission-state changes.

## Delivered behavior

`arc-detector` owns closed request/result types and a pure, bounded quote-aware scanner/parser. It distinguishes Cargo build/test/clippy/fmt, GitHub workflow/run syntax, SSH remote execution, forwarding ambiguity, mixed local/remote compounds, quoted arguments, unsupported syntax and conservative unknowns. Groups and literal shell wrappers share counters and effective leaves. No analyzed command, nested shell, substitution, script or endpoint is executed or probed. Analysis has no process, filesystem, environment, logging, storage or socket API.

`arc-ipc` re-exports the types and adds protocol-1 `job.classify` without changing `dispatch`'s public signature. Version mismatch still precedes payload validation. Unknown/missing/wrong payload fields get fixed INVALID_REQUEST. Classification cannot call the doctor closure. The daemon classifies before clearing payload, then drops command data before response awaits; database and runtime reporting paths remain unchanged.

`arcctl classify [--shell <kind>] [--json] -- <command>` sends exactly one literal UTF-8 argument. It performs one local IPC exchange, validates protocol, correlation, status, closed fields and semantic combinations, then prints enum labels or four-field JSON. Errors and recovery text are fixed and input-free. Existing status/doctor output and usage/error paths are preserved. Human classification follows task-1's exact five-line format.

## Acceptance evidence

- [x] 45 delivered contract cases run through detector, IPC dispatch, actual daemon, human CLI and JSON CLI. Cargo families have distinct reasons; gh workflow/run operations are REMOTE_CI; SSH commands are REMOTE_EXEC; resolved local/remote compounds are MIXED; unresolved scripts/expansions/Docker remain UNKNOWN/OBSERVE.
- [x] Inclusive/exceeded 16384-byte UTF-8, 1024-token, 128-invocation and depth-8 bounds are isolated; nested wrappers/groups share counters. Work helper consumes exactly 65536 units, refuses the next operation and rejects overflow. The production exhaustion generator has 12033 bytes and reaches WORK_LIMIT with work=60203, tokens=6, invocations=2, depth=1 before the next bulk charge. It is below every other limit. This trace's detector inputs match final source bytes.
- [x] Twelve Unix-monitor privacy families show zero PATH-trap starts, zero substitution markers, zero endpoint connections and zero persisted synthetic canaries. Captured responses, both CLI formats, daemon stdout/stderr, database main/live WAL/SHM and runtime artifacts are scanned before/after classification and shutdown. Only identifiers, counters and outcomes are retained in evidence.
- [ ] Required **TCP** monitor privacy proof: `crates/arc-daemon/tests/classification/privacy.rs:95` fails at `TcpListener::bind("127.0.0.1:0")`, errno 1, `Operation not permitted`. The mandatory test is retained without ignore, capability skip or changed expectation. Unix evidence and source inspection do not replace it.
- [x] Existing protocol, status, doctor, same-user authentication, admission/deadline, health and shutdown regressions pass in the complete no-fail-fast run. The 100000-attempt test passes. Maximum/work-exhausting classification saturation retains default 64 connections, 16 requests and DB queue 8, with database high-water use zero, successful health and signal-to-exit under the existing 3500 ms limit.
- [x] All 63 resulting Rust files, including untracked additions, are within 500 lines; maximum is 474. Migration and harness bytes equal baseline.
- [x] Exact fmt and clippy commands pass on final sources.
- [ ] Exact workspace-test command passes: it exits 101 solely at the required TCP monitor test. The complete no-fail-fast run reports **202 passed, 1 failed, 0 ignored**.
- [x] Unchanged harness run retained: exit 1, missing `test:coverage`, coverage unresolved. No dummy task or coverage skip was added.
- [ ] Independent oma-qa review: assigned to the separate reviewer; not performed or represented as complete by this implementation Actor.

## Final commands and results

| Command | Exit | Receipt/log stem |
| --- | ---: | --- |
| `cargo fmt --all -- --check` | 0 | verified-fmt |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | verified-clippy |
| `cargo test --workspace --no-fail-fast` | 101 | verified-all-tests; all targets exercised, only TCP prerequisite failed |
| `cargo test --workspace` | 101 | verified-workspace-tests; exact operator gate, no skip/extra flags |
| `./scripts/harness/validate.sh` | 1 | final-harness; exact unchanged command, test:coverage unresolved |
| `python3 .../implementation/inventory.py` | 0 | source-inventory.json; byte hashes, lines, baseline invariants and contract digest checked |
| `python3 .../implementation/finalize.py` | 0 | summary.json; final receipts/log digests match current source inventory |
| `git diff --exit-code 05f0812d0841485bbe07b3f1f8f6fde3861a1d55 -- crates/arc-storage/migrations/0001_phase0.sql scripts/harness/validate.sh` | 0 | direct baseline comparison, also reproduced by inventory.py |

Every final Cargo/harness receipt records actual argv, exit, elapsed time, complete captured log digest, before/after source hashes and `sourcesMatch=true`. Finalize verifies those maps against the inventory. Cargo ran with CARGO_NET_OFFLINE=true; harness coverage was not skipped. No monetary/resource savings or comparative performance result is inferred.

## Preparation and failed attempts

HEAD is unchanged at required baseline `05f0812d0841485bbe07b3f1f8f6fde3861a1d55`. Initial scoped source status was clean; preceding contract/CLI/value outputs were preserved. Serena was unavailable, so only known-path source reads were used. Rust/Cargo 1.95.0, rustfmt and clippy were installed. Native-target offline fetch succeeded after the additive lockfile update. An earlier native fetch with --locked failed because the lockfile needed updating; its receipt is retained. Initial all-platform offline fetch reported uncached Windows anstyle-wincon 3.0.11; its standalone exit was not captured. Native final Cargo commands need no download. Mise repeatedly warned that it could not write its external tracked-config symlink; no repair or GUI workaround was attempted.

Intermediate detector failures were fixture problems: single-quote nesting grew beyond the byte cap, and an exhaustion test incorrectly expected a bulk refusal to consume unused budget. Corrected generators preserve production caps and exact helper-boundary assertions. Initial integration assertions used a nonexistent database counter; they now assert existing DB gauges. A full run exposed a fake-peer race from an inherited nonblocking accepted socket; blocking mode plus the unchanged finite timeouts repaired it, and the final full run passes every fake-peer case. Failed/passing logs remain in this directory. Earlier `workspace-all-targets`/`gate-harness` runs have source changes during execution and are not final-source receipts. The focused `--skip required_loopback...` command was diagnostic only; no acceptance uses that skip. No TDD-first claim is made.

## Interface conflicts for Chief

Actual input artifacts were consumed and preserved. `contract.md` requires human `Classification:` with exactly five lines; `cli-requirements.md` requires `Class:` and an extra UNKNOWN explanation. Contract EMPTY_COMMAND/INPUT_LIMIT observations conflict with task-2's empty/whitespace rejection and size-error expectations. Implementation follows the concrete task-1 contract on these points and applies task-2's compatible quoting, help, JSON, error sanitization/recovery and compatibility requirements. Neither saved instructions nor acceptance criteria were edited. This is an unresolved input-interface conflict, not evidence that both contradictory formats pass.

## Scope and remaining risks

The required loopback TCP permission is an environment prerequisite. Chief/reviewer must obtain permitted execution of the retained test rather than weaken it. The missing harness coverage task remains outside this slice and unresolved. The two input-format conflicts need Chief disposition. Independent verification/reporting remains with the assigned reviewer; no reviewer-owned, final-report or mission-state files were written.

This is a restricted detector foundation, not the full detector catalog. Scheduling, managed-execution, desktop and native Windows acceptance remain open. NONE means syntax resolved within the subset, not verified topology, executable identity or execution permission. Dropping transient input is not secure memory erasure. Tests start only owned daemon/client/listener fixtures, with retained-child RAII cleanup and joined load threads; there is no intended surviving daemon or monitor.

Changed product paths are listed below and in summary.json. All other produced artifacts stay under this implementation directory: full logs/receipts, native observation JSON, preparation/conflict record, requirement map, static inspection, reproducible inventory/finalization scripts and final summary. No preceding Actor artifact was edited.

- `Cargo.lock`
- `Cargo.toml`
- `crates/arc-cli/Cargo.toml`
- `crates/arc-cli/src/classify.rs`
- `crates/arc-cli/src/main.rs`
- `crates/arc-cli/tests/classification.rs`
- `crates/arc-daemon/src/connection.rs`
- `crates/arc-daemon/tests/classification.rs`
- `crates/arc-daemon/tests/classification/boundaries.rs`
- `crates/arc-daemon/tests/classification/privacy.rs`
- `crates/arc-daemon/tests/classification/saturation.rs`
- `crates/arc-detector/Cargo.toml`
- `crates/arc-detector/src/budget.rs`
- `crates/arc-detector/src/lexer.rs`
- `crates/arc-detector/src/lib.rs`
- `crates/arc-detector/src/parser.rs`
- `crates/arc-detector/src/rules.rs`
- `crates/arc-detector/src/tests.rs`
- `crates/arc-detector/src/types.rs`
- `crates/arc-ipc/Cargo.toml`
- `crates/arc-ipc/src/lib.rs`
- `crates/arc-ipc/tests/classification.rs`
