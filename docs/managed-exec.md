# Direct execution and lifecycle tracking

Run a program directly:

```sh
arcctl exec -- cargo test
arcctl exec -- bun test
arcctl exec -- printf '%s\n' 'two words'
arcctl exec -- '/path/with spaces/tool' '' '--help' '|'
```

The daemon is optional for execution. ARC attempts the command once when an outage leaves lifecycle tracking unavailable or unconfirmed. Cancellation, invalid arguments, explicit authenticated rejection, invalid protocol replies and local preparation failures do not start a child; a native spawn failure is reported separately.

Pass the executable and each argument separately after `--`. Your shell handles quoting. ARC passes native arguments directly to the process API, without interpreting pipes, redirections or substitutions. An executable name containing spaces stays one name. Unsupported classification, including native non-UTF-8 arguments, becomes UNKNOWN and still executes.

The CLI owns one child and inherits the caller's working directory, environment and standard descriptors. For scheduled jobs, ARC adds an opaque `ARC_TRAFFIC_LINEAGE` value to the child's environment only if the caller did not supply one; an existing value, including an empty value, is preserved. ARC also records the scheduled lineage in a private sidecar. A new nested CLI prefers a live ancestor's sidecar and falls back to the environment when the wrapper is no longer a live ancestor. A nested eligible job can reuse its ancestor's permit only after the daemon verifies its process ancestry; the value alone does not grant admission. Child exit codes, including 17, are preserved; native signal termination is re-raised after bookkeeping. With a controlling terminal, ARC keeps its child in the caller's process group and does not change foreground ownership. Terminal signals reach the whole job without duplicate forwarding. Without a controlling terminal, the child gets a separate process group. Wrapper-directed nonterminal signals, including SIGTERM and SIGHUP, are forwarded to the child. A stopped child suspends the wrapper; the shell's background and foreground controls apply to the whole terminal job. Inherited ignored signal dispositions and terminal modes are preserved.

## Scheduling

The daemon records lifecycle metadata and opaque scheduling capabilities; it does not record command text or other environment values. With a healthy daemon that supports the relevant Traffic capability, directly executed local `cargo build`, `cargo test`, `cargo clippy`, `cargo fmt` and one-shot `bun test` commands normally enter a durable FIFO queue. A verified nested job can reuse its ancestor's permit. The detector checks for the program token `cargo` or `bun` followed by a supported subcommand; it does not resolve the executable. An explicit path such as `/path/to/bun`, a wrapper invoked under an unrecognized name such as `bun-wrapper`, or a package script invoked through `bun run` is unresolved and bypasses Traffic admission. A PATH shim invoked as `bun` or `cargo` can still be eligible.

For Bun, test paths and recognized one-shot options can be scheduled. Watch or hot mode, such as `bun test --watch` or `bun test --hot`, and unresolved forms such as unknown or ambiguous options bypass the Traffic queue and reservations. Remote, UNKNOWN and other unsupported classifications also bypass them; they do not wait for a Traffic permit. When started, Bun receives the original arguments, including for bypassed forms.

`arcctl classify` analyzes literal command text rather than the native argument vector used by `arcctl exec`. Its conservative Bun result can be unresolved even when a direct invocation is eligible. A classification alone does not show whether a particular execution was admitted.

The current daemon uses these fixed class windows and reservation estimates:

| Class | Commands | Window | CPU reservation | Memory reservation |
| --- | --- | --- | --- | --- |
| Build | `cargo build` | 1 | 2 cores | 2 GiB |
| Test | `cargo test`, one-shot `bun test` | 1 | 1 core | 1 GiB |
| Lint | `cargo clippy`, `cargo fmt` | 1 | 1 core | 1 GiB |

Cargo and Bun tests share the Test window and the global budget of 2 CPU cores and 4 GiB. Admission requires both a class window and enough shared budget. These are estimates for admission, not operating-system resource limits. There is no user-facing setting for changing these windows or budgets in this slice.

When the daemon confirms a wait, ARC attempts this notice once on stderr before starting the child:

```text
QUEUED
Waiting to start.
Reference: <JOB_ID>
```

While the daemon is healthy, the command waits until admission without a promised start time. If it was queued and then starts under a confirmed grant, ARC prints `STARTED` and `Command started.` after the native spawn succeeds. An immediately admitted command normally has no queue or start notice. ARC diagnostics are best effort when stderr is blocked or broken and can interleave with the child's output.

## Status and cancellation

Use `arcctl job status` to list up to 50 recent last-known records, or `arcctl job status <JOB_ID>` to inspect a reference from a notice. The by-ID view can report queued, starting, running, completed, terminated by signal, spawn failed, cancelled or unknown states. It shows queue wait and exit or signal details when recorded. An unconfirmed grant reports `outcome unknown` with its reservation retained. If the Traffic status query is unavailable or expires but lifecycle status is available, ARC reports an unknown state rather than a current lifecycle state. If the lifecycle query is unavailable too, ARC reports `STATUS_UNAVAILABLE`. Status does not print command text, and timestamps are Unix milliseconds. A successful query does not imply a successful child outcome.

