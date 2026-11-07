# ARC runtime identity: usage and recovery

## Start and inspect

With the existing `arc-daemon` and `arcctl` binaries on your PATH, start the daemon:

```sh
arc-daemon
```

After it reports readiness, use another terminal:

```sh
arcctl status
```

Use `arcctl doctor` when you want local IPC and database diagnostics. It is optional; starting and checking status requires no extra setup or diagnostic step. The daemon stays in the foreground. Stop the instance you started with Ctrl-C or SIGTERM to its known PID.

Successful `status` proves daemon liveness and protocol compatibility. It does not check SQLite integrity. `doctor` checks SQLite integrity and the recorded migration count. Both commands still need a free connection and request slot; bypassing database work does not reserve transport capacity for status.

On macOS, the default socket is `<native-runtime-root>/arc/arc.sock`. ARC obtains that root with `confstr(_CS_DARWIN_USER_TEMP_DIR)`. Unset, changed and unwritable `TMPDIR` values select the same default endpoint. A failed native query is an error; ARC does not select a TMPDIR fallback.

`ARC_SOCKET_PATH` overrides the **socket file**, including its parent path. A nonempty value takes precedence; an empty value selects the default. For example, after choosing a separate, short, owned socket directory:

```sh
ARC_SOCKET_PATH="$socket_file" arc-daemon
```

In the client terminal, set `socket_file` to that same full socket-file path:

```sh
ARC_SOCKET_PATH="$socket_file" arcctl status
ARC_SOCKET_PATH="$socket_file" arcctl doctor
```

Clients must use the active daemon's configured override. Default `arcctl status` does not discover arbitrary overridden endpoints. Changing `ARC_DB_PATH` does not tell the client where the socket is.

Database policy is unchanged: macOS defaults to `$HOME/Library/Application Support/arc/arc.sqlite3`; a nonempty `ARC_DB_PATH` overrides it. SQLite remains persistent and belongs to the daemon; `arcctl` does not open it. Changing TMPDIR does not relocate the production database. The default-endpoint integration fixture deliberately uses an isolated database override, so its successful run is not evidence of opening a user's production database.

## Verified CLI behavior

The following output is grounded in delivered executable assertions and inspected source. `<socket>` and `<anchor>` are explanatory placeholders, not literal output or a captured terminal transcript.

| Situation | Output and exit behavior | Evidence |
| --- | --- | --- |
| Serving daemon, `arcctl status` | Exit 0; `running. Socket: <socket>. Protocol: 1. Sweeper off.` | Exact stdout assertion in `resident_contract_holds_until_sigterm` |
| Serving daemon, `arcctl doctor` | Exit 0; `ok. IPC owner-only. Tray not connected. Vendors not detected. Command classification not checked. Sweeper off.` | Exact stdout assertion in the same test |
| Absent endpoint, either client command | Nonzero exit; stderr contains `DAEMON_UNAVAILABLE` | `commands_report_daemon_unavailable_when_absent` |
| Concurrent starts with different socket and database overrides | Exactly one remains serving. The loser exits nonzero, emits no readiness and reports `only one daemon per user`; its database parent, database/WAL/SHM and socket directory remain absent | `concurrent_distinct_overrides_admit_one_and_recover_after_shutdown_and_death` |
| Rejected socket location | Nonzero exit, empty stdout, one stderr line beginning `Daemon startup failed: socket directory:`, naming the rejected path, reason and `ARC_SOCKET_PATH` guidance; fixture contents remain unchanged | `assert_preserved_rejection` and its unsafe-directory, symlink, non-directory, overlong-path and existing-entry cases |

Doctor's success covers the local owner-only IPC check, SQLite integrity result and one recorded migration. Its text explicitly leaves tray connectivity, vendor detection and command classification unavailable, and Sweeper off. It is not a claim that those capabilities work.

There is one daemon per user even with different `ARC_SOCKET_PATH`, `ARC_DB_PATH` or TMPDIR values. The admission anchor is `<native-runtime-root>/arc-instance-<effective-uid>/daemon.lock`. The inspected duplicate-error formatter produces:

```text
Daemon startup failed: instance lock: socket already served by another daemon for this user; only one daemon per user is allowed (instance anchor: <anchor>).
```

Tests assert the compatibility fragment `socket already served` and the per-user explanation separately; they do not compare this whole line byte for byte. A rejection does not mean the losing requested socket is active. Connect to the existing daemon using its configured socket.

The inspected server prints `arc-daemon ready. Socket: <socket>. Protocol: 1. Sweeper off.` only after database initialization, signal-handler setup, socket binding and socket permission setup. Resident tests establish readiness with a successful protocol hello, rather than socket-file existence alone. Rejection tests require empty stdout. No separate successful-start transcript was supplied.

