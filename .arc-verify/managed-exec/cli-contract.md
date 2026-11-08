# Managed execution: CLI and recovery contract

Status: CLI specification complete; product conformance and final mission acceptance remain unverified by this task.
Owner: Design Director, task-2, attempt 3. Owned output: this file only.
Skill used: `oma-pm` for interface requirements and deterministic acceptance criteria.

## Evidence and scope

Accepted committed baseline: `19d7a507044cbf83918f58cd41056905c275c8d4`, tree `d2f8ace96256a7e073ab9d1bfb10fdc5cae3bf38`.
Inspected Actor-local HEAD and supplied initial snapshot commit: `7eb4a919309740202cab6f8e07bd24de5352800a`, tree `718ed68af094726b7487daaa2d37d8c00214302f`. Git resolves HEAD and its tree to those values; this snapshot includes a source overlay absent from the accepted baseline. The supplied snapshot ref is `refs/agent-valley/task-evidence/a5b368321dcdfcee-78686d2704d4d690-3/snapshot`; the supplied worker ref is `refs/agent-valley/task-evidence/a5b368321dcdfcee-78686d2704d4d690-3/worker`. Neither named ref resolves in this checkout (`git rev-parse --verify` exits 128). The supplied snapshot commit does resolve as local HEAD; do not replace the missing refs with an invented target or substitute local HEAD for the accepted baseline.

The entry scoped status and owned-file diff were clean. This report was already tracked in the Actor snapshot; this attempt revises it as an uncommitted task edit. That tracking fact does not establish its presence in the Chief's review checkout or accepted baseline. The Chief must verify the delivered bytes in the integrated worktree and refresh any dependent receipt bound to the previous report hash.

Observed by source inspection: this snapshot already contains `exec`, `job status`, static execution recovery messages and bounded metadata retries. `main.rs` retains the separate `status`, `doctor` and `classify` paths; `classify.rs` still says “submit again” for non-executing classification. No command was executed through ARC in this task. Source inspection and document checks are observations of files, not measurements of process correctness or usability. Supplied earlier pass/failure reports are historical inputs, not fresh reproductions. The operator's foreground-pipeline guidance is reflected below; no pre-note test receipt is used to establish conformance.

Inspected Actor-snapshot source and interface SHA-256 values (not accepted-baseline hashes):

| Path | SHA-256 |
| --- | --- |
| `crates/arc-cli/src/main.rs` | `22c10fbcb888a29930d9ff3dcc37bebb8e02ff45c7ced4447fdd6cadcb1107ed` |
| `crates/arc-cli/src/classify.rs` | `5721f556725dca554d0e7de130c84f6831a6989eef2d39e137a65d58fcff0028` |
| `crates/arc-cli/src/doctor.rs` | `0bf990c8dd1a694ee42307e9a8cb5c471994746916c5609d82a610b0e0765221` |
| `crates/arc-cli/src/exec.rs` | `238937ec7f455a1dc3832489bb15aad183c6c7e7f8338ea2a82777ef341cca80` |
| `crates/arc-cli/src/exec/terminal.rs` | `c3a5c74713d36c1fde3a332563d5cdb4260aa341a86e50573587f9eb95a882df` |
| `crates/arc-cli/src/exec/signals.rs` | `7424005b7228de7a5adc7ed9e4e46babdc0a6f18a5ebb44e17dd4e58021cfca6` |
| `crates/arc-cli/src/exec/transport.rs` | `e127b2c16eb3a9f1ae02593252ecc6d12ce5934cafe92417a96945dad8e5e9df` |
| `crates/arc-cli/src/job.rs` | `d96ef24bf66f511ef5b1030a5a08d3197f96e9c083fbec6366671160cdcf689b` |
| `crates/arc-cli/tests/binary.rs` | `fcdf17704312c735e33af4b8cc9947abbecc06ef1205f3a81c551fe0c9fe8da3` |
| `crates/arc-cli/tests/fixtures/pipeline.rs` | `c9abc64de69c0d2dd197a07f3f49437d33d23eff5a0a44e4f5d567fea1f3bda7` |
| `.arc-verify/managed-exec/contract.json` | `ef17b11f3318871537939d68da5e1f1b671aab86bd5a5051de1878e82dd7d4ec` |

