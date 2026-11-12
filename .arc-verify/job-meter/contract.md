# Managed-job Meter contract v1 — finalized technical decision

Status: **finalized for dependent implementation**, task-4, document revision 3,
wire schema 1, 2026-11-12.
Authority: the saved task-4 assignment transfers this file from backend-native
to the Technical Director. This decision fixes the shared interpretation for
MTR-002, MTR-003, MTR-005 and AT-MTR-01; it does not certify mission acceptance.
The compiling interfaces and native feasibility come from task-1. Task-4 used
**oma-architecture**, Recommendation/ADR mode, to compare three routes and audit
those delivered interfaces. Task-1 used **oma-backend** and **oma-db**. Task-4
changes only this contract; product code, native.json and other Actors' files
remain read-only. No build, test, native workload, coverage or screen check ran
in task-4. The owned contract is also this task's material architecture report;
no separate ADR, registry change or mission-state mutation is needed.

## Source provenance and readiness

- Accepted committed baseline: `18091eeb1ac0232dfdda4a937113fa3bd149b220`.
- Its exact product tree: `c1ed103170701e587991486e318eea47d42d709b`.
- Actor snapshot commit: `7d3fee520f1c9e25d35c3355ba13f5a7f74996d3`;
  snapshot tree is the same product tree, with no initial source overlay.
- Snapshot ref: `refs/agent-valley/task-evidence/903a567164e5b961-7afaa346b4bf92bf-1/snapshot`.
- Worker ref: `refs/agent-valley/task-evidence/903a567164e5b961-7afaa346b4bf92bf-1/worker`.
- In the originating task-1 clone both symbolic refs were unavailable; preserve
  that historical diagnostic in native.json. In this task-4 mission worktree,
  `git rev-parse --verify` resolves **both** refs to `7d3fee520f1c9e25d35c3355ba13f5a7f74996d3`.
  Their shared baseline tree is not a committed implementation tree. Current
  HEAD is the accepted commit above; the task-1 implementation is a staged
  overlay. All **22** `native.json#/sourceInputs` SHA-256 values match current
  files, including shared wire contracts, tests, manifests, lock and native
  helper inputs.
  This byte comparison, rather than ref equality, binds the delivered interfaces
  to the task-1 receipts. Task-1's original branch was
  `av-task/903a567164e5b961-7afaa346b4bf92bf-1`.
- Task-1's initial scoped diffs showed no existing task edits. Task-4's initial
  status contains 21 preceding staged paths (including the three Directors'
  documents); preserve them. Git emitted `av.yaml: Operation not permitted`;
  a whole-checkout clean claim is not made.
- Task-1 native readback on 2026-11-12: macOS 27.2.1 / Darwin 27.2.0 arm64,
  8 logical CPUs, 16 GiB RAM, Apple Clang 21.0.0, an installed Xcode macOS SDK,
  Rust/Cargo 1.95.0, and Bun 1.4.4. Actual compilation/tests below establish
  readiness for task-1 inputs. Task-4 independently read `rustc --version`,
  `cargo --version`, `bun --version`, `clang --version`, `uname -sm` and
  `sw_vers -productVersion`: all returned 0 and confirmed the same versions and
  native architecture. SDK usability, LLVM coverage tools, desktop checks and
  final combined compilation were not re-established by version reads.
- Serena is configured but no Serena tools were exposed. Only scoped reads of
  already located source files were used; source-search completeness is not
  claimed. No harness validator source was inspected.
- **Container/service observation: not applicable.** The supplied task-4 repair
  clarification establishes that no container/service observation policy is
  configured and that `av.yaml` denial is not a blocker. No container-policy
  readback or container/service check is required for this mission. The earlier
  unavailable-policy observation in native.json remains historical evidence;
  it creates no unfinished acceptance work.

Original SHA-256 digests were collected from the accepted commit **before any
implementation** and compared with the Actor's corresponding current files:

| Immutable migration | Accepted/current SHA-256 |
| --- | --- |
| `crates/arc-storage/migrations/0001_phase0.sql` | `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384` |
| `crates/arc-storage/migrations/0002_managed_exec.sql` | `464a1c88270f01a56ccab6610f87eff7648e0640f276ebce53b8a97026c50814` |
| `crates/arc-storage/migrations/0003_traffic.sql` | `1f4522e3f27c124aea7127d92434c7bd22be36168fef0a4bf665f8cf16debe90` |

Preserve those files byte-for-byte. The store owner adds `0004`, rather than
editing prior migration bytes or bypassing migration checksums.

## Architecture decision and option comparison

Problem: explain one CLI-owned execution's cumulative CPU and elapsed duration
without losing short-lived reaped descendants, charging unrelated shell peers,
or counting a nested inclusive total twice. The important boundaries are native
ownership, immutable report delivery and single-writer persistence. Existing
Rust 1.95/libc/Tokio/rusqlite, arc-platform, arc-meter, protocol-1 IPC and
React/Tauri/Bun remain the stack. The only delivered dependency change is the
workspace edge `arc-cli -> arc-platform`, also added to Cargo.lock; no dependency
version, external package, service or Bun lock changed.

| Option | Short-lived descendants / ownership | Implementation, operating and maintenance cost | Decision |
| --- | --- | --- | --- |
| PID-specific terminal `wait4` in the existing CLI waiter | Captures the owned child's final user/system counters and usage propagated by waits, including the probe's eight reaped grandchildren. A positive child PID excludes unrelated pipeline peers. Cannot establish coverage of detached/unwaited work. | Narrow native adapter and one terminal report; existing reaper/signal policy survives. No periodic collector or extra dependency. Actual production overhead is unmeasured; cost advantage is structural. | **Select for this scope**, supported by the actual native probe and compiling interface receipts. |
| Periodic PID or process-group accumulation | A process may start and be reaped between samples. Group membership is not ownership, and sampled children plus inclusive parent counters can overlap. Identity tokens mitigate PID reuse but do not recover missed terminal counters. | Requires polling, identity histories, counter-delta/reparent rules and deduplication. More state and runtime work; higher rate reduces, but cannot eliminate, the sampling gap. No measured accuracy/overhead advantage was supplied. | Reject as the authoritative total. Preserve the separate host snapshot contract. |
| Live tree collection with native lifecycle events | Could broaden lineage visibility, but tree enumeration alone still races short-lived exits; complete accounting needs terminal-event coverage, identity/reparent handling and detached-work policy. | New event/lifecycle machinery and recovery obligations; greater platform-specific implementation and team complexity. Permissions, delivery guarantees and calibration have not been established. | Defer; live process-tree metering remains outstanding. |

