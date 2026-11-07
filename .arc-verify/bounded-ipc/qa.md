# Independent bounded IPC review

## Review Result: NOT_ASSESSED

Verification completeness: partial. The current macOS bounded-runtime, compatibility and performance checks passed. Three MEDIUM harness conformance findings remain, and configured-target applicability is unverified because the supported read of av.yaml is denied. No CRITICAL or HIGH runtime defect was confirmed. This report does not certify mission completion.

Actor reviewer, task-8, attempt 3; skill oma-qa. Read the execution policy, result contract, QA execution protocol and checklist. Existing changes were inspected first. No product code, tests, manifest, saved assignment, acceptance criterion, mission state or framework task review was edited. No delegation, browser work, installation, download or standalone build occurred. Serena is absent from the exposed tools; review used known-path reads and scoped Git diffs. No run ID or claim path was injected, so these assigned files and associated receipts are the material handoff.

## Evidence boundary

HEAD remains accepted `42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0`. The implementation is an uncommitted overlay. All **51** crate source/test/manifest/toolchain files fingerprint to `9e3335b98f73d9ef964809c8fbb08fb1530661adb1d6e2c1adac492d28543970`. The sorted path/hash/mode records are in [context](qa-review/attempt-3/context.json) and each command's source manifest. Exact command receipts bind identical before/after bytes. Prior task-8 top-level summaries were copied to preceding-* in attempt-3 before replacement; prior receipts and framework reviews remain at their original paths. This run supersedes the older 50-file QA execution without rewriting its history.

## Findings and remediation ownership

### CRITICAL

None confirmed.

### HIGH

None confirmed.

### MEDIUM

- `crates/arc-daemon/tests/resident.rs:501`: 901 lines exceeds the unchanged harness's 500-line cap. Chief Director / Runtime Test Engineer must coordinate a split into focused integration modules, preserving support::serial, fixture lifetimes and all assertions. Retain the original check and cap. QA has no test-edit ownership.
- `crates/arc-ipc/src/lib.rs:501`: 551 lines exceeds the same cap (accepted HEAD: 549). Chief Director / IPC Engineer own a behavior-preserving split. Concrete structure: `#[cfg(test)] mod tests;` in lib.rs, moving the existing test body to src/tests.rs while retaining the public wire/framing API.
- `scripts/harness/validate.sh:420`: the unchanged command runs missing `test:coverage` and exits 1. Chief Director / harness owner must resolve the Rust-workspace/inherited coverage contract outside task-8. Preserve this failure; do not add dummy coverage, skip flags, alternate verification commands or JavaScript tooling to manufacture conformance. No honest task-8 code patch can resolve that ownership conflict.

### LOW

None confirmed.

## Exact executed gates

Commands ran serially in this mission worktree with CARGO_NET_OFFLINE=true and prepared caches. No argv was changed. Raw receipts retain argv/cwd, timestamps, PID/process group, exit, timeout state, output hashes and source fingerprints. No command timed out. Saved limits were preserved: deadline 2026-11-08T10:52:35.601Z, duration 86,400 seconds, 200 runs, three retries, maximum three concurrent workspace tasks. This review used one check at a time, with a 120-second command timeout and bounded owned-process termination. Host resource limits and deadline provenance are recorded in context.json. No cancellation was received and no mission state was changed.

| Command | Exit | Elapsed | Evidence |
| --- | --- | --- | --- |
| `cargo fmt --all -- --check` | 0 | 0.492 s | [receipt](qa-review/attempt-3/fmt.json) / [output](qa-review/attempt-3/fmt.txt) |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 0.442 s | [receipt](qa-review/attempt-3/clippy.json) / [output](qa-review/attempt-3/clippy.txt) |
| `cargo test --workspace` | 0 | 29.297 s | [receipt](qa-review/attempt-3/workspace.json) / [output](qa-review/attempt-3/workspace.txt) |
| `./scripts/harness/validate.sh` | 1 | 41.270 s | [receipt](qa-review/attempt-3/harness.json) / [output](qa-review/attempt-3/harness.txt) |

Workspace tests: **183 passing invocations**, zero failed/ignored/measured/filtered. These include imported production unit modules repeated across test targets and child-entry functions that return in parent execution; 183 is not a count of unique scenarios. Parent scenarios do launch their controlled subprocesses. The exact workspace command independently executed the delivered native probe/workload and all required signal/CLI cases. No extra runtime replay was needed after passing checks.

