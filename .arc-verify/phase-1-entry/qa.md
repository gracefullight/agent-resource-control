# Phase 1 entry: independent QA

## Review Result: WARNING

No CRITICAL or HIGH. Three MEDIUM, six LOW, three unreproduced gaps. Row statuses are not in this snapshot (`closing.md:1071` still reads 18/1/8/13); they are task-5-integration's.

Snapshot: commit `9c93f89`, tree `03b3c4de` (contract task plus lanes A, B, C on base `393f2a3`, tree `f425c30`). Written once by task-6-qa; later deliveries are not covered.

## Evidence

- **Read:** `contract.md`, the Phase 1 lists, SRS L2425-2429, L2657-2695, §9.6-9.8, the new test targets and helpers, the lane product diffs.
- **Run by QA:** git, hash and directory listings only; no test, build or coverage.
- **Runner log (read):** scoped `cargo test`, started 2026-11-21T22:28:24Z in the integrated worktree, which is byte-identical to this snapshot. `phase0_targets` 5 passed in 40.51 s, `agent_test_journey` 3 in 10.10 s, `project_fairness` 2 in 10.94 s, `bounded` 11 in 19.64 s; 0 ignored. One failure elsewhere (M1). Bun: 372 pass, 0 fail.

Paths are under `crates/`, except dotted ones and `docs/`; a bare file name is in `arc-daemon/tests/` or repeats an earlier path.

## MEDIUM

**M1. Fixture name collision fails a runner step.** `arc-daemon/tests/support/native_fixture.rs:31-36`, owner task-5-integration. The root `b{pid:x}-{n:x}` is created with `unwrap()`. In the run above `registered_python_lifecycle_and_compatibility` panicked there with `AlreadyExists` and passed alone. The per-user temp root held 598 stale `b*-*` directories at review, none made during that run. Fix: loop on the create and take the next counter value on a collision (the file is at 500 lines, so move a helper out):

```rust
Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
```

**M2. NFR-003 margin depends on the reference.** `arc-daemon/tests/phase0_targets.rs:178-184,207-209,225`, owner Chief. The asserted bound subtracts the P95 of `/bin/sh -c '/usr/bin/true'`; the bare spawn (`direct_p95`) is printed only. SRS L2427 is the delay the hook adds; without ARC no hook shell starts. Actor-reported worst loaded values: 5.4 ms (9.3x) with the shell reference, 27.2 ms (1.8x) with the bare spawn, 34.3 ms absolute (1.5x). Only the first meets the five-times rule; QA could not verify the figures. Fix: rule on the reference. For the bare spawn, assert `hook_p95.saturating_sub(direct_p95) <= NFR_003_OVERHEAD_NS` and leave NFR-003 Partial with the numbers. Either way record all three values in `closing.md`.

**M3. Full-run time is 46 s under budget.** `.arc-verify/test-performance/contract.md:26` requires passes below 900 s; the run above took 854.1 s and, failing on M1, is not a pass. `phase0_targets.rs:47,93-112` holds the per-user daemon lock across a 30 s window. Owner Chief. Fix: add no serial-lock time; a 16 s window still contains one 15 s idle tick.

## Verification gaps (cited lines, not reproduced; owner lane C)

- **G1.** Memory hold keeps the 40 ms cadence at depth. `arc-storage/src/jobs/traffic/admission.rs:365-376` answers `MEMORY_BUDGET` to every waiter before head selection; `arc-cli/src/exec/traffic/cadence.rs:53-56,81` then polls at 40 ms without the turn lock. The saturation of `contract.md:20` (DB_BUSY, then unqueued starts) can recur under memory pressure. Fix: back off and take the turn for `WaitReason::Fifo | WaitReason::MemoryBudget`; add a deep-queue probe under a memory hold.
- **G2.** `cadence.rs:88-96` waits for the turn lock without bound: a wrapper stopped while holding `.traffic-wait-turn` stops every behind-head poll. Fix: after a bounded number of slices, poll without the lock.
- **G3.** `arc-daemon/src/db_worker/jobs.rs:66-77,111-114` skips expiry on five step operations; no test pins that it runs on the operation that follows.

## LOW

