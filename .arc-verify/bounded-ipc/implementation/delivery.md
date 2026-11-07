# Runtime Test Engineer delivery

Status: task-5 implementation and local verification complete; ready for Chief Director collection and independent review. Mission completion is not claimed. Skill used: oma-backend, with backend-engineer role, execution policy, result contract, backend checklist/rule, i18n rule, task-1 contract/targets and task-4 executable integration. The existing Rust/Tokio/rusqlite stack was detected from Cargo manifests. No new dependency or external service was introduced.

## Changed paths and ownership

Seven new owned source files:

- `crates/arc-daemon/tests/bounded.rs`
- `crates/arc-daemon/tests/worker_runtime.rs`
- `crates/arc-daemon/tests/support/runtime.rs`
- `crates/arc-daemon/tests/support/native.rs`
- `crates/arc-daemon/tests/support/measurements.rs`
- `crates/arc-daemon/tests/support/admission_load.rs`
- `crates/arc-daemon/tests/support/deadline_recovery.rs`

Evidence and reproducible collectors are under `.arc-verify/bounded-ipc/implementation/`. Every inherited tracked product file matches snapshot 39fa9765dcb70a6c12049d84e59bc0086487b539; production modules, manifests/lockfiles, existing tests/support module, CLI sources, migration, harness and prior evidence remain unchanged. There is no ownership conflict or interface change. Test targets import the real production modules with `#[path]`; crate-wide dead-code allowance is confined to the imported-module test target. Existing tests remain active. The full suite has 173 passing test invocations, including duplicate production unit tests compiled into the controlled target; this is not a claim of 173 unique scenarios.

## Acceptance evidence

| Assigned criterion | Evidence |
| --- | --- |
| Deterministic unit, subprocess and CLI coverage | `workspace-final.txt`: 3 bounded tests and 48 worker-runtime tests, plus every preexisting suite, all passing with zero ignored/filtered tests. `accounting.json` and raw receipts reconcile four separately scoped 100,000-attempt experiments. |
| Every admission cap and bounded rejection | Unit experiments call production connection/request/DB admission 100,000 times each, asserting every independently atomic gauge/current/high-water after every attempt. Native socketpairs authenticate actual same-user peers; eight retained tasks admit 8 and reject 99,992 with rejection_work=1 and no rejection factory/task. Requests admit 4/reject 99,996. DB admits 9/rejects 99,991 with Q=8 plus one executing; canceled queued work stays counted until drain. Terminal categories are disjoint. |
| Real 100,000 request workload | Four persistent clients each send 25,000 real hello/health exchanges with matching response IDs. 100,000 admitted/completed, zero rejected/timed-out/shutdown-cancelled. Separate warmup, latency and CLI exchanges are disclosed; final frames_completed=110,402. Atomic high waters verify configured default caps throughout. This workload is not conflated with direct admission rejection tests. |
| Stalls, trickling, malformed input and real backpressure | Existing active paused-time transport tests prove absolute header/body and shared frame deadlines, trickle resistance, completion retention and response-buffer release. New real-daemon tests observe stalled-header/body closure and preserve JSON/version/length errors. A 512 KiB echoed ID plus reduced peer receive buffer produces real unread socket backpressure; write_timeouts=1, observed HUP in 105.772 ms, then both actual CLI commands succeed. |
| Saturation and same-process recovery | The controlled supervisor reaches all R=4 requests/C=8 connections; real CLI status/doctor return exactly DAEMON_UNAVAILABLE under predecode rejection. All 4 partial bodies and 4 headers expire; both CLI commands recover in the same process in 302.265 ms (300 ms frame budget + disclosed 250 ms observation margin). |
| DB overload and health independence | Controlled Q=2 plus executing=1 returns exact IPC_ERROR for real CLI doctor; status stays available. A separate test gates SQLite's native preparation callback for fixed PRAGMA integrity_check, confirms status in 3.724 ms before release, then executes real diagnostics and verifies doctor/status recovery. This controlled SQL-preparation delay is not a SQL compute benchmark. |
| Both signals within five seconds | SIGINT 7917 us; SIGTERM 4036 us from signal through wait4-reaped successful exit, with 3 database requests, a partial body and 4 stalled headers active. Reports prove zero gauges/remaining handlers, equal spawned/reaped, worker started=joined=1, SQLite closed and socket removed. Actual new daemon admission on the same socket/database and successful CLI checks follow each case; database reopen/integrity succeeds. |
| Native measurements and SRS | `performance.json`, `performance.md` and raw timestamp/sample artifacts. Both NFR-002 idle RSS and NFR-004 P95 comparisons passed on this macOS/debug host; structural counters are separate. |
| Compatibility and preservation | Unchanged resident/startup/CLI/core/storage/protocol suites pass. They retain TMPDIR-independent identity, single-daemon admission, same-user policy, filesystem/link protections, protocol 1, CLI output and migration checks. Frozen migration hash is `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384` and bytes match accepted baseline. |