The unchanged harness ran in all-tracked mode over 1,337 entries and exited **1**, retaining exactly the three findings above. New untracked Rust modules are outside its tracked inventory but inside the independent 51-file source fingerprint, Clippy and test run. No coverage skip was supplied. Its secret/layer/shell checks pass within that inventory; no comprehensive dependency-vulnerability or license audit is claimed.

## Race, security and lifecycle audit

`admission.rs:36`: one supervisor reaps JoinSet entries before admission, authenticates the peer, checks retained length and acquires the permit before invoking the handler factory. Completed but unreaped entries remain bounded. Inline rejection drops the stream without creating a task; rejection-work high water is one. RequestGuard drops its gauge before returning its semaphore permit. Active tests cover unpolled cancellation, retained completions, concurrent request admission and unwind.

`connection.rs:36` / `transport.rs:10`: one frame clock starts before reading the four-byte big-endian header. Size validation and request admission precede body allocation. Header/body partial progress cannot extend expiry. Both readiness loops check the original absolute deadline. Capped response serialization checks retained capacity and length before any prefix; an oversize fallback shares its original write deadline. Request/body payload is discarded before database waits. Byte gauges bound frame/reservation ownership, not all decoded heap, native RSS or Tokio internals. Parsing remains bounded by the finite 1 MiB admitted frame.

`db_worker.rs:132` / `db_worker/execution.rs:87`: exactly one owned SQLite thread receives a finite mpsc queue. A separate queue permit survives dequeue until queue-gauge release; canceled queued work remains accounted until drain. Only two fixed read-only diagnostic statements run in production; clients cannot supply SQL. Sticky checks around statements plus progress and busy callbacks cover early-interrupt/query-start races. Unit tests actively execute SQLite VM cancellation, lock waits, execution/response deadlines, empty-check/park races, canceled concurrent producers, full-queue shutdown and native-handle retention after a shutdown timeout.

`lifecycle.rs:107`: signals have priority; handlers are continuously reaped. Accept failure backoff is inline and capped at 100 ms, creates no retry task and remains signal/reaping responsive. Shutdown drops the listener, stops the worker, aborts/reaps handlers, joins the actual worker, closes SQLite and removes only the owned socket inode under one 3,500 ms deadline. The instance lock remains held through joining; the executed lifecycle fixture checks reacquisition failure before release. Fatal listener, startup and report failures exercise cleanup. Worker Drop joins rather than detaching. Tests establish these controlled conditions; indefinitely stalled kernel/filesystem calls are not qualified.

`runtime_report.rs:37`: optional export validates a private parent, opens create-new/no-follow, verifies same UID/0600/regular type/link count, retains the descriptor and caps serialization at 4,096 bytes. Active tests cover unsafe/replaced paths, output overflow and real OS write failure without bypassing cleanup. No request-derived history/labels accumulate. Same-user native socket checks retain UID policy; live same-user/non-socket cases pass, while foreign-UID policy is statically preserved rather than claimed as a new cross-user runtime experiment.

The classification repair in `lifecycle.rs` maps operational/resource/unexpected DB failures to DB_BUSY and only positive SQLITE_CORRUPT/SQLITE_NOTADB or failed diagnostic reports to INTEGRITY_FAILED. Executed regressions pass typed errors through the production adapter/connection handler and actual CLI, checking exact IPC_ERROR versus INTEGRITY_FAILED, sanitization and status/doctor recovery. This is a typed failure injection; it does not claim the host actually exhausted memory or suffered disk I/O failure.

Defaults are C=64 connections/retained handlers, R=16 requests, Q=8 queued jobs plus one executing. Header/frame/write budgets: 1,000/2,000/500 ms; database response/execution/busy budgets: 1,500/1,000/100 ms. Test profile: C=8/R=4/Q=2, transport 150/300/100 ms, database 500/300/50 ms. Both use a 3,500 ms shutdown budget. Invalid limits/profiles are actively rejected. Hello/health bypass SQLite, but admission provides no reserved health slots during complete transport saturation.

## Acceptance evidence

The [runtime validator](qa-review/attempt-3/runtime-validation.json) independently recomputes every reported high water against its configured cap, final zero gauges, attempt reconciliation and sample percentiles. Fresh JSON observations are in qa-review/attempt-3/workspace-raw. Summary booleans were not taken as proof.

