# ARC target sources and measurement requirements

Status: task-1 evidence, 2026-11-07. Targets located; performance has not been measured in this task.

## Source identity

The locally available ARC SRS is `/Users/gracefullight/Downloads/Agent_Resource_Control_ARC_Requirements_SRS_2026-11-06.md`. SHA-256 on this run: `a8ef945251f9f3fd7c3e337d4efc77ff9914f95114212ca9e6c90ae2ce3e3d9e`. It is outside the worktree and was read in place; no SRS copy was added. Its title/product identify Agent Resource Control, unlike this repository's inherited Agent Valley PRD.

The front matter at lines 2–9 labels it `1.1.0-draft`, dated 2026-11-06, and an implementation-ready design baseline with unimplemented status. The summary table at lines 21–24 instead says `v1.0.0-draft`; the footer at line 3683 repeats that older label. Cite the filename, hash, section and requirement ID rather than silently resolving this internal version inconsistency. This is the available ARC requirements source, not proof that any target has already been reached.

| Requirement | Source location | Target and supplied conditions | Comparison rule |
| --- | --- | --- | --- |
| NFR-002 | §18, line 2426 | Daemon **idle** RSS below 50 MB, measured by platform | Record idle current RSS and native process peak RSS separately; compare idle to 50,000,000 bytes and also display MiB. Report the decimal-MB interpretation as a measurement convention because the SRS does not define units more precisely |
| NFR-004 | §18, line 2428 | Local **small-message IPC round trip** P95 within 20 ms | Measure a real local request/response; disclose message sizes, operation mix, connection reuse and clock; compare observed P95 <=20 ms |
| NFR-001 | §18, line 2425 | Idle CPU long-run average below 0.5% target under ordinary workload | Relevant later qualification; a short peak-RSS or request benchmark cannot certify it |
| NFR-010 | §18, line 2434 | DB integrity after forced termination | Retain existing storage tests; graceful shutdown/reopen alone does not certify crash recovery for every failure mode |
| IPC flood | §13.4, line 1755 | Prevent an adapter's 100,000 IPC requests from overloading daemon | Mission supplies this slice's finite global bounds and explicit 100,000-attempt proof; per-adapter identity/rate policy for later adapters remains outside current protocol implementation |
| Platform scope | summary lines 24–25; Phase 0 lines 3121–3135 | Required macOS/Windows, additional Linux; Phase 0 includes daemon/tray/CLI on both OSes | One native macOS result does not complete Linux or Windows runtime qualification |

§18 line 2421 explicitly presents these numbers as development targets needing actual measurement and platform qualification. It supplies no peak-under-saturation RSS threshold, native thread limit, Tokio task threshold, precise small-message byte cutoff, sample count, run duration, warm-up rule or CPU/machine definition. Do not invent those as SRS requirements. The [runtime contract](bounded-ipc-contract.md) defines engineering bounds and a reproducible experiment below; label them as mission measurement conventions.

## Discovery and access evidence

- Repository documentation and `.arc-verify` text searches for SRS/RSS/P95/NFR found historical reviews saying no SRS had been copied and an unrelated Agent Valley webhook-to-spawn P95 target in `docs/prd.md:258`. That target is not applicable to ARC IPC.
- Filename discovery outside the mission worktree located the ARC SRS in Downloads. Only this matching source was opened; unrelated requirements files were not read.
- Connected Google Drive searches `ARC SRS`, `ARC` and `SRS` yielded no matching ARC requirements document. Unrelated results were not fetched. The successful local source supersedes this discovery gap.
- Serena was configured in `.mcp.json`, but no Serena tool was exposed in the active tool inventory. Source inspection therefore used scoped reads of known paths, Git diffs and non-code documentation searches, not native recursive code search.
- Direct supported read of ignored `av.yaml` returned `PermissionError: [Errno 1] Operation not permitted`. `av.example.yaml:198–209` contains only a commented container-observation example. No deployed container target is established by the supplied assignment. Actual configuration could not be inspected, so this task cannot assert that configured targets are absent or healthy. No service was restarted and no restriction was bypassed.

## Native measurement protocol for later tasks

Record host OS/build, CPU model/architecture/logical cores, RAM, exact source revision/diff hash, rustc/Cargo versions, profile, configuration, timestamp and measurement commands. This task's host probe reports Darwin 27.2.0 arm64, 8 logical CPUs and 17,179,869,184 bytes RAM; it is context, not performance evidence. Use installed Rust 1.95.0 and existing libc/native APIs. Do not introduce a benchmark framework or telemetry service.

