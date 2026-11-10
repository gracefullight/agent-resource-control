# CLI execution state and recovery contract

Status: completed design handoff for task-2. Implementation and runtime verification belong to task-3 and independent QA. This file proposes copy; it does not establish that fallback is implemented.

## Scope and source provenance

Only this file is owned and changed by the Design Director. Task-3 owns product help, documentation, implementation and tests. No new flags, prompts, spinners, retention nudges, daemon-start requirements or field-test gates are proposed.

| Evidence scope | Inspected revision or value |
| --- | --- |
| Accepted committed baseline | `00773c69a8e5e7fd417fd5cc78d03e222364f609` |
| Accepted baseline product tree | `b98f996caa04fe13eeeeab1072c3c528f6d18c65` |
| Actor snapshot and inspected local HEAD | `ce857b0554c5d06f0000d85a06219444fcbc5329` |
| Snapshot product tree | `b98f996caa04fe13eeeeab1072c3c528f6d18c65` |
| Snapshot ref supplied by mission | `refs/agent-valley/task-evidence/a546003e0df37906-78686d2704d4d690-1/snapshot` |
| Worker ref supplied by mission | `refs/agent-valley/task-evidence/a546003e0df37906-78686d2704d4d690-1/worker` |

`git diff --name-status 00773c69a8e5e7fd417fd5cc78d03e222364f609 HEAD` returned no product changes. Initial status excluding `av.yaml` was empty. The inspected source is therefore shared with the accepted baseline, although snapshot HEAD differs. This new, uncommitted contract is Actor-local work; it is not part of either committed tree. Integration must verify its delivered bytes in the review worktree. `av.yaml` was excluded from status and was not inspected.

SHA-256 of inspected input bytes:

| Path | SHA-256 |
| --- | --- |
| `crates/arc-cli/src/main.rs` | `22c10fbcb888a29930d9ff3dcc37bebb8e02ff45c7ced4447fdd6cadcb1107ed` |
| `crates/arc-cli/src/exec.rs` | `238937ec7f455a1dc3832489bb15aad183c6c7e7f8338ea2a82777ef341cca80` |
| `crates/arc-cli/src/exec/transport.rs` | `e127b2c16eb3a9f1ae02593252ecc6d12ce5934cafe92417a96945dad8e5e9df` |
| `crates/arc-cli/src/job.rs` | `d96ef24bf66f511ef5b1030a5a08d3197f96e9c083fbec6366671160cdcf689b` |
| `docs/managed-exec.md` | `76a634416a406038e605ce9aa09f4f672a829b6a724444a47482c2c35b05e2f5` |

## Observed source behavior

These are source observations at the inspected snapshot, not fresh execution results:

- `main.rs`, `Command::Exec`: usage is `arcctl exec -- <PROGRAM> [ARG...]`; help preserves native argument examples, disables color, requires starting the daemon and offers read-only job inspection.
- `exec.rs`, `run` and `not_started`: unconfirmed registration or claim prevents spawn with `NOT_STARTED`. There is one `Command::spawn` site. Successful spawn precedes attachment; attachment failure emits `TRACKING_UNCERTAIN`. Failed completion emits `COMPLETION_UNCONFIRMED` with the locally observed exit or signal.
- `crates/arc-cli/src/exec/transport.rs`, `call`: each metadata call starts its own two-second deadline, retries up to three times and erases error distinctions into `Result<Value, ()>`. This is not proof of a shared two-second budget.
- `job.rs`, `run`: recent records are marked last-known; by-ID inspection reports identity at observation. Missing records use `JOB_NOT_FOUND`; unreachable status uses `STATUS_UNAVAILABLE`. A status request launches no work.
- `docs/managed-exec.md`, paragraphs beginning “The CLI owns” and “The daemon records”: documents native descriptors and terminal behavior, quiet normal execution, fail-closed launch and metadata-only retries. Its final paragraph leaves scheduling, desktop and native Windows outstanding; vendor hooks must also be named in the amendment.

## Required interaction and state rules

The journey is: `arcctl exec -- <PROGRAM> [ARG...]` → native program interaction → actual command outcome. Read-only tracking inspection is optional and follows that journey. There is no preliminary daemon setup step. Metadata outages converge on the existing single spawn path and never create a second execution owner.

