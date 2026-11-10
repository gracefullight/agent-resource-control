# Direct execution and lifecycle tracking

Run a program directly:

```sh
arcctl exec -- cargo test
arcctl exec -- printf '%s\n' 'two words'
arcctl exec -- '/path/with spaces/tool' '' '--help' '|'
```

The daemon is optional for execution. ARC attempts the command once when an outage leaves lifecycle tracking unavailable or unconfirmed. Cancellation, invalid arguments, explicit authenticated rejection, invalid protocol replies and local preparation failures do not start a child; a native spawn failure is reported separately.

Pass the executable and each argument separately after `--`. Your shell handles quoting. ARC passes native arguments directly to the process API, without interpreting pipes, redirections or substitutions. An executable name containing spaces stays one name. Unsupported classification, including native non-UTF-8 arguments, becomes UNKNOWN and still executes.

The CLI owns one child and inherits the caller's working directory, environment and standard descriptors. Normal execution adds no output. Child exit codes, including 17, are preserved; native signal termination is re-raised after bookkeeping. With a controlling terminal, ARC keeps its child in the caller's process group and does not change foreground ownership. Terminal signals reach the whole job without duplicate forwarding. Without a controlling terminal, the child gets a separate process group. Wrapper-directed nonterminal signals, including SIGTERM and SIGHUP, are forwarded to the child. A stopped child suspends the wrapper; the shell's background and foreground controls apply to the whole terminal job. Inherited ignored signal dispositions and terminal modes are preserved.

The daemon records lifecycle metadata, not commands or environment values. Registration, claim and their retries share one pre-spawn metadata deadline of at most two seconds. Metadata retries never launch a child. If the daemon is absent, unreachable, stalled or disconnects during preparation, ARC continues to its single native spawn attempt. This includes requests that may have committed before their acknowledgements were lost. Fallback reports TRACKING_UNAVAILABLE or TRACKING_UNCERTAIN on stderr only after successful spawn. Neither message confirms lifecycle persistence. ARC waits for the same child and preserves its locally observed outcome. An uncertain completion message does not authorize another invocation. Fallback does not allocate reservations or initiate ARC cleanup.

Stopping or crashing the daemon does not stop the CLI's child. After daemon restart, status checks native PID, start token and boot identity and reconciles the record. A missing or mismatched process has an unknown outcome unless the launch owner recorded its exit. Status never replays work or invents an exit code.

Tracking recovery is observational. A by-ID status check may update the lifecycle record with a fresh observation, but never replays the command. If ARC printed a reference, it is a lookup key, not proof that a record was saved. After daemon recovery, use arcctl job status <JOB_ID> to inspect it, replacing <JOB_ID> with that reference. Use arcctl job status to list recent last-known records. A missing lifecycle record does not establish whether the command ran. Daemon recovery never authorizes replay; do not launch another copy to recover tracking. A local exit code or signal describes the observed child outcome, independently of whether that outcome was persisted.

`job status` lists up to 50 recent last-known records, identifies truncation and prints no command text. `job status <JOB_ID>` refreshes the selected record's identity observation. Timestamps are Unix milliseconds. Query success is independent of the child's outcome. Existing `status`, `doctor` and `classify` commands retain their protocol-1 behavior.

This is Phase 0 lifecycle tracking. No class receives resource reservations, including REMOTE_CI, REMOTE_EXEC and UNKNOWN. Transport and SQLite admission remain bounded. Desktop integration, native Windows acceptance, vendor hooks and Phase 1 scheduling remain outstanding.

Preparation uses a 1,800 ms absolute metadata deadline, leaving a 200 ms engineering allowance for native handoff. REGISTER, CLAIM and retries never renew it. Each exchange uses at most 600 ms and cancellation polling uses at most 25 ms. Native tests measure the larger parent-spawn-to-child-handshake interval; host suspension is not a hard real-time guarantee.

DB_BUSY is retryable metadata uncertainty: the response can follow a commit. Exhaustion permits fallback. Other matching protocol-1 error responses reject preparation. Complete malformed, empty, oversized or mismatched replies stop with IPC_ERROR (or PROTOCOL_INCOMPATIBLE for a version mismatch), rather than becoming outage fallback. A disconnected or truncated stream is an outage and may leave tracking uncertain. Invalid local socket configuration stops preparation. Fallback makes no post-spawn metadata requests; managed ATTACH and COMPLETE remain bounded best effort.
