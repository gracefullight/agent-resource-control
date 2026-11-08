# Phase 0 managed exec execution decision

Status: task-4 specification complete, attempt 3. Final integrated acceptance requires refreshed bindings.
Owner: Chief Director. Skill: `oma-pm`, bound to `.agents/agents/pm-planner.md`.
Owned files: this decision and [contract.json](contract.json). The named assignment
outputs override generic skill plan/report destinations. Saved task instructions,
goalBrief, verificationContract, mission graph, Actor assignments and the empty
verifyCommand remain fixed.

## Integrated inputs

Accepted baseline and current HEAD: `19d7a507044cbf83918f58cd41056905c275c8d4`;
tree `d2f8ace96256a7e073ab9d1bfb10fdc5cae3bf38`. The worktree already contains
staged lifecycle implementation, tests, documentation and receipts, plus unstaged
preceding-report and harness-speed updates. They were inspected and preserved.
HEAD alone does not identify this overlay. `av.yaml` was excluded and not read.

Actual delivered reports were read and hashed in this worktree:

| Dependency | Bytes | SHA-256 |
| --- | ---: | --- |
| [technical-contract.md](technical-contract.md) | 38667 | `02191308c6341da453a31383cefaf0ab3b94a65c166d89e6b1a1e3b7bf936801` |
| [cli-contract.md](cli-contract.md) | 32582 | `ce9c5b1bd27e9bcc3127d4b4d5ce9070ac647e5aa21a373f24a158d9e4f555fd` |
| [value-scope.md](value-scope.md) | 14668 | `1d4511e447ac12ba1df484268b83e1b74c4ad2c682ba9f0179e0b8b76cc87166` |

The supplied framework reviews pass tasks 1–3. The reports distinguish their
Actor snapshots from the accepted baseline and record named-ref availability
limits. This decision consumes the integrated bytes above; it does not substitute
a snapshot HEAD for the accepted baseline or change report provenance.

Retained `final-summary.json` reports accepted native macOS lifecycle and
`qa.json` reports renewed independent PASS. Both 177-file content/mode sets match
before this edit. Earlier pipeline FAIL and workspace exit 101 are historical;
current located sources and later reports show the subsequent repair. This task
ran no runtime suite and does not independently recertify those retained results.
The two owned-file edits invalidate their retained input hashes, and the revised
three reports invalidate receipt-index artifact entries. Final Chief integration
must reconcile those bindings under saved ownership before reuse. No prior
receipt, QA verdict or final summary is rewritten here.

## Chosen implementation

Use one CLI-owned `std::process::Command` child with original `Vec<OsString>`.
The daemon owns short lifecycle transactions and identity reconciliation; it
never launches or waits for the child. Reuse the six Rust crates, maintained
classifier rules, authenticated protocol-1 IPC, finite SQLite worker, checksummed
migration registry and libc. No new crate, service, ORM or scheduler is planned.

This follows the Technical Director's reuse advice and the Design Director's
transparent-wrapper recommendation. Daemon-owned launch adds descriptor transfer,
waiters and crash fencing. An execution console or confirmation wizard adds
interaction and competes with child streams. Neither is selected. One backend
writer is appropriate because wire types, SQL invariants, worker plumbing,
identity, launch/retry handling and preservation tests must change together.

Marketing's assessments, outreach, referrals, publication and spending proposals
remain outside this assignment. No commercial experiment or usability study is a
completion gate. Customer demand, time savings, retention, revenue and ROI are
unmeasured. Every class executes without resource permits in Phase 0, including
REMOTE_CI, REMOTE_EXEC and UNKNOWN. Finite IPC/DB admission is retained separately
from resource scheduling.

## Launch protocol and exact interfaces

1. Keep native argv locally; classify through the bounded argv adapter using
   existing recognition rules. Never join/reparse argv or construct shell text.
   Non-UTF-8 or unsupported classification becomes UNKNOWN without changing bytes.
2. Capture native observer identity and allocate one OS-random 128-bit key,
   encoded as 32 lowercase hex. No command hash, PID/clock fallback or supplied
   prior-ID execution option is added.
3. Confirm durable REGISTER and same-owner CLAIM before entering the sole spawn
   site. Retry only the same metadata/key/value. An ambiguous acknowledgement
   without durable confirmation means this invocation starts no child.
