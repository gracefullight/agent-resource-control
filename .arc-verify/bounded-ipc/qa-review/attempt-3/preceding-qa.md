# Independent bounded IPC review

## Review Result: NOT_ASSESSED

Status: partial. The local bounded-runtime and compatibility checks passed. Three MEDIUM harness conformance findings remain; deployed-target applicability cannot be established because the supported read of av.yaml is denied. No CRITICAL or HIGH defect was confirmed. This is not a clean-harness or mission-completion verdict. Actor reviewer; task-8; attempt 2; skill oma-qa, with execution policy, result contract, QA execution protocol and checklist.

Existing changes were inspected first. HEAD remains accepted `42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0`; implementation is uncommitted. All 50 source/test/manifest/toolchain files fingerprint to `b3610549adfebffdd626a760741a200c848ec7ee26a88482bcf7e38e563383c4` using sorted path, SHA-256 and mode records. Exact commands bind the same before/after bytes. No product, test, manifest, verification contract, saved task or mission-state edit was made. No new agents, installs, downloads, standalone builds or browser checks were used. Serena tools are absent from the available inventory; review used known-path reads and scoped Git diffs. No claim path or OMA run ID was injected; the assigned qa.md/qa.json are the material handoff. No new mission run was created.

## Findings and remediation ownership

### CRITICAL

None confirmed.

### HIGH

None confirmed.

### MEDIUM

- `crates/arc-daemon/tests/resident.rs:501`: retained 901-line suite exceeds the unchanged harness 500-line cap. Chief Director / Runtime Test Engineer own a separately coordinated split. Move existing test groups into focused integration modules, preserving support::serial, fixture lifetime and every assertion; do not raise the cap or remove tests.
- `crates/arc-ipc/src/lib.rs:501`: 551-line module exceeds that cap (accepted HEAD: 549). Chief Director / IPC Engineer own a behavior-preserving split. Concrete structure: retain the public API in lib.rs and use `#[cfg(test)] mod tests;`, placing the existing test body in src/tests.rs. QA made no such change.
- `scripts/harness/validate.sh:420`: unchanged harness invokes `bun run test:coverage`, which exits with missing script. Chief Director / repository harness owner must resolve the Rust-workspace versus inherited coverage contract outside task-8. Retain this failure; do not add dummy scripts, skip flags or tooling to manufacture a pass. There is no authorized task-8 code patch for that contract mismatch.

### LOW

None confirmed.

## Executed checks

All required commands ran serially from this mission worktree, with CARGO_NET_OFFLINE=true and the prepared cache. Each receipt records exact argv/cwd, timestamps, PID/process group, observed exit, timeout status, raw-output hash and source fingerprints. Commands were bounded to 120 seconds, with owned-process shutdown on timeout. None timed out. Saved mission limits were retained: 86,400 seconds, 200 runs, three retries, at most three concurrent workspace tasks; this review used one command at a time. The original absolute mission deadline was not injected, so no new mission allowance was inferred. Host limits are recorded in context.json.

| Command | Exit | Elapsed | Raw receipt |
| --- | --- | --- | --- |
| `cargo fmt --all -- --check` | 0 | 0.444 s | attempt-2/fmt.json / .txt |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 1.322 s | attempt-2/clippy.json / .txt |
| `cargo test --workspace` | 0 | 33.438 s | attempt-2/workspace.json / .txt |
| `./scripts/harness/validate.sh` | 1 | 41.134 s | attempt-2/harness.json / .txt |

`cargo test --workspace` ran 173 passing invocations; zero failed, ignored, measured or filtered. Count includes duplicated production unit modules and child-entry tests that return in ordinary parent execution; it is not 173 unique scenarios. Parent scenarios actually launch the controlled subprocess entries. The native probe and full workload executed as part of the exact required workspace command. No additional Cargo replay was needed.

The harness ran unchanged in all-tracked mode over 1,337 entries, including the restored task-5 report. Its actual exit is 1 with the three findings above. New untracked Rust modules are outside its tracked inventory; they are included in the independent 50-file fingerprint, lint/tests and dependency-test-audit.json. No coverage skip was supplied. Its successful secret/layer/shell checks are limited to its inventory and do not constitute a comprehensive dependency-vulnerability audit.

