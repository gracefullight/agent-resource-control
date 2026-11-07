# Runtime identity technical review

Status: completed technical inspection. No required product correction found in the delivered patch. Required Cargo results below are the Runtime Engineer's hash-verified evidence, not a new execution by this reviewer. Integrated acceptance and the additional failing repository validator remain with the Chief.

## Scope and provenance

The architecture problem is identity consistency: launcher environment and socket/database configuration must not divide one macOS user's daemon into multiple instances. This review covers resolution, admission, filesystem preservation, recovery and fixture coordination. It changes only this report.

Accepted committed source baseline: `073802555b639aa2fec183d93f748d9437cbbcad`; independently resolved baseline tree: `ce009646036415990a815170955421e1ab8d5c2f`.

Inspected task-2 local HEAD and input snapshot: `2322aee06da9220435dbeb49a54082baf8feebb0`; independently resolved tree: `ba589d1a9ddfcf68dc1572317495601ebdd31ca0`. Initial scoped status was empty. The implementation is present in this isolated snapshot, whose tree differs from the baseline. This does not establish that it is committed in the accepted workspace. The supplied snapshot ref could not be resolved locally (`fatal: Needed a single revision`); the commit itself resolves. No substitute revision was used.

Task-1's [delivery receipt](../.arc-verify/runtime-identity/implementation/delivery.json) describes its original **uncommitted Actor-local delivery** at local HEAD `7e46a56f0df04a0c0c4716e1cbb494e56d4ef4c5`. Its baseline and snapshot input tree are `ce009646036415990a815170955421e1ab8d5c2f`. Its unresolved worker ref is retained as reported. That original scope differs from this review snapshot; neither is the accepted baseline's repaired source.

I independently recomputed all 23 entries in [source.json](../.arc-verify/runtime-identity/implementation/source.json), including modes, and every changed-file and artifact digest in the delivery receipt. There were zero mismatches. Recomputing SHA-256 over the sorted compact JSON source map yielded `84110f4f1a4da327a8fabec09bb0cf642b1e4ea4ececbf173d2d8c40f00088f9`, matching the final implementation checks. This is a scoped file-map fingerprint, not a Git tree hash. New test/helper files are tracked in this local snapshot; tracking in a later integration checkout must be checked there.

Method: focused reliability and preservation review using `oma-architecture`. The shared execution policy, code-intelligence contract, architecture protocol/checklist and existing architecture decision index were read. The existing AV execution/state ADR concerns the harness, not ARC daemon identity; this report does not amend it. Serena tools were unavailable, so inspection used supplied paths and scoped reads, with Git comparisons for the known files. No agents, workflow, browser, installation or standalone build were started.

## Architecture and cost

| Approach | Implementation and maintenance cost | Operational behavior | Assessment |
| --- | --- | --- | --- |
| Native runtime root plus existing file-lock admission | Reuses arc-core, directory traversal, process identity and native locks; adds one edge to an existing libc dependency | Fixed per-user anchor; configured IPC endpoint remains explicit | Delivered approach; appropriate for this repair |
| Separate discovery service or registry with endpoint negotiation | Adds lifecycle, persistence, protocol and recovery work across daemon and CLI | Could discover an overridden endpoint, but introduces another source of truth | No requirement justifies this expansion |

Socket-specific admission cannot satisfy differing-override uniqueness. The delivered solution retains five crates, Tokio Unix IPC, bundled rusqlite, the existing lock implementation and Rust 1.95.0. `Cargo.lock` adds only `libc` to arc-core's dependency list. `crates/arc-core/Cargo.toml` uses the existing workspace dependency under `cfg(unix)`. There are no package upgrades, new crates, services, migrations or protocol changes. Implementation and maintenance work stay within the coupled resolver/startup modules and their fixtures; financial savings were not measured.

## Resolution and configuration

