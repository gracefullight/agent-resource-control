## Schema compatibility and older clients

Startup automatically applies pending compiled migrations before opening IPC. Doctor uses the same bounded SQLite worker to assess the complete migration history and version witness. It reports the actual migration count alongside `schema.status`, `schema.applied_version`, `schema.supported_version` and `schema.history_verified`. Integrity `ok` alone does not establish schema compatibility. Production remains version 1; additive version-2 migrations are isolated storage-test inputs.

Current successful doctor output appends `Schema compatible. Version 1. Migration history verified.` to the existing capability disclosures. A compatible two-version protocol fixture appends version 2 and succeeds with its actual count 2. Applied and supported versions must match, history must be verified and the count must agree with the contiguous registry. Unknown or inconsistent present details cannot fall back to legacy success.

Protocol remains version 1. Added `schema` details are optional when decoding an older daemon. A legacy result with integrity `ok` and count 1 succeeds with `Schema compatibility details unavailable (legacy daemon).` It does not claim verified compatibility. Any other legacy count fails with `SCHEMA_DETAILS_UNAVAILABLE`. A malformed present schema object also fails with that code; an unknown status produces `SCHEMA_INCOMPATIBLE`.

The original client ignores added fields and succeeds against the new production daemon's truthful count 1. Its count-based doctor rejects an isolated version-2 report as `INTEGRITY_FAILED`, even when the new client accepts that report. Additive fields and protocol 1 cannot remove that old-client limitation. Counts are never falsified. Schema error envelopes retain diagnostic results and use `SCHEMA_INCOMPATIBLE`, preventing legacy success even at count 1; an original client maps that unfamiliar code to `IPC_ERROR`.

| Schema status | Current doctor outcome and recovery |
| --- | --- |
| `compatible` | Exit 0 only with consistent verified details; print the applied version. |
| `upgrade_required` | Exit 1; restart with a binary supporting the database version. Doctor does not apply migrations. |
| `checksum_mismatch` | Exit 1; preserve the database and investigate against a trusted copy. |
| `missing_history` | Exit 1; preserve the database and investigate against a trusted copy. |
| `invalid_history` | Exit 1; preserve the database and investigate against a trusted copy. |
| `unsupported_newer` | Exit 1; use a binary supporting the advertised version. Downgrade execution is not supported. |
| Unrecognized status or inconsistent details | Exit 1; compatibility is unavailable, with no success fallback. |

Schema failures print `SCHEMA_INCOMPATIBLE: <reason>. Applied version: <value or unavailable>. Supported version: <value>.` on stderr with empty stdout. A newer advertised version is unverified. Ordinary success does not dump checksums, SQL or application records.

An incompatible database is rejected before daemon readiness and IPC binding. Startup stderr reports the storage cause and recovery guidance, for example:

```text
Database startup failed: migration checksum mismatch; preserve the database and investigate against a trusted copy.
Database startup failed: unsupported newer schema version 2 (supported 1); use a binary supporting that version; downgrade execution is not supported.
```

There is no doctor endpoint after that startup rejection. A later CLI attempt reports `DAEMON_UNAVAILABLE`; inspect daemon stderr for the underlying cause. No history repair, checksum rewrite or downgrade command is provided. Rejection is not downgrade execution. SQL migration failures remain startup failures; no successful rollback is inferred from terminal wording.

Backend evidence is in [the task-5 report](../.arc-verify/schema-compatibility/backend/report.md). It maps protocol fixtures, actual CLI exchanges, startup preservation and live worker diagnostics to executable tests. The existing health, admission, identity and shutdown checks remain in place; schema-specific gates additionally exercise health during assessment, cancellation, deadline expiry and worker join.
