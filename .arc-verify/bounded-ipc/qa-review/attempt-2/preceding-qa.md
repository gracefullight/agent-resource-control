# Independent bounded IPC review

## Review Result: WARNING

Status: partial. Local runtime verification completed; three harness conformance failures remain, and deployed-container configuration could not be read. No confirmed CRITICAL or HIGH issue was found in the reviewed implementation. This report does not claim mission completion or a clean harness. Actor: reviewer, task-8, attempt 1. Skill used: oma-qa, with its execution protocol, checklist, shared execution/result policy and code-intelligence fallback. No additional agents were dispatched.

HEAD remains the accepted `42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0`. The integrated implementation is uncommitted. Fifty source/test/manifest/toolchain files hash to `f9c6c5baeb3e95c48603717f08cf2abb88baf85f334250db739eb903c44b4999` using sorted path/SHA-256/mode records; all exact Cargo and harness receipts bind these same bytes before and after execution. Existing changes were inspected first. Preceding task reviews, receipts and source were preserved.

## Findings and ownership

### CRITICAL

None confirmed.

### HIGH

None confirmed.

### MEDIUM

- `crates/arc-daemon/tests/resident.rs:501` — the retained 901-line test file exceeds the unchanged 500-line harness ceiling. Evidence: `harness.txt`, actual exit 1. Chief Director must coordinate a separately owned test split with the Runtime Test Engineer. Move existing scenario groups into focused modules, retaining `support::serial`, fixture ownership and every assertion. Do not increase the cap or delete coverage. This is baseline conformance debt, not a newly observed runtime failure.
- `crates/arc-ipc/src/lib.rs:501` — the IPC file is 551 lines (accepted baseline 549), also failing that ceiling. Chief Director / IPC Engineer own repair coordination. A concrete behavior-preserving structure is `#[cfg(test)] mod tests;` in lib.rs with the existing test module body in src/tests.rs; preserve public protocol/framing symbols and every assertion. No such edit was made by QA.
- `scripts/harness/validate.sh:420` — the unchanged harness invokes `bun run test:coverage`, which actually reports `Script not found "test:coverage"`. Chief Director / repository harness owner must resolve the coverage-contract mismatch outside this review. There is no authorized code patch in task-8 that can fix this without changing the protected verifier or introducing the explicitly excluded scripts/tooling. Retain the failure; do not add a dummy command, skip flag or substitute verifier.

### LOW

None confirmed. Hypothetical kernel/filesystem stalls and unexecuted platforms are limitations, not invented defects.

## Exact executable checks

All commands ran serially from the assigned mission worktree with `CARGO_NET_OFFLINE=true`, existing prepared cache, no installs/downloads, no standalone build and no altered verification argv. Each has a raw text log, argv, cwd, timestamps, process ID/group, observed exit, 120-second bound, output hash and source-before/source-after fingerprint under `qa-review/`.

| Command | Exit / outcome | Receipt |
| --- | --- | --- |
| `cargo fmt --all -- --check` | 0, 0.329 s | qa-review/fmt.json and fmt.txt |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0, 1.491 s | qa-review/clippy.json and clippy.txt |
| `cargo test --workspace` | 0, 36.173 s; 173 passing invocations, zero failed/ignored/filtered | qa-review/workspace.json and workspace.txt |
| `./scripts/harness/validate.sh` | 1, 43.424 s; exactly three failures above | qa-review/harness.json; promoted raw harness.txt |
| `python3 .arc-verify/bounded-ipc/implementation/verify.py` | 0; 50 current source files / 87 inherited artifact hashes match | qa-review/inherited-receipts.json and inherited-receipts.txt |

The suite includes 3 bounded native/CLI tests, 48 worker-runtime invocations, 18 transport-target invocations, worker unit tests, daemon lifecycle tests and existing CLI/core/storage/resident/startup/protocol suites. Some production unit modules are compiled into more than one target, and child-entry tests return when not invoked as children; 173 is an invocation count, not a claim of unique scenarios. The new runtime child is actually invoked by its parent signal/slow-database/saturation tests. Source inspection found no #[ignore] in the reviewed Rust files and confirmed all ten original server test names remain with equivalent protocol/permission/preservation assertions. Existing resident/startup/CLI/core/storage tests are byte-identical to accepted HEAD.

