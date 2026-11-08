# Managed exec technical contract

Status: task-1 contract completed, attempt 3. Method: `oma-architecture`, comparing launch ownership and failure cases. This report specifies implementation and verification requirements; it does not grant product acceptance. Only this file is owned by this task. That explicit scope overrides the skill's generic ADR, diagram-sibling and run-report destinations. No workflow, assignment or mission state is changed.

## Source scope and handoff

The accepted committed source baseline is `19d7a507044cbf83918f58cd41056905c275c8d4`, tree `d2f8ace96256a7e073ab9d1bfb10fdc5cae3bf38`. Actor-local HEAD and the supplied input snapshot commit are `a92046c4467a9a612ffc2b0010452861f8b1c265`, tree `718ed68af094726b7487daaa2d37d8c00214302f`. Both commit/tree pairs were resolved locally. The latter contains lifecycle implementation and reports absent from the accepted baseline. Snapshot tracking does not establish tracking or integration in another checkout.

The supplied refs `refs/agent-valley/task-evidence/a5b368321dcdfcee-7afaa346b4bf92bf-3/snapshot` and `refs/agent-valley/task-evidence/a5b368321dcdfcee-7afaa346b4bf92bf-3/worker` each fail `git rev-parse --verify` with exit 128, `fatal: Needed a single revision`. Their supplied names are preserved; no substitute revision is assigned to them. The supplied snapshot commit resolves independently.

Entry status and staged/unstaged diff statistics, scoped to exclude `av.yaml`, were clean. The existing report was tracked in the snapshot: 47,085 bytes, SHA-256 `8d88f11c2d057b167f16a0d2d4c19db9571849dd61d12ee7438ffa51abbcd604`. This revision is an uncommitted Actor-local change to that file only. The Chief must use the delivered bytes and hash after integration and rebind downstream references to this report. This task cannot edit `contract.json`, QA receipts, final summaries or another Actor's output.

The input `final-summary.json` reports accepted native macOS lifecycle results and independent QA. Those are retained claims, not freshly validated acceptance in this attempt. This report neither repeats the obsolete terminal blocker from its earlier revision as a current defect nor certifies that all reported gates still bind after integration. No Cargo, coverage collector or harness suite ran here. The host is Darwin arm64; host identity alone proves no lifecycle behavior.

Serena tools are unavailable in this session. Inspection used direct reads of already located files and Git objects, with no native recursive code search or GUI fallback. `docs/decisions/README.md` and the proposed `adr-runtime-execution-and-state-ownership.md` were read; their uncertainty rules are compatible but concern AV orchestration rather than ARC child ownership. No ADR is superseded. The operator's schema-2 and POSIX job-control clarifications are incorporated below. The abandoned collector note does not authorize this file owner to alter run records or launch collection.

## Architecture decision and cost

The problem is launch ownership across independently failing IPC replies, SQLite commits and native process creation. A transport retry must never become a second child, and daemon shutdown must remain bounded while work continues.

| Option | Implementation and future change cost | Operating cost and team complexity | Decision |
| --- | --- | --- | --- |
| CLI owns one native `Command` child; daemon persists metadata | Reuses native argv/descriptor inheritance, classifier, IPC and SQLite worker; needs a small caller state machine | One wrapper per invocation; short database work; no daemon child descriptors, waiters or joins | Choose, as required by the mission |
| Daemon launches and waits | Requires descriptor transfer, terminal ownership, durable launch fencing and child ownership beyond cancellable handlers | Adds persistent per-job tasks and shutdown/recovery machinery | Reject for this slice |

Costs are qualitative. No latency, savings, throughput or monetary result was measured. Reuse the six crates, maintained workspace dependencies and libc. No new crate, runtime, ORM, service or scheduler is needed. Lifecycle tracking is separate from scheduling capability. REMOTE_CI, REMOTE_EXEC and UNKNOWN commands execute directly without resource reservations; Phase 0 creates no permits for any class.

```mermaid
sequenceDiagram
    participant C as CLI invocation
    participant D as Authenticated daemon handler
    participant W as Bounded SQLite worker
    participant P as Native child
    C->>D: job.start REGISTER then CLAIM, stable job_id
    D->>W: Short transactional operations
    W-->>C: Durable snapshot through IPC
    C->>P: Command spawn once, outside metadata retries
    C->>D: job.start ATTACH native identity
    C->>P: Wait with inherited descriptors and job control
    C->>D: job.complete observed result, same value on retries
    Note over D,W: job.status reconciles identity; never launches
```

## Existing boundaries and reuse

