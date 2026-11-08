# Direct execution and lifecycle tracking

Start `arc-daemon`, wait for its readiness message, then run:

```sh
arcctl exec -- cargo test
arcctl exec -- printf '%s\n' 'two words'
arcctl job status
arcctl job status <JOB_ID>
```

Pass the executable and each argument separately after `--`. Your shell handles quoting. ARC passes native arguments directly to the process API, without interpreting pipes, redirections or substitutions. An executable name containing spaces stays one name. Unsupported classification, including native non-UTF-8 arguments, becomes UNKNOWN and still executes.

The CLI owns one child and inherits the caller's working directory, environment and standard descriptors. Normal execution adds no output. Child exit codes, including 17, are preserved; native signal termination is re-raised after bookkeeping. With a controlling terminal, ARC keeps its child in the caller's process group and does not change foreground ownership. Terminal signals reach the whole job without duplicate forwarding. Without a controlling terminal, the child gets a separate process group. Wrapper-directed nonterminal signals, including SIGTERM and SIGHUP, are forwarded to the child. A stopped child suspends the wrapper; the shell's background and foreground controls apply to the whole terminal job. Inherited ignored signal dispositions and terminal modes are preserved.

The daemon records lifecycle metadata, not commands or environment values. Registration and launch claiming must be confirmed before spawn. Each metadata operation retries at most three times, with a two-second absolute budget and at most 600 milliseconds per exchange. These retries never spawn. If recording fails after launch, ARC waits for that same child and returns its observed outcome. An uncertain completion message does not authorize another invocation.

Stopping or crashing the daemon does not stop the CLI's child. After daemon restart, status checks native PID, start token and boot identity and reconciles the record. A missing or mismatched process has an unknown outcome unless the launch owner recorded its exit. Status never replays work or invents an exit code.

`job status` lists up to 50 recent last-known records, identifies truncation and prints no command text. `job status <JOB_ID>` refreshes the selected record's identity observation. Timestamps are Unix milliseconds. Query success is independent of the child's outcome. Existing `status`, `doctor` and `classify` commands retain their protocol-1 behavior.

This is Phase 0 lifecycle tracking. No class receives resource reservations, including REMOTE_CI, REMOTE_EXEC and UNKNOWN. Transport and SQLite admission remain bounded. Phase 1 scheduling, desktop integration and native Windows acceptance remain outstanding.