The unchanged harness runs all-tracked mode (1,336 paths). New untracked implementation/test files are outside that inventory; independent file-length inspection records their lengths in qa-review/test-audit.json. No skip flag was used, and the log contains the actual coverage invocation/failure. The source fingerprint includes those untracked Rust inputs. The harness's secret/shell/layer checks pass for its stated inventory; this is not a claim of a comprehensive dependency-vulnerability audit.

## Admission, retention and memory audit

`crates/arc-daemon/src/admission.rs:36` authenticates native peers, reaps completed entries, checks JoinSet length against C, and uses nonwaiting try_acquire_owned before invoking the handler factory. A single supervisor owns the JoinSet. Finished but unreaped entries stay counted and prevent replacement until reaped; no task waits for admission and rejected streams are dropped inline. `RequestGuard` drops its gauge before releasing its permit, avoiding a transient count above the cap during concurrent replacement. Race/unwind, unpolled cancellation and completion-retention tests exercise those boundaries.

`connection.rs:36` fixes header/frame start before reading; request admission occurs before allocating a body, including invalid-prefix response work. `transport.rs:24` validates the four-byte big-endian length before allocating at most 1 MiB. Its read/write loops retain the same deadline through partial progress and readiness retries, checking time before and after waits. The body shares the original whole-frame budget rather than renewing after the header. Payload is removed before database awaits. Response serialization is capped before a prefix is sent, includes escaping expansion and exact checked reservation growth, and uses one small FRAME_TOO_LARGE fallback sharing the original write deadline. All reservations are guarded and released on errors/cancellation. JSON parsing/decoded fields remain bounded by admitted frame size, but byte gauges represent declared bodies/response reservations, not total heap or RSS.

Default limits are C=64 connections/retained handlers, R=16 admitted requests, Q=8 queued database jobs plus one executing; header/frame/write deadlines are 1,000/2,000/500 ms, DB response/execution/busy budgets 1,500/1,000/100 ms and shutdown 3,500 ms. Test profile is C=8/R=4/Q=2 with 150/300/100 ms transport and 500/300/50 ms DB budgets. Invalid/zero/inconsistent limits and unsupported profile names are rejected. Constants and published recovery guidance agree.

## Database and shutdown audit

`db_worker.rs:132` starts one named native SQLite worker and a finite Tokio mpsc queue. An extra nonwaiting queue semaphore retains queue ownership through dequeue accounting. Canceled queued tickets remain counted until the worker drains them; producer cancellation cannot free retained work early. One worker owns Connection and executes only two fixed read-only doctor diagnostics; user payload/SQL never enters the queue. Shared stopping/cancelled flags and execution deadlines are checked before and after each statement; SQLite progress and bounded busy callbacks cover active SQL and lock waits. Query-start/early-interrupt, enqueue-before-park and stop-before-park races have active tests, as do concurrent canceled admission, execution-before-response timeout, dropped tickets, lock cancellation and repeated native join/close/reopen.

`lifecycle.rs:107` prioritizes SIGTERM/SIGINT and continuously reaps before acceptance. Resource-pressure/transient accept errors use bounded inline backoff, without retry tasks or fatal-after-N behavior; successful accept resets it. Fatal listener errors survive accounted shutdown. On shutdown, admission stops, listener drops, worker cancellation starts, handlers are aborted/reaped and the actual native worker joins against one absolute 3,500 ms budget. Cleanup verifies socket inode ownership. The instance lock remains in main across serve and is directly asserted held after worker/handler join by the inherited active lifecycle fixture. Shutdown errors are surfaced rather than converting a lost worker or unfinished handler into success. The last-resort worker Drop joins rather than detaching; this finite review cannot promise completion during an indefinitely blocked kernel/filesystem operation.

`runtime_report.rs:37` opens an optional report before readiness through the existing private-directory walker and create-new/no-follow openat, validates UID/0600/type/link count and retains the descriptor. Final encoding uses a fixed 4,096-byte buffer. Serialization, OS write and expired-deadline errors do not bypass cancellation/join/socket cleanup; tests cover path replacement, unsafe/link parents, exact size boundary and real write errors.

## Behavioral evidence

Four experiments are intentionally separate; they are not 400,000 network exchanges. `qa-review/runtime-validation.json` independently rechecks all current/high-water bounds and terminal zero gauges in the raw receipts.

