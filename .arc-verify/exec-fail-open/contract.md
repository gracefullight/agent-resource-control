# Exec fail-open execution and preservation contract

Status: task-1 contract revision 3; receipt cycle repaired; implementation and acceptance remain unverified.
Owner: Technical Director. Skill used: `oma-architecture` (Recommendation Mode).
This task owns only this file and `contract.json`. They are the material report
and machine-readable handoff; the assignment overrides generic skill report paths.

## Inspected source and limits

Accepted committed baseline: `00773c69a8e5e7fd417fd5cc78d03e222364f609`,
tree `b98f996caa04fe13eeeeab1072c3c528f6d18c65`.
The first attempt inspected Actor snapshot `9bae9c95b10d9b99848afca3864de9c4e53c65ae`
with the same tree; its recorded observations remain attributed to that snapshot.
This repair attempt directly resolved HEAD to the accepted baseline above and
rechecked all 23 recorded file hashes against both worktree and accepted Git
content. All match. Initial accessible status contained only the existing
untracked `.arc-verify/exec-fail-open/` directory. `av.yaml` was excluded.
Prior supplied snapshot/worker ref limitations remain historical provenance;
this attempt does not substitute HEAD for an unverified ref. New contract files
are task output until integrated, not baseline content.

The configured provider is Serena (`.agents/oma-config.cue`). Discovery found no
Serena file, pattern or symbol tools. As permitted by the code-intelligence
contract, inspection used scoped reads of already located files and Git identity
checks, without substitute recursive code search. `docs/decisions/index.md` is
absent. Historical `.arc-verify/managed-exec/decision.md` and preservation docs
were read as evidence. Their fail-closed semantics remain historical.

`contract.json.sourceObservations` records exact inspected file hashes. Principal
observations at the Actor HEAD, also byte-equal to accepted source:

- `arc-cli/src/exec.rs` has one spawn, but REGISTER and CLAIM errors return
  NOT_STARTED. Native argv, signal ownership and descriptors already have owners.
- `exec/transport.rs` erases errors into `Result<Value, ()>`, renews a two-second
  deadline per call, and permits three 600 ms attempts per operation.
- `exec/signals.rs` uses `quiet` for terminal INT/QUIT in shared jobs. It does
  not set the pre-spawn pending bit there; cancellation coverage must address
  this interval while retaining single OS delivery after child attachment.
- `arc-daemon/src/admission.rs` authenticates same-user peers before creating
  handlers. `platform.rs` uses native peer credentials; `server.rs` enforces a
  private owned directory/socket. This is the existing trust boundary, not proof
  that the CLI currently performs mutual server authentication.
- `connection.rs` returns DB_BUSY on the database response deadline;
  `db_worker/jobs.rs` also maps admission/worker failures to DB_BUSY.
  `arc-storage/src/jobs/transitions.rs` warns explicitly that failure can follow
  COMMIT and makes repeats of the same key/operation idempotent.
- The live preservation guard still uses feature baseline `19d7a507...`, which
  predates these managed-exec test additions. A separate `00773c69...` amendment
  must protect those additions; advancing the older baseline would hide changes.

First-attempt probes recorded Bun 1.4.4, Cargo/Rust 1.95.0, cargo-llvm-cov 0.6.21,
LLVM 22.1.2 and the installed Rust LLVM component on Darwin 27.2.0 arm64,
Rust host `aarch64-apple-darwin`. This repair re-ran Bun/Cargo/Rust version probes:
all exit 0 at those same versions. Other host/tool facts remain first-attempt
observations. No build, runtime test, coverage collector, lint, harness or native
timing measurement was run for this contract.
Offline dependency availability and historical fixture preparation are unverified.

## Decision and cost

The problem is launch ownership: optional metadata currently vetoes native work.
Select a small typed preparation result local to `arc-cli::exec`, with a typed
transport error and one absolute deadline passed into the existing transport.
Keep the sole CLI-owned `Command::spawn`, signals, terminal and descriptor paths.
Keep protocol 1, the classifier, lifecycle types, idempotent transactions and the
bounded daemon database worker. No schema or wire-format change is needed.

| Option | Implementation and maintenance | Runtime/operational cost | Decision |
| --- | --- | --- | --- |
| Local preparation outcome plus existing spawn | Changes two local boundaries; existing native tests stay useful; small enum is reviewable | Bounded metadata attempts; no new worker or service | Selected |
| General execution abstraction or daemon launch owner | New state machine, descriptor/identity transfer and signal interfaces; more migration and review work | More ownership, recovery and synchronization work | No concrete source need; rejected |

