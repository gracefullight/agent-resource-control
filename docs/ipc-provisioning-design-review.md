# IPC provisioning: CLI rejection and recovery review

Status: completed for task-3's report; implementation acceptance remains unverified.
Review Result: FAIL for the inspected baseline's directory-preservation behavior.
Verification completeness: partial for runtime behavior; source assessment completed.

## Scope and evidence

This review uses oma-qa and the installed `.agents/agents/qa-reviewer.md` definition. It owns only this report. It assesses the accepted baseline, the immutable provisioning goal and the supplied design advice. No product code, fixtures, manifests, saved task instructions or mission state were changed.

Observed on 2026-11-07: worktree HEAD is `3fd269e311b4a6b8e701a6b23588f11127b821bc` (`AV task snapshot task-3`), immediately following accepted `f6971037ed1c3524f88e10732e8910909a4facd3`. The scoped comparison of daemon provisioning, daemon main, core resolution, CLI main, resident tests and startup tests against the accepted revision was empty. Their working-tree diff was also empty. This report did not exist before this task.

Evidence below is source inspection, not a daemon run, a simulation or a field test. No participants, analytics, conversion, churn, retention, revenue or recovery-time measurements were supplied or collected. No simulations were performed.

## Confirmed baseline findings

### HIGH

- `crates/arc-daemon/src/server.rs:48-49`: `create_dir_all(parent)` followed by unconditional `set_permissions(parent, ...0700)` changes existing directories and follows directory symlinks. The helper contains no ownership check. This directly conflicts with preservation and rejection requirements. Remediation for the engineer: replace those two operations with separate private creation and non-mutating validation; require an owned directory with mode 0700, reject relevant symlink redirection, and finish validation before instance-lock acquisition. Do not chmod, chown, remove or replace rejected entries. This is a source-confirmed defect; this Actor did not reproduce it by mutating a fixture.

### MEDIUM

- `crates/arc-daemon/tests/resident.rs:45` and `crates/arc-daemon/tests/startup.rs:25`: valid fixtures use ambient directory permissions. Resident assertions at `resident.rs:233-237` observe privacy only after daemon startup. Remediation: explicitly establish fixture mode 0700 before spawning; establish 0755/0770 explicitly in rejection cases. Keep `startup.rs:62-91` reaching its migration-checksum failure. These source assertions do not prove rejected-directory preservation; add the deterministic checks below.

No separate performance defect was established. Text-only, color-independent output is relevant to accessibility; browser, screen, layout and web-audit checks do not apply to this assignment.

## Options and operator journey

| Option | Journey and compatibility | Decision |
| --- | --- | --- |
| Concise reason only | Reports failure, but leaves the operator to infer a safe next step. | Usable minimum, weaker recovery hypothesis. |
| Rejected path, specific condition, one recovery instruction | Explains what prevented startup and points to a separate owned 0700 directory through the existing override. Adds no steps on successful startup. | Recommend. |
| Automatic permission repair | Changes an existing directory or target. | Exclude: violates preservation. |
| Automatic path fallback | Starts at a different location from the resolved path and can leave the CLI addressing the original location. | Exclude: violates resolution compatibility. |
| Pressure, urgency or repeated prompts | Adds friction without evidence of better successful recovery. | Exclude: no demonstrated value. |

Inferred personas, not observed participants: a Unix developer on first startup, a returning developer checking liveness, and an operator selecting a filesystem location with `ARC_SOCKET_PATH`. These follow from `crates/arc-core/src/lib.rs:8-17` and `crates/arc-cli/src/main.rs:35-40`.

Expected journeys:

1. First startup with missing ARC directories: start the daemon; ARC creates private directories and reports readiness. No setup prompt or confirmation.
2. Existing valid owned 0700 directory: start normally; use `arcctl status` or `arcctl doctor` without extra steps.
3. Rejected directory: read one daemon stderr line; select a separate directory owned by the effective user with mode 0700 using `ARC_SOCKET_PATH`; apply the same socket path to daemon and CLI; retry. The override names a socket file inside that directory, not the directory itself. A missing separate directory may be provisioned by the daemon.

Do not tell the operator to chmod, chown, replace, delete or otherwise modify the rejected directory or its symlink target. Recovery guidance belongs in daemon startup stderr: the disconnected CLI cannot establish the cause of startup failure.

Hypothesis: actionable copy reduces recovery attempts and time while preserving the original directory. This report supplies no measured benefit and adds no user-study or retention gate.

