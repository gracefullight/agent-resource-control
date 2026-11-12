# Host Meter contract — MTR-001 / MTR-006 first macOS slice

Normative interface version: `meter schema_version = 1`, carried by unchanged
authenticated IPC protocol 1. This task establishes the compiled contract and
scaffolds; native measurement, daemon installation, CLI, Dashboard and final
native acceptance are dependent deliveries, not claims made by this document.
The Technical Director may append an explicitly nonnormative assessment after
task-1 handoff. The normative contract is frozen at that handoff.

## Requirements and baseline

Accepted HEAD: `efedf0f4eb9b97eed641c82f3c4766bb16f2e3de`, verified before mutation.
The first implementation action copied the exact authoritative Downloads bytes
to `docs/requirements/ARC_SRS_2026-11-06.md`, creating its missing parent.
Both SHA-256 checks immediately after the copy yielded
`a8ef945251f9f3fd7c3e337d4efc77ff9914f95114212ca9e6c90ae2ce3e3d9e`.
The destination has 199470 bytes. The source is
`/Users/gracefullight/Downloads/Agent_Resource_Control_ARC_Requirements_SRS_2026-11-06.md`.
The exact copy satisfies amendment 9's prerequisite; it was not reconstructed,
translated or normalized. Raw copy evidence: `target/host-meter/task-1/requirements.json`.

Before mutation, accessible assigned tracked paths showed no changes. Root Git
status/diff reported `av.yaml: Operation not permitted`; a full-clean worktree
claim is therefore not made. The destination did not previously exist. No
existing migration or checksum is edited. Task-1 preserves protocol, admission,
job execution, fail-open behavior and old operation semantics.

Skills/profile used: installed `backend-engineer`, primary `oma-backend`, companion
`oma-frontend`. Detected stack: Rust 2021, Tokio, serde, libc, SQLite; existing
desktop React 19, Tauri 2, Bun and TypeScript 7. Existing stack choices are retained.
No new third-party dependency or dependency upgrade is required by this contract.

## Ownership and dependency handoff

Task-1 owns the SRS, this contract, `crates/arc-core/src/meter.rs` and its children,
the shared JSON fixture, `crates/arc-ipc/src/meter.rs` and children, the minimum
client unsupported mapping and its tests, new `arc-platform`/`arc-meter` scaffolds,
all required Cargo workspace/path manifests and Cargo.lock. It is the sole shared
manifest writer in this wave. `arc-core` gains a serde_json dev dependency;
`arc-daemon` gains arc-meter. Existing CLI and desktop already depend on core/IPC.

Dependency direction: `arc-daemon -> arc-meter -> arc-platform -> arc-core`;
`arc-ipc -> arc-core`; CLI and desktop consume core/IPC. arc-platform performs
finite native observations; arc-meter owns arithmetic, policy and cached state;
daemon lifecycle owns the sampler; handlers only read a cached snapshot. This
separates platform observation from scheduling without adding another framework.

After task-1 handoff, runtime owns native adapter, sampler, daemon and CLI code;
backend-desktop owns Tauri/TypeScript bridge, existing refresh-controller extension,
Dashboard and component tests; backend-gates owns coverage inventory/tooling,
harness receipt reuse, integrations, final native gates, desktop compatibility
commands, notices regeneration and native observations. Any needed manifest
change after this settled dependency handoff must be coordinated with the Chief.
Other Actors' work must be preserved. No mission graph, acceptance criteria or
saved assignment is edited. Every new source/test file must remain below 500 lines.

## Shared wire types and fixed operation

Rust source of truth is `arc_core::meter`. The canonical synthetic, deterministic
wire example is `crates/arc-core/tests/fixtures/meter-snapshot.json`. Its observations
are fixtures, not real native observations. TypeScript mirrors these types in its
existing bridge boundary and validates its received presentation contract.

`arc_ipc::meter::snapshot_request(id)` constructs operation `meter.snapshot` with
`payload: null`. `decode_request` rejects every other payload, including `{}`.
There is no client-controlled path, PID, sampling frequency or historical query.
`decode_snapshot(&Value)` validates object shapes (including nested objects),
known fields, finite numbers, units, provenance, identities, timestamps and bounds.
`snapshot_response(&Request, &MeterSnapshot)` produces a bounded valid response or
`METER_UNAVAILABLE`. It does not install a daemon handler.

