# Technical Director handoff

Status: completed contract task; product implementation and verification are pending.

Mission: `1f8a7d5e-7262-4d6d-acdd-bfed3071e32b`. Task: `task-1`. Attempt: 1. Actor: `technical-director`. Owned branch: `av-task/fdf57c1d58d8223e-7afaa346b4bf92bf-1`.

The deliverable is [the shared contract](../../../docs/schema-compatibility-contract.md), revision 1. It defines the public storage API, compiled registry, validation and error states, atomic startup, read-only assessment, protocol-1 fields, legacy fallback, ownership and deterministic fixture obligations. No product stubs were written.

## Inspection scope and provenance

| Evidence | Observed value |
| --- | --- |
| Accepted committed baseline | `bf28a35b594f5ae83ac97c216bd3b09e60a2dfa1` |
| Baseline product tree | `cde83859dbe37796b62b134384627f657eeae010` |
| Actor-local inspected snapshot HEAD | `5cbb04f332d9e3144ed4c71f99bc145d06451bdb` |
| Actor-local snapshot tree | `cde83859dbe37796b62b134384627f657eeae010` |
| Supplied snapshot and worker ref names | Both failed local `git rev-parse --verify`, exit 128 |
| Scoped initial status and staged diff | No changes in owned contract, crates, manifests or harness |
| Inspected product/source inputs | 15 exact known paths; every input matched baseline bytes |
| Global status/diff | Returned exit 0 while emitting `av.yaml: Operation not permitted`; global cleanliness is not established |

The initial local snapshot is a distinct commit from the baseline, with the same product tree. Contract/report edits are Actor-local uncommitted deliverables; they are not claimed to exist at the accepted baseline. The integration owner must use its delivery receipt and inspect actual integrated paths. An Actor-local file or commit does not establish tracking or content in another checkout.

[source-inspection.json](source-inspection.json) records cwd-derived scope, revisions, per-file SHA-256, baseline blobs, sizes, line counts, exact Git argv, exits, stdout and stderr. It preserves the actual supplied/observed hashes rather than substituting another checkout's HEAD.

`0001_phase0.sql` is 3,917 bytes and 154 lines. Its baseline and inspected SHA-256 is `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`; baseline Git blob is `8d8e5d5eb4ea9706119c054dc77cdacbdb3725e8`. It was read and hashed, never edited.

Serena tool discovery returned no matching callable tools. Inspection used direct reads of paths supplied by the task, skills and existing documents; no native project-wide code search substituted for Serena. An attempted read of `crates/arc-daemon/src/diagnostics.rs` failed because that path is absent; diagnostics were inspected at the supplied actual worker execution path instead. Prior architecture documents were inspected under `docs/decisions/`. Their Turso team-ledger/AV orchestration scope does not replace ARC's Rust storage stack. Legacy `docs/design-docs/` and `docs/plans/designs/` directories were absent. No prior schema-compatibility output directory existed at initial inspection.

## Architecture problem and decision

Storage currently validates only `0001_phase0`, compares one checksum, and insists on one history row. Worker diagnostics read integrity and row count. IPC and CLI separately equate count 1 with acceptable schema. Adding migrations without changing those interfaces would reject valid schemas and obscure invalid history.

Use a small ordered static registry, a shared storage assessment and an explicit optional doctor schema report. Keep migration execution in the existing startup connection and diagnostics in the existing single worker. No module boundary or deployment topology changes; no structural diagram is needed.

| Option | Implementation and change cost | Operational and maintenance cost | Decision |
| --- | --- | --- | --- |
| Existing rusqlite transaction plus compiled Rust registry and sha2 | Small storage module, exhaustive adapter mapping and focused fixtures; reuse current APIs | No new service, generator or dependency; new migrations require reviewed source entries | Select |
| Migration framework/ORM with its own journal and CLI | New dependency/API, legacy-journal adaptation and compatibility work; does not remove doctor integration | Additional version/pinning, journal and toolchain maintenance; possible generated artifacts | Reject for this mission |
| Runtime discovery of migration files | Requires packaging/discovery/error handling and external input integrity | Binary behavior depends on installed files; additional distribution and support burden | Reject; violates compiled-input goal |

The selected design has no demonstrated need for changes to Cargo manifests or lockfile. Cost comparison is qualitative; API/Actor prices and measured implementation time are unknown. Do not turn this recommendation into a numerical cost-saving claim.

## Interface details for implementers

Storage adds `Migration`, `MIGRATIONS`, `SchemaStatus`, `SchemaCompatibility`, `MigrationError` and `assess_schema(&Connection)`. Existing `StorageResult`, `open_default`, `open_and_migrate` and `MIGRATION_ID` stay available. The registry-aware engine remains private. Fixtures call that engine in test scope, without runtime SQL configuration.

Registry versions are contiguous and ordered; history is validated completely by canonical numeric version, exact ID and exact SQL checksum. No sorting repairs a malformed registry. No checksum/history repair or inferred baseline stamping is permitted. Absent history over recognizable ARC objects rejects even when application tables have no rows.

An independent witness is necessary for a deleted final history record: a prefix alone cannot distinguish an unapplied suffix from a deleted suffix. The contract uses native `PRAGMA user_version` inside the same transaction, preserving all history columns and original SQL bytes. Existing unmarked version-1 databases have a narrowly defined legacy acceptance path; their validated first open establishes the witness. This is a design requirement, not an observed implementation. The baseline source does not assign user_version; default-zero treatment follows that source and SQLite's initial value.