No new dependency, async CLI runtime, service, shell reconstruction, detached
retry loop, resource reservation, ARC cleanup, scheduler or daemon auto-start.
No stored claim, recovery status or receipt authorizes another launch. Exactly
once here means one spawn attempt in this invocation, including spawn failure;
it does not deduplicate separate operator invocations.

## Local decision model

Conceptual interface: `prepare(context, absolute_deadline, cancelled)` returns
`Managed`, `Unavailable(reason)`, `Uncertain(reason, optional_job_id)`,
`Rejected(code)`, `InvalidReply(reason)`, `InvalidArguments`, `Cancelled`, or
`LocalFailure(reason)`. Names may follow existing Rust style; distinctions are
mandatory. Transport errors preserve outage, DB_BUSY, valid explicit rejection,
invalid response and cancellation separately. Do not use `Result<_, ()>` to
erase these pre-spawn decisions. No new published API is implied.

| Result | Evidence/meaning | Launch and reporting |
| --- | --- | --- |
| Managed | Valid REGISTER and CLAIM replies match key, observer, class, phase and lifecycle validity; same-invocation UNCHANGED is valid | Enter sole spawn; retain existing ATTACH/COMPLETE tracking |
| Unavailable | No lifecycle request may have reached the daemon, e.g. absent/refused socket on every attempt | Enter sole spawn; after success print static TRACKING_UNAVAILABLE guidance |
| Uncertain | Request bytes may have been sent; REGISTER may have committed; REGISTER confirmed but CLAIM unavailable; or DB_BUSY exhausted | Enter sole spawn; after success print TRACKING_UNCERTAIN, never claim saved persistence |
| Rejected | Well-formed matching protocol-1 error response, except retryable DB_BUSY | No spawn; static rejection diagnostic, exit failure; never print raw message/payload |
| InvalidReply | Wrong version/ID, contradictory status/error/result, missing or malformed result, wrong key/owner/class/phase, empty/oversized/malformed JSON frame | No spawn; static invalid/protocol diagnostic; never silently relabel an invalid reply as outage |
| InvalidArguments | Missing/empty program, NUL argv, invalid exec grammar | No metadata launch, no spawn; preserve INVALID_REQUEST and exit 2 |
| Cancelled | Observed pre-spawn cancellation, including while waiting or just before spawn | No spawn, no fallback-start wording; preserve existing signal conventions |
| LocalFailure | Cannot establish required local signal supervision; invalid local metadata/configuration distinct from transport loss | No spawn; static diagnostic; no invented tracking |
| SpawnFailed | Sole spawn attempt failed after an eligible decision | No second attempt; preserve static SPAWN_FAILED and exit 1; no 'Command started' wording |

Connection failures, read/write timeouts, reset, EOF or truncated frame on a
disconnected stream are transport outages. Mark uncertainty conservatively as
soon as a write may have delivered any lifecycle request bytes; a failed retry
cannot erase it. A complete malformed reply is not the same as disconnection.
A later valid idempotent success can resolve uncertainty. A valid definitive
rejection ends preparation even after a preceding ambiguous attempt. Peer/path
authentication or local invalid-input failures must remain distinguishable from
ordinary socket absence; do not weaken the existing IPC boundary.

DB_BUSY is an authenticated response with existing overload/deadline semantics,
not a definitive refusal of this invocation and not a transport exception.
Retry the same operation, request ID, job ID, observer and value inside the same
budget. On exhaustion return Uncertain with reason DB_BUSY: rollback is unknown.
All other well-formed error codes, including unknown codes, remain Rejected.
Protocol incompatibility retains a static protocol-1 diagnostic. Do not display
arbitrary server error text. Do not send CLAIM after an unconfirmed REGISTER.

Fallback may use an opaque locally generated ID for read-only recovery guidance,
but must state that the reference does not confirm a saved record. An absent
record does not prove nonexecution. Keep normal child stdout untouched and add
only static ARC guidance to stderr. Child output itself may contain secrets;
ARC diagnostics, storage and receipts must not copy argv/environment values.
For fallback, finish preparation and run/wait locally without a post-spawn
REGISTER/CLAIM recovery loop. Do not ATTACH/COMPLETE against an unconfirmed claim.
Managed post-spawn ATTACH/COMPLETE retain bounded best-effort behavior and actual
outcome propagation; their failure cannot relaunch or kill the owned child.