The chosen route reuses the existing flow: CLI sole reaper -> authenticated IPC
-> finite DbWorker -> existing SQLite connection; CLI/production desktop read
the same summary operation. Module/service ownership does not move. Retaining
one native adapter and the existing wire validators limits future changes to
the relevant boundary. Any future live-tree collector must introduce its own
scope/quality contract; it cannot silently replace or augment this inclusive
terminal value. No new aggregation API is introduced.

Prior context inspected: `docs/decisions/README.md` and
`adr-runtime-execution-and-state-ownership.md`. The latter concerns AV runtime
ownership and remains Proposed; it supplies no ARC accounting schema to
supersede. No prior ARC job-meter ADR was identified in the located decision
index. `.agents/results/architecture/` and legacy design directories were absent.
The saved Director advice is advisory evidence; this decision is supported by
the current interfaces/probe, not those recommendations alone.

## Feasibility and native boundary

`scripts/job-meter/native-workload.c` runs two CPU workers for a requested ten
seconds. Each worker creates and explicitly reaps four short-lived CPU children;
the root explicitly reaps both workers. Identities include actual PIDs, birth
tokens and a boot UUID. The C reference uses raw `getrusage(RUSAGE_SELF)` plus
`getrusage(RUSAGE_CHILDREN)` and independent integer-microsecond arithmetic after
those waits. It shares neither Rust's timeval conversion nor its outer reaper.

`crates/arc-platform/examples/job_meter_probe.rs` owns that same workload child,
captures its identity, and uses the production native interface to obtain its
terminal usage. `native.json#/feasibility` records raw reference and terminal
values, identities, measured elapsed time, absolute error, tolerance, and scope.
Reference sampling precedes a small amount of final printing/exit CPU included
by outer wait4; this residual must remain within the engineering tolerance.
No nominal twenty-core-second value is substituted for a native measurement.

