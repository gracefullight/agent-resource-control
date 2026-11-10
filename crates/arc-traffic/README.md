# Traffic policy and integration contract

This crate implements policy only. It does not create a queue, grant or release a
permit, authenticate a process, run a command, or enable scheduling in the daemon.
The SQLite worker, daemon and CLI must supply those pieces before advertising
the Traffic capability.

## Admission

Call `decide(classification, active, &policy, is_fifo_head)` with the detector's
complete classification. Only a consistent direct Cargo build, test, clippy or
fmt classification is eligible. Remote, unknown and other classifications
return `Bypass` without consulting queue or resource availability. The caller
must not create a Traffic queue entry or reservation for a bypass.
The CLI must use `classify_argv` on the exact native arguments it will execute;
the classification value alone cannot prove how its caller obtained it.

For an eligible job, `Policy::validate` rejects zero or out-of-range budgets,
zero windows or reservations, and class reservations exceeding the shared
budget. An invalid classification, configuration or aggregate arithmetic
returns `PolicyError`; it is not an admission. For a valid policy, a non-head
job waits for FIFO, and the head waits when its class window, shared CPU or
shared memory limit is full. Exact budget boundaries are admissible. Checked
integer arithmetic protects both totals; values must fit a signed SQLite
integer. Each reservation uses millicores and bytes.

The default window is one for each class. Build reserves 2,000 millicores and
2 GiB; test and lint each reserve 1,000 millicores and 1 GiB. The aggregate
budget is 2,000 millicores and 4 GiB. These fixed estimates are a conservative
starting profile, not host measurements or operating-system resource caps.
Daemon configuration may replace them only with a validated `Policy`.

Storage must assign a durable monotonic queue sequence and, in one short write
transaction, select the oldest live queued job, obtain the active-permit counts
and sums, call the policy, and commit a grant and its exact reservation together.
The `is_fifo_head` argument is a storage fact; a caller must not infer it from a
timestamp or process-local arrival order. `Active` includes all active grants,
including uncertain reservations. Reconcile a stale unreserved head safely
before admitting a later entry. A decision alone does not mutate a queue or
reserve capacity, and storage must reject conflicting duplicate intents.

## Protocol-1 extension

`arc_ipc::traffic` defines two new operation names; existing `job.start`,
`job.complete`, `job.status`, their payloads and the protocol version remain
unchanged. `traffic.capabilities` is a probe with a null request payload and the
strict supported response `{ "capability": "traffic.v1" }`. A daemon must advertise it
only when the whole extension is available. The current legacy dispatcher still
returns `UNSUPPORTED_CAPABILITY`. `interpret_probe` checks version, request ID,
status and the response shape on an **already authenticated** connection. Only
the explicit matching unsupported result permits fallback to the legacy path;
transport failure, an invalid response or another rejection is not that result.
Transport outages retain the existing bounded fail-open behavior; they do not
establish that an old daemon responded.
An old client continues using its unchanged operations.

`traffic.job` carries a bounded, deny-unknown-fields `JobRequest`:

| Action | Purpose |
| --- | --- |
| `ENQUEUE` | Bind a job ID, observer and complete classification to one intent, plus an optional inherited lineage context. A repeat must return the existing result; a conflicting intent must be rejected. |
| `POLL` | Poll the same job and owner without holding an IPC or database ticket across the wait. |
| `ATTACH` | Attach a verified child to a matching permit generation after the single native spawn. |
| `COMPLETE` | Record an observed outcome and release the matching permit generation atomically when safe. |
| `CANCEL` | Cancel before spawn where proven; an optional generation identifies an acknowledged grant. A running child retains the existing signal and wait semantics. |
| `FALLBACK` | Reconcile one fail-open decision and optional child/outcome after a lost acknowledgement. A missing generation is possible when the grant reply was lost; it is not proof that no grant exists. |
| `STATUS` | Read the separate Traffic status without returning a lineage capability. |

`JobResult` distinguishes bypass, queued, granted, verified lineage reuse,
running, completion, cancellation, abandonment, retained reservation and a
missing record. `StatusResult` has no context field. Both have `is_valid` checks
for use on untrusted decoded replies. `JobId` and process identity reuse the
existing lifecycle types; `Outcome` preserves its established representation.
The daemon must authenticate its peer, validate the request with `decode_job`,
and independently verify process identity and native ancestry. The context's
64 lowercase hex characters are a format only: generate them unpredictably,
bind them to an active lineage and never log or display them. Possession alone
must never cause `Reused`. The typed context redacts its value in `Debug`. IPC
request and response `Debug` output shows the request ID, operation or status,
and a fixed redaction marker without formatting any payload or result. Wire
serialization preserves the complete values, including `lineage_context`.

A grant and a reused parent return `permit_generation`; attach, completion and
acknowledged cancellation use that generation to reject stale transitions.
Unknown-generation fallback is keyed by the original job and owner and must
retain an uncertain grant until reliable child/outcome evidence permits release.
Persist terminal tombstones so delayed requests cannot revive a job. Keep a
parent's permit while verified descendants need it. Never replay a child on
daemon restart or from an IPC retry. The CLI remains the only process launcher.
Legacy operations for a Traffic-owned job must not bypass admission or permit
transitions. None of these storage, daemon or CLI behaviors is implemented by
this crate or the IPC data definitions.
