# SQLite migration and compatibility contract

Status: implementation contract for mission `1f8a7d5e-7262-4d6d-acdd-bfed3071e32b`, task-1, revision 1. This document specifies interfaces; it does not establish implemented behavior or passing tests. The Chief owns integration decisions. Storage and backend owners implement this contract in their assigned tasks.

## Scope and source

Replace the single-migration assumption with ordered, checksum-verified startup migration and read-only diagnostics. Retain the existing Rust workspace, rusqlite, sha2, Tokio, Serde and Clap. Keep production inputs compiled, protocol version 1, one bounded SQLite worker, existing instance identity, independent hello/health dispatch and shutdown ownership.

Accepted baseline: `bf28a35b594f5ae83ac97c216bd3b09e60a2dfa1`, tree `cde83859dbe37796b62b134384627f657eeae010`. Actor-local inspected HEAD: `5cbb04f332d9e3144ed4c71f99bc145d06451bdb`, with the same tree. These commits are distinct. Inspected source files match the accepted baseline; later task edits must be verified in the integrated worktree.

Preserve [0001_phase0.sql](../crates/arc-storage/migrations/0001_phase0.sql) byte-for-byte: 3,917 bytes, SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. Production remains version 1 for this mission. No production 0002 is needed to demonstrate the engine.

Current integration points are [storage](../crates/arc-storage/src/lib.rs), [worker execution](../crates/arc-daemon/src/db_worker/execution.rs), [IPC](../crates/arc-ipc/src/lib.rs), [CLI](../crates/arc-cli/src/main.rs), [startup](../crates/arc-daemon/src/main.rs) and [lifecycle](../crates/arc-daemon/src/lifecycle.rs). Inspection hashes and limitations are in [source-inspection.json](../.arc-verify/schema-compatibility/technical/source-inspection.json).

## Interface ownership

| Interface or files | Sole writer | Consumer and boundary |
| --- | --- | --- |
| This contract; `.arc-verify/schema-compatibility/technical/` | Technical Director | Storage, backend and Chief; contract changes require communicating the exact revision |
| `crates/arc-storage/src/**`, storage tests and fixture SQL | Migration Engineer using oma-db | Owns all storage types, registry, assessment, error display and engine |
| `crates/arc-storage/migrations/0001_phase0.sql` | No writer in this mission | Immutable input; storage owner proves preservation |
| `crates/arc-ipc/src/**`, including doctor wire types and dispatch | Backend Engineer using oma-backend | Maps storage assessment to IPC types; storage never imports IPC |
| `crates/arc-daemon/src/**`, daemon tests, `crates/arc-cli/src/**`, CLI tests | Backend Engineer | Owns worker adapter, startup messages and CLI rendering; preserves runtime guarantees |
| User-facing compatibility documentation other than this contract | Assigned documentation owner, coordinated by Chief | Uses the interface below; does not redefine it |
| `.arc-verify/schema-compatibility/verification/`, final evidence and delivery | Chief or explicitly assigned verifier | Binds integrated sources, commands, logs and acceptance matrix |
| `.arc-verify/schema-compatibility/review/` | Independent Reviewer using oma-qa | Read-only product review after integration |
| Manifests, lockfile, shared generated registries | Chief retains coordination; no writer needed by this design | Report a demonstrated need before any ownership handoff |
| Harness, `.agents/` definitions, task instructions and mission state | No task-1 writer | Preserve unchanged |

Storage owns `arc_storage::SchemaStatus`; backend owns `arc_ipc::SchemaStatus`. They deliberately have separate representations to avoid a new crate dependency or Serde dependency in storage. The daemon adapter exhaustively maps the storage enum to its wire counterpart. No Actor edits another owner's types to unblock itself.

## Storage API

Retain existing signatures and behavior for path resolution and connection configuration:

```rust
pub type StorageResult<T> = Result<T, Box<dyn std::error::Error>>;
pub fn open_default() -> StorageResult<rusqlite::Connection>;
pub fn open_and_migrate(path: impl AsRef<std::path::Path>)
    -> StorageResult<rusqlite::Connection>;
pub const MIGRATION_ID: &str = "0001_phase0";
```

