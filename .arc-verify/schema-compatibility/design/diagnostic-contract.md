# Terminal schema compatibility contract

Status: completed design input for task-2; implementation and executable acceptance are pending with their owners.

## Ownership and inspected revision

This task owns only `.arc-verify/schema-compatibility/design/diagnostic-contract.md`. CLI, IPC, storage, startup, tests and shared documentation were inspected read-only. This file specifies the backend handoff; it does not establish that the proposed interface exists.

| Provenance item | Value and scope |
| --- | --- |
| Accepted committed baseline | `bf28a35b594f5ae83ac97c216bd3b09e60a2dfa1` |
| Baseline tree, verified with Git | `cde83859dbe37796b62b134384627f657eeae010` |
| Actor-local HEAD / supplied snapshot commit | `bffb0cfd41408c3e73d1af1f6553d6e438836064` |
| Actor-local HEAD tree / supplied snapshot tree | `cde83859dbe37796b62b134384627f657eeae010` |
| Supplied snapshot ref | `refs/agent-valley/task-evidence/fdf57c1d58d8223e-78686d2704d4d690-1/snapshot`; local resolution failed, exit 128 |
| Supplied worker ref | `refs/agent-valley/task-evidence/fdf57c1d58d8223e-78686d2704d4d690-1/worker`; local resolution failed, exit 128 |
| Initial scoped status / staged and unstaged diff | No changes under `.arc-verify/schema-compatibility`, `crates`, `docs` or `scripts`; assigned file absent |

All inspected inputs in the hash table below were compared byte-for-byte with their baseline versions and matched. The Actor-local HEAD differs from the accepted baseline commit despite the equal trees. This report is a new task-local file, absent from that baseline. Its presence in this worktree does not prove receipt in the integrated worktree. The Chief must verify delivered bytes after collection. Neither unresolved evidence ref is replaced with HEAD.

Serena discovery tools were unavailable in the tool inventory. Inspection used only supplied known paths, without repository-wide code discovery. No operator note changing prerequisites was supplied.

## Observed source behavior

These are static source observations at the inspected revision, not fresh execution results.

| Source and symbol | Observed behavior |
| --- | --- |
| `crates/arc-daemon/src/main.rs`, `main` | Acquires instance admission, calls `open_and_migrate`, then starts the IPC server. Failed migration returns before IPC is served. |
| Same file, `fail_database` / `sanitize` | Writes one newline-terminated `Database startup failed: <sanitized error>.` line to stderr and exits 1. CR/LF in the error are flattened. |
| `crates/arc-daemon/src/lifecycle.rs`, startup closure | Prints `arc-daemon ready. Socket: <socket>. Protocol: 1. Sweeper off.` after binding the service; flushes stdout. |
| `crates/arc-storage/src/lib.rs`, `open_and_migrate` | Compiles one SQL input, validates its checksum and requires exactly one history row. Missing/checksum-mismatched first entry shares one error; additional entries get unsupported-history failure. |
| `crates/arc-daemon/src/db_worker/execution.rs`, `execute` | Reads integrity and migration count through the existing SQLite worker. It does not report ordered-history compatibility. |
| `crates/arc-ipc/src/lib.rs`, `DoctorReport` / `dispatch` | Protocol 1; doctor report has `integrity_check` and `schema_migrations`. Dispatch accepts only integrity `ok` and count 1. |
| `crates/arc-cli/src/main.rs`, `doctor` | Repeats the integrity/count-1 check. Success discloses owner-only IPC and unconnected/unavailable capabilities. |
| Same file, `request` / `main` | Request errors are handled before success fields. Known integrity/protocol errors keep their codes; unknown codes, including `DB_BUSY`, become `IPC_ERROR`. Diagnostic errors exit 1 with empty stdout and one code line on stderr. |
| Same file, `status` / Clap definition | Status uses hello, without SQLite. Output is `running. Socket: <socket>. Protocol: 1. Sweeper off.`; color is disabled. |
| `crates/arc-daemon/src/connection.rs` and `lifecycle.rs` | Doctor uses bounded DB work. Hello/health bypass it; transport admission can still reject either. Busy is separate from positive corruption evidence. |
| `crates/arc-daemon/tests/startup.rs` | Existing assertion expects exactly `Database startup failed: Phase 0 migration checksum is missing or mismatched.\n`, exit failure and unchanged invalid checksum. This is an existing test assertion, not this task's test result. |
| `crates/arc-cli/tests/binary.rs` and `docs/bounded-ipc-recovery.md` | Discoverable commands, no ANSI, diagnostic output contracts and busy/recovery semantics are established responsibilities. Historical document test receipts are not fresh mission evidence. |