| Requirement | Actual fresh result | Assessment |
| --- | --- | --- |
| 100,000 connection attempts | 8 admitted + 99,992 rejected; 8 shutdown-canceled/reaped; retained-task high water 8, inline rejection-work 1 | Verified |
| 100,000 request admissions | 4 admitted + 99,996 rejected; four released without dispatch; recovered permit succeeds | Verified |
| 100,000 DB attempts | 9 admitted + 99,991 Busy; canceled/drained; queue 8 + executing 1; worker joins, reopen integrity ok | Verified |
| Native daemon exchanges | Four persistent clients ×25,000 = 100,000 completed; no rejection/timeout/cancellation | Verified |
| Absolute input deadlines | Paused-time header/body trickle and shared frame-clock cases; native stalled headers/partial bodies and invalid/version recovery | Verified |
| Real write backpressure | Non-reading 512 KiB echo with SO_RCVBUF=1024; write-timeout count 1; HUP observed at 113891 µs without draining before closure; both CLI commands recover | Verified |
| Transport saturation/recovery | C=8/R=4 attained; both overload CLI mappings checked; deadline releases and status/doctor recover in same process after 303135 µs | Verified |
| Slow SQL / health | Actual status CLI completes in 4424 µs while bounded test-only authorizer gate stalls preparation of real integrity SQL; normal diagnostics recover after release | Verified |
| Saturated SIGINT | 2268 µs to reaped successful exit; 11/11 handlers, 1/1 worker; zero gauges, SQLite closed, socket removed, restart and reopen integrity ok | Verified, below five seconds |
| Saturated SIGTERM | 7479 µs; same complete accounting/recovery | Verified, below five seconds |
| Protocol/CLI/identity/security | Existing protocol-1 framing, CLI output/error/version cases, TMPDIR-independent identity, single-instance and filesystem protections execute successfully | Verified locally |
| Frozen migration | Direct byte comparison with accepted HEAD; SHA-256 046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384 | Unchanged |
| Native performance / SRS | Fresh samples recalculated; NFR-002 idle RSS and NFR-004 P95 pass on this host | Verified for this slice |
| Harness | Actual unchanged command exits 1; all three failures retained | Evidence complete; conformance unresolved |
| Configured targets | Direct av.yaml read denied; target existence, health, availability and recent errors not established | Unmet access/applicability |

The four 100,000-attempt experiments are separate; they are not 400,000 network exchanges. Direct boundary tests assert bounds after each attempt, and atomically retained high waters record transient maxima. Controlled races supplement the boundary loops. Native measurements and structural accounting remain separate. Both signal fixtures synchronize three DB jobs, a partial body and four stalled headers before signaling and require work to remain active at that point.

Direct accepted-HEAD comparisons confirm byte-identical CLI source/binary tests, core identity, storage source/NFR-010 and instance code. The 9,965-byte socket preparation/binding block remains byte-identical. All ten relocated server test names remain, with filesystem/protocol assertions inspected. Resident/support changes release only the shared process lock while retaining the thread mutex until the child exits; the coordination regression passes with its assertions retained. No tests were disabled, no gates weakened. Synchronous CLI framing remains; additive DB_BUSY uses the unchanged generic IPC_ERROR fallback. Web/JWT/CORS/XSS/visual accessibility checks do not apply to this Unix daemon/CLI change; successful text is stable and color-independent, but no assistive-technology runtime qualification is claimed.

## Handoff and remaining work

Chief Director must retain the three harness findings and route any repair to the owners above. The denied av.yaml read is an access gap, not evidence of absent or healthy deployed targets. Supported configuration access is needed before actual timestamped health/availability/recent-error observations can be collected. No external restart, rollback or delivery occurred.

Chief / task-9 must refresh `docs/bounded-ipc-final.md:9`, which still cites the 50-file b3610549 snapshot and attempt-2 QA results. Use this 51-file fingerprint, attempt-3 receipts, 183 passing invocations and current measurements/signals. QA preserved the historical final assessment and its framework review; neither is promoted to a current-source approval.

Remaining Phase 0 includes Windows Named Pipe IPC/runtime/security, exec/Managed Job, desktop/menu-bar/tray integration, hook/locality fixtures and two-OS integration. Linux and Windows runtime/performance qualification remain unexecuted here. Vendor detection/classification, Meter/Traffic/Sweeper, NFR-001 long-run idle CPU, signing/packaging and broad crash-mode qualification remain open. Successful graceful reopen does not certify every forced-termination mode.

Recorded failed operations: av.yaml read returned Operation not permitted; .cargo/config.toml was absent; shell startup emitted a denied mise tracking-link warning. All required Cargo gates still passed. No denied operation was bypassed. Changed paths are only qa.json, qa.md, performance.json, performance.md, harness.txt and associated qa-review/attempt-3 raw receipts/scripts. Product interfaces and other Actors' work remain preserved.