4. Consume the local one-shot launch point and call
   `Command::new(&argv[0]).args(&argv[1..]).spawn()` once, outside IPC retries.
   A spawn failure consumes that attempt too. No restarted client may interpret
   a stored claim or status response as permission to launch.
5. Retain the unreaped handle, capture PID/start/boot identity and ATTACH.
   Recording loss cannot kill or relaunch work. Release connections, admission
   and worker tickets before native waiting.
6. Observe the actual wait result; COMPLETE the same tagged value on retries.
   Preserve exit 17, spawn failure or native signal status despite recording loss.
   Never invent an exit code from process disappearance.

`contract.json` retains exact embedded request/result schemas, required nullable
fields, enums, range checks and 12 transition rules. Additional lifecycle fields
are rejected; raw JSON is released before worker awaits.

| Operation | Required payload | Typed result |
| --- | --- | --- |
| job.start | action REGISTER, job_id, observer, classification | disposition CREATED/UNCHANGED and job snapshot |
| job.start | action CLAIM, job_id, observer | disposition APPLIED/UNCHANGED and snapshot |
| job.start | action ATTACH, job_id, observer, child | disposition APPLIED/UNCHANGED and snapshot |
| job.complete | job_id, observer, tagged outcome | disposition APPLIED/UNCHANGED and snapshot |
| job.status | query BY_ID, job_id | kind FOUND with snapshot, or NOT_FOUND |
| job.status | query RECENT | kind RECENT, up to 50 summaries, truncated, observed_at_ms, liveness_fresh:false |

Identity consists of positive PID, canonical positive u64 decimal start_token
and native boot UUID. Supplied tokens require native verification. EXITED has
code 0..255; SIGNALED has signal 1..127 and core_dumped; SPAWN_FAILED has static
reason and required nullable i32 os_code. Timestamps/revisions are bounded to
9,007,199,254,740,991 and revision starts at 1. Semantic consistency, u64 overflow,
native identity and races remain executable validation obligations.

Adopt task-1's existing retry policy in `exec/transport.rs`: at most three
identical exchanges within one monotonic two-second action deadline, each capped
at 600 ms and remaining time. No automatic interleaved status or sleeps are
required. REGISTER plus CLAIM is at most four seconds; ATTACH, COMPLETE and
optional status each have two seconds. A validated idempotent reply can resolve
a commit without a separate status request. Definitive errors stop the action;
ambiguous transport/DB_BUSY retries never enter spawn. These bookkeeping bounds
replace the previous task-4 12-second/backoff design detail, preserve the fixed
goal and tests, and impose no child runtime timeout.

## State, identity and uncertainty

REGISTERED preserves no-claim state; LAUNCHING means spawn may have happened.
RUNNING requires an attached natively matched child at its observation time.
Missing, mismatched or indeterminate evidence without an observed outcome means
OUTCOME_UNKNOWN. The original attached identity can later verify running, and
the original observer can refine unknown with its actual wait result.
COMPLETED, SIGNALED and SPAWN_FAILED keep immutable outcomes. Identical completion
preserves revision/time; conflicting owner/value is rejected. Child binding is
one-time; repeated existing attachment may remain idempotent after completion,
but a first attachment to terminal state is invalid. Spawn failure requires a
claim and no attached child. Completion without identity attachment can preserve
a real result but cannot fabricate verification.

Reuse `arc-core::process_identity`: native macOS start microseconds and
kern.bootsessionuuid, existing Linux ticks/boot UUID, conservative lock behavior,
and distinct match/missing/mismatch/indeterminate probes. Capture fast-exit
identity before reap and distinguish zombie identity from live status. No sleep,
fabricated token or second launch substitutes for this proof.

BY_ID reconciliation uses at most three read/probe/conditional-update attempts
under the request deadline. Release the worker ticket before probing; apply only
if revision, original identity and nonterminal state still match. A terminal
result wins; exhausted stale retries return DB_BUSY. RECENT selects 51 indexed
rows, returns 50 and truncation, and performs no per-row native probes. No startup
history scan, background task per row or replay is introduced.

