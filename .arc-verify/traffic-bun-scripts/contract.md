# Task-5 concrete shared interface freeze

Backend-contract delivery scope: compiled shared interfaces and binding prerequisite observations, using oma-backend and oma-db. The preserved Chief decisions below remain the behavioral and verification authority. Task-5 does not implement the script resolver, enable a daemon capability, implement the diagnostic read, or certify native Bun-script execution. Their assigned downstream tasks remain necessary. This section freezes the concrete consumer boundary; no saved task, acceptance criterion, mission state, migration or gate recipe changes.

## Concrete detector and metadata boundary

`crates/arc-detector/src/scripts.rs` supplies `PackageSnapshot { package_dir: PathBuf, scripts: BTreeMap<String, String>, fingerprint: MetadataFingerprint { bytes: usize, sha256: [u8; 32] } }`, `MetadataStats`, `MetadataError`, `PackageMetadata`, `ScriptIssue` and `ScriptResolution`. Snapshots are transient and redact Debug. No script body, native argument, cwd, package path or fingerprint enters IPC, SQLite or normal logs. The shared CLI manifest now enables the already-maintained workspace sha2 dependency for fresh content hashes; Cargo.lock changes only arc-cli's dependency edge, with no new package/version. Root/desktop manifests and bun.lock remain unchanged.

The task-6 implementation/export target is `analyze_script_argv(argv: &[OsString], cwd: &Path, metadata: &mut impl PackageMetadata) -> ScriptResolution` in arc-detector, exported from lib.rs. This function is deliberately not stubbed in task-5. Original argv includes the native program token, as in the existing analyze_argv API. The result carries `classification`, the shared `AnalysisStats`, `metadata: MetadataStats`, `graph_edges`, consulted `packages`, and optional `issue`. Existing classify/analyze/classify_argv remain syntax-only, with no filesystem or process access. Context traversal uses the metadata port; the CLI adapter alone performs filesystem reads. Pure parser/rule/budget modules never open files or execute tools. The adapter must reject unbounded/non-regular reads and unsupported discovery; it never searches a whole workspace or runs a command to identify it.

The metadata port has exactly `package(&mut self, cwd: &Path, budget: &mut MetadataStats) -> Result<PackageSnapshot, MetadataError>` and `revalidate(&mut self, packages: &[PackageSnapshot], budget: &mut MetadataStats) -> Result<(), MetadataError>`. Create a fresh adapter per original invocation. Relative explicit cwd is interpreted from its invocation's actual context, then the adapter returns canonical package-directory identity. Deduplicate consulted canonical manifests within that invocation only. Cycle identity includes canonical package and script name; changing forwarded arguments does not evade cycle detection. Revalidation compares freshly read content identity for every consulted manifest, not mtime or length alone. Missing, unreadable, changed or over-budget metadata produces observe/bypass. Revalidate before the first enqueue and charge its reads to the same budget. Later edits, including while queued or between revalidation and Bun's own read, are an explicit non-atomic boundary: the recorded classification describes the inspected snapshot, not a promise to freeze future filesystem contents. Never execute a rewritten snapshot or rewrite native argv to eliminate that race.

| Limit | Frozen value and charging rule |
| --- | --- |
| One package.json | MAX_PACKAGE_BYTES=262144 bytes; bounded read at most permitted bytes plus one sentinel, reject before parsing/retaining an oversized body |
| All reads in one resolution/revalidation | MAX_METADATA_BYTES=1048576; reads, including fresh revalidation, share this total; cap each read by remaining allowance before I/O |
| Unique retained manifests | MAX_PACKAGE_FILES=16; MetadataStats.file before retaining each new canonical manifest |
| Lookup attempts | MAX_PACKAGE_LOOKUPS=64; MetadataStats.lookup before every attempted discovery/open/revalidation, including misses; no uncharged recursive discovery |
| Scripts in one manifest | MAX_PACKAGE_SCRIPTS=256; validate string-valued names/bodies before retention |
| Retained script names plus bodies across manifests | MAX_RETAINED_SCRIPT_BYTES=65536; MetadataStats.scripts uses checked aggregate charging before cloning/retaining |
| Graph edges | MAX_GRAPH_EDGES=128, including nested/lifecycle edges, charged before traversal |
| Existing parser bounds, shared across the graph | MAX_INPUT_BYTES=16384 per parsed command/native input; MAX_TOKENS=1024, MAX_INVOCATIONS=128, MAX_DEPTH=8 and MAX_WORK=65536 remain enforced across every body, leaf and lifecycle traversal |

The port's budget methods reject before committing an overrun and use checked arithmetic. The adapter/resolver must apply all limits; these interface methods alone are not completed bounded-I/O implementation. MetadataError distinguishes Missing/Unreadable/Invalid/Changed and PackageBytes/AggregateBytes/FileLimit/LookupLimit/ScriptLimit/RetainedScriptBytes. ScriptIssue adds Cycle/Dynamic/Unsupported/Mixed/GraphLimit. Map unsafe/unresolved analysis to existing UNKNOWN/OBSERVE reasons and no local reservation; these transient issue details add no legacy wire enum. Remote-only remains REMOTE with no local reservation. Task-6 must supply actual invalidation, cycle, poison-body and bounded discovery/traversal tests, including same-size preserved-mtime edits, and pinned Bun lifecycle/forwarded-argument semantics. No current test receipt claims those implementations complete.

## Classes, admission and durable accounting

`JobClass::Typecheck` serializes TYPECHECK. The only new classification reasons are BUN_SCRIPT_BUILD, BUN_SCRIPT_TEST, BUN_SCRIPT_LINT and BUN_SCRIPT_TYPECHECK, each valid only for its matching host-local class with uncertainty NONE. All supported leaves of one eligible graph collapse to one corresponding script classification. Names never determine class, and individual leaves never become extra submissions. TYPECHECK/COMPOUND_LOCAL is structurally consistent for pure aggregation, but is not accepted as a script enqueue: the final supported script result uses the matching BUN_SCRIPT reason. Unknown, remote and compound bypass behavior remains unchanged.

`TrafficClass::Typecheck`, `Policy.typecheck` and `Active.typecheck` are compiled. Defaults are window=1, cpu_millis=1000 and memory_bytes=1073741824. The existing global defaults remain cpu_millis=2000 and memory_bytes=4294967296. decide uses the same FIFO, class-window and checked global CPU/memory paths for all four classes. Cargo BUILD and script BUILD use one Build counter. The storage active-permit scan and policy-class codec accept TYPECHECK; generic lifecycle classification serialization already stores its reason and uncertainty. Transactional enqueue/poll/replay/outcome/reopen/release tests cover the new class and both durable BUILD orderings, without claiming real child execution.

`arc_policy::ScriptPolicyLimits { build, test, lint, typecheck, budget }` is the new four-class DTO. It validates the actual positive, representable policy, each reservation against its shared budget, and safe-integer wire resources; it permits valid configured limits rather than asserting defaults are the only possible policy. Its default is the Policy default above. Existing PolicyLimits/EffectivePolicy and policy.memory.v1 retain their original three-class bytes and validation. ScriptPolicyLimits.legacy drops only typecheck for that old shape; it does not change stored accounting or supply downgrade support.

No migration is added. All six compiled migration bytes and hashes are recorded in integration.json/preflight/migrationReadback and match accepted HEAD. Keep application policy in Rust decide and the existing service/writer, with permit/accounting transitions under the existing SQLite IMMEDIATE transaction and publication/launch fences. No trigger, procedure, DB job, outbox or alternate writer is added. DB constraints and recovery ownership stay in their maintained paths. Older binaries opening a database containing the new class remain outside the protocol-interoperability promise; do not relabel this as binary/database downgrade acceptance.

## Negotiated operations, lifecycle and legacy projection

Protocol version remains 1. The new probe returns exactly `{ "capability": "traffic.bun-scripts.v1" }` for `traffic.bun-scripts.capabilities`; payload is null. `traffic.bun-scripts.job` uses the existing GovernorJobRequest envelope, NOT a bare traffic::JobRequest: for enqueue it is `{ "action": "TRAFFIC", "request": { "action": "ENQUEUE", ...existing fields... } }`. `traffic::scripts::decode_job(operation, payload)` checks the operation before decoding, strictly bounds the object tree to MAX_EXECUTION_BYTES=16384, validates the existing wrapper/identities and accepts only matching BUN_SCRIPT classifications at enqueue. Other owner lifecycle/status actions reuse the wrapper. Reply is existing GovernorJobResult, preserving reserved/unreserved/unconfirmed semantics. Launch and ACK use unchanged governor.launch/governor.launch.ack contracts with existing one-shot release markers, pressure/publication fences and native witness recovery. Classification is absent from those launch payloads. The task-7 daemon must dispatch this new operation to the same governed worker path and advertise it only when complete; no handler is enabled by task-5.

Existing `traffic.v1`, `traffic.bun.v1` and Governor capability responses remain exact. traffic.job rejects all new classifications. traffic.bun.job continues to accept direct BUN_TEST only at enqueue. governor.job keeps its predecessor vocabulary (including direct BUN_TEST), rejecting all new script/TYPECHECK enqueues. job.start registration rejects the new vocabulary. `Classification::legacy()` projects direct Bun-test and all script/new-class classifications to UNKNOWN/UNKNOWN/UNRESOLVED_COMMAND/OBSERVE. job.classify and every legacy lifecycle snapshot reply apply this projection without modifying the stored classification. The shared projection helper is tested for every new script reason, direct Bun-test and unchanged Cargo replies. Task-7 must retain it on every old response path it touches; new diagnostics return the actual stored classification.

`traffic::interpret_probe_for` binds exact local operation/capability pairs, request ID, protocol, object shape and response envelope; old interpret_probe remains bound only to traffic.v1. Only an explicit valid unsupported reply supports a pre-submission fallback. Disabled/unrecognized operations return UNSUPPORTED_CAPABILITY before decoding new enum payloads. Invalid, busy, unavailable or mismatched replies are not negative durable evidence. Consumers must probe the new capability independently; Governor support or direct-Bun support alone is insufficient.

Allocate one original job ID and observer pid/start-token/boot-id tuple for the whole native invocation. Once enqueue may have been sent, preserve that ID, its witness, lineage/release context, Continuity and uncertainty across retries, lost ACKs and peer replacement. Reconcile the same durable job rather than resubmitting with another ID or spawning again. An unsupported replacement peer cannot prove the earlier enqueue did not commit. Before a definitely unsent unsupported fallback, project to the legacy observation vocabulary and preserve the original argv/cwd/env/stdio/signals. After uncertain submission, follow the existing bounded witness/fallback/recovery protocol and retain uncertainty where no authoritative reconciliation exists. Healthy queued waits are not outage deadlines. STARTED requires actual native spawn and attach evidence. New capability routing cannot bypass durable OFF, pressure checks, exactly-once launch fences, Meter lineage or nested root-permit reuse.

## Negotiated diagnostic and desktop boundary