Use one shared pre-spawn metadata deadline of at most two seconds across registration, claim and retries. This is an engineering requirement awaiting deterministic and native timing evidence, not measured current performance. It bounds metadata preparation, not command duration or total CLI wall time. Cancellation, invalid arguments and explicit authenticated rejection remain non-launching outcomes. Do not describe malformed or mismatched replies as authenticated acceptance or rejection; task-3 must preserve protocol diagnostics and document its typed error mapping.

| State | Evidence required | Output and behavior |
| --- | --- | --- |
| Managed | Valid matching acknowledgements confirm the relevant transitions; spawn succeeds | No success banner. Preserve child output and actual exit/signal. Acknowledged registration does not confirm later attachment or completion. |
| Unavailable | Preparation ends without any lifecycle write possibly reaching the daemon; spawn succeeds | Emit `TRACKING_UNAVAILABLE` after successful spawn. Do not claim a saved record or print a reference that suggests one. |
| Uncertain | A lifecycle write may have reached the daemon, or an acknowledged registration is followed by unconfirmed claim/attachment; spawn succeeds | Emit `TRACKING_UNCERTAIN` after successful spawn. A generated reference is only a lookup key; persistence is unconfirmed. Lost acknowledgements after commit belong here. |
| Rejected | An authenticated, matching response explicitly rejects preparation | Emit non-launch copy; do not fall back. Keep any applicable protocol-1 diagnostic. |
| Cancelled before spawn | Cancellation observed during preparation or immediately before the sole spawn site | Emit non-launch copy; do not fall back. Preserve existing signal/cancellation semantics. |
| Failed spawn | The native spawn call returns an error | Emit `SPAWN_FAILED`; never say “Command started,” even if metadata was registered or claimed. |
| Invalid request | Argument validation fails | Preserve `INVALID_REQUEST`, usage and exit 2; no launch. |

Classify uncertainty conservatively: inability to confirm a response is not proof that no write committed. Explicit rejection and cancellation take precedence over fallback. Missing local tracking prerequisites must not be described as an explicit daemon rejection.

## Exact proposed stderr copy

Each block is plain text. Print once for the relevant transition, without color or terminal controls. Bracketed placeholders below are substitutions, never literal user input. Only ARC-generated validated job IDs and numeric locally observed outcomes may be interpolated. Never echo argv, executable names, cwd, environment, raw request payloads, daemon error text or OS error strings. Child-written stdout and stderr pass through unchanged; this restriction applies to wrapper diagnostics.

Managed success: no additional output. Do not print a persistence promise.

Unavailable tracking, only after successful spawn:

```text
TRACKING_UNAVAILABLE
Command started. ARC tracking is unavailable. A lifecycle record may be absent.
Do not launch another copy to recover tracking.
```

Potentially committed but unconfirmed recording, only after successful spawn:

```text
TRACKING_UNCERTAIN
Command started. Lifecycle recording is unconfirmed.
Reference: <JOB_ID>
After daemon recovery, inspect arcctl job status <JOB_ID>.
A missing record does not show whether the command ran. Do not launch another copy to recover tracking.
```

Substitute the same validated ID in both positions. Omit both reference lines if no safe reference exists; retain the state, uncertainty and no-relaunch guidance. Do not label the reference “saved job” or “recorded job.” Later attachment loss uses this same uncertain state and does not repeat an earlier fallback notice for the same uncertainty.

Explicit authenticated rejection before spawn:

```text
NOT_STARTED
ARC rejected execution. No child was started.
```

Cancellation before spawn:

```text
NOT_STARTED
Execution cancelled. No child was started.
```

Failed native spawn, preserving existing copy:

```text
SPAWN_FAILED
No child was started. Check the executable and execution permissions.
```

Invalid arguments, preserving existing copy:

```text
INVALID_REQUEST
Use arcctl exec -- <PROGRAM> [ARG...]. No child was started.
```

Unconfirmed completion after a locally observed exit:

```text
COMPLETION_UNCONFIRMED
The child exited with code <CODE>. Recording is unconfirmed.
Do not launch another copy to recover tracking.
```