## Selected journey and inferred users

| Approach | Operator steps and tradeoff | Decision |
| --- | --- | --- |
| Automatic atomic migration at startup | Start once; readiness means startup completed; repeat opening needs no migration action. Rejection is explained on startup stderr. | Select the existing journey. |
| Separate preflight/migrate command | Adds discovery, command ordering and another action before normal startup. Could offer explicit control in a separately scoped product change. | Do not add. |
| Interactive wizard | Adds prompts, navigation and scripting complexity for a local daemon. | Do not add. |

Personas are inferred from existing CLI/startup responsibilities: a developer starting or updating ARC, an operator diagnosing a rejected startup, and an integration maintainer handling mixed client versions. No participant data establishes these personas. Fewer required commands and explicit reasons are hypothesized usability benefits, not measured task-time, retention, conversion or churn improvements.

Successful path: start the daemon, observe the existing readiness line, continue work. Doctor is optional confirmation. Repeat startup is idempotent. A startup rejection sends the operator to daemon stderr; asking doctor to explain that failure is not a reachable recovery step. Do not promise status responsiveness before readiness or under full transport saturation.

## Proposed terminal and machine contract

The following is proposed behavior for implementation. Examples use migration IDs as version labels, avoiding an assumption that a row count identifies a version. `<database>` and `<supported>` mean actual observed history tip and compiled registry tip. Unknown values remain unavailable; do not infer them from count. `0002_fixture` below is an isolated test migration, not a requested production migration.

Preserve protocol version 1, framing, operation names, request IDs, existing envelope fields and truthful `schema_migrations`. Do not change hello/health, shutdown, identity or admission behavior. Production retains its original migration set; no production version 2 is needed for this mission.

Recommended additive doctor result field:

```json
{
  "integrity_check": "ok",
  "schema_migrations": 1,
  "schema_compatibility": {
    "status": "compatible",
    "database_version": "0001_phase0",
    "supported_version": "0001_phase0",
    "history_verified": true
  }
}
```

Keep all existing capability fields alongside this excerpt. `history_verified: true` requires full stored ID/checksum-prefix validation against compiled inputs, not integrity success or count equality. An absent compatibility object decodes as absent, never as verified. An explicitly malformed object fails validation; it is not a legacy response.

The storage/backend owners own the shared Rust assessment type and final serialized field names. The recommended names above and error names below provide a concrete starting contract; if the integrated shared type uses other names, preserve every semantic distinction and map the transcript assertions to that actual type in backend evidence. Do not create a second independent compatibility algorithm in the CLI or IPC layer.

Machine expectations:

- Successful modern diagnostics use `status: "ok"`, unchanged request ID and protocol 1, integrity `ok`, actual count, actual applied/supported labels and explicit compatible/verified assessment.
- Rejected schema diagnostics use `status: "error"` and a stable typed reason. Proposed `error.code` values: `SCHEMA_CHECKSUM_MISMATCH`, `SCHEMA_HISTORY_MISSING`, `SCHEMA_NEWER_UNSUPPORTED`, `SCHEMA_HISTORY_INVALID`. They must not masquerade as `DB_BUSY`, protocol mismatch or SQLite corruption. An equivalent common schema error code requires a structured reason carrying the same distinction.
- Where available, rejected reports carry actual database/supported labels and `history_verified: false`; unknown or untrusted versions stay null/unavailable. Do not expose raw SQL, checksum dumps or application records in ordinary terminal messages.
- Preserve `DB_BUSY` for existing admission/deadline/contention/operational failures and `INTEGRITY_FAILED` for failed integrity or positive corruption evidence. Neither establishes a migration-history problem.
- `legacy_detail_unavailable` is a client-side interpretation of absent new fields, not fabricated server verification. A new client can report legacy basic diagnostic success only when the existing legacy integrity/count-1 checks pass.
- No new `doctor --json` option is required. Machine consumers use existing protocol-1 diagnostics; classify's existing JSON interface remains unaffected.

Validate the protocol/envelope and any explicit error before consuming success fields. For a successful doctor envelope, check integrity before compatibility. A malformed assessment or unsupported status must fail closed as unavailable detail; do not print a success disclosure first and then discover an error.

## Transcript assertion table

`\n` denotes the terminating newline. CLI failure means exit 1, empty stdout and the stated stderr. CLI success means exit 0 and empty stderr. A running daemon has no successful process exit at readiness; its later shutdown semantics remain unchanged. New schema-code spelling may follow the backend-owned type as described above, but the reason and success/failure classification must remain testable.