Serena discovery returned no tools. Inspection used direct reads of known paths, Git inventories and the supplied assignment; no recursive source search or denied-path workaround was used. OMA execution policy, `oma-pm`, its execution protocol, result contract and the language rule were read. The one-file assignment overrides generic skill output paths: no new plan, mission state or extra result file is created.

## Users and alternatives

These personas and usability effects are hypotheses derived from the assignment; no participants were observed.

| Hypothesized user | Successful journey | Principal error to prevent |
| --- | --- | --- |
| Interactive macOS developer | Run the familiar tool through one wrapper; use its terminal normally | Lost terminal input, duplicate signals or a false “finished” label |
| Script author | Substitute `arcctl exec --` while retaining bytes, streams and process result | ARC prose corrupting a pipeline or replacing exit code 17 |
| Recovery operator | Inspect an existing lifecycle ID after daemon loss | Starting a second child because an acknowledgement was lost |

| Approach | Possibility | Decision for this slice |
| --- | --- | --- |
| Transparent wrapper and optional status | One invocation; child owns its ordinary interaction; separate recovery view | Selected: closest fit to the required process behavior |
| Execution console | Continuous ARC state and history in the execution view | Defer: additional interface and stream/terminal interference risk |
| Classify-confirm-execute wizard | Classification visible before launch | Defer: extra decisions and repeat-use steps without observed benefit |

No normal prompt, spinner, banner, progress percentage, retention message or exit barrier is permitted. A future help-order experiment is only a hypothesis; no conversion, churn, retention, commercial or accessibility-study result is claimed. No simulation was run. Human task success and screen-reader usability remain unmeasured; deterministic terminal tests establish interface behavior only.

## Execution help and direct arguments

The following is the required semantic help content; wrapping and alignment may follow existing Clap formatting. Help exits 0, writes only stdout, uses no ANSI codes and requires no daemon connection.

```text
Run a program and its arguments directly.
Usage: arcctl exec -- <PROGRAM> [ARG...]

Examples:
  arcctl exec -- cargo test
  arcctl exec -- printf '%s\n' 'two words'
  arcctl exec -- '/path/with spaces/tool' '' '--help' '|'

Your shell handles quoting before ARC receives arguments.
ARC does not interpret pipes, redirections, substitutions or shell operators.
Start arc-daemon before managed execution.
Stopping the daemon does not stop the running child.
Use arcctl job status to find lifecycle IDs.
Use arcctl job status <JOB_ID> to inspect an existing job without launching it.
Lifecycle tracking is available; Phase 1 scheduling is not implemented.
```

`--` separates ARC options from the program. Every subsequent argument, including `--help`, `--json`, empty arguments and literal `|`, belongs to the child. A program path containing spaces is one executable name, not executable-plus-arguments. `arcctl exec -- 'cargo test'` attempts that literal executable name; ARC never repairs it by splitting it. To request shell behavior, a caller can explicitly supply a shell executable and its arguments; ARC itself never constructs shell text. Classification must not change the executable arguments or execute text to determine a class.

Capture native `OsString` arguments and pass them directly to the process API. Preserve native non-UTF-8 bytes on macOS; unsupported classification becomes UNKNOWN without lossy conversion or refusal to execute. Native process APIs cannot accept embedded NUL bytes; reject such constructed input before spawn with a static error. Ordinary operating-system argument/environment size limits still apply, with sanitized spawn failure.

Normal use needs no preliminary classify/status/doctor call, confirmation or scheduling choice. First use starts the daemon and waits for its readiness using existing facilities. This slice adds no automatic daemon startup. Remote and UNKNOWN commands still execute once through the wrapper, with no permit or resource reservation. Classification is metadata, not an execution restriction or a scheduling promise.

## Optional lifecycle status and ID discovery

Specify a separate read-only surface, `arcctl job status [<JOB_ID>]`, backed by typed `job.status`. With an ID, report that job. Without an ID, show at most 50 recent records containing opaque IDs, registration/updated timestamps, last-known states and static reasons, ordered by registration time descending with ID descending as the tie-breaker. These summaries are not fresh process observations: mark `Last known: true`; select an ID for current reconciliation. An empty list includes `No lifecycle records.` and exits 0. More than 50 records adds `Truncated: showing the 50 most recent records.`; do not imply an exhaustive history. No command text, environment, working-directory path or child output is needed to discover an ID. Concurrent records can be distinguished by ID and timestamp; a “latest” row alone is not proof of which child the operator intends.