`native_default_and_cli_ignore_unset_changed_and_unwritable_tmpdir` runs three daemon environments against three client environments for both commands. It asserts success throughout, checks the default path in status output, proves the unwritable TMPDIR rejects a write, and verifies 0700 directories plus 0600 socket and anchor. It also verifies that the changed/unwritable TMPDIR roots gain no `arc` directory.

## Recovery guidance

For `DAEMON_UNAVAILABLE`, check that the daemon started and that the client uses its configured socket. Read the daemon's startup stderr if startup failed. A disconnected client reports unavailability; it does not diagnose the daemon's path or database failure. Transport saturation can also produce this error. Changing TMPDIR is not a way to select another default endpoint on macOS.

For transient saturation, pause four seconds after the failed attempt, then run `arcctl status` once. This pause is based on the default server's combined frame, database-response and write budgets. It does not guarantee a free slot: persistent connections get a new idle-header budget after each response, and other clients can occupy capacity. Use `arcctl doctor` afterward only if you need database diagnostics. A decoded doctor request rejected by the database queue or its deadline gets protocol `DB_BUSY` if the bounded response write succeeds; the existing CLI then prints exactly `IPC_ERROR` on stderr, leaves stdout empty and exits nonzero. Operational and resource failures, such as SQLite I/O or allocation errors, also map to `DB_BUSY` → `IPC_ERROR`; this general result does not identify saturation or guarantee that retry will resolve it. Positive SQLite corruption codes, failed integrity results and invalid migration counts retain `INTEGRITY_FAILED`. If the response write fails or expires, the connection closes and the failed exchange maps to `DAEMON_UNAVAILABLE`. That result alone does not establish corruption or a protocol mismatch. If status still fails, inspect the foreground daemon's stderr and socket configuration. ARC adds no automatic retries, waiting prompt or extra mandatory command.

The default daemon retains at most 64 connection handlers, admits 16 requests and queues eight database jobs plus one executing job. Header reads expire after 1,000 ms; a whole header and body share 2,000 ms from frame start. A response has 500 ms for serialization and writing. Partial progress never restarts these deadlines. On connection or request admission failure, ARC closes before decoding instead of sending a busy response. For the fixed test profile, database/shutdown budgets, protocol details and current executable evidence, see [bounded IPC recovery](bounded-ipc-recovery.md).

For a duplicate start, use the already running instance. If you intend to change its configuration, gracefully stop your known instance, wait for it to exit, then start with the new configuration. Do not treat a different socket or database override as a separate instance slot.

For a rejected socket location, choose a **separate directory owned by you with exact 0700 permissions**, and set `ARC_SOCKET_PATH` to a short socket-file path inside it. ARC creates missing directory components privately. Leave the rejected existing directory and its contents intact; deletion or permission changes are not part of this recovery. Existing socket entries must be owned 0600 Unix sockets. Arbitrary symlinks, non-socket entries, shared temporary roots as the final parent and unsafe directory permissions are rejected. The native socket-path limit on the verified macOS host is 103 bytes, including the directory path but excluding the terminating NUL; use a shorter path for an overlong rejection.

An unsafe instance-anchor error concerns the fixed per-user anchor. Changing socket or database overrides cannot relocate that anchor. Preserve the rejected entry and report the exact daemon diagnostic for investigation; socket-directory guidance does not resolve an unsafe anchor.