- `crates/arc-core/src/lib.rs:12` calls `confstr(_CS_DARWIN_USER_TEMP_DIR)` on macOS. The helper at line 28 performs a null sizing query, bounds allocation, retries a growing response, checks zero/error responses, final termination, embedded NULs, emptiness and absolute-path form. `OsString::from_vec` preserves native bytes. A failed query returns an error; there is no TMPDIR fallback.
- `lib.rs:79` retains nonempty `ARC_SOCKET_PATH` precedence; an empty override still uses `<runtime_root>/arc/arc.sock`. Both the daemon and unchanged CLI call this resolver. `server.rs:263` classifies the native root rather than the environment's temporary directory. Canonicalization here compares shared-root identities only; actual traversal still rejects arbitrary links.
- `lib.rs:68` derives `<native-root>/arc-instance-<effective-uid>/daemon.lock`. Neither socket nor database override participates. On other Unix platforms the anchor is fixed under `/tmp`, while default socket resolution retains the existing `env::temp_dir()` branch. Non-macOS execution was not performed.
- `lib.rs:88` preserves database resolution. A nonempty `ARC_DB_PATH` overrides; macOS defaults to `HOME/Library/Application Support/arc/arc.sqlite3`. Storage implementation bytes match the baseline. This repair does not move production SQLite into temporary storage.
- `crates/arc-cli/src/main.rs:58` keeps `status` and `doctor` on the configured socket. Per-user admission does not implement discovery of a daemon's overridden endpoint. The duplicate diagnostic at `instance.rs:24` retains the compatibility phrase and explains the one-daemon-per-user restriction with the anchor, without naming the losing socket as a served endpoint.

## Admission, effects and lifetime

`crates/arc-daemon/src/main.rs:15` resolves the database path without opening it. `prepare_instance` at line 35 performs socket preflight, resolves and validates the anchor entry, prepares its private directory, acquires admission, and only then prepares the requested socket directory. `open_and_migrate` is called after this returns. Thus a valid competing startup can inspect its override paths but cannot create their directories, database/WAL/SHM or socket after losing admission. Read-only rejection of an invalid requested endpoint may precede the duplicate diagnostic, which is consistent with preservation.

`server.rs:48` measures the full native socket pathname against `sockaddr_un.sun_path`, reserving the final NUL byte, before directory creation, anchor contents, storage or socket removal. The macOS maximum is 103 pathname bytes. Embedded NULs reject at the same preflight. Existing entries must be owned 0600 Unix sockets. Files, links and unsafe sockets reject before SQLite initialization. Binding repeats validation before removing a stale socket; ordinary bind/I/O failures after admission are not claimed to roll back an already opened database.

The `_instance` binding at `main.rs:20` is a named lifetime guard, not a discarded `_` pattern. It survives `server::serve` and its shutdown cleanup. `server.rs:293` installs signal handlers before readiness; shutdown drops the listener, closes SQLite and removes the socket while admission remains held. Errors unwind the same guard. Forced process death releases the OS file lock; a subsequent admitted daemon can replace its stale socket. The anchor pathname stays across shutdown.

## Existing entries, permissions and races

`server.rs:30` and `walk_directory` at line 155 retain descriptor-relative `openat`/`mkdirat`, `O_DIRECTORY`, `O_NOFOLLOW`, effective-UID checking and exact 0700 final-directory policy. Ordinary existing ancestors remain supported. Newly created components request 0700; existing entries are validated rather than chmodded or removed. A component that appears between inspection and creation is checked before nested creation. The final private directory is reopened and compared by device/inode before preparation returns.

Only the exact macOS `/var` → `/private/var` and `/tmp` → `/private/tmp` aliases receive an exception (`server.rs:249`). Shared temporary roots themselves reject as socket parents. Arbitrary intermediate, final and dangling links still reject; no whole-path canonicalization is substituted for the no-follow walk. Production has no recursive cleanup. Socket mode becomes 0600 before readiness; its enclosing private directory prevents other users reaching the bind-to-chmod interval.

`instance.rs:170` validates pre-existing anchor type, owner and exact 0600 mode without truncation. Acquisition at line 61 repeats validation, takes the private directory's acquisition lock, opens the file with `O_NOFOLLOW | O_NONBLOCK`, checks the opened descriptor's metadata, and acquires its exclusive OS lock before `write_owner` truncates the owner record. FIFOs cannot stall the opener. Bad modes and special entries are rejected without permission repair.