`traffic.diagnostics.capabilities` returns exactly `{ "capability": "traffic.diagnostics.v1" }` for a null probe. `traffic.diagnostics.job` takes the strict object DiagnosticRequest `{ job_id }`, at most 512 compact bytes. DiagnosticResult is bounded to 8192 bytes, ID-bound, schema_version=1 and liveness_fresh=false. It carries recorded_at_ms (the read snapshot's upper-bound timestamp, not a native liveness refresh), nullable actual Classification, nullable AdmissionDiagnostic and actual ScriptPolicyLimits. Absent classification means no classification was recorded/read, not unsupported-peer proof or a guessed class. Unsupported transport/capability stays an explicit unavailable result at the consumer, not this null case.

AdmissionDiagnostic has state QUEUED/RESERVED/RUNNING/COMPLETED/CANCELLED/ABANDONED/UNCONFIRMED/UNRESERVED; sequence and queued_at_ms; required nullable granted_at_ms, queue_wait_ms, wait_reason, last_decision, root_job_id, permit_generation, reservation and outcome; and permit_active. Permit identity/resources are all present or all null. A historical permit may remain visible with permit_active=false; it is not an active reservation. Root identity is the job ID for an owned permit, or the actual root for reuse. Waiting cause is a historical DecisionReason and must match a recorded WAIT decision; absent cause remains unknown, never a fabricated class wait. Reserved requires an active permit; completed requires an outcome and may still share an active root permit with verified descendants until they finish. UNRESERVED cannot claim an active permit. All numbers are validated safe integers, timestamp ordering and any available grant/wait arithmetic agree, and a recorded admission requires a supported local class. Unknown fields, positional arrays, missing required nullable fields, wrong IDs, invalid enum/combination/range and misleading freshness are rejected. No lineage secret, raw argv, environment or cwd is exposed. Existing policy reads retain effective pressure/freshness and historical audit semantics; this new read never runs another scheduler or native sampler.

Task-7 implements the bounded owner-authenticated daemon read on the existing worker/connection admission, deadline and transactional read paths, using one coherent committed SQLite snapshot. Task-8 registers and validates the production desktop bridge projection, then extends existing selected-job diagnostics and accessible interactions. It must use the negotiated data for truthful Typecheck, class-window versus shared CPU/memory/pressure/FIFO causes and uncertainty; it must preserve controls and avoid a desktop DB connection. Classification, execution/outcome and permit reconciliation remain task-9 native acceptance obligations.

Rust-derived declarations and serialized fixtures are delivered at `apps/desktop/src/bridge/traffic-scripts.generated.ts` and `apps/desktop/tests/fixtures/traffic-scripts-wire.json`. The explicit maintained-style exporter is `ARC_TRAFFIC_SCRIPTS_EXPORT=1 cargo test -p arc-ipc --lib traffic::diagnostics::typescript::regenerate -- --exact`; ordinary runs only compare bytes. ts-rs 12.0.1 derives the real diagnostic and Outcome structures; valid Classification unions and numeric limit structures come from Rust/serde values. Shared existing DecisionReason/RecordedDecision declarations are imported from the unchanged policy.types. No production dependency on ts-rs is introduced. Task-8 supplies its assigned compiler/runtime consumer tests; task-5 also compiled serialized fixtures against these declarations using installed TypeScript 7.0.2, including negative type assertions.

## Exact retained ownership and handoff

Backend-contract retains all Cargo/package manifests, Cargo.lock, bun.lock, maintained coverage registries (including scripts/coverage/config.ts), all arc-ipc contracts/exporter sources, arc-policy, arc-traffic, arc-storage and daemon production paths for its task-7/final-gate scope. In particular retain `crates/arc-detector/src/types.rs` as the shared class/reason contract, and `crates/arc-daemon/src/connection/legacy_projection.rs`. Requests to change these shared interfaces return to this writer through the Chief; consumers do not edit a shared manifest concurrently.

The actual Git-visible task-5 code dependency transfers these exact paths to backend-execution for task-6: `crates/arc-detector/src/lib.rs`, `crates/arc-detector/src/scripts.rs`, `crates/arc-detector/tests/script_contracts.rs`, and the remaining existing detector parser/argv/rules/lexer/budget implementation files within that Actor's saved ownership; all existing CLI source/test implementation paths remain that Actor's. The frozen public signatures, limits and class/wire semantics above remain compatible. Necessary new resolver/metadata adapter modules and tests live under its owned detector/CLI paths. The CLI manifest and lockfiles stay with backend-contract.

The actual Git-visible dependency transfers `apps/desktop/src/bridge/traffic-scripts.generated.ts` and `apps/desktop/tests/fixtures/traffic-scripts-wire.json` to backend-native together with its existing desktop consumer/test ownership. Regeneration must use the delivered Rust exporter; no manual wire edits. Backend-native retains the designated task-9 entry points `crates/arc-daemon/tests/traffic_bun_scripts.rs` and `apps/desktop/src-tauri/tests/traffic_bun_scripts.rs` and fixture modules. These later files are not placeholder-created here. Backend-contract stops writing the transferred paths at delivery. All other ownership/acceptance from the preserved Chief contract remains unchanged; no task instructions or Actor assignment is edited.

contract.md freezes at this delivery. Backend-contract owns only this task's preflight addition to integration.json; task-9 receives the whole design matrix and this preflight evidence as an actual dependency and becomes its aggregate writer. Prior design/native-status sections are preserved unchanged. final.md remains untouched. gates.json/qa.json are not created early. There are still exactly five allowed canonical destinations, not five placeholder passes.

## Task-5 prerequisite and verification evidence boundary

Current receipts, migration hashes, tool versions, preparation, scoped workspace observations, output visibility, fixture readiness and command results are in integration.json/preflight. The exact six final recipes, two mandated desktop argument orders, traceability checks, post-collection evidence check, operator note and timeout fields in the preserved Chief body below remain unchanged. The current task runs only preparation and interface/regression checks, plus the workspace fmt/clippy/test checks required by the operator's environment note. Those are task-5 checks, not the exclusive final gate or Bun-script native acceptance. Any earlier check invalidated by a subsequent source edit is labeled superseded; only a current-input run can satisfy delivery.

Coverage collection count remains zero. `BUN_SCRIPT_INPUTS=[".arc-verify/traffic-bun-scripts/contract.md"]` is appended to maintained FROZEN_INPUTS, with no generated integration/gates/QA/final report added, no baseline/floor edits and no harness recipe change. The maintained --preflight-only command is rerun on the delivered contract and code. All eleven 80% floors remain requirements for the exclusive final collection; none is claimed measured here. Configured-target readiness uses the supplied no-observation-policy finding, never a bypass read of av.yaml. Existing absolute mission deadline, task/run/retry/concurrency limits and 7200000 ms per-check allowance remain in force, without a new mission or deadline.

---

## Preserved task-4 decision record (unchanged body)

# Bun package-script Traffic contract — frozen Chief decisions

Status: task-4 attempt-5 recovery decision repair complete, 2026-11-14. Behavioral acceptance, preservation boundaries, ownership sequence and retained verification recipes are frozen. Concrete implementation signatures and finite metadata caps remain task-5 deliverables; they may refine interfaces without weakening acceptance. Implementation, native acceptance, integration preflight, final gates, coverage and final QA remain pending. Supported Bun-script slice only.

The Chief used oma-pm and oma-coordination against the existing saved assignment and resumed its unstaged report repairs. No replacement plan, Actor assignment, mission graph, workflow, validator or additional report is created. This decision record precedes the preserved task-1 technical draft below; statements there about draft status, ownership or unavailable inputs describe that earlier task. Its behavioral, preservation and gate-command obligations remain in force. The current recovery instruction supplies the operative preflight, configured-target and timeout decisions below and explicitly supersedes the earlier review's uncertainty about timeout precedence.

## Original charter and unchanged goalBrief

The operator's original mission goal is retained verbatim:

> At accepted HEAD 5578e69fc63ada7a73190f5681e7960da17969f7, deliver macOS Traffic admission for one-shot Bun package scripts across BUILD, TEST, LINT and TYPECHECK, addressing DET-003/004/008/009 and TRF-002/003. Assign implementation, tests, repairs and native gates to backend-engineer Actors using oma-backend, with oma-db and oma-frontend where affected, followed by independent read-only oma-qa. Resolve bun run scripts using bounded package.json metadata, including explicit cwd, nested scripts, lifecycle scripts and forwarded arguments. Support graphs resolving to Bun/Vite builds, Bun tests, Biome checks and tsc --noEmit, including this repository's desktop build/typecheck scripts. Classification must inspect commands rather than script names and must never execute them. Unresolved, cyclic, dynamic, mixed or unsupported graphs and watch/hot/workspace fan-out remain non-blocking observe/bypass; remote-only scripts consume no local reservation. Add a distinct TYPECHECK window sharing the existing global resource budget. Preserve original invocation semantics, Governor control, pressure enforcement, Meter accounting, migration bytes and protocol-1 compatibility through capability negotiation and legacy projections. Success requires deterministic resolution and metadata-invalidation tests; real Bun build/test/lint/typecheck executions with independently checked outputs and outcomes; and synchronized production-daemon CLI tests showing Cargo and Bun-script builds share a one-slot BUILD window in both arrival orders, with the second visibly queued and both executing exactly once. Two typechecks must similarly respect a one-slot TYPECHECK window, and mixed classes must stay within the shared budget. Reconcile classifications, execution counts, outcomes and released permits through SQLite, CLI and the production desktop bridge. Cover exit 17, cwd/environment/stdio/signals, nested execution, cancellation, lost acknowledgments, restart and bounded outage fail-open. Pass the six mandated gates and affected desktop checks, retaining every crate's 80% production-line floor. Follow the binding integration preflight, exclusive backend-owned final gate, coverage reuse and compact evidence policies. Update only affected traceability rows and counts; add no document or evidence validators. Limit completion claims to this supported Bun-script slice.

The supplied goalBrief interpretation is unchanged:

> At accepted HEAD 5578e69fc63ada7a73190f5681e7960da17969f7, extend macOS Traffic to the requested one-shot Bun package-script slice across BUILD, TEST, LINT and TYPECHECK, addressing DET-003/004/008/009 and TRF-002/003. The immediate product value is predictable admission for developers mixing Cargo and Bun worktrees while preserving unchanged invocations and trustworthy outcomes. Choose bounded package metadata snapshots and the maintained pure parser, an additive negotiated protocol boundary, one shared resource budget, and automatic admission with contextual CLI/job-detail status. These choices adopt the Technical Director's reuse recommendations and the Design Director's contextual-status option. The Marketing Director's proposed assessments, acquisition routes and retention experiments remain outside this technical charter because demand, costs and returns are unmeasured. A general shell engine, broad workspace support and unrelated UI or commercial changes would delay the requested payoff. Backend-engineer Actors own every implementation, test, repair and native gate; independent oma-qa follows. Supplied Director findings are advisory evidence with unresolved acceptance obligations, not approvals. This response plans later execution; the current planning stage changes no files and runs no gates.

The last sentence describes the earlier goalBrief planning stage. This task is explicitly authorized to write the two decision reports; it runs no code gate.

The six goalBrief success criteria are retained without alteration:

1. Deterministic Rust and native macOS tests prove bounded four-class Bun-script resolution, contention, exactly-once recovery and preservation, reconciled through SQLite, CLI and the registered bridge.
2. Desktop and headless diagnostics expose truthful classifications and queue causes through negotiated interfaces while preserving controls and accessible interactions.
3. All six mandated gates and affected desktop checks pass exclusively on the frozen final inputs, with binding preflight and unchanged verification recipes.
4. Each of eleven crates retains at least 80% production-line coverage, with one successful final-input collection and validated harness receipt reuse.
5. Independent read-only QA delivers accurate, reproducible findings and evidence limitations bound to the final inputs.
6. Affected traceability rows and counts, compact evidence and the final report accurately limit completion to the supported Bun-script slice.

The goalBrief assumptions are preserved as supplied, rather than promoted to observed results:

1. The accepted HEAD was read back as 5578e69fc63ada7a73190f5681e7960da17969f7. Existing migration bytes, historical accounting baselines, Governor behavior and protocol-1 contracts remain preservation requirements.
2. Eligible graphs contain supported one-shot command leaves resolving to Bun/Vite builds, Bun tests, Biome checks or tsc --noEmit. Classification uses command contents, cwd, lifecycle and argument semantics rather than script names. Ineligible or uncertain graphs bypass without blocking; remote-only graphs reserve no local resources.
3. Fresh bounded package.json reads per resolution are the initial metadata invalidation strategy. No persistent cache, general shell interpreter, new service or runtime dependency is justified without a demonstrated implementation gap.
4. The root desktop:build and desktop:typecheck delegations, including bun run --cwd apps/desktop and the two-command tsc --noEmit chain, are required supported cases.
5. Native acceptance requires actual macOS execution with the pinned Rust/Bun toolchain and real installed tools. Missing tools, denied access or unavailable dependencies are unmet prerequisites, never skipped acceptance or grounds for GUI workarounds.
6. Serena tools were unavailable during planning, so inspection used already identified file reads and non-code evidence. av.yaml access was denied and was not bypassed. Runtime-supplied saved preflight, verification and configured-target requirements must be reconciled before execution claims; their absence is not proof that no additional requirements exist.
7. The mission allows at most three concurrent workspace tasks, retains its existing 172800-second absolute budget, 400-run limit and three-retry limit, and allows 7200000 milliseconds per verification check without resetting the mission deadline.
8. Use five non-ignored canonical evidence files under .arc-verify/traffic-bun-scripts, collectively at most 300000 UTF-8 bytes. Generated reports do not enter coverage input identity; the frozen contract does. Ignored raw outputs are local diagnostics and cannot be the sole handoff.
9. No live deployment, installed-service restart, publication, messaging or spending is authorized by this plan. Native tests may manage only their own isolated fixture processes. Any separately required live deployment, rollback or restart needs an authorized effectScope external task and actual service health evidence.

The assumptions above preserve the supplied goalBrief wording. The current task-4 recovery retains the review finding that no container or service observation policy is configured and supplies the saved task-5/task-11 obligations. It explicitly establishes that mission.verificationCheckTimeoutMs=7200000 is the framework's per-check timeout, whereas verificationContract timeoutMs=120000 values are planning estimates and never terminate a check. Both numeric fields remain unchanged; replays and wrappers must allow at least 7200000 ms, subject to the existing absolute deadline and cancellation rules. This current authority supersedes the earlier review's uncertainty about precedence. Neither denied av.yaml access nor missing text is the basis for these decisions; no additional target inventory is required.

## Inputs consumed and interpretation of Director advice

These actual dependency bytes were read in the integrated mission worktree before edits:

| Dependency / transferred ownership | Bytes | SHA-256 at task-4 intake |
| --- | ---: | --- |
| task-1 technical contract; contract.md now transfers to Chief, then task-5 for signature completion | 27300 | 5235741fbbbbe80ef7ce16668a55631ef8c13783dc27104abe46b447567976c3 |
| task-2 design matrix; integration.json remains read-only in task-4 | 66119 | 098437395f41ca34796c7215dfef52744cbd836ac1c8271d3fe631374c5e6dde |
| task-3 value assessment; final.md now transfers to Chief | 16008 | d623a51823a9bc47161a6450adac68e4ee9e03d922535ba2dcf351df71eacf57 |

The three supplied dependency reviews passed their report-only acceptance. All three original Director advisory records have passed=false. Those initial non-passing statuses identify unresolved obligations; they are neither a veto nor a technical approval. Report acceptance does not turn a recommendation into executed product acceptance.

| Director | Chief decision and specific interpretation | Alternatives deferred or rejected |
| --- | --- | --- |
| Technical | Adopt the maintained Rust/Tokio/rusqlite/serde and pinned Bun/Tauri/React stack, a bounded CLI metadata adapter feeding pure contextual parsing, fresh reads, additive negotiated admission/diagnostics, checked shared accounting and existing native/coverage machinery. Preserve every technical finding's acceptance obligation, including peer replacement after uncertain enqueue and migration-byte integrity. | Reject a general shell interpreter, resolver service, whole-project scanner, persistent cache and new dependency absent a demonstrated gap. Reject silently widening traffic.v1/traffic.bun.v1, classifying by script name, rewriting invocations, or treating protocol interoperability as same-database downgrade proof. No measured cost/model evidence justifies changing the selected Chief route. |
| Design | Select automatic admission with contextual status in existing CLI and selected-job detail. Preserve the complete D01–D16 matrix in integration.json, associated semantic labels, actual queue causes, freshness, keyboard/focus and meaningful announcements. P1 concurrent macOS worktrees, P2 mixed Rust/frontend maintenance and P3 keyboard/headless recovery are inferred contexts, not studied participants. | Silent admission gives less waiting/recovery context; a pre-execution confirmation adds an unsupported decision. No new confirmation, onboarding, navigation, dashboard redesign, ETA, progress claim, reminder or automatic re-enable. Reminder/nagging retention proposals remain unmeasured and outside scope; durable OFF remains authoritative. |
| Marketing | Finish the supported technical slice before considering paid assessments or distribution. Accept the workload-fit inference from actual desktop scripts as a reason to prioritize this gap. Retain task-3's qualified value assessment and future measurement proposals in final.md. | Defer paid setup, referrals, public-manifest prospecting, community promotion, catalog distribution, acquisition, billing and retention experiments. No outreach, spending, customer demand, measured benefit, profitable price, CAC, LTV or ROI follows from this task or predecessor receipts. |

The selected product journey is one unchanged invocation, truthful waiting/start/outcome context when available, and optional inspection of the same job ID. Readback repairs knowledge; it never reruns work. Reservation estimates, observed consumption, historical decisions and current liveness remain distinct.

## Frozen behavioral acceptance

The CLI captures original native argv and caller cwd, acquires only bounded package metadata needed by the reachable literal graph, and supplies snapshots to the maintained pure detector. Analysis never runs a script body, tool probe subprocess, shell expansion or package installation. Apply finite per-file and aggregate bytes, file/lookups, retained script bytes, graph depth/edges/invocations, tokens and work budgets across all recursion and lifecycle traversal. Existing parser protections remain; task-5 publishes additional concrete caps, checked charging rules and boundary tests. Finite bounds cannot be deferred into unbounded I/O.

Fresh reads per top-level invocation must detect replacement, deletion, nested/lifecycle edits and same-size edits with preserved mtime. Per-resolution memoization is allowed. Task-5 specifies the metadata snapshot/revalidation boundary and race behavior; it cannot claim atomicity over Bun's later reads. Preserve original execution semantics rather than running a rewritten snapshot.

Resolve explicit cwd relative to the relevant invocation context, nested scripts, applicable lifecycle scripts and forwarded arguments including their option/quoting boundaries. Supported leaves are one-shot Bun/Vite builds, Bun tests, Biome checks and tsc --noEmit; both desktop typecheck commands and root delegations are mandatory. Same-class chains may be eligible only after every reachable leaf is resolved. Names supply no class. Cyclic, dynamic, unresolved, mixed, unsupported and watch/hot/workspace fan-out graphs observe/bypass without a local reservation or execution block; proven remote-only graphs consume no local reservation.

TYPECHECK has its own window while all four classes use the same checked CPU/memory resource sums. Cargo and Bun-script BUILD share BUILD admission. One original invocation receives at most its applicable root reservation; script leaves do not become separate child submissions. Preserve verified nested permit reuse, lineage, non-additive Meter accounting, queue/run timing, durable Governor controls, pressure freshness/enforcement, launch fences, native stdio/signals/exit behavior and exactly-once recovery.

New class/reason/policy/read vocabulary requires additive capability negotiation. Existing protocol-1 response vocabulary and shapes retain valid legacy projections on every reply path. Reject unsupported operations before decoding new enums, including peer replacement after a probe. A lost enqueue ACK followed by an unsupported peer cannot prove that submission never committed; retain job IDs, witnesses and uncertainty. Preserve migrations 0001–0006 byte for byte. Unrestricted TEXT alone justifies no migration. Same-database older-binary readback is a separate unsupported promise unless independently proved.

### Compact acceptance case matrix

These case keys index this mission's evidence; they are not new SRS requirements, new validators or executed passes. The backend owners bind exact maintained test selectors and observed results at task-5/integration. Every original obligation and every D01–D16 assertion remains required even when grouped here. All cases are pending at task-4 delivery.

| Key / design cross-reference | Required deterministic/native observation | Result destination |
| --- | --- | --- |
| BS-R01 / D01–D02 | Pure command-based BUILD/TEST/LINT/TYPECHECK classification; misleading names; explicit cwd; nested and applicable pre/post lifecycle scripts; forwarded argv; same-class chains; actual desktop build/typecheck graphs including both tsc commands. Poison-script fixtures prove analysis never executes bodies. | integration.json /resolution/allRequiredCasesPassed |
| BS-R02 / D03 | Fresh bounded metadata reads, replacement/deletion, same-size preserved-mtime edits, nested/lifecycle changes, missing/invalid/unreadable/oversized metadata, graph cycles and every per-file/aggregate limit boundary; race/snapshot boundary assertions. | /resolution/allRequiredCasesPassed |
| BS-R03 / D04 | Dynamic, unresolved, mixed, unsupported, watch/hot/workspace fan-out bypass without local reservation or blocking; remote-only consumes none. Unsupported detail remains unknown rather than invented remote/no-permit proof. | /resolution/allRequiredCasesPassed; /reconciliation/executionCountsOutcomesAndReleasedPermitsPassed |
| BS-N01 / D01, D15 | Real installed Bun build and Vite build package scripts; independently inspect produced artifacts and their usable output, native outcome and body/invocation count. Include actual repository desktop build. | /nativeAcceptance/realToolsAndIndependentOutputsPassed |
| BS-N02 / D01, D15 | Real Bun-test script executes independently witnessed test bodies with checked expected success/failure outcomes; output alone is insufficient execution proof. | /nativeAcceptance/realToolsAndIndependentOutputsPassed |
| BS-N03 / D01, D15 | Real Biome-check script on known clean and invalid inputs, independent expected diagnostics/outcomes and execution counts. | /nativeAcceptance/realToolsAndIndependentOutputsPassed |
| BS-N04 / D01, D15 | Real tsc --noEmit scripts on known valid and invalid inputs; independently check diagnostics, outcome and lack of emitted files. Include root desktop:typecheck and both project checks. | /nativeAcceptance/realToolsAndIndependentOutputsPassed |
| BS-N05 / D05, D07, D15 | Production daemon: Cargo BUILD first, Bun-script BUILD second, BUILD window=1. Synchronize real entry/release; observe second queued with zero starts while first lives, then exactly one execution and checked outcome each. Independent work/output directories prevent tool locks from explaining serialization. | /nativeAcceptance/buildBothArrivalOrdersPassed |
| BS-N06 / D05, D07, D15 | Repeat the same assertions with Bun-script BUILD first and Cargo second. Both orders require their own complete queue/count/outcome/release evidence. | /nativeAcceptance/buildBothArrivalOrdersPassed |
| BS-N07 / D05, D15 | Two real typecheck jobs, TYPECHECK window=1; second visibly queued until release, zero premature starts, exactly one original execution each and independently checked outcomes. | /nativeAcceptance/typecheckSingleWindowPassed |
| BS-N08 / D06, D15 | Mixed supported classes exceed one shared CPU reservation budget while class slots and memory allow; independently demonstrate budget queuing, then grant/release without exceeding checked global sums. | /nativeAcceptance/mixedSharedBudgetPassed |
| BS-N09 / D06, D15 | Independently constrain shared memory reservation budget while class slots and CPU allow; distinguish reservation cause from measured pressure and prove no permit leak. | /nativeAcceptance/mixedSharedBudgetPassed |
| BS-N10 / D07–D09, D16 | Original program/argv including spaces/empty/forwarded arguments, caller/explicit cwd, environment and stdin/stdout/stderr; terminal/nonterminal signal forwarding, exact exit 17, signal identity, spawn failure and known versus unconfirmed outcome. One native spawn path, STARTED only after actual spawn. | /semantics/allRequiredCasesPassed |
| BS-N11 / D10, D13, D16 | Verified nested execution preserves ancestry, root permit ownership/release and Meter lineage; no second independent reservation or double-counted nested usage. Queue wait and run elapsed retain their bases; null/zero/Partial stay distinct. | /semantics/allRequiredCasesPassed; /preservation/governorPressureMeterPassed |
| BS-N12 / D09–D10 | Cancellation before enqueue/spawn, while queued and during execution: exact start counts, actual native outcome, durable cleanup/release and uncertainty where cleanup cannot be confirmed. No revival or duplicate execution. | /recovery/allRequiredCasesPassed |
| BS-N13 / D10, D14 | Lost enqueue/start/completion acknowledgments, probe/enqueue peer replacement and lost-ACK then unsupported replacement; reconcile same job ID, witness and outcome without blind resubmit/respawn. | /recovery/allRequiredCasesPassed; /compatibility/protocol1AndPeerReplacementPassed |
| BS-N14 / D07–D10 | Isolated production-daemon restart and bounded outage fail-open, including queued/running ownership and final release. Healthy queue wait is not an outage deadline. Preserve single execution through recovery and bounded preparation. | /recovery/allRequiredCasesPassed |
| BS-P01 / D06, D11, D13 | Governor ON/OFF durability and lost-mutation-ACK readback, pause/resume while queued, survival of running children; fresh/stale pressure and launch races; Meter outcome/lineage/timing preservation. | /preservation/governorPressureMeterPassed |
| BS-C01 / D04, D14 | New/old client/daemon combinations, strict protocol-1 shapes and every new-class/reason legacy projection; authenticate and bound new reads; reject malformed/wrong-ID/unknown variants. Compare all six original migration bytes against accepted HEAD. | /compatibility/protocol1AndPeerReplacementPassed; /preservation/migrations0001Through0006Match |
| BS-D01 / all D01–D16 | Registered production desktop router and CLI expose truthful class, actual class/FIFO/CPU/memory/pressure cause, freshness, uncertainty and outcomes through negotiated DTOs. Rust-derived TS/compiler/runtime agreement; labels, focus, Space/Enter, list navigation and meaningful-only announcements. Existing Governor behavior remains. | /diagnostics/cliAndRegisteredBridgeTruthful; /desktop/rustTypeScriptAgreementPassed; /desktop/interactionAndQueueCauseCasesPassed |
| BS-X01 / D05, D09–D15 | For every relevant native case reconcile stable job IDs, stored classifications, reservations/root/generation, visibly queued states, execution counts, outcomes, Meter and final released permits through SQLite, CLI and the production registered bridge. Read-only status commands are not child-success proof. | /reconciliation/sqliteCliRegisteredBridge; /reconciliation/executionCountsOutcomesAndReleasedPermitsPassed |
| BS-T01 | Change only DET-003/004/008/009 and TRF-002/003 traceability rows and consequent counts from delivered evidence. Preserve unrelated rows, broad unsupported scope and pre-existing evidence. | /traceability/targetRequirementIds; /traceability/unrelatedRowsUnchangedAndCountsReconciled |

Native rows require actual macOS tools, production daemon/CLI and registered production bridge reads, with independent output/outcome oracles. Command stubs, mocked Ready responses, declaration-only scenarios, sleeps alone, tool-internal locks or predecessor receipts cannot satisfy them. Synchronization must expose the waiting interval and zero premature child/body starts. Consumers may use deterministic fixtures for interaction/contract assertions; those are distinct evidence.

## Ownership, integration and review sequence

This table records the existing Actor responsibilities and allowed handoffs; it does not widen a saved task's scope. Preserve other Actors' changes. Report any conflict between this table and an actual saved task to Chief with exact paths and source-bound evidence before touching the contested path.

| Stage / owner | Exclusive responsibility and boundary | Dependency artifact before the next owner |
| --- | --- | --- |
| task-4 Chief | contract.md decisions and final.md strategy; integration.json read-only. Consume actual task-1/2/3 artifacts. No product code, code gates, nested mission or external effect. | This non-ignored contract and preliminary final report, preserving all three dependency bodies. |
| task-5 backend-contract, using oma-backend and oma-db as needed | Complete concrete signatures, wire names/reasons, metadata budget/read boundary, TYPECHECK defaults, bounded diagnostic DTOs, generation/read/recovery contracts and focused regression selectors in this contract. Own shared public definitions and all manifest/lockfile/registry edits. Execute the supplied binding preflight handoff below, preserve immutable verification inputs and retain the established absence of a container/service observation policy. | Git-visible shared definitions, compilable dependent interfaces, exact signatures and ownership release recorded here; no consumer implements an imagined API. Behavioral acceptance and recipes cannot be weakened. |
| Post-contract lane A: backend-contract | Traffic/policy/storage/daemon implementation and deterministic tests, durable writer/accounting/recovery and legacy projection. Own crates/arc-traffic, crates/arc-storage, crates/arc-policy, crates/arc-ipc and daemon product paths/tests except native-acceptance fixture paths assigned below. Use oma-backend; durable changes additionally use oma-db. | Delivered shared/runtime contracts and tests, actual whole-workspace integration-check evidence, preservation and migration evidence. |
| Post-contract lane B: backend-execution | Pure resolver and bounded metadata adapter under crates/arc-detector and original-invocation CLI integration/diagnostics under crates/arc-cli. Shared detector type/export edits begin only after task-5's actual file handoff. Own deterministic resolution/invalidation/CLI tests and later only the six affected traceability rows/counts. Use oma-backend. | Git-visible resolver/CLI code and meaningful tests, exact selector/outcome receipts and traceability diff. Request shared manifest/wire changes from lane A rather than editing them. |
| Post-contract lane C: backend-native | Registered desktop Rust bridge, generated/validated TS consumers, contextual diagnostics and deterministic desktop tests under apps/desktop. Task-9's designated maintained native entry points are crates/arc-daemon/tests/traffic_bun_scripts.rs and apps/desktop/src-tauri/tests/traffic_bun_scripts.rs, with their fixture modules. Existing traffic_cli fixtures remain reusable support within the saved ownership scope. Use oma-backend, oma-frontend for consumers and oma-db for reconciliation. | Git-visible bridge/tests/native fixture changes and compact independently checked execution observations. Shared manifests/registries stay lane A-owned. |
| Integration handoff: backend-native within its saved integration scope | Consume all three actual code artifacts, integrate native/consumer behavior and own integration.json as the single aggregate evidence writer after task-2 handoff. Coordinate product repairs with the exact backend path owner; no concurrent edits to another lane's paths. | Final combined source/tests, exact executed selectors, native case observations and reconciliation, ready for affected traceability completion and final input freeze. Evidence-only requests do not justify product replay. |
| Final gate: backend-contract, exclusively | After all approved repairs, affected traceability changes and task-5 contract additions are delivered, freeze inputs and stop other writers/compute tasks. Run the binding integration preflight and all final gates alone in the integrated mission workspace. Write gates.json and the single final input identity. Only this backend-engineer owns native final gates. | Source-bound final-input command receipts, one successful coverage collection and validated harness reuse. A repair invalidates affected earlier results and requires necessary checks on the repaired inputs; never relabel stale gates current. |
| Independent reviewer, oma-qa | Product files, tests, configuration and gate inputs read-only; write only qa.json. Review delivered assertions, compact native observations, reproducibility and final-input receipts. Do not repair code or rerun native build gates/unchanged expensive coverage. | qa.json linked to the same final identity with accurate findings/limits. Defects go to responsible backend owner within existing recovery limits, followed by affected rechecks and review. |
| Chief final judgment | Complete final.md using current acceptance, preflight, gate, coverage, traceability and independent QA evidence. No native gate or implementation. | Supported-slice decision and remaining limitations; no whole-phase, commercial or external-effect completion claim without its own evidence. |

The sole shared-manifest/lockfile/registry writer is backend-contract, including Cargo.toml manifests, Cargo.lock, package.json manifests, bun.lock, shared export/DTO registries and maintained coverage-input registration. Desktop command registration remains with backend-native under its established consumer scope after shared API delivery; concurrent registration edits are prohibited. Adding this frozen contract to maintained coverage identity is a backend-contract change, not authority for Chief to edit tooling.

An ownership transfer requires the outgoing writer to stop, identify exact paths/current revision/unfinished work, and deliver actual non-ignored Git-visible changes that the dependent saved task receives. Mere agreement or a future API description is not a delivered dependency. If a new overlap appears, Chief resolves the existing ownership; no Actor may modify saved tasks, acceptance criteria, graph, Actor assignments or mission state. No dispatch is performed by task-4.

## Exact verification recipes, preflight and exclusive final inputs

Working directory for every command below is the integrated mission workspace:

    /Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/bb94c0dc-5bb1-454e-bcf7-c34dde821115/workspace

The supplied immutable verificationContract version 1 and the saved task-5/task-11 obligations supplied by the current recovery are authorized current inputs. The empty verifyCommand waives none of them. Saved task-11 retains the six exact recipes below, also preserved in task-1 and its named predecessor reference. Their argv is unchanged. The immutable verificationContract separately requires the three Rust commands and its traffic-scripts-first desktop command, along with the exact file/JSON assertions catalogued next. The backend owner binds actual execution to these current inputs and the operator note; historical recipe text alone is never a current pass.

Configured-target decision: the supplied task-4 review finding, expressly retained by current recovery decision 1, establishes that no container or service observation policy is configured. No additional configured-target snapshot, inventory or external observation is required. The exclusive gate writer explains the eventual /configuredTargets/requirementsSatisfied=true on that basis, identifying the supplied finding as its source and making the absence of a configured policy explicit. This means there is no configured observation obligation to execute; it is not evidence of a container/service health check, deployment or external effect. It does not satisfy or waive the genuine binding integration preflight, installed-tool prerequisites or any native acceptance case. Denied av.yaml access is not a blocker for this decision and must not be bypassed.

~~~sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bun run test:coverage
./scripts/harness/validate.sh
bun test apps/desktop/tests/bridge-client.test.ts apps/desktop/tests/contracts.test.ts apps/desktop/tests/credits.test.ts apps/desktop/tests/dashboard.test.ts apps/desktop/tests/lifecycle.test.ts apps/desktop/tests/meter.test.ts scripts/coverage/accounting.test.ts scripts/coverage/desktop-foundation.test.ts scripts/coverage/gate.test.ts scripts/coverage/hygiene.test.ts scripts/coverage/terminal-outcomes.test.ts scripts/coverage/traffic-accounting.test.ts scripts/harness/equivalence.test.ts scripts/harness/traffic-targets.test.ts scripts/harness/coverage-reuse.test.ts
~~~

The saved task-11 desktop command, supplied in current recovery decision 4, is exactly:

~~~sh
bun test apps/desktop/tests/governor.test.ts apps/desktop/tests/governor-contract.test.ts apps/desktop/tests/traffic-scripts.test.ts apps/desktop/tests/traffic-scripts-contract.test.ts
~~~

Separately, the immutable verificationContract's desktop command retains its traffic-scripts-first order exactly:

~~~sh
bun test apps/desktop/tests/traffic-scripts.test.ts apps/desktop/tests/traffic-scripts-contract.test.ts apps/desktop/tests/governor.test.ts apps/desktop/tests/governor-contract.test.ts
~~~

Both source-specific commands remain required; neither replaces or reorders the other. Retain task-11's affected desktop and existing traceability checks below, plus the native suites at task-9's designated maintained entry points and their fixture modules. Do not introduce additional document/evidence validators.

~~~sh
bun test scripts/requirements/traceability-parser.test.ts scripts/requirements/traceability.test.ts
bun run desktop:typecheck
bun run desktop:check
bun run desktop:build
bun run desktop:smoke:tauri
~~~

The current recovery supplies the saved task-5/task-11 binding integration preflight handoff. Backend-contract records current observations for every item before prerequisite certification and dependent final gates:

| Binding preflight item | Required backend observation |
| --- | --- |
| Baseline and ancestry | Read back accepted HEAD 5578e69fc63ada7a73190f5681e7960da17969f7 and verify the integrated input ancestry. |
| Scoped workspace and index | Record actual scoped worktree/index state and ownership, preserving other Actors' work and excluding the denied av.yaml from inspection. |
| Immutable checks and environment note | Retain exact commands, verificationContract fields and the 2026-11-13T05:14:23.639Z operator note; rerun affected checks in the changed environment and retain observed integrationCheck. |
| Migrations | Record hashes and compare migrations 0001–0006 byte-for-byte with the accepted baseline. |
| Deliverable and fixture readiness | Verify non-ignored Git-visible output paths and writable isolated fixture/output locations; ignored raw outputs cannot be the sole handoff. |
| Toolchain and dependency readiness | Read current tool versions against Rust 1.95.0, Bun 1.4.4 and cargo-llvm-cov 0.6.21 with matching LLVM; verify offline Cargo dependency readiness and required installed real-tool availability. |
| Maintained coverage preflight | Execute the unchanged preflight command below and retain its actual result before collection. |

These are future backend observations, not passes established by this report repair. The supplied instructions are available; their prerequisite outcomes remain pending. Before collection, the assigned backend owner runs the binding integration preflight and the maintained coverage preflight:

~~~sh
bun scripts/coverage/run.ts --preflight-only
~~~

Use a mission-local ignored raw-output directory such as target/traffic-bun-scripts through maintained ARC_COVERAGE_OUTPUT_ROOT support. Collect coverage successfully once on the final input set with the unchanged coverage recipe. Saved task-11 also requires this post-collection check, retained by current recovery decision 4:

~~~sh
bun test scripts/coverage/evidence.test.ts
~~~

This check accompanies the six unchanged recipes; it replaces none of them. Validate the current-input reuse.json and native profile evidence, then supply its actual path as ARC_COVERAGE_RECEIPT to the unchanged harness command. Invalid, missing or stale receipts fail reuse rather than triggering silent recollection. Preserve historical baseline data and every production line in coverage accounting. No averaging, lower threshold, weakened tests, skips or copied historical coverage can pass this charter.

Each crate independently retains at least 80% production-line coverage: arc-core, arc-detector, arc-storage, arc-ipc, arc-daemon, arc-cli, arc-traffic, arc-desktop, arc-platform, arc-meter and arc-policy. gates.json retains integer covered/total production-line counts and measured percentages per crate, final identity, tool/native-profile bindings, collection count and evidence of successful harness receipt reuse.

The operator note at 2026-11-13T05:14:23.639Z changes the environment: Agent Valley now executes contract cargo fmt, cargo clippy, cargo test --workspace and the listed Bun tests on the integrated workspace after every code delivery and before review, exposing integrationCheck. Every code task must leave the whole workspace formatted, lint-clean, compiling and passing all existing tests, and run cargo test --workspace before delivering shared-code changes. Only a contract-named test file assigned to a later task may temporarily be absent; no other failure is excused. Backend owners must rerun affected checks under this current environment; predecessor checks cannot satisfy the note. Task-4 is report-only and runs no native gate.

Timeout authority is current recovery decision 2 and the current assignment's explicit timeout instruction. They supersede the earlier review's uncertainty about precedence: mission.verificationCheckTimeoutMs=7200000 is the framework's per-check timeout; verificationContract timeoutMs=120000 entries for cargo fmt, cargo clippy, cargo test --workspace and its four-file desktop Bun command are planning estimates and never terminate a check. Preserve both numeric fields unchanged, including timeoutMs=120000. Every replay or wrapper must allow at least 7200000 ms per check. Receipts retain the declared fields and actual framework outcomes. The preserved task-1 draft and read-only integration.json /design/deterministicAcceptance/mandatedDesktopCheck/timeoutPolicy agree with that wrapper allowance; the current instruction supplies its explicit authority. No command, field or mission state is changed by this report.

Preserve the existing 172800-second absolute mission deadline, maxParallel=3, maxRuns=400 and maxRetries=3 across resumes. The deadline/cancellation still governs whether a check can start. The final gate stage runs alone: other implementation, documentation, QA and unrelated gate processes are quiescent, with actual exclusivity evidence rather than an assertion based on task metadata.

No Playwright/browser E2E, Playwright-package/browser-binary downloads, GUI workaround or screen-based planning/implementation/individual review is allowed. Any relevant visual check belongs only to the one final-review pass after code verification using the existing permitted DevTools; it does not substitute for a missing native prerequisite. Native tests manage only their isolated owned fixture processes. No installed service restart, deployment, publication, messaging, spending or other external delivery is authorized here.

## Verification result pointers and evidence ownership

The following are the exact supplied verificationContract destinations and expected final values. This is a record of existing checks, not a new checker or authorization to set a pass flag. A pending/failed/unexecuted case never becomes true. No success-shaped gates.json or qa.json placeholder is created in task-4.

All integration pointers are rooted in .arc-verify/traffic-bun-scripts/integration.json:

| JSON pointer | Required final value / evidence |
| --- | --- |
| /baseline/acceptedHead | 5578e69fc63ada7a73190f5681e7960da17969f7 |
| /nativeAcceptance/platform | macos, observed native environment rather than plannedPlatform |
| /resolution/allRequiredCasesPassed | true, BS-R01–R03 |
| /nativeAcceptance/realToolsAndIndependentOutputsPassed | true, BS-N01–N04 |
| /nativeAcceptance/buildBothArrivalOrdersPassed | true, BS-N05 and BS-N06 separately |
| /nativeAcceptance/typecheckSingleWindowPassed | true, BS-N07 |
| /nativeAcceptance/mixedSharedBudgetPassed | true, BS-N08 and BS-N09 independently |
| /reconciliation/sqliteCliRegisteredBridge | true, BS-X01 across actual surfaces |
| /reconciliation/executionCountsOutcomesAndReleasedPermitsPassed | true, BS-X01 with native cases, counts/outcomes/releases |
| /semantics/allRequiredCasesPassed | true, BS-N10–N11 |
| /recovery/allRequiredCasesPassed | true, BS-N12–N14 |
| /compatibility/protocol1AndPeerReplacementPassed | true, BS-N13 and BS-C01 |
| /preservation/governorPressureMeterPassed | true, BS-P01 and BS-N11 |
| /preservation/migrations0001Through0006Match | true, six baseline byte comparisons |
| /diagnostics/cliAndRegisteredBridgeTruthful | true, BS-D01 and BS-X01 |
| /desktop/rustTypeScriptAgreementPassed | true, BS-D01 with maintained generation/compiler/runtime checks |
| /desktop/interactionAndQueueCauseCasesPassed | true, BS-D01 and complete D01–D16 |
| /traceability/targetRequirementIds | ["DET-003","DET-004","DET-008","DET-009","TRF-002","TRF-003"] in this order |
| /traceability/unrelatedRowsUnchangedAndCountsReconciled | true, BS-T01 |

All gate pointers are rooted in .arc-verify/traffic-bun-scripts/gates.json and owned by the exclusive backend gate task:

| JSON pointer | Required final value / evidence |
| --- | --- |
| /integrationPreflight/bindingPrerequisitesSatisfied | true, actual current immutable prerequisite comparison and rerun checks |
| /execution/finalGateRanAloneInMissionWorkspace | true, integrated-workspace exclusivity |
| /inputs/identityVerified | true, maintained current-input digest verification |
| /inputs/finalInputsUnchanged | true, frozen final gate inputs rechecked |
| /sixGates/recipesUnchanged | true, actual authorized saved-recipe comparison |
| /sixGates/allPassed | true, all six actual command receipts |
| /desktopChecks/allPassed | true, saved desktop tests and affected checks |
| /nativeAcceptance/allRequiredSuitesExecutedWithoutSkips | true, executed suite/case counts with no required skip |
| /configuredTargets/requirementsSatisfied | true, because the supplied task-4 review finding retained by current recovery decision 1 establishes no container or service observation policy is configured; record that source/basis, with no additional inventory or claimed external observation |
| /coverage/preflightPassed | true, current preflight |
| /coverage/successfulCollectionsForFinalInputs | 1 |
| /coverage/validatedReceiptReusedByHarness | true |
| /coverage/thresholdPercent | 80 |
| /coverage/nativeProfileEvidenceVerified | true |
| /coverage/liveEvidencePassed | true |
| /coverage/crates/arc-core/productionFloor80Passed | true |
| /coverage/crates/arc-detector/productionFloor80Passed | true |
| /coverage/crates/arc-storage/productionFloor80Passed | true |
| /coverage/crates/arc-ipc/productionFloor80Passed | true |
| /coverage/crates/arc-daemon/productionFloor80Passed | true |
| /coverage/crates/arc-cli/productionFloor80Passed | true |
| /coverage/crates/arc-traffic/productionFloor80Passed | true |
| /coverage/crates/arc-desktop/productionFloor80Passed | true |
| /coverage/crates/arc-platform/productionFloor80Passed | true |
| /coverage/crates/arc-meter/productionFloor80Passed | true |
| /coverage/crates/arc-policy/productionFloor80Passed | true |
| /evidence/fiveFileBudgetSatisfied | true, canonical inventory and aggregate UTF-8 bytes |

Independent qa.json must be at least 200 bytes and contain /skill="oma-qa", /reviewComplete=true, /inputsMatchFinalGates=true, /requiredEvidenceAssessmentComplete=true and /findingsReproducibilityAssessed=true from actual read-only final-input review. A reproducible product defect must be reported accurately; a complete assessment is not permission to suppress it.

Existing file assertions remain: contract.md is at least 200 bytes and contains the accepted HEAD and BUILD/TEST/LINT/TYPECHECK; final.md is at least 200 bytes and contains "Supported Bun-script slice only" and all six target requirement IDs; docs/requirements/traceability.md is at least 200 bytes and retains all six IDs. These textual assertions do not replace executable acceptance or authorize broad traceability edits.

## Canonical files, compact receipts and traceability boundary

Exactly five canonical non-ignored Git-visible deliverables live under .arc-verify/traffic-bun-scripts:

| File | Writer sequence / content |
| --- | --- |
| contract.md | task-1 technical evidence -> task-4 Chief decision freeze -> task-5 concrete implementation signatures; freeze all final bytes before coverage and final gates. This file enters maintained coverage input identity. |
| integration.json | task-2 design evidence remains intact -> backend-native integration observations, incorporating actual backend handoffs and later traceability evidence through its single writer. Read-only for task-4. |
| gates.json | Exclusive backend-contract final-gate writer; one source-bound final identity, command/coverage/native-profile/exclusivity receipts. Not yet produced. |
| qa.json | Independent oma-qa reviewer only; final-input evidence assessment and reproducible findings. Not yet produced. |
| final.md | task-3 value assessment -> task-4 Chief strategy -> Chief final business/technical judgment from actual results. No false completion or measured-benefit claims. |

Their collective maximum is 300000 UTF-8 bytes; this is one shared budget, not permission to grow each file to that size. Keep single-source inventories and compact observations, not duplicate full logs, source catalogs or receipt copies. The three files present at intake total 109427 bytes. Future writers must account for task-4 additions and leave room for actual gates and QA. Establishing this inventory does not let Chief write another Actor's gate/QA report.

gates.json/inputs is the single final input identity with maintained digest and relevant source/test/configuration/manifest/lockfile/frozen-contract bindings. Other reports reference it. Generated integration/gates/QA/final reports do not become coverage inputs. Record exact argv/cwd, tool identities, actual exit, counts, times/order, source identity, independent output/outcome observations and raw path/hash compactly in the assigned canonical report. Store raw logs/build outputs locally under ignored target paths; they cannot be the sole handoff. Across isolated workspaces a required file must travel as non-ignored Git-visible changes. Hashing an unavailable ignored artifact alone delivers no reviewable result.

Do not introduce any document/evidence validator or sixth tracked report, including skill-default plan/result/task-board files. The user explicitly confines these outputs to the canonical files and preserves the existing saved plan. Use existing coverage identity/reuse, contract generation, requirement checks and deterministic/native fixtures; never change their acceptance to fit a report.

The baseline six target statuses read in task-4 are DET-003 Partial, DET-004 Partial, DET-008 Pending/Unverified, DET-009 Pending/Unverified, TRF-002 Partial and TRF-003 Partial. This task changes no traceability row or count. Backend-execution later updates only those rows and consequent counts against delivered evidence, preserving existing acceptance-test IDs and unrelated rows byte-for-byte. In particular, Bun-script evidence cannot complete DET-003's nine-family catalog or DET-009's broader workspace analysis. Unsupported breadth and whole-phase completion remain outstanding unless separately evidenced within the original task.

## Remaining prerequisites, authorization and task-4 evidence

| Item / current finding | Owner and required next evidence |
| --- | --- |
| Current recovery supplies saved task-5/task-11 instructions and the binding preflight handoff; the version-1 verificationContract and operator note are also available. | Backend-contract executes the baseline/ancestry, scoped workspace/index, immutable-command/note, migration-hash, output-visibility, writable-fixture, pinned-tool, offline-dependency and maintained-coverage-preflight obligations above. Record their actual outcomes before certification or dependent final gates; the supplied text alone proves no prerequisite pass. No predecessor substitute, changed command or av.yaml bypass. |
| The supplied task-4 review finding retained by current recovery decision 1 establishes no container or service observation policy is configured. | No target snapshot or new inventory is needed. The gate writer explains /configuredTargets/requirementsSatisfied=true from that supplied fact, without claiming observations were performed. Real tool, integration-preflight and native-case evidence remains mandatory. |
| Initial unscoped Git status/diff emitted "av.yaml: Operation not permitted" despite exit 0. | File remains unread and untouched; scoped checks explicitly exclude it. The denial is not a configured-target blocker: the no-policy fact comes from the supplied review input, not an inference about this file. No alternate read, Git-object extraction, GUI or escalation bypass. |
| Current environment-change note requires whole-workspace checks after code deliveries; no current code delivery/check has occurred in task-4. | Backend owners rerun affected checks in the post-note environment and retain framework integrationCheck plus their required workspace checks. Earlier report reviews are not those results. |
| Finite metadata caps, snapshot/revalidation signatures, exact new wire/DTO names, defaults and native test selectors remain implementation prerequisites. | task-5 backend-contract publishes concrete Git-visible interfaces without weakening any case, preserved behavior or recipe; downstream owners implement only after delivery. |
| Rust 1.95.0, Bun 1.4.4, cargo-llvm-cov 0.6.21/matching LLVM, offline Cargo dependencies, installed real tools, native suites, eleven coverage floors, final input identity and exclusivity are unverified for this slice. | Assigned backend owners provide current preflight and real command/output evidence. A missing prerequisite is reported as failed/unmet; no skip, downloaded browser, invented credential or historical success substitutes. |
| No container/service observation policy is configured, and no external delivery is authorized for this assignment. | Perform no live restart/deploy/publication/message/spending. There is no configured-target external action to satisfy here; preserve the original assignment and manage only owned native fixture processes in their assigned later tasks. |

Current recovery readback confirms HEAD 5578e69fc63ada7a73190f5681e7960da17969f7 and tree 888ddd6cbe9d02611b87ed98c8529882d8a0778b. Initial status found three staged dependency additions, with unstaged repairs in the two owned reports; the older staged report bytes did not match disk. Those repairs were resumed. Intake SHA-256/byte counts above identify the preserved original bodies and read-only design artifact. Earlier task-4 inspection resolved all six supplied task-1/2/3 snapshot/worker refs to their respective snapshot commits (66e710abfd83ef0e4ead8339c95759bb65e41ac6, f6fef88d79e9eb4fac35e91da6171f33c1acd05c and 288668a7f6d30ff020be17709032c8911d69c109); that ref observation is retained as historical evidence, not a new ref check in this repair.

Skills used: oma-pm and oma-coordination, with shared execution/collaboration/result policies and always-on language guidance. The existing saved assignment makes new planning artifacts and nested dispatch unnecessary; the explicit two-file scope supersedes those skills' default output conventions. Serena was unavailable; named report/policy reads and plain-document searches were used, with no product code search or provider installation. A lightweight memory search yielded no task-specific basis; all decisions use supplied inputs and current reports.

Task-4 attempt-1 inspection evidence is retained: owned-path whitespace, non-ignored destinations, original-body suffix and unchanged integration comparisons passed then, without resolving the two frozen-decision contradictions. Current recovery uses the explicit current timeout authority, preserves the no-policy finding and reconciles the supplied task-5/task-11 handoff plus task-9 entry points. Scoped report checks and final staged-byte comparisons are recorded in final.md and the task handoff; no product or native gate is claimed.

Task-4 acceptance: [x] actual three dependency artifacts consumed and preserved; [x] original charter/goalBrief and all verification/preservation obligations retained; [x] strategy, rejected alternatives, ownership and compact case/pointer matrix recorded; [x] preliminary final-report strategy and deferred commercial/retention work explained; [x] supplied current instructions, pending prerequisite observations, authorization and measured-evidence limits stated. Final staged Git/byte/whitespace readback is returned in the handoff; no software gate, product implementation, independent final QA or native acceptance is claimed.

---

## Preserved task-1 technical report

The following original task-1 body is retained verbatim, including its then-current draft/provenance statements. The Chief decision sections above establish the current behavioral freeze and ownership sequence. Current recovery supplies the saved task-5/task-11 obligations that were unavailable in task-1's earlier payload and expressly establishes the framework timeout precedence and two-hour wrapper allowance, preserving both numeric fields. The matching timeout wording in this original body and read-only task-2 handoff remains consistent with that current authority. Earlier requests to obtain configured-target requirements do not require another inventory: the retained review finding establishes no container/service observation policy. Genuine integration-preflight, tool and acceptance observations remain required and pending.

# Bun package-script Traffic contract — technical draft

Status: task-1 technical report complete; proposed interfaces, not frozen. Task-5 must freeze concrete interfaces after integrating the assigned Director and backend handoffs. Date: 2026-11-13. Author: Technical Director. Skill: `oma-architecture` (Recommendation Mode, with an explicit alternative comparison). Supported Bun-script slice only; this document records no implementation or gate pass.

## Scope and source provenance

The architecture problem is to recognize eligible one-shot Bun package-script graphs without running them, then admit the unchanged native invocation through the existing Governor, Traffic, pressure and Meter machinery. The target classes are BUILD, TEST, LINT and a distinct TYPECHECK window sharing one global resource budget. Requirement scope is DET-003/004/008/009 and TRF-002/003; this slice does not complete their broader unsupported language/workspace coverage.

| Provenance | Exact value / observation |
| --- | --- |
| Accepted committed baseline (`baselineHead`) | `5578e69fc63ada7a73190f5681e7960da17969f7` |
| Accepted product tree (`baselineTree`) | `888ddd6cbe9d02611b87ed98c8529882d8a0778b` |
| Actor snapshot commit and inspected local HEAD | `66e710abfd83ef0e4ead8339c95759bb65e41ac6` |
| Snapshot input tree and inspected HEAD tree | `888ddd6cbe9d02611b87ed98c8529882d8a0778b` |
| Actor branch | `av-task/69a59d52a042fc13-7afaa346b4bf92bf-1` |
| Supplied snapshot ref | `refs/agent-valley/task-evidence/69a59d52a042fc13-7afaa346b4bf92bf-1/snapshot`; local resolution failed (exit 128). The commit above resolved independently. |
| Supplied worker ref | `refs/agent-valley/task-evidence/69a59d52a042fc13-7afaa346b4bf92bf-1/worker`; not independently verified. |

The source observations below are scoped to this Actor's snapshot/local HEAD. Its tree equals the accepted baseline tree; no source overlay was observed. Initial status and staged/unstaged diff checks excluding the denied `av.yaml` were empty. This new contract is an Actor-local task edit, absent from those commits; its Git-visible delivery and later integrated bytes need review in the destination checkout. No claim is made about another workspace's HEAD or tracking state.

Serena discovery returned no callable tools. Only already identified source files were read; native content search was confined to non-code contract text. Unscoped `git status --short` reported `av.yaml: Operation not permitted` despite exit 0. Its contents were not read through another route. The supplied advice also records this denial; it is not evidence that configured targets or prerequisites are absent. No AV invocation, provider installation, mission-state mutation, browser/GUI interaction, build, test, linter, typecheck or coverage collection occurred.

## Existing seams and diagnosed gaps

Paths and line anchors refer to the inspected revision above, not future implementation.

| Source | Observed fact / resulting obligation |
| --- | --- |
| `crates/arc-detector/src/lib.rs:1,21,37` | Detector is pure; restricted parsing and leaf aggregation already exist. Preserve this boundary and old classification behavior. |
| `crates/arc-detector/src/argv.rs:10,37`; `rules.rs:129` | Native argv recognizes direct Bun tests; package scripts remain unresolved. Add contextual resolution at the execution boundary. |
| `crates/arc-detector/src/budget.rs:3`; `parser.rs:63,94` | Deterministic work/token/invocation/depth accounting and compound parsing exist. Extend their accounting across the whole script graph. |
| `package.json:9`; `apps/desktop/package.json:7` | Root desktop scripts delegate through `bun run --cwd apps/desktop`; desktop build is `vite build`, typecheck is two sequential `tsc --noEmit` commands. |
| `crates/arc-detector/src/types.rs:4,140`; `crates/arc-traffic/src/lib.rs:28,41,111,178` | Closed class/reason validation and three class windows/counters need coordinated TYPECHECK support and eligible-script admission. |
| `crates/arc-cli/src/exec.rs:47,108,169`; `exec/governor.rs:30,80` | CLI classifies before admission and owns the single original native spawn. Governor eligibility currently has a closed direct-command reason list. |
| `crates/arc-ipc/src/traffic.rs:15,23,139`; `crates/arc-ipc/src/policy.rs:13,53` | Strict existing capability/reply DTOs cannot silently acquire new script vocabulary or policy fields. |
| `crates/arc-daemon/src/connection/legacy_projection.rs:3` | Legacy snapshot projection currently handles BUN_TEST only. Every new class/reason needs a safe legacy projection. |
| `crates/arc-storage/src/jobs/traffic/store.rs:114`; `migrations/0001_phase0.sql:39` | Active accounting rejects other classes; jobs.job_class is unrestricted TEXT. Update codecs/accounting, without a migration merely for a new enum. |
| `apps/desktop/src-tauri/src/dto.rs:46,60`; `crates/arc-ipc/src/jobs/types.rs:130,156` | Job detail drops classification; job summary lacks class/permit facts. Current projection cannot prove the requested reconciliation. |
| `apps/desktop/src/bridge/policy.types.ts:47`; `dashboard/policy-messages.ts:31` | Desktop exposes three class policies and generic class-slot copy. Extend negotiated diagnostics using actual queue causes. |
| `crates/arc-daemon/tests/traffic_cli/measurement.rs:66`; `apps/desktop/src-tauri/tests/governor.rs:1` | Existing synchronized native/registered-router fixtures are reusable mechanisms, not current Bun-script acceptance. |
| `scripts/coverage/config.ts:2,5,26` | Maintained pins, eleven crates and frozen-input identity exist; the new frozen contract is not yet registered there. Backend contract owner must coordinate this input change. |

`docs/decisions/README.md` and the proposed runtime/state-ownership ADR were inspected as prior context. Those AV harness decisions and historical Governor/Bun contracts are left unchanged; this task proposes no replacement ADR or new service.

## Decision and alternatives

Select bounded package metadata acquisition at the CLI boundary plus contextual traversal using the maintained pure parser. Retain Rust/Tokio/rusqlite/serde and the pinned Bun/Tauri/React stack. Reuse existing JSON support, queue, launch fences, witness, lineage and bridge-generation modules. No new runtime dependency is justified by the observed gap.

| Option | Implementation and team cost | Operational and future-change cost | Judgment |
| --- | --- | --- | --- |
| Extend maintained parser with explicit metadata snapshots | Localized reader/resolver and budget tests; existing parser semantics remain shared. Bounded support must be stated precisely. | No new process or service; pinned Bun semantics require focused upkeep. Pure fixtures remain deterministic. | Selected draft direction. |
| Adopt a general shell engine/interpreter | Integrate a larger grammar, shell/runtime compatibility and dependency surface; evaluation is unnecessary for classification. | More dependency maintenance and platform semantics; larger analysis surface still cannot safely resolve arbitrary dynamic scripts without executing them. | Reject for this slice; reconsider only for a demonstrated supported-case gap. |

This is a qualitative cost comparison, not a latency, price or resource-saving measurement. Conservative bypass can leave some workloads unscheduled; that is preferable to inventing a class from a script name. No measured model/vendor comparison supports changing the operator-selected Chief route.

## Metadata reader and pure resolver boundary

The CLI adapter captures the caller's cwd and reads only package metadata needed by the literal invocation and reachable nested `bun run` calls. It supplies immutable, directory-keyed script maps and bounded lookup failures to pure analysis. Filesystem lookups, regular-file checks and package identity normalization belong to that adapter; parsing, literal argv/cwd propagation, cycle detection and class aggregation belong to the detector. If a nested cwd needs another manifest, acquire it through the same bounded adapter context, not hidden filesystem I/O in the parser. No command execution, tool lookup subprocess, shell expansion, package installation or project-wide scanning is part of classification.

Use fresh bounded reads for each top-level resolution, with only per-resolution memoization of an already read package. Read bytes rather than trusting mtime or size as a cache identity. Follow only the bounded lookup/ancestor rules validated against the pinned Bun runtime; resolve explicit cwd relative to the invoking script context. Reject unavailable, malformed or oversized metadata into observe/bypass. Bound and diagnose file identity/lookup races; the snapshot describes bytes observed at admission, not an atomic lock over Bun's later filesystem reads. Preserve invocation semantics instead of executing a rewritten snapshot. Task-5 must freeze the read/revalidation boundary and its deterministic race tests.

Freeze explicit finite limits for: bytes per manifest, total metadata bytes, distinct manifests, candidate/ancestor lookups, total retained script bytes, graph depth, visited invocations/edges, decoded tokens and aggregate work. Charge before allocation/traversal, use checked arithmetic, and include lifecycle bodies, nested shells, argument propagation, failed lookups and repeated references. Limit counters must not reset per nested script or per manifest; cycle identity includes normalized package identity and script name, with lifecycle edges included. No unbounded symlink/path traversal or blocking nonregular metadata read is acceptable.

The maintained parser currently caps input at 16,384 bytes, tokens at 1,024, invocations at 128, depth at 8 and work at 65,536. Retain these protections and apply a shared graph budget; they are not by themselves metadata-read limits. Exact additional metadata caps and their public stats/reason representation remain a task-5 freeze item, not invented implementation constants in this report.

Resolution must inspect script contents, including applicable pre/post lifecycle scripts under the pinned runtime. Forwarded arguments and `--` boundaries retain token meaning, order and quoting; options that change class, cwd or one-shot behavior must affect analysis. Unsupported argument semantics bypass, rather than concatenating data into shell text. Literal same-class chains may resolve; an unknown or unsupported reachable leaf cannot be discarded. Dynamic expansion, cycles, unresolved graphs, mixed local classes/locality, unsupported commands and watch/hot/workspace fan-out remain non-blocking observe/bypass with no local reservation. Remote-only graphs remain reservation-free.

| Required graph | Classification obligation |
| --- | --- |
| Supported literal `bun build` or `vite build` leaves | BUILD, including root `desktop:build` -> explicit desktop cwd -> `vite build`. |
| Supported one-shot `bun test` leaves | TEST, reusing direct-test option validation where applicable. |
| Supported `biome check` leaves | LINT, including root `desktop:check`; validate options rather than using the script name. |
| Supported `tsc --noEmit` leaves | TYPECHECK, including both commands in desktop `typecheck` and the root delegation. Emitting or watch variants are outside this supported leaf. |
| Maintained remote leaves only | Preserve remote classification/bypass; allocate no local permits. |

Deterministic fixtures must cover misleading names, explicit cwd/option positions, missing/invalid metadata, nested and lifecycle graphs, forwarded arguments, same-class chains, dynamic/mixed/unsupported/watch/hot/fan-out graphs, cycles and every limit boundary. Invalidation cases include replacement, deletion, same-size preserved-mtime edits and nested/lifecycle metadata changes. Analysis tests must prove no script body executes. Native tests separately establish which pinned Bun argument/lifecycle behaviors the adapter models.

## Interfaces required before consumers implement

The following are concrete handoff obligations, not declarations that these APIs exist. Task-5 freezes exact Rust signatures, enum/reason spellings, wire names, defaults, limits and fixture selectors with the assigned writers.

| Interface / writer | Required inputs, outputs and compatibility |
| --- | --- |
| Metadata adapter + resolver / backend-execution | Original native argv, caller cwd, immutable package/script snapshot and one resolution budget -> validated Classification, deterministic aggregate stats and bounded unresolved/limit reason. Metadata provenance stays local; command/path/env data is not added to IPC. Preserve old pure `classify`/`classify_argv` behavior for legacy callers; richer contextual analysis is an explicit entry point. |
| Shared classification / backend-contract, consumed by resolver | Add Typecheck with wire TYPECHECK; publish the eligible Bun/Vite/Biome/tsc and resolved-script reason combinations, locality/uncertainty checks and conservative legacy mapping. A same-class compound needs an explicitly eligible resolved-graph result; admitting all existing compound reasons would widen unrelated support. |
| Traffic and policy / backend-contract | Add `TrafficClass::Typecheck`, `Policy.typecheck`, `Active.typecheck`, validated window/reservation defaults and effective `limits.typecheck`. Include TYPECHECK in admission, pressure-policy transformations, serialization, recovery and integer overflow checks. Expose richer policy limits only across an opted-in boundary. |
| Negotiated admission / backend-contract | Publish a distinct Bun-script capability/probe and job operation, strict action/result DTOs, payload limits, owner validation, unsupported/error behavior and routing through existing governed admission/launch helpers. Existing traffic.v1, traffic.bun.v1 and Governor capability responses retain their vocabulary/shapes. The new operation must not become a Governor bypass. |
| Durable state / backend-contract using oma-db | Validated new class/reasons round-trip through existing job/permit codecs and writer transactions; active snapshots count every root reservation and share one resource sum. Preserve job ID, generation, outcome, witness, lineage and release idempotency. |
| Negotiated diagnostics / backend-contract -> backend-native | Publish authenticated bounded read DTOs keyed by stable job ID: truthful classification; queued/admitted/bypassed/running/terminal or unavailable state; class-window/FIFO/CPU/memory/pressure cause; reservation/root/generation/release facts or explicit unavailability; timestamps/revisions/freshness. Reuse lifecycle, policy and Meter data rather than inventing parallel accounting. |
| Registered bridge + TS / backend-native | Registered production Tauri handler, Rust projection, generated/validated TypeScript agreement and existing CLI consumers use those DTOs. Expose TYPECHECK and actual queue causes while preserving Governor controls and keyboard/focus behavior. No desktop database connection or dashboard redesign. |

## Admission, persistence and compatibility preservation

TYPECHECK is a separate class window under the same FIFO and checked CPU-millicore/memory-byte budget used by BUILD/TEST/LINT. Cargo and eligible Bun-script builds consume the same BUILD window. A graph is one original invocation and must not reserve per resolved leaf. Prove class-window contention independently from cross-class CPU-budget and memory-budget contention; resource estimates are reservations, not OS utilization caps.

Reuse the single durable writer and transactional queue/grant/release path. Active accounting must include TYPECHECK rather than ignore unknown rows or reset totals. Retain root permits for live descendants as required; nested execution must neither double-reserve nor double-count Meter totals. Preserve durable Governor OFF/ON, pressure freshness/launch enforcement, actual outcome accounting, cancellation and existing launch fences. STARTED requires a real spawn. Exit 17, signals, spawn failure and uncertain outcome remain distinct.

Preserve all migration 0001–0006 bytes against the accepted baseline. Unrestricted TEXT for job_class does not require a migration; add one only for a demonstrated durable-data need coordinated by backend-contract. Protocol compatibility does not promise an older binary can read new class/reason values in the same database. Document and test that distinction without changing stored classifications to satisfy legacy readers.

An old daemon must reject the new job operation before attempting to decode a new class/reason, including replacement between capability probe and enqueue. New daemon/old client, new client/old daemon and replacement-peer cases need deterministic coverage. Project every new class/reason on old lifecycle, status and policy surfaces to valid legacy values/shapes; audit all reply paths, not only the current BUN_TEST projection. Keep full stored facts for new readers.

Unsupported-peer fallback is permitted only when no earlier possibly committed submission is being erased. A lost enqueue acknowledgment followed by an unsupported replacement peer is not proof of rollback. Retain original job identity, witness, continuity and uncertainty across retry/recovery; never resubmit a new job or spawn a second child to recover metadata. Preserve bounded outage fail-open and its exactly-once constraints. Always execute the original Bun argv once via the existing CLI child path, retaining cwd, environment, stdin/stdout/stderr, signal/terminal behavior and native exit semantics; do not flatten the graph into replacement child commands.

## Ownership and evidence obligations

Task-1 owns only this file; product code is read-only. Task-5 later owns contract integration/freeze. Shared contract/manifests/lockfiles/registries have one writer, backend-contract; its concrete delivered artifact must precede consumer handoff. Backend-execution owns metadata/resolver/CLI changes and later only affected traceability rows/counts. Backend-native owns registered desktop consumers, deterministic interaction tests, native acceptance and integrated repairs within its assigned paths. Implementation, tests, repairs and native gates remain backend-engineer work using oma-backend, with oma-db/oma-frontend where affected. Exact-path overlap is a Chief coordination issue, not permission to edit another Actor's files. No assignment/mission graph is changed by this draft.

Independent reviewer uses oma-qa after backend delivery, reads product/gate inputs without modification, writes only qa.json and does not rerun unchanged expensive gates. Chief owns decision/final reporting. Commercial proposals and retention experiments are outside this charter.

Native acceptance must use the isolated production daemon, real installed pinned tools, actual CLI and registered production desktop router. Extend maintained fixtures; no new browser E2E dependency or document/evidence validator. A command stub or historical receipt cannot establish real Bun-script success.

| Obligation | Evidence to retain on final inputs |
| --- | --- |
| Real tools | Actual Bun build, Vite build, Bun test, Biome check and tsc --noEmit runs; independent built artifact/test-body/diagnostic checks and native outcomes, tool identities/versions, exact argv/cwd and process identities. Include repository desktop build/typecheck resolution and required desktop checks. |
| BUILD contention | Cargo first/Bun-script second and Bun-script first/Cargo second under BUILD window 1. Synchronize entry/release; second is visibly queued with zero child/body starts while first is live; each then executes exactly once. Use independent directories/output roots so tool locks cannot explain serialization. |
| TYPECHECK and mixed classes | Two actual typechecks obey TYPECHECK window 1. Mixed classes independently exercise shared CPU and memory budgets, without hiding the constraint behind a class-window limit. |
| Three-surface reconciliation | Match stable job IDs and classifications, queue causes, child/body counts, exit outcomes, permit roots/generations/resources and final released permits through SQLite, CLI and registered bridge. Distinguish recorded from fresh observations. |
| Invocation semantics | Exit 17; cwd/env/forwarded argv; stdin/stdout/stderr; signals/terminal semantics; nested lineage and reservation/Meter ownership; cancellation before enqueue, while queued and during execution. |
| Recovery and preservation | Lost acknowledgments, daemon replacement/restart and bounded outage fail-open; zero duplicate execution or leaked permits. Preserve Governor controls, pressure enforcement, Meter and immutable migrations. |
| Desktop | Rust/TS agreement plus deterministic labeled class/wait/outcome text, keyboard/focus, meaningful status announcements and unavailable/stale recovery. Proposed labels such as Typecheck and Waiting for another typecheck require actual class/cause evidence. No rendered/accessibility-field or user-study pass is inferred. |

## Retained gate recipes and integration preflight

Task-11 is the designated exclusive backend-owned final-gate task referenced by this assignment. Its full saved instructions, binding integration preflight, configured-target requirements and any later operator notes were not supplied in this task payload. Chief/backend-contract must reconcile them before task-5 freeze and final execution; missing text or denied av.yaml does not waive an obligation. Re-run affected prerequisite checks after any operator note reports an environment change. The supplied structured verification contract is current authority; an empty verifyCommand removes nothing.

For reconciliation with task-11, retain these six exact recipes from `.arc-verify/governor/contract.md:181–209` (historical recipe reference, not this mission's success evidence):

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bun run test:coverage
./scripts/harness/validate.sh
bun test apps/desktop/tests/bridge-client.test.ts apps/desktop/tests/contracts.test.ts apps/desktop/tests/credits.test.ts apps/desktop/tests/dashboard.test.ts apps/desktop/tests/lifecycle.test.ts apps/desktop/tests/meter.test.ts scripts/coverage/accounting.test.ts scripts/coverage/desktop-foundation.test.ts scripts/coverage/gate.test.ts scripts/coverage/hygiene.test.ts scripts/coverage/terminal-outcomes.test.ts scripts/coverage/traffic-accounting.test.ts scripts/harness/equivalence.test.ts scripts/harness/traffic-targets.test.ts scripts/harness/coverage-reuse.test.ts
```

The supplied current desktop verification command is:

```sh
bun test apps/desktop/tests/traffic-scripts.test.ts apps/desktop/tests/traffic-scripts-contract.test.ts apps/desktop/tests/governor.test.ts apps/desktop/tests/governor-contract.test.ts
```

Also retain affected desktop/typecheck/check/build/debug-no-bundle smoke and existing traceability checks for reconciliation with task-11:

```sh
bun test scripts/requirements/traceability-parser.test.ts scripts/requirements/traceability.test.ts
bun run desktop:typecheck
bun run desktop:check
bun run desktop:build
bun run desktop:smoke:tauri
```

Do not alter the operator's verification argv or replace required native suites with focused tests. Replay/wrappers must allow at least 7,200,000 ms per check; smaller timeoutMs fields are planning estimates. Final gates run alone in the integrated mission workspace, with implementation/docs/QA writers stopped and final inputs frozen. Retain the existing absolute 172,800-second mission budget, maxParallel 3, maxRuns 400 and maxRetries 3; resuming does not reset them. This report task executes none of these commands.

Use the binding integration preflight and maintained `bun scripts/coverage/run.ts --preflight-only` before collection. Register this contract as a frozen coverage input under the assigned backend writer; generated integration/gates/QA/final reports must not enter input identity. Collect coverage successfully once per final input set, validate current-input/native-profile evidence and pass its reuse.json through ARC_COVERAGE_RECEIPT to the unchanged harness command. A stale/missing receipt fails reuse, rather than silently recollecting or borrowing historical output. Preserve eleven separate 80% production-line floors: arc-core, arc-detector, arc-storage, arc-ipc, arc-daemon, arc-cli, arc-traffic, arc-desktop, arc-platform, arc-meter and arc-policy. No averaging, weakened tests, skip flags or production exclusions.

Keep exactly five canonical non-ignored handoff files under `.arc-verify/traffic-bun-scripts/`: contract.md, integration.json, gates.json, qa.json and final.md, collectively at most 300,000 UTF-8 bytes. Later owners write only their assigned files; task-1 creates no placeholder passes. Keep compact final input identities, exact commands/outcomes, integer coverage counts and native observations in those files. Ignored target logs may support local diagnostics but cannot be the sole cross-workspace evidence. Preserve historical receipts. Update only the six affected traceability rows and consequent counts; do not expand document scope or add validators.

## Task-1 check evidence, assumptions and remaining risks

Read-only inspection commands: `git rev-parse HEAD HEAD^{tree} 5578e69fc63ada7a73190f5681e7960da17969f7^{tree} 66e710abfd83ef0e4ead8339c95759bb65e41ac6^{tree}` exited 0 with the provenance values above. Scoped `git status --short -- . ':!av.yaml'`, `git diff --stat -- . ':!av.yaml'` and the corresponding cached diff exited 0 with no initial changes. `git check-ignore -v -- .arc-verify/traffic-bun-scripts/contract.md` exited 1 with no match (non-ignored path). Named-file reads confirmed the source seams and historical recipes above. `docs/decisions/index.md` was absent; the existing README index was read instead. Named snapshot-ref resolution failed; no ref was created or substituted. These are inspection outcomes, not software acceptance checks.

Assumptions requiring task-5/native evidence: pinned Bun cwd/lookup/lifecycle/argument semantics can be represented conservatively; required tools are available in the current macOS environment; the new negotiated DTOs can reuse maintained launch/writer/accounting helpers; current saved preflight and target obligations can be obtained by their assigned owner. None is certified by this draft. Fresh metadata can still change after observation, so the documented snapshot/revalidation boundary must not imply atomicity. Legacy projection omissions and lost-ack peer replacement are specific integration risks. Overlapping shared-file ownership must be resolved before edits, not through concurrent manifest writes.

Only this contract is the material deliverable. No runtime interface changed. All implementation, deterministic/native acceptance, real-tool outputs, gate/coverage/preflight results, traceability completion and independent QA remain pending with their assigned owners. Predecessor Governor and direct-Bun receipts establish no pass for the current Bun-script slice. The technical strategy is ready for integration, and the contract remains a draft until task-5 freezes the concrete handoffs and reconciled obligations.