## One deadline and timing proof

Create `t_meta` before socket resolution, identity/key acquisition, classification
and the first metadata request. Set one monotonic deadline `t_meta + 1800 ms`.
Pass it unchanged through REGISTER, CLAIM, reconnects, retries, polling, frame
prefix/body writes and reads. No stage or partial frame renews it. At most three
exchanges per operation, each ending at `min(shared_deadline, now + 600 ms)`;
local serialization/validation is charged to the same budget. Faster connection
failure may finish earlier; do not delay fallback to fill the budget.

Reserve 200 ms for local decision-to-spawn and native handoff; this is an
engineering budget, not a measured guarantee. Expiry yields Unavailable or
Uncertain according to send history, rather than granting a fresh CLAIM budget.
Before each attempt, before/after each bounded wait/read/write and immediately
before the sole spawn, check cancellation. Use a bounded poll quantum at most
25 ms (or the remaining time if smaller) to observe cancellation under restartable
signals. Existing pending-signal handoff remains responsible for a signal racing
the final check; do not add a second child owner or a process-group broadcast.
Metadata expiry does not impose a child runtime timeout.

Required measurements use one native monotonic clock domain or record an
explicit valid conversion. Record `t_meta`, preparation decision and spawn-entry
where directly observable. In native CLI tests, also record parent time just
before wrapper spawn and the child invocation marker/control handshake. That
larger interval is an upper bound on metadata-to-invocation and must be <=2000 ms
on the controlled host. Proxy first-request time alone cannot stand in for
`t_meta`; wall-clock timestamps alone cannot establish durations. Report wrapper
startup, handoff, observer/IPC overhead and total separately when observable;
otherwise label subdivisions unavailable, not zero. Do not subtract scheduling
delay to turn a >2000 ms observation into a pass. Record overshoots as failures
and investigate. Arbitrary host suspension is not a hard real-time guarantee.

Use deterministic clock/deadline tests for budget arithmetic, no renewal after
REGISTER, partial frames, retry exhaustion and cancellation. Native proxies use
barriers and actual commits; sleeps alone cannot prove a commit or invocation.

## Historical assertions and fixture adapter

Protect accepted baseline `00773c69...` independently of older preservation
baselines. Preserve the original assertion bodies, test attributes/names and
helpers. Only two command-construction expressions may gain adapters:

1. `retries::cli_has_one_child_under_before_after_commit_reply_loss_and_retains_exit17`:
   route only the `mode == "before-register"` command to the fixed historical
   arcctl. Its exit-1, missing-marker and NOT_STARTED assertions remain verbatim.
   `around-commit` and `completion` continue using live arcctl, with the original
   retry-count, one-child, exit-17, streams and persistence assertions intact.
2. `cli::spawn_failure_quick_exit_help_and_status_have_static_diagnostics`:
   route only the `/usr/bin/true` exec command after `daemon.stop` and `f.report`
   to historical arcctl. Preserve its exact NOT_STARTED bytes. Earlier spawn,
   quick-exit, syntax, help and status checks and the following unavailable-status
   request remain live with original assertions.

Task-3 adds `crates/arc-daemon/tests/managed_exec/historical.rs` and declares it
in `managed_exec.rs`. Adapter API: `before_register(command, mode) -> Command`
returns the original command unless mode is exactly `before-register`;
`after_shutdown(command) -> Command` is called only at the single named site.
For a routed command, replace only its program with the verified historical
binary before stdio customization, copying args, env removals/values and cwd via
native Command getters. Preserve current shared helpers and their protections.
No global binary override, PATH substitution, environment-controlled baseline,
test-name/thread-name detection, ignore, cfg suppression, early return, assertion
macro replacement or change to the retry Proxy is allowed.

A new `scripts/harness/exec-fail-open-history.ts` helper materializes the exact
accepted Git tree into an isolated ignored `target/exec-fail-open/history/` root,
including required root manifests/lock/tool configuration and crate files. Reuse
the maintained Git/checkout helpers where applicable; `historical.ts` alone
checks out only crates and the harness, so it is not a complete Cargo fixture.
Fail if the accepted object/tree is unavailable or materialized bytes differ.
Prepare the baseline arcctl by actually running `cargo test -p arc-cli` in that
fixture with `--locked --offline --target-dir <isolated-target>`; record the
resolved actual argv and cwd. Do not use
`--no-run` as execution evidence or require a standalone build command. Strip
inherited instrumentation/target overrides from this historical preparation;
it is historical execution and cannot contribute candidate coverage. Do not
modify the current collector or its child/PID accounting. The ordinary current
`cargo test --workspace` must prepare/verify this fixture on first use if absent,
using the helper noninteractively; no optional environment switch may skip it.
Cache once per baseline tree/toolchain; verify source and binary hashes before
reuse. Fixtures never write product sources or mutate the mission worktree/index.