Define `D` as the existing exact success disclosure:

```text
ok. IPC owner-only. Tray not connected. Vendors not detected. Command classification not checked. Sweeper off.
```

The appended schema text below follows `D` on the same stdout line, separated by one space. Examples are plain text, without ANSI or color-dependent state.

| Case / controlled input | Expected observable output | Machine and exit assertions |
| --- | --- | --- |
| Fresh or reopened compatible database; startup | `arc-daemon ready. Socket: <socket>. Protocol: 1. Sweeper off.\n` | Ready only after migration commits and normal service startup completes. No mandatory doctor, migration prompt or fabricated progress. |
| Modern compatible doctor | `D Schema compatible. Database: <database>. Supported: <supported>. History verified.\n` | Exit 0; integrity `ok`, explicit `compatible`, verified complete history, actual count. Production example: database/supported `0001_phase0`, count 1. |
| Isolated compatible additive fixture | Same success template with database/supported `0002_fixture` | Exit 0 with actual count 2. No production migration is added. Count 2 must not itself cause failure in the new client. |
| New client, legacy daemon; integrity `ok`, count 1, no compatibility field | `D Schema compatibility details unavailable (legacy daemon).\n` | Exit 0 for legacy basic diagnostics; do not print `Schema compatible` or `History verified`. Absence is explicit, never silently upgraded into verification. |
| Legacy response with count other than 1, or malformed modern assessment | `SCHEMA_COMPATIBILITY_UNAVAILABLE\n` | Exit 1; missing trustworthy detail cannot establish compatibility. Keep integrity failure separate if integrity itself is bad. No fallback from malformed explicit detail. |
| Known checksum mismatch; live diagnostic fixture | `SCHEMA_CHECKSUM_MISMATCH\n` | Exit 1, matching typed wire reason; history not verified. The ordinary startup path rejects before this endpoint exists. |
| Required history entry missing; live diagnostic fixture | `SCHEMA_HISTORY_MISSING\n` | Exit 1, matching typed reason. Distinguish an empty new database eligible for initialization from history loss in an existing schema. |
| Database newer than this binary; live diagnostic fixture | `SCHEMA_NEWER_UNSUPPORTED\n` | Exit 1; report actual supported tip and observed newer entry if available. This is explicit downgrade rejection, not downgrade execution. |
| Gap, unknown interior entry, or otherwise invalid ordered history | `SCHEMA_HISTORY_INVALID\n` | Exit 1; no automatic repair or success based on total count. Storage owner defines precedence for combinations of invalid history. |
| Migration SQL fails or execution is interrupted during startup | Startup failure template below, using `MIGRATION_FAILED` or `MIGRATION_INTERRUPTED` | Daemon exit 1; no readiness line and no diagnostic endpoint. No claim of successful rollback in copy without confirmed rollback. |
| Live decoded doctor cannot enter/complete DB work | `IPC_ERROR\n` | Exit 1; wire `DB_BUSY`. Preserve the existing CLI category instead of silently changing established busy stderr. Do not label it corruption or an unsupported schema. |
| Integrity result fails or typed CORRUPT/NOTADB | `INTEGRITY_FAILED\n` | Exit 1; wire `INTEGRITY_FAILED`. Integrity `ok` alone never proves compatible history. |
| Endpoint absent after rejected startup, or transport exchange fails | `DAEMON_UNAVAILABLE\n` | Exit 1; no fabricated underlying schema reason. Consult the daemon's startup stderr. |
| Ordinary status while DB work is slow, with transport slots available | `running. Socket: <socket>. Protocol: 1. Sweeper off.\n` | Exit 0; hello/health remains DB-independent. Do not add schema validation to status. |
| Protocol mismatch | `PROTOCOL_INCOMPATIBLE\n` | Exit 1; retain protocol error semantics, distinct from schema version incompatibility. |

The code-only diagnostic error lines preserve the existing terminal error style. Human recovery guidance belongs in startup stderr and doctor help; do not make a new prompt or command a prerequisite for ordinary use.

Recommended doctor help: `Check database integrity and schema compatibility through the daemon.` Add: `If the daemon failed before startup, inspect its stderr for the cause.` Preserve existing capability disclosures; successful schema checks do not establish tray connection, vendor detection, classification verification or an enabled sweeper.

## Startup rejection and recovery copy

Use one sanitized, newline-terminated stderr line per failed startup, with empty stdout, exit 1, no ready line and no claim that doctor is reachable. Proposed format: `Database startup failed: <CODE>. <reason and next action>\n`. Existing exact-startup assertions must be updated intentionally by their owner. Do not introduce raw SQLite statement text or line breaks from external error strings.

