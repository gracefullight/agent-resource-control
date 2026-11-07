# Task-5 attempt-3 native measurements

Actual daemon measurements on Apple M1 Pro, 8 logical CPUs, 17,179,869,184 bytes RAM;
Darwin Eunkwangui-MacBookPro.local 27.2.0 Darwin Kernel Version 27.2.0: Thu Oct 15 00:02:46 PDT 2026; root:xnu-13432.41.4~25/RELEASE_ARM64_T6000 arm64. Rust/Cargo 1.95.0, debug profile. Current Actor HEAD
is 5aa081fb2f73f3d606ef009e30872d32aa94ce6b plus two owned uncommitted fixture
changes. Source fingerprint: `b3610549adfebffdd626a760741a200c848ec7ee26a88482bcf7e38e563383c4`. This is local snapshot/worktree
evidence, not code attributed to accepted baseline 42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0.

| Measurement | Actual | SRS comparison |
| --- | ---: | --- |
| Maximum sampled idle RSS | 5,586,944 bytes (5.586944 MB; 5.328125 MiB) | NFR-002 <50,000,000 bytes: pass |
| Native lifetime peak RSS | 5,996,544 bytes | No SRS load-peak threshold |
| Sampled OS thread peak | 4 | No SRS thread threshold |
| Retained handler/task high water | 4 | Instrumented task gauge; supervisor separately 1 |
| 10,000 small-message P95 | 0.089916 ms | NFR-004 <=20 ms: pass |
| P50 / max | 0.063458 / 0.284208 ms | Zero failures |
| After-load 200-message P95 | 0.089667 ms | Recovery observation |
| Four-client 100,000-attempt load | 2.286652 seconds | All 100,000 completed; no rejections, timeouts or cancellation |

A separate 200-request subprocess probe ran before full load and verified native
sampling, successful shutdown, accounting and database integrity. The measured
child used default C=64/R=16/Q=8, two Tokio worker threads and one SQLite thread.
After five seconds settling, 100 idle samples used nominal 100 ms cadence; actual
first-to-last span was 10.269732 seconds. Load snapshots used nominal 10 ms cadence.
`proc_pidinfo(PROC_PIDTASKINFO)` measures daemon RSS bytes and native thread count.
Owned-child `wait4` ru_maxrss records lifetime peak bytes on macOS; Linux fixture
code converts KiB to bytes but was not executed. Sampling happens in the parent;
sampled thread peaks can miss short spikes. Instrumentation overhead is unquantified.
Task gauges count application JoinSet entries, including completed entries before
reaping, separately from native threads and the supervisor; they are not RSS data.

After 200 process-warming hello/health exchanges on a separate connection, the
latency sample uses one persistent same-user Unix connection and alternates
hello/health with ID `small` and null payload. Request bodies are 85/86 bytes;
responses are 124 bytes, with four-byte prefixes each way. `std::time::Instant`
spans request write through decoded response, including serialization. Connection
setup is separate (80834 ns).
Nearest-rank P95 is sorted index 9499. All 10,000 nanosecond durations and message
sizes are retained in `workspace-final-raw/small-message-samples.json`; no failed
exchange was discarded. No during-saturation latency claim is made.

The SRS was read in place at
`/Users/gracefullight/Downloads/Agent_Resource_Control_ARC_Requirements_SRS_2026-11-06.md`,
SHA-256 a8ef945251f9f3fd7c3e337d4efc77ff9914f95114212ca9e6c90ae2ce3e3d9e.
Section 18 lines 2426/2428 supplies NFR-002/NFR-004. Decimal MB is the disclosed
comparison convention. Its draft version labels differ; hash and requirement IDs
identify the source. Both comparisons are complete and passed on this host/profile.
This does not establish an accepted-baseline improvement, release-profile behavior,
long-run idle CPU, Linux/Windows qualification or the remaining Phase 0 features.

Reproduce using a new output directory:
`ARC_TEST_EVIDENCE_DIR=<new-directory> CARGO_NET_OFFLINE=true cargo test --workspace`.
The required workspace suite supplies the actual CLI binary prerequisite. Focused
tests afterward: `cargo test -p arc-daemon --test bounded --test worker_runtime -- --nocapture`.
The active native fixture is `bounded::native_probe_then_full_100000_attempt_workload`.
`record.py` records exact argv, cwd, bounded 120-second process ownership, output
hashes and source hashes. Earlier and focused measurements remain separate raw runs;
this report uses only the successful final workspace run.