First run a small owned subprocess probe (for example 200 sequential hello exchanges) to confirm readiness, accounting, native sampler and bounded cleanup. Then use the default profile for the required load and performance observations. A test profile result is additional adversarial evidence, not the default-profile efficiency measurement.

1. After migration/readiness and a disclosed idle settling interval (proposed five seconds), sample current daemon RSS and native thread count at fixed intervals for a disclosed idle window (proposed ten seconds). Native startup peak may exceed current idle RSS. Report both; NFR-002 applies to idle RSS. Use bytes internally, MB and MiB in the report.
2. Record daemon-native peak RSS through `wait4`/`getrusage` for the owned daemon child, or native process APIs with a clearly described sampling limit. macOS `ru_maxrss` is bytes; Linux reports KiB and must be converted. A test-parent RSS is not the daemon RSS, and sampled maxima must be labeled sampled maxima if no native lifetime peak is available.
3. Capture native thread counts while idle and during load through macOS `proc_pidinfo` / Linux `/proc/<pid>/status` or an existing native equivalent. Disclose sampling cadence; sampled thread peaks may miss brief spikes. The final fixed-size runtime report supplies handler/task and DB high waters independently. A fixed thread configuration is not a measured thread count.
4. Warm a persistent same-user Unix connection with 200 unmeasured hello/health exchanges, then time 10,000 successful sequential small exchanges using `std::time::Instant`. Proposed mix: alternating hello and health, compact ordinary request IDs and null payload. Record exact encoded request/response body sizes; no doctor SQL in this baseline latency sample. Time from before request write through complete decoded response, with socket setup reported separately. Sort durations and select nearest-rank P95 at `ceil(0.95*N)-1`; report P50/max and failure count as context. Failures remain visible and may not be discarded to manufacture a passing P95.
5. Run the separate 100,000-attempt bounded-concurrency workload with counters reconciled by outcome, native RSS/thread observations and final runtime task gauges. Also measure a disclosed small-message sample during and after saturation if making a saturation-latency claim. Do not compare rejected attempts' short duration with successful round-trip latency or substitute structural byte/task caps for measurements. The idle RSS threshold does not become an invented saturation-peak threshold.
6. When comparing accepted HEAD and repaired code, use the same host/profile/workload and isolated owned children. Keep baseline adversarial load to a reversible safe probe if unbounded native-thread behavior prevents completing it; report that limitation rather than risking the host or inventing a baseline. No baseline build was requested/performed by task-1. Compile only as part of the explicitly required test/gate work or a separately authorized build.
7. Measure signal-to-reaped-exit for both SIGTERM and SIGINT, assert strictly below five seconds, and retain actual worker-join/handler accounting. Reopen the database and run integrity checks using the existing storage interface. Record socket cleanup/restart. These measurements do not prove shutdown in an indefinitely stalled kernel filesystem operation.

The later `.arc-verify/bounded-ipc/performance.json` must set `nativeMeasurementsRecorded` only when native measurements exist, and `srsComparisonComplete` only when both NFR-002 idle RSS and NFR-004 small-message P95 have actual results compared with these source targets. Accompany them with platform, unit conventions, workload conditions, idle versus load peak distinction, thread/task definitions and failures in `performance.md`. A failed target is a measured failure, not a reason to change its threshold or mark comparison as success. Reviewer conclusions must distinguish comparison completeness from target achievement.

## Remaining obligations

Phase 0 in SRS lines 3121–3135 additionally calls for Windows Named Pipe IPC, `exec`, desktop scaffold, macOS menu bar/Windows tray, hook/locality fixtures, Managed Job execution and two-OS integration. This mission repairs the existing Unix daemon/status/doctor slice. It does not complete those features, vendor detection/classification, Meter/Traffic/Sweeper, signing or packaging. Linux native runtime checks remain separate from macOS results; Windows runtime work and its native measurements remain unexecuted here.

The unchanged repository harness must still run after integration. Historical unresolved size/coverage failures are context only; retain the new exit and every remaining failure. This task adds no JavaScript tooling or harness exception. Container health/availability/recent-error qualification remains conditional on access to real configured targets; the `av.yaml` read restriction is still unresolved.
