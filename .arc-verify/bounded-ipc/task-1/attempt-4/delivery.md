# Task-1 runtime-contract handoff, attempt 4

Status: completed within task-1 ownership. Mission completion remains unclaimed. Checks recorded at 2026-11-07 16:19 UTC on Darwin 27.2.0 arm64.

Retain `docs/bounded-ipc-contract.md`, `docs/bounded-ipc-targets.md` and the existing dependency changes. The assigned contract work already exists and matches the inspected integrated interfaces. Replacing it would repeat accepted work and invalidate downstream evidence. This attempt creates only a report, collection index and check receipts; it changes no product source, manifests, lockfile, preceding reports, saved tasks or mission state.

## Assignment and existing changes

HEAD remains accepted baseline `42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0`. The worktree already contains the transport, worker, tests, documentation and evidence from preceding Actors. `dirty-before.stdout` records those boundaries; Git also reports denied access to `av.yaml`, so the status output cannot establish its contents. No reset, staging, commit or ownership reassignment occurred.

The Technical Director remains the sole manifest/lockfile writer for this wave. There is no new dependency request or file-ownership handoff. Later shared-manifest changes must return to this writer through an actual artifact dependency. The missing task-6 report belongs to the Design Director; its collection manifest has not been edited.

Skills used: **oma-architecture** for the bounded-runtime interface and cost comparison; **oma-backend** for transport, admission, dependency and lifecycle compatibility. Read their execution protocols/checklists, shared execution/result contracts and the always-applicable language rule. REST/JWT/ORM/cloud checklist defaults do not apply to this existing same-user Unix IPC service. The separate SQLite implementation and independent QA remain attributed to their assigned Actors.

Serena is configured in `.mcp.json`, but no Serena tools were exposed. Inspection used known-path source reads, scoped Git diffs and plain-documentation/artifact reads. No recursive native source search or provider installation was used. Prior decision documentation concerns the surrounding Agent Valley system; it supplies no conflicting ARC runtime contract. No new architecture decision or diagram was needed on resumption.

## Acceptance evidence

| Requirement | Delivered evidence |
| --- | --- |
| Finite limits, deadlines, permit lifetimes, bounded buffers/logging and shutdown accounting | Existing `docs/bounded-ipc-contract.md`; inspected `limits.rs`, `admission.rs`, `connection.rs`, `transport.rs`, `runtime_metrics.rs`, `lifecycle.rs`, `main.rs` and `platform.rs` |
| Independently implementable producer/consumer signatures and ownership | Contract's exact `DbConfig`, `DbError`, `DbWorker`, `DbClient`, `DbTicket`, transport functions, metrics methods and test hooks; inspected worker/transport signatures match; no interface change this attempt |
| Authoritative ARC targets with conditions | Existing targets document and fresh `srs-source.json`; original SRS hash revalidated in place, without copying it |
| Minimal reconciled dependencies and frozen migration | Fresh `preservation.json`, `manifest-diff.stdout`, offline metadata/fetch receipts and source snapshots |
| Material technical report, limitations and intermediate checks | This report, `checks.json`, `configuration-access.json`, `outside-ownership.json`, command receipts and receiving-collection checker |

The contract compares fixed blocking workers, tracked Tokio handlers with one SQLite worker, and database pools/per-request blocking tasks. The selected option reuses the existing stack with one explicit database cancellation/join boundary. Fixed workers tie health capacity to stalled reads; pools add connections, queues and cancellation paths without a demonstrated need. Cost/maintenance comparisons are qualitative; this task measures neither savings nor performance improvements.

Default caps remain C=64 connection/retained handler entries, R=16 requests, Q=8 queued database jobs plus one executing job. Test caps remain 8/4/2. Admission precedes handler creation and body allocation; overload closes before decode or returns protocol-1 `DB_BUSY` for decoded doctor requests. The existing CLI maps these paths to `DAEMON_UNAVAILABLE` and `IPC_ERROR` respectively. Hello/health bypass SQLite; complete exhaustion of connection/request slots is separately bounded transport saturation.

Default absolute budgets remain 1,000 ms header, 2,000 ms whole frame, 500 ms serialization/write, 1,500 ms database response, 1,000 ms database execution and 100 ms busy wait. One 3,500 ms shutdown deadline covers cancellation, handler reaping, database close/join and cleanup, within the five-second external requirement. The contract accounts for completed-but-unreaped tasks, reserved/queued/executing database work, bodies, response budgets and rejection work. Structural arithmetic remains separate from native measurements.

Current dependencies retain all cached package versions and checksums: Tokio 1.54.1 and rusqlite 0.41.1. The workspace adds only Tokio `sync`/`time`; daemon tests add `test-util`; daemon rusqlite adds `hooks`; daemon serialization uses the already-locked `serde_json`. No package is added or removed. The sole lockfile change is the daemon's dependency edge to existing serde_json. No `full`, `io-util`, `bytes`, pool, ORM or extra runtime was introduced.

The frozen migration remains byte-identical to accepted HEAD, SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. The unchanged harness also matches accepted HEAD bytes. Before/after snapshots preserve the hashes and modes of 57 checked files, including all 50 files in the latest independent QA product snapshot and the assigned contract/targets documents. See `source-before.json`, `source-after.json` and `preservation.json`.

## Targets and verification boundary

The original local ARC SRS remains `/Users/gracefullight/Downloads/Agent_Resource_Control_ARC_Requirements_SRS_2026-11-06.md`, SHA-256 `a8ef945251f9f3fd7c3e337d4efc77ff9914f95114212ca9e6c90ae2ce3e3d9e`. Fresh checks confirm §18 line 2426, NFR-002: daemon **idle RSS below 50 MB**, platform-measured; and line 2428, NFR-004: local small-message round-trip **P95 within 20 ms**. The targets document retains the source's inconsistent draft version labels and the measurement conventions it does not define. Agent Valley webhook targets are not substituted. This attempt performs no benchmark or native RSS/thread measurement.