This surface never starts, resumes, replays, stops or reserves work. Its CLI spelling and the enumeration requirement are specified here for the implementation owner; wire schemas and bounded storage queries remain that owner's responsibility. If another adopted interface cannot provide enumeration, report the exact interface conflict to the Chief rather than dropping ID discoverability or inserting banners into child streams.

IDs are 32 lowercase hexadecimal characters, per the existing `contract.json` JobId schema. Do not embed command text or secrets in them. Timestamps use labelled Unix milliseconds, matching the adopted interface; these examples are illustrative fixtures, not observed records. Example recent-list entry:

```text
Job: 0123456789abcdef0123456789abcdef
State: running
Registered (Unix ms): 1794128400000
Updated (Unix ms): 1794128401000
Reason: CHILD_VERIFIED
Last known: true
Scheduling: not implemented
```

Example by-ID output:

```text
Job: 0123456789abcdef0123456789abcdef
State: running
Identity: verified at observation
Registered (Unix ms): 1794128400000
Observed (Unix ms): 1794128401000
Reason: CHILD_VERIFIED
Scheduling: not implemented
```

| Text label | Evidence required and meaning |
| --- | --- |
| `starting` | Registration exists; child launch/identity has not been verified. This does not establish that no child exists. |
| `running` / `Identity: verified at observation` | PID and native process-start token match a live process at the stated observation time. It can exit immediately afterward. |
| `completed` / `Exit code: 17` | An observed normal child exit was durably recorded; include completion timestamp. |
| `spawn failed` / `Reason: SPAWN_FAILED` | The launch owner observed spawn failure; no child was created. |
| `terminated by signal` / `Signal: 2` | The launch owner observed native signal termination; do not invent a normal exit code. |
| `outcome unknown` / `Reason: PROCESS_MISSING` | Verified observation cannot find the recorded process; its exit status was not observed. |
| `outcome unknown` / `Reason: IDENTITY_MISMATCH` | PID exists but its native start token differs; never attribute the replacement process to this job. |
| `outcome unknown` / `Reason: IDENTITY_UNAVAILABLE` | Process identity could not be determined; neither running nor completed is established. |

State/reason labels are bounded, static values. Use `unavailable` for timestamps or identity fields never observed; never fabricate them. An opaque missing ID produces `JOB_NOT_FOUND` and `No lifecycle record was found for this ID.`, nonzero exit and empty stdout; absence of a record does not prove absence of a child. Status transport failure produces `STATUS_UNAVAILABLE` and `Job status is unavailable. No work was launched by this status request.` Status query success exits 0 even if the job failed or its outcome is unknown: the query result and child result are separate concepts. No raw untrusted ID is reflected in error text.

## Launch ownership and recovery

One invocation allocates one stable opaque lifecycle ID. Confirm durable registration and a same-owner durable launch claim through `job.start` REGISTER/CLAIM before attempting spawn. An unchanged claim reply is usable only by the same still-live invocation before its one spawn site; it never authorizes a new invocation or recovery process to launch. The CLI owns at most one child; neither `job.start`, `job.status`, `job.complete` handlers nor IPC retries spawn it. Retries retain the same lifecycle ID and operation content. Repeated completion with the same outcome returns the existing outcome; conflicting completion is rejected without replacing the recorded result. A process that exits before identity attachment still has its observed result recorded; do not relaunch it to obtain identity.

A lost registration reply before spawn is different from uncertainty after spawn. The local launch owner can truthfully report “not started” only while it knows it never called spawn, or after a definitive failed spawn. A recovered daemon cannot infer this solely from a `starting` record. After a possible launch, use the same ID for attachment/completion recovery and reconciliation; never launch another child to resolve uncertainty. Reconciliation verifies identity, reports live/missing/indeterminate state, and never replays work or guesses an unobserved exit code.

New exec-owned errors use the following text, empty stdout, LF termination and no ANSI. Codes are specification labels for downstream mapping, not claims about existing enums. Apart from the validated opaque ID, the messages are static; do not print argv, executable paths, environment, cwd, OS error strings or remote error messages.