Ctrl-C (SIGINT) and SIGTERM begin shutdown directly. The server stops admission, cancels and reaps handlers, interrupts and joins the SQLite worker, and removes its socket under one 3,500 ms internal budget. The required signal-to-exit check is below five seconds; the [bounded IPC recovery evidence](bounded-ipc-recovery.md#evidence-and-limits) identifies the executed fixtures and their scope. There is no exit confirmation or retention barrier.

Graceful shutdown closes SQLite and removes the socket. The anchor pathname may remain after shutdown; its presence alone does not prove a live daemon. The resident lifecycle test verifies graceful restart, then forced-death recovery with a stale socket left behind. After the process has exited, restart normally: ARC reacquires admission and handles the stale socket. Manual lock/socket deletion is not a required recovery step.

## Bounded IPC documentation scope

The current bounded-behavior review inspects task-6 attempt-4 local snapshot HEAD `eb45d23b3b3be650f4a5cb8e064ba6715ae78b86`, tree `4746b22753c020491e2570d684e5edd86c02d468`. It includes the integrated database-error classification repair and supersedes the earlier task-6 inspection at `eed6986a06f66dcfd09eb2d05f04a3b5ca9e5206`. The mission's accepted committed baseline is `42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0`, tree `c802f71ac4d91ed6b8a602d5c83bf9cf57116b76`; the snapshot contains the integrated repair beyond that baseline. These edits do not claim the repair is committed in the baseline. [Bounded IPC recovery](bounded-ipc-recovery.md) records the frozen contract, source/test support, fresh deterministic checks and remaining evidence limits. Only the two assigned documentation files are edited by task-6; product interfaces remain unchanged.

## Earlier runtime identity evidence (historical)

The remainder of this section records the earlier documentation task and its revisions, checks and handoff. It is historical evidence, not a fresh bounded-IPC verification claim.

Accepted committed baseline: `073802555b639aa2fec183d93f748d9437cbbcad`, tree `ce009646036415990a815170955421e1ab8d5c2f`. This documentation Actor inspected local snapshot HEAD `18af67c79013869ec2b624a3f0718ee87092731e`, tree `ba589d1a9ddfcf68dc1572317495601ebdd31ca0`. That snapshot includes the delivered repair; these observations do not claim it exists in the accepted baseline. The worker ref was not yet resolvable during inspection.

The [implementation report](runtime-identity-implementation.md), [delivery receipt](../.arc-verify/runtime-identity/implementation/delivery.json), [command records](../.arc-verify/runtime-identity/implementation/commands.json) and [test log](../.arc-verify/runtime-identity/implementation/test.txt) record the engineer's macOS run: all three required Cargo gates exit 0, 53 tests pass. The receipt's 12 changed-file hashes were independently recomputed here and all matched. The delivered source fingerprint is `84110f4f1a4da327a8fabec09bb0cf642b1e4ea4ececbf173d2d8c40f00088f9`; it is a source-manifest digest, not a Git revision.

Fresh checks in this documentation Actor's Darwin arm64 worktree on 2026-11-07:

| Check | Outcome |
| --- | --- |
| `cargo test --workspace` | Exit 0; 53 passed, zero failed or ignored, including all 11 resident cases |
| `cargo fmt --all -- --check` | Exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0 |
| Python SHA-256/mode comparison against the delivered source manifest | All 23 source entries match, including the unchanged migration |
| Manual local Markdown-reference check | All 11 linked paths resolve |

An earlier `cargo test -p arc-daemon --test resident` exited 101: 9 passed and 2 failed. The absent-endpoint test could not find the sibling `arcctl` binary within 30 seconds in the fresh target directory; the cross-process coordination probe also exceeded its 10-second exit wait. That run overlapped the workspace run, so the probe failure's cause was not isolated. The complete workspace command produced the client binary and subsequently passed every case. These failures are retained here for the Chief; the package-only attempt is not reported as a pass. No assertion or timeout was changed.

`oma docs verify --help` exited 0 and confirmed that verification always regenerates `docs/generated/doc-refs.json`. The scan was not run because this assignment owns only this usage file; reference verification above is manual inspection, not OMA CLI output. Initial unscoped Git status could not access ignored `av.yaml`; the scoped status excluding that path was clean before this file was added. Mise tracking-symlink warnings did not prevent the recorded commands from running. The worker-ref lookup exited 128 because the ref was absent. No denied operation was bypassed.

Source checks used [the client](../crates/arc-cli/src/main.rs), [configuration resolver](../crates/arc-core/src/lib.rs), [startup](../crates/arc-daemon/src/main.rs), [server](../crates/arc-daemon/src/server.rs), [instance errors](../crates/arc-daemon/src/instance.rs), [resident assertions](../crates/arc-daemon/tests/resident.rs) and [configuration subprocess tests](../crates/arc-core/tests/runtime.rs). Serena exposed no tools; supplied paths were read directly. No product code was changed. Skill used: `oma-docs`; this documentation-only task required no visual-design supplier or screen check.

No participant study, usability metric, conversion, churn or retention outcome was collected. The tests establish executable behavior under their fixture conditions, not measured human task success. No simulation or retention experiment was run. The implementation report separately records the failed additional harness validator and sandbox limits; this usage review does not clear those findings or replace the Chief's integrated acceptance decision.

Acceptance checklist: the required behavior and recovery sections are present; default and override journeys preserve the two-command client; duplicate and unavailable states are distinguished; diagnostic claims cite executable assertions or explicitly identified source inspection; recovery preserves existing contents. Only `docs/runtime-identity-usage.md` is changed by this task. CLI and protocol interfaces are unchanged.

Chief handoff: `server.rs::directory_error` is reused by `main.rs::prepare_instance` for the fixed anchor directory, so an anchor-directory rejection can suggest changing `ARC_SOCKET_PATH` even though that cannot relocate the anchor. The recovery section above distinguishes these cases. Any product-wording correction belongs to the implementation owner; no product file was edited here. Full integrated acceptance and the additional harness finding remain with the Chief.