All references in this table are to accepted baseline `19d7a507044cbf83918f58cd41056905c275c8d4`. Extensions are requirements, not claims that the feature exists at that commit.

| Boundary | Source reference | Required use |
| --- | --- | --- |
| CLI diagnostics | `crates/arc-cli/src/main.rs:72-93,128-145` | Preserve status/doctor/classify behavior. Add exec and separate optional job status. Request IDs remain correlation metadata. |
| Classifier | `crates/arc-detector/src/lib.rs:13-32`; `rules.rs:6-50` | Add argv adapter using existing recognition and budgets; do not join argv and invoke the shell-text parser. Preserve shell-text classify. |
| Protocol/framing | `crates/arc-ipc/src/lib.rs:22-67,220-269` | Keep protocol 1, existing envelopes, version-first errors and 1 MiB frame cap; add typed operations. |
| Authentication/admission | `crates/arc-daemon/src/admission.rs:36-64,98-105,116-135` | Same-user credentials before handler construction; nonwaiting finite admission; aborted handlers do not own children. |
| Payload lifetime | `crates/arc-daemon/src/connection.rs:95-127` | Decode bounded typed metadata before await, then clear raw JSON. No command/environment payload enters SQLite work. |
| Single database worker | `crates/arc-daemon/src/db_worker.rs:71-110,145-173,244-313` | Extend jobs/tickets with operation/result enums on the same finite channel and semaphore; retain doctor compatibility and ticket-drop cancellation. |
| Cancellation | `crates/arc-daemon/src/db_worker/execution.rs:10-21,25-80,93-123` | Retain sticky stop/deadline/cancel checks, busy budget, progress hook and interrupt. A failure after COMMIT is ambiguous. |
| Shutdown | `crates/arc-daemon/src/lifecycle.rs:140-187`; `limits.rs:19-32,64-102` | One absolute deadline for handler abort/reap and worker interrupt/join; socket cleanup and zero-gauge accounting; maximum 3,500 ms. |
| Native identity | `crates/arc-daemon/src/platform.rs:67-120` | Move/reuse native start/liveness helpers through arc-core; conservative boolean alive is not verified identity. |
| Migrations | `crates/arc-storage/src/migration.rs:5-10,97-119,142-173` | Append 0002 to the compiled registry; retain sequencing, checksum/history checks, immediate transaction, rollback and version witness. |
| Jobs and permits | `crates/arc-storage/migrations/0001_phase0.sql:39-67` | Reuse jobs summary; add lifecycle details through 0002; no resource permit insertion. |

At inspected snapshot `a92046c4467a9a612ffc2b0010452861f8b1c265`, these files implement the proposed boundaries. Hashes identify inspected input bytes; they do not bind the revised report or another checkout.

| Snapshot path | Observation | SHA-256 |
| --- | --- | --- |
| `crates/arc-cli/src/exec.rs:50-184` | One direct spawn at line 104; process group created only outside a shared terminal job | `238937ec7f455a1dc3832489bb15aad183c6c7e7f8338ea2a82777ef341cca80` |
| `crates/arc-cli/src/exec/terminal.rs:3-17` | Detects controlling terminal, including /dev/tty; no foreground transfer | `c3a5c74713d36c1fde3a332563d5cdb4260aa341a86e50573587f9eb95a882df` |
| `crates/arc-cli/src/exec/signals.rs:55-115,137-228` | Shared-job terminal signals are not forwarded; native stop/continue/wait and SIGCHLD preservation | `7424005b7228de7a5adc7ed9e4e46babdc0a6f18a5ebb44e17dd4e58021cfca6` |
| `crates/arc-ipc/src/jobs.rs:12-118,146-176` | Typed lifecycle operations and bounded static-error decoder | `c75464c6cd8239c862ba100a501975b2189ae37aa6202a0931f7875432619a3e` |
| `crates/arc-daemon/src/db_worker/jobs.rs:10-55,62-123` | Existing worker dispatch; by-ID reconciliation retries at most three times | `37423dee5cd9827f79e69d505e4f7f50745c39d571f6421fa8051f702e36cc6d` |
| `crates/arc-core/src/process_identity.rs:38-156,211-295,340-363` | Boot/start identity, live and unreaped probes, conservative lock helper and native random key | `1c7c7a5e9dce3941541ca0646a22aaab1713f3047334eebc7e533d0110ad8f56` |
| `crates/arc-storage/migrations/0002_managed_exec.sql` | Additive allowlisted detail table, constraints and recent index | `464a1c88270f01a56ccab6610f87eff7648e0640f276ebce53b8a97026c50814` |