| Situation | stderr example | Result |
| --- | --- | --- |
| Missing program or malformed ARC exec options | `INVALID_REQUEST` then `Use arcctl exec -- <PROGRAM> [ARG...]. No child was started.` | Exit 2; zero launches and no daemon call for parse errors |
| Daemon unavailable or registration cannot be confirmed before spawn | `NOT_STARTED` then `No child was started. Check the ARC daemon connection.` | Exit 1; zero launches, even if a registration commit succeeded |
| Native spawn definitively fails | `SPAWN_FAILED` then `No child was started. Check the executable and execution permissions.` | Exit 1; persist spawn failure when reachable; never echo failing path |
| Attachment acknowledgement is lost after launch, child still supervised | `TRACKING_UNCERTAIN` then `Job: 0123456789abcdef0123456789abcdef` then `Lifecycle tracking is uncertain. Inspect arcctl job status for this job. Do not launch another copy.` | Continue waiting for the existing child; eventual child result wins |
| Child exit 17 observed but completion acknowledgement is lost | `COMPLETION_UNCONFIRMED` then `Job: 0123456789abcdef0123456789abcdef` then `The child exited with code 17. Recording is unconfirmed. Inspect arcctl job status for this job. Do not launch another copy.` | Exit 17; retry completion with the same ID within bounded policy |
| Outcome cannot be observed | `OUTCOME_UNKNOWN` then `Job: 0123456789abcdef0123456789abcdef` then `The child outcome is unknown. Inspect arcctl job status for this job. Do not launch another copy.` | Wrapper failure, exit 1, explicitly not a child exit-code claim |

Observed numeric exit codes/signals in diagnostics come from native wait results, never request text. Ordinary successful tracking emits none of these messages. Exceptional tracking diagnostics may use stderr only; never add stdout output. Do not delay returning an observed child result indefinitely for bookkeeping. The inspected `exec/transport.rs` specifies at most three attempts, a two-second overall deadline per metadata call and a 600 ms cap per attempt. These are source observations, not measured latency. Test those finite bounds against stalled peers; preserve the existing bounded IPC and shutdown contract. Exhausting a bookkeeping budget must not kill the child, relaunch it or overwrite its result. Daemon shutdown/crash alone does not make a supervised child's actual result unknown to the CLI.

## Process and terminal contract

- Preserve caller cwd and the inherited environment, including PATH lookup semantics; add no ARC variables to the child and remove no caller variables. Use neither shell reconstruction nor an environment wrapper to launch it.
- Inherit stdin, stdout and stderr descriptors directly, including closed descriptors, pipes, file redirection and TTY attachment. ARC must not read, buffer, merge, capture, prefix or consume child bytes. No background thread competes for terminal input.
- In a controlling terminal, keep the child in the caller's shell job, including when that job is a pipeline. Do not call `setpgid`/`process_group(0)` or `tcsetpgrp` for this shared-job path. Preserve foreground process-group behavior. Interactive reads must work without SIGTTIN/SIGTTOU failures. Ctrl-C/Ctrl-\\ reach the child through ordinary terminal signaling exactly once; do not forward a second copy of a signal already delivered to its process group.
- Preserve terminal-generated Ctrl-Z/SIGTSTP, stopped-job reporting and foreground/background resume against the direct-launch baseline. After `fg`, input works and the same child continues; suspension is not completion or a reason to create a new lifecycle ID. A background invocation must not steal the foreground.
- For a caller-created foreground pipeline, replacing one command with the wrapper preserves the other pipeline processes' terminal access and terminal-generated signals. Do not give the wrapped child exclusive foreground ownership at the expense of its pipeline peers. The caller's shell creates this pipeline; ARC still receives and executes only its argv.
- Forward directly received non-terminal signals (`SIGTERM`, `SIGHUP`, `SIGUSR1`, `SIGUSR2`) to the live child PID. In the shared-job path, do not forward terminal `SIGINT`, `SIGQUIT` or `SIGTSTP` again: the terminal delivers them to the whole job. Observe stops/continues with `waitpid` and `WUNTRACED|WCONTINUED`; mirror a child stop by stopping the wrapper with `SIGSTOP`, then continue supervising the same child after `SIGCONT`. Whole-job continuation must not be duplicated; a child-only stop requires resuming that child when its wrapper resumes. Keep signal handling distinct from bookkeeping so interrupted IPC cannot trigger another spawn. Only use a separate child process group when no controlling terminal exists or the invocation is explicitly detached/managed without shell-job peers; never infer detachment merely from redirected standard descriptors. Do not create a new session to obtain daemon independence.
- Preserve the child's inherited signal mask and ignored dispositions, including SIGCHLD, while allowing the wrapper to reap its own child. Restore wrapper-installed handlers/masks during cleanup and clear child ownership before PID reuse can receive a forwarded signal.
- Normal exit returns the child's exact exit code, including 17. Observed signal termination preserves the native signaled result after required local cleanup, rather than reporting an invented normal exit code. Shells report signal termination as `128+signal`; preserve the existing native parent-visible signaled wait status after cleanup. Test that status, not only shell `$?`.
- Preserve terminal modes and foreground ownership on normal exit, spawn failure and interruption to the extent the ordinary direct-child baseline does. Restore any ARC-made changes; do not claim arbitrary child-induced terminal changes can always be repaired. Test against the same native fixture run directly.
- Daemon graceful shutdown and forced death do not terminate, stop or reap the CLI-owned child. A long-running child holds no IPC admission slot or database ticket. Daemon independence does not claim survival of terminal-session closure or arbitrary wrapper death.

