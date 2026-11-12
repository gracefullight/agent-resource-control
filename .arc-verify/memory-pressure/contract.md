# macOS memory-pressure admission contract

Accepted HEAD: `338631893b51b2e7cd4045e636bb1301ab4db955`.
This immutable task-1 handoff defines the first macOS slice of POL-001,
POL-003 and POL-004. It is an implementation contract, not acceptance evidence.
Authoritative requirements: `docs/requirements/ARC_SRS_2026-11-06.md`, SHA-256
`a8ef945251f9f3fd7c3e337d4efc77ff9914f95114212ca9e6c90ae2ce3e3d9e`,
especially sections 4.2, 11.2, 11.6, 11.9, 14.5, 14.6 and AT-POL-02.

## Boundary and initial policy

`arc-policy` depends only on existing `arc-core`, `arc-traffic` and serde types.
It owns pure evaluation and versioned decision contracts, without native I/O,
SQLite, Tokio workers, IPC dispatch or child ownership. Task 1 provides types
and validators; it does not claim to implement the evaluator or admission.
`arc-meter` retains one sampler, one native work slot and one latest snapshot.
The existing daemon SQLite worker remains the only writer. Native resource
sampling never occurs in a write transaction. No new registry dependency is
needed. The existing native ancestry/lifecycle checks retain their owners.

Policy schema is 1; policy version is `macos-memory-pressure-v1`; the conservative
initial profile is `macos-memory-l0-v1` (L0). Its exact existing Traffic limits:

| Class | Supported local commands | Window | CPU reservation | Memory reservation |
| --- | --- | ---: | ---: | ---: |
| Build | direct Cargo build | 1 | 2000 millicores | 2147483648 bytes |
| Test | direct Cargo test; supported one-shot Bun test | 1 | 1000 millicores | 1073741824 bytes |
| Lint | direct Cargo clippy and fmt | 1 | 1000 millicores | 1073741824 bytes |

Global reservation budget is 2000 millicores and 4294967296 bytes. All three
supported classes have positive memory reservations and are memory-intensive
for this slice. Pressure overlays the existing scheduler; it does not replace
FIFO, class windows, resource accounting, owner evidence or lineage validation.
Remote, UNKNOWN and unsupported invocations retain their existing bypass rules.
Running jobs and verified nested execution using an active existing reservation
are preserved. A caller-supplied lineage string alone is never verification.

## Frozen observation interface

`crates/arc-policy/src/observation.rs` exports `ObservationProvider`,
`ObservationInput`, `SamplerIdentity`, `ObservationCursor`, `SamplerStatus`,
`ObservationAvailability` and `ObservationSummary`. The provider invokes its
callback exactly once with a borrowed bounded cache view, even when missing.
It performs no native calls. Its callback holds the same publication fence used
by the sampler publisher. The daemon supplies the active sampler identity; it
must not adopt the identity of whatever sample happens to be cached.
The cache view includes independent sampler health (missing/running/failed/stopped).
The worker publishes failure/stop health even when retaining an older snapshot;
failed health selects explicit Failed/L0 immediately, rather than treating an
old cached Critical as healthy until its age expires. Initial running-without-
snapshot is Missing/L0. Unsupported platform remains Unsupported/L0.

`SamplerIdentity` combines the existing process identity (boot ID, PID, start
token), a positive generation and a positive monotonic epoch identifier. Both
tokens fit JavaScript safe integers. Replacing a sampler, including in the same
daemon process, changes generation; resetting the monotonic epoch changes its
identifier. These fields are internal/additive and do not alter Meter schema 1.
The publication's sampler identity must equal the active identity, and its
snapshot's daemon identity must equal that identity's daemon instance.

After acquiring the SQLite IMMEDIATE transaction, obtain the current cache view
and current monotonic time in that sampler's epoch. Validate the full existing
`MeterSnapshot::is_valid()`, supported platform, active/published identities,
positive safe sequence, cadence and clock arithmetic. A per-sampler high-water
cursor rejects sequence or sample-time regression. Equal sequence is allowed
only for the same immutable publication; changed content requires a new
sequence. Generation/epoch replacement clears the cursor and cached value
atomically, rather than accepting an older generation's reading.