## Commands and outcomes

- `cargo fmt --all -- --check`: exit 0 (`fmt-final.json/txt`).
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0 (`clippy-final.json/txt`).
- `cargo test --workspace`: exit 0 in 32.685 seconds; 173 passing invocations (`workspace-final.json/txt`).
- `./scripts/harness/validate.sh`: exit 1, unchanged script; three retained failures: resident.rs 901 lines, arc-ipc/lib.rs 551 lines, missing test:coverage. No skip flag, dummy coverage, JavaScript tooling or changed acceptance was added.
- Focused `cargo test -p arc-daemon --test bounded --test worker_runtime -- --nocapture` passed before the last two scenario additions. Their focused sql-health/deadline commands passed; the final exact workspace suite includes all final scenarios. Older focused receipts are labeled by their actual source fingerprints, not reused as final-source evidence.
- `verify.py` checks current collected source/artifact hashes and final receipt output hashes. It reports the review checkout's actual HEAD separately rather than requiring the Actor's snapshot HEAD.

The three final Cargo receipts and harness receipt bind identical product fingerprint `f9c6c5baeb3e95c48603717f08cf2abb88baf85f334250db739eb903c44b4999`. Receipts retain command argv, exact cwd, toolchain, offline environment, owned outer PID/process group, timeout, observed process exit and log hashes. Test children are owned by RAII guards and stopped/reaped on assertion failure; normal signal fixtures use wait4 and retain successful exit/accounting evidence.

## Provenance and failed attempts

Accepted committed baseline is 42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0, tree c802f71ac4d91ed6b8a602d5c83bf9cf57116b76. Actor input snapshot is 39fa9765dcb70a6c12049d84e59bc0086487b539, tree edd2a960a87599dcba29942c6a4ff557c1ed95fc, observed local HEAD 39fa9765dcb70a6c12049d84e59bc0086487b539. The supplied snapshot/worker ref names do not resolve in this isolated checkout; `source-evidence.json` preserves their names and actual failed resolutions. No missing revision was replaced with another checkout's HEAD. The seven task-5 tests remain uncommitted; a review checkout must verify collected bytes with this receipt. Snapshot tracking does not establish tracking in the accepted baseline or review checkout.

`attempts.md`, focused-first/workspace-first/workspace-check logs and receipts retain the initial missing-CLI prerequisite, wrong collector root permission failure and macOS disconnected-socket timeout-option failure. Only the owned fixture/collector were repaired. Optional ps inventory and inherited av.yaml access were denied; no GUI/toolchain/security restriction workaround was attempted. All final required Cargo checks passed. Historical integration evidence was read, not rewritten.

## Remaining risks and handoff

The unchanged harness remains failing with all three concrete conformance failures retained. Its all-tracked mode does not inspect new untracked tests in this Actor checkout; all seven new files are separately recorded at <=500 lines. Independent QA and shared final evidence promotion belong to subsequent assigned writers. Do not mark mission-wide QA complete from this Actor report.

Measurements apply to this debug macOS host. Thread peaks are sampled, instrumentation overhead is unquantified, no baseline performance delta is established, and no latency during saturation is claimed. Linux and Windows runtime/signal/IPC/native measurements remain unexecuted. Remaining Phase 0 includes Windows Named Pipe IPC, two-OS tray/desktop integration, exec/Managed Job/hook/locality work, vendor detection/classification and later Meter/Traffic/Sweeper features; long-run idle CPU and broader crash-recovery qualification remain open. Configured deployed-container targets could not be inspected because av.yaml is permission-denied; no deployment health or external-effect proof is invented.