## Diagnostic and protocol compatibility

Preserve the existing `status` line exactly, including dynamic socket substitution:

```text
running. Socket: <socket>. Protocol: 1. Sweeper off.
```

This proves daemon liveness, not a job outcome. Do not append job information or change it to a scheduling claim. Preserve `doctor`'s existing prefix and its schema-dependent suffix:

```text
ok. IPC owner-only. Tray not connected. Vendors not detected. Command classification not checked. Sweeper off. Schema compatible. Version <supported_version>. Migration history verified.
```

Retain legacy schema-detail/error cases; an additive migration may change the truthful supported version, not the diagnostic grammar. Preserve classify's five-line human result (`Classification`, `Locality`, `Reason`, `Uncertainty`, `Command not executed.`), its four-field JSON result, static errors, exit behavior and no-execution guarantee. Retain legacy usage errors, `arcctl 0.1.0` version output, protocol-1 clients and existing bounded admission/shutdown regressions. Adding exec help must not replace existing classify guidance about one quoted command string. Scope secret-safe parse handling to exec while preserving existing diagnostics.

Persist only allowlisted lifecycle metadata: opaque ID, typed classification, state/reason, verified numeric identity, timestamps and observed outcome. Persist no raw argv, environment values, child output or hashes derived from secret-bearing command text. Do not put these in diagnostics either. Child-owned output can legitimately contain caller data; test ARC-generated diagnostics separately rather than promising to censor the child.

This is lifecycle management on native macOS, not a scheduler or resource optimizer. Desktop, native Windows and Phase 1 scheduling remain outstanding. All classes remain subject to existing finite transport/storage admission; that is not a resource reservation or scheduling policy.

## Deterministic implementation and review assertions

These are obligations for the implementation owner and independent QA, not tests executed by this documentation task. Use native fixtures, local fake protocol peers, bounded synchronization barriers and libc PTYs; no real remote service, shell reconstruction, browser suite or participant study is needed.