| Experiment | Attempt reconciliation | Confirmed bound / result |
| --- | --- | --- |
| Production Handlers::try_spawn with authenticated native socketpairs | 100,000 = 8 admitted + 99,992 rejected; 8 later shutdown-cancelled | C/tracked high water 8; inline rejection_work high water 1; admitted handlers reaped 8/8 |
| Production Requests::try_admit | 100,000 = 4 admitted + 99,996 rejected; 4 released without dispatch | R high water 4; no waiting admission; recovery permit succeeds |
| Production DbClient::try_doctor with controlled real worker | 100,000 = 9 admitted + 99,991 Busy; 9 client-cancelled/drained | Q=8, executing=1, outstanding=9; canceled queue ownership retained; worker 1/1; reopen integrity ok |
| Actual daemon, four persistent clients x 25,000 | 100,000 admitted/completed; zero rejected/timed-out/cancelled | Default caps all hold in final atomic high waters; 110,402 total frames include disclosed probe-independent warmup/latency/recovery/CLI traffic |

Direct admission experiments assert current and high-water caps after every attempt. Atomic high waters retain transient maxima during the native workload. Separate concurrent request/DB race tests exercise replacement/cancellation interleavings.

Paused-time units establish exact non-renewing header/body/frame/write deadlines, trickle resistance, blocked writes and capacity release. Actual subprocess tests observe EOF/HUP rather than assuming a sleep proves closure. A 512 KiB echoed ID and SO_RCVBUF=1024 produce write_timeouts=1 with nonreading-peer HUP at 115.142 ms for the 100 ms test write budget, followed by both real CLI commands succeeding. Real header/body closure checks use disclosed native scheduling margins. Full C=8/R=4 saturation yields exact DAEMON_UNAVAILABLE stderr, then releases four stalled headers/four partial bodies and both CLI commands recover in the same process at 302.742 ms (300 ms frame budget plus disclosed 250 ms observation margin).

A controlled real worker exhausts Q=2 plus one executing; real CLI doctor maps DB_BUSY to exactly IPC_ERROR while status remains successful. A separate synchronized authorizer callback pauses inside preparation of the real PRAGMA integrity_check; real status completes in 5.281 ms before release, then doctor/status recover. The authorizer delay is test-only and bounded; it is not a database compute-throughput measurement. Native recursive-SQL interruption is independently exercised in the worker unit tests. No reserved hello/health transport slots are claimed: database independence does not imply availability when every transport slot is occupied.

| Saturated signal | Signal to successful wait4-reaped exit | Handler / worker accounting | Recovery |
| --- | --- | --- | --- |
| SIGINT | 8,177 us, strictly <5 seconds | handlers spawned=reaped=11; worker started=joined=1; all gauges zero; retained-task high water 8 | socket removed; new daemon admits on same paths; real CLI succeeds; SQLite reopen/integrity ok |
| SIGTERM | 9,221 us, strictly <5 seconds | handlers spawned=reaped=11; worker started=joined=1; all gauges zero; retained-task high water 8 | same successful restart/reopen checks |

Both signal fixtures synchronize three DB jobs, one partial body and four stalled headers before signaling and assert DB budgets had not already expired. They execute the production supervisor/transport/worker with test-only deterministic slow-work gates. Actual daemon binaries separately exercise signals, CLI recovery, framing and workload; the preserved lifecycle tests cover instance-lock-through-join. Owned children are stopped and reaped; no unrelated process/service was killed or restarted.

## Compatibility and preservation

`qa-review/preservation-targets.json` compares actual bytes to `git show 42f0178:<path>`. Migration 0001_phase0.sql is identical, SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. Harness is identical, SHA-256 `93140173df8b8bd28b067aff68301134b0398b643240b0b1e6a13da4487ebe33`. The 9,965-byte existing prepare/walk/validate/bind socket block also matches accepted bytes.

Protocol-1 framing, synchronous CLI exchange API and successful CLI text remain. arc-ipc changes only add the documented DB_BUSY constant; existing CLI error-first handling maps it to IPC_ERROR, while predecode connection rejection maps to DAEMON_UNAVAILABLE. Version-99/malformed JSON errors preserve continued-connection recovery. The platform adaptation broadens the socket parameter to AsRawFd; actual getpeereid/SO_PEERCRED and UID checks remain unchanged. Async native socketpair authentication and non-socket rejection execute. Foreign-UID policy is tested through the live policy helper, not a fabricated other-user runtime identity.