For native signal termination, replace the second line with `The child terminated by signal <SIGNAL>. Recording is unconfirmed.` A validated lookup reference and its recovery lines may follow when available, using the uncertain-state wording above. Do not imply completion was recorded merely because the wrapper observed it. Preserve the child's actual exit code, including 17, or native signal; diagnostics do not turn that outcome into wrapper success or failure. If no completion write is attempted in unavailable mode, the initial unavailable notice is sufficient; do not invent an unconfirmed completion request.

If waiting fails and the outcome cannot be observed, preserve `OUTCOME_UNKNOWN` and use:

```text
OUTCOME_UNKNOWN
The child outcome is unknown. Do not launch another copy to recover tracking.
```

A validated lookup reference and recovery lines may follow. Do not invent an exit or say that the child stopped. Cancellation after spawn remains child signal/terminal handling, not the pre-spawn “No child was started” case.

Preserve existing `status`, `doctor` and `classify` protocol-1 diagnostics, including `PROTOCOL_INCOMPATIBLE`, `DAEMON_UNAVAILABLE`, `INTEGRITY_FAILED` and `IPC_ERROR` where applicable. Do not replace a specific protocol error with an outage success claim. Exec may print an applicable allowlisted diagnostic label alongside non-launch copy; never relay arbitrary peer text. Keep `JOB_NOT_FOUND`, `STATUS_UNAVAILABLE`, last-known labels and existing read-only status behavior. Distinctions must remain readable in redirected output and without color.

## Exact help amendment for task-3

In `crates/arc-cli/src/main.rs`, retain the exec usage and replace its `after_help` text with the following rendered text. Encode it using the existing Rust string convention; retain the literal backslash in `'%s\n'`.

```text
Examples:
  arcctl exec -- cargo test
  arcctl exec -- printf '%s\n' 'two words'
  arcctl exec -- '/path/with spaces/tool' '' '--help' '|'

Your shell handles quoting before ARC receives arguments.
ARC does not interpret pipes, redirections, substitutions or shell operators.
If the daemon is unavailable or tracking cannot be confirmed, ARC still attempts the command once.
Registration, claim and retries share one pre-spawn metadata budget of at most two seconds.
Cancellation, invalid arguments and explicit authenticated rejection do not start a child.
Tracking diagnostics go to stderr; the child keeps its native streams, exit status and terminal behavior.
Stopping the daemon does not stop the running child.
Use arcctl job status to list last-known lifecycle records.
Use arcctl job status <JOB_ID> for read-only inspection after daemon recovery.
A reference does not confirm a saved record. A missing record does not show whether the command ran.
Daemon recovery never replays the command. Do not launch another copy to recover tracking.
Desktop integration, native Windows acceptance, vendor hooks and scheduling remain outstanding.
```

“Attempts” preserves the possibility of native spawn failure. Do not replace it with “always runs successfully.” This target wording must ship with matching implementation and timing evidence; this contract alone does not verify it.

## Exact documentation amendments for task-3

Apply to `docs/managed-exec.md`; preserve the rest of the native and protocol description.

1. Replace the opening daemon-start sentence and example block with:

````markdown
Run a program directly:

```sh
arcctl exec -- cargo test
arcctl exec -- printf '%s\n' 'two words'
arcctl exec -- '/path/with spaces/tool' '' '--help' '|'
```

The daemon is optional for execution. ARC attempts the command once when lifecycle tracking is unavailable or unconfirmed. Cancellation, invalid arguments and explicit authenticated rejection do not start a child; a native spawn failure is reported separately.
````

The path example passes an empty argument, `--help` and a literal pipe as three arguments. Shell pipeline syntax outside the invocation remains the shell's responsibility.

2. Replace the paragraph beginning “The daemon records” with:

```text
The daemon records lifecycle metadata, not commands or environment values. Registration, claim and their retries share one pre-spawn metadata deadline of at most two seconds. Metadata retries never launch a child. If the daemon is absent, unreachable, stalled or disconnects during preparation, ARC continues to its single native spawn attempt. This includes requests that may have committed before their acknowledgements were lost. Fallback reports TRACKING_UNAVAILABLE or TRACKING_UNCERTAIN on stderr only after successful spawn. Neither message confirms lifecycle persistence. ARC waits for the same child and preserves its locally observed outcome. An uncertain completion message does not authorize another invocation. Fallback does not allocate reservations or initiate ARC cleanup.
```

3. Retain the daemon-loss and identity-reconciliation paragraph. Add immediately after it:

