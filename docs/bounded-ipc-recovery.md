# Bounded IPC: CLI behavior and recovery

## Start, inspect and stop

Start `arc-daemon` in the foreground. After readiness, run `arcctl status` in another terminal with the same socket configuration. Run `arcctl doctor` only when database diagnostics are needed. [Runtime identity usage](runtime-identity-usage.md) covers paths, overrides and filesystem-error recovery.

Successful output remains exactly the following, with `<socket>` replaced by the configured socket path. Each line ends with a newline and exits 0:

```text
running. Socket: <socket>. Protocol: 1. Sweeper off.
```

```text
ok. IPC owner-only. Tray not connected. Vendors not detected. Command classification not checked. Sweeper off. Schema compatible. Version 1. Migration history verified.
```

`status` sends protocol `hello`; it proves daemon liveness and protocol compatibility. It does not inspect SQLite. `doctor` checks SQLite integrity and validates explicit schema compatibility against the compiled migration registry. Its successful text does not claim tray connectivity, vendor detection, command classification or an active Sweeper.

Ctrl-C (SIGINT) and SIGTERM to the known daemon PID initiate shutdown directly. No confirmation, diagnostic command or retention step is required. Wait for process exit before starting a replacement. The instance lock remains held while handlers and the database worker stop. ARC removes its socket on graceful shutdown; an anchor pathname may remain. After forced death, restart normally so ARC can reacquire admission and handle its stale socket. Do not delete locks or sockets as a saturation remedy.

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

## Limits and waiting budgets

These values match the [frozen contract](bounded-ipc-contract.md) and the inspected [RuntimeLimits implementation](../crates/arc-daemon/src/limits.rs). Unset or empty `ARC_IPC_PROFILE` selects defaults; `ARC_IPC_PROFILE=test` selects the fixed smaller profile for fixtures. Other nonempty values fail startup. There are no arbitrary cap or timeout environment overrides.

| Resource or budget | Default | Fixed test profile |
| --- | ---: | ---: |
| Retained connection handlers, including completed entries awaiting reaping | 64 | 8 |
| Admitted requests, including error responses and DB waits | 16 | 4 |
| Queued or reserved doctor jobs | 8 | 2 |
| Additional executing database job / dedicated worker | 1 / 1 | 1 / 1 |
| Tokio runtime worker threads | 2 | 2 |
| Maximum frame body and serialized response body | 1,048,576 bytes | 1,048,576 bytes |
| Header read | 1,000 ms | 150 ms |
| Whole frame read, including header | 2,000 ms | 300 ms |
| Response serialization and write | 500 ms | 100 ms |
| Database response, including queue wait | 1,500 ms | 500 ms |
| Database execution, also limited by remaining response time | 1,000 ms | 300 ms |
| SQLite busy wait | 100 ms | 50 ms |
| Shared internal shutdown budget | 3,500 ms | 3,500 ms |

The four-byte big-endian length prefix and protocol-1 JSON are unchanged. Frame size is checked before body allocation. At frame start F, all four header bytes must arrive by F plus the header budget; the complete body must arrive by F plus the whole-frame budget. An idle persistent connection also uses the next header's budget. Sending occasional bytes does not renew either deadline.

The response deadline starts before serialization and covers prefix and body writes. An oversized serialized response gets at most one small `FRAME_TOO_LARGE` fallback under that same deadline, then closure. Read/write expiry closes the stream and releases its handler/request capacity; it does not start a fresh timeout for an error response. A peer that never reads encounters the write deadline when output is backpressured. If a small response fits the socket buffer, it may complete; the next idle header deadline then releases the connection.

The default frame, DB-response and write budgets sum to four seconds. This is a server processing envelope, not a measured end-to-end latency or an absolute client timeout. `arcctl` retains five-second socket read/write timeouts; those apply to socket operations and do not establish a five-second total exchange deadline. OS scheduling and native calls still require runtime evidence.

## Busy, unavailable and recovery

[Admission](../crates/arc-daemon/src/admission.rs) uses nonwaiting permits and a reaped task set. [Request handling](../crates/arc-daemon/src/connection.rs) admits a request after the header and before allocating/reading its body. No connection or request waits in an application admission queue, and no rejection task is spawned.