## Rejection copy contract

Retain `main.rs:43-58`'s `Daemon startup failed: ` envelope, `socket directory: ` stage, stderr destination, failure exit and trailing newline. Include the rejected path, the actual condition and this neutral recovery instruction: `select a separate directory owned by your user with mode 0700 using ARC_SOCKET_PATH`.

Suggested complete lines, with placeholders replaced by observed values:

```text
Daemon startup failed: socket directory: <path> has permissions 0755; ARC requires 0700 and left it unchanged; select a separate directory owned by your user with mode 0700 using ARC_SOCKET_PATH.
Daemon startup failed: socket directory: <path> is owned by UID <actual>, expected effective UID <expected>; ARC left it unchanged; select a separate directory owned by your user with mode 0700 using ARC_SOCKET_PATH.
Daemon startup failed: socket directory: <component> is a symlink in socket directory <path>; ARC left it unchanged; select a separate directory owned by your user with mode 0700 using ARC_SOCKET_PATH.
```

Use the same permission template with 0770 when that is the actual mode. Report a non-directory as such. For an intermediate symlink, identify the offending component; do not report a target's permissions as the cause when redirection caused rejection. Do not claim that an entry was left unchanged unless the implementation and preservation checks support that claim. General creation or I/O failures must retain their actual reason rather than being relabeled as permission-policy failures.

Exact prose for new errors can vary while these semantic assertions remain testable. Existing successful output and CLI error tokens below remain unchanged. CR and LF from paths or errors must still become spaces through `sanitize`; output must be one logical line with one terminal newline. Do not introduce multiline instructions or embedded shell commands.

## Compatibility contracts for engineer and reviewer

| Contract | Source basis | Deterministic assertion |
| --- | --- | --- |
| Socket resolution | `crates/arc-core/src/lib.rs:11-17` | Nonempty `ARC_SOCKET_PATH` wins; unset or empty uses `env::temp_dir()/arc/arc.sock`. Exercise default resolution in an isolated child environment. Preserve legitimate platform temporary-root aliases while rejecting ARC-directory redirection. |
| Initialization order | `crates/arc-daemon/src/main.rs:15-39` | Socket-directory validation precedes `InstanceLock::acquire` and `open_and_migrate`. Database-path resolution currently occurs earlier; it is not database initialization. Rejection creates no lock, database, WAL, SHM or socket artifacts. |
| Daemon readiness | `crates/arc-daemon/src/server.rs:80-89` | Successful stdout line remains `arc-daemon ready. Socket: <socket>. Protocol: 1. Sweeper off.` followed by newline, after binding. Rejected startup has empty stdout, no readiness text and a nonzero exit. |
| Successful status | `crates/arc-cli/src/main.rs:62-75`; `resident.rs:282-291` | Exit success; stdout is `running. Socket: <socket>. Protocol: 1. Sweeper off.` followed by newline. |
| Successful doctor | `crates/arc-cli/src/main.rs:78-93`; `resident.rs:294-302` | Exit success; stdout is `ok. IPC owner-only. Tray not connected. Vendors not detected. Command classification not checked. Sweeper off.` followed by newline. Preserve stated limitations. |
| CLI unavailability | `crates/arc-cli/src/main.rs:43-59,96-106`; `resident.rs:352-363` | With no daemon after rejection, both existing commands exit nonzero, stdout is empty and stderr is exactly `DAEMON_UNAVAILABLE\n`. Do not invent a client diagnosis of permission or ownership failure. |
| Single-line startup failure | `crates/arc-daemon/src/main.rs:43-58` | Permission, ownership and symlink failures use stderr and failure exit. Inject CR/LF into a fixture path/error and assert the rendered line contains neither embedded CR nor embedded LF. |
| Duplicate instance | `crates/arc-daemon/src/instance.rs:24-32`; `resident.rs:304-318` | Second daemon exits nonzero, stderr retains `socket already served`, no readiness is announced by the second daemon, and the first still answers hello/status. Do not reinterpret a valid private directory as an error merely because it contains the live daemon's files. |
| Socket and shutdown | `crates/arc-daemon/src/server.rs:74,78`; `resident.rs:320-349` | Socket remains 0600. SIGTERM exits successfully, removes the socket and permits a clean database reopen. No new exit confirmation or retention barrier. |

`<socket>` and `<path>` above are fixture values, not literal output. Keep successful text color-independent (`crates/arc-cli/src/main.rs:26`). No new command, wizard, fallback location or product surface is required.