```text
Tracking recovery is observational. If ARC printed a reference, it is a lookup key, not proof that a record was saved. After daemon recovery, use arcctl job status <JOB_ID> to inspect it, replacing <JOB_ID> with that reference. Use arcctl job status to list recent last-known records. A missing lifecycle record does not establish whether the command ran. Daemon recovery never authorizes replay; do not launch another copy to recover tracking. A local exit code or signal describes the observed child outcome, independently of whether that outcome was persisted.
```

4. Replace the final paragraph with:

```text
This is Phase 0 lifecycle tracking. No class receives resource reservations, including REMOTE_CI, REMOTE_EXEC and UNKNOWN. Transport and SQLite admission remain bounded. Desktop integration, native Windows acceptance, vendor hooks and Phase 1 scheduling remain outstanding.
```

Keep the paragraph on cwd, environment, descriptors, stdout/stderr, exit 17, signals, foreground ownership, process groups, stop/resume, ignored dispositions and terminal modes unchanged. Quiet normal managed execution still adds no output. Preserve native non-UTF-8 argument support, UNKNOWN classification, status-list truncation and protocol-1 behavior. No daemon restart or cleanup instruction is added to execution.

## Evidence limits and handoff checks

Interactive developers, script authors and recovery operators are requirement-derived personas. Shorter task journeys and better comprehension are hypotheses; no interviews, field tests, usability measurements, conversion or retention results were observed. Usability remains unmeasured. No simulations were performed. No comprehension or field-test gate is added to this repair.

Task-3 and independent QA should verify copy against deterministic unit, integration and native macOS CLI cases: absent/unreachable/stalled/disconnected metadata peers; lost registration and claim acknowledgements after commit; one invocation; shared deadline timing; no replay after recovery; cancellation, invalid arguments and authenticated rejection with zero launches; failed spawn without started wording; quiet managed success; native argv/cwd/environment/descriptors/streams, exit 17, signals and foreground pipelines. Check diagnostic output for secret leakage and keep actual child stdout untouched. These are verification obligations, not results from this design task. Historical contradictory fail-closed assertions remain the Chief's assigned fixed-baseline preservation obligation; this contract does not edit or waive them.

Skill used: `oma-docs`. Its verify mode regenerates `docs/generated/doc-refs.json`, outside this task's ownership. Reference scans therefore run on isolated temporary copies, with URLs disabled, leaving the repository index untouched. Serena discovery tools were unavailable in the active tool catalog; known assignment paths were read directly, with no replacement code search.

Checks performed:

- Git status, HEAD/tree inspection and baseline-to-snapshot name diff: initial accessible status clean; identical product trees confirmed. Input SHA-256 hashes recorded above.
- `oma docs verify --help`: exit 0; confirmed index-writing side effect and supported `--no-urls` flag.
- `oma docs verify docs/managed-exec.md --json --no-urls` in an isolated copy before writing this contract: exit 0, one document scanned, zero extracted references, zero broken references. This is a reference scan only; it does not validate semantics or fallback behavior.

Acceptance review: all six requested outcome classes have copy and emission conditions; execution is separated from persistence; recovery never relaunches; proposed help/docs preserve native syntax and text accessibility; all four outstanding areas are explicit; observations, required behavior and unmeasured hypotheses are separated. No product files, tests, shared registries, migration or coverage protections were changed. No runtime suites, builds, browser or screen checks were performed for this prose-only assignment.

The first isolated contract scan exited 1: one shorthand source reference was unresolved. Replaced it with the full repository-relative source path. An unscoped `git diff --check` encountered `av.yaml: Operation not permitted`; no access workaround was attempted. Follow-up checks are confined to the owned file and status explicitly excluding that path. The repository-wide whitespace check is not claimed as passed.

The corrected isolated contract scan exited 0 with no broken references; the second isolated scan of the unchanged managed-exec document also exited 0. URLs were not checked. Final accessible status exited 0 and showed only this new contract. All five input hashes remained unchanged. `git diff --no-index --check` against an empty file emitted no whitespace diagnostics but exited 1 for the file addition; it is not reported as a zero-exit check. Product help and docs are proposals awaiting task-3; native behavior, the two-second target and independent QA remain unverified by this task. There are no design ownership or interface blockers.