Other inspected snapshot boundaries are `arc-detector/src/argv.rs`, `arc-cli/src/exec/{transport,descriptors}.rs`, `arc-ipc/src/jobs/types.rs`, `arc-storage/src/jobs/{transitions,reconcile}.rs`, and daemon connection/worker cancellation. Snapshot `connection.rs:124-136` clears the payload before dispatch await; `db_worker/execution.rs:115-123` checks cancellation on both sides of lifecycle execution; `db_worker.rs:342-345` cancels dropped tickets.

## Identities and wire contract

Keep three identities distinct:

1. `request_id`: exchange correlation echoed by the envelope; retries may reuse or change it. It does not authorize launch or deduplicate work.
2. `job_id`: one opaque 128-bit OS-random key, fixed 32 lowercase hexadecimal characters, allocated once per CLI invocation before registration. Reuse across metadata retries only. Never derive it from argv, environment, cwd, PID or time; RNG failure is a static prelaunch failure. A new invocation gets a new key and cannot resume execution from an old key.
3. `ProcessIdentity`: `{pid, start_token, boot_id}`. PID is in 1..2,147,483,647; start_token is canonical nonzero u64 decimal text; boot_id is a bounded canonical lowercase boot UUID. Observer and child are separate tuples. macOS tokens are start microseconds; Linux tokens are boot-relative clock ticks. Daemon instance identity is neither tuple.

Boot identity uses native macOS boot-session UUID or Linux boot ID, through arc-core beside the existing native start helper. A daemon restart in the same OS boot does not invalidate a live child. Boot/start mismatch is not a match; permission, unsupported platform and incomplete reads are INDETERMINATE. No synthetic zero or daemon nonce substitutes for unavailable evidence. Same-user authentication is the trust boundary; parent completion is not independent proof against a malicious process of that user.

Use Serde structs/tagged enums, deny unknown fields on new payloads, validate nested object shape and enum consistency before worker admission, and expose only static errors. Snapshot wire types are `arc-ipc/src/jobs.rs:21-118` and `jobs/types.rs:32-162`. Preserve their spelling and coordinate any discrepancy with the Chief-owned `contract.json`.

| Operation | Request payload | Result |
| --- | --- | --- |
| `job.start` register | `{action:"REGISTER", job_id, observer, classification}` | `StartResult {disposition:"CREATED" or "UNCHANGED", job:JobSnapshot}` |
| `job.start` claim | `{action:"CLAIM", job_id, observer}` | `StartResult {disposition:"APPLIED" or "UNCHANGED", job}` |
| `job.start` attach | `{action:"ATTACH", job_id, observer, child}` | `StartResult {disposition:"APPLIED" or "UNCHANGED", job}` |
| `job.complete` | `{job_id, observer, outcome}` | `CompleteResult {disposition:"APPLIED" or "UNCHANGED", job}` |
| `job.status` by ID | `{query:"BY_ID", job_id}` | `{kind:"FOUND", job}` or `{kind:"NOT_FOUND"}` |
| `job.status` recent | `{query:"RECENT"}` | `{kind:"RECENT", jobs:JobSummary[], truncated, observed_at_ms, liveness_fresh:false}`; at most 50 indexed last-known rows |

`Outcome` is `{kind:"EXITED", code:u8}`, `{kind:"SIGNALED", signal:u8, core_dumped:bool}`, or `{kind:"SPAWN_FAILED", reason, os_code:i32|null}`. Spawn reasons are NOT_FOUND, PERMISSION_DENIED, INVALID_INPUT, RESOURCE_UNAVAILABLE and OTHER. The nullable os_code field is required, never a free-form error. Signal range is 1..127. Timestamps/revisions must fit the storage/wire safe integer range ending at 9,007,199,254,740,991; revision starts at 1. Lifecycle requests have an 8,192-byte serialized cap inside the unchanged frame cap.

`JobSnapshot` contains job_id, phase, allowlisted classification, observer, observer_verified_at_ms, nullable child and child_verified_at_ms, typed observation, static reason, nullable outcome, registered_at_ms, nullable launch_claimed_at_ms/completed_at_ms, updated_at_ms and revision. `IdentityObservation` contains target (OBSERVER/CHILD/NONE), status (VERIFIED_MATCH/MISSING/MISMATCH/INDETERMINATE/NOT_ATTACHED), observed_at_ms and reason. Past verification time is not current liveness. `JobSummary` contains job_id, phase, registered_at_ms, updated_at_ms and static reason; preserve their per-ID associations when formatting populated recent results.