| Condition | Wire behavior | Existing CLI result |
| --- | --- | --- |
| Connection cap reached, or request permit unavailable before decoding | Close the accepted connection without a response | Failed exchange maps to nonzero exit, empty stdout and `DAEMON_UNAVAILABLE` on stderr |
| Authenticated, decoded protocol-1 doctor request; DB queue full, worker stopping/unavailable, contention or DB deadline | Error response with the same request ID and `DB_BUSY` | Nonzero exit, empty stdout and exactly `IPC_ERROR\n` on stderr |
| Decoded doctor request; operational, resource or unexpected database error, including SQLite I/O or allocation failure | `DB_BUSY` | Nonzero exit, empty stdout and exactly `IPC_ERROR\n` on stderr |
| Positive SQLite corruption evidence (`CORRUPT` or `NOTADB`), failed integrity result | `INTEGRITY_FAILED` | Nonzero exit, empty stdout and exactly `INTEGRITY_FAILED\n` on stderr |
| Readable incompatible schema history | Error response with `SCHEMA_INCOMPATIBLE` and diagnostic result | Nonzero exit, empty stdout, cause and applied/supported versions on stderr |
| Protocol version mismatch | `PROTOCOL_INCOMPATIBLE`; a later valid frame may continue on the connection | Nonzero exit with `PROTOCOL_INCOMPATIBLE` on stderr |

Busy responses use the same bounded write as other responses. If that write fails or expires, the stream closes; the failed CLI exchange then reports `DAEMON_UNAVAILABLE`. A decoded request therefore does not guarantee delivery of its busy response.

The [client](../crates/arc-cli/src/main.rs) checks protocol and request identity, then handles explicit errors before interpreting success fields. Doctor retains the diagnostic result on schema errors. It maps unknown error codes, including `DB_BUSY`, to `IPC_ERROR`. Ordinary saturation must therefore not be interpreted as corruption or a version mismatch. Conversely, `IPC_ERROR` is a general CLI category; it does not identify DB overload by itself or guarantee that retry will resolve the underlying failure. An unavailable endpoint, a lost exchange and predecode overload share `DAEMON_UNAVAILABLE`.

For a transient busy/unavailable result, pause four seconds after the failed attempt, then run `arcctl status` once. This is recovery guidance derived from the default server budgets, not a measured optimal retry interval or a retry guarantee. A persistent connection starts another idle-header budget after a response, so the four-second pause alone does not ensure every connection has expired. If status succeeds, continue your work; use doctor only if integrity diagnostics are needed. If it still fails, check the foreground daemon's stderr and the configured socket. Persistent traffic can keep exhausting capacity, so repeated retries do not establish a broken database or justify lock deletion. Keep the existing identity, duplicate-start and unsafe-path recovery guidance in the usage document.

Only doctor enters the bounded SQLite queue. Hello/health can complete during slow DB work when a connection and request slot remain. They have no reserved transport slots: all connections or requests being occupied can reject status/health too. Database independence is distinct from reachability during complete transport saturation. Capacity returns when stalled reads/writes expire or admitted work completes; ongoing arrivals can occupy it again.

Signals stop admission, abort/drain handlers, cancel queued jobs, interrupt active SQL, close/join the worker and remove the owned socket under one shared 3,500 ms deadline. The external requirement is observed signal-to-exit below five seconds. A deadline error is a failure, not evidence that detached work terminated. The implementation reports cleanup/accounting errors rather than treating a timed-out wait as a successful join. No claim is made that an uninterruptible kernel call can be forcibly bounded.

## Interaction decision and untested assumptions

Preserve the current two-command journey and fail fast at capacity. Automatic retry could reduce manual attempts, but could also extend waits and add load. Progress/queue controls would add commands and state to a short local diagnostic journey. Those alternatives remain hypotheses; this repair implements neither. No nags, obscured failures, exit barriers or extra mandatory steps were added.