| Startup cause | Proposed code, reason and next action after the common prefix |
| --- | --- |
| Altered checksum | `SCHEMA_CHECKSUM_MISMATCH. Migration checksum mismatch. Preserve the database and investigate against a trusted copy.` |
| Missing required history | `SCHEMA_HISTORY_MISSING. Required migration history is missing. Preserve the database and investigate against a trusted copy.` |
| Newer unsupported schema | `SCHEMA_NEWER_UNSUPPORTED. Database schema is newer than this binary supports. Use a binary supporting this database version; downgrade execution is not supported.` Include actual version labels when trustworthy. |
| Other invalid history | `SCHEMA_HISTORY_INVALID. Migration history is invalid. Preserve the database and investigate against a trusted copy.` |
| Migration SQL failure | `MIGRATION_FAILED. Database upgrade failed. Preserve the database and inspect the failure before retrying with a corrected binary.` |
| Observed interruption handled by the process | `MIGRATION_INTERRUPTED. Database upgrade was interrupted. Preserve the database and restart normally to revalidate it.` Do not promise this line after an uncatchable process kill. |
| Busy/locked database at startup | `DB_BUSY. Database is unavailable or busy. Wait for the current operation to finish, then retry startup once; inspect daemon stderr if it persists.` |
| Positive corruption or failed integrity, if startup detects it | `INTEGRITY_FAILED. Database integrity check failed. Preserve the database and investigate against a trusted copy.` This does not add an otherwise absent startup integrity scan. |

Missing or inconsistent history must not trigger initialization over existing application tables. Rejection copy must not suggest editing checksums/history, deleting records, removing locks/sockets or executing a downgrade. Preserve the database for investigation. Only say `Upgrade rolled back` when the engine positively confirms that attempted SQL and history writes rolled back; this contract's default wording makes no such unobserved claim.

An uncatchable termination can leave no stderr explanation. The next normal startup must revalidate history and either recover atomically or reject. Deterministic interruption acceptance belongs to the storage owner; a simulated transcript is not interruption proof.

For a live `IPC_ERROR` / transient unavailable result, retain existing bounded recovery guidance: pause four seconds, try `arcctl status` once, and consult daemon stderr/configured socket if it persists. This interval is existing budget-derived guidance, not a measured optimal delay or guarantee. `IPC_ERROR` is broader than contention; persistent errors require investigation. Doctor is optional after readiness, not a mandatory second liveness command.

## Mixed-version acceptance boundary

| Client / server combination | Required interpretation |
| --- | --- |
| Baseline client / new daemon, original production schema | Old client reads truthful count 1, ignores additive JSON fields and retains its existing successful output. Prove this with an old-decoder/actual-client fixture, not a protocol-version assertion alone. |
| New client / baseline daemon, original production schema | Legacy basic success with explicit detail-unavailable suffix. No fabricated history verification. |
| New client / new daemon, two-version fixture | Explicit compatible verified assessment and truthful count 2 succeed. |
| Baseline client / two-version fixture daemon | Baseline count-1 logic rejects count 2 as `INTEGRITY_FAILED`; record this legacy limitation honestly. Never falsify count to make it succeed. This is not a required production schema expansion. |
| Lower supported registry opening an upgraded fixture | Reject newer schema without changing application data or migration history. Protocol 1 does not authorize schema downgrade. |
| Old client / new schema error code | Unknown typed code maps to existing `IPC_ERROR`, exit 1. Do not imply an old binary can display new reason detail. |

Backend tests should cover deterministic response decoding and exact CLI stdout/stderr/exit behavior for every table row, including absent vs malformed new fields. Startup subprocess tests should prove rejection occurs before readiness/IPC and preserve application records plus complete history. Storage tests must prove the mutation guarantees; wording alone cannot. Retain worker, health, identity and shutdown regressions in the existing deterministic suites. No browser or participant test is part of this task.

## Checks, evidence and handoff

Skills used: `oma-design`, local documentation/interaction-contract route; `oma-docs`, verify mode guidance and manual source alignment. No external supplier is needed for an existing semantic contract without new visual judgment. No design-system file was changed.