No payload/result stores argv, environment, cwd, executable path/name, child output or free-form diagnostic text. Do not substitute command hashes: they still expose command-derived data. Reject malformed tags, nulls in required nonnullable fields, invalid ranges and contradictory phases/outcomes with INVALID_REQUEST, without Serde input excerpts. Retain old envelope errors. New static errors are JOB_CONFLICT, INVALID_JOB_TRANSITION and PROCESS_IDENTITY_UNAVAILABLE. Status absence is typed NOT_FOUND; missing-row mutation is invalid transition. DB_BUSY/deadline/worker loss is an operational failure, never evidence of rollback.

## State and launch protocol

| Phase | Meaning | Allowed refinement |
| --- | --- | --- |
| REGISTERED | Durable observer/classification; no launch claim | CLAIM → LAUNCHING; observer disappearance retains not-claimed fact |
| LAUNCHING | Claim committed; spawn may or may not have happened | ATTACH with native evidence; actual completion; otherwise unresolved/OUTCOME_UNKNOWN |
| RUNNING | Attached tuple was verified live at an observation time | Reconcile current identity; actual completion; missing/mismatched/indeterminate evidence → OUTCOME_UNKNOWN |
| COMPLETED | Parent observed normal exit, with exact code including 17 | Immutable; identical completion returns unchanged |
| SIGNALED | Parent observed signal termination and core flag | Immutable; no invented numeric child exit |
| SPAWN_FAILED | Native spawn returned an error; no executable was launched | Immutable; persistence retry never retries spawn |
| OUTCOME_UNKNOWN | No observed terminal result; launch/outcome cannot be established | Original owner may attach original tuple or submit actual wait result; no replay |

The conforming CLI performs these steps once per invocation:

1. Retain original `Vec<OsString>` locally. Use bounded argv classification without shell parsing; native non-UTF-8 or unsupported/budget-exhausting input becomes UNKNOWN while execution bytes stay unchanged.
2. REGISTER with the fixed key, observer and classification; require a validated matching REGISTERED snapshot before proceeding. Retry only metadata. Exhaustion before the local spawn site means this invocation starts no child, even if a registration committed.
3. CLAIM durably. A matching LAUNCHING acknowledgement, including UNCHANGED after response loss, allows only this still-live invocation before its one-shot launch point to proceed. Without confirmation, do not spawn. A new process/restart never interprets a stored claim as launch permission.
4. Enter the local one-shot guard and call `Command::new(&argv[0]).args(&argv[1..]).spawn()` exactly once, outside IPC retry handlers/closures. There is no shell reconstruction, `sh -c`, argument reparsing or daemon spawn. A spawn error consumes the launch attempt.
5. Capture the child tuple while retaining its unreaped handle, then ATTACH with bounded retries. Metadata failure cannot kill/relaunch the child or replace the observed outcome. Keep waiting while the daemon is unavailable.
6. Wait for the native result and COMPLETE with the same tagged observation on every retry. Return exit 17 as 17 regardless of recording failure. Persist signal termination separately. Spawn failure returns a distinct nonzero wrapper result and never a fake child exit.

Snapshot `exec/transport.rs::call` makes at most three metadata exchanges under one two-second deadline, each attempt bounded to 600 ms. It retries DB_BUSY/transport loss with the same request payload; it never launches. Snapshot `exec.rs:28-48,186-206` validates returned identity and outcome fields. These are inspected implementation choices, not measured latency guarantees.

Immediate transactions and unique job_id deduplicate durable mutations. Identical REGISTER with the same observer/classification returns unchanged; changed owner/classification conflicts. CLAIM commits once; duplicate requests converge. ATTACH binds exactly one child tuple; another tuple conflicts even after the first child disappears. COMPLETE from the same observer with the same tagged value is unchanged; changed code, signal, core flag, spawn reason/os_code or owner conflicts without touching the first terminal fields/time. SPAWN_FAILED after a child has attached is invalid. Observed exit without attachment is allowed when identity capture failed, but must not fabricate verified identity.

Concurrent identical metadata requests create one record; none owns a daemon spawn task. The local one-shot guard prevents duplicate replies from launching twice. This guarantees one child despite IPC/database retries for a conforming invocation. It does not guarantee eventual launch after CLI death between CLAIM and spawn, recover an unobserved exit after observer death, or provide a distributed atomic transaction with OS spawn. Those cases retain uncertainty and never replay work.

## Commit ambiguity and reconciliation