Compute pressure age from the actual memory-pressure field:
`now_monotonic_ms - host.memory_pressure.observed_monotonic_ms`, with checked
subtraction. Validate observed <= sampled <= now, and the snapshot's recomputed
age == now - sampled. Fresh means **age <= cadence.stale_after_ms**; stale means
**age > cadence.stale_after_ms**, including a one-millisecond boundary test.
The existing cadence invariant is `stale_after_ms = 2 * interval_ms + 1000`:
idle intervals 10000..30000 ms, active/pressure intervals 1000..3000 ms.
UTC is presentation only; missing/backward/forward wall time cannot renew age.
A regressed, absent, overflowing or mismatched monotonic clock selects invalid
observation fallback. Receipt time, health responses and duplicate polls never
renew the measurement timestamp.

Fresh Available Critical selects `memory_hold` and `memory_critical`. Fresh
Normal/Warning selects L0 with `pressure_normal`/`pressure_warning`. Missing,
stale, failed, unsupported, denied or invalid observations select L0 and the
corresponding explicit reason. Invalid sampler identity and invalid clock use
`sampler_mismatch` and `clock_invalid`. No fallback invents Normal. A stale
Critical value may remain visible as stale, but cannot retain a current hold.
Unsupported hosts report unsupported; they are not native macOS evidence.

## Grant and launch ordering

Lock order is SQLite writer/IMMEDIATE lock, then the publication fence. The
publisher never waits on the SQLite writer and samples outside the cache lock.
Admission obtains the cache after writer backlog/lock acquisition, evaluates
new root eligibility, persists the decision transition and permit atomically,
and retains the fence through commit. A Critical publication linearized before
the grant must therefore prevent that grant. A publication racing with a grant
linearizes on one side of the same fence. No snapshot captured before queuing a
writer request may authorize a later grant.

Every enqueue, poll, retry and RESERVED-but-unspawned root launch checks this
policy. Existing running/reused permits remain valid. During fresh Critical,
new root jobs cannot receive new permits, and a reserved unspawned root stays
held without child execution. Preserve its reservation/generation and queue
ordering; do not mint another permit or silently invoke outage fallback.

The current CLI must perform a final `traffic.launch` handshake after its local
command/lineage/witness preparation and immediately around its one native spawn.
`LaunchRequest` binds job ID, authenticated observer and permit generation.
`LaunchResult::Held` keeps waiting and cancellation active; `Ready` is never
child-start proof. A verified nested reuse may return `Reused` under Critical.
Terminal/cancelled/missing or mismatched identities/generations cannot release
a launch. The gate is required independently of the new display capability;
absence of a policy display read cannot waive root admission.

For a root Ready response, retain the publication fence through the CLI's one
spawn attempt and matching `traffic.launch.ack` on that same authenticated
connection. End the SQLite transaction before waiting for that acknowledgement.
The fence has one absolute deadline: at most 2000 ms; when relying on a fresh
Available reading, cap it by the observation's remaining freshness. Explicit
L0 fallback uses the same 2000 ms coordination bound without inventing a fresh
observation deadline, so stale/missing Critical cannot keep a job held forever.
Reevaluate an exhausted fresh-reading deadline before releasing execution.
There is no TTL renewal or replay-triggered
spawn. `Started` contains captured child identity; `Unspawned` confirms no
spawn. Native child evidence/attach is collected outside write transactions.
The implementation must prevent execution after a lost/expired release (a
pre-exec native gate is suitable), reconcile uncertainty with the existing
witness, and never blindly retry a possible spawn. Do not hold a database
transaction across IPC or block the sampler's native work while waiting.

Critical is a published, validated observation state. Independently count actual
command executions; a grant, Ready message, pre-exec helper, or ACK alone is not
child-start evidence. Native acceptance must force Critical between reservation
and launch, and test loss/expiry/restart of launch coordination. Existing legacy
protocol-1 reply shapes remain decodable: pressure waits map to MEMORY_BUDGET,
and old status replies acquire no fields or variants. Existing raw legacy
enqueue/poll paths still enforce Critical at grant; tests must distinguish this
wire compatibility from the stronger current-CLI fenced launch guarantee.
The separate existing bounded daemon-outage fail-open remains unchanged and
explicitly reported; a healthy pressure hold is not an outage.

Composition order is classification/bypass, preservation of running state,
verified nested reservation reuse, Critical hold for every remaining supported
new root, then the existing FIFO/window/reservation evaluator. Critical holds
use `memory_pressure` even when another queue rule would also hold the job;
queue sequence and resource ownership remain unchanged.

## Additive read and wire contract

`crates/arc-ipc/src/policy.rs` owns these strict protocol-1 operations over the
existing owner-authenticated transport. No new authentication system is added.