- **L1** `phase0_targets.rs:127`: `assert_ne!(mode, Active)` admits Pressure, so an idle tick is not strictly asserted. Fix: `assert_eq!((idle.mode, idle.interval_ms), (SamplingMode::Idle, 15_000))`.
- **L2** `arc-cli/tests/agent_hooks_test_literals.rs:47-58`: no explicit configuration paths. Fix: set `XDG_CONFIG_HOME`, `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_NOSYSTEM=1` and `GH_CONFIG_DIR`, as `Sandbox::command` does.
- **L3** `contract.md:9-10`: D3 omits the advertisement lock (`.traffic-fairness-v1`) that gates the key; D4 omits that the wire reply stays `FIFO` and that `--json` gains `last_recorded_reason`. Owner task-5-integration.
- **L4** `docs/managed-exec.md:65`: the QUEUED sentence for the two Test literals is untested; the two files beside the socket and the 2.56 s back-off are undocumented.
- **L5** `arc-storage/src/jobs/traffic/fair_head.rs:52-63`: each poll reads every retained entry of each candidate project; a stream of never-granted keys passes older projects.
- **L6** `arc-daemon/tests/support/measurements.rs:206`: the receipt scope still names an earlier mission's task.

## Checks

| Check | Result | Evidence |
| --- | --- | --- |
| (a) Production path | passed | Installed Claude hook fed the recorded 2.1.327 payload, then `hook-exec`; built `arcctl exec`; real isolated daemon. NFR-005 times daemon round trips after three untimed `arcctl classify` runs. |
| (b) Exactly once, fail-open | passed | `agent_test_journey.rs:204-246`: one stub start, equal to the direct run in args, cwd, stdin, environment, stdout and exit 17; stderr is the D8 notice then the stub's bytes; database and `runtime.json` byte-identical; no socket; bystander alive. Three hook failures emit no decision (`:260-285`); no permission key in a reply (`:73-74`). |
| (c) AT-TRF-05 | passed | Triggers in the daemon's own transactions log held permits (`ledger.rs:61-70`): maximum 1, last 0. ACTIVE while the root waits at a FIFO gate after the nested exit (`agent_test_journey.rs:381,394`), RELEASED after the root exits (`:431-433`). No elapsed time. |
| (d) AT-TRF-08 | passed | Each enqueue read as QUEUED from SQLite (`project_fairness.rs:254-266`); `before_b <= 1` (`:351-355`); reason in SQLite, `--json` and text (`:361-373`); 97 cancelled with zero starts; permits released. Keyless order is proven in storage only (`fair_head.rs:72-79`, `null_project_keys_keep_sequence_order`), not through a protocol-1 peer. |
| (e) Privacy, compatibility | passed | The key is a domain-separated SHA-256 (`arc-cli/src/exec/project_key.rs:83-100`), validated on the wire and by CHECK. Existing readers get the projection (`arc-storage/src/policy.rs:272`); only the opt-in decision read returns the raw tag. Under `apps/` only one Rust test changed (`project: None`); no TypeScript or binding. |
| (f) Lane A | passed, M2 open | Bounds are the SRS numbers (`phase0_targets.rs:40-44`, `measurements.rs:51-53`). Mach timebase conversion (`support/native.rs:69-79`) with a `wait4` calibration within 10% (`phase0_targets.rs:96-107`). Tick asserted (`:129`; L1). Margin comments exist; figures not determinable. No other elapsed-time assertion in the new tests. |
| (g) Fixtures | passed, L2 | Cleared environment, temporary HOME, explicit paths, `LLVM_PROFILE_FILE` (`support/journey.rs:113-138`); 60 s watchdogs; nothing ignored; largest changed file 496 lines; no Bun or handoff file. The D9 test inherits the runner's environment. |

## Earlier-named gaps

- **Closed:** 101-waiter load (zero rejections), arm64 CPU unit, isolation of the NFR-002/004 test, NFR-005 method, rendered fairness reason, AT-TRF-06 record, R-004 test named and passing.
- **Closed by reading:** `/bin/sh` drops the interposer, but only `ssh`, `docker`, `act` and git-SSH binding reach `getifaddrs` (`arc-cli/src/exec/endpoint_context.rs:52,94-118`).
- **Open:** M1, M2, G1-G3, L2, L3, L4. Lane A values are not in the runner log. Coverage of the new modules is unmeasured.

## Limits

One macOS arm64 host, debug build, fixture workloads. Code search was down; dependents were read in named files. No live vendor session, no queued wait against a vendor tool timeout, no customer or revenue evidence.