| Case | Required outcome | Deterministic check |
| --- | --- | --- |
| Concurrent same-key register/claim | One owner/row and one mutation; duplicates unchanged, conflicts rejected | Barrier-controlled concurrent submissions, both tables checked |
| Queued cancellation or proven precommit rollback | No partial summary/detail write | Worker barriers before SQL/COMMIT; retry same key |
| Timeout, abort, worker loss or reply loss after possible COMMIT | Ambiguous to caller; no assumption of rollback | Drop response before/after each REGISTER/CLAIM/ATTACH/COMPLETE commit; child marker stays exactly one |
| Fast exit before attach | Verify unreaped tuple without reporting zombie as live; then retain actual exit 17 | Immediate-exit fixture without child sleep; separate blocked-live fixture |
| Failed native identity capture | Actual wait result can complete with unknown identity | Inject failure; assert no fabricated VERIFIED_MATCH |
| PID reuse or different OS boot | MISMATCH, never original live child; no guessed exit | Same PID/different start and same PID/start/different boot |
| EPERM, unsupported or short native read | INDETERMINATE, not dead or complete | Separate injected native outcomes |
| Missing PID | MISSING/OUTCOME_UNKNOWN; exit/signal null absent observed completion | ESRCH fixture |
| Status probe races completion/attach | Revision and identity guard rejects stale evidence; terminal result wins | Pause probe, commit newer state, release probe |
| Daemon restart in mutation window | Durable state survives; matching retries converge; no replay | Owned daemon crash/restart barriers and invocation counter |
| Unknown then real parent completion | Original observer can refine to actual outcome once | Submit original wait value twice; preserve first terminal timestamp |

For ambiguous operations, use `job.status BY_ID` to inspect the same key and compare observer, classification, child tuple and outcome. NOT_FOUND permits repeating REGISTER under that key, never assuming an earlier possible launch did not happen. A matching committed state resolves the relevant mutation; a different owner or value is conflict. Status unavailable leaves uncertainty. Repeat only the original metadata operation/value within its budget. An actual status request is not required between successful idempotent retries: their validated snapshots can also establish commit. No status response grants a fresh invocation permission to spawn.

Prelaunch CLI code knows whether it reached its spawn site. It may truthfully print NOT_STARTED when it exits before that site even if the daemon retains LAUNCHING after a lost claim acknowledgement. Daemon status cannot infer that local fact from LAUNCHING alone. After launch, diagnostics distinguish TRACKING_UNCERTAIN, COMPLETION_UNCONFIRMED with actual observed result, and OUTCOME_UNKNOWN. Point to job status using an opaque key; never advise blind execution retry.

Use lazy by-ID reconciliation on the same worker: fetch a snapshot, release its ticket, probe native identity, then conditionally apply evidence bound to revision and tuple. Retry stale probes at most three times under the request deadline (`db_worker/jobs.rs:95-123`, snapshot). An observed terminal outcome always wins. No startup history scan, per-row background task or replay is needed. Recent status returns indexed last-known summaries with liveness_fresh:false and no per-row native probes.

REGISTERED retains its no-claim fact even when its observer disappears. LAUNCHING with no child remains unresolved, including when the observer lives. A matching attached live child can be RUNNING even without its original observer. A missing/mismatched/indeterminate child yields unknown outcome; original identity evidence or actual original-parent completion can later refine it. A missing PID never establishes code 0, 17 or any signal. A clock or validation error must leave prior state intact, not synthesize timestamps or outcomes.

Reuse `arc-core::process_identity::ProcessIdentity::capture` and the module functions `probe` and `probe_live`. Capture and identity-only probe may inspect an unreaped zombie; probe_live must not call that running. The snapshot macOS helper uses proc_pidinfo plus a bounded KERN_PROC zombie fallback (`process_identity.rs:38-156`); Linux distinguishes Z in proc stat. Native tests must verify immediate-exit PID/start/boot evidence before reap and distinguish it from liveness. Permission/partial reads remain indeterminate. Probes occur outside transactions and ticket ownership. Reuse the conservative lock helper without weakening existing daemon lock behavior.

## Transparent execution, signals and shutdown

Pass native argv directly, preserving empty arguments, spaces, leading dashes, shell metacharacters, executable paths containing spaces and non-UTF-8 bytes. Inherit PATH lookup, cwd, environment and stdio. Do not capture/relay child streams through ARC. Restore intentionally closed standard descriptors in the child when Rust startup opened replacements; ARC sockets must be close-on-exec. Tests compare direct and wrapped pipes, files and closed descriptors. Keep normal child streams free of ARC banners, spinners and job IDs. The secret prohibition concerns ARC records/diagnostics; it does not censor child-owned output.