Assumed users are a developer starting ARC, an operator investigating a stalled local tool and an integration developer handling protocol failures. These personas are inferred from the existing CLI, not recruited participants. Shorter recovery, clearer state and reduced abandonment are intended outcomes, not measured findings. There was no participant study, screen-reader test, screen check, conversion/churn/retention measurement or user-reaction collection. Paused-time tests simulate elapsed time; subprocess/CLI tests execute native software under controlled fixture conditions. Neither establishes human usability or production performance.

## Evidence from the earlier bounded-IPC mission

The evidence below predates schema compatibility integration. Current backend evidence is recorded in the task-5 report linked above; mission-wide final verification belongs to the Chief.

The accepted committed baseline is `42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0`, tree `c802f71ac4d91ed6b8a602d5c83bf9cf57116b76`. This attempt inspected local snapshot HEAD `eb45d23b3b3be650f4a5cb8e064ba6715ae78b86`, tree `4746b22753c020491e2570d684e5edd86c02d468`, matching the supplied snapshot commit/tree. The snapshot includes integrated repair and tests absent from the accepted baseline. The supplied snapshot and worker ref names each failed local resolution with exit 128; their values are not inferred from HEAD. See [preflight observations](../.arc-verify/bounded-ipc/task-6/attempt-4/preflight.json).

The two documentation edits are task-local uncommitted changes. Source/test observations below refer to the inspected snapshot and the hashed inputs in the fresh test receipt, not committed baseline behavior. The Chief must verify delivered bytes in the receiving checkout after collection. The [task-6 report](../.arc-verify/bounded-ipc/task-6/attempt-4/delivery.md) is a material, nonignored file; it replaces reliance on the missing earlier `.agents/results/` report without inventing that report's contents.

| Guidance | Source and deterministic executable support |
| --- | --- |
| Fixed caps, profiles and budgets | [Limits](../crates/arc-daemon/src/limits.rs), `RuntimeLimits::{default,test_profile,from_profile,validate}` and `limits_reject_invalid_config_and_profiles`; [startup](../crates/arc-daemon/src/main.rs) selects two Tokio runtime workers. |
| Absolute partial-read and non-reading-peer deadlines | [Transport tests](../crates/arc-daemon/src/transport_tests.rs): `headers_and_trickled_bodies_have_absolute_deadlines`, `stalled_header_and_partial_body_release_request_capacity`, `nonreading_peer_expires_under_write_backpressure` use paused time. [Native daemon tests](../crates/arc-daemon/tests/bounded.rs): `malformed_and_stalled_frames_recover_actual_cli` and `nonreading_peer_releases_real_daemon_capacity` observe closure and successful CLI recovery. |
| Closure before decode and recovery after full transport saturation | [Deadline recovery fixture](../crates/arc-daemon/tests/support/deadline_recovery.rs), registered by [worker runtime tests](../crates/arc-daemon/tests/worker_runtime.rs): `full_request_and_connection_saturation_recovers_without_restart` fills request/connection caps, checks exact `DAEMON_UNAVAILABLE\n` with empty stdout/nonzero exit, observes deadline release, then checks both successful CLI commands in the same process. |
| Decoded DB busy, slow-DB health and successful recovery | `controlled_slow_database_does_not_block_health_and_cli_recovers` and `health_completes_while_native_sql_preparation_is_deliberately_stalled` in worker runtime tests assert exact `IPC_ERROR\n`, liveness while the real worker is held, and status/doctor after release. These fixtures leave transport capacity available for health. |
| Direct signals and complete cleanup | Both [lifecycle signal fixtures](../crates/arc-daemon/src/lifecycle_tests.rs), plus `saturated_sigterm_finishes_within_five_seconds` and `saturated_sigint_finishes_within_five_seconds` in worker runtime tests, require below-five-second exit, handler reaping, worker join, zero gauges, socket removal and reopened integrity `ok`. The saturated fixtures also restart the real daemon and check both CLI commands. |
| Exact successful text and identity compatibility | [Resident tests](../crates/arc-daemon/tests/resident.rs): `resident_contract_holds_until_sigterm`, absent-endpoint, TMPDIR and duplicate-start cases. [CLI binary test](../crates/arc-cli/tests/binary.rs) names both commands and rejects ANSI in help output; the client disables color and emits plain error-code lines. |
| Busy versus integrity error mapping | [Lifecycle adapter](../crates/arc-daemon/src/lifecycle.rs), `database_contention_and_unavailability_are_not_integrity_failures`; [typed failure regressions](../crates/arc-daemon/src/database_failure_tests.rs) cover operational/resource/unexpected SQL failures, non-SQLiteFailure errors, positive corruption and failed integrity/schema reports. `typed_worker_errors_reach_protocol_and_actual_cli_then_recover` sends typed IOERR, IOERR_READ, NOMEM, CORRUPT and NOTADB errors through the production adapter/handler and real CLI, asserts exact wire/stderr mapping and successful status/doctor recovery, then reaps handlers, joins the worker and reopens with integrity `ok`. Faults are injected in test code; they are not observed production incidents. |