The acquisition-directory lock serializes participating daemon recoverers. A losing process sees either the live file lock or its completed owner record, rather than a partially written record from another acquirer. Recovery judges the opened inode using PID plus process-start token; inability to obtain a live process's token conservatively preserves its ownership. `remove_if_same_inode` at line 143 compares device/inode before unlinking, and the lock object's destructor never unlinks its successor. Unit cases explicitly exercise a replaced pathname and stale-holder drop.

These protections do not make pathname operations atomic against arbitrary external namespace mutation. Lock open, stale unlink, socket bind/chmod and shutdown unlink still use pathnames after validation; the inode comparison is a check followed by an unlink, not a kernel conditional unlink. The native per-user root and 0700 anchor protect admission from other users, and cooperating ARC daemons share the acquisition guard. An owner or administrator can still replace its own namespace; a configurable endpoint beneath a separately mutable ancestor has that inherited limitation. No adversarial namespace-swap experiment was run, and this review does not claim protection against it. The delivered code preserves and strengthens the existing directory protections without adding an unrequested restriction on ordinary ancestors.

## Fixtures and regression coverage

`crates/arc-daemon/tests/support/mod.rs:16` combines a thread mutex with a separate 0600, owned, no-follow file lock at `<native-root>/arc-fixtures-<uid>.lock`. Its cross-process wait is bounded at 90 seconds. This lock coordinates tests, not production admission. Resident fixtures and the migration-failure integration test hold it across their entire owned daemon lifecycle. Unit tests use separate scratch anchors and do not take the production daemon anchor.

Resident fixture declaration/drop order releases children before the coordination guard; `Daemon::drop` kills only its owned child when still running and always waits. The startup integration helper has a bounded wait and reaps on timeout. Short 0700 scratch directories now use the native root in the instance/server unit tests, both daemon integration binaries and storage tests. Environment and umask changes occur in children. No production bypass or global test-thread override was added.

| Requirement | Inspected checks and source |
| --- | --- |
| Native resolver edge cases and byte preservation | arc-core unit tests at `src/lib.rs:134` and `:151`; injected confstr responses include growth, zero/error, malformed and unstable sizes |
| Actual default under unset, changed and unwritable TMPDIR | `resident.rs:687`: actual shared default; failed write proves the 0500 TMPDIR fixture is unwritable; three daemon environments × three client environments × status/doctor; 0700/0600 ownership/mode checks |
| Exactly one differing-override daemon, no losing effects | `resident.rs:742`: both gated children exist before release; different socket directories, database paths and TMPDIR; exactly one nonzero loser, empty loser stdout, duplicate reason, complete no-follow snapshot equality, absent database parent/WAL/SHM/socket directory |
| Winner health and shutdown/death recovery | Same test verifies both CLI operations, graceful restart, explicit forced kill and reaping, retained stale socket, then recovered service and graceful cleanup |
| Unsafe paths preserve contents | `resident.rs:472` snapshots entries/inodes/owners/modes/bytes/link targets before daemon and CLI rejection; matrices retain unsafe directory, intermediate/final/dangling link, nondirectory and `..` coverage |
| Length rejection before admission writes | `resident.rs:834` compares anchor contents as well as rejected fixture snapshots; `server.rs:704` tests native-byte boundary and existing-entry preservation |
| Unsafe lock entries and stale recovery | `instance.rs:270` onward covers live duplicates, PID/start mismatch, dead PID, successor inode preservation, modes, links, directories and FIFO |
| Serialization across processes | `resident.rs:873` spawns a separate probe, observes no acquired marker while the parent holds coordination, releases it and requires successful progress. The source file-lock protocol supplies exclusion; the marker observation alone is not an exhaustive scheduling proof |
| Retained storage failure and persistent policy | `tests/startup.rs:64` retains the exact checksum-mismatch sentence and database assertions; storage test uses a child for environment configuration and asserts the frozen digest |

The gated contender launch does not control kernel scheduling or mandate which child wins; it checks either outcome symmetrically. Native-default tests keep the real persistent anchor and default directory, remove only their owned socket through shutdown, and do not stop an unrelated daemon to obtain admission. An unrelated running daemon can therefore cause fixture failure rather than being silently displaced.