## Runtime and race audit

`admission.rs:36` reaps ready JoinSet entries before nonwaiting admission, authenticates the native peer, checks retained length and acquires a permit before invoking the factory. One supervisor owns that set; completed entries remain counted until reaped. Rejection drops the socket inline with high water one; it creates no waiting task. RequestGuard fields drop the accounting gauge before releasing the permit, preventing replacement from counting both owners. Cancellation-before-poll, retained completion, unwind and concurrent producer tests passed.

`connection.rs:36` starts one frame clock before the header. Request admission precedes body allocation and invalid-prefix response work. `transport.rs:24` validates the big-endian declared size before allocating at most 1 MiB; header/body progress never renews deadlines. Readiness and partial-write loops check the original absolute deadline. The response writer bounds length and retained reservation, including escaping expansion, before emitting any prefix. One small overflow fallback shares the original write deadline. Parsing is synchronous over the admitted finite frame; byte gauges describe body/response reservations, not total heap or native RSS. Payload/body are dropped before database awaits.

Defaults: C=64 connections and retained handlers, R=16 requests, Q=8 queued DB jobs plus one executing. Header/frame/write deadlines are 1,000/2,000/500 ms; DB response/execution/busy budgets 1,500/1,000/100 ms; shutdown is 3,500 ms. Test profile: C=8/R=4/Q=2; transport 150/300/100 ms; DB 500/300/50 ms. Invalid limits/profiles are actively rejected. Documentation agrees with implementation.

`db_worker.rs:132` creates one tracked native worker and finite mpsc queue. Extra queue permits stay owned through dequeue gauge release; canceled queued tickets remain counted until drained. The worker alone owns SQLite and runs two fixed read-only diagnostics. No request SQL enters it. Stopping/cancellation/deadline checks surround statements; sticky progress and bounded busy callbacks cover early-interrupt/query-start races. Tests exercise active SQLite VM cancellation, lock waits, enqueue/stop-before-park, concurrent canceled producers, execution/response deadlines, full queue discard and native join retention after a timeout.

`lifecycle.rs:107` prioritizes signals, reaps continuously and uses inline capped accept-error backoff, with no retry tasks. Shutdown drops admission/listener, starts DB cancellation, aborts/reaps handlers, joins the real worker and removes only the owned socket inode under one absolute deadline. The instance lock survives handler/worker join; an active subprocess asserts this before reacquisition. Fatal accept, startup and report failures retain cleanup. Worker Drop joins rather than detaching. Native filesystem/kernel stalls remain a limitation; the executed signal tests establish the tested conditions, not all possible OS stalls.

`runtime_report.rs:37` uses private-directory validation and create-new/no-follow descriptor opening, validates UID/0600/type/link count, retains the descriptor and caps final encoding at 4,096 bytes. Active tests cover unsafe/replaced parents, replacement after open, report size, real write errors and cleanup. No request-derived labels/log history accumulate.

## Behavioral evidence and acceptance checklist

Fresh raw outputs are under qa-review/attempt-2/workspace-raw. The independent runtime-validation.json recomputes counters, every high water, terminal zero gauges and sample percentiles; it does not trust summary booleans.