`snapshot_until(socket, request_id, absolute_deadline, cancelled)` is the typed
Unix client. It reuses `arc_ipc::client::exchange_until` and its same-user endpoint
and peer checks, framing, request identity and cancellation. No daemon launch,
credential, transport, thread or renewed deadline is introduced. A caller uses
its existing read budget (desktop currently two seconds); busy retry is disabled
for this cheap current-value operation. Existing frame maximum remains 1 MiB;
the compact JSON **result** must additionally fit in 16384 bytes. No collections
or historical arrays appear in this result. A protocol-1 old daemon's explicit
`UNSUPPORTED_CAPABILITY` maps to `Failure::Unsupported`, scoped only to Meter;
health/job inspection continue. A Meter `METER_UNAVAILABLE` reply maps to
`Failure::Unavailable` without retry. Other operations retain their semantics.

The later CLI command is exactly `arcctl meter snapshot --json`. On success it
prints the validated snapshot object as JSON once and exits zero. Unsupported,
unavailable, malformed or failed exchanges return a nonzero status with a static
local explanation; they must never print a fabricated all-zero snapshot. No job
registration, permit, storage mutation or execution is triggered by this read.

Every measurement is an object with `unit`, `provenance`, `observed_at_ms`,
`observed_monotonic_ms` and `reading`. Units are `percent`, `bytes`, `core_seconds`
or `level`. Provenance is `native` or `derived`. `observed_at_ms` is a nullable UTC
millisecond presentation time; a missing wall clock never becomes Unix epoch zero.
Monotonic milliseconds are relative to the owning sampler's epoch.

`reading` is a discriminated union tagged by `availability`:

| Availability | Fields | Meaning |
| --- | --- | --- |
| `available` | `value: T` | A successful observation, including legitimate zero |
| `first_interval` | no value | Host CPU has no comparable previous counter sample |
| `unsupported` | no value | Native feature or platform is unsupported |
| `denied` | no value | OS access was denied |
| `failed` | `diagnostic: string` | Native observation or derivation failed |

Only host CPU may use `first_interval`. Diagnostics are at most 128 UTF-8 bytes,
contain no control characters and use bounded local measurement labels; raw peer
diagnostics are not displayed. Absent values never become zero. JSON numbers
representing integers are bounded by 9007199254740991 for exact desktop transport.
Unknown fields, nested positional arrays, NaN/infinity and inconsistent units are
rejected. One field's measurement failure leaves the other readings available.

## Measurement semantics

`MeterSnapshot` carries schema version 1, reused `ProcessIdentity` as
`daemon_instance` (boot ID, PID and start token), positive `sequence`, nullable
`sampled_at_ms`, `sampled_monotonic_ms`, current `age_ms`, `cadence`, `host` and
`daemon`. Sequence counts completed native passes, not IPC reads. Restart changes
daemon identity, resets sequence and discards previous counters and sampled peak.

| Path below snapshot | Unit / provenance | Meaning |
| --- | --- | --- |
| `host.cpu_utilization` | percent / derived | Busy tick delta / total tick delta * 100, all host logical CPUs, 0–100 |
| `host.memory_available_bytes` | bytes / native | Darwin (free_count + inactive_count) * native page size; reclaimable-page estimate |
| `host.memory_pressure` | level / native | Native normal/warning/critical state, never inferred from RAM percentage |
| `host.swap_used_bytes` | bytes / native | Native used swap, not swap capacity or process usage |
| `host.filesystem.capacity_bytes` | bytes / native | Total capacity of the identified local filesystem |
| `host.filesystem.free_bytes` | bytes / native | All unallocated filesystem blocks, including reserved space |
| `host.filesystem.available_bytes` | bytes / native | Free bytes available to this unprivileged caller |
| `host.filesystem.pressure` | level / derived | Explicit free-space policy judgment, separate from native observations |
| `daemon.cpu_core_seconds` | core_seconds / native | This daemon process's cumulative user + system CPU since process launch |
| `daemon.rss_bytes` | bytes / native | Current resident set of this daemon process, excluding children |
| `daemon.observed_peak_rss_bytes` | bytes / derived | Maximum successful sampled native RSS in the declared observation window |

