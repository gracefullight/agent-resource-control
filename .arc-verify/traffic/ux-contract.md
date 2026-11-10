# Direct-execution queue journey contract

## Scope and evidence

This is a proposed CLI interaction contract for the first Traffic slice. It is a
handoff to the implementation and verification owners, not evidence that the
scheduler is implemented or that users have tested it. The route is the local
OMA design contract; no external visual supplier or screen inspection is needed
for this text-only task.

The inspected Actor checkout was at snapshot commit
`a37e3e9f490496b9674c918727ca197d348534d4`, tree
`4455896fdf976244313a70a9fa38b730b344b411`. The accepted baseline commit is
`4d2f71c1a1482925f6c9918296d7ceb3435b7592`, with the same tree. These source
observations apply to that snapshot and baseline, not to later Actor work:

- [Managed execution](../../docs/managed-exec.md) documents the one-command
  `arcctl exec -- <PROGRAM> [ARG...]` interface, inherited streams and native
  execution behavior, bounded tracking-outage fallback, observational status,
  and no replay. It identifies scheduling as outstanding at this revision.
- [CLI entry and help](../../crates/arc-cli/src/main.rs) document separate native
  arguments and optional `arcctl job status [JOB_ID]`. The existing
  [status implementation](../../crates/arc-cli/src/job.rs) presents last-known
  lifecycle states and says `Scheduling: not implemented`.
- [Execution preparation](../../crates/arc-cli/src/exec/preparation.rs) uses
  best-effort diagnostics, including a bounded finish when stderr is blocked;
  [execution](../../crates/arc-cli/src/exec.rs) owns the native child. These are
  source observations, not fresh runtime measurements of Traffic.

The supplied design advice recommends a neutral queue notice and reports no
observed users or measured adoption, retention, conversion, churn, or scheduler
usability. No field test or Traffic performance measurement was performed for
this contract.

## Journey decision

| Approach | Journey and consequence | Decision |
| --- | --- | --- |
| Existing one-command execution with an inline notice | Invoke the command once; see a waiting notice only when admission confirms a queue; inspect status only if needed. Works without an interactive answer. | Adopt for this slice. |
| Classification or admission confirmation | Stop to ask the user to approve the command or its class. Adds a decision and prevents unattended use when no one can answer. | Do not add. |
| Separate queue console | Require a second surface to understand waiting or start work. Makes the normal path longer and adds implementation scope. | Do not require; retain optional job status. |

Potential users are hypotheses: an interactive macOS developer running two
worktrees, a script author relying on native descriptors and exit status, and
someone checking recovery after a daemon interruption. It is a usability
hypothesis that an inline notice reduces uncertainty and unnecessary relaunches;
no observed user behavior establishes that claim. Retention effects are also
unknown. Do not introduce opt-out friction, preselected choices, or a
confirmation step in this slice.

## Observable transitions

The command remains `arcctl exec -- <PROGRAM> [ARG...]`, with each native
argument passed separately. No confirmation, status lookup, or new mandatory
setup is part of execution. The proposed wording below is plain text on stderr;
`<JOB_ID>` is an opaque reference, not command text or a context token. The
reference is a lookup key, not proof that a record remains available.

1. **Confirmed wait.** Once the daemon confirms that this invocation is queued,
   attempt this notice once, before starting the child:

   ```text
   QUEUED
   Waiting to start.
   Reference: <JOB_ID>
   ```

   Do not print it merely because a request was sent or retry it for every poll.
   A healthy queue wait is not an outage and has no promised start time. Bound
   individual exchanges and actual outage detection without imposing the old
   pre-spawn metadata deadline on a healthy queue wait. Recognized remote and
   UNKNOWN commands bypass resource reservations and must not be presented as
   waiting for this queue.

2. **Admission and local start.** On a confirmed grant, check cancellation before
   the single native spawn attempt. For an invocation that waited, after a
   successful native spawn, report:

   ```text
   STARTED
   Command started.
   ```

   A grant alone is not a started child. A spawn failure reports `SPAWN_FAILED`
   and must not report `STARTED`. `STARTED` confirms the local spawn only; it
   does not claim that lifecycle recording or the eventual outcome is confirmed.
   Ordinary immediately admitted execution need not add a start message.