## Required preservation and recovery checks

These are test expectations for implementation and independent QA, not results from this Actor:

- Unit cases: missing and nested ARC directories become 0700; an existing owned 0700 directory validates without mutation; explicit 0755 and 0770 directories reject; unsafe ownership rejects deterministically through a policy seam without privileged chown; final and relevant intermediate symlinks, including dangling links, reject. An AlreadyExists creation race must validate the winner rather than grant permission to change it.
- Daemon/CLI cases: repeat rejection cases with a sentinel in each existing directory and in private/nonprivate symlink targets. Include both private and permissive targets so rejecting only target permissions cannot pass. Assert the path, actual reason and recovery instruction in daemon stderr, failure exit, empty stdout and no readiness. Then run both CLI commands against that socket and assert the unavailability contract.
- Before rejection and after process exit, compare device/inode identity, full mode bits, UID/GID, sorted entry names and sentinel bytes. For symlinks also compare lstat identity and link text, plus target snapshots. Check for no new lock, database, WAL, SHM, socket or nested directories under the rejected path/target. Keep any database path outside that subtree observable too, so an unchanged socket directory alone cannot hide early database initialization.
- Successful integration: explicit owned 0700 fixtures, newly created nested directories, existing override and isolated default resolution, mode 0600 socket, exact successful CLI output, duplicate-instance rejection and successful SIGTERM lifecycle. Test restrictive/permissive umasks in child processes rather than changing the parallel runner's global umask.
- Sanitization: use a rejection fixture with CR/LF in its path, assert sanitization and one stderr line, and verify preservation using the original filesystem path. The output must never suggest executing the path as a command.
- Keep the migration-checksum test's exact `Database startup failed: Phase 0 migration checksum is missing or mismatched.\n` assertion (`startup.rs:73-75`) and establish its directory as 0700 before spawning.

## Commands, limits and handoff

Commands executed: `git diff --stat`, `git status --short --untracked-files=no`, scoped `git diff`/`git status`, `git rev-parse HEAD`, `git log -3 --oneline`, `git diff f6971037ed1c3524f88e10732e8910909a4facd3 HEAD -- <reviewed paths>`, and `git diff --check`; scoped `cat` and `nl -ba`/`sed` reads of the cited files and QA instructions. All completed with exit 0. Broad Git inspection emitted `av.yaml: Operation not permitted`; that file was not read or needed for this report. Scoped source comparisons were empty. Final report checks are recorded below.

Tool discovery found no Serena search/navigation tools. Used known-path scoped reads, as allowed by the code-intelligence fallback; no substitute recursive source search. An external memory registry keyword check returned no relevant entry (exit 1); no historical memory claim informs this report. No credentials, external effects or delivery were attempted.

Final artifact checks: scoped Git status showed only this report as untracked and no product diff (exit 0). `git diff --no-index --check /dev/null docs/ipc-provisioning-design-review.md` emitted no whitespace diagnostics and exited 1 for the new-file difference; that is not a product-test pass. An initial Python heredoc could not create its shell temporary file (exit 1). An inline retry used a case-sensitive marker assertion and failed on the existing capitalized `Single-line` label (exit 1); the Python shim also emitted mise tracking-permission warnings. The corrected case-insensitive integrity check through existing `/usr/bin/python3 -c` passed (exit 0): UTF-8 report, 14,226 bytes before this evidence paragraph, ten required contract markers and no trailing whitespace. No permissions were changed and no tools were installed to resolve these check attempts.

No Cargo, daemon or CLI runtime checks were run by this report-only Actor. The engineer and independent reviewer retain the required pinned Rust 1.95.0 gates: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`. `rust-toolchain.toml` still specifies 1.95.0, rustfmt and clippy. This source observation is not a fresh toolchain-execution or gate pass. No browser, GUI, visual check, installation, user recruitment or product edit was performed.

Acceptance checklist:

- [x] Specifies testable rejection/recovery expectations and existing success/exit contracts.
- [x] Guidance selects a separate owned 0700 directory and never directs modification of a rejected directory.
- [x] Separates source observations from hypotheses; reports no invented field-test or retention results.
- [x] Only owned file produced: `docs/ipc-provisioning-design-review.md`.

Remaining risk: source inspection cannot establish non-mutation under live execution, race handling or final error wording. Independent runtime tests and the three pinned-toolchain gates must establish those results on the repaired revision. No ownership or interface conflict was found in the reviewed files.
