# Technical Director task-1 resumption

Status: completed within task-1 ownership. Mission closure remains incomplete. Inspection and checks ran on 2026-11-07 UTC, native session `01a1b685-19d3-7e32-b9db-1498329b014e`, saved attempt 3. No OMA run ID or claim path was injected; this report uses the supplied task/attempt and native session without starting another mission or changing mission state.

## Result and ownership

The existing [runtime contract](../../../../docs/bounded-ipc-contract.md), [SRS target evidence](../../../../docs/bounded-ipc-targets.md), manifests and lockfile already implement task-1's deliverables. They were preserved. This attempt adds only this collected report, its [check script](verify.py), command receipts and source/access evidence in `.arc-verify/bounded-ipc/task-1/attempt-3/`, plus a run-scoped report under `.agents/results/`.

HEAD remains accepted `42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0`. Initial status contained the preceding Actors' integrated implementation, tests, docs and evidence. No reset, checkout, clean, source repair, manifest edit, service operation, download or browser action occurred. Before/after manifests prove all 53 checked files retained their bytes and modes: the 50 source files in the independent QA manifest, both task-1 documents and the unchanged harness. See [source-before](source-before.json), [source-after](source-after.json), [dirty boundary](dirty-before.stdout) and [checks](checks.json). This comparison is scoped to those files, not a claim of visibility into denied configuration.

## Contract and interface assessment

Known-path reads covered `limits.rs`, `admission.rs`, `connection.rs`, `transport.rs`, `db_worker.rs`, `lifecycle.rs`, main/server/platform, synchronous IPC dispatch, storage opening, CLI error mapping and the shared fixture lock. Existing runtime/state ADR material concerns Agent Valley orchestration and supplies no contrary ARC transport interface. No new architecture decision or Actor assignment was made.

The contract's default C/R/Q caps are 64/16/8; the fixed test profile is 8/4/2. Header/frame/write budgets are absolute 1000/2000/500 ms by default and 150/300/100 ms in tests. DB response/execution/busy budgets are 1500/1000/100 ms by default and 500/300/50 ms in tests. Internal shutdown uses one 3500 ms deadline below the external five-second limit. These values match `RuntimeLimits` in the current implementation.

The shared worker signatures remain compatible: `DbWorker::start(Connection, DbConfig, Arc<RuntimeMetrics>)`, `client`, `begin_shutdown`, deadline-bearing async `shutdown`, `DbClient::try_doctor(std::time::Instant)` and `DbTicket::receive`. Transport uses the documented `read_header`, `read_body` and `write_response` signatures. Test hooks retain their bounded started/release channels. The database implementation's extra queue-slot semaphore retains ownership through dequeue accounting; it changes no producer/consumer signature.

The selected tracked-Tokio-task/single-SQLite-thread approach still has the smallest maintenance surface among the compared options in the contract. A blocking connection pool would retain separate blocking-read cancellation and health contention; a DB pool/per-job blocking tasks would add cancellation paths without a required parallel SQL workload. No alternative implementation or duplicate helper is needed.

Source inspection confirms admission before handler creation/body allocation, request permits through response writes, JoinSet length accounting for completed/unreaped entries, nonwaiting DB reservation, Q queued plus one executing job, cancellation tokens retained until discard/completion, and capped serialization with one fallback under the original deadline. These are source observations; this attempt does not relabel them as measured RSS or performance. Hello/health bypass SQL; exhausting all connection/request slots remains distinct from DB saturation. `DB_BUSY` continues through the existing CLI fallback to exactly `IPC_ERROR`; predecode closure maps to `DAEMON_UNAVAILABLE`. Successful CLI output, synchronous protocol-1 APIs, same-user checks, startup lock ordering and storage API remain compatible.

The existing contract already specifies bounded fixed-label telemetry, body/response reservations, rejection work, test synchronization, absolute clocks and actual handler/worker shutdown accounting. The current implementation splits supervision and admission into focused modules without changing that external boundary. All future manifest/lock edits still return to the assigned Technical Director writer; this report authorizes no ownership handoff.

## Dependencies and preservation

[Preservation evidence](preservation.json) compares actual files with `git show` of accepted HEAD. `0001_phase0.sql` is byte-identical with SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. The harness is byte-identical with SHA-256 `93140173df8b8bd28b067aff68301134b0398b643240b0b1e6a13da4487ebe33`.

The retained manifest diff is minimal: workspace Tokio adds `sync`/`time`; daemon rusqlite adds `hooks`, daemon serde_json becomes a direct edge, and tests add Tokio `test-util`. Native metadata resolves Tokio 1.54.1 and rusqlite 0.41.1; it includes the required features without `io-util`. No package was added/removed or changed version/source/checksum. The only lock entry difference is the daemon's edge to already-present serde_json. Native offline resolution and fetch succeed; no package installation or network fetch was performed. The original task-1 report retains the earlier all-platform cache failure for existing Windows `anstyle-wincon`; this attempt does not repeat it or claim Windows prerequisites ready.

The ARC SRS remains at `/Users/gracefullight/Downloads/Agent_Resource_Control_ARC_Requirements_SRS_2026-11-06.md`, SHA-256 `a8ef945251f9f3fd7c3e337d4efc77ff9914f95114212ca9e6c90ae2ce3e3d9e`. This attempt rehashed it and read §18 and Phase 0 in place. No copy was made. NFR-002 line 2426 targets idle RSS below 50 MB; NFR-004 line 2428 targets local small-message IPC round-trip P95 within 20 ms. The retained target document distinguishes idle RSS from native lifetime peak, task bounds from measurements, and mission measurement conventions from unspecified SRS workload conditions. Internal draft version labels remain inconsistent as disclosed there. No benchmark was rerun and no performance target achievement is newly claimed by task-1.