Follow the operator's 2026-11-08 POSIX guidance. With a controlling terminal, keep child and wrapper in the caller's shell job, including when that job is backgrounded. Do not call setpgid/process_group(0) or tcsetpgrp in that path. The terminal sends INT/QUIT/TSTP to the job and its pipeline peers. The wrapper must not duplicate those signals to PID or group. Sender metadata or group membership cannot prove whether an external signal was PID- or group-targeted; do not invent an inference mechanism.

Forward directly received nonterminal TERM/HUP/USR1/USR2 only to the owned child PID. Wait with WUNTRACED|WCONTINUED, consume stop/continue events, mirror an outstanding child stop with wrapper SIGSTOP, and resume on SIGCONT without stealing foreground ownership. A target-only SIGSTOP may need PID-only continuation; normal job-wide stop/continue is delivered by the OS. Clear child ownership before unmasking after reap so a recycled PID cannot receive a late signal. Handle preattachment signals without launching another child. Retain inherited ignored dispositions and masks in the executed program. The wrapper needs its own SIGCHLD waiter even when the caller ignored/blocked CHLD or used NOCLDWAIT; restore the caller's disposition in the child and the original wrapper state when done.

Only a nonterminal or detached/managed invocation without caller job peers gets a private child group. Snapshot `terminal.rs:3-17` detects a controlling terminal even with redirected standard descriptors; `exec.rs:60-62,96-104` selects shared versus private group. `signals.rs:76-86` avoids shared-job INT/QUIT forwarding; `98-115,137-228` preserves CHLD, drains wait events and reports native signal status. Reraising the observed terminating signal after recording retains parent-visible signaled status and conventional shell 128+signal reporting; recording failure cannot replace that result.

Require direct/wrapped native PTY and real pipeline comparisons: terminal attachment/foreground ownership, peer terminal reads, input/echo/modes, INT/QUIT exactly once, wrapper-only TERM, whole-job Ctrl-Z, background continuation without foreground theft, foreground resume, terminal restoration, inherited ignored/blocked CHLD, closed descriptors and exit 17. The regression entry is `crates/arc-daemon/tests/managed_exec/pipeline.rs:3-24`; its no-terminal counterpart starts at line 26. `pgid == pid` is appropriate only for the no-terminal private-group case, not the caller's foreground job. The old unconditional private-group/foreground-transfer design is not present at the inspected snapshot; retained earlier failing receipts remain historical rather than current acceptance.

The CLI owns waiting, not a handler or database ticket. Release metadata connections/admission/tickets between operations; no request slot is held for the child's lifetime. Resource permits and transport admission are different concepts: resource permits stay unused while finite IPC admission remains enforced. Daemon shutdown/crash must not signal or wait for these children. Test only owned fixture daemon SIGTERM/SIGKILL; verify matching child identity and usable I/O after daemon exit, restart status, one completion and one invocation. Keep existing cancellation, shared shutdown deadline and zero-gauge/worker/handler accounting guarantees.

## Additive storage and module boundaries

