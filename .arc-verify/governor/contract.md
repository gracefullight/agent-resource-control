# Governor pause/resume — frozen implementation and verification contract

Status: task-5 shared-contract delivery; runtime, client behavior, native acceptance, final gates and QA remain pending. Scope: **Governor slice only**. Accepted baseline: `08e7e526cce7e03ca9264afc926ed2e878dfde6d`, tree `d3cbd610653d0b78adeb2ffc26a9cab1790e877f`. Freeze cycle: `governor-2026-11-13`. This file is the material task-5 report and the immutable coverage input after task-5 delivery. Consumers implement these interfaces; they do not edit acceptance, saved tasks or the mission graph.

## Inputs, authority and refinements

The operator's saved assignment and verification contract govern this work. The task-4 decision record at [final.md](final.md) retains the complete original charter and five success criteria. Consumed dependencies:

| Input | SHA-256 at task-5 entry | Use |
| --- | --- | --- |
| Task-1 draft at this path | `f4d31172ff5e995068da837da0d018f87880e337547c05917672750ecf02e736` | Existing writer, lifecycle, compatibility and original migration analysis; ownership expressly transferred to task-5. Draft retained locally at `target/governor/task5/task1-draft.md`. |
| [Governor experience](../../docs/requirements/governor-experience.md) | `5947e445ef500728e8f9f9f92955bd15fff1388d3842aff83a6ae5fed906bb28` | G01–G13, dashboard placement, pending/rejection/uncertainty and accessibility. Unchanged by task-5. |
| [Governor value](../../docs/requirements/governor-value.md) | `c68ed1e60fcd7c3a239a0795ecea24eb93d5752f5c0b1d7e308ae6aed8fe3c25` | Reliability first; no new commercial experiment/distribution. Unchanged by task-5. |
| [Authoritative SRS](../../docs/requirements/ARC_SRS_2026-11-06.md) | `a8ef945251f9f3fd7c3e337d4efc77ff9914f95114212ca9e6c90ae2ce3e3d9e` | §§7.3, 9.2, 9.9, 15.5, 15.7 and complete amendment-13 inventory. |

Task-5 resolves the draft's open choices: one `policy_states` row; revision-bound mutation IDs with bounded receipts; two additive metadata tables rather than a rewritten Traffic schema; explicit current-client wrappers; receipt-aware readback; pinned test-only ts-rs; existing identity/reuse helpers. The readback refinement is essential: a current-state read alone cannot identify a lost no-op acknowledgment or distinguish an old committed receipt from the current switch. No production handler is enabled by this delivery.

The existing Rust/Tokio/rusqlite/serde and Tauri/React/Base UI/Bun stack remains. No new runtime, ORM, scheduler service, authentication system or state framework is introduced. Same-user transport ownership and independent native job-identity verification remain the existing authentication mechanisms; JWT/password defaults do not apply to these local sockets.

## Ownership and actual handoffs

Task-5 is the sole writer of the shared surfaces in this wave. Tasks 6 and 7 start only after this Git-visible dependency is delivered. Task-8 receives shared implementation/manifest ownership through its actual dependency; this frozen acceptance contract does not change.

| Paths/surface | Task-5 delivery and next owner |
| --- | --- |
| `.arc-verify/governor/contract.md` | Frozen here; consumers read only. Task-4's `final.md` and both Director docs remain untouched. |
| `Cargo.toml`, `Cargo.lock`, `crates/arc-ipc/Cargo.toml` | Exact dev-only exporter pin and corresponding lock update. Task-5 retains manifests until task-8's dependency handoff. Root/desktop `package.json` and `bun.lock` are unchanged. |
| `crates/arc-ipc/src/lib.rs`, `src/governor.rs`, `src/governor/{switch,execution,wire,typescript}.rs`, `crates/arc-ipc/tests/governor.rs` | Compiled shared public DTOs, validators and test-only generator. Retained shared contract surfaces; task-8 receives implementation repair ownership, not authority to weaken this contract. |
| `crates/arc-storage/src/lib.rs`, `src/migration.rs`, `migrations/0006_governor.sql` | Public module and additive schema registration. Migrations 0001–0005 remain byte-identical. Retained shared surfaces until task-8. |
| `crates/arc-storage/src/governor.rs` | Real authoritative `get_switch(&Connection) -> JobResult<GovernorSwitchState>`; task-6 receives module implementation ownership and extends it with the transactional setter/receipts. Public getter and DTO compatibility remain. |
| `crates/arc-storage/tests/governor_migration.rs` and migration-specific assertions in `tests/{managed_exec_migration,memory_pressure}.rs`, `src/{migration/tests/acceptance,jobs/tests/migration,job_meter/tests/migration}.rs` | Additive schema/read/rollback tests; existing current-schema expectations advance from 5 to 6. No assertion of old-byte/history preservation is removed. Task-6 receives runtime/storage tests after delivery; shared registration still remains retained. |
| `apps/desktop/src/bridge/governor.generated.ts`, `apps/desktop/tests/fixtures/governor-wire.json` | Rust-derived declarations and serialized fixtures. Consumers import them; no handwritten duplicate switch wire declarations. Regenerate only with the explicit Rust command below. |
| `apps/desktop/tests/governor-contract.test.ts` | Compiler-backed structural agreement against actual Rust fixtures. Task-7 receives consumer-test ownership and may add decoder checks while preserving generation/agreement assertions; generated files remain retained. |
| `scripts/coverage/config.ts`, `scripts/harness/coverage-reuse.test.ts` | Governor frozen-input registration and receipt-identity regression. Historical baseline constants, all eleven 80% floors and harness command semantics are preserved. Task-8 receives maintained tooling/manifest ownership through delivery. |
| Task-6 runtime | Owns daemon dispatch, existing writer integration, coordination, storage transactions, recovery, policy and Traffic repairs/tests/fixtures under its saved scope. No parallel edits to retained manifests, exports, DTOs, migration bytes or coverage config. |
| Task-7 consumers | Owns headless CLI, sole-spawn unreserved mode, registered Tauri bridge, accessible React switch and deterministic consumer/native tests under its saved scope. |
| Task-8 integration; final gate owner; requirements-docs; independent QA | Consume actual task-5/6/7 artifacts. Integration owns repairs and backend parser/test implementation; final gates run alone; docs owns `docs/requirements/traceability.md`; read-only QA writes `qa.json`; Chief finalizes `final.md`. |

No commit, push, external delivery, installed-daemon restart, reset, new OMA/AV run or mission-state mutation is part of task-5. Logs/caches are local preparation, not artifact transport. Every required producer artifact listed above is nonignored and Git-visible before handoff.

## Strict protocol-1 switch interfaces

Rust source of truth: `arc_ipc::governor`. Existing strict `traffic::Capabilities`, `policy::Capabilities`, Traffic `JobResult`/`StatusResult`, launch variants and `PROTOCOL_VERSION=1` are unchanged. Payloads are JSON objects (capability probe is null); arrays, positional nested structs, unknown fields, invalid IDs and over-budget payloads are rejected before dispatch. Switch/control/read payloads/results are at most 2048 serialized UTF-8 bytes; current-client execution payloads/results/ACKs are at most 16384 bytes, still under existing frame limits. Apply the existing connection/request/writer deadlines and queue limits.

| Operation | Request payload | Result |
| --- | --- | --- |
| `governor.capabilities` | `null` | Exactly `{capability:"governor.v1",execution_capability:"governor.execution.v1"}`. Advertise only with complete runtime support. |
| `policy.get_switch` | `GetGovernorSwitchRequest`: `{key:"governor.enabled", mutation_id?:string|null}` | `GovernorSwitchReadResult`: `{switch:GovernorSwitchState, mutation_result?:GovernorMutationResult|null}`. Optional receipt lookup is read-only. |
| `policy.set_switch` | `SetGovernorSwitchRequest`: `{key:"governor.enabled",enabled:boolean,expected_revision:number,mutation_id:string}` | `GovernorMutationResult` below, only after the writer commits or proves a no-effect conflict/rejection. |
| `daemon.pause` | Same complete setter payload, with `enabled:false` | Same desired-state setter/result. An inconsistent alias/value is `INVALID_REQUEST`. |
| `daemon.resume` | Same complete setter payload, with `enabled:true` | Same desired-state setter/result. Never implement either alias by inversion. |