Add these public, non-Serde storage types and assessment function:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    pub version: u32,
    pub id: &'static str,
    pub sql: &'static str,
}
pub const MIGRATIONS: &[Migration] = &[/* compiled entries */];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaStatus {
    Compatible, UpgradeRequired, MissingHistory,
    ChecksumMismatch, InvalidHistory, UnsupportedNewer,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaCompatibility {
    pub status: SchemaStatus,
    pub applied_version: Option<u32>,
    pub supported_version: u32,
    pub history_verified: bool,
    pub schema_migrations: i64,
}
#[derive(Debug)]
pub enum MigrationError {
    InvalidRegistry(&'static str),
    Incompatible(SchemaCompatibility),
    Sql(rusqlite::Error),
}
pub fn assess_schema(connection: &rusqlite::Connection)
    -> Result<SchemaCompatibility, MigrationError>;
```

`MigrationError` implements Display and Error, preserving the SQLite error through `source()` and `Sql`. Existing boxed startup results can contain it without changing caller signatures. Assessment returns an incompatible **value** for readable invalid history, not `Incompatible`; the latter is the migration engine's refusal to open that value. `InvalidRegistry` is a binary/input defect, not database corruption. Actual SQL failures remain typed `Sql`, never guessed to be checksum failures.

The production registry contains exactly `Migration { version: 1, id: MIGRATION_ID, sql: include_str!("../migrations/0001_phase0.sql") }`. Module-relative include paths may change when storage is split. Checksum is lowercase hexadecimal SHA-256 of the exact UTF-8 SQL bytes. Do not normalize whitespace or line endings, or hash a parsed/reformatted statement.

The common engine and registry-aware assessment are private functions taking `&[Migration]`; the storage owner may choose their private names. All public production entry points select only `MIGRATIONS`. No public fixture-registry setter, environment override, runtime SQL path, migration RPC, plugin scan or build-time generator is introduced.

## Registry and stored-history rules

1. Validate the whole registry before executing SQL: nonempty; version sequence exactly `1..=N`; nonempty unique IDs; nonempty SQL. IDs use a decimal version prefix padded to at least four digits, an underscore and a nonempty ASCII lowercase/digit/underscore suffix. The numeric prefix equals `version`; versions fit SQLite's positive signed 32-bit `user_version`. Reject duplicate, reversed and skipped versions and ID/version disagreements. Do not sort or repair a malformed registry.
2. Keep `schema_migrations(id TEXT PRIMARY KEY, applied_at INTEGER NOT NULL, checksum TEXT NOT NULL)` unchanged. The first compiled SQL creates this table as it does today. Do not create it outside the migration transaction or use `IF NOT EXISTS` to conceal incomplete initialization.
3. Read the complete history, including IDs, timestamps and checksums. Compare in numeric version order, never insertion order, timestamp order or count alone. IDs must have canonical prefixes, be unique by version, and match the registry's exact ID for every supported version. Validate timestamp/column types and non-null values; a timestamp is an audit value, not an ordering key. Reject malformed history instead of coercing it into a supported prefix.
4. Stored versions must start at 1 without gaps. Compare **every** known checksum, including migrations with no pending successor. A valid history is an exact prefix of the compiled registry. An unknown ID within the supported range is `InvalidHistory`; an otherwise valid prefix followed by a version above the supported maximum is `UnsupportedNewer`. Unknown future checksum content cannot be verified by this binary.
5. A missing history table or empty history alongside any recognizable ARC application object is `MissingHistory`, including an empty application table. Recognizable names include all 13 Phase 0 application tables and its named indexes. Inspect object names in `sqlite_schema`; do not infer a fresh database from application row counts. An existing empty history table also requires rejection, since no committed ARC initialization produces it. An unrelated object does not by itself authorize adoption of an ARC schema; any DDL collision still fails atomically.
6. A database with no history, no recognizable ARC objects and version witness 0 is fresh. It may initialize. A nonzero witness with absent history is never fresh. No history reconstruction, synthetic checksum, history deletion or automatic baseline stamping based only on table resemblance is allowed.

## Version witness and compatibility states

Use SQLite's existing `PRAGMA user_version` as an independent version witness; this changes no table or production SQL file. Write it to the final applied version **inside the same transaction** as pending SQL and history inserts. This catches deleted trailing history, which a valid-prefix comparison alone cannot distinguish from an unapplied upgrade. Read it during assessment, including doctor; assessment never sets it.

The accepted original database has witness 0. Permit that legacy form only with exactly the validated version-1 history (the original production ID and checksum). A successful startup stamps witness 1 in its migration transaction before returning, or stamps the final version if an upgrade is pending. A read-only diagnostic of this validated legacy form can report compatible without stamping. A zero witness with a claimed later applied version is invalid history. This exception preserves existing databases; it does not attest that their physical schema has never been edited outside ARC.

| Status / wire spelling | Meaning | `history_verified` / applied version | Startup action |
| --- | --- | --- | --- |
| `Compatible` / `compatible` | Complete supported registry matches history and witness, or validated original legacy form | true / exact supported version | Return connection after commit |
| `UpgradeRequired` / `upgrade_required` | Fresh database, or valid shorter prefix whose witness matches it | true / 0 for fresh, otherwise last verified version | Apply remaining entries atomically |
| `MissingHistory` / `missing_history` | Gap, absent/empty history over ARC objects, or witness shows a deleted trailing record | false / null | Reject |
| `ChecksumMismatch` / `checksum_mismatch` | Supported stored ID has a different checksum | false / null | Reject |
| `InvalidHistory` / `invalid_history` | Malformed history, wrong known ID or witness inconsistent with recorded history | false / null | Reject |
| `UnsupportedNewer` / `unsupported_newer` | Readable history or witness advertises a version above this binary's maximum | false / advertised maximum version | Reject; no downgrade |

`supported_version` is the final compiled registry version, independent of the row count. `schema_migrations` is the actual stored row count (zero when the table is absent), never the supported version disguised as a count. `applied_version` is not presented as verified in an incompatible report. For unsupported newer versions it is the advertised maximum of parsed history and witness, explicitly unverified.

Validation precedence is deterministic: invalid registry first; unreadable SQL errors propagate; malformed stored layout/values become `InvalidHistory` when safely inspectable; then earliest known-prefix defect in numeric order (gap, ID, checksum); then unsupported future history/witness; then missing trailing history indicated by the witness; then other witness disagreement; then compatible/upgrade-required. A known checksum defect is not hidden by a later future row. Once an incompatibility is found, no migration, witness update or history write is permitted.

Read-only assessment uses one SQLite read snapshot for object detection, history and witness, so concurrent commits cannot produce a mixed report. It may begin/end a deferred read transaction when not already in a transaction; when called by the migration engine it uses that engine's existing transaction. No nested BEGIN, write lock or PRAGMA assignment is needed for diagnostics. Layout problems that prevent safely reading history may propagate typed SQLite failures; do not invent count/version values after a failed read.

## Atomic startup and concurrency

1. Preserve local-file validation, parent creation, foreign keys, WAL and the existing five-second startup busy timeout. Validate the registry before migration side effects. Physical WAL/connection setup is distinct from the application-data/history preservation guarantee.
2. Acquire `BEGIN IMMEDIATE` on the same connection before taking the authoritative history snapshot. A pre-lock observation cannot decide which migration is pending. A second upgrader waits within the existing timeout, acquires the write lock and rereads the committed history.
3. Assess the entire registry/history/witness under that transaction. Reject incompatible states without writes. A fresh database runs entry 1; a verified prefix runs only its pending suffix.
4. Execute each pending entry in order, then parameterize its `(id, unixepoch(), checksum)` insertion. Update the witness only after all pending entries complete. Commit SQL, history and witness once as one unit. No intermediate commits or per-version transactions.
5. On SQL error, cancellation/interruption or process exit before commit, leave no committed pending DDL, application updates, history rows or witness changes. A later normal open retries from the last committed prefix. Explicitly propagate a rollback failure if the API can observe one; never return a connection after a failed upgrade.
6. Compiled migration SQL must not contain transaction control, VACUUM, ATTACH, external I/O or connection-setting PRAGMAs. The engine owns the transaction. These are trusted reviewed source inputs, not arbitrary user SQL; no SQL parser dependency is needed. Future changes remain additive in this mission's fixtures. There is no DOWN SQL or downgrade command.

The declaration is still forward-only. Opening an upgraded two-version fixture with a lower supported registry is **downgrade rejection**. It must leave schema objects, application rows, full `(id, applied_at, checksum)` tuples and witness unchanged. Do not describe rejection as successful downgrade execution.

## Protocol-1 report and CLI contract

Keep `PROTOCOL_VERSION = 1`, framing, operation names, request defaults and existing response envelope fields. Add no required request parameter. Retain `integrity_check`, the accurate `schema_migrations` count and existing doctor capability fields. Add one optional field, `schema`, to `arc_ipc::DoctorReport` and its serialized diagnostic result:

```json
{
  "integrity_check": "ok",
  "schema_migrations": 1,
  "schema": {
    "status": "compatible",
    "applied_version": 1,
    "supported_version": 1,
    "history_verified": true
  }
}
```

Backend owns `arc_ipc::SchemaReport` with fields `status: SchemaStatus`, `applied_version: Option<u32>`, `supported_version: u32`, `history_verified: bool`. Its enum uses the six spellings above and an `Unknown` variant using Serde's unknown-variant fallback. Unknown means incompatible/unrecognized; it never becomes compatible. Storage's row count maps to the existing outer `DoctorReport::schema_migrations: i64`.

`DoctorReport::schema: Option<SchemaReport>` has `#[serde(default, skip_serializing_if = "Option::is_none")]`. Missing or null means details unavailable. Inside a present `schema` object, status, supported_version and history_verified are required; applied_version may be absent or null and then means unavailable. Compatibility still requires a non-null applied version. Do not default missing status or `history_verified` to success. Ignore additive unknown JSON fields as existing Serde consumers do; malformed present reports fail validation.

The doctor dispatcher follows these rules:

- Integrity other than `ok` retains `INTEGRITY_FAILED`. Positive SQLite corruption retains the current corruption mapping; operational lock, queue, timeout, cancellation and resource errors retain `DB_BUSY`. Registry defects are operational failures, not evidence of corruption.
- A present compatible report requires `history_verified = true`, equal non-null applied/supported versions greater than zero, and an accurate count agreeing with the contiguous version count. It returns `status: "ok"` with the result including schema. Do not impose `schema_migrations == 1`.
- A present non-compatible or internally inconsistent schema report returns `status: "error"`, `error.code: "SCHEMA_INCOMPATIBLE"` (new constant `ERROR_SCHEMA_INCOMPATIBLE`) and the diagnostic result containing the schema and legacy fields. Preserve the existing envelope type; it already permits both `result` and `error`. This prevents a legacy count-only client from claiming success on an incompatible version-1 report. The CLI doctor path must retain that result instead of discarding it in generic error mapping.
- A legacy report without schema keeps its existing success criterion (`integrity_check == "ok"`, count 1), but a new CLI prints that schema compatibility details are unavailable. With any other count, the new CLI fails with `SCHEMA_DETAILS_UNAVAILABLE`. A legacy report cannot establish checksum verification.

CLI successful current-schema output retains the existing capability disclosures and adds `Schema compatible. Version 1. Migration history verified.` A compatible fixture version 2 prints version 2 and succeeds. Successful legacy fallback adds `Schema compatibility details unavailable (legacy daemon).` and never prints verified history. Schema failures print `SCHEMA_INCOMPATIBLE` plus the explicit status reason and advertised/supported versions when known, with nonzero exit. Use plain text with no mandatory prompt or color-only state. Preserve current status output and existing non-schema error behavior.

| Client / daemon pairing | Required behavior |
| --- | --- |
| Original client / new production daemon, schema 1 | Original request and decoder work; true count 1 and old fields satisfy its existing checks |
| New client / original daemon | Missing schema decodes as None; legacy success is labeled as lacking compatibility detail |
| Original client / fixture schema 2 report | Original decoder accepts added fields, but its count-only doctor rejects count 2; retain this known limitation |
| New client / compatible fixture schema 2 report | Explicit report succeeds without count-equals-one assumption |
| New client / unknown status or malformed present report | Fail explicitly; never infer compatibility from count 1 |
| Any supported client / unsupported protocol request | Existing protocol mismatch rejection precedes database work |

Startup still migrates before binding/serving IPC. A rejected startup therefore has no reachable doctor endpoint. Startup stderr names the storage reason: newer schema requires a binary supporting the advertised version; missing/changed history requires preserving the database and investigating a trusted copy. Do not suggest deleting records, rewriting checksums or running a downgrade. A subsequent unavailable CLI connection is not a successful compatibility diagnosis.

## Runtime integration boundary

The backend calls storage assessment from the existing fixed doctor job on the sole SQLite worker. It maps data into IPC types locally, without pooling, opening a second diagnostic connection, scheduling a background migrator or accepting SQL payloads. The read snapshot finishes before releasing job ownership.

Preserve worker progress/busy cancellation, checks before and after SQLite calls, finite admission, health bypass, absolute request deadlines, two configured Tokio runtime workers, instance lock/inode identity, common shutdown deadline, connection close and worker join. Map `MigrationError::Sql` back to the existing typed SQLite error route; preserve extended SQLite error codes. An incompatible assessment is a completed report, not DB_BUSY or a guessed integrity failure.

No startup responsiveness before readiness is promised. Health independence assumes transport admission is available; it does not promise service through full connection saturation. New SQL diagnostics remain read-only, bounded by existing execution cancellation, and use no separate maintenance thread.

## Deterministic fixtures and acceptance handoff

Storage unit tests inject immutable fixture slices into the same private engine. Fixture version 1 creates the unchanged history columns and an ARC-named record table (or uses the original compiled migration); fixture version 2 adds a column/table/index. Fixture SQL and synchronization hooks live under `cfg(test)` or test-only fixture paths and never enter production configuration. Preserve original SQL/hash tests separately.

Concurrency uses channels/barriers: upgrader A signals after acquiring its transaction; B signals its actual busy-handler/lock attempt; only then release A. Both must succeed with exactly one committed history tuple per version and preserved rows. A timeout bounds failure; elapsed sleep is not the synchronization proof.

Interruption coordinates an actual in-progress SQLite statement after earlier pending DDL/data changes but before commit. Use a test-only progress callback to signal statement execution, then `InterruptHandle` with an explicit handshake; clear the callback/handle state before retry. A child-process exit coordinated after pending changes and before commit may additionally prove crash recovery. Returning an injected Rust error before executing SQL alone does not establish SQLite interruption rollback. No runtime fault-injection switch is exposed.

The storage owner supplies cases for initialization, reopen, version-1 to version-2 additive upgrade, record preservation, repeat-open idempotency (including exact timestamps), serialized concurrent upgrade, failing SQL and interrupted-upgrade rollback, modified checksums, missing first/middle/trailing/all history, absent history over recognizable empty/nonempty schemas, invalid registries/IDs, newer-schema rejection and lower-registry downgrade rejection. Compare application rows, `sqlite_schema` objects, complete history tuples and witness before/after each rejection. Snapshot logical data, not WAL file byte identity.

Backend supplies protocol request/decoder fixtures, a frozen original client validator, new-client legacy fallback, schema status/error cases, daemon startup rejection and read-only live diagnostics. Retain existing deterministic worker admission, slow-database health/recovery, identity and shutdown cases. Split implementation/tests before any source file exceeds 500 lines.

After integration, the verifier runs exactly `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and unchanged `./scripts/harness/validate.sh`. Retain command logs, exits, source hashes, original SQL comparison and acceptance mapping under `.arc-verify/schema-compatibility/verification/`. Record actual unresolved harness coverage failure if it persists. Independent oma-qa reviews final integrated sources and evidence. This task runs none of those implementation gates and makes no product acceptance claim.