Record actual historical invocation receipts: accepted tree and file inventory,
preparation command/test counts, binary SHA-256, adapter site/case, actual program,
argv shape (synthetic fixture only) and the current command's executed test
identity/output hashes. Record a per-child PID or output hash only when actually
observed; otherwise use null with a reason. The unchanged positive test and
source-bound assertions establish the historical outcome: zero marker/exit 1 for
before-register, and exact NOT_STARTED stderr after shutdown. The latter uses
`/usr/bin/true` and has no marker assertion; do not invent one.
Bind them to the current test command receipt and source fingerprint. Historical
passes prove the retained old behavior only; separate live cases must prove each
replacement behavior. Missing fixture/binary/invocation proof is a blocker, not a
skip or a reason to route more tests historically.

The new preservation validator compares all accepted existing Rust test/helper
items and reachability at `00773c69...`. For the two files above it permits only
the two fixed adapter recipes, reversing them to the accepted token sequence
before comparison; ambiguous/multiple matches fail. New standalone tests and the
one historical module declaration are allowed and must be executed. Preserve
the original files' imports/helpers, all assertions and attributes. Protect the
new adapter/helper against widened routing with exact permitted site checks and
tamper fixtures. Do not rewrite old guards or choose a later preservation head.

Reject mutations of assertions, constants, attributes, early-return/cfg/ignore
paths, helper inputs, module/target reachability, macro shadowing, baseline/tree,
fixture program/hash/source, mode predicate, additional adapter callers, omitted
historical invocation evidence and live cases pointed at the old binary. The
live counterpart assertions must fail against historical arcctl; record that
negative control as an expected fixture failure, never as a passing live run.

## Live test inventory

Each live test reports its identity, actual candidate binary hash, scenario,
invocation count and outcome. Reuse current native child and proxy/barrier
fixtures. New modules stay focused; every source file stays within 500 lines.

| Group | Required scenarios and observations |
| --- | --- |
| Preparation unit | Managed success/idempotent retry; shared deadline/partial frames/reconnect; unavailable vs uncertain; DB_BUSY retry/exhaustion; explicit errors and invalid replies; cancellation |
| Outage integration/CLI | Absent socket; stale/refused/unreachable endpoint; accepted-but-stalled REGISTER and CLAIM; disconnect before send and during REGISTER/CLAIM; every REGISTER reply lost after real commit; every CLAIM reply lost after real commit; REGISTER consumes most budget then CLAIM stalls |
| Native compatibility | For every outage class compare direct child and candidate: one marker, byte-exact argv (empty/Unicode/non-UTF-8/metacharacters), cwd, environment/PATH, inherited/open/closed descriptors, stdin including NUL, stdout, child stderr and exit 17; ARC stderr checked separately |
| Non-launch | Invalid grammar/program/NUL, pre-spawn cancellation while stalled REGISTER and CLAIM and before spawn, authenticated definitive errors at both stages, malformed replies, invalid local configuration; zero marker and no started wording |
| Native signals/jobs | Fallback TERM/HUP and actual signaled wait status; terminal INT/QUIT exactly once; inherited ignored dispositions and mask; stop/continue including stopped target killed; foreground pipeline peers, foreground ownership, no duplicate forwarding or orphan |
| Managed regressions | Original successful tracking, idempotency/claim ownership, commit retry counts, completion uncertainty, daemon shutdown/crash survival and reconciliation, identity rejection, protocol-1 diagnostics, schema protection all remain live |
| Recovery/privacy/scope | Recover real daemon after each outage, observe barriers/status before and after; marker remains one; no execution on recovery/status, no permits/reservations, cleanup or persisted command/environment sentinels; uncertain IDs never claim saved history |

## Ownership required for task-3