Keep `0001_phase0.sql` byte-for-byte: SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`, verified against the accepted baseline. Append only necessary `0002_managed_exec.sql` through the existing checksummed migration registry. Reuse jobs.id/class/locality/state/outcome/timestamps. Use static observe/pass-through enforcement; leave reservation, workload signature, project and session fields null when no evidence exists. No fabricated project/session/vendor record or permit.

The job_lifecycle detail table holds observer/child tuples, claim/verification times, typed observation/reason, signal/core/spawn metadata and revision. Keep identity tuples wholly present or absent, start tokens lossless decimal text, foreign/unique keys and mutually exclusive terminal CHECK constraints. Update summary/detail together in an immediate transaction; never REPLACE away history. Server timestamps describe observations, not reconstructed exact process start/exit. No raw argv, environment, cwd, path, output, payload JSON or hashes of secret-bearing input. Upgrade tests cover fresh install, original 0001 rows/checksums, repeated upgrade, invalid history, newer schema, failed migration rollback and legacy witness semantics.

| Existing crate | Modules to reuse/extend | Responsibility |
| --- | --- | --- |
| arc-core | `process_identity.rs`, native submodules if needed | Start/boot/liveness identity, native key and lock compatibility |
| arc-detector | `argv.rs`, existing rules/budget | Classification adapter; native execution bytes stay local |
| arc-ipc | `jobs.rs`, `jobs/types.rs`, `jobs/validation.rs` | Typed operation/results, limits, consistency and static errors |
| arc-storage | `jobs.rs`, `jobs/{codec,transitions,reconcile}.rs`, migration registry | Atomic transitions, deduplication, immutable outcomes and guarded evidence |
| arc-daemon | `db_worker/jobs.rs`, existing execution/connection/lifecycle | Same bounded queue, cancellation and native reconciliation; no launch/wait service |
| arc-cli | `exec.rs`, `exec/{transport,descriptors,terminal,signals}.rs`, `job.rs` | One native launch, inherited behavior, metadata retries, wait and optional status |

Keep every source file within 500 lines; split tests/helpers before crossing that limit. The Lifecycle Engineer owns coupled product interfaces, tests, shared manifests and protection changes. This task modifies none. Prefer existing workspace dependencies; any added crate must enter coverage inventory and retain the 80% floor. Report outside-scope changes to the Chief rather than creating a second writer.

## Preservation and exact verification inventory

Historical whole-source obligations remain at their original commits: coverage baseline `fbe76a5f15101ad415f355ef06f1deeb1f97d568` and hygiene baseline `aacb3b4359f13c31c0705c0cffc5bf1ac00f387e`. Accepted-baseline `scripts/coverage/preservation.ts:27-46` and `hygiene.ts:88-119` applied whole-source restrictions; the feature must scope those historical contracts to immutable fixtures rather than reject authorized production changes. Preserve original token/byte/addition/removal/mode/migration rejection cases. Do not move historical baselines, return unconditional success, add broad exemptions or bypass via environment flags.

For live sources, protect unchanged 0001, harness path/type/executable mode and behavioral equivalence, complete existing test/helper items and their reachability, assertion/macro strength and coverage accounting. Positive tests must accept an authorized lifecycle source addition/change. Negative fixtures must reject removed/weakened assertions, changed helper inputs, early returns, ignored tests, cfg bypasses, shadowed assertions/macros, migration changes, harness changes and accounting manipulation. Snapshot `scripts/coverage/{historical,current-preservation,schema-adaptations}.ts` and `scripts/harness/managed-exec.test.ts` express this separation; their existence is not a test pass.

The operator's schema-2 clarification permits literal count/version/table/witness updates in `crates/arc-storage/src/migration/tests/acceptance.rs`, `crates/arc-storage/tests/nfr010.rs`, `crates/arc-daemon/src/db_worker/tests.rs`, `crates/arc-daemon/src/server_tests.rs` and `crates/arc-daemon/src/database_failure_tests.rs`. Keep schema-1 legacy-witness scenarios at schema 1. Refresh the feature guard's fixed literal adaptations, not historical baselines or rejection semantics. Migration 0002 is authorized; stale schema-1 production expectations are not protected behavior. Coordinate any additional adaptation path through the owning implementation task.

The implementation owner and independent QA retain these exact gates:

| Command | Required evidence |
| --- | --- |
| `cargo fmt --all -- --check` | Exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0, no warnings |
| `cargo test --workspace` | All deterministic unit, integration and CLI cases pass |
| `bun test scripts/harness/managed-exec.test.ts` | Historical/live separation, authorized-change positives and protection negatives pass |
| `bun test scripts/harness/coverage-verification.test.ts` | Complete coverage inventory and five serial noncollecting suites pass |
| `bun test scripts/coverage/clean-checkout.test.ts` | Isolated current-index checkout, no inherited native artifacts, real collection/live evidence pass |
| `bun test scripts/coverage/coverage-cli.test.ts` | Actual `bun run test:coverage` passes |
| `bun test scripts/coverage/harness.test.ts` | Actual full `./scripts/harness/validate.sh`, no skips, zero violations and every crate PASS |
| `bun test scripts/coverage/evidence.test.ts` | Live source/native/profile/accounting check after final collection passes |
| `bun test scripts/harness/managed-exec-evidence.test.ts` | Final source-bound command receipts, independent QA and evidence inventory pass |

Exact nine-suite `scripts/coverage` inventory: `accounting.test.ts`, `gate.test.ts`, `hygiene-preservation.test.ts`, `hygiene.test.ts`, `preservation.test.ts`, `clean-checkout.test.ts`, `coverage-cli.test.ts`, `harness.test.ts`, `evidence.test.ts`. The first five run serially inside `scripts/harness/coverage-verification.test.ts:6-27`; the last four are separately required. Preserve all original negative tests. For harness/protection changes also retain `bun test scripts/harness/equivalence.test.ts` and its stdout/stderr/exit/coverage-call/scaling assertions; it does not replace the real harness.

Retain the existing standalone `bun test scripts/harness/managed-exec-immutability.test.ts` regression for the unfiltered evidence suite: retained bytes, membership, index and review bindings must survive execution. It supplements the fixed operator gates and must not rewrite evidence or replace the required evidence command.

Serialize collectors writing latest.json. Use actual command receipts from passing wrappers instead of duplicate identical collections. Stage intended inputs before clean-checkout, which tests the index; its isolated pass does not replace the collecting checkout's final live evidence check. Finish implementation/verification inputs before collecting; input changes invalidate measurements. Every verification-contract command or replay must allow at least 3,600,000 ms, retaining longer inner allowances; 120,000 ms contract values are planning estimates, not timeout authority. Bounded Actor attempts must not abandon running collectors without truthful interruption records owned by the designated writer.

Each existing crate (arc-core, arc-detector, arc-storage, arc-ipc, arc-daemon, arc-cli) retains 80% production-line coverage independently, as does any added crate. Preserve exact integer accounting, production inventory/reachability/denominator checks, stale/missing/malformed failures, native LCOV/LLVM agreement, profile hashes, explicit child PID evidence and exact merge membership. No workspace average, production exclusion, coverage-only cfg, skipped test or copied pass pointer substitutes. Native reports/profiles remain local under existing hygiene rules.

## Acceptance checks, assumptions and task validation

| Layer | Required deterministic proof |
| --- | --- |
| Unit | Typed/range/enum/null errors; bounded argv and native-byte UNKNOWN; concurrent same-key/conflicting transitions; immutable completion; match/missing/reuse/boot/indeterminate identity; stale evidence rejection; migration constraints/rollback; zero permits |
| Daemon | Authenticated protocol-1 diagnostics/classification compatibility; finite admission/cancellation/shutdown; before/after commit reply loss for every mutation; restart without replay; status races; no argv/environment secrets in any ARC rows, DB/WAL/journal or diagnostics |
| CLI | Exactly one native child marker despite metadata retries; byte-exact argv/cwd/environment/PATH/streams; pipes/files/closed descriptors; immediate exit 17 with verified unreaped identity; real spawn failure; native PTY/pipeline signals/stop/resume/restoration; daemon SIGTERM/SIGKILL survival and restart; reservation-free remote/UNKNOWN |
| Independent QA | Reproduce focused native failures/recovery in final integrated sources; audit final source/mode/output/profile bindings and all gates without product edits or weaker assertions |

Use readiness/child/commit barriers and bounded deadlines, not sleeps as proof. Record real exits/output and owned-process cleanup. Synthetic secret sentinels must stay out of ARC persistence and diagnostics while child-owned streams remain unchanged. Native macOS establishes this slice; Linux cannot substitute. Outstanding work: desktop, native Windows and Phase 1 scheduling. No browser test, Playwright installation or screen-based review belongs to this task.

Implementation assumptions needing executable proof remain native fast-exit identity visibility, SDK/probe error mapping, full signal/mask/terminal behavior, retry/crash races and measured per-crate coverage. A truthful unverified-identity completion is a recovery result, not satisfaction of verified fast-exit acceptance. Source inspection alone settles none of these. The design deliberately leaves observer-death outcome uncertainty and scheduling out of scope rather than inventing recovery results. No measured cost or commercial benefit is claimed.

This contract is the sole produced artifact. Chief-owned delivery also requires `contract.json`, `preservation.md`, `qa.md`, `final.md`, `final-summary.json` and compact source/host/command receipts under `.arc-verify/managed-exec/`, with fewer than 150 tracked evidence files. Bind actual baseline, snapshot, integrated inputs/modes and outputs separately. Do not substitute one checkout's HEAD for another or treat a worker commit as containing uncommitted edits. Updating this report requires downstream artifact rebinding by the assigned owner; earlier receipts retain their original hashes and authority.

Checks for this documentation task: scoped entry/final status and diffs; separate baseline/snapshot commit/tree resolution; direct source/skill/prior-ADR reads; native host identification; SHA-256 binding of inspected snapshot paths; byte equality of 0001 against the accepted baseline; content/reference/ownership checks; and `git diff --check -- .arc-verify/managed-exec/technical-contract.md`. Missing snapshot/worker refs are recorded above. No subagents, build, product tests, coverage collection, harness run, dependency installation, GUI, external delivery or mission-state mutation occurred. Final artifact hash/size and check outcomes are supplied in the handoff.