## Executed evidence and limits

Commands executed by this reviewer:

- `git status --short --untracked-files=normal -- . ':!av.yaml'`: exit 0, initially empty.
- `git rev-parse HEAD '073802555b639aa2fec183d93f748d9437cbbcad^{tree}' 'HEAD^{tree}'`: exit 0, revisions above.
- Scoped baseline-to-snapshot Git diffs and `git diff --check ... -- crates Cargo.toml Cargo.lock rust-toolchain.toml`: exit 0; only the expected 11 product/test paths differ, with no whitespace error.
- Python SHA-256/mode verification of all 23 source entries and all delivery receipt entries: exit 0, no mismatch, fingerprint above.
- Baseline migration bytes from `git show` compared with the actual file: equal. `0001_phase0.sql` SHA-256 is `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`.
- `uname -srm`: exit 0, `Darwin 27.2.0 arm64`; `getconf DARWIN_USER_TEMP_DIR`: exit 0, `/var/folders/r9/0qvpcx794n385zwgzwfvt55h0000gn/T/`, matching the recorded fixture root.
- Snapshot-ref resolution: exit 128, ref unavailable locally; snapshot commit/tree independently resolve. Reads of `.agents/oma-config.yaml` and `.agents/results/architecture/` found them absent; known-path review proceeded without creating them.

No full suite was repeated on unchanged source. The receipt-bound [commands.json](../.arc-verify/runtime-identity/implementation/commands.json) and logs record:

| Engineer-executed command | Exit | Evidence |
| --- | --- | --- |
| `cargo fmt --all -- --check` | 0 | `implementation/fmt.txt` |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | `implementation/clippy.txt` |
| `cargo test --workspace` | 0 | `implementation/test.txt`: 53 passed; 0 failed/ignored/filtered |

The final command timestamps are 2026-11-07 09:47:31–09:47:38 UTC. The source manifest records unchanged bytes during checks and independently matches this snapshot. Test names and counts agree with the delivered tests: 2 CLI, 2 core unit, 3 core integration/probe, 17 daemon unit, 11 resident/probe, 1 startup, 13 IPC and 4 storage. Child-probe entrypoints are included in the 53 count; it is not a count of 53 independent feature scenarios.

Sandbox limits remain explicit: the engineer records setuid stripping and denied creation of non-UTF-8 filenames. Direct attribute-policy tests and native-byte resolver/path-length tests remain, with actual filesystem preservation checked on supported entries. Actual foreign-UID changes, non-UTF-8 file creation and non-macOS execution were not demonstrated. This review's shell commands worked; the earlier independent review's `grantpt` failure is not attributed to this run. Local Python startup emitted denied mise tracking-symlink warnings; the hash verification itself completed successfully. No GUI fallback or sandbox bypass was used.

## Findings and Chief handoff

There are no unresolved required technical findings in resolver identity, per-user admission, losing-start ordering, the retained configuration interfaces or the delivered preservation cases. Recommend retaining this focused implementation and proceeding with independent integrated-worktree acceptance. This report does not mark that separate acceptance complete.

One existing repository-check failure must remain visible at mission level: the hash-verified `implementation/harness.txt` records `./scripts/harness/validate.sh` exit 1. It flags `crates/arc-daemon/src/server.rs` (762 lines), `crates/arc-daemon/tests/resident.rs` (901) and unchanged `crates/arc-ipc/src/lib.rs` (549), plus missing `test:coverage`. All three files already exceeded 500 lines at the accepted baseline (690/655/549); the two owned modules grew. Consequence: architecture-validator conformance has not passed. Required disposition: the Chief must coordinate ownership and a separate module/validator follow-up or record this failure in the final decision; no waiver or repair is claimed here. This reviewer did not change the validator, acceptance commands, source ownership or assertions.

The Chief must verify the actual integrated bytes and new-file tracking, run the fixed final acceptance contract, and retain the platform/race/evidence limits above. Produced artifact: `docs/runtime-identity-technical-review.md`. Product and dependency interfaces are unchanged except the intended shared native-root/anchor helpers and per-user admission semantics.