Possible COMMIT followed by cancellation, timeout or lost reply is ambiguous.
Resolve using the same key/operation or optional status; never assume rollback
or replay work. CLI death in the claim/spawn window or loss of an unrecorded wait
result can leave unknown outcome. The guarantee covers one conforming invocation
through tested retries, not eventual launch or separate operator invocations.

## CLI, terminal and storage

Keep `arcctl exec -- <PROGRAM> [ARG...]` and optional read-only
`arcctl job status [<JOB_ID>]`. Preserve task-2 assertions E01–E17 and semantic
help: direct argv examples, caller-shell quoting, literal operators, daemon
prerequisite, survival through daemon loss, ID discovery and scheduling scope.
Normal execution adds no ARC banners, job IDs, prompts or progress output.
Keep original cwd, environment/PATH, inherited streams, closed descriptors,
CLOEXEC metadata and exact child outcomes. Secrets may appear in child-owned
output; ARC must not copy them into its records or diagnostics.

RECENT prints opaque IDs, registered/updated Unix milliseconds, state, static
reason, Last known: true, truncation and empty-history text. Current `job.rs`
contains these fields. BY_ID shows observation/identity, state/reason, timestamps
and actual code or signal/core flag. Status never launches; query success exits
0, missing/unavailable exits 1 and syntax errors exit 2. Exec parse errors exit
2; confirmed prelaunch/spawn failure exits 1. Uncertain tracking uses static
stderr plus validated ID/status guidance; never advise execution retry. Preserve
legacy status/doctor/classify output grammar and protocol 1; doctor may report
schema 2 truthfully. Native signal status retains shell convention 128+signal
and the parent's actual signaled wait result.

Follow operator guidance at 2026-11-08T15:24:49.402Z. With a controlling terminal,
keep wrapper and child in the caller's job with pipeline peers, including a
background job. Do not create a process group or call tcsetpgrp in that path.
The OS delivers terminal INT/QUIT/TSTP to eligible job members; ARC does not
forward those terminal signals again. Forward directly received TERM/HUP/USR1/
USR2 only to the owned child PID. Wait with WUNTRACED|WCONTINUED, mirror a still
outstanding child stop through wrapper SIGSTOP and continue on SIGCONT without
taking foreground. Only nonterminal/detached execution without caller job peers
uses a private child group. Preserve inherited ignored dispositions/masks,
preattach signals, stop/continue ordering and terminal restoration. Clear child
ownership before late delivery can target a recycled PID. Sender/code metadata
cannot establish PID-versus-group targeting; no inference or group broadcast
is selected.

Located `exec.rs`, `terminal.rs` and `signals.rs` now follow this structure. The
old unconditional-private-group defect is historical, not a new reproduction.
Require direct/wrapped native PTY and pipeline proofs for peer input, INT/QUIT
once, wrapper-only TERM, whole-job Ctrl-Z, background continuation/read stops,
foreground resume, terminal modes, ignored/blocked CHLD, cleanup and exit 17.
Owned fixture-daemon SIGTERM/SIGKILL must leave the same child and usable I/O,
with restart reconciliation and one completion. This proves no terminal-closure,
logout, arbitrary CLI-death or reboot survival promise.

Keep `0001_phase0.sql` byte-for-byte, SHA-256
`046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`.
Append checksummed `0002_managed_exec.sql` through the registry. Update jobs and
job_lifecycle atomically in parameterized immediate transactions, with complete
or absent identity tuples and exclusive terminal constraints. Preserve old
rows, legacy witnesses, rollback and newer-schema rejection. Unestablished
project/session/signature/reservation fields stay null; create no permits.
Persist no argv, environment, cwd/path, executable name, child output, reversible
encoding or command-derived hashes. Sentinel checks cover rows and raw DB/WAL/
journal where present; diagnostics use static codes, never SQL/OS error text.

## Ownership and checkpoints

Backend is the sole writer of production, tests, manifests, Cargo.lock,
migration registration, coverage tooling and harness tests throughout saved
tasks 5–7. Other Actors own their named reports; the reviewer never edits product
or weakens tests. Any handoff needs an actual artifact dependency and Chief
coordination. No graph, Actor, deadline or task instruction is changed here.