Apple's native free_count already includes speculative pages; adding
speculative_count again would double-count. The available-memory estimate excludes
compressed memory and overlapping purgeable accounting, and is not Activity
Monitor's Memory Used complement. Native source reference:
[Apple XNU host statistics](https://github.com/apple-oss-distributions/xnu/blob/main/osfmk/kern/host.c).
Native calibration must record actual reference differences, rather than infer
accuracy from this formula. OS features not present on a host remain unsupported.

Available host CPU requires `host.cpu_interval`: nullable UTC endpoints,
`started_monotonic_ms`, `ended_monotonic_ms`, positive `elapsed_ms`, positive
`logical_cpu_count` and `normalization: host_capacity`. Elapsed time uses monotonic
end minus start; wall-clock reversals do not alter it. Host normalization already
averages all cores; never divide its percent by the core count again. A first
sample, changed CPU topology, clock regression or incomparable/reset counters
must not produce a zero or spike. The sampler's deterministic arithmetic tests
must cover declared counter width, legitimate wrap and reset discrimination.
Daemon CPU core-seconds is not host utilization and is not divided by cores.

`host.filesystem.identity` is nullable when unavailable. Known disk values require
an identified local filesystem: observed absolute `path`, absolute `mount_point`,
`filesystem_type`, native `volume_id` and `is_local: true`. Paths/mount points are
each at most 1024 UTF-8 bytes; filesystem type at most 64, volume ID at most 128.
Initially the native provider observes `/`, unless daemon configuration chooses
another local path. A request cannot choose it. Native free <= capacity and
available <= free. APFS capacity sharing, snapshots and reservations may explain
differences from directory sizes or other native tools. These fields are neither
disk writes, worktree footprint, reclaimed bytes nor vendor/job attribution.

Derived disk pressure uses available / capacity: <=5% critical, <=10% warning,
otherwise normal. Missing/zero capacity or missing available space makes the
judgment unknown; native values remain independent. This is a displayed judgment,
not a scheduler restriction or a native macOS memory-pressure signal.

`daemon.cpu_accounting` is `process_lifetime`; existing ProcessIdentity identifies
that window. The sampled peak window is separately declared by
`peak_observation_started_at_ms` and `peak_observation_started_monotonic_ms`.
It begins when the sampler starts, not at an invented OS high-water mark. Peak is
at least current successful RSS. Its observation time is the last successful RSS
pass confirming the running maximum, not necessarily when that maximum occurred.
A failed current RSS retains an earlier peak with its earlier observation time;
it does not become zero or reset the peak. This process-only slice must not imply
managed-job/vendor attribution.

## Native and sampler boundaries; ownership and freshness

`arc_platform::NativeSource: Send + 'static` exposes exactly
`fn sample(&mut self) -> NativeSample`. The source belongs to one bounded worker.
`NativeSample` declares nullable UTC, sampler-relative monotonic time, per-field
`Reading<T>`, `NativeFilesystem` and `CpuCounters { user, system, nice, idle,
counter_bits, logical_cpu_count }`. Tick values are cumulative and unsigned;
counter_bits declares the native width. The source performs a finite native pass,
does not spawn external commands, perform sampling policy, persist data or use GUI
state. Native API failures are recorded per field. libc remains the only native
binding dependency unless an observed API gap is coordinated and justified.

arc-meter converts native ticks to intervals, handles counter/clock changes,
tracks one previous CPU sample and observed RSS maximum, and produces validated
`MeterSnapshot`s. `SamplingPolicy::new(idle_ms, active_ms, pressure_ms)` validates
cadence; `cadence(active_jobs, elevated_pressure)` returns a shared typed cadence.
Default intervals: idle 15000 ms, active 2000 ms, pressure 1000 ms. Idle limits are
10000–30000 ms; active/pressure limits are 1000–3000 ms. Elevated native pressure
can select pressure cadence. The existing job state supplies activity; there is
no new job identity model, GUI dependency or per-client sampling owner.

Exactly one supervised daemon lifecycle owner starts sampling independently of
desktop connectivity/visibility, stops admission and joins its owned worker on
shutdown. At most one native operation may run or be admitted; no overlapping
spawn_blocking calls may accumulate after a timeout. Delayed ticks coalesce/skip
missed intervals instead of building catch-up work. Latest-value capacity is one,
previous-CPU capacity one, native-work capacity one. IPC readers clone only this
bounded value and recompute age. A canceled read never creates a sampler.

This slice persists **zero** resource samples and has no resource history queue,
SQLite table or migration. If later persistence is introduced, the existing single
writer and SRS §14.5 retention apply: raw 72 hours, minute aggregates 30 days,
hour/day aggregates one year. It may not silently introduce unlimited history.

`age_ms` is monotonic age at daemon read time; subsequent desktop age adds elapsed
desktop monotonic time since receipt, not wall-clock subtraction. `cadence` carries
mode, interval_ms and stale_after_ms = 2 * interval_ms + 1000. Values are stale when
effective age exceeds this threshold. Refreshing an unchanged sequence must not
reset its age. Field observation age also includes sampled_monotonic_ms minus that
field's observed_monotonic_ms, so an older retained peak remains honestly dated.
The existing desktop refresh controller remains the sole read owner. Meter has
sampling-aware freshness independent of job/health's existing threshold. Late
replies cannot replace a newer daemon instance/sequence; restart/reconnect clear
incomparable monotonic baselines. Unsupported Meter never disables health/jobs.

## Consumer examples and verification entry points

Rust: `arc_ipc::meter::snapshot_until(&socket, "cli-meter-1", deadline, &cancelled)`
returns `MeterSnapshot` or existing client Failure. The daemon routes only
`OPERATION_METER_SNAPSHOT` after its existing envelope/auth checks and calls
`snapshot_response` with a bounded cached value. Desktop retains its existing
bounded blocking permit and `Received<T>` receipt clock, adding a typed read.
In presentation, inspect `field.reading.availability` before accessing value;
first_interval reads “Waiting for next sample”. Never coerce absent values to 0.

Task-1 deterministic Rust entries: `arc_core::meter::tests`,
`arc_ipc::meter::tests`, `arc_ipc::client::meter_tests`, `arc_meter::tests`.
Focused compilation and test command receipts, stdout/stderr and input hashes
belong in `target/host-meter/task-1/`. Native adapter, arithmetic and supervisor
tests are added by runtime; ten successive native samples, CLI/bridge recovery,
restart, old-daemon response and independent observations belong to backend-gates.

Only two new active Bun entry files are reserved:
`apps/desktop/tests/meter.test.ts` and `scripts/harness/coverage-reuse.test.ts`.
Additional Bun cases go in existing listed suites or non-entry helpers. The fixed
active Bun command is `bun test` with these exact paths, in order:

```text
apps/desktop/tests/bridge-client.test.ts
apps/desktop/tests/contracts.test.ts
apps/desktop/tests/credits.test.ts
apps/desktop/tests/dashboard.test.ts
apps/desktop/tests/lifecycle.test.ts
apps/desktop/tests/meter.test.ts
scripts/coverage/accounting.test.ts
scripts/coverage/desktop-foundation.test.ts
scripts/coverage/gate.test.ts
scripts/coverage/hygiene.test.ts
scripts/coverage/terminal-outcomes.test.ts
scripts/coverage/traffic-accounting.test.ts
scripts/harness/equivalence.test.ts
scripts/harness/traffic-targets.test.ts
scripts/harness/coverage-reuse.test.ts
```

The six gates remain cargo fmt, clippy, workspace tests, production-line coverage,
architecture harness and the explicit active Bun set. The backend gate owner runs
affected desktop typecheck, Biome and build compatibility; its receipts, rather
than unapproved command-shape substitutions, supply JSON-pointer verification.
The gate owner updates coverage accounting to include the exact crate inventory:
arc-core, arc-detector, arc-storage, arc-ipc, arc-daemon, arc-cli, arc-traffic,
arc-desktop, arc-platform, arc-meter. Every crate retains the 80% production-line
floor. Historical baseline identities and migration checksums remain unchanged.

Coverage is collected exactly once per successful gate pass. Harness fresh-receipt
reuse must validate current source/test/configuration, manifests/locks, the copied
SRS and this frozen contract as **inputs**, including changes to native boundaries.
Generated receipts/reports are **outputs**, not circular freshness inputs; their
referenced bytes must still be hash-checked. Reuse must recompute accounting and
validate successful native reports, profile bytes and exact merge membership.
Missing, changed, failed or tampered evidence is rejected. Standalone harness may
collect when no reuse receipt is supplied. No SKIP_COVERAGE or weakened checks.

## Evidence inventory and JSON-pointer schema

Exactly seven tracked mission evidence files are reserved under
`.arc-verify/host-meter/`, with at most 5 MB total: `contract.md` (runtime),
`experience.md` (desktop), `value.md` (Chief/business reconciliation), `native.json`
(gates), `gates.json` (gates), `qa.json` (independent reviewer), `final.md` (Chief).
No eighth tracked evidence file is added. All raw execution logs are under
`target/host-meter/`. One small checker is reserved at
`scripts/coverage/host-meter-evidence.ts`; it validates this mission only and must
not recertify historical mission claims. Historical raw-log availability remains
a separate provenance fact; isolated fixtures may test historical validators.

The final verification contract owns the following JSON pointers and exact values;
owners populate them only from actual corresponding evidence:

| File | Pointers / values |
| --- | --- |
| gates.json | `/baseline/acceptedHead` = accepted HEAD; `/requirements/{sourceSha256,destinationSha256}` = required hash; `/requirements/actualBytesVerified` = true |
| gates.json | `/contract/satisfied` = true only after integrated contract delivery |
| gates.json | `/tests/deterministic/status` = passed; `/tests/deterministic/requiredCases` = [counter-deltas, normalization, reset-wrap, clock-changes, partial-failures, sampling-bounds] |
| gates.json | `/tests/native/successiveNativeSamplesAtLeast10` = true; `/tests/native/{disconnectReconnect,daemonRestart,oldDaemonUnsupported}` = passed |
| gates.json | `/sampling/{boundsVerified,headlessAndAdaptiveVerified}` = true; `/compatibility/protocol1FailOpenAndExecutionVerified` = true |
| gates.json | `/tests/components/status` = passed; `/tests/components/requiredStates` = [fresh, stale, unavailable]; `/dashboard/preservedBehaviorsVerified` = true |
| gates.json | `/gates/allSixPassed` = true; `/gates/{workspaceTest,coverage,architecture,activeBun}/status` = passed |
| gates.json | `/coverage/productionLineFloorPercent` = 80; `/coverage/crates` = ordered inventory above; `/coverage/allCratesMeetFloor` = true; `/coverage/collectionCount` = 1 |
| gates.json | `/coverage/reuse/{currentInputMatch,harnessValidated}` = true; `/coverage/reuse/recollected` = false |
| gates.json | `/desktopCompatibility/allPassed` = true; `/containerTargets/completionRequirementsSatisfied` = true only from supported current configuration/observations; `/compatibility/migrationChecksumsPreserved` = true |
| gates.json | `/evidence/fileCount` = 7; `/evidence/withinByteBudget` = true; `/evidence/smallEvidenceCheckCount` = 1; `/evidenceCheck/status` = passed |
| native.json | `/calibration/status` = measured; `/calibration/{independentReferences,discrepanciesExplained}` = true |
| native.json | `/idle/status` = measured; `/idle/{warmed,noninstrumented,desktopHidden,durationMeetsMinimum,independentObserver}` = true |
| native.json | `/idle/cpu/denominator` = one_core; `/idle/cpu/limitPercent` = 0.5; `/idle/cpu/belowLimit` = true; `/idle/rss/limitBytes` = 50000000; `/idle/rss/belowLimit` = true |
| qa.json | `/completed` = true; `/reviewMode` = independent_read_only; `/inputIdentityMatches` = true; `/requiredEvidenceComplete` = true; `/blockingFindingCount` = 0 |

Receipts should additionally retain actual command argv/cwd, start/end UTC,
monotonic duration, exit code, input SHA-256 identities and hash/byte references to
raw stdout/stderr. `/desktopCompatibility` may record each exact command and its
receipt before setting allPassed. Snapshot calibration records native references,
sample timestamps/sequence and actual discrepancies side by side. Idle evidence
requires >=300 seconds after warm-up, noninstrumented daemon, actually hidden
desktop, independent observer and environment. CPU denominator is aggregate
user+system CPU-time delta / monotonic elapsed * 100, one core, without dividing
by logical CPUs. Report RSS in bytes and display units; limit is 50000000 bytes.
These bounded observations cannot establish long-term overhead or customer savings.

## Tool/configuration limits and remaining scope

Serena is configured in `.agents/oma-config.cue` but no Serena tool was exposed in
this run. Per the configured fallback, inspection used scoped reads of already
located paths; native source search was not substituted. Tools available include
Rust/Cargo and Bun on macOS arm64; versions and actual command outcomes are raw
check receipts, not assumptions of success. No browser, GUI, Playwright package,
browser binary or screen-based check is used for this task.

Supported direct access to `av.yaml` returned `Operation not permitted`.
`av.example.yaml` documents optional container targets but is not authoritative
configuration. Configured target existence, current health, availability and
recent errors remain an explicit gap for the Chief/gate owner; absence cannot be
inferred. No restricted file or missing native prerequisite is worked around using
GUI tools, and no AV order/resume/operate call or mission-state mutation is made.

Remaining scope after even a successful slice: Managed-job/vendor attribution,
Native Windows, Vendor hooks, Full Meter/Phase 1 acceptance. Commercial demand,
pricing, savings, acquisition, retention and profit remain unmeasured; no outreach,
publication, service installation or external distribution is authorized here.

## Technical Director assessment — task-2, nonnormative

Status: assessment completed using `oma-architecture`, in Recommendation mode.
Retain the delivered thin boundaries and daemon-owned latest-value design. This
is engineering advice for Chief/backend coordination, not an execution veto,
implementation approval or mission acceptance. The preceding normative contract
and the saved goalBrief are unchanged. This appendix introduces no scope change.

### Inspected revision and evidence

The accepted committed baseline remains
`efedf0f4eb9b97eed641c82f3c4766bb16f2e3de`, tree
`64dd3d10ba1458a42a4dcae1fa42e6bcbee43b7e`. The Actor's inspected local HEAD is
the isolated input snapshot `d75ff72869205c63e6d237c1eb9d2339e0a09a74`, tree
`962b8cd3085d2dce5b00292efbf001fed23cbc12`; it is not the accepted baseline.
Before this append, all 20 task-1 delivered paths matched that snapshot byte for
byte. Source citations below refer to those snapshot/local bytes. In particular,
the new Meter files are tracked in this snapshot; that does not establish their
presence in the accepted baseline or the eventual review checkout. This appendix
is an uncommitted task-2 change until integrated and verified there.

Direct SHA-256 checks of the Downloads source and restored destination both yield
`a8ef945251f9f3fd7c3e337d4efc77ff9914f95114212ca9e6c90ae2ce3e3d9e`.
The authoritative SRS grounds the boundaries in §4.2 (lines 285–295), host versus
process accounting in §10.1–10.2 (lines 999–1044), cadence in §10.7
(1086–1095), MTR-001/MTR-006 (1115/1120), retention in §14.5 (2005–2018), and
overhead targets in §18 (2425–2426). The broader SRS remains broader than this slice.

Task-1's `target/host-meter/task-1/handoff.md` and final raw receipts were read
from the supported source workspace, not assumed present in this isolated tree:
`/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/c35609d7-4967-4fa0-a254-6010099bfdee/workspace`.
`scaffold-check-final.json` SHA-256 is
`5102572e5a9a4f28b5ead3d2b2a95565f4d19ca0da449adf8c43f2567534585a`;
`focused-contract-tests-final.json` SHA-256 is
`0a588bbad3bbc391d08cc383f6e01fc1b9dbff400cb2211f6cdf0256371f2310`.
Both record exit 0, unchanged before/after inputs, a 7200000 ms check allowance,
and input identity `ebe504e8aa2c3cd7382f5907b83d0292e07fb7c9a3674bd45e374baa21150f51`.
All 248 recorded input files matched local bytes before this append; the four
stdout/stderr references matched their recorded SHA-256 and byte counts. The
test log records 8 core, 44 IPC, 1 meter and 0 platform tests, all passing.
These substantiate task-1's compiled interfaces and focused tests at those inputs.
No compilation, tests or native gates were executed by this assessment. The
append changes the contract input, so those receipts are not final-pass receipts.

### Boundary, ownership and cost judgment

The architectural problem is to supply honest current host and daemon readings
without putting OS work on authenticated request handling, duplicating desktop
polling ownership or adding a storage subsystem. The material alternatives are:

| Option | Implementation and maintenance | Operating cost and tradeoff |
| --- | --- | --- |
| Chosen: one daemon sampler, thin native adapter, latest value | Two small crates and one supervised owner; arithmetic is testable separately from native APIs | Bounded work/state independent of readers; no history queries. Actual CPU/RSS cost awaits measurement. |
| Persist a time series and query it for each surface | Adds schema, writer coordination, rollups, pruning, migrations and restart/history semantics | Enables trends outside this slice; adds writes, storage and retention work without serving a required current-value use case. |
| Sample inside each GUI/CLI request | Fewer initial scheduler files, but consumers acquire interval/state and native failure logic | GUI absence stops GUI-owned work; simultaneous readers can multiply native work. Does not satisfy the selected headless ownership contract. |

Prefer the first option on implementation effort, operational responsibility and
future change cost. These are qualitative engineering judgments; no Actor prices,
latency, overhead reductions, customer savings or operating results were measured.

The actual `arc-platform/src/lib.rs:7–49` boundary exposes typed cumulative CPU
counters with width/topology, a filesystem identity and per-field `Reading<T>`
through `NativeSource::sample`. It contains no native implementation yet. Keep
finite observations there; keep interval arithmetic, reset/wrap decisions and
policy in arc-meter. `arc-meter/src/lib.rs:4–36` currently declares capacities and
implements cadence selection only. Constants of one latest snapshot, one previous
CPU sample and one native work item do not yet prove bounded runtime admission.
The existing supervisor at `crates/arc-daemon/src/lifecycle.rs:53–131` is the
integration owner to extend, with one joined worker and skipped/coalesced ticks.
An IPC timeout must never allow another native operation to accumulate behind a
still-running call. Native failure must not alter admission/execution semantics.

Identity and authentication reuse are concrete: `crates/arc-core/src/meter.rs:209`
embeds the existing `ProcessIdentity`; `process_identity.rs:164–219` retains its
validated PID, string start token and boot ID. `crates/arc-ipc/src/meter.rs:104–122`
calls existing `exchange_until` with the caller's absolute deadline, cancellation
and busy retry disabled; `client.rs:235–316` retains same-user peer checking.
The Meter-only error branches at `client.rs:149–167` do not change health's error
mapping. The null-only request and bounded result decoder at `meter.rs:15–59`
provide a small additive protocol-1 operation without a new permission or transport.
This is static compatibility evidence; production CLI/bridge recovery still needs
the assigned integrations. The fixture and scripted replies are synthetic.

`Measurement<T>` and `Reading<T>` (`arc-core/src/meter.rs:64–108`) preserve units,
observation clocks, availability and provenance. Validation separates native memory
pressure from derived disk pressure, requires filesystem identity for known space
values, and bounds finite numbers and sampled peaks (`meter/validation.rs`). Keep
host normalization distinct from daemon cumulative core-seconds and observed peak
RSS distinct from an OS high-water mark. Neither supplies vendor/job attribution,
disk writes or reclaimed space. Current tests validate these contracts; they do
not yet implement or test native counter-delta/reset arithmetic.

Workspace/path manifests now connect daemon → meter → platform → core, while
IPC/CLI/desktop reuse core/IPC. Comparison of Cargo.lock package records with a
registry/source field against the accepted baseline found no changes. Existing
libc, Tokio and serde suffice for these interfaces; serde_json's core addition is
test-only. Keep dependency additions contingent on a demonstrated native API gap
and Chief-coordinated shared-manifest ownership. Crate separation has real coverage
and review maintenance cost; it does not justify another framework or service.

### Latest-value memory and conditional retention

The CLI and inline Dashboard need a current reading, not persisted history. A
previous CPU counter/time pair supplies the interval; a running maximum plus its
observation metadata supplies sampled peak RSS. Restart identity makes the reset
explicit. Latest-value memory therefore serves the slice without a SQLite writer,
history queue or migration. This preserves existing migration bytes and avoids
rollup/deletion work. Capacity declarations remain promises until runtime tests
demonstrate bounded allocation, scheduling and shutdown under delays and failures.
If persisted product resource samples appear, backend-runtime/backend-gates must
coordinate the existing single writer, bounded storage and SRS §14.5 deletion:
raw 72 hours, minute aggregates 30 days, hour/day aggregates one year, including
the documented disk-budget response. Persistence cannot be treated as a free
extension of this snapshot contract or silently introduced as diagnostics history.

### Upfront findings reconciled and routed

| Finding and current evidence | Coordination / correction owner |
| --- | --- |
| Missing SRS and crates are resolved in the task-1 snapshot. Exact SRS bytes/hash and path dependencies are verified; the native adapter and sampler engine remain scaffolds. | backend-runtime implements the adapter, arithmetic, lifecycle wiring and CLI within these interfaces; this is dependent work, not a task-1 defect or an approval gap. |
| `scripts/coverage/config.ts:3–17` still inventories eight crates; `gate.ts:31–38` requires workspace equality, while the workspace now has ten. | backend-gates adds arc-platform/arc-meter roots and accounting tests, retaining every existing crate's independent 80% production-line floor and historical baseline identities. This mismatch must be resolved before final collection. |
| `scripts/coverage/source.ts:177–194` hashes the crate/script/desktop trees, but explicitly names only the old desktop contract; it omits this SRS and the host-meter contract. | backend-gates adds both authoritative inputs and hashes this entire post-append file. Preserve existing inputs; keep generated reports as hash-validated outputs rather than circular freshness inputs. |
| `scripts/harness/validate.sh:419–443` still recollects coverage. `coverage-receipts.ts:34–51` already recomputes accounting, verifies native execution and validates profile/merge evidence; `evidence.ts:18–29` checks bytes. | backend-gates reuses these maintained validators for the harness's fresh-receipt path, rejects changed/missing/failed/tampered evidence, and proves one collection per successful pass. Retain standalone collection and all gate checks; do not use SKIP_COVERAGE. |
| `DesktopRefreshController` still has a 10000 ms stale threshold and no Meter section (`desktop-refresh-controller.ts:26–49`); `DesktopClient` has no Meter method (`bridge/types.ts:116–125`). Tauri already admits one blocking read (`src-tauri/src/bridge.rs:95–131`). | backend-desktop extends these existing owners. Meter age must combine daemon age and desktop monotonic elapsed time with the declared cadence; repeated reads cannot freshen an old sequence. Keep health/job freshness and working reads independent of Meter failure. |
| `desktop-foundation.test.ts:6–11` invokes a historical raw-log validator. Availability-only inspection of that validator's direct log references found 66 distinct paths and zero present in this isolated worktree. | backend-gates uses isolated fixtures or the stated historical-availability contract while retaining regression assertions. Missing historical logs cannot be reported as verified bytes or remedied by recertifying an old mission. |
| The two reserved new Bun entries and host-meter gates/native reports are absent at this input snapshot. Seven evidence files and one checker are already reserved above. | backend-desktop/backend-gates supply their owned files, preserve the exact active Bun paths and six gates, and publish current JSON-pointer receipts. This append consumes no extra evidence slot. |
| Root status/diff returned `av.yaml: Operation not permitted` in this run. The example cannot establish configured target absence, health, availability or recent errors. | Chief/backend-gates obtains supported authoritative configuration and current observations, or records the unmet completion prerequisite. No denied file or configuration access was bypassed. |

Runtime and desktop coordination should preserve the shared schema and settled
manifests; report a concrete required interface change to the Chief before crossing
ownership. No inspected interface contradiction requires this assessment to rewrite
the normative contract. The listed integration corrections remain with backend
owners; independent oma-qa review is still a separate required delivery.

### Validation boundary and freeze handoff

Backend-runtime must deliver deterministic deltas, normalization, reset/wrap,
clock-change, partial-failure and sampling-bound tests. Backend-gates must establish
ten genuinely successive native passes through production daemon CLI/bridge use,
disconnect/reconnect, restart and old-daemon unsupported behavior. Backend-desktop
must establish fresh/stale/unavailable presentation, retained selection/outcomes,
credits, accessibility and hide/quit lifecycle behavior using the assigned tests.
These are the existing mission requirements; policy/serialization tests alone
cannot establish them. No browser/GUI or screen-based review was performed here.

Calibration still needs independent timestamped native references beside actual
samples, with explanations for observed RAM definitions, APFS capacity/available
space, interval timing and process accounting discrepancies. Do not assert agreement
from formulas or synthetic fixtures. Separately, backend-gates must observe a
warmed, noninstrumented daemon for at least 300 monotonic seconds with the actual
desktop hidden: aggregate user+system CPU delta / elapsed * 100, one-core
denominator, below 0.5%; independent RSS below 50000000 bytes. Record environment,
warm-up, observation interval, actual bytes and display units. No such performance
or calibration measurements exist in this assessment, and passing that bounded
run would establish neither long-term behavior nor customer savings.

Executed read-only checks: `git rev-parse` for local/supplied revision trees;
`git diff --name-status <accepted> <snapshot>` for the 20 delivered paths;
`shasum -a 256` for both SRS copies; Python byte/hash comparisons for snapshot
paths, task-1's 248 inputs, receipts/logs and external lock records; and scoped
reads of the cited interfaces/tooling. No native gate was replayed. Full root Git
status was incomplete because of denied av.yaml access. Serena is configured in
`.agents/oma-config.cue`, but discovery exposed no applicable tools; only permitted
scoped reads of already located source files were used. `docs/decisions/index.md`
was absent; no existing decision or saved plan was edited. No run/claim identity
was injected, and none was invented or registered by this task.

The preserved normative prefix is exactly 23486 bytes with SHA-256
`6f135aa7900e6d6061e28e83e3be1bf4008f1ad9067a3bf8c3f3b8fa26c031af`.
Final handoff checks compare that prefix to the snapshot, require only this
appended assessment among the recorded input changes, and run
`git diff --check -- .arc-verify/host-meter/contract.md`. The complete file hash is
reported in the task-2 delivery and must be read back in the integrated worktree.
This file is frozen after this append for final receipt input hashing; no further
Technical Director edits are planned. Any later coordinated edit invalidates the
matching receipt and requires its normal freshness validation. Managed-job/vendor
attribution, native Windows, vendor hooks and full Meter/Phase 1 acceptance remain
outside this slice, as already recorded above.