Backend remains the sole product/test/validator writer under its saved assignment.
This contract does not change assignments. Required edit surfaces are enumerated
in `contract.json.implementationSurfaces`: exec/transport and local new modules,
main help, the two adapter sites/module declaration, new live native test modules,
new harness validators/helpers and `docs/managed-exec.md`. Existing signal,
terminal, descriptor, IPC/authentication/storage and fixture modules are reused;
changes there need demonstrated necessity and saved-scope ownership confirmation
by the Chief. The observed pre-spawn terminal-cancellation gap is a concrete
reason for a narrow change in `exec/signals.rs`; keep its native ownership model
and existing assertions. This task writes none of them. Shared manifests, lockfiles, existing
guards, collector and historical evidence need no edits. Report any actual
interface conflict with exact path rather than editing another Actor's files.
Claims review and independent QA/final artifacts remain with their saved owners.

## Receipt and validator interfaces

`contract.json.receiptInterfaces` defines required types/fields. All evidence is
under `.arc-verify/exec-fail-open/`; raw logs/profiles stay local in ignored target
or existing coverage run directories. Fewer than 150 tracked evidence files;
required final evidence must match index bytes/mode, be regular files and have
safe canonical relative paths. Do not stage another Actor's unfinished changes.

Use maintained SHA-256, inventory, command-output parser and accounting helpers.
New validators are exactly `scripts/harness/exec-fail-open-gates.test.ts`,
`exec-fail-open-preservation.test.ts`, `exec-fail-open-docs.test.ts` and
`exec-fail-open-evidence.test.ts`. The JSON lists each validator's required checks
and tamper cases. Existing historical managed-exec receipts/validators retain
their baselines and meanings; do not relabel their old pass as current fallback.

Compute a sorted file-content/mode input fingerprint over crates, coverage and
harness scripts, root build/tool/package configuration, accounting contract,
these two contracts and relevant current docs/claims review. Include new
untracked implementation inputs, not only `git ls-files`. Exclude generated
receipts/reviews/final verdicts to avoid self-reference. Preserve the separate
existing coverage `inputIdentity` meaning. Record before/after equality for each
command and bind coverage to its own current fingerprint as well. Record actual
HEAD, baseline tree and binary hashes; HEAD alone cannot bind an uncommitted
implementation. QA must reproduce against the integrated final fingerprint.

Each compact command receipt records actual argv/cwd, actor/skill, source hash,
host and tool versions, UTC start/end, monotonic elapsed ms, PID, exit, output
digests, timeout allowance >=3,600,000 ms and `skipFlags:false`. Keep raw output
local with hashed path references; parse real test identities/counts. No zero
test, ignored/skipped, stubbed Cargo, failed prerequisite or old-source receipt
passes. Do not store raw environment or real command secrets.
Retain failed-attempt references and their resolution in the gate receipt rather
than replacing them with a later pass without explanation.

The receipt architecture separates implementation readiness from terminal
validation. The former design required gates to contain a receipt for their own
validator and for the evidence validator, which requires final evidence derived
from gates. That cycle prevented a first successful pass. Removing commands or
creating provisional successful receipts would lose required evidence. Select
an acyclic sequence with a terminal sink: it keeps every command and adds only
one compact receipt file, using the existing receipt helper interface.

`commandPhases` partitions the unchanged 13 `mandatoryCommands` arrays exactly:
the first 11 are prerequisite checks; the last two are the gates and evidence
validators. `implementation/gates.json.commands` covers exactly those 11
prerequisites, including preservation/docs validators, measured timing and
coverage checks. It never requires either terminal validator receipt, its own
hash, QA, final or terminal evidence. `passed:true` means implementation checks
passed; complete acceptance still requires both terminal commands.

`qa/review.json` binds independently authored `qa/review.md`, current inputs,
gates and predecessor receipt hashes, actual reproduction commands and findings.
Require unfiltered independent `cargo test --workspace`, with zero failed/ignored
tests. QA can inspect/reproduce gates validation, but cannot require final,
full evidence validation or terminal artifacts before writing its review.

`final.json` binds the actual gates, timings, independent review/report and claims
review by byte hash. Its required fields remain `verdict` and `openBlockers`.
A pass requires zero blockers, passed gates/review, measured timing and validated
current-source evidence. It never requires or references terminal validator
receipts or the terminal sink. A final pass alone is insufficient for mission
acceptance: both real terminal validator processes must also succeed.