The latest independent QA receipts bind to all 50 current product files, fingerprint `b3610549adfebffdd626a760741a200c848ec7ee26a88482bcf7e38e563383c4`. Fresh hash checks match their outputs and source bindings. `inherited-gates.json` explicitly marks each receipt as historical and not replayed:

| Original command | Original result |
| --- | --- |
| `cargo fmt --all -- --check` | Exit 0, 2026-11-07 16:02 UTC |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0 |
| `cargo test --workspace` | Exit 0; QA reports 173 passing invocations |
| `./scripts/harness/validate.sh` | Exit 1; unresolved failures retained |

These receipts live under `.arc-verify/bounded-ipc/qa-review/attempt-2/`. They are source-matched evidence of the original deterministic unit, subprocess and CLI runs, not newly executed test results. Repeating their runtime workloads is unnecessary for this unchanged contract/dependency resumption. Required mission verification commands remain unchanged.

## Commands executed in this attempt

`python3 -B .arc-verify/bounded-ipc/task-1/attempt-4/verify.py` exited 0. It recorded these serial subprocess checks, with a 120-second per-process timeout:

| Command | Result |
| --- | --- |
| `git rev-parse HEAD` | Exit 0; accepted baseline |
| `git status --short` | Exit 0; existing changes retained; denied `av.yaml` reported |
| `git diff <accepted HEAD> -- Cargo.toml Cargo.lock crates/arc-daemon/Cargo.toml` | Exit 0; minimal inherited dependency diff |
| `cargo --version`; `rustc --version` | Both exit 0; 1.95.0 |
| `rustup component list --installed` | Exit 0; native Rust, Clippy and rustfmt already installed |
| `cargo metadata --offline --locked --filter-platform aarch64-apple-darwin --format-version 1` | Exit 0; five-crate native resolution and expected features |
| `cargo fetch --offline --locked --target aarch64-apple-darwin` | Exit 0; existing native cache sufficient; no network/download |
| `cargo fmt --all -- --check` | Exit 0 |
| `git diff --check -- Cargo.toml Cargo.lock crates docs` | Exit 0 |

The script also reads accepted-HEAD migration/harness/lockfile blobs with `git show` and compares exact bytes/package records. Native metadata, dependency-cache and formatting checks together took less than one second. No build, service restart, delegated agent, GUI, browser, Playwright or external delivery was used. Inherited mission duration/run/retry allocations were not reset; the original absolute start/deadline was not injected. This attempt admits only the short serial checks above.

Failed inspection: a guessed `.arc-verify/bounded-ipc/qa-review/attempt-2/source-manifest.json` read returned “No such file.” Inspection continued with the actual `workspace-source.json`. Shell startup also reports a denied mise tracking symlink outside the worktree; no toolchain repair was attempted and the Cargo checks passed.

## Findings returned to the Chief

1. **Configured targets remain unknown.** Fresh supported `Path.read_text` of `av.yaml` at 2026-11-07 16:19:11 UTC failed with `PermissionError`, errno 1, `Operation not permitted`. See `configuration-access.json`. No alternate route or permission workaround was attempted. This satisfies task-1's requirement to record the actual access limitation, but cannot satisfy mission-wide configured-target applicability. The Chief must obtain supported configuration evidence and, if targets exist, assign authorized timestamped health, availability and recent-error observations. No service was restarted; subprocess fixtures do not prove deployed-target health.
2. **Task-6 collection remains outside this ownership.** `.arc-verify/bounded-ipc/task-6/attempt-3/delivery.json` indexes the absent `.agents/results/result-design-director-task-6-attempt-3-local.md`, exactly 6,043 bytes, SHA-256 `899b1bcff07a2ac60f4c389c71884cfd65af52bbf4e3f20c8cf6cdd55a7ddecb`. See `outside-ownership.json`. No handoff authorizes this Actor to restore or recollect that report. The Design Director must deliver the exact bytes through the supported artifact mechanism and obtain the requested receiving-checkout review; the Chief coordinates it.
3. **Three harness failures remain:** `crates/arc-daemon/tests/resident.rs` exceeds 500 lines; `crates/arc-ipc/src/lib.rs` exceeds 500 lines; the unchanged coverage gate invokes missing `test:coverage`. This assignment does not alter those paths or the verifier. Retain the actual failure rather than introducing dummy JavaScript tooling or exceptions.
4. **Remaining qualification:** Phase 0 features, Linux native runtime and Windows Named Pipe/tray/runtime obligations remain incomplete. This report supplies no platform-wide, deployed-service or full mission-completion claim. Native filesystem stalls remain outside the deterministic shutdown fixtures' coverage.

Recommendation: accept the preserved task-1 contract/dependency handoff and coordinate the two final-review findings with their owners. Reopening the bounded transport or adding dependencies would not address either finding.

## Produced artifacts

The new files are confined to `.arc-verify/bounded-ipc/task-1/attempt-4/` and `.agents/results/result-technical-director-task-1-attempt-4-local.md`. `delivery.json` lists exact hashes/sizes of this attempt's evidence plus the retained assigned contract/dependency deliverables. `check_delivery.py` verifies every indexed entry from the receiving checkout without executing Cargo or generating new receipts. The run-scoped report has an identical collected copy at `delivery.md`; both are indexed. All preceding reports and source bytes are preserved.