`GovernorSwitchState` is `{key:"governor.enabled",enabled:boolean,revision:number,updated_at_ms?:number|null}`. All numeric revisions and timestamps are validated integers in `0..=9007199254740991`. Only an absent row means enabled/revision 0 with the timestamp omitted. A missing/null timestamp is valid only for ON/revision 0. A real row uses the committed `updated_at` in milliseconds, including explicit revision 0 if present; unavailable/corrupt storage is an error, never a guessed boolean. No-op writes leave both revision and state timestamp unchanged. The existing unrelated `memory_admission.policy_revision` remains a schema version, not this counter.

Mutation identity is exactly `<expected_revision>-<32 lowercase hexadecimal digits>` (34–49 ASCII bytes; canonical decimal, no leading zeros). Use existing OS-random job-ID entropy for the nonce. The embedded revision must equal `expected_revision`. The ID is stable across transport retries and aliases, has no authentication role, and cannot be rebased onto a different revision. This prevents a receipt evicted at an older revision from later becoming a fresh write.

The strict mutation union uses `status` (not job `state`):

- `COMMITTED`: `{status:"COMMITTED",mutation_id,switch,changed:boolean,replayed:boolean}`. `changed=true` means `switch.revision=expected_revision+1`; false means the revision stays at the CAS base. The resulting enabled value must equal the request. `replayed=true` contains the original committed state, which can be older than current state.
- `CONFLICT`: `{status:"CONFLICT",mutation_id,switch,reason:"REVISION_CONFLICT"|"MUTATION_ID_CONFLICT"}`. Switch is authoritative at the rejecting transaction; no requested mutation effects occur. Revision conflicts require an unequal current revision.
- `REJECTED`: `{status:"REJECTED",mutation_id,switch,reason:"REVISION_EXHAUSTED"|"RECEIPT_CAPACITY"}`. Confirmed no mutation effects, with matching CAS base. Exhaustion applies only to a changing write at the maximum revision. A same-state maximum-revision no-op is still permitted when receipt capacity exists.

Envelope errors retain existing `INVALID_REQUEST`, `PROTOCOL_INCOMPATIBLE`, `UNSUPPORTED_CAPABILITY`, identity/transition and transport semantics. `GOVERNOR_UNAVAILABLE` is a failed read, not OFF. `GOVERNOR_WRITE_UNCERTAIN` explicitly means a mutation may have committed. Generic `DB_BUSY`, response cancellation, timeout, disconnect, malformed reply or dropped post-COMMIT result are also uncertain once dispatch may have occurred. They must never become the typed `REJECTED` union. Only demonstrable pre-dispatch failure can be classified as no-effect transport rejection. No fake rollback or success is permitted.

### Readback and client settlement

An optional valid mutation ID asks the existing writer for that durable committed receipt and the current state in one read snapshot. A present `mutation_result` must be a `COMMITTED` result with `replayed=true`, matching the requested identity; it is validated independently against the caller's original immutable intent. It cannot have a revision newer than the current state; equal revisions must have identical state including timestamp. Omit it if not recorded/evicted/not yet committed. An absent receipt is explicitly **not** proof that an outstanding write was rejected or cannot still commit.

Clients retain confirmed state, requested intent, CAS base and stable ID separately. An uncertain write triggers identity-aware readback, never blind inversion or automatic rebasing. A committed receipt settles that request while the current state determines display. Current evidence can update display without attributing success to an unknown request. An older/equal base with no receipt cannot settle a potentially delayed write; retain unconfirmed state and serialize further mutation. Once the current revision exceeds the immutable CAS base, the old request cannot change later state, but its historical outcome may remain unknown after receipt eviction. Discard older state revisions; equal revision/different state is inconsistent evidence requiring reconciliation. The bridge/mutation owner outlives polling generations and navigation.

## SQLite authority, integrity and retention

External schema: owner-authenticated switch/current-receipt reads and one desired-state setter, plus existing job/Meter/permit projections. Conceptual schema: one Governor switch, bounded committed mutation receipts, and one negotiated execution disposition belonging to an existing lifecycle job. There is no second job queue, outcome store, permit authority or client preference. Internal schema is additive migration `0006_governor`, registered as schema version 6.

| Durable facts | Representation and rules |
| --- | --- |
| Master state | Existing `policy_states`, fixed key `governor.enabled`; `enabled` 0/1, safe nonnegative `policy_revision`, `updated_at` UTC milliseconds. Use `policy_version='governor.v1'` for this row; `state_json` need not duplicate the boolean. `get_switch` validates legacy unconstrained column values. Never rewrite the memory-policy row. |
| Mutation receipts | `governor_mutation_receipts`: monotonic `sequence`, unique revision-bound `mutation_id`, `expected_revision`, `desired_enabled`, `result_enabled`, `result_revision`, nullable `result_updated_at_ms`, `changed`, `committed_at_ms`. SQL enforces ID/base agreement, bounds and resulting revision/value consistency. Reconstruct the original COMMITTED result; `replayed` is transport metadata, not a second state. |
| Receipt bound | Existing application writer caps at `MAX_MUTATION_RECEIPTS=1024`. Resolve duplicates before pruning/CAS. Retain newest receipts; evict only receipts whose base is older than the resulting authoritative revision. Never evict a live current-base no-op receipt while that revision remains current. If 1024 current-base no-ops exhaust capacity, a further no-op is confirmed `RECEIPT_CAPACITY`; a changing write may advance revision and atomically evict an obsolete oldest receipt. No automatic reset or unbounded receipt growth. |
| Replay horizon | An identical retained receipt replays even across intervening writes/restart. Conflicting contents under a retained identity produce ID conflict. After bounded eviction, the embedded old base necessarily fails CAS; it cannot apply again. Stale/evicted requests return revision conflict/current state, not invented committed proof. Receipt-capacity rejection does not claim a state change. |
| Negotiated jobs | `governor_jobs.job_id` is a PK/FK to `job_lifecycle`; no legacy job acquires this row merely through a read. `mode` is SCHEDULED or UNRESERVED; `switch_revision` records the disposition decision. UNRESERVED requires `unreserved_at_ms`; `previous_generation` is optional positive former reservation metadata. SCHEDULED clears unreserved-specific fields. |
| One-shot release | Nullable unique lowercase-hex32 `release_id` and matching `release_claimed_at_ms` on `governor_jobs`. Both absent or both present; claim time cannot precede eligibility. Retain a possibly delivered claim across timeout, restart and missing ACK. Lifecycle observer/child identities, revision and outcome remain authoritative in existing tables. |
| Index/access paths | Unique mutation-ID lookup, `(expected_revision,sequence)` for bounded pruning; job PK for owner lookup and `(mode,job_id)` for bounded recovery. FIFO ordering remains `traffic_entries.sequence`, not a new sequence. |

The missing representation justifies the two tables: migration 0003 cannot represent unreserved disposition/claims without violating its grant/generation/context checks, and existing audit events have no immutable CAS/result or execution-mode schema. SQL contains integrity constraints and indexes only: no triggers, business scheduler or external I/O. The existing single writer owns all policies, transitions, receipt pruning and outcome/permit reconciliation. Use parameterized SQL. No second connection/pool/writer or authoritative cache is introduced.

Task-6's setter runs inside the existing writer's IMMEDIATE transaction. The task-6 storage entry is `set_switch(tx: &rusqlite::Transaction<'_>, request: &SetGovernorSwitchRequest, now_ms: u64) -> JobResult<GovernorMutationResult>`, returning a tentative result; daemon publication is after COMMIT. This signature is a runtime handoff obligation, not an unimplemented function exported by task-5. The public getter already exists. Order: validate/authenticate before admission; lock/coordinate; read receipt and authoritative state after backlog; replay/conflict; enforce CAS; verify revision/capacity; apply switch and eligible job/reservation transitions; store receipt and bounded pruning; commit; acknowledge. Same-state no-op commits only its receipt. Rejected/conflicting mutations cannot cause queue/permit/release changes. A failed commit rolls back the transaction; post-commit acknowledgment loss remains uncertain.