`terminal.json` is a downstream sink containing inline real command receipts
for exactly the two terminal validators, their actual exits/test counts/times,
current source/host/tool identity and byte-hash references to immutable gates,
review and final. No predecessor or validator requires its presence, verdict or
hash; it does not hash itself. The recorder writes it from completed process
observations, without a later validating command as a creation prerequisite.
The Chief checks it alongside actual command outcomes. An invented pass field
cannot replace actual execution; a failed terminal command blocks acceptance
even when gates or final still record their earlier successful stage.

Required sealed evidence is staged before terminal validation. `final.evidenceFiles`
counts the sealed set excluding only the exact `terminal.json` path and must be
1..148. Reserve one slot for the sink. The evidence validator checks sealed
count/index bytes and all current tracked evidence for regular type, matching
index bytes and total <150. On the first run the sink is absent; on a replay it
may be present and counts toward the total, without becoming a required input.
Write/stage the sink only after both terminal processes finish, leaving sealed
bytes unchanged. Terminal creation therefore changes neither final nor its hash.
This exact exclusion is for a downstream result, never an exemption for source,
claims, measurements, QA or any other required evidence.

Task-3 must add a deterministic first-run case to the evidence validator. In an
isolated scratch root, start with no generated receipts or gates/QA/final/sink.
Exercise the same assembler and validation functions with an explicitly
fixture-only recipe catalog: run real small subprocess checks and capture actual
outcomes, create prerequisite observations, assemble/validate gates while all
successors are absent, perform separately attributed fixture reproduction, write
review then final, validate evidence without its own receipt, and finally record
both actual terminal results. Assert no successor read, no placeholder success,
unchanged predecessor hashes, exact command partition and topological ordering.
Reject a back-edge, a tampered predecessor and a terminal failure relabeled pass.
The fixture namespace/catalog cannot satisfy production source/host/mandatory
argv checks; these fixture commands never substitute for the 13 real commands.
This tests the receipt lifecycle without repeating full collectors. Missing
checks remain failed/blocked, never inferred from artifact existence.

## Mandatory final checks and remaining work

After focused iteration, follow `verificationSchedule.orderedStages` and the
exact command arrays. All original formatting, warning-denying Clippy, workspace
tests, coverage collection, full no-skip harness, preservation/equivalence,
coverage checks and four new validators remain mandatory. The executable order is:

1. Freeze source/config/contracts/claims inputs and compute the current fingerprint.
2. Execute all 11 prerequisite commands and produce actual command, timing,
   historical, preservation and coverage observations. Serialize collectors;
   run live `scripts/coverage/evidence.test.ts` after the last collector. Stage
   completed predecessor evidence. No gates/QA/final/terminal artifacts needed.
3. Assemble immutable gates from those observations and prerequisite receipts.
4. Execute `bun test scripts/harness/exec-fail-open-gates.test.ts`; retain its
   actual completed result locally for the terminal recorder. Gates validation
   reads only gates and predecessors, not its own receipt or future artifacts.
5. Independent oma-qa reproduces and writes/stages its review and receipts.
   Ordinary workspace reproduction does not collect coverage. If QA collects,
   rerun live coverage verification and regenerate affected upstream bindings.
6. The saved final owner writes/stages final evidence binding the actual upstream
   hashes. Confirm sealed count <=148 and all required index bytes match.
7. Execute `bun test scripts/harness/exec-fail-open-evidence.test.ts`; record
   the real result without requiring its own receipt or the terminal sink.
8. Write/stage `terminal.json` from both real terminal command observations,
   preserving every sealed byte/hash and keeping total tracked files <=149.
   Acceptance requires the final decision and actual terminal outcomes to pass.

Hash bindings point from each later artifact to existing predecessors: inputs,
prerequisite observations, gates, review, final, then terminal. The terminal
validator results flow only into the sink. No placeholder pass or optional
production validation mode is allowed. If a terminal check fails, preserve the
failure, repair within ownership, and rerun affected dependent stages. Source
changes invalidate current-source receipts; unchanged inputs do not justify
repeating collectors. The unchanged external verification commands can replay
these validators after terminal creation without demanding another receipt.
Framework per-check timeout is 3,600,000 ms; planning estimates do not shorten it.
`verifyCommand` remains empty; the supplied verification contract is unchanged.