| ID | Fixture and assertion |
| --- | --- |
| E01 | Help/parse fixtures: exact semantic help and examples; help exit 0/stdout only/no IPC; malformed exec exit 2/empty stdout/static secret-safe stderr; no child launch. |
| E02 | Native argv fixture receives exact argument count and bytes: empty, spaces, Unicode, non-UTF-8, leading dashes, executable path with spaces and literal `;`, `|`, `$()`; marker proves no shell interpretation. |
| E03 | Fixture compares cwd, caller environment including a sentinel and PATH lookup against direct launch; descriptor checks include closed stdin, pipes and redirected stdout/stderr. Byte-for-byte stream comparison has no ARC additions in normal execution. |
| E04 | Child returns 17 after distinct stdout/stderr bytes; wrapper returns 17. Repeat with lost completion reply and finite bookkeeping failure: observed child result remains 17 and diagnostics contain no sentinel. |
| E05 | PTY fixture compares direct launch with wrapper: isatty, controlling terminal, foreground group, interactive input, terminal modes and restoration on success/failure/interruption. Assert parent-visible native signal status, including Ctrl-C, not only printed output. |
| E06 | Deliver terminal-generated SIGINT/SIGQUIT and wrapper-only SIGTERM/SIGHUP/SIGUSR1/SIGUSR2 using barriers; count child observations to exclude duplicate terminal forwarding and verify PID-only non-terminal forwarding. Compare inherited ignored dispositions/masks, including ignored or blocked SIGCHLD, against direct launch; reap exit 17 and stop forwarding after reaping. Zero extra invocation after interrupted IPC. |
| E07 | Definitive missing/non-executable program: zero child marker, sanitized SPAWN_FAILED, no raw path/secret; persistent spawn-failure state when recording succeeds. |
| E08 | Lose replies around registration, identity attachment and completion commits, retry identical operations and query the same ID: exactly one child marker and one lifecycle record. Pre-spawn registration ambiguity produces zero launches and NOT_STARTED. |
| E09 | Duplicate completion is idempotent; conflicting completion does not overwrite exit/signal. Fast-exit fixture records its actual result without restart or a false verified-running claim. |
| E10 | Keep child behind a barrier while stopping/killing its owned fixture daemon; child continues, descriptors work, wrapper eventually returns its actual result. Restart daemon: verified identity reconciles without any second invocation. |
| E11 | Missing process, PID/start-token mismatch and unavailable identity produce distinct outcome-unknown reasons; no guessed exit 0, false running state or replay. Inspect observation timestamps. |
| E12 | ID discovery lists at most 50 records, ordered by registration time/ID descending, with updated time, reason, explicit last-known labelling and a truncation indication; malformed IDs never echo their input. Lists are not fresh liveness proof and contain no argv/cwd/environment/output; inspect by ID; empty list, truncation, unknown ID and unavailable status have the specified meanings. Status requests never start work. |
| E13 | Remote/UNKNOWN fixtures execute once with exact arguments and zero reservations/permits. No external endpoint is needed to establish classification and execution pass-through. |
| E14 | Sentinel audit covers database rows, raw persistence artifacts including WAL when present, ARC diagnostics and retained evidence; argv/environment sentinel absent. Child output is assessed separately. |
| E15 | Existing status/doctor/classify fixtures retain output, exits and protocol 1 before/after execution and daemon restart; admission/shutdown tests retain their original assertions. |
| E16 | Under a controlling PTY, compare direct foreground pipeline and wrapped equivalent using native producer/consumer fixtures and readiness barriers. Assert wrapper, child and peers remain in the caller-created job's process group, peer terminal reads work, foreground signals reach each intended process once, no SIGTTIN/SIGTTOU or wait hang occurs, all statuses are reaped and foreground ownership returns. Keep the wrapped child exit 17 assertion separate from the pipeline's final-command result. |
| E17 | PTY stop/resume fixtures: terminal Ctrl-Z stops the job, the caller regains control, `fg` resumes the same child and restores input; a background start never steals the foreground, background continuation retains job ownership, and later foregrounding works. Cover child-only stop/continue separately from a whole-job terminal stop. Bound every wait and assert one launch, no false completion and preserved final result. |

## Current source observations for the Chief

At Actor-local snapshot `7eb4a919309740202cab6f8e07bd24de5352800a`, `crates/arc-cli/src/job.rs`, `run`, Recent arm now prints updated time and static reason as well as ID, state, registration time and `Last known: true`. The previous report's omission claim does not describe this snapshot. No populated-history test was executed here.

At that same snapshot, `crates/arc-cli/src/exec/terminal.rs`, `shares_job`, checks inherited descriptors and `/dev/tty` for a controlling terminal. `crates/arc-cli/src/exec.rs`, `run`, creates a child process group only when `shared_job` is false. `crates/arc-cli/src/exec/signals.rs` uses quiet terminal-signal handlers for a shared job, PID-directed non-terminal forwarding, and wait/stop/continue supervision. Those are file observations consistent with the operator's requested model; they are not a fresh native pipeline, signal or acceptance result. Backend and independent QA must verify E05, E06, E16 and E17 against current integrated sources after the operator note.