The mutation boundary is `BEGIN IMMEDIATE` → reread all history/witness → validate → apply all pending SQL/history → set final witness → single commit. Invalid input rejects before history/data writes. Busy timeout retains existing startup behavior. Failed/interrupted execution must roll back all pending changes. A later opener acquires its own lock and rereads committed state.

Backend owns separate Serde wire types, avoiding a storage-to-IPC or IPC-to-storage dependency. `DoctorReport.schema` is optional with default None; a present successful report requires a verified applied/supported version and consistent real row count. Unknown statuses cannot default to compatible. An incompatible live assessment returns `SCHEMA_INCOMPATIBLE` with structured diagnostics; actual integrity and operational failures retain their typed routes.

Old client/new production daemon compatibility keeps truthful count 1 and existing fields. A new client receiving an old report labels compatibility detail unavailable. An old count-only validator still rejects fixture version 2; protocol-1 JSON compatibility cannot erase that semantic limitation. The lower-registry test proves explicit downgrade rejection and preservation, never downgrade execution.

## Database responsibility review

Affected existing SQL creates tables, indexes and declarative constraints. It contains no affected triggers, routines, DB jobs, ORM hooks or external effects to relocate. Migration policy, version ordering and compatibility remain application-owned Rust logic. Keep SQLite PK/FK/UNIQUE/NOT NULL enforcement and atomic history insertion. No schema normalization, business policy, retention, backup product or access-control change is needed for this scoped contract.

External view: startup acceptance and diagnostic status. Conceptual view: each immutable migration version has one committed audit tuple. Internal view: existing history columns plus native version witness, with no production schema addition. Checksums establish compiled-input/history agreement; they do not authenticate arbitrary physical schema edits or constitute a backup/restore certification.

## Assumptions, risks and Chief handoff

- Production remains at schema version 1; additive upgrades are isolated fixtures. If a real production migration is later requested, it needs its own reviewed SQL and history entry.
- Native user_version is unused by inspected ARC code. The storage owner must retain the legacy exception and test unexpected witness disagreement, removed suffix records and rollback of witness writes. Do not replace this with a count-only implementation.
- The legacy zero-witness exception cannot prove that external tools never edited schema objects. The mission proves orderly upgrades and specified corruption rejection, not forensic schema authentication.
- The new schema error includes both envelope error and diagnostic result. Backend must test this explicitly and preserve existing errors and old-client decoding; generic CLI error handling currently discards results.
- Read-only assessment needs one consistent snapshot. Worker cancellation/error mapping must preserve native SQLite interruption and extended error codes; no additional connection or worker is justified.
- Identified needs outside task-1 ownership: storage implementation/tests; IPC/daemon/CLI implementation/tests; user documentation alignment; integrated source-bound verification; independent oma-qa. These remain assigned owners' work, with the Chief coordinating any interface revision.
- Tool availability and inaccessible `av.yaml` limit inventory, not the documented known-path interface decision. No denied command was bypassed through GUI or an alternative permission route.

## Checks and acceptance evidence

| Check performed | Outcome |
| --- | --- |
| Exact HEAD/tree and baseline-tree resolution | Exit 0; values above |
| Scoped initial status; staged diff | Exit 0; empty for assigned source/contract scopes |
| `git diff --exit-code bf28a35b594f5ae83ac97c216bd3b09e60a2dfa1 -- crates Cargo.toml Cargo.lock scripts/harness/validate.sh` | Exit 0; inspected product inputs unchanged |
| Known-path SHA-256 and baseline-byte comparison | Exit 0; 15/15 inputs match |
| Named snapshot/worker refs | Exit 128 individually; unresolved locally |
| Global status/diff | Exit 0 with access diagnostic; no global-clean claim |
| Skill architecture/DB checklist review | Contract covers APIs, alternatives, integrity, ownership, concurrency, assumptions, risks and validation handoff; unaffected full-modeling items omitted |

Task acceptance mapping:

1. Exact public types/functions, wire fields, legacy semantics and single-writer boundaries: contract sections Storage API, Interface ownership and Protocol-1 report and CLI contract.
2. Ordering, full history validation, rollback, concurrency, absent history and downgrade rejection without discovery: Registry and stored-history rules, Version witness, Atomic startup and Deterministic fixtures.
3. Source references, hash, assumptions and maintainability decision: this report and source-inspection.json. Implementation and test success are expressly pending.

Contract artifact checks and final ownership checks are recorded in [artifact-checks.json](artifact-checks.json). This documentation task did not run cargo fmt, clippy, tests or harness. Required final commands remain exactly `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and `./scripts/harness/validate.sh`. Retain the actual harness coverage result. No acceptance command was changed, skipped by a substitute or claimed from historical receipts.

Skills used: oma-architecture (recommendation comparison, API evolution and checklist), oma-db (migration transaction/integrity design and checklist). Read the shared execution policy, applicable always-on i18n rule and both skill execution protocols. The task's explicit owned output paths override the skills' generic ADR/result locations; no additional OMA run or mission state was created. No new workflow, agent delegation, build, dependency installation, browser/screen check or external delivery occurred.