Fresh checks ran on Darwin arm64 on 2026-11-07 UTC:

| Check | Observed result |
| --- | --- |
| `cargo test --workspace` | Exit 0 in 50.12 seconds; 183 passing invocations across 17 suites, zero failed/ignored/measured/filtered. Some modules are registered in multiple targets, so this is not a distinct-test count. Ran offline with a worktree-local target directory. [Full log](../.arc-verify/bounded-ipc/task-6/attempt-4/workspace-test.txt) and [receipt](../.arc-verify/bounded-ipc/task-6/attempt-4/workspace-test.json) retain argv, cwd, PID, times, exit and output hash. |
| Product inputs before/after tests | All 51 files retain their hashes and modes; the receipt binds both source manifests. |
| Migration and harness preservation | Both match accepted baseline bytes; [hash comparison](../.arc-verify/bounded-ipc/task-6/attempt-4/preservation.json). |
| Assigned-document diff and references | `git diff --check -- docs/runtime-identity-usage.md docs/bounded-ipc-recovery.md` exits 0. [Reference results](../.arc-verify/bounded-ipc/task-6/attempt-4/references-after.json) and [ownership boundary](../.arc-verify/bounded-ipc/task-6/attempt-4/boundary.json) retain the final local path/fragment and change-scope checks. |

The earlier task-6 inspection at `eed6986a06f66dcfd09eb2d05f04a3b5ca9e5206` and its [173-invocation test receipt](../.arc-verify/bounded-ipc/task-6/attempt-3/workspace-test.json) remain historical. The attempt-4 receipt above validates the integrated classification repair; older evidence is not presented as a fresh run.

`oma docs verify --help` exits 0 and confirms that scans regenerate the shared `docs/generated/doc-refs.json` registry. No ownership handoff exists for that registry, so verify/sync were not run. The link check is **manual inspection — `oma docs verify` did not run**, supported by deterministic local path/heading resolution. No external URLs occur in these two documents. `av.yaml` access remains denied and excluded from the ownership manifest; configured-target applicability is unverified. Mise tracking-symlink warnings did not prevent the tests. The supplied snapshot/worker refs remain unresolved locally. No denied operation was bypassed.

Task-6 changes only the two assigned documents and its evidence/report outputs. CLI, protocol, configuration and report interfaces are unchanged. Workspace tests execute the 100,000-attempt cases and the native measurement fixture, but mission-wide accounting and native RSS/thread/task/P95/SRS acceptance remain with the test/review owners. Structural caps are not performance measurements. [Located targets](bounded-ipc-targets.md) identify the SRS source.

The integration owner's [delivery](../.arc-verify/bounded-ipc/integration/delivery.md) retains harness exit 1 for oversized `resident.rs`, oversized IPC library and missing `test:coverage`. This task did not rerun or clear those failures or replace final formatting, Clippy and independent QA gates. Remaining Phase 0 features, Linux qualification and Windows runtime, Named Pipe and tray obligations remain open.

Skills used: `oma-docs` for source/document alignment and `oma-design` through its local contract route for the existing interaction decision. No visual system changed. No participant research, screen checks, external delivery, delegation, standalone build command, installation or download occurred. Serena tools were unavailable; source inspection used supplied known paths.