| Operation | Exact payload | Result | Bound |
| --- | --- | --- | --- |
| `policy.capabilities` | null | capability=`policy.memory.v1`, launch_capability=`traffic.memory-launch.v1` | 512 bytes |
| `policy.current` | null | `EffectivePolicy` | 16384 bytes |
| `policy.job` | object with only `job_id` | `JobPolicyResult` bound to that ID | 512-byte query, 16384-byte result |
| `traffic.launch` | `LaunchRequest` | `LaunchResult` | 2048-byte query, 16384-byte result |
| `traffic.launch.ack` | `LaunchAck` matching the fenced request | same-connection acknowledgement | 2048 bytes |

Reads accept no path, history/rate, arbitrary SQL, PID query or pagination.
Struct-array alternatives, unknown fields, oversized values and unsafe numbers
are rejected. Read deadlines reuse the existing absolute deadline, clamped to
2000 ms, single flight and cancellation. Only explicit UNSUPPORTED_CAPABILITY
means unsupported; disconnected/unavailable, stale and malformed remain distinct.
No old hello, Traffic capability, Traffic job/status or Meter payload is changed.

`EffectivePolicy` contains schema/policy versions, profile, mode, reason, exact
limits and a current observation summary. `JobPolicyResult` separately contains
that current policy and the job's last committed `RecordedDecision` with its
original observation metadata/age and decision time. Decision availability is
available/not_recorded/expired/not_found. Do not infer a job reason from a host
snapshot or overwrite a historical decision's age on reads. The CLI status and
existing desktop selected-job detail present confirmed pressure waits, automatic
recovery and explicit L0 fallback; they preserve stale/disconnected retained
data and protocol-1 unsupported behavior. No new screen or ETA is required.

## Storage, transitions and retention

Migration `0005_memory_pressure` extends maintained `policy_states` and
`policy_decisions`; existing migrations 0001..0004 retain their bytes. Schema
registry and actual schema assertions advance together to version 5.
The canonical policy-state key is `memory_admission`.
`policy_states.state_json` is bounded to 16384 bytes and carries the latest
semantic effective state. Decisions add availability, pressure-field age,
bounded observation JSON, transition key, permit generation, explicit millisecond recording/expiry
times and indexes for last-job and bounded-expiry reads. Historic null metadata
does not become an invented observation. Existing `created_at` bytes/meaning
are preserved; new code uses `recorded_at_ms` for policy decision ordering.

Application-owned code, parameterized SQL and the single writer own all policy,
dedupe and retention; no trigger, stored function, new scheduler or parallel
audit subsystem is introduced. Commit transitions with permit changes.
The semantic transition key includes scope/job, policy version/profile, verdict,
reason, observation availability and sampler generation/epoch changes. Age,
poll time, ordinary sequence increments and equivalent fresh readings alone
do not append rows. A->B->A remains three transitions, not global-key dedupe.

Retain at most 64 recent transitions per job and 128 global transitions. Apply
the SRS 90-day job metadata retention from terminal lifecycle time for job-linked
decisions; active/queued/running and referenced reservations protect the latest
decision. Global transitions use their recording time for 90-day expiry.
Expiry and cap pruning are bounded batches of 128 on
the existing writer. Reads enforce expiry without starting another writer.
Resource raw/rollup retention (72 hours/30 days/1 year) does not justify persisting
a new resource sample stream. Audit count must remain unchanged under duplicate
polling, and cancellation/recovery/expiry must have deterministic tests.

## Ownership and handoff

Task 1 / backend-contracts is sole writer of shared manifests, Cargo.lock,
`arc-policy` contract modules, IPC policy/launch types and exports, migration 5
and registration/assertion updates, current coverage registries/receipt inputs,
and this contract until the explicit task-6 integration handoff. It reserves
the five Rust acceptance entry files below with compiling foundation checks;
these are not production pressure acceptance evidence.

Task 2 / backend-admission receives `arc-policy` evaluator implementation files
(new `src/evaluator.rs` and its own helpers/tests), the bounded Meter adapter,
transactional admission/launch coordination, daemon routing, policy persistence,
dedupe and retention. It extends `crates/arc-policy/tests/memory_pressure.rs`,
`crates/arc-storage/tests/memory_pressure.rs` and
`crates/arc-daemon/tests/memory_pressure.rs`. Request shared export/manifest
changes through backend-contracts; do not independently rewrite frozen types.
Implement `arc_policy::evaluator::evaluate` with the compiled
`EvaluateMemory = for<'a> fn(ObservationInput<'a>) -> EffectivePolicy` signature.
The pre-registered evaluator module contains this type only, with no stub verdict.
The registered `crates/arc-storage/src/policy.rs` implementation module also
passes to task 2; task 1 retains the storage crate's shared root exports.