To cancel while waiting, interrupt the invocation as usual. If cancellation is confirmed before the spawn attempt, ARC reports `NOT_STARTED` and that no child was started. If queue cleanup cannot be confirmed, it also reports `QUEUE_CLEANUP_UNCONFIRMED` when diagnostics can be delivered; this does not confirm that the queue entry or permit was removed. After the child starts, signals follow the native process behavior described above.

The new status query opts into the unconfirmed-grant state; the original Traffic status reply keeps its existing shape. When an older daemon does not support the new query, the CLI falls back to the protocol-1 lifecycle status. If a protocol-1 daemon does not support Bun Traffic admission, the new CLI uses the legacy unresolved classification and lifecycle tracking for `bun test`, without a Traffic reservation. Older protocol-1 daemons without Traffic use lifecycle tracking only. If an earlier enqueue may have committed before a daemon change or lost reply, ARC retains the recovery witness and treats tracking as uncertain rather than assuming there was no reservation.

An old nested CLI cannot read the lineage sidecar. If the caller supplied an unrelated `ARC_TRAFFIC_LINEAGE` value, or a new nested CLI falls back to that value after the wrapper exits, an older daemon that relies on the claim may be unable to find the outer permit for reuse.

## Outages and recovery

Metadata retries never launch a child. A healthy queue can wait beyond the outage deadline because each bounded exchange releases its IPC and database tickets before polling again. If the daemon is absent, unreachable, stalled or disconnects during preparation, ARC continues to its single native spawn attempt. This includes requests that may have committed before their acknowledgements were lost. Fallback reports `TRACKING_UNAVAILABLE` or `TRACKING_UNCERTAIN` on stderr only after successful spawn. Neither message confirms lifecycle persistence. ARC waits for the same child and preserves its locally observed outcome. `COMPLETION_UNCONFIRMED` means the completion recording was not confirmed; when a child has exited, ARC also reports its locally observed exit code or signal. This does not authorize another invocation. Traffic fallback attempts bounded metadata updates to record the child and outcome or reconcile an uncertain queue entry.

Stopping or crashing the daemon does not stop the CLI's child. When using Traffic, the CLI writes a private witness containing the owner's PID and start token before requesting admission. Immediately before executing the target, the child adds its own PID and start token. If the witness cannot be created or updated before execution, the command fails with `SPAWN_FAILED` and the target does not run. A file lock prevents recovery from reading a partial pre-exec update. After a failed spawn or cancellation, the CLI attempts to record a no-spawn outcome in the witness before sending the completion or cancellation request. If that evidence cannot be written, it reports `SPAWN_EVIDENCE_UNAVAILABLE` when diagnostics can be delivered. The witness contains an opaque capability and process identities or a no-spawn outcome, but no command text or other environment values.

When the owner is verified dead, recovery releases a grant whose witness confirms that no child started or that its recorded child has exited. A verified live child retains the grant. If the child's liveness is unknown, status reports an unknown outcome and the grant is held for a grace period starting at the first verified dead-owner observation. The default is 120 seconds; `ARC_TRAFFIC_RECOVERY_GRACE_MS` configures a positive duration in milliseconds on the daemon. After the grace period, reconciliation releases the grant and records an audit event. Restart and later status or admission requests perform reconciliation without replaying work.

After daemon restart, status checks native PID, start token and boot identity and reconciles the record. A missing or mismatched process has an unknown outcome unless the launch owner recorded its exit. Status never replays work or invents an exit code.

Tracking recovery is observational. A by-ID status check may update the lifecycle record with a fresh observation, but never replays the command. If ARC printed a reference, it is a lookup key, not proof that a record was saved. A missing lifecycle record does not establish whether the command ran. Daemon recovery never authorizes replay; do not launch another copy to recover tracking. A local exit code or signal describes the observed child outcome, independently of whether that outcome was persisted.

`job status` identifies truncation. `job status <JOB_ID>` refreshes the selected record's identity observation. Existing `status`, `doctor` and `classify` commands retain their protocol-1 behavior.

Transport and SQLite admission remain bounded. Desktop integration, native Windows acceptance, vendor hooks, broader command detection and remaining scheduling policy remain outstanding.

## Timing and protocol errors

Initial preparation begins with a 1,800 ms absolute metadata deadline. Legacy REGISTER, CLAIM and retries share one deadline: the initial deadline when legacy preparation is entered directly or a capability probe is unsupported, and a fresh 1,800 ms deadline if a Bun enqueue is unsupported or Traffic returns bypass when no request may have committed. Those fallback paths can therefore exceed the initial deadline plus the 200 ms engineering allowance for native handoff. Traffic queue polls receive a fresh bounded deadline after each healthy response; time spent waiting in a healthy queue is not an outage. Each transport attempt uses at most 600 ms and checks cancellation at intervals of at most 25 ms. Native tests measure the larger parent-spawn-to-child-handshake interval; host suspension is not a hard real-time guarantee.

DB_BUSY is retryable metadata uncertainty: the response can follow a commit. Exhaustion permits fallback. Other matching protocol-1 error responses reject preparation. Complete malformed, empty, oversized or mismatched replies stop with IPC_ERROR (or PROTOCOL_INCOMPATIBLE for a version mismatch), rather than becoming outage fallback. A disconnected or truncated stream is an outage and may leave tracking uncertain. Invalid local socket configuration stops preparation. Managed ATTACH, COMPLETE and Traffic fallback updates remain bounded best effort.