Migration is expand-only, forward-only under the existing atomic migrator. On failure, rollback all new DDL/history/user_version, preserving the schema-5 DB. Old binaries reject unsupported newer schema rather than downgrade it. No deployed user database is opened by task-5; tests use isolated temporary databases. Existing lifecycle/terminal retention also owns cleanup of dependent `governor_jobs` rows before lifecycle deletion; never discard uncertain live releases to satisfy retention. Required recovery facts survive until reconciliation. No new privacy payloads, command strings, paths, credentials or environment values are stored. Backup/restore remains one SQLite/WAL database with unchanged migration witnesses.

## Current-client grant, launch and acknowledgment contract

Negotiate `governor.capabilities` separately from existing Traffic, Bun and memory-policy probes. Unsupported explicitly selects the maintained legacy path; malformed responses do not assert support. Supported local Cargo build/test/clippy/fmt and supported one-shot Bun test use `governor.job` and `governor.launch`. Remote/UNKNOWN/unsupported commands retain their existing classification/bypass, streams, environment, arguments, directory, signals, cancellation, status, bounded outage fail-open and Meter handling.

`GovernorJobRequest` is tagged by `action`:

- `TRAFFIC {request:traffic::JobRequest}` wraps the unchanged existing Enqueue, Poll, Attach, Complete, Cancel, Fallback and Status/StatusV2 contracts. The runtime enforces classification/owner restrictions just as on the existing Cargo/Bun routes. No command/path/env crosses the wire. Only actual negotiated enqueue registers a `governor_jobs` row.
- `ATTACH {job_id,observer,release_id,child}` and `COMPLETE {job_id,observer,release_id,outcome}` operate on an unreserved release, after the same independent peer/child/native evidence checks. No permit_generation is invented.

`GovernorJobResult` is tagged by `state`:

- `TRAFFIC {result:traffic::JobResult}` wraps a real legacy-shaped lifecycle-operation result, including terminal/running states; this wrapper alone makes no claim that a permit exists. This is the reply to wrapped requests other than Status/StatusV2.
- `STATUS {result:traffic::StatusResult}` is the reply to wrapped Status/StatusV2 requests. It preserves the separate strict status schema: QUEUED `{sequence,queued_at_ms}`; RESERVED `{sequence,queued_at_ms,granted_at_ms,reservation}`; RUNNING `{queue_wait_ms}`; COMPLETED `{queue_wait_ms,outcome}`; CANCELLED, ABANDONED, UNCONFIRMED or NOT_FOUND `{}`. The nested result retains its existing `state` tag, validation and Status versus StatusV2 semantics; no generation, lineage context or owner-only fields are added. `decode_job_result` validates this wrapper with `traffic::StatusResult::is_valid`, rather than treating status as a `traffic::JobResult`. Existing `traffic.job` status replies remain unwrapped and byte-compatible.
- `UNRESERVED {job_id,switch_revision,ready_at_ms}` is eligibility only, with validated safe numbers and no permit, generation or lineage capability. CLI must still perform final launch authorization.
- `UNCONFIRMED {job_id}` means a release might have started; reconcile, do not start another child.

`GovernorLaunchRequest` is tagged by `mode`: `RESERVED {request:policy::launch::LaunchRequest}` or `UNRESERVED {job_id,observer}`. `GovernorLaunchResult` is tagged by `state`: `RESERVED {result:policy::launch::LaunchResult}` (boxed only in Rust; identical nested JSON); `UNRESERVED {switch_revision,release_id,valid_for_ms}`; `RETRY {}`; `UNCONFIRMED {}`; `CANCELLED {}`; `NOT_FOUND {}`. A reserved request may receive UNRESERVED if pause won the ordering. A stale unreserved request after resume receives RETRY and polls under restored admission; it is not a permit-free authorization under an old OFF read. UNRESERVED validity is `1..=2000` milliseconds and is shortened to the existing absolute IPC/fence budget, never refreshed by transport delay.

A native release uses the existing fenced connection through write and same-connection `governor.launch.ack`. `GovernorLaunchAck`, tagged `mode`, is `RESERVED {ack:existing LaunchAck}` or `UNRESERVED {ack:GovernorUnreservedAck}`. The latter is tagged `state`: STARTED `{job_id,observer,release_id,child}` or UNSPAWNED `{job_id,observer,release_id}`. Match both the original authenticated request and delivered result; a RESERVED ACK must match the actual generation and Ready/Reused result. An unreserved ACK matches the delivered opaque release ID even when the original request was RESERVED. Standalone, mismatched, malformed or late unrelated ACKs cannot clear any claim. ACK grace may confirm a known unspawned release after expiry but never extend launch validity. Child start is established by native/OS and lifecycle evidence, never a Ready/ACK alone.

### Serialization and transition rules

The existing single writer orders durable decisions, but a delivered launch fence outlives its transaction. Extend existing launch/control coordination across Governor setters, grants and both launch modes, including missing/failed sampler fallback; do not depend on a healthy observation source. Required acquisition order is existing writer → IMMEDIATE → nonblocking common launch/control lease → observation publication fence. A contended common lease rolls back/returns bounded deferral so the writer never waits on a lease holder that needs the writer. A single existing-budget owned semaphore/lease is an acceptable implementation of that extension; no opposite lease→writer blocking order is allowed. Retain the common lease through native release/ACK/absolute expiry. Setters/grants retain it only through their commit. Drop both launch leases before enqueueing UNSPAWNED cleanup. Never retain a SQLite transaction while waiting for IPC/native spawn. Prove the timeout/Unspawned race has no deadlock.

The switch COMMIT linearizes state. A launch either already owns its bounded one-shot release under the earlier state, or observes the committed switch before authorizing. Pausing never kills an already running/possibly spawned child or unconditionally frees its reservation. Resuming also serializes with outstanding unreserved releases: no stale OFF read can authorize a new later release. The CLI keeps exactly one spawn site and its adjacent cancellation/continuity/deadline check. Uncertainty is persisted and reconciled through the existing witness, not retried as another spawn.

| Job evidence at OFF | Transactional disposition |
| --- | --- |
| New supported current-client root | Register existing lifecycle/Traffic identity and negotiated UNRESERVED mode; bypass resource-window and fresh-Critical holds; mint no permit. Preserve FIFO/queue timestamps for actual wait accounting. |
| Negotiated QUEUED | Convert the existing ID once to UNRESERVED, retaining sequence and original queue time; no user resubmission, no permit. |
| Negotiated RESERVED, independently known unspawned, no delivered/maybe-delivered one-shot marker | Convert once; reconcile the former reservation in the same transaction. Preserve the former generation in disposition/audit as appropriate. Do not release a root reservation still referenced by a verified descendant. |
| Any running/attached child, delivered Ready/uncertain marker or retained root shared by descendants | Preserve execution ownership, claims and necessary reservations; reconcile through the maintained witness/identity path. No blanket release or second grant/release. |
| Subsequent OFF requests | Bypass admission even with fresh Critical; retain measurement, child tracking, cancellation and outcomes. |
| Resume before an unreserved release is claimed | Return/reconcile to SCHEDULED with the same identity/sequence, restored windows and fresh-pressure rules. Clear only known-unclaimed mode facts; a later grant uses a fresh valid generation, never generation 0 or reuse of a released authorization. Existing unique-per-job permit representation must be reconciled, not bypassed by another permit row. |
| Late duplicate completion/cancel/ACK | Existing identity/generation/release and lifecycle revision rules prevent a second execution or double release. |

Legacy strict clients do not receive new variants. New legacy supported work while OFF can receive the existing `Bypassed {reason:Unsupported}`: the daemon currently offers no permit-backed route for that unnegotiated new request, and the unchanged legacy CLI falls back to lifecycle tracking. This does not rewrite its classification or forge Remote/Unknown. Do not convert an already owned legacy queued/reserved job into that result or release its reservation behind an old client's back; existing owned legacy requests retain compatible status/ownership until they resolve. The mission's queued/reserved conversion proof explicitly concerns negotiated current clients. Keep nested reservation checks and owner-only contexts; unreserved work never exports a fictitious lineage token or a zero-resource permit.