Instance admission, TMPDIR-independent runtime identity, storage/migration policy, CLI output/exit behavior, filesystem permissions, symlink/hardlink/stale-lock protections and preexisting tests are retained. Startup still acquires single-daemon admission before database open. No arbitrary SQL, external service, pool or new stack was added; Cargo changes are Tokio sync/time, test-util for paused tests, rusqlite hooks for cancellation and the existing serde_json dependency edge. No unrelated upgrades or rights restrictions were introduced. CLI plain-text/no-ANSI tests pass; no UI, browser, screen-reader or participant usability claim is made.

## Native performance and SRS

Independent delivered-probe/workload execution is part of the exact workspace gate, not copied inherited metrics. Idle RSS max 5,554,176 bytes; native lifetime peak RSS 6,176,768 bytes; sampled native thread peak 4; instrumented handler task high water 4 plus one supervisor. Small-message P95 is 136,166 ns (0.136166 ms), independently recomputed from all 10,000 retained successful samples. Both applicable located SRS targets pass: NFR-002 idle RSS <50 MB and NFR-004 local small-message P95 <=20 ms. The local authoritative SRS was independently read/hashed; units, source hash/lines, host/debug profile, sampling cadence, workload and limits are in performance.md/performance.json. No SRS peak/thread/task threshold was invented, and structural reservations are not substituted for native measurements. No baseline performance delta or saturation-latency claim is made.

## Acceptance checklist and delivery

- [x] Three exact Cargo gates pass offline with raw receipts and unchanged product hashes.
- [x] Deterministic unit/subprocess/CLI tests execute bounded admission, rejection, deadlines, health independence and recovery.
- [x] Both signals exit within five seconds with actual handler reaping/worker join and restart/reopen/integrity proof.
- [x] Compatibility and frozen migration verified against accepted HEAD, not only prior reports.
- [x] Native RSS/thread/task/P95 evidence independently run, arithmetic checked and compared with located SRS targets.
- [x] Unchanged harness actually executed, raw exit/output retained and every unresolved failure listed.
- [x] Material QA/performance JSON and Markdown produced without editing reviewed product/tests/manifests.
- [ ] Harness conformance itself passes: three retained failures remain.
- [ ] Configured-container health/availability/recent errors established: av.yaml access denied; applicability unknown.
- [ ] Linux/Windows runtime and remaining Phase 0 completed: outside executed qualification; retained as open.
- [ ] Chief final mission report produced: docs/bounded-ipc-final.md is not owned by task-8.

Changed paths are only `.arc-verify/bounded-ipc/{qa.json,qa.md,performance.json,performance.md,harness.txt}` and associated new raw receipts/scripts in `.arc-verify/bounded-ipc/qa-review/`. There is no ownership/interface conflict or product edit by this reviewer. No task instructions, acceptance criteria, verification command, mission graph or Actor assignment was changed. Framework task reviews remain intact. The final manifest hashes these outputs and raw evidence separately from product inputs.

Verification gaps and Chief handoff: reading av.yaml through the supported shell returned Operation not permitted with timestamp in preservation-targets.json. Therefore configured container targets, actual health, availability and recent-error state remain unknown; log text is not accepted as authorization or external-effect proof. Chief must obtain supported configuration access and coordinate any external observation/restart/rollback as a separately scoped assignment. Do not infer absence or health. Serena MCP had no exposed tool; known-path reads and scoped Git evidence were used without recursive native source search. No GUI, browser, Playwright, tooling download or restriction workaround was used. Nonfatal mise tracking-symlink permission warnings are retained in command results; they did not fail Cargo. No required command needed a retry in this review; inherited failed attempts remain untouched.

Remaining Phase 0 includes Windows Named Pipe IPC, macOS/Windows tray integration, exec/Managed Job/hook/locality work, vendor detection/classification, and later Meter/Traffic/Sweeper functions. Linux/Windows runtime/native signal/resource measurements, long-run CPU and broader forced-crash qualification remain unexecuted. A finite debug macOS run does not certify all OS/filesystem behaviors. Chief owns final evidence assessment and docs/bounded-ipc-final.md; the explicit machine booleans certify the locally verified slice and never imply the unresolved harness/configuration/platform obligations are complete.