The adopted `contract.json` still contains historical annotations saying that Recent lacks updated time/reason and that the terminal repair is unresolved. Its wire types, CLI labels and recovery contract are compatible with this report; those source-status annotations differ from the current files above. Chief should coordinate annotation and dependent-evidence refresh with their owners. This task changes no contract JSON or product source.

The 32-hex ID, 50-row bound, last-known discovery, Unix-millisecond labels, REGISTER/CLAIM/ATTACH ownership and static recovery copy remain unchanged. No instruction here grants status or reconciliation permission to launch work.

## Handoff and document verification

Acceptance checklist for this assigned report:

- [x] Help, ID discovery, status labels and static error examples are concrete; E01–E17 define deterministic checks.
- [x] Lost acknowledgements cannot authorize replay; observed child outcomes take precedence over recording failures.
- [x] Existing diagnostic/protocol behavior is preserved, and lifecycle scope is separate from scheduling.
- [x] Personas and proposed usability effects are hypotheses; no study, screen check, simulation or retention result is claimed.

Commands executed in this attempt:

| Command or inspection | Outcome |
| --- | --- |
| `pwd`; scoped `git status --short --untracked-files=normal -- . ':!av.yaml'`; owned-file staged/unstaged diffs | Exit 0; entry worktree clean within this scope; no preceding task edits reset |
| `git rev-parse HEAD HEAD^{tree}`; `git show 19d7a507044cbf83918f58cd41056905c275c8d4:crates/arc-cli/src/main.rs`; `git ls-files` with named paths | Exit 0; snapshot and baseline distinguished; accepted-baseline CLI has status/doctor/classify and no exec command |
| Combined baseline-tree/ref lookup; separate `git rev-parse --verify` for each supplied ref | Exit 128 for absent named refs; baseline tree resolves to the value above, and local HEAD supplies the inspected snapshot identity |
| Direct `cat`/`sed` reads of this report, named CLI sources/tests, contract JSON, runtime usage and OMA policy/skill references; inline Python hash/JSON inspection | Exit 0; local source observations only; no execution or retained pass receipt reused as current verification |
| Memory-registry `rg` for ARC/arcctl/managed-exec/transparent wrapper | Exit 1, no relevant hits; no memory-derived claim used |
| Initial inline Python document check | Exit 1; explicit `128+signal` wording was absent. Added the shell convention while preserving the native signal-status contract; this was a document-check failure, not a product test |
| Inline Python one-file edit and final document/hash/boundary inspection | Exit 0; 17 unique assertions, 11 matching source/interface hashes, verified baseline/snapshot trees, valid example IDs, required content and only this file changed |
| `git diff --check -- .arc-verify/managed-exec/cli-contract.md` | Exit 0; no whitespace diagnostics |

Test approach: automated product tests are not applicable to this report-only task. Verification is document inspection, provenance/hash comparison, assertion inventory and a one-file diff check. No build, Cargo test, coverage collection, harness, browser/GUI operation, screen check, installation, simulation or external delivery was run. Human usability, accessibility, retention and conversion outcomes remain unmeasured.

No product code, test, manifest, lockfile, migration, saved assignment, mission state or other Actor output was changed. The mission still requires cargo fmt, warning-denying clippy, workspace tests, coverage-verification inventory and the full harness without skips, unchanged `0001_phase0.sql`, source files within 500 lines, per-crate 80% production-line floors and independent QA. This task does not certify those gates or alter them.

Remaining risks and handoff: native process behavior and human task success are not established by this report. Native macOS acceptance completes this lifecycle slice; desktop, native Windows and Phase 1 scheduling remain outstanding. `.arc-verify/managed-exec/receipt-index.json` binds the preceding report hash `0666901675d70dd7397fd113542f26a658f336724eed4776103a6bb70694c073`; this authorized one-file edit invalidates that artifact binding. Chief must verify the delivered bytes in the integrated worktree and have the assigned evidence owner refresh dependent bindings before reuse. No receipt or prior pass claim was rewritten here. Missing named refs are a provenance limit, not an invented revision or a blocker to this specification. No ownership conflict prevented the assigned edit.