Migration `0001_phase0.sql` SHA-256 is
`046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`,
currently identical in worktree, index and accepted source, regular mode 0644.
Preserve its bytes, mode and type. Keep accounting baseline `fbe76a5...`, feature
guard baseline `19d7a507...` and all protected tooling. Preserve unique canonical
production-file/LLVM-line union, zero-hit lines, test-only exclusions, full source
inventory, fresh native JSON/LCOV, profile/PID/merge equality and exact integer
`covered * 100 >= total * 80` separately for all six crates. No average, rounding,
coverage cfg or new exclusions may compensate for a failure.

Current constraints: Serena is unavailable; snapshot ref is absent locally;
offline dependency readiness, historical fixture execution, runtime timing and
all final gates are unverified. Scoped source reads suffice for this contract;
missing fixture/dependencies or failing checks block implementation acceptance
until actually repaired/rechecked. No acceptance pass is claimed here.
Outstanding: desktop, native Windows, vendor hooks and scheduling. User
adoption, usability, saved time, revenue and ROI are unmeasured.

## Repair verification and handoff

Attempt 2 changed only `contract.md` and `contract.json`. Used `oma-architecture`
Recommendation Mode and its execution/checklist guidance; no structural runtime
boundary, dependency, manifest, guard or preservation baseline changed.
The receipt schema adds `terminal.json`, explicit command phases, dependency
rules and a first-run validator case. Task-3 owns implementing these interfaces;
the saved final/verification owner records the terminal sink. This is an evidence
interface addition, not an Actor or mission-state reassignment.

Executed read-only checks: Git status/HEAD/tree; 23 worktree and accepted-source
SHA-256 comparisons; Bun/Cargo/Rust version probes; scoped policy, contract and
harness reads. All completed successfully except one local inspection script
that incorrectly treated the observation array as a map. Its corrected rerun
verified all 23 bindings; no product test failed. Contract-only structural checks
passed: valid JSON, all 13 exact original argv arrays retained, disjoint 11+2
partition, eight topologically ordered stages and an acyclic first-run receipt
order. Migration bytes matched worktree, index and accepted Git content at mode
0644. Commands executed: `git status --short --untracked-files=all -- .
':!av.yaml'`, `git rev-parse HEAD` and tree inspection, `bun --version`,
`cargo --version`, `rustc --version`, and inline Python JSON/hash/DAG checks; all
returned exit 0 apart from the corrected inspection-script attempt noted above.
Only the two owned contracts were written; the existing design contract belongs
to another Actor and was preserved. Runtime,
coverage, native timing and independent QA acceptance remain downstream work.
Serena tools remain unavailable; no substitute recursive source search was used.


## Revision 3 delivery repair

Attempt 3 found both revision 2 repairs already in the worktree: JSON 50,908
bytes, Markdown 32,573 bytes. The supplied review says delivery tree
`98561a7310708c3c62527905d67362725925d159` instead held revision 1. Preserve the
local repair and capture this revised pair together; neither prior rejected
manifest describes these new bytes. `deliveryProtocol` requires actual byte
counts and SHA-256 values in the handoff, outside these self-hashed files. The
Chief must compare both entries in the fresh AV changeSet and read both files
from the actual delivery tree before admitting task-3. A mismatch blocks delivery.
AV creates that metadata; this task does not edit mission state or invent a tree.

`receiptDependencyGraph` makes the existing dependency rules explicit. Required
predecessors are: inputs; prerequisite observations; gates; independent QA
reproduction and review; final. The gates-command observation depends only on
gates and predecessors. The evidence-command observation depends only on final
and predecessors. Both observations flow into `terminal.json`, which has no
consumer in this graph. The gates command must pass before QA starts; this
control-flow condition does not add a stored terminal receipt to review or final.
Every node must work with successor outputs absent. Index checks and file globs
must obey the same direction. Preservation/docs validators need no gates, QA,
final or sink; the unchanged full harness calls coverage only, so no prerequisite
command may auto-discover either terminal validator. This retains all 13 exact
commands and the disjoint 11+2 split and requires the first-run regression above.

Fresh checks: HEAD/tree, all 23 source bindings, migration worktree/index/accepted
identity and mode, and Bun/Cargo/Rust/host probes passed. The contract-only
`python3 -` command passed JSON/Markdown revision agreement, all 13 exact argv
arrays, disjoint 11+2 phases, nine-node DAG/eight-stage ordering, predecessor-only
construction, three deliberate back-edge rejections and both adapter recipes. These checks do not implement or execute
task-3's validator regression. No runtime suite, coverage collector, native timing
or independent QA pass is claimed. Only the two owned files changed; the design
contract and product files remain untouched. Serena remains unavailable.