3. **Completion.** Preserve the child's locally observed exit code or signal.
   Record queue wait and completion outcomes when available; status must label
   them as recorded observations, not a prediction. If completion recording is
   unconfirmed, retain `COMPLETION_UNCONFIRMED`, state the locally observed exit
   or signal separately, and say that recording is unconfirmed. Do not relaunch
   to recover tracking.

For a draining, writable stderr sink, the queued notice must be delivered before
the native spawn, and `STARTED` must be emitted only after successful spawn.
The implementation must bound any diagnostic wait: an unavailable sink cannot
hold admission, cancellation, child supervision, or shutdown indefinitely. If
stderr is blocked, closed, or broken, delivery is best effort and the notice may
be absent; do not claim that the user saw it or that the ordering was observed.
Do not allow a delayed queued message to be presented as a current state after
the start transition. Test ordering with a writable sink and progress with full
and broken sinks. A post-spawn start diagnostic can appear after child output;
no total ordering between stdout and stderr is promised.

## Cancellation and uncertainty

- If cancellation is locally confirmed before the sole spawn attempt, report
  `NOT_STARTED` and `Execution cancelled. No child was started.` Do not claim a
  durable queue entry was removed or a permit released unless that transition is
  confirmed. If cleanup acknowledgement is lost, say that queue cleanup is
  unconfirmed and that status may be checked after recovery.
- After a child has started, cancellation follows the existing signal and child
  supervision behavior. Never say `No child was started` for that case; report
  only the locally observed signal or exit and the separately known recording
  state.
- If a real daemon outage triggers bounded fail-open, attempt the command once.
  Use `TRACKING_UNAVAILABLE` when tracking is unavailable and
  `TRACKING_UNCERTAIN` when admission or lifecycle recording is unconfirmed.
  Say `Command started` only after the native spawn succeeds. An unconfirmed
  acknowledgement is not proof that no queue entry or permit exists.
- If completion or the child's outcome is uncertain, say `Recording is
  unconfirmed` or `The child outcome is unknown`, as applicable. A missing
  record does not establish whether a command ran. A status query observes and
  may reconcile a record; it never authorizes replay. Do not launch another copy
  merely to recover tracking.

## Optional status and interface preservation

`arcctl job status` remains optional and lists last-known records;
`arcctl job status <JOB_ID>` inspects a specific reference. A current response
may distinguish queued, starting, running, completed, signaled, spawn failed,
cancelled, and unknown when supported by recorded evidence. Mark stale or
last-known data accordingly. For example, a last-known queued record may show:

```text
Job: <JOB_ID>
State: queued
Last known: true
```

A queued record is not proof of the current child
state after communication is lost; a missing record is not proof of nonexecution.
Show queue wait and completion details only when actually recorded. For older
records, bypassed commands, unavailable daemons, and older peers, show only what
is known and use unavailable or unknown where necessary. Do not keep the blanket
`Scheduling: not implemented` statement for a record whose scheduling status is
actually known, or invent a scheduling state for records without that data.
Keep protocol-1 client and daemon compatibility when exposing new state.

ARC diagnostics must not include command text, arguments, working-directory
paths, environment values, or lineage secrets. Child stdin, stdout, and stderr
remain inherited and its bytes are not filtered or rewritten. The new notices
are additional ARC stderr output; child output and diagnostics can interleave.
Preserve cwd, environment, terminal behavior, signals and native exit semantics.
Messages must be understandable without color, animation, or a terminal and must
work in unattended scripts and assistive-technology workflows.

## Handoff checks

The implementation owners should verify the prescribed order using two
independent Cargo projects: the second queued notice is observed on a draining
stderr sink while the first job still holds the permit, and the second starts
only after release. Also check immediate admission, remote and UNKNOWN bypass
while full, cancellation on both sides of spawn, failed spawn, lost
acknowledgements, restart, and full or broken stderr. Verify that redacted
diagnostics, native streams, signals and outcomes remain intact. These are
required downstream checks, not results of this document-only task; no runtime,
accessibility, or field-test pass is claimed here.