## Executed checks and retained gate evidence

`python3 .arc-verify/bounded-ipc/task-1/attempt-3/verify.py` exited 0. It ran the following serially, each with a 120-second process timeout; raw stdout/stderr and timestamped argv/exit/hash receipts are collected beside this report.

| Command/check | Outcome |
| --- | --- |
| `git rev-parse HEAD`, `git status --short`, scoped manifest `git diff` | Exit 0; accepted HEAD and preserved dirty boundary; status reports denied `av.yaml` |
| `cargo --version`, `rustc --version`, `rustup component list --installed` | Exit 0; installed 1.95.0 toolchain, native std/rustfmt/Clippy |
| `cargo metadata --offline --locked --filter-platform aarch64-apple-darwin --format-version 1` | Exit 0; five members, pinned native dependency/features checked |
| `cargo fetch --offline --locked --target aarch64-apple-darwin` | Exit 0; existing native cache sufficient |
| `cargo fmt --all -- --check` | Exit 0 |
| `git diff --check -- Cargo.toml Cargo.lock crates docs` | Exit 0 |
| Python byte/mode/lock/SRS assertions | Pass; 53 checked files unchanged, migration/harness identical, no package upgrades |
| Direct `Path('av.yaml').read_text()` | Failed with `PermissionError`, errno 1; failure preserved and caught for reporting |
| Three task-6 receipt existence checks | All three absent; exact paths below |

[Inherited gate binding checks](inherited-gates.json) rehashed each retained QA log and matched every file/hash/mode in its 50-file source manifest to the current checkout. Those earlier receipts record the exact fmt, Clippy and workspace-test commands exiting 0, and `./scripts/harness/validate.sh` exiting 1. This is receipt-to-current-byte verification, not fresh execution of Clippy, workspace tests or the harness in task-1. No product changes justify repeating the already source-bound runtime gate workload during this contract-only resumption. Final integration retains all exact required commands; no test, threshold, command or acceptance criterion was weakened.

The retained unchanged harness has three unresolved failures: `crates/arc-daemon/tests/resident.rs` has 901 lines, `crates/arc-ipc/src/lib.rs` has 551 lines, and `test:coverage` is missing. This task does not claim harness conformance or introduce JavaScript tooling to change that result.

## Access limit and Chief handoff

At `2026-11-07T15:01:15.167558+00:00`, the supported direct `av.yaml` read returned `PermissionError: [Errno 1] Operation not permitted`. See [configuration-access.json](configuration-access.json). The commented example at `av.example.yaml:198–209` is not actual configured-target evidence. No supported configuration service tool is exposed in this run. Target existence, health, availability and recent-error observations remain unestablished. No alternate path, privilege change, GUI workaround, service restart or invented target was used. The Chief must obtain supported configuration evidence from the configuration owner/harness and route observations for any configured targets before closing the conditional mission prerequisite. This task's explicit option to record an actual access limitation is satisfied; the broader final-review prerequisite remains unresolved.

The final-review documentation finding is also reproduced. These paths linked by `docs/bounded-ipc-recovery.md:74,88,91` are absent:

- `.agents/results/task-6-design/workspace-test.json`
- `.agents/results/task-6-design/workspace-test.txt`
- `.agents/results/task-6-design/checks.json`

See [outside-ownership.json](outside-ownership.json). Repair belongs to the documentation Actor's files and receipts, outside task-1 ownership. The Chief should route original-receipt recovery or explicitly superseding source-bound verification to that Actor. This attempt neither fabricates historical receipts nor edits its recovery document, unsupported claims, saved task or mission graph. No shared-interface conflict was found; these are evidence/access blockers for mission closure.

## Acceptance, limits and next checks

All five task-1 acceptance items have current evidence: implementable finite contracts, compatible worker/transport signatures and ownership, hashed ARC SRS targets, minimal reconciled dependencies with frozen migration, and this material technical report with access limits and check plan. Existing owned deliverables were reused rather than rewritten. The task-1 document/manifest bytes and other Actors' product/test work are preserved.

Skills used: `oma-architecture` for the option/interface/ownership assessment and `oma-backend` for runtime/auth/CLI/dependency boundaries. Their protocols and applicable checklists were read. REST/JWT/OpenAPI/ORM defaults do not apply to this local same-user IPC service. No subagent, workflow or AV lifecycle command was started. `oma-db` execution and independent `oma-qa` remain the respective assigned Actors' evidence, not work newly claimed by this task.

For any subsequent implementation repair, run its focused deterministic units first, then the existing serialized subprocess and CLI fixtures, the 100,000-attempt bounds cases and accounted SIGINT/SIGTERM/reopen checks. Repeat full Cargo gates only for changed inputs or unresolved failures; retain the unchanged harness result and all outstanding failures. Configured-target verification still requires real timestamped observations once supported target evidence exists. The Phase 0 desktop/tray/exec/hooks/Managed Job obligations, Linux qualification, Windows Named Pipe/runtime and platform measurements remain open; this Unix contract task does not complete them.
