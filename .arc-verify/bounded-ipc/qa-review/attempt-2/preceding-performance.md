# Independent native performance review

Status: measured and compared for the two applicable ARC SRS targets on this host. This report records task-8's own run, not copied task-5 measurements. Skill: oma-qa. Run started 2026-11-07T14:35:36.034559+00:00; source fingerprint `f9c6c5baeb3e95c48603717f08cf2abb88baf85f334250db739eb903c44b4999` covers 50 files, including all Rust source/tests, manifests, lockfile and toolchain. HEAD remains accepted baseline 42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0; implemented changes are uncommitted.

| Observation | Independently measured | Applicable SRS target | Result |
| --- | --- | --- | --- |
| Idle RSS maximum | 5,554,176 bytes = 5.554176 MB = 5.296875 MiB | NFR-002: idle RSS <50 MB | Pass on this host |
| Native lifetime peak RSS, measurement daemon | 6,176,768 bytes = 6.176768 MB | No saturation/lifetime peak target supplied | Observation only |
| Native sampled thread peak | 4 threads | No numerical thread target supplied | Observation only |
| Instrumented handler task high water | 4 tasks; one supervisor separately accounted | Engineering connection cap 64; no SRS task target | Within configured cap |
| Small-message P95 | 136,166 ns = 0.136166 ms | NFR-004: local small-message IPC P95 <=20 ms | Pass on this host |
| Small-message P50 / maximum | 0.061375 / 1.330209 ms | No supplied target | Context |
| After-load P95 | 0.144416 ms, 200 exchanges | No additional supplied target | Context |

The source is `/Users/gracefullight/Downloads/Agent_Resource_Control_ARC_Requirements_SRS_2026-11-06.md`, SHA-256 `a8ef945251f9f3fd7c3e337d4efc77ff9914f95114212ca9e6c90ae2ce3e3d9e`. I read and hashed it in this review. Section 18, lines 2426 and 2428 supply NFR-002 and NFR-004. Line 2421 labels the numbers development targets requiring actual measurement and platform qualification. Front matter says 1.1.0-draft while its summary/footer retain 1.0.0-draft; the hash and requirement IDs identify the source. Decimal MB (50,000,000 bytes) is the disclosed comparison convention. The SRS specifies idle RSS, so the lifetime peak is kept separate. Its remaining performance requirements are not certified here.

## Host and workload

Native macOS Darwin 27.2.0 arm64, Apple M1 Pro, 8 logical CPUs, 17,179,869,184 bytes RAM; Rust/Cargo 1.95.0; debug profile. Exact uname/toolchain are in `qa-review/context.json` and `performance.json`. This is the repaired integrated tree; no accepted-HEAD performance delta or release-profile result is claimed.

The exact `cargo test --workspace` gate executes `bounded::native_probe_then_full_100000_attempt_workload`, which runs the delivered 200-exchange small probe first, then an owned real daemon with default limits (C=64/R=16/Q=8). The probe asserts all 200 responses and accounted shutdown. After readiness, the measurement daemon settles for five seconds, then takes 100 idle RSS/thread observations at nominal 100 ms cadence. Actual first-to-last idle observation span was 10.536004 seconds. Native `proc_pidinfo(PROC_PIDTASKINFO)` measures the daemon, not the test parent. Native `wait4` ru_maxrss records its lifetime peak in bytes on macOS. Thread peaks are sampled and may miss brief spikes.

A persistent same-user Unix connection receives 200 warm-up exchanges, then 10,000 sequential alternating `daemon.hello`/`daemon.health` round trips using request ID `small` and null payload. Encoded request bodies are 85/86 bytes; response bodies are 124 bytes; each direction adds a four-byte protocol-1 frame prefix. Timing uses `std::time::Instant` from before the write through decoded response; connection setup (30584 ns) is reported separately. All 10,000 succeed; no failed sample was discarded. I independently sorted the retained durations and recomputed nearest-rank P95 at index 9499. P50/max are also retained.

The separate real workload uses four persistent clients, each completing 25,000 matching-ID hello/health exchanges: 100,000 attempted, admitted and completed, zero rejected/timeouts/cancelled. It took 3.766090 seconds. Native load sampling uses nominal 10 ms cadence (299 observations). Recovery includes 200 additional timed exchanges and actual CLI status/doctor success. Total `frames_completed` is 110,402, including warmup/measurement/recovery/CLI requests. Both final gauges and handler/worker accounting pass.

The task high-water counters measure retained connection handlers, including completed entries awaiting reap; one supervisor is exported separately, and retry_tasks=0. They are instrumentation evidence, not a census of Tokio internals. The measurement workload reached four retained handlers; the separate adversarial tests reach eight test-profile handlers, four request slots and queue/executing/outstanding limits. Structural reservations, such as ResponseBytes, are not RSS measurements. No latency-under-saturation claim is made.

## Reproduction and evidence

`qa-review/workspace.json` records the exact offline `cargo test --workspace` command, timestamps, exit 0, process ownership and unchanged source fingerprints. It independently reruns the delivered probe/workload as part of the required suite; no standalone build or modified command was used. Raw durations, idle/load samples, native summary, probe and shutdown/cap receipts are in `qa-review/workspace-raw/`. `qa-review/runtime-validation.json` records independently recomputed percentiles, maxima, caps and reconciled outcomes. `performance.json` exposes nativeMeasurementsRecorded=true and srsComparisonComplete=true only for the two located and verified comparisons above.

Remaining obligations: release/profile generalization, Linux and Windows runtime/native qualification, long-run idle CPU NFR-001, broader crash-recovery NFR-010 and the rest of Phase 0 remain open. Kernel/filesystem stalls and host scheduling cannot be exhaustively established by this finite native run. No configured deployed-container health claim is possible because reading av.yaml returned Operation not permitted. Harness conformance remains unresolved as recorded in qa.md. No baseline speedup, financial outcome or cross-platform completion is inferred.