| Boundary | Final backend-owned modules/tests |
| --- | --- |
| Identity/classifier | arc-core src/process_identity.rs and tests/identity.rs; detector src/argv.rs and tests/argv.rs |
| IPC | arc-ipc src/jobs.rs, jobs/types.rs, jobs/validation.rs; tests/jobs.rs |
| Storage | 0002_managed_exec.sql, migration registry, src/jobs.rs and jobs/{codec,transitions,reconcile}.rs; jobs tests and migration tests |
| Daemon | Existing connection/db_worker/execution/lifecycle adapters; db_worker/jobs.rs; tests/managed_exec.rs and managed_exec/{protocol,retries,cli,scenarios,terminal,pipeline}.rs plus pipeline.py |
| CLI | Existing main routing; src/exec.rs, exec/{transport,descriptors,signals,terminal}.rs, src/job.rs; native child/pipeline fixtures |
| Protection | Existing scripts/coverage and scripts/harness modules, managed-exec and evidence/immutability tests |

JSON lists exact paths and a 27-row requirement-to-test matrix covering every
immutable obligation, with available named cases from retained QA. Those names
are evidence pointers, not new passing execution claims. Keep source files at
500 lines or fewer. No new crate is planned; any addition enters the inventory
and retains the same individual 80% production-line floor.

| Actor | Installed skill/profile | Material deliverable |
| --- | --- | --- |
| Chief Director | oma-pm / pm-planner | decision.md and contract.json; final synthesis in its saved task |
| Technical Director | oma-architecture / architecture-reviewer | technical-contract.md, delivered |
| Design Director | oma-pm / pm-planner | cli-contract.md, delivered |
| Marketing Director | oma-docs / docs-curator | value-scope.md, delivered |
| Backend | oma-backend / backend-engineer | Implementation, tests, preservation/storage reports and compact receipts, present |
| Reviewer | oma-qa / qa-reviewer | qa.md, qa.json and independent reproductions, present as retained evidence |

Bindings were checked against `.agents/agents/` profile frontmatter and installed
`.agents/skills/` entries. Tasks 1–3 can run concurrently in isolated worktrees
and integrate in task order; their actual required artifacts are now present.
This task dispatches no Actor. Framework task reviews and final verification
remain required; advisory reviews do not replace them or independent oma-qa.

## Preservation and verification

Historical whole-source assertions stay at original contract fixtures
`fbe76a5f15101ad415f355ef06f1deeb1f97d568` and
`aacb3b4359f13c31c0705c0cffc5bf1ac00f387e`, retaining rejection tests. Live guards
protect 0001, regression assertions/helpers/reachability, harness path/type/mode/
behavior and coverage accounting. Authorized feature changes must pass positive
fixtures; removed/weakened assertions, early returns, ignore/cfg/helper/macro
bypasses, migration edits, harness changes and accounting manipulation must fail.
No moving historical baseline, blanket exemption or coverage exclusion is allowed.

The 2026-11-08 schema-2 note authorizes literal count/version/table/witness updates
in storage migration acceptance/nfr010 and daemon worker/server/database-failure
tests. Keep legacy schema-1 witness semantics and fixed historical revisions;
backend owns adaptation expectations and current affected checks. The collector
note declares run 14213b3c-fbfb-48c3-867d-4602607bdd05 abandoned: its owner records
commands:[], pass:false and interruptedAt, starts no wait for a result, and uses
fresh bounded collection. Task-4 neither edits run records nor asserts that this
action is complete. All three operator notes are captured in JSON; changed
prerequisites require affected checks in the current environment.

Required operator commands remain exact, project-relative cwd `.`:

- `cargo test --workspace`
- `bun test scripts/harness/managed-exec.test.ts`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `bun test scripts/harness/coverage-verification.test.ts`
- `bun test scripts/coverage/clean-checkout.test.ts`
- `bun test scripts/coverage/coverage-cli.test.ts`
- `bun test scripts/coverage/harness.test.ts`
- `bun test scripts/coverage/evidence.test.ts`
- `bun test scripts/harness/managed-exec-evidence.test.ts`

Also retain `bun test scripts/harness/equivalence.test.ts` and existing standalone
`bun test scripts/harness/managed-exec-immutability.test.ts` for relevant protection
work. Execute actual `bun run test:coverage` and full
`./scripts/harness/validate.sh` without skips. Required wrappers may provide
receipts for their actual underlying executions; they do not waive any wrapper.
All nine coverage suites remain inventoried: five noncollecting suites plus
clean-checkout, coverage-cli, harness and evidence. Serialize collectors, stage
intended inputs under ownership before current-index clean-checkout, and perform
live evidence validation after the last collection. Do not duplicate unchanged
collections or reuse stale pass pointers.