The [archived Apple wait manual](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/wait.2.html)
documents positive-PID child selection and terminal resource statistics;
[Apple's getrusage manual](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/getrusage.2.html)
defines user/system usage and self/terminated-child measurements. Both were
read during task-4. These archived manuals motivate the interface; the current
installed-platform probe tests actual waited-descendant propagation. This
demonstrates the controlled
**native_waited_descendants** scope, not arbitrary live process-tree coverage.

Task-4 independently recomputed the stored raw values, without running a probe:

| Observation | User core-seconds | System core-seconds | Total core-seconds |
| --- | ---: | ---: | ---: |
| PID-specific terminal wait | 19.744362 | 0.054723 | 19.799085 |
| C self + waited children | 19.744334 | 0.054675 | 19.799009 |

The absolute difference is **0.000076**, below
`max(0.1, 0.05 * 19.799009) = 0.98995045` core-seconds. Root PID 3359 and
start token 1794451156878947 match between observations; boot UUIDs match after
canonical lowercase conversion. Two distinct workers each identify four
distinct reaped short-lived children. This is a scope-matched feasibility pass,
not persisted AT-MTR-01 acceptance: `native.json#/calibration`, `/matrix`,
`/persistence` and `/compatibility` remain `not_run` with null result flags.

Stable exported native API, in `arc_platform::child_wait` (Unix):

```rust
pub fn wait_owned_child(pid: u32, mode: WaitMode)
    -> std::io::Result<Option<ChildWaitEvent>>;
// WaitMode::{Blocking, Poll}
// ChildWaitEvent { status: ExitStatus,
//                  terminal_usage: Option<Result<TerminalUsage, UsageError>> }
// TerminalUsage { raw_user: CpuTimeval, raw_system: CpuTimeval,
//   user_core_seconds: f64, system_core_seconds: f64, total_core_seconds: f64 }
// CpuTimeval { seconds: i64, microseconds: i64 }
```

Only positive individual child PIDs are accepted. Poll uses WNOHANG; both modes
preserve WUNTRACED/WCONTINUED status events. Usage is consumed only for native
exit/signal events; conversion failure leaves the original terminal status
available. EINTR is returned to the existing signal owner. The existing CLI
waiter remains the sole reaper and retains its signal mask, PID forwarding,
terminal/pipeline policy and exit status/reraise behavior. Do not add another
waiter or call group-wide wait, process-group aggregation, or RUSAGE_CHILDREN
for the entire CLI process as job attribution.

## Frozen shared wire model

`arc_core::job_meter` owns `JOB_METER_SCHEMA_VERSION = 1`,
`MAX_JOB_METER_BYTES = 16384`, `JOB_METADATA_RETENTION_MS = 7776000000`, and the
types below. The existing `arc_core::meter::MeterSnapshot` is unchanged.
Wire names are snake_case except terminal-outcome tags; job, report and root
job IDs have the existing lifecycle grammar of exactly 32 lowercase hexadecimal
characters. Process start tokens remain canonical positive decimal u64 strings,
and boot IDs retain ProcessIdentity's canonical UUID validation. Numbers must be
finite,
nonnegative and within JavaScript's safe-number bound. Nullable fields are
required keys whose unknown value is JSON null, not omitted or zero.

| Type / field | Meaning and invariants |
| --- | --- |
| `JobMeterReport.schema_version, report_id, job_id` | Version 1; random report ID retained across every retry; durable job key. |
| `observer, child: ProcessIdentity` | Existing PID + decimal start token + boot UUID; distinct PIDs, same boot. These tuples are evidence to bind, not authorization by themselves. |
| `cpu_core_seconds: CpuCoreSeconds {user, system, total}` | Nullable cumulative user and system CPU, with total equal to their sum when both are known (validator tolerance `max(1e-9, total * f64::EPSILON * 2)`). No CPU-count normalization. Total must be null when either component is unknown. Raw timeval components are retained in native evidence, not added to this persisted wire type. |
| `run_elapsed: RunElapsed {seconds, clock, includes_stopped}` | Nullable elapsed seconds, `clock: monotonic`, `includes_stopped: true`. Timer begins immediately around native spawn and ends at terminal observation, before completion/report I/O. Includes stopped intervals; excludes queueing. Never compare daemon epochs across restart. |
| `attribution_basis` | `native_owned_child_wait4`; no sampling or admission estimate substituted. |
| `scope` | `native_waited_descendants`: owned child plus usage propagated by waits. It does not claim all descendants, detached work, or process-group membership. |
| `completeness` | `complete`, `partial`, or `unknown` for the stated scope, independently of exit success. |
| `missing_coverage` | At most eight unique enum reasons: `unobserved_descendants`, `detached_descendants`, `native_usage_unavailable`, `run_duration_unavailable`, `lineage_unverified`, `interrupted`, `unsupported`. No raw diagnostic text. |
| `accounting_relation` | Tagged `canonical_root {root_job_id}`, `included_in_root {root_job_id}`, or `unverified {}`. Root equals this job only for canonical roots. |
| `outcome` | `EXITED {code: u8}` or `SIGNALED {signal: 1..64, core_dumped: bool}`; exactly matches durable lifecycle outcome. Spawn failure does not manufacture child CPU. |
| `terminated_at_ms` | Nullable client wall presentation time only; not the run clock or retention anchor. |
| `JobMeterSummary {schema_version, job_id, accounting, queue_wait}` | Correlated point lookup, including a separate queue measurement. |
| `SummaryAccounting` | Tagged by `state`: `recorded {report}`, `unknown {reason}`, `missing {}`, `expired {}`. Unknown reasons: `legacy`, `interrupted`, `unavailable`, `not_terminated`, `unsupported`. |
| `QueueWait {seconds, clock, basis}` | Scheduler-derived QUEUED-to-RESERVED seconds with `daemon_wall_clock` / `queued_to_reserved`; new lifecycle-only jobs have established 0 with `not_applicable` / `lifecycle_only`; unavailable/legacy intervals have null with `unknown` / `unavailable`. |

`QueueWait::from_wall_timestamps` returns unknown on missing, reversed or invalid
wall timestamps. Wall-clock discontinuity remains a limitation of this explicitly
labeled scheduling observation; never infer queue time by subtracting run time
or mixing epochs. Legacy data must not be reclassified as newly measured zero.

Complete requires both CPU components, their valid sum, known run duration,
verified lineage and no missing-coverage reason. An opaque command cannot prove
that all possible descendants were accounted for: default it to Partial with
`unobserved_descendants`, retaining the measured waited contribution. The
controlled workload may establish complete coverage of its explicitly waited
scope. Missing native CPU or run duration requires an appropriate reason.
Unknown CPU values remain null even when elapsed time is independently known.

`Completeness::Unknown` in a recorded report requires all three CPU fields null
and a nonempty reason list; run duration may still be known. Partial requires
at least one missing-coverage reason. `native_usage_unavailable` requires a null
CPU total; `run_duration_unavailable` requires null run seconds. An unverified
relation requires `lineage_unverified` and cannot be Complete; a verified root
or nested relation cannot carry that reason. These are existing validator rules,
not new consumer interpretations. Native zero is a valid measurement, never a
fallback for absence. Cumulative core-seconds may exceed elapsed seconds with
parallel workers; this contract provides no per-job utilization percentage.

Canonical root policy: reuse verified Traffic root relationships and native
ancestry, including lifecycle-only nesting in the CLI implementation. An
inclusive nested observation remains `included_in_root`, with its root job ID
and `AccountingRelation::is_additive() == false`. Unverified lineage remains
non-additive and cannot be Complete. Never add nested inclusive totals or live
samples onto another inclusive total. Exclude unrelated shell pipeline peers.
This slice exposes individual summaries, not a new aggregation API.

### Canonical root and cross-mode nesting rules

| Execution relationship | Required record interpretation |
| --- | --- |
| Proven top-level job R | `canonical_root {root_job_id: R}`. Its CPU is the one terminal native observation of its own child, not a sum of registered descendants. |
| Verified nested N below R, scheduled or lifecycle-only, including mixed modes | `included_in_root {root_job_id: R}`, preserving N's own job/report/outcome. Resolve chains to the canonical root, not an intermediate N. `is_additive()` is false even if N has a Complete scoped measurement. |
| Missing, stale, reparented or contradictory lineage evidence | `unverified {}` with `lineage_unverified`, Partial or Unknown and no additive claim. Never promote an uncertain nested job to a new canonical root merely because its root is unavailable. |

Reuse verified Traffic root relationships when present. A Traffic root's nullable
database parent/root field is mapped to its **own durable job ID** for a canonical
report; it is not serialized as a null root. Native ancestry must also cover
lifecycle-only nesting, for which the baseline creates no lineage sidecar.
Neither group IDs, a bare inherited environment/context token nor matching wall
times establish ancestry. The CLI's private ancestry/sidecar observation and the
daemon's independently verified durable relation must agree. Gather live native
evidence while registration/attachment can verify observer and child identities;
preserve the relation for later terminal reports. The store owner records the
binding through additive storage, not new fields on legacy start/complete
payloads. Any internal lineage handoff needed by the native/store implementations
must use that actual artifact dependency; the report's assertion is not proof.

This relation prevents overlapping totals from being added. A verified ancestor
relationship alone does not prove every nested descendant was waited for. The
root remains Partial when propagation is unproven; detached/unobserved work
does not become a complete tree through a root link. Never reconstruct a missing
root CPU value by summing nested reports, subtract N from R to invent exclusive
CPU, or call a partial canonical value a complete process-tree total. The probe
establishes completeness only for its controlled, explicitly waited scope.

### Run and queue clocks

The CLI captures one `Instant` immediately before its sole spawn attempt and
freezes elapsed seconds at the terminal event returned by the sole waiter. It
does not restart this timer after attachment, SIGCONT or a metadata retry.
SIGSTOP-to-SIGCONT time remains included (`includes_stopped: true`); stop and
continue events have no terminal CPU report. Capture the terminal endpoint
before completion/report traffic. Observation delay is not a second source of
CPU and must not be corrected with wall timestamps. A spawn failure produces
no JobMeterReport because there was no owned native child.

Queue wait is independent: for scheduled jobs use durable
`traffic_entries.queued_at_ms -> granted_at_ms` (reservation), in milliseconds
converted once to seconds with `QueueWait::from_wall_timestamps`. Reservation
through spawn/attachment is not scheduler queue wait. Restart does not invalidate
two persisted wall timestamps or authorize subtraction of monotonic values from
different processes/epochs. Reversed/missing/invalid facts remain unknown;
undetectable wall-clock jumps remain a limitation of the labeled wall clock.
Only affirmative new lifecycle-only facts justify `QueueWait::lifecycle_only()`
and zero. Absence of a Traffic row in an old database is insufficient: the store
must distinguish pre-meter rows from measurement-aware lifecycle registrations.

Stable synthetic fixture for all three implementation consumers:
`crates/arc-core/tests/fixtures/job-meter-report.json`. Its fake identities and
numbers are **test data**, not native acceptance evidence. Core and IPC tests
decode this exact fixture. Construct a summary with `SummaryAccounting::Recorded`
and a separately chosen `QueueWait`; use `QueueWait::unknown()` for missing and
expired states. `JobMeterReport::is_valid()` and `JobMeterSummary::is_valid()`
validate the typed model; IPC decoders additionally enforce bounded object wire
shape and reject positional struct arrays and unknown fields.

## Additive protocol-1 interfaces

The exports are available through both `arc_ipc::meter::job` and
`arc_ipc::meter`. No protocol bump, host-snapshot payload change, or completion
payload extension is made. Old daemon dispatch returns UNSUPPORTED_CAPABILITY.
The helpers do not install new daemon handlers.

```rust
// All decode/build errors are &'static str; response errors are local enums.
report_request(request_id, &JobMeterReport) -> Result<Request, &'static str>
summary_request(request_id, job_id) -> Result<Request, &'static str>
decode_report(&Value) -> Result<JobMeterReport, &'static str>
decode_summary_request(&Value) -> Result<SummaryRequest, &'static str>
decode_summary(&Value, job_id: &str) -> Result<JobMeterSummary, &'static str>
decode_report_receipt(&Value, &JobMeterReport) -> Result<ReportReceipt, &'static str>
JobMeterRequest::decode(operation, &Value) -> Option<Result<JobMeterRequest, &'static str>>
matches_terminal_outcome(&JobMeterReport, &arc_ipc::jobs::Outcome) -> bool
report_response(&Request, &ReportReceipt) -> Response
summary_response(&Request, &JobMeterSummary) -> Response
interpret_report(&Request, &Response) -> Result<ReportReceipt, JobMeterError>
interpret_summary(&Request, &Response) -> Result<JobMeterSummary, JobMeterError>
// Unix, using the existing authenticated client:
report_until(socket, request_id, &report, absolute_deadline, cancelled,
             &mut possibly_committed) -> Result<ReportReceipt, client::Failure>
summary_until(socket, request_id, job_id, absolute_deadline, cancelled)
    -> Result<JobMeterSummary, client::Failure>
```

- `meter.report` payload is the report object itself; `meter.summary` payload is
  exactly `{ "job_id": "<32 lowercase hex>" }`.
- Report success is `ReportReceipt {schema_version: 1, job_id, report_id,
  disposition: "accepted" | "unchanged"}`. Summary success is JobMeterSummary.
- Static errors: `METER_REPORT_CONFLICT`, `METER_IDENTITY_MISMATCH`,
  `METER_NOT_TERMINAL`, existing `INVALID_REQUEST`, `METER_UNAVAILABLE`,
  `UNSUPPORTED_CAPABILITY`, `PROTOCOL_INCOMPATIBLE`, and `DB_BUSY` as appropriate.
- Typed envelope interpretation distinguishes conflict, identity mismatch,
  not-terminal, unavailable, unsupported, protocol and invalid reply. Existing
  `client::Failure` keeps its established variants: server rejections map to
  Rejected; unsupported/unavailable remain distinct. No raw peer messages leak.
- The existing owner-authenticated Unix transport is reused. Reporting clamps
  the caller's absolute deadline to at most 500 ms **after termination**;
  summary reading clamps to at most 2000 ms. Retries use the same request/report
  identity and the same deadline, including DB_BUSY backoff. Cancellation wins.
  Existing pre-spawn metadata's shared two-second limit is unchanged. No new
  execution, daemon launch, detached uploader or unbounded queue is introduced.
- A malformed report ACK preserves `possibly_committed`; lost-ACK retries send
  identical metadata. Exhaustion cannot trigger command relaunch or discard an
  already persisted report. Reconciliation is a summary read by the same job ID.

The helper clamps to `min(caller_deadline, now + 500 ms)`; it has no terminal
timestamp parameter. Therefore the CLI must create and retain one report deadline
at terminal observation (`terminal_observed + 500 ms`) and pass it unchanged.
Completion remains the existing bounded lifecycle operation and precedes report
acceptance; time spent awaiting its ACK consumes this report window. If the
window is exhausted, skip reporting and preserve unknown/unconfirmed metadata.
Do not renew the report window after a slow completion, DB_BUSY or lost ACK.
Summary reads use their separate two-second absolute budget and cannot execute
or relaunch work. Native terminal status always controls exit/reraise even when
reporting, validation or persistence fails.

## Authoritative report acceptance and retention

The store/daemon consumer must enforce these checks within the existing bounded
single SQLite writer. Helpers' schema validation is not a substitute:

1. Authenticate the accepted local peer using the existing UID transport. Bind
   the native peer's process identity to the durable observer where the platform
   exposes it. Validate the complete report before worker admission.
2. Load the durable job. Match its observer tuple, previously attached owned child
   tuple, terminal outcome and verified accounting root. Never require a fresh
   live child probe after terminal wait has reaped it, and never accept a PID or
   process-group membership alone as identity. A missing/unbound job cannot
   acquire ownership by reporting. Root claims must match verified lineage;
   unknown lineage may only be unverified/non-additive.
3. Require durable terminal observation; reject an unconfirmed running job with
   METER_NOT_TERMINAL. Preserve completion's existing request shape and workflow.
   A lost completion/report during outage leaves unknown/partial accounting,
   rather than inventing a terminal total or rerunning the command.
4. An unknown/legacy/interrupted placeholder has no accepted report identity and
   can refine once when supported terminal evidence arrives. Prefer deriving
   placeholders at read time; do not let placeholder materialization prevent a
   genuine report. Spawn-failed jobs can have a known lifecycle-only queue zero
   but unknown/unavailable child CPU.
5. Atomically accept one immutable report per job. Exact typed payload and report
   ID retry is a no-op returning `unchanged`. Different report ID, changed CPU,
   scope, relation, identity, timing or outcome is a conflict; never overwrite the
   first accepted report. Preserve this decision across daemon restart/commit
   before lost ACK. No per-sample table or second writer/connection is needed.
6. Queue wait is derived from authoritative scheduler facts by the writer/read
   path, not accepted from a reporting client. It stays independent of CPU/run.

**90-day metadata retention:** anchor summary expiry to the first durable
terminal lifecycle timestamp (`completed_at_ms` for observed terminal outcomes;
the first persisted terminalization timestamp for interrupted/outcome-unknown
jobs). Preserve that anchor once set. Never anchor it to client wall time,
latest summary read, report retry, daemon uptime, or late report arrival.
At `anchor + JOB_METADATA_RETENTION_MS`, make old summaries unavailable and
remove expired payloads through bounded writer maintenance (up to 128 job keys
per batch). Protect nonterminal jobs, active leases/permits and verified live
executions; do not delete independently owned lifecycle/Traffic dependencies.
Validate expiry on reads as well as cleanup, so delayed maintenance cannot expose
expired measurements. Use SQL parameters, PK/UNIQUE job key, FK integrity and an
expiry lookup index. Keep retention decisions in application code and execute
bounded set-based SQL within the existing transaction/connection; no policy
trigger, new service or side effect is needed. Store owner chooses the additive
physical representation and tests crash/reopen and deletion boundaries.

Missing is not proof of expiry or nonexecution. Emit `expired` only when durable
expiry evidence exists; otherwise use `missing`. Do not create fake expiry proof
from a failed fetch or infer that absent legacy CPU was zero. The database owner
must test cutoff equality, active protection, bounded maintenance, foreign keys
and retry immutability independently of CLI and desktop projection.

Retention also applies to late delivery: expiry is checked before accepting a
first report or retry. Once terminal metadata has reached its protected cutoff,
do not resurrect or extend a summary by reinserting a deleted report. Return
`METER_UNAVAILABLE` for an expired report write; a summary can return `expired`
only while its durable cutoff/expiry evidence remains available. If that proof
is later absent, use `missing`. Keep the report payload separate from minimal
existing job/lineage references required by active jobs; cleanup of this slice
must not cascade-delete other owners' lifecycle, permit or Traffic records.

### Unknown placeholders and restart reconciliation

Resolve known expiry first, then an accepted unexpired report, then durable
interruption evidence before the other no-report placeholders. A successful
lookup's unknown reason is a durable-data classification, not a transport error.

| Durable/read condition | `SummaryAccounting` and recovery |
| --- | --- |
| Valid unexpired accepted report | `recorded {report}`, retaining original quality/root after restart. A stale UI retrieval does not change measurement completeness. |
| Known active/nonterminal job, no report and no durable interruption | `unknown {reason: not_terminated}`; no live extrapolation. |
| Pre-meter terminal metadata, no report | `unknown {reason: legacy}` with unknown queue; never reclassify legacy absence as a newly measured zero. |
| Observer lost/interrupted without a valid report | `unknown {reason: interrupted}`. Do not manufacture CPU or a terminal outcome from liveness loss. |
| New terminal job with no usable report, including spawn failure | `unknown {reason: unavailable}`; queue remains independently known or unknown. |
| Known durable expiry / no durable job or expiry proof | `expired {}` / `missing {}` respectively; **both require `QueueWait::unknown()`** under the delivered validator. |
| Old daemon does not implement this operation | Preserve transport `Unsupported`; a frontend may describe Unknown/unsupported, but must not fabricate a successful summary response. |

These read-time placeholders contain no report ID and cannot lock out a later
valid, unexpired terminal report. A recorded report with Unknown quality is still
immutable; it is not such a placeholder. Daemon restart preserves accepted report
and root identities, terminal/outcome binding, cutoff and conflict decisions.
For a commit with a lost ACK, a bounded retry of the identical report returns
`unchanged`; independent `meter.summary` reconciliation checks the same job and
report. A lost completion ACK is resolved against the durable terminal state:
report acceptance may succeed if completion committed, but cannot create that
terminal state itself. If it did not commit, return `METER_NOT_TERMINAL` and leave
measurement unknown. Do not allocate another job/permit, rerun the child, start a
background uploader or replay a report after its deadline to repair tracking.

## Ownership and handoff

| Owner | Files / output |
| --- | --- |
| backend-native, task-1 | Initial contract and native.json; shared `crates/arc-core/src/job_meter*`; additive `crates/arc-ipc/src/meter/job*` plus client operation classification; `crates/arc-platform/src/child_wait*`; feasibility example; `scripts/job-meter/native-workload.c`; shared fixture. This document transfers through task-1 -> task-4; product interfaces/native.json do not. |
| Technical Director, task-4 | Finalize only `.arc-verify/job-meter/contract.md` after reading compiling task-1 interfaces and receipts. Product code, native.json, experience.md and value.md remain read-only. No build/test/native gate ownership transfers. |
| backend-native, manifest writer | Sole Cargo/package manifest and lock writer during contract preparation and any coordinated subsequent change. Only current edge is arc-cli -> arc-platform; Cargo.lock adds that edge with no version change. No package/Bun lock changes. |
| backend-store, dependent task | Additive 0004 migration, storage repository/service and tests; authenticated daemon handler and single-worker routing; idempotence, retention and restart receipts. Consume frozen types without changing shared interfaces unilaterally. |
| Design Director, task-2; Marketing Director, task-3 | Delivered experience.md and value.md respectively. These are consumer inputs, not implementation evidence. |
| backend-desktop, dependent task | Production Tauri registration/bridge, typed TS validation/client, selected-job presentation and deterministic tests. Consume experience.md, summary helpers and fixture; preserve selection correlation, stale data, keyboard behavior and redaction. |
| backend-native, dependent implementation/acceptance | Integrate PID-only wait into CLI, report builder and lifecycle-only ancestry, `arcctl meter summary --job <JOB_ID> --json`, native matrix/reference helpers, combined native.json; one small `scripts/job-meter/evidence.test.ts`; gates.json and final.md. |
| independent reviewer | Read-only oma-qa product review and qa.json after backend verification; no native gates or coverage recollection. |
| Chief | Coordinate immutable assignment dependencies, manifest ownership and final reconciliation/acceptance. Apply the supplied clarification that container/service observation is not applicable. |

The task-1 artifact dependency is the ownership handoff: consumers start only
after receiving these compiling types/fixture and the native feasibility receipt.
Any necessary signature/schema or shared manifest change returns to the single
writer through an actual dependent artifact. This table documents coordination;
it does not change saved Actor assignments or the mission graph.

## Evidence schemas frozen for the final handoff

Exactly seven canonical files are reserved below `.arc-verify/job-meter/`:
`contract.md`, `experience.md`, `value.md`, `native.json`, `gates.json`, `qa.json`,
`final.md`. Combined bytes must not exceed 5,000,000. Task-1 creates only contract
and native; unrun future checks remain null/pending, never true. Existing standard
coverage artifacts can be referenced by path and digest, not copied. Generated
OMA handoff summaries link these receipts instead of duplicating raw evidence.

`native.json` (`schemaVersion: 1`) fields required by the verification contract:

| JSON path | Final type / evidence |
| --- | --- |
| `baseline.acceptedHead` | Exact accepted commit above; baselineTree and snapshot/worker provenance recorded separately. |
| `calibration.workerCount` | 2; final persisted controlled workload, separate from initial `feasibility`. |
| `calibration.shortLivedDescendantsReaped`, `scopeMatched`, `withinTolerance` | Booleans established by recorded raw native reference/terminal usage and identical execution identity/scope; absolute error <= max(0.1 core-seconds, 5% of reference). |
| `matrix.allRequiredCasesPassed` | True only when all required case receipts pass. |
| `persistence.sqliteCliDesktopAgree` | True only for independent SQLite, arcctl JSON, and production Tauri bridge comparison for the same job/report. |
| `persistence.retention90DaysVerified` | True with cutoff/active/expiry tests. |
| `compatibility.allRequiredCasesPassed` | True only with semantic, protocol/snapshot/redaction and bounded fail-open receipts. |

`calibration` also retains requested/actual elapsed time, identities, user/system
raw values, derived core-seconds, scope, missing coverage, reference method,
commands/exit codes and source hashes. `matrix.cases` records individual named
cases with commands, outcomes and assertions, rather than only an aggregate flag.
Required cases: scheduled and lifecycle-only; separate queue/run; exit 0 and 17;
signals; stop/continue; nested wrappers; unrelated pipeline processes; identical
and conflicting reports; daemon restart and lost acknowledgements. Compatibility
also covers one execution during outages, unchanged command/signal semantics,
bounded metadata, protocol-1 old peers, host snapshot shape and redaction.

`gates.json` required final paths (field names and types are fixed):

```json
{
  "gates": {"allSixPassed": true},
  "coverage": {
    "floorPercent": 80, "crateCount": 10, "perCrateFloorPassed": true,
    "productionLinesOnly": true, "collectionCount": 1,
    "receiptValidated": true, "harnessReusedSameReceipt": true,
    "unmodifiedFinalInputs": true
  },
  "desktop": {"allAffectedChecksPassed": true},
  "containerTargets": {"completionRequirementsSatisfied": true},
  "integrity": {
    "migrationBytesPreserved": true, "newSourceAndTestsBelow500Lines": true,
    "evidenceWithinLimits": true
  }
}
```

This is an expected schema example, **not a gate result**. Backend gate records
must additionally contain actual argv/cwd, timestamps, exit status, output
summary/digest and final input hashes. Under the supplied mission clarification,
the container/service check is not applicable because no observation policy is
configured. Retain `containerTargets.completionRequirementsSatisfied: true`
with that applicability and rationale in the final gate receipt; it records
satisfaction of the unchanged field without claiming a container check ran.
No container-policy readback is a completion prerequisite. Preserve native.json's
historical unavailable-policy observation without treating it as pending work.

The six unchanged gates are cargo fmt,
workspace Clippy with -D warnings, cargo test --workspace, one configured
coverage collection (80% production lines **per each of ten crates**), harness
validation reusing that validated ARC_COVERAGE_RECEIPT, and the exact explicit
active Bun paths from the mission verification contract (including the one
small evidence check). Record affected desktop checks as well. Do not inspect
harness validator source, change required argv, replace explicit paths with
discovery, weaken tests, or recollect coverage during QA. Allow at least the
framework's 7,200,000 ms per-check timeout in any wrapper; planning timeoutMs
estimates do not terminate checks. Do not collect coverage in task-1.

The unchanged final gate command shapes are:

| Gate | Program and arguments / obligation |
| --- | --- |
| Formatting | `cargo fmt --all -- --check` |
| Clippy | `cargo clippy --workspace --all-targets -- -D warnings` |
| Workspace/native integration | `cargo test --workspace` |
| Production-line coverage | `bun run test:coverage`; one collection for final inputs using the existing configured collector. |
| Harness | `./scripts/harness/validate.sh`, supplying the collector's validated receipt through `ARC_COVERAGE_RECEIPT`; reuse the same collection, with no validator change. |
| Explicit active Bun tests | Exact command below, including the one small evidence checker. |

```sh
bun test apps/desktop/tests/bridge-client.test.ts apps/desktop/tests/contracts.test.ts apps/desktop/tests/credits.test.ts apps/desktop/tests/dashboard.test.ts apps/desktop/tests/lifecycle.test.ts apps/desktop/tests/meter.test.ts scripts/coverage/accounting.test.ts scripts/coverage/desktop-foundation.test.ts scripts/coverage/gate.test.ts scripts/coverage/hygiene.test.ts scripts/coverage/terminal-outcomes.test.ts scripts/coverage/traffic-accounting.test.ts scripts/harness/equivalence.test.ts scripts/harness/traffic-targets.test.ts scripts/harness/coverage-reuse.test.ts scripts/job-meter/evidence.test.ts
```

`scripts/coverage/config.ts` currently includes **arc-core, arc-detector,
arc-storage, arc-ipc, arc-daemon, arc-cli, arc-traffic, arc-desktop, arc-platform
and arc-meter**. Each of these ten crates must independently reach at least 80%
production-line coverage; a workspace average or a historical six-crate floor
does not suffice. Finish code/test repairs and input-affecting review changes
before the one collection; preserve its run ID, hashes, per-crate counts and
validated receipt. A changed final input invalidates reuse; disclose it rather
than changing counts or claiming a stale receipt is current. Backend gates.json
must reconcile actual collections and must not hide failed attempts.

Record affected desktop TypeScript/lint checks and deterministic production
bridge evidence as backend JSON, with actual command/exit/hash data. The existing
`bun run desktop:typecheck` and `bun run desktop:check` entries are available;
production Tauri registered-command and daemon-bridge integration belongs in
the native workspace checks. A frontend fixture alone does not prove dispatch,
authentication or persisted reconciliation. No browser E2E suite or screen pass
is substituted for those checks. Only an explicitly applicable final-review
visual pass may use the existing allowed DevTools route, after code verification.

`qa.json` must contain `reviewer.profile: "qa-reviewer"`, `independent: true`,
`productReadOnly: true`; `finalReview.complete: true`,
`unresolvedProductBlockers: 0`, `evidenceVerified: true`, `inputsMatch: true`.
Those booleans are written by the independent reviewer only after verification.

`experience.md` explains CPU used (core-seconds), Run elapsed, Queue wait, Unknown
and Partial in the actual selected-job path. `value.md` records commercial
outcomes as unmeasured, experiments not run and no outreach, and does not turn
CPU core-seconds into dollar savings. `final.md` maps MTR-002/MTR-003/MTR-005/
AT-MTR-01 to current receipts and states:

- vendor/project aggregation: outstanding
- live process-tree metering: outstanding
- Windows: outstanding
- vendor hooks: outstanding
- full Meter acceptance: outstanding

No screen check, browser E2E, Playwright installation, outreach, deployment or
external delivery is part of task-1. Preserve existing work and mission state.

## Task-1 verified outcome

The final Actor-local receipts are in `native.json#/preparation`: all **117**
tests passed for `cargo test --locked -p arc-core -p arc-ipc -p arc-platform`;
`cargo clippy --locked -p arc-core -p arc-ipc -p arc-platform --all-targets -- -D
warnings`, `cargo fmt --all -- --check` and `cargo check --locked --workspace`
passed. The latter compiles existing consumers, including the Tauri crate, but
does not prove new persistence or desktop presentation. Exact argv, UTC times,
exit codes, outputs and scoped source-input hashes are retained in the receipt.
New source/test files are below 500 lines. Coverage was not collected.

Final feasibility on the current native implementation: terminal CPU
**19.799085 core-seconds**, independent reference **19.799009 core-seconds**;
absolute difference **0.000076 core-seconds**, below the native reference's
**0.98995045 core-second** engineering tolerance. The observed spawn-to-terminal
interval was 10.003717708 seconds; the C workload's inner monotonic interval was
10.000535 seconds. Both scope/identity comparison and all eight explicit
short-lived reaps passed. The earlier feasibility observation is retained under
`feasibilityRuns` with its original input hashes; it is not substituted for this
final observation. Neither observation is persisted AT-MTR-01 acceptance.

The initial compile type-inference failure and Clippy failures were repaired.
A focused regression also demonstrated RED when nonfinite typed CPU values
could serialize to JSON null; validating typed reports/summaries before JSON
serialization produced GREEN. No test, lint rule or acceptance threshold was
weakened. Host snapshot/lifecycle compatibility tests still pass.

The shared interface is ready for its dependent implementation tasks. Remaining
mission work includes CLI integration, single-writer persistence/retention,
production desktop delivery, the persisted native/behavior matrix, independent
QA and the six final gates with one coverage collection. Container/service
observation is not applicable under the supplied mission clarification and adds
no prerequisite to the Chief handoff. The originating clone's
symbolic-ref limitation is reconciled by the task-4 readback above; no committed
implementation delivery is inferred from those baseline refs.

## Task-4 conformance audit and handoff

Recommendation: proceed with the narrow PID-specific terminal route for
`native_waited_descendants`, subject to the unchanged dependent implementation
and final verification. There is **no observed task-1 interface/native-probe
blocker** requiring a repair before consumers proceed. This is a contract
convergence result, not a passing combined acceptance or a native gate run.

| Delivered source inspected | Conformance result / consumer responsibility |
| --- | --- |
| `crates/arc-core/src/job_meter.rs` and `job_meter/validation.rs` | Schema/version/limits, null keys, CPU sum tolerance, quality/reason constraints, tagged root relations, queue triples and unknown/missing/expired states match this document. `JobMeterSummary.accounting` contains a boxed report in `recorded`; queue is a sibling, never part of the client report. |
| `crates/arc-core/tests/fixtures/job-meter-report.json` and `job_meter/tests.rs` | Same synthetic fixture for consumers; tests define known zero, invalid numeric/identity values, non-additive nesting, queue separation and unknown distinctions. Their execution is attested only by task-1's matching receipt. |
| `crates/arc-ipc/src/meter/job.rs`, `meter.rs` and client operation diff | Signatures, `JobMeterRequest::Report(Box<JobMeterReport>)` / `Summary(SummaryRequest)`, request/response correlation, 16 KiB object validation, static errors and deadline helpers match. Handler installation and durable acceptance are still backend-store work. |
| `crates/arc-platform/src/child_wait.rs`, its tests and probe example | `wait_owned_child(u32, WaitMode) -> io::Result<Option<ChildWaitEvent>>` preserves raw status, makes usage terminal-only and returns conversion errors without replacing status. No ancestry scan, second waiter or process-group selector was introduced. CLI integration is still backend-native work. |
| `crates/arc-cli/src/exec.rs`, `exec/signals.rs`, `exec/lineage.rs`, `exec/traffic.rs`, `exec/transport.rs` | Baseline sole spawn/wait, signal masking, zombie identity capture, bounded transport and scheduled-only lineage sidecar explain the consumer work. Lifecycle-only ancestry and terminal report capture are obligations, not existing implementation claims. |
| Storage migrations 0001–0003, `jobs/transitions.rs` and daemon `db_worker.rs` / `db_worker/jobs.rs` | Existing durable observer/child/outcome facts and single SQLite worker are the authority. New summary/root/expiry storage is additive. `completed_at_ms` is the logical lifecycle snapshot timestamp, not a proposed rename of an existing SQL column. |

Task-4 attempt 1 read-only commands/inspections:

- `git status --short`, scoped cached/unstaged diffs and
  `git rev-parse HEAD`: accepted HEAD and preceding staged changes observed;
  status/diff emit `av.yaml: Operation not permitted`, so whole-tree cleanliness
  is not claimed. Container applicability follows the supplied mission
  clarification above; this access diagnostic is not a blocker.
- `git rev-parse --verify <snapshot-ref>` and `<worker-ref>`: both exit 0 with
  the snapshot hash documented above; no replacement worker tree invented.
- Python standard-library SHA-256 inspection: all 22 receipt source inputs
  match; terminal/reference arithmetic and normalized root identity reconcile.
  Immutable migration bytes are compared with the accepted Git blobs. Scoped
  diff whitespace and final file-integrity checks accompany delivery.
- Tool version commands listed in Source provenance: all exit 0; source reads
  and Apple manual reads support the decision. `.agents/oma-config.yaml` was
  absent; `.agents/oma-config.cue` was readable. Serena discovery exposed no
  applicable tools; only located-source reads were used, without native source
  search. Three atomic documentation patches rejected unmatched context;
  corrected exact hunks were applied without product/native.json changes.

Integrity outcome: `git diff --check -- .arc-verify/job-meter/contract.md` exits
0. All 20 other preceding staged paths retain their original SHA-256 bytes;
all 22 recorded source inputs and all three accepted migration blobs match.
The evidence directory contains four canonical files within seven files and
5,000,000 bytes. Task-4 adds no source/test file and collects no coverage. The
one small evidence checker and three remaining canonical receipts belong to
the already assigned dependent tasks, not new files in this handoff.

Task-1's original contract digest
`d3218aff6ec53967dcae2f50ebc7953cb4f3273bca219b6b9c9b294f421c6d66`
in native.json refers to **document revision 1**. It is intentionally not rewritten
when this assignment transfers ownership. Task-4's final contract digest belongs
to its delivery, while native.json remains byte-identical at
`f2b87227bb65ae2353c5f019c769195963cf3b3c740603bccb53240e39ae55f4`.
Do not interpret the predecessor document binding as a hash of this revision or
rewrite historical receipt bytes to make it appear so. Compiled source-input
hashes are unchanged.

### Task-4 attempt 2 repair record

Revision 3 applies the supplied mission clarification: no container/service
observation policy is configured, so that check is not applicable. The earlier
policy-readback prerequisite has been removed from readiness, the Chief handoff
and remaining work. The final gate receipt retains the required
`containerTargets.completionRequirementsSatisfied` field with the not-applicable
rationale. native.json's historical unavailable-policy observation is preserved;
it is not unfinished acceptance work.

Read-only artifact inspection confirmed all 22 task-1 source-input hashes,
all three migration blobs against accepted HEAD, and unchanged bytes for all
28 preserved paths outside this contract, including native.json. Rust, JSON and
command code blocks are unchanged from revision 2, preserving the compiling
interfaces and verification commands. Raw terminal/reference arithmetic still
reconciles to an absolute error of 0.000076 core-seconds within tolerance; this
is inspection of the existing feasibility receipt, not a new native run.
`git diff --check -- .arc-verify/job-meter/contract.md` returned 0. The evidence
directory retains four canonical files below the seven-file/5,000,000-byte
limits. Only contract.md changed in this repair; no builds, tests, coverage,
product edits or mission-state mutations were performed.

### Remaining prerequisites, uncertainties and responsible owners

| Item | Evidence / consequence | Owner and next required evidence |
| --- | --- | --- |
| Final toolchain/gate readiness | Installed versions match; task-1 receipts prove compilation only for those matched inputs. Current combined build, SDK/LLVM coverage readiness and affected desktop checks were not executed in task-4. | Backend gate owner compiles/tests the integrated implementation and records actual results with the unchanged two-hour per-check allowance. Report a concrete prerequisite failure if encountered. |
| Scope beyond the controlled waited workload | The probe proves two workers and eight explicitly reaped descendants; no arbitrary opaque/detached process-tree calibration exists. | Backend-native verifies the full saved matrix and truthful Partial/Unknown classification. If probe identities, raw arithmetic or scope cease to match after integration, repair task-1's native implementation/evidence before proceeding; never replace the criterion or assert agreement. |
| Cross-mode root binding and durable recovery | Types support the contract; production lifecycle-only ancestry, root verification, idempotence/conflicts, terminal binding and expiry are not yet integrated. | Backend-native and backend-store consume this actual dependency artifact; store owns additive durable binding and native owns caller/wait integration. Return any required shared-interface or ownership change to the Chief/single writer with exact paths. |
| Persisted calibration and production desktop | native.json's final acceptance sections are still `not_run`; the initial probe registered no job. | Backend-native/desktop independently compare SQLite, CLI JSON and registered production Tauri bridge for the same job/report, then obtain read-only oma-qa and final receipt assessment. |

These items preserve the goal and acceptance criteria; they are neither invented
approvals nor grounds to reduce the assigned implementation scope. Commercial
cost/benefit, production overhead and user comprehension remain unmeasured.
Vendor/project aggregation, live process-tree metering, Windows, vendor hooks
and full Meter acceptance remain outstanding as specified above.