Task 3 / backend-surfaces receives CLI launch/status consumers, the production
desktop bridge and existing job-detail presentation/tests. It extends
`crates/arc-cli/tests/memory_pressure.rs` and
`apps/desktop/src-tauri/tests/memory_pressure.rs`. Read/launch types above are its
dependency. Task 2 owns the daemon side of the fenced launch protocol; task 3
owns its matching CLI side. Interface conflicts go to the Chief with exact
paths and evidence, without changing the saved task or acceptance criteria.

Backend integration tasks own production fixtures/native gates and gate reports.
Independent oma-qa is read-only for product/tests/configuration and runs no
native gates or coverage. All new source/test files remain below 500 lines.

## Verification and compact evidence

The existing coverage runner's `--preflight-only` executes its tool/LLVM,
configuration, offline metadata, workspace membership and production inventory
preflight, then exits before any llvm-cov collection. Raw task-1 commands,
versions, SDK/dependency results, initial migration/history hashes and handoff
digests live under `target/memory-pressure/task-1/`. Provision only missing
prerequisites using maintained setup, preserving pins and the lockfile.
No coverage is collected in task 1.

The six mandated gates retain these commands:

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace`
4. `bun run test:coverage`
5. `./scripts/harness/validate.sh`, reusing the same validated receipt via ARC_COVERAGE_RECEIPT
6. This explicit active Bun command (no unqualified test discovery):

```sh
bun test apps/desktop/tests/bridge-client.test.ts apps/desktop/tests/contracts.test.ts apps/desktop/tests/credits.test.ts apps/desktop/tests/dashboard.test.ts apps/desktop/tests/lifecycle.test.ts apps/desktop/tests/meter.test.ts scripts/coverage/accounting.test.ts scripts/coverage/desktop-foundation.test.ts scripts/coverage/gate.test.ts scripts/coverage/hygiene.test.ts scripts/coverage/terminal-outcomes.test.ts scripts/coverage/traffic-accounting.test.ts scripts/harness/equivalence.test.ts scripts/harness/traffic-targets.test.ts scripts/harness/coverage-reuse.test.ts
```

Affected desktop typecheck/check/build/Tauri smoke remain backend-recorded
requirements. Verification wrappers allow at least 7200000 ms per check; the
mission absolute deadline and cancellation still apply. No Playwright, browser
downloads, individual screen checks, deployment or installed-daemon restart.
No configured external target was supplied in task 1's authorized runtime
information. `av.yaml` access is denied and is not bypassed; the Chief must
reconcile any additional target requirements exposed by its runtime.

Current coverage inventory is eleven crates, retaining the integer 80%
production-line floor per crate including arc-policy; historical inventories,
baselines and reports remain unchanged. Source identity includes this frozen
contract and the authoritative SRS using maintained coverage identity/reuse
helpers; generated results are excluded to avoid circular hashes. At final
collection, `gates.json.inputs` is the sole canonical final mission digest and
raw input manifest. Other reports refer to that identity, not copied inventories.
Collect once for the final successful inputs; amendment 11 allows documented
corrective recovery with preserved failed/intermediate receipts. Task-1 file
handoff digests are not competing final mission input identities.

Exactly five canonical tracked evidence files are permitted under
`.arc-verify/memory-pressure/`: `contract.md`, `gates.json`, `integration.json`,
`qa.json`, `final.md`, totaling at most 300000 bytes. Raw logs/receipts stay under
`target/memory-pressure/`. Freeze this contract before final collection.
Keep historical evidence and `scripts/job-meter/evidence.test.ts` unchanged;
do not run that historical validator or recertify earlier missions.

Integration must prove an explicitly injected production-daemon
Normal->Critical->Normal sequence; zero new root permits and command starts
through Critical including an existing child's exit; unchanged running outcome;
and exactly one held-job execution after recovery. Independently reconcile
SQLite, CLI and the production desktop bridge. Required deterministic cases:
writer backlog, exact freshness, stale/failed/missing/unsupported fallback,
sampler restart, clock changes, duplicate polls, queued cancellation, daemon
recovery, remote/UNKNOWN bypass, nested reservation reuse, reserved launch and
bounded outage fail-open. Audit dedupe/retention and bounded authenticated reads
must be verified. Label a separate actual SystemSource smoke with its actual
observations; injected Critical never establishes native Critical.

Outstanding beyond this slice: CPU/disk pressure control, statistical learning,
vendor hooks, native Windows and full policy acceptance. No throughput, savings,
commercial, user-study or native Critical result is established by this contract.