| Command or check | Actual outcome |
| --- | --- |
| `git status --short --untracked-files=normal -- .arc-verify/schema-compatibility crates docs scripts` | Exit 0, empty before writing. Scoped observation only. |
| `git diff --stat -- .arc-verify/schema-compatibility crates docs scripts` and corresponding cached diff | Exit 0, empty before writing. |
| `git rev-parse HEAD 'HEAD^{tree}' bf28a35b594f5ae83ac97c216bd3b09e60a2dfa1^{tree}` | Exit 0; revisions recorded above. |
| Snapshot-ref resolution / separate `git rev-parse --verify <worker-ref>` | Each exit 128; refs unresolved locally. No source revision invented. |
| Known-path `cat` / `nl -ba` source and policy reads | Exit 0; source observations above. No code search/discovery fallback. |
| Python SHA-256 and `git show <baseline>:<path>` byte comparisons | Exit 0; all 11 inspected inputs match the accepted baseline. |
| `oma docs verify --help` | Exit 0; confirms verify regenerates shared `docs/generated/doc-refs.json`. Verify/sync not run because this task has no registry ownership or handoff. |
| Final scoped `git status --short --untracked-files=all -- .arc-verify/schema-compatibility crates docs scripts Cargo.toml Cargo.lock` | Exit 0; only the new assigned report is listed. No inspected product/shared-document change. |
| Python path/hash/baseline comparisons and required-state inspection | Exit 0; all 11 source paths resolve and match their recorded hashes/baseline bytes; all requested states are present. No Markdown links or external URLs occur. |
| `git diff --no-index --check /dev/null .arc-verify/schema-compatibility/design/diagnostic-contract.md` | Exit 1, empty output: a no-index comparison of the new file differs from `/dev/null`; no whitespace diagnostic was emitted. This is not recorded as an exit-0 gate. |
| Python final whitespace and code-fence inspection | Exit 0; no trailing whitespace, final newline present, balanced code fences. |

Documentation verification is **manual inspection — `oma docs verify` did not run**. The file uses code-form source paths, not external URLs or Markdown links. Final path resolution and whitespace checks cover this artifact only. Mise emitted tracking-symlink permission warnings during tool startup; reads and help still completed. No denied operation was retried through another surface.

| Inspected input | SHA-256 at Actor-local snapshot and matching baseline |
| --- | --- |
| `crates/arc-cli/src/main.rs` | `4e05860a6f57116fa02469138dd196fe9d5d9cea52f78a3af57ed2e9a14d6d90` |
| `crates/arc-ipc/src/lib.rs` | `4fe35c6cf064cf5c06fc79aecd0e2af0a06c8525997be2125d8870297ad51a28` |
| `crates/arc-daemon/src/main.rs` | `5d709825a6d5ca2cd003864460e1cc1f5fc185e83e9211ccdb2a35f0d5a7cd4b` |
| `crates/arc-daemon/src/db_worker/execution.rs` | `eeb3830c26dcc17219734caa9eea082569293793936dde950bb0b95002ecde32` |
| `crates/arc-daemon/src/connection.rs` | `8dc08ced1aaebfcb3d79053b774e293a9590ee59c61e3a6a5fef6355f57e4539` |
| `crates/arc-daemon/src/lifecycle.rs` | `2e025c3713b3a7432e8a75ba34c5ecbbabcaa3fe04f5c09d86afbb60fe7a46d3` |
| `crates/arc-daemon/tests/startup.rs` | `145d25de967338b71e554e609901052f02b21e64642306ae238c9ccad3af2724` |
| `crates/arc-cli/tests/binary.rs` | `b0c93d83c0f61d43f16dca3addb6bcc598a766cea66a2180e73adaf491eb7e2a` |
| `crates/arc-storage/src/lib.rs` | `fbbbbf0f9ae4f6628475d79e7f6dbd348691dfdcaf99567ef2f275e6eff2dcab` |
| `docs/bounded-ipc-recovery.md` | `d690a0432c86a418557a0dac6bc772e6cc68cded79245678f5ed167f616317b5` |
| `crates/arc-storage/migrations/0001_phase0.sql` | `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384` |

Acceptance checklist:

- Successful startup/doctor, legacy fallback and every requested rejection reason have observable text, stream and exit expectations.
- Startup recovery respects pre-IPC rejection and does not recommend history edits, record deletion or downgrade execution.
- Inspected facts, proposed interface, fixture expectations and unmeasured user outcomes are explicitly separated.

This task ran no Cargo tests, Clippy, formatter, harness, build, participant research, reminders, analytics, browser or screen checks. No runtime, rollback, field usability or business result is claimed. The Chief retains the required final-source Cargo gates, unchanged harness run including actual coverage failure if present, and independent QA. This report does not replace them.

Remaining handoff: backend/storage owners implement and bind concrete shared field/error names to the assertions; independent QA verifies actual transcripts and storage preservation. No ownership conflict was found. Only the assigned report is delivered; no shared manifest, lockfile, registry, mission state or product file is changed.