| Check | Actual evidence | Result |
| --- | --- | --- |
| 100,000 connection attempts | 8 admitted + 99,992 rejected; 8 canceled/reaped; retained tasks high water 8, rejection work 1 | Verified |
| 100,000 request admissions | 4 admitted + 99,996 rejected; four released; recovered admission succeeds | Verified |
| 100,000 DB attempts | 9 admitted + 99,991 Busy; canceled/drained; Q=8 plus executing=1; actual worker joins/reopen integrity ok | Verified |
| Native workload | Four persistent clients × 25,000 = 100,000 completed; zero rejections/timeouts; separately counted probe/warmup/latency/CLI traffic | Verified |
| Absolute deadlines | Paused-time trickled header/body and actual write backpressure; native stalled headers/bodies, malformed/version recovery | Verified |
| Full saturation recovery | Test caps attained, status/doctor rejected then recover in same process after 303152 µs; 300 ms deadline plus 250 ms disclosed native margin | Verified |
| Non-reading peer | No draining before HUP; write timeout counter 1; closure observed after 116004 µs against 100 ms write budget plus native observation margin | Verified |
| Slow DB / health | Controlled real SQLite preparation blocked while actual status CLI completes in 5209 µs; normal SQL follows release | Verified |
| SIGINT | 4866 µs signal-to-reaped exit; 11/11 handlers, 1/1 worker; zero gauges, socket removed, reopen integrity ok, restart works | Verified |
| SIGTERM | 9643 µs; same accounting, integrity and restart checks | Verified |
| Protocol / CLI / authentication / identity | Existing CLI/core/storage/instance files byte-identical; active protocol-1/error/version/permissions/symlink/TMPDIR/single-instance cases pass | Verified locally |
| Frozen migration | Actual bytes equal accepted HEAD; SHA-256 046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384 | Verified |
| Native performance / SRS | Fresh measurements independently recomputed and compared in performance.json/md | Verified on this host |
| Harness | Actual unchanged command exit 1, all three failures retained | Executed; conformance unresolved |
| Configured deployed targets | av.yaml read denied; no actual target health, availability or recent-error observations | Unmet applicability/access |

These four 100,000-attempt experiments are distinct, not 400,000 network exchanges. Direct admission experiments assert bounds after each attempt; native high-water atomics retain transient maxima. Controlled race tests supplement the single-owner admission loops. Structural bounds are not performance measurements.

All ten original server test names remain in the relocated module, with protocol and filesystem assertions inspected. Resident.rs has only the reviewed process-lock handoff correction: it releases the file lock while retaining its thread mutex through child exit, preserving the serialization assertion. Existing CLI binary, core, storage, NFR-010 and instance files are byte-identical. The 107 indexed task-5 artifacts hash-match. Framework reviews and their historical receipts remain untouched; older boundary verifiers are historical and were not rewritten to accept these new QA outputs.

## Remaining work and limits

Chief Director must retain the three harness failures and coordinate any repairs with the owners above. Supported av.yaml access is required to establish whether deployed container targets exist; logs or local code checks cannot prove their health, availability or recent-error status. No external effect, restart or rollback occurred.

macOS results do not qualify Linux or Windows. Windows Named Pipe IPC/runtime/security, Windows tray and native measurements remain unexecuted. Remaining Phase 0 includes exec/Managed Job, desktop/menu-bar/tray integration on required OSes, hook/locality fixtures and two-OS integration. Vendor detection/classification and Meter/Traffic/Sweeper remain unfinished; existing CLI truthfully reports those limits. NFR-001 long-run idle CPU, signing/packaging and full crash-mode qualification are not completed by this review. Task-9 final mission assessment belongs to the Chief.

Failed read attempts: av.yaml returned Operation not permitted; two guessed db_worker subpaths did not exist and review continued with execution.rs. Shell entry emitted a denied mise tracking-link warning; all Cargo checks nevertheless passed. No restriction was bypassed. Previous top-level task-8 outputs were preserved as preceding-* in the attempt-2 receipt directory before replacement. Changed paths are only the five assigned top-level QA/performance/harness files and associated qa-review/attempt-2 receipts; interfaces are unchanged.

Current handoff to Chief / task-9: `docs/bounded-ipc-final.md:11` still cites the earlier f9c6c5ba source fingerprint and attempt-1 receipt set. It needs the current b3610549 fingerprint, attempt-2 receipt links, 5.619712 MB idle RSS, 6,651,904-byte native lifetime peak, 90,750 ns P95 and current signal timings. This is stale downstream evidence, not a newly demonstrated runtime defect. That file and preceding framework task reviews were preserved; QA did not claim their current-tree reapproval. The new reports supersede the top-level summaries while archived preceding-* copies retain their exact earlier bytes.