## Rust-derived TypeScript and desktop consumer handoff

`ts-rs = "=12.0.1"` is declared in workspace dependencies and consumed **only** as an arc-ipc dev-dependency. Production DTOs use `cfg_attr(test, derive(ts_rs::TS))`; production normal dependency graphs do not contain the exporter. Versioned [ts-rs docs](https://docs.rs/ts-rs/12.0.1/ts_rs/) were checked, then the actual locked crate was fetched/compiled. Serde attributes supply names, enum tags/spelling and omitted/nullable optionality. Only validated u64 switch revision/timestamp fields map to TypeScript number; no global large-int mapping or new runtime dependency. The `deny_unknown_fields` exporter warning is about a serde runtime rule that TypeScript cannot express; actual Rust decoder rejection is tested. No no-op implementation replaces those checks.

Generated `governor.generated.ts` contains GovernorSwitchKey, GovernorSwitchState, GetGovernorSwitchRequest, SetGovernorSwitchRequest, GovernorConflictReason, GovernorRejectionReason, GovernorMutationResult and GovernorSwitchReadResult. It preserves ts-rs declaration layout with deterministic trailing-whitespace normalization and a file-local format annotation; semantic lint/type checks still run. Rust also serializes the Git-visible `governor-wire.json` fixtures, including every result/rejection reason, no-op/replay, optional omission and upper bound. Explicit update command:

```sh
ARC_GOVERNOR_EXPORT=1 cargo test -p arc-ipc --lib governor::typescript::regenerate -- --exact
```

Normal `governor::typescript::generated_types_are_current_without_writing` compares both outputs generated in memory with tracked bytes, without rewriting. The regeneration entry writes only when ARC_GOVERNOR_EXPORT=1 is explicitly supplied; ordinary runs execute a nonmutating agreement check, with zero ignored tests. This preserves the maintained receipt policy requiring complete native execution. Bun's `governor-contract.test.ts` uses the installed TypeScript 7 native `tsc --noEmit` against temporary source literals produced from those Rust fixtures, exercises negative structural expectations, and checks generated artifacts remain unchanged. It uses no unstable compiler API, extra TypeScript version or browser. Safe-integer validation remains a runtime obligation for the task-7 decoder; TS number alone cannot enforce that bound.

Task-7 implements `arcctl daemon pause` / `arcctl daemon resume` and authoritative headless state/revision readback using these same payloads and existing authenticated transport. Preserve old command/exit semantics. Expose stable identity/CAS inputs for deterministic duplicate/conflict checks; new convenience controls read before creating a fresh immutable intent, never retry by inversion. Registered bridge commands should use `governor_get_switch` with `{query:GetGovernorSwitchRequest}` and `governor_set_switch` with `{request:SetGovernorSwitchRequest}`; return the same Rust DTOs and preserve native Uncertain versus Rejected. Both commands share existing single-flight coordination. Rust aliases remain available for headless/IPC controls without introducing another setter.

The dashboard switch keeps the name Agent Resource Control, checked=confirmed enabled, text Active / Paused / Governor unavailable, initial Checking Governor… and separate connection/refresh states. Applying intent is not a confirmed state. Confirmed rejection restores prior confirmed state unless newer authoritative evidence supersedes it. Uncertain acknowledgment says Change not confirmed. Checking Governor… and initiates identity-aware readback. The mutation owner survives About/hide/reopen; stale reads cannot retire it. Retain G01–G13 from the experience handoff, keyboard/focus/help/polite announcement requirements, current policy explanations and live Meter readings. Paused copy explains supported waiting jobs can proceed without resubmitting while running work and measurement continue. This delivery includes no UI interaction or rendered/VoiceOver completion claim.

## Required native and deterministic case inventory

These IDs are Governor-cycle evidence keys, not invented replacements for SRS acceptance IDs. Task-8/final gate owner records the real test selectors, command receipts, case outcomes and synchronized observations under these keys. Every case must execute on final inputs; a mock/Ready/checked switch is not child-execution proof.

| Case | Required observation and acceptance |
| --- | --- |
| GV-N01 Owner/durability | Production daemon verifies same-user transport for all controls/reads and native owner identity for jobs. Default ON/0, committed pause/resume and readback match SQLite; no acknowledgment before COMMIT. |
| GV-N02 One-slot sequence | A has one actual child and active permit; B is queued with zero starts. Pause via CLI or registered bridge commits; B starts exactly once without a new permit; A remains alive. Count child starts/PIDs independently. |
| GV-N03 Paused bypass | Further supported local requests under OFF start once with zero new permits, including injected fresh Critical. Track command outcomes and Meter without daemon-outage fallback. |
| GV-N04 Permit accounting | Existing permits release once on actual outcomes; descendants retain necessary root reservations; final active/released totals reconcile with SQLite, CLI and registered bridge. No resource totals or lifecycle outcomes are fabricated. |
| GV-N05 Restart | Stop/restart only the isolated production-daemon fixture with the same DB. OFF and revision survive; no job is automatically replayed after restart. |
| GV-N06 Resume | Resume advances once, restores one-slot queuing and fresh-Critical enforcement. A retained pre-backlog Normal cannot authorize a new launch; later genuine fresh recovery reuses maintained admission behavior. |
| GV-N07 Duplicate/conflict/no-op | Stable replay across restart/intervening writes, same ID/different intent, concurrent CAS, unchanged-state receipt/timestamp, maximum revision, bounded capacity and stale evicted-ID safety. Zero duplicate transitions/releases. |
| GV-N08 Backlog | Synchronized existing writer/SQLite backlog; state and observation are read after admission/lock, not captured before waiting. Cancellation before commit and post-commit deadline ambiguity are distinguished. |
| GV-N09 Reserved race | Synchronize RESERVED, toggle, launch Ready, pre-exec, spawn and UNSPAWNED boundaries. Known unspawned converts exactly once; maybe-delivered release stays claimed. Actual child count/outcome and permit reconciliation agree. |
| GV-N10 Launch ordering | Pause and resume each race outstanding reserved/unreserved releases; both healthy sampler and fallback paths serialize; ACK expiry/Unspawned cleanup cannot deadlock or authorize a later stale spawn. |
| GV-N11 Cancellation | Cancel queued, converted and reserved work, including cancellation before enqueue and late duplicate terminal writes. At most one child; known unspawned cleanup versus uncertain witness recovery remains explicit. |
| GV-N12 Lost acknowledgments | Lost post-COMMIT setter reply, ambiguous DB_BUSY, lost native ACK and disconnected writer waiter. Identity-aware readback returns current state plus any original receipt; missing receipt is not rollback proof. No blind mutation/spawn retry. |
| GV-N13 Reconnect/surfaces | CLI/registered bridge disconnected before/after dispatch, reconnect, cross-client writes, stale/equal-conflicting revisions and navigation during pending write. UI recovery distinguishes committed/rejected/uncertain and eventually converges on authoritative evidence. |
| GV-N14 Compatibility | Protocol-1 strict legacy round trips/capabilities, legacy OFF projection, existing Cargo/Bun/unknown/remote behavior, arguments/env/cwd/streams/signals/exit status, nested reservation ownership, bounded outage fail-open, Meter outcomes/accounting and immutable migrations. |
| GV-N15 Structural/interaction | Rust regeneration agreement, TS compiler/runtime decoder bounds, all G01–G13 and labeled keyboard/focus/polite announcement interaction cases. Deterministic fixtures are labeled as such. |

Reuse production startup/routing/writer seams in `crates/arc-daemon/tests/fixtures/memory_pressure_daemon.rs` and registered Tauri invocation seams in `apps/desktop/src-tauri/tests/memory_pressure/`. Add Governor-specific modules under the respective implementation owners. Pressure injection is explicitly injected, separate from actual SystemSource observations. Required independent reconciliation includes switch enabled/revisions, actual child starts and PID identities, terminal outcomes, permit state/generation and reserved resources through SQLite, CLI and the production registered bridge router. Socket fixtures establish transport only. No throughput, native-Critical OS event, usability study, savings or commercial result is implied by deterministic fixture success.

## Immutable migration inputs

Measured from the accepted Git blobs and checked against current bytes; never edit these five files:

| Path under `crates/arc-storage/migrations/` | SHA-256 |
| --- | --- |
| `0001_phase0.sql` | `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384` |
| `0002_managed_exec.sql` | `464a1c88270f01a56ccab6610f87eff7648e0640f276ebce53b8a97026c50814` |
| `0003_traffic.sql` | `1f4522e3f27c124aea7127d92434c7bd22be36168fef0a4bf665f8cf16debe90` |
| `0004_job_meter.sql` | `e4c40db876032d858667e2af5d6be524c2f0bd204a7c6b97ba878e082496c540` |
| `0005_memory_pressure.sql` | `6396adb0918a1bacfea16945caa866b12e91d322454ec64a00d316b0bccf0326` |

## Exact verification recipes and exclusivity

The final gate task runs **alone** in the integrated mission workspace `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/6dd4dd59-bec1-4e67-a1ba-65f73e0660ce/workspace`. No implementation/docs/QA task concurrently changes inputs. Task-5 performs focused contracts/migrations/preparation only and makes no final-gate exclusivity claim. An empty top-level verifyCommand does not remove the saved structured contract. Do not alter any operator verification argv. Every replay/wrapper allows at least `7200000` milliseconds per check; the contract's smaller timeoutMs values are estimates. Cancellation and the existing absolute 172800-second mission limit still apply, with maxParallel 3, maxRuns 400 and maxRetries 3; no task restarts that budget.

The six gates remain exactly:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bun run test:coverage
./scripts/harness/validate.sh
bun test apps/desktop/tests/bridge-client.test.ts apps/desktop/tests/contracts.test.ts apps/desktop/tests/credits.test.ts apps/desktop/tests/dashboard.test.ts apps/desktop/tests/lifecycle.test.ts apps/desktop/tests/meter.test.ts scripts/coverage/accounting.test.ts scripts/coverage/desktop-foundation.test.ts scripts/coverage/gate.test.ts scripts/coverage/hygiene.test.ts scripts/coverage/terminal-outcomes.test.ts scripts/coverage/traffic-accounting.test.ts scripts/harness/equivalence.test.ts scripts/harness/traffic-targets.test.ts scripts/harness/coverage-reuse.test.ts
```

Before the coverage gate use `bun scripts/coverage/run.ts --preflight-only`; task-5 collects **zero** coverage. For the final successful input set collect once, using maintained `ARC_COVERAGE_OUTPUT_ROOT=target/governor` for raw output. Set `ARC_COVERAGE_RECEIPT` to the validated final-input `reuse.json` when executing the unchanged harness recipe. Reuse fails closed without recollecting on invalid/missing/stale receipts. Register this frozen file through `GOVERNOR_INPUTS`/`FROZEN_INPUTS` in the maintained helper; do not hash generated Governor reports into collection identity. Keep all historical constants/receipts and independent 80% production-line floors for arc-core, arc-detector, arc-storage, arc-ipc, arc-daemon, arc-cli, arc-traffic, arc-desktop, arc-platform, arc-meter and arc-policy. No averaging or test-only production exclusions are introduced.

Required additional commands remain:

```sh
bun test apps/desktop/tests/governor.test.ts apps/desktop/tests/governor-contract.test.ts
bun test scripts/requirements/traceability-parser.test.ts scripts/requirements/traceability.test.ts
bun run desktop:typecheck
bun run desktop:check
bun run desktop:build
bun run desktop:smoke:tauri
```

The mission explicitly authorizes relevant compilation/tests, desktop build and debug/no-bundle smoke. These final checks are not replaced by task-5's narrower successful commands. Use deterministic unit, integration and CLI checks. Do not introduce/run Playwright/browser E2E, download browser packages/binaries, perform GUI workarounds, run screen checks during individual work/review, or run unrelated historical validators. A permitted visual check belongs only to the single final-review pass after code verification.

## Canonical evidence files and saved JSON pointers

The canonical inventory is exactly five files under `.arc-verify/governor/`: `contract.md`, `gates.json`, `integration.json`, `qa.json`, `final.md`, collectively at most 300000 UTF-8 bytes. This delivery owns the frozen contract, preserves the preceding Chief record, and leaves actual integration/gate/QA reports for their assigned writers. No sixth per-task report, raw log, observation file or copied receipt belongs in that directory. All raw logs/coverage inventories/native outputs stay under `target/governor`. Required cross-worktree observations/receipt summaries are embedded compactly in the appropriate canonical tracked report; ignored target files alone are never the handoff. Each executable receipt summary includes exact argv/cwd, actual exit, counts, input identity and raw path/hash. Raw hashes are references, not standalone proof that a missing isolated artifact was delivered.

`gates.json.inputs` holds the single final input identity (maintained digest, frozen input hashes and verification method). Other reports reference that identity. Avoid repeated inventories/full logs. Retain compact successful native observations and coverage integer numerators/denominators, tool versions and validated reuse evidence so independent QA can verify/recompute current input identity from the delivered source without an ignored-only dependency. Set pass booleans only from actual final evidence; pending/failed cases cannot be reported true.

All saved assertions remain mandatory, with the following exact pointers and values:

| File | Pointer | Required value |
| --- | --- | --- |
| integration.json | `/baseline/acceptedHead` | `08e7e526cce7e03ca9264afc926ed2e878dfde6d` |
| integration.json | `/nativeAcceptance/platform` | `macos` |
| integration.json | `/nativeAcceptance/allRequiredExecutedAndPassed` | `true` |
| integration.json | `/switch/ownerAuthenticatedCommitDurabilityPassed` | `true` |
| integration.json | `/reconciliation/sqliteCliRegisteredBridge` | `true` |
| integration.json | `/reconciliation/noDuplicateExecutionsOrPermitLeaks` | `true` |
| integration.json | `/compatibility/allPassed` | `true` |
| integration.json | `/migrationPreservation/migrations0001Through0005Match` | `true` |
| integration.json | `/desktop/interactionCasesPassed` | `true` |
| integration.json | `/contracts/rustToTypeScriptAgreementPassed` | `true` |
| gates.json | `/execution/finalGateRanAloneInMissionWorkspace` | `true` |
| gates.json | `/inputs/identityVerified` | `true` |
| gates.json | `/sixGates/allPassed` | `true` |
| gates.json | `/desktopChecks/allPassed` | `true` |
| gates.json | `/coverage/preflightPassed` | `true` |
| gates.json | `/coverage/successfulCollectionsForFinalInputs` | `1` |
| gates.json | `/coverage/validatedReceiptReusedByHarness` | `true` |
| gates.json | `/coverage/thresholdPercent` | `80` |
| gates.json | `/coverage/crates/<crate>/productionFloor80Passed` for EACH of the eleven crate names above | `true` |
| gates.json | `/traceability/missingRequirementIds` | `[]` |
| gates.json | `/traceability/missingAcceptanceTestIds` | `[]` |
| gates.json | `/traceability/duplicateIds` | `[]` |
| gates.json | `/traceability/allRowsHaveStatusEvidenceAndCycle` | `true` |
| gates.json | `/traceability/phaseCountsReconciled` | `true` |
| qa.json | `/reviewComplete` | `true` |
| qa.json | `/requiredEvidenceComplete` | `true` |
| qa.json | `/inputsMatchFinalGates` | `true` |
| qa.json | `/assessment/unresolvedGovernorSliceBlockers` | `[]` |

The generated TS file must be at least 200 bytes and contain Generated from Rust, GovernorSwitchState, SetGovernorSwitchRequest and GovernorMutationResult. Traceability must be at least 8000 bytes, include Requirement ID, Acceptance-test ID, Status, Evidence, Cycle, governor.enabled and Phase 0–5. Chief final.md must be at least 2000 bytes and retain Governor slice only, Decisions, Alternatives, Measured results, Lessons, Next actions and Phase 0–5. These file assertions do not replace executable/native acceptance. QA remains independent and read-only for implementation/tests/configuration, writes only its report and neither runs gates nor recollects coverage.

## Amendment 13 traceability format and counts

Dedicated requirements-docs creates `docs/requirements/traceability.md` from the complete authoritative SRS. Use an explicit requirements inventory table with `Requirement ID | Acceptance-test ID | Phase | Status | Evidence | Cycle`, and an explicit acceptance inventory with `Acceptance-test ID | Requirement ID | Phase | Status | Evidence | Cycle`. One primary row per declared requirement ID and per AT heading ID; cross-reference cells may repeat links but do not create duplicate primary IDs. Expand every range. Include any other formally declared requirement/goal-ID catalog in its identified inventory rather than silently dropping it; textual Governor clauses §§7.3/15.5 are supplementary mappings, not fabricated original IDs.

Statuses are Complete, Partial, Pending or Blocked. Every row has a nonempty concrete evidence path/test selector or explicit pending/blocker reason, and a cycle identifier (predecessor cycles supported by their receipts, or current governor-2026-11-13). One primary Phase 0–5 is assigned from SRS delivery scope; priority P0/P1/P2 is not a phase number. Explain cross-phase assignments and unknown evidence. Requirement and acceptance counts are separate: for each phase report total and each status count, with totals equaling the unique inventory and no double counting. All six phases appear even if the count is zero.

Backend-owned `scripts/requirements/traceability-parser.test.ts` and `traceability.test.ts` verify source extraction, every ID, duplicates, row fields and reconciled counts; the docs Actor edits no implementation/tests/gate config and runs no native gates. Historical behavior needs its own evidence/cycle; this slice cannot complete full multi-vendor, Sweeper, learning, Windows, distribution, user-study or savings requirements. Task-12 reports the evidence-backed per-phase totals from the final matrix; task-5 does not invent completion counts.

## Task-5 check evidence and limits

Skills applied: installed backend-engineer profile with **oma-backend** primary (existing transport/service/repository contracts and validation); **oma-db** (normalized additive metadata, integrity, IMMEDIATE/commit/recovery boundaries and migration checks); **oma-frontend** (Rust-derived TS consumer contract, strict optionality and existing TypeScript 7 verification). Their relevant execution/checklist and database rules were read. Product-interface visual review is not applicable to this shared-contract task.

Code intelligence: configured Serena, mode unavailable; tool discovery listed no callable Serena tools. Scoped reads used only already located source paths and test failure paths; native content search was restricted to non-code SRS or external dependency/memory paths. No substitute recursive project-source search, provider installation/registration or guard bypass. `av.yaml` returns Operation not permitted; its contents and any external target it might name remain unverified. No immutable deployment/container target or credentials were supplied to task-5.

Host probe: Darwin arm64, 8 CPUs, 17179869184 bytes RAM, about 82 GiB free at entry. Rust 1.95.0 / LLVM 22.1.2, Bun 1.4.4, cargo-llvm-cov 0.6.21 and LLVM tools are installed. Cargo preparation uses at most two build jobs; no subagent or external task was dispatched. Supplied mission limits/deadline persist (earliest supplied start 2026-11-12T18:13Z implies conservative outer bound 2026-11-14T18:13Z; the scheduler owns the actual deadline). No price/cost claim is inferred from resource metadata.


All commands use the mission workspace cwd above. Asynchronous shell sessions were allowed to finish; no two-hour check was shortened by a wrapper. The following compact summaries and log hashes carry the actual task-5 outcomes across worktrees; full logs remain scratch files.

| Command | Exit | Actual outcome | Raw log / SHA-256 |
| --- | ---: | --- | --- |
| `cargo fetch --offline` | 101 | Pinned ts-rs missing from cache; normal locked fetch below repaired this prerequisite. | `target/governor/task5/fetch-offline.log` / `26dbe2338c1a2ff4090c512139c5cf549a015e216f6c03e734d643363cab93d4` |
| `cargo fetch --locked` | 0 | Fetched ts-rs/ts-rs-macros 12.0.1; lock includes three required dev-transitive entries. | `target/governor/task5/fetch.log` / `fda3256c2b220f5c1bd513e39c2bb7ca4b761c5d0c31d82411be9cb92e16640f` |
| `bun install --frozen-lockfile` | 0 | Installed 85 locked packages locally; no Bun manifest/lock changes. | `target/governor/task5/bun-install.log` / `47cb654578adc9f0f719ee003b8ea10714c8363029e73007cb3720df9777ea34` |
| `ARC_GOVERNOR_EXPORT=1 CARGO_BUILD_JOBS=2 cargo test -p arc-ipc --lib governor::typescript::regenerate -- --exact` | 0 | Explicit generation: 1 passed; normal generator mode is nonmutating. | `target/governor/task5/generate-final.log` / `8f4da85a348a5b7a94b6a7c18c459d89e89c53f11f282ade67e37e13b177ff74` |
| `CARGO_BUILD_JOBS=2 cargo test -p arc-ipc --lib --test governor --test memory_pressure --test traffic` | 0 | 58 library + 7 Governor + 3 policy + 9 legacy Traffic = 77 passed, 0 failed, 0 ignored. Includes nonmutating Rust regeneration/fixture agreement. | `target/governor/task5/ipc-final.log` / `bf9b9c9ae8aa6c8c72210f15d2b70c9741d54f274322b0b3991763f5e28b263b` |
| `CARGO_BUILD_JOBS=2 cargo test -p arc-storage --test governor_migration --test managed_exec_migration --test memory_pressure` | 0 | 5 new schema/read + 6 current migration + 20 pressure = 31 passed, 0 failed, 0 ignored. | `target/governor/task5/storage.log` / `e17a90779d8610a9afc112b51bcab1137289235609fd7d40166660687db9c943` |
| `CARGO_BUILD_JOBS=2 cargo test -p arc-storage --lib migration` | 0 | 24 passed, 0 failed, 0 ignored; selector filters 75 unrelated library tests. This is a focused check, not final full-workspace execution. | `target/governor/task5/migrator.log` / `344b2adb2fb13cea1ae1ba21f8d2ec6ffcff367c24db544c9e93074cdfcd4f22` |
| `CARGO_BUILD_JOBS=2 cargo clippy -p arc-ipc -p arc-storage --all-targets -- -D warnings` | 0 | Both affected crates and targets pass. ts-rs emits the documented serde-attribute diagnostic; no Clippy lint is suppressed. | `target/governor/task5/clippy-final.log` / `897134b7aead85fffb84ea4108ceb76c631d09017659a97009e92c5b146fd802` |
| `bun test apps/desktop/tests/governor-contract.test.ts` | 0 | 2 passed, 0 failed; installed native TypeScript compiler checks actual Rust fixtures, negative structural cases and nonmutation. | `target/governor/task5/typescript-final.log` / `07b38c86742aa5baaa8f8aaefbfefc605cd0dffe52f224f269206f18e54b8983` |
| `bun run desktop:typecheck` | 0 | Both existing desktop no-emit TypeScript projects pass with the generated contract and test. | `target/governor/task5/desktop-typecheck-final.log` / `7c3cb6045e4508ad4ca264a0bdbb3207275c78410b843909c0ea071a97268c0f` |
| `bunx --no-install biome check apps/desktop/tests/governor-contract.test.ts apps/desktop/tests/fixtures/governor-wire.json apps/desktop/src/bridge/governor.generated.ts` | 0 | 3 affected files pass; generated declarations keep the narrow reproducibility-format annotation. | `target/governor/task5/biome-final.log` / `0867169c3092253043f8add68fe16f9b6a673447075bf76d2a95a910703438ac` |
| `bun test scripts/harness/coverage-reuse.test.ts scripts/coverage/hygiene.test.ts` | 0 | 20 passed, 0 failed; synthetic receipts verify Governor input binding, unchanged floors, stale/missing receipt rejection and no recollection. Not native measurement evidence. | `target/governor/task5/identity-tests.log` / `3754c2328693dfc6ae52bd726af81aa879846d66e6fdde916cb1cc1c8722e47c` |
| `cargo tree -p arc-ipc --edges normal --locked --offline` | 0 | No ts-rs in normal production dependency graph. | `target/governor/task5/production-deps.log` / `f4d535d54f01e3a163f113899610eacd8bd132abe67ccc9e4d3094202cfc74db` |
| `rustfmt --edition 2021 --check` with the 12 changed Rust entry/test paths listed below (module recursion enabled) | 0 | All owned Rust formatting passes; not represented as the final workspace fmt gate. | `target/governor/task5/rustfmt.log` / `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `ARC_COVERAGE_OUTPUT_ROOT=target/governor/task5 bun scripts/coverage/run.ts --preflight-only` | 0 | Pinned tools, matching LLVM, locked/offline metadata, all eleven workspace members and source inventory pass. coverageCollected=false; no collection/reuse receipt was produced. This preparation predates the final report append, so it is not a final-input coverage identity. | `target/governor/task5/preflight.log` / `8e0aa792d308456294422e9a30ccf8d67a0ab4ba27d27dc59c355570ec1f89e5` |

The exact owned formatting entry paths were `crates/arc-ipc/src/lib.rs`, `crates/arc-ipc/src/governor.rs`, `crates/arc-ipc/tests/governor.rs`, `crates/arc-storage/src/lib.rs`, `crates/arc-storage/src/governor.rs`, `crates/arc-storage/src/migration.rs`, `crates/arc-storage/tests/governor_migration.rs`, `crates/arc-storage/tests/managed_exec_migration.rs`, `crates/arc-storage/tests/memory_pressure.rs`, `crates/arc-storage/src/migration/tests/acceptance.rs`, `crates/arc-storage/src/jobs/tests/migration.rs` and `crates/arc-storage/src/job_meter/tests/migration.rs`.

Resolved attempts are retained as findings rather than erased completion claims:

- Offline fetch exited 101 because ts-rs was absent; the authorized locked fetch succeeded. No credentials or global toolchain route was invented.
- Initial TypeScript test exited 1 with `TypeError: undefined is not an object (evaluating 'ts.ScriptTarget.ES2022')`; the installed TypeScript 7 package exports a native CLI rather than the old default compiler API. Replaced the API use with the existing pinned native tsc executable; no additional compiler dependency.
- Desktop typecheck initially exited 1 on `NonSharedBuffer | undefined` under noUncheckedIndexedAccess; a readonly tuple gives the before/after artifact snapshots exact positions. Final typecheck passes.
- Migration-library check initially exited 101: 21 passed, 3 failed because current-registry assertions expected schema 5 (including table/object counts). Updated only current schema expectations to 6 / the additive inventory; legacy hash/history and data-preservation assertions remain. Final 24 focused migration tests pass.
- Focused Clippy initially exited 101 on `large_enum_variant` for GovernorLaunchResult::Reserved. Boxed that nested result in Rust; JSON is unchanged and reserved/unreserved ACK regression checks pass. No lint allowance was added.
- Inspection of the maintained coverage validator found it rejects every ignored native test. Removed the initial ignored generator entry before collection; explicit environment-gated generation now executes an ordinary nonmutating agreement check in default runs. Final IPC output has zero ignored tests; no coverage collection was attempted or failed.
- A patch context failed after formatter reflow; reread the located test file and applied the corrected patch. That failed patch made no partial source change.

Acceptance checklist for task-5:

- [x] Compiled strict additive switch/capability/current-client contracts; default/CAS/identity/replay/conflict/uncertainty, transaction and owner-authentication obligations frozen.
- [x] Actual Rust-derived TypeScript declarations and fixtures, pinned dev-only exporter, nonmutating agreement and compiler-backed structural checks.
- [x] Additive registered schema, real authoritative getter, upgrade/reopen/rollback/integrity tests and five original migration hashes preserved.
- [x] Existing coverage preflight succeeds without collection; Governor contract enters maintained identity/reuse with historical baselines/floors preserved.
- [x] Exact interfaces, ownership transfers, report pointers, native case inventory, traceability format and unchanged final commands recorded. Only explicit owned paths are staged for transport; preceding staged artifacts remain intact.

Runtime/client implementation, production owner-authenticated durability/launch proof, all GV-N cases, G01–G13 interaction execution, complete SRS traceability, final six gates/coverage, affected desktop build/smoke and independent QA are subsequent saved-task work, **not** completed by this shared-contract delivery. Broader migration-version assumptions outside the focused storage checks remain subject to their owning runtime/client/integration checks. No external-effect or broader SRS completion claim is made.

### Phase-count reporting handoff

These are explicit pending accounting responsibilities, not invented completion totals. The dedicated matrix and its parser reconcile actual numbers before the Chief reports them.

| Phase | Task-5 accounting status |
| --- | --- |
| Phase 0 | Pending dedicated inventory/count reconciliation. |
| Phase 1 | Pending dedicated inventory/count reconciliation; this Governor slice alone is not the complete phase. |
| Phase 2 | Pending dedicated inventory/count reconciliation. |
| Phase 3 | Pending dedicated inventory/count reconciliation. |
| Phase 4 | Pending dedicated inventory/count reconciliation. |
| Phase 5 | Pending dedicated inventory/count reconciliation. |

The first document-integrity script found the phase table used only an aggregate Phase 0–5 label; explicit phase labels above fix that reporting gap without asserting counts. The first staged diff check found ts-rs trailing spaces before generated JSDoc newlines; fixed in the Rust generator with deterministic line-end normalization, then explicitly regenerated. No generated type was hand-edited. A successful preliminary post-report preflight was performed, but the final whitespace/doc repairs require another preflight on the delivered frozen inputs; no collection occurs in either run.

Post-hygiene repair checks all returned exit 0 (same mission cwd):

| Command/check | Outcome | Raw log / SHA-256 |
| --- | --- | --- |
| Explicit environment-gated Rust generation | 1 passed, no ignored helper | `target/governor/task5/generate-hygiene.log` / `ae9cd7ab2b3b27105914daad669c896f7f8357be0a9c8ba8081d289c8c3f1e93` |
| Affected IPC library/Governor/policy/Traffic test command above | 77 passed, 0 failed, 0 ignored | `target/governor/task5/ipc-hygiene.log` / `60114d6d142d1f3d0f569fc4b748bf9c92de9e9c9647141fae62321b3ce83d81` |
| Affected two-crate all-target Clippy command above | Passed with -D warnings | `target/governor/task5/clippy-hygiene.log` / `9966716359c5b14452c6e717cb7f9c21cecef55f243c38727a5905c47bdbe75e` |
| Bun Governor structural test command above | 2 passed, 0 failed | `target/governor/task5/typescript-hygiene.log` / `6f73a3699b7f7bff8ed26188a9b3b92991f0715fd71100cb483c043e59e0e3ac` |
| Affected three-file Biome command above | Passed | `target/governor/task5/biome-hygiene.log` / `8f540173d29040a8e370759d2c3495d6e2279c1f051649b6651434490bfb6234` |

The delivered contract is frozen after this append. A final preflight on these frozen bytes and staged-byte/whitespace/provenance checks are the remaining task-5 completion checks; their process outcomes are returned with the handoff, without rewriting this frozen input or collecting coverage.

## Task-5 attempt-2 repair and consumer handoff

Status: shared-contract repair completed; final-input coverage preflight and final Git provenance checks follow the report freeze below. This section supersedes the earlier pre-dispatch freeze only to resolve the supplied review finding. Runtime/client/native/final-gate/QA work remains with its saved downstream owners.

The review identified a real response-schema gap: Governor accepted wrapped Status and StatusV2 requests, but only wrapped traffic::JobResult, which cannot carry valid RESERVED or RUNNING-with-queue_wait_ms status replies. Added GovernorJobResult::Status { result: traffic::StatusResult }, serialized exactly as `{ "state":"STATUS", "result":<unchanged legacy status object> }`. Its validation delegates to the existing status validator. The interface section above now explicitly pairs both status requests with this response wrapper. Other job operations keep TRAFFIC results; legacy traffic.job replies and both original Traffic enums are unchanged.

The two deterministic regression tests exercise both request spellings and every one of the eight legacy status variants, preserving nested status shape. They reject invalid sequence/time/resource values, invalid outcomes, nonobject results/reservations, missing/extra fields, owner-only permit fields and routing the status-specific RESERVED/RUNNING shapes through TRAFFIC. No production handler, capability advertisement or authentication behavior is enabled by this repair.

Only these three paths changed in this retry: `.arc-verify/governor/contract.md`, `crates/arc-ipc/src/governor/execution.rs`, `crates/arc-ipc/tests/governor.rs`. The other 26 staged paths present at entry retain exactly their entry bytes, including all manifests/locks, generated TS/fixtures, schema/registration, tooling, the Chief strategy and both Director handoffs. The original task-5 deliverable inventory and its ownership transfer above remain in effect. No ownership/interface conflict remains for consumer dispatch.

### Reproducible regression evidence

```text
TDD_EVIDENCE
command: CARGO_BUILD_JOBS=2 cargo test -p arc-ipc --test governor governor_status_route_round_trips_legacy_status_payloads -- --exact
RED: exit 101; valid RESERVED reply rejected as INVALID_REQUEST before the fix (0 passed, 1 failed).
GREEN: exit 0; the same regression passed after the fix (1 passed, 0 failed).
```

Full command logs stay under `target/governor/task5/attempt2/`; these compact outcomes and hashes are the Git-visible handoff. Commands run serially in the mission workspace with CARGO_BUILD_JOBS=2 for Cargo, no subagents, and a 7200000 ms per-check allowance. Mission limits/deadline remain unchanged. Host recheck: macOS arm64, 8 CPUs, approximately 86.8 GB free. No toolchain/dependency installation was needed in this retry.

| Check / exact command | Exit | Result | Raw log SHA-256 |
| --- | ---: | --- | --- |
| Regression RED command above | 101 | Expected reproduction of the supplied defect. | `2fab074be3da91c63d750737e6e764ca756d8eadf9a302288e82f55b555aee84` (`status-red.log`) |
| `cargo test -p arc-ipc --test governor governor_status_route_round_trips_legacy_status_payloads -- --exact` | 0 | 1 regression passed after the additive STATUS result was implemented. | `3881c2e8dd4bfd0fc103a2c00665179f69755fb61031fe8de42b9ce4d3d55bdd` (`status-green.log`) |
| `cargo test -p arc-ipc --lib --test governor --test memory_pressure --test traffic` | 0 | 79 passed: 58 library, 9 Governor, 3 memory-policy and 9 Traffic; 0 failed, 0 ignored. Both ordinary Rust generation/agreement checks remain nonmutating. | `db2944a6722765b89352bfb9f341abbe68c5e7d90ab935067dcb68e54bf34a8e` (`ipc.log`) |
| `cargo test -p arc-storage --test governor_migration --test managed_exec_migration --test memory_pressure` | 0 | 31 passed: 5 Governor migration/read, 6 managed-exec migration and 20 memory-pressure; 0 failed, 0 ignored. | `2b6a284753f6c5bbfcc44aea820f471a112212aa2a2e4dbc29fdf04fc2708f47` (`storage.log`) |
| `cargo test -p arc-storage --lib migration` | 0 | 24 focused migration tests passed; 75 unrelated tests filtered, 0 failed, 0 ignored. | `9b47944731de8dbc48fcbc37bbfc1e716c5299b3948f751b443c81899186f49f` (`migrator.log`) |
| `cargo clippy -p arc-ipc -p arc-storage --all-targets -- -D warnings` | 0 | Both affected crates and all targets pass with -D warnings. Existing ts-rs serde-attribute diagnostic remains; no new suppression. | `0e0d81ce3c0d6a2a1cc66027e8bc89f2e1570c33ebe0e708c7ce5d3657cd8f16` (`clippy.log`) |
| `bun test apps/desktop/tests/governor-contract.test.ts` | 0 | 2 structural agreement tests passed; serialized Rust switch fixtures compile against generated TS, including negative cases; artifacts unchanged. | `e7c75a2090b039f9e31e39098a3b781cfd5530013a2cc30e04edb941a88f37ce` (`typescript.log`) |
| `bun run desktop:typecheck` | 0 | Both desktop no-emit TypeScript projects pass. | `7c3cb6045e4508ad4ca264a0bdbb3207275c78410b843909c0ea071a97268c0f` (`desktop-typecheck.log`) |
| `bun test scripts/harness/coverage-reuse.test.ts scripts/coverage/hygiene.test.ts` | 0 | 20 synthetic identity/reuse/hygiene tests passed; 0 failed. These are receipt-validation tests, not a coverage collection. | `bb377df7e8c0d0afeb892a75db9d8d7066e620c22668469f34dc3ccd5548e340` (`identity-tests.log`) |
| `rustfmt --edition 2021 --check crates/arc-ipc/src/governor/execution.rs crates/arc-ipc/tests/governor.rs` | 0 | Both repaired Rust files pass formatting. | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` (`rustfmt.log`) |

The migration byte comparison was rerun against accepted HEAD `08e7e526cce7e03ca9264afc926ed2e878dfde6d`: all five original files match the hashes in Immutable migration inputs above. An initial ad hoc preservation script used the nonexistent name `0004_host_meter.sql` and exited 1; rerunning with the contract's actual `0004_job_meter.sql` passed. This was a check-path mistake, with no source or migration change.

Skills used in this retry: oma-backend primary for the strict IPC repair/regression; oma-db for migration and durable-state contract checks; oma-frontend for the Rust-derived consumer agreement and no-emit checks. Serena remains unavailable; only scoped reads of already located source paths were used. No browser, screen check, GUI workaround, mission-state operation or external delivery occurred. `av.yaml` remains access-denied/uninspected.

Acceptance remains bounded to task-5: compiled durable setter/authentication/idempotency rules and additive execution DTOs; Rust-derived TS and nonmutating agreement; passing focused migration/contracts and original-byte preservation; maintained coverage preflight without collection; exact consumer/ownership/report/native/traceability/final-command handoffs. The runtime setter and routes, CLI/desktop controls, synchronized native acceptance, final 80% per-crate measurements, traceability matrix and independent QA are not implemented or claimed complete by task-5.

### Attempt-2 preparation receipt and final freeze

`ARC_COVERAGE_OUTPUT_ROOT=target/governor/task5/attempt2 CARGO_BUILD_JOBS=2 bun scripts/coverage/run.ts --preflight-only` returned exit 0 after the repair/report above: pass=true, coverageCollected=false, 594 recorded inputs and all eleven workspace crates validated. Pinned Bun 1.4.4, Rust 1.95.0, cargo-llvm-cov 0.6.21, matching LLVM tools, locked/offline Cargo metadata and production-source inventory passed. Every nested command receipt exited 0. No coverage report, collection receipt or reuse receipt was generated. Raw directory: `target/governor/task5/attempt2/coverage/runs/2026-11-12T19-15-11-389Z-ba57992e-7356-48a0-b12d-4a6215a6d65d`; command-log SHA-256 `5e1d98f3b0e439f77db0b2649cce98992b4356f0921ed0d2852e0b909962bc9f`.

This preparation identity predates only this receipt/freeze append. The final-input preflight is rerun after this file is staged and frozen; its input map is then compared with current bytes through the maintained `requireInputIdentity` helper. The final handoff returns its actual outcome and digest without editing this frozen file again. This distinction preserves self-referential evidence integrity and does not turn preflight into measurement or final gates.

Task-5 acceptance checklist: [x] compiled shared strict contracts with the repaired status response; [x] Rust-derived switch agreement; [x] focused migration/contract checks and original hashes; [x] zero-collection preparation; [x] Git-visible exact interfaces and downstream ownership. Required final-input identity/provenance checks complete only when the following final handoff reports their observed results. No remaining shared-contract blocker is known; native/runtime/client/final-gate work remains explicitly pending with its owners.