arc-core, arc-detector, arc-storage, arc-ipc, arc-daemon and arc-cli each retain
80% production-line coverage. Preserve denominator/reachability, native LCOV/LLVM
agreement, source/profile hashes, exact merge lists and child PID evidence.
Keep protocol 1, diagnostics, finite admission/queue, cancellation/interrupt/join/
gauges and the shared shutdown deadline capped at 3,500 ms. Long-lived children
hold no request or DB tickets. Independent oma-qa reproduces focused native
failures/recovery and audits final-source receipts without product edits.

Actual per-check allowance is 3,600,000 ms; preserved verificationContract
120,000 ms values are planning estimates. Wrappers/replays allow at least the
actual allowance, preserving longer inner allowances. Mission deadline and
cancellation still govern admission. Do not alter or replace empty verifyCommand.
Keep fewer than 150 tracked evidence files under managed-exec; the current index
contains 127. Native raw reports remain local. Only actual final-source evidence
can establish nativeMacos, independentReviewPassed and sourceBound flags. Native
macOS lifecycle acceptance completes this slice; desktop, native Windows and
Phase 1 scheduling remain outstanding.

## Current prerequisites, limits and validation

Fresh host/tool probes at `2026-11-08T18:18:55Z` report Darwin arm64, macOS 27.2
build 26B123, eight CPUs, 17,179,869,184 bytes RAM and approximately 18 GiB free.
File-descriptor soft limit is 1,048,576. Installed: Rust/Cargo 1.95.0, rustfmt
1.9.0-stable, Clippy 0.1.95, cargo-llvm-cov 0.6.21, native LLVM-tools component,
Bun 1.4.4, Python 3.14.10 and Xcode clang path. Probes establish availability,
not linker/process/PTY correctness or product acceptance. No install or build ran.

Saved execution limits remain maximum three concurrent tasks, 86,400 seconds
across the mission, 200 runs, three retries, 1,000 ms retry delay and auto-resume.
The supplied operations snapshot records 100 measured runs; 100 is a snapshot
remaining-run ceiling, not fresh accounting. Authoritative mission start/deadline,
cancellation and exact remaining seconds are not injected. JSON records deadline
and remaining time as null; the supervising framework retains actual admission
and cumulative accounting. The previous attempt's activity timestamp is historical,
not a new start or permission to reset the timer. No downstream process is started.

Installed `.agents/oma-config.cue` uses model_preset auto; its only explicit
Actor override is unrelated eval. YAML/local overlays are absent. Retain supplied
codex/native-default route. Costs and model capability comparisons are unverified.
Serena methods are absent from the available tool inventory. Known paths were
read directly; no recursive code search, provider install, GUI or browser was
used. The external memory-registry lookup had no relevant ARC hit.

Document validation checks three delivered hashes; six installed skill/profile
bindings; all 27 matrix rows, 10 exact gates and schema references; listed module/
test paths; unchanged copied goalBrief/verificationContract, wire interfaces,
transitions and migration 0001; and nonowned content/modes plus index preservation.
The embedded schema is unchanged; no full JSON Schema validator is installed or
claimed. `git diff --check` and explicit newline/trailing-space checks cover the
two owned artifacts. Commands/outcomes and counts are in contract.json.

Acceptance checklist for this task:

- [x] Concrete interfaces, state transitions, uncertainty and full test matrix.
- [x] Director tradeoffs, POSIX guidance, ownership and checkpoints resolved.
- [x] Every roster Actor has a material deliverable and installed skill binding.
- [x] Protocol, diagnostic, bounded-runtime, coverage and native macOS gates retained.

Only decision.md and contract.json change. No product test, collector, harness,
GUI, screen check, Playwright, external delivery, new plan or mission-state
mutation ran. One named YAML read failed because it is absent; the CUE config
was read. Remaining work is owner-controlled artifact/input-binding reconciliation,
affected verification and final integrated acceptance. Existing earlier failure
receipts and later passing claims retain their recorded scopes.
