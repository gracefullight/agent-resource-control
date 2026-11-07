# IPC directory-provisioning repair

Status: evidence-report correction complete; independent integrated verification and Chief acceptance pending.
Recovery skill: oma-debug, following the supplied installed debug-investigator definition. This recovery changes only this report and does not replay implementation, regenerate tests or run Cargo checks. The original engineer report records oma-debug for implementation and oma-qa for its scoped checks; those historical checks do not substitute for task-5's independent oma-qa verification. No subagents or AV operations were launched.

## Baseline and ownership

The original engineer report records that the initial worktree HEAD was `578786122258b6a33cdc7cadf43f909cf2802a99` (AV task snapshot task-1), immediately following the accepted `f6971037ed1c3524f88e10732e8910909a4facd3` snapshot. The assigned daemon files had no preceding working-tree changes; their committed diff between those snapshots was empty.

Implementation/report paths:

- `crates/arc-daemon/src/server.rs`: provisioning implementation, policy and deterministic unit regressions.
- `crates/arc-daemon/tests/resident.rs`: explicit private fixtures, isolated child configuration and daemon/CLI preservation/startup regressions.
- `crates/arc-daemon/tests/startup.rs`: explicitly private migration-failure fixture.
- `docs/ipc-provisioning-implementation.md`: this handoff, the only file changed by this recovery.

Historical logs were recovered read-only from the retained engineer workspace:

```text
/Users/gracefullight/workspace/arc/.agent-valley/operations/0b22258d-d7c5-4fb0-bc7b-73945a671689/workspaces/fa572687-8b27-4ea3-8c17-867d5159fa1d/workspace/.agent-valley/parallel/workspaces/efcac0c89f4fc294-7afaa346b4bf92bf-1/.agents/results/bugs/
```

The three log files exist there and are ignored by Git through `.gitignore:5:.agents/results/`, freshly confirmed by `git check-ignore -v` (exit 0). Python `Path.exists()` freshly confirmed that the log directory is absent from both this recovery workspace (`efcac0c89f4fc294-7afaa346b4bf92bf-5`) and the integrated mission workspace above `.agent-valley/parallel/`. Their available output is embedded below so the evidence travels with this report. No ignored logs were force-added or copied into another workspace. The formatter log is empty; that alone does not establish success or exit zero.

This recovery attempt resumed at HEAD `d4ec3d2d93aee96d448ceeaa549abdf229197803` (`AV task snapshot task-1`), whose parent is the accepted `f6971037ed1c3524f88e10732e8910909a4facd3`, with scoped Git status empty at the start. It retains the implementation and embedded evidence already present at that revision. The incoming report described an earlier `-3` recovery workspace and HEAD `6fe80206ab84036e3167547a75ab7a768fc34b10`; those references describe prior report work, not this run. This attempt updates the workspace identity and rechecks the evidence without replaying product work.

No ownership expansion was needed. `main.rs`, `instance.rs`, manifests, Cargo.lock, migrations, the toolchain pin and core path resolution have no diff against f697103. Other Actors' reports and mission state were not edited. Broad Git status/diff commands reported `av.yaml: Operation not permitted`; scoped status and diff excluding that unrelated configuration succeeded. That file was not changed or used to infer authorization.

## Root cause and fix

The accepted implementation called `create_dir_all(parent)` and then unconditionally changed the parent to mode 0700. An existing 0755/0770 directory was silently modified; both operations followed directory symlinks. Ownership was not validated. Existing successful fixtures relied on this mutation to establish privacy.

Provisioning now separates inspection, creation and validation:

1. Read-only traversal opens each existing component with `openat(O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)` relative to a held directory descriptor. It rejects files and directory symlinks. On macOS, only the standard root aliases `/tmp -> private/tmp` and `/var -> private/var` (or their absolute target spellings) are allowed. Arbitrary TMPDIR or override symlinks are rejected.
2. A missing component is recorded before any creation. Inspection continues across `..` returning from a missing prefix, so a later existing symlink cannot cause a new prefix to be created before rejection. Existing final directories are validated before the creation pass.
3. Each missing component is created with `mkdirat(..., 0700)`. There is no directory chmod, chown, deletion or cleanup. `AlreadyExists` opens and validates the encountered entry. A directory that appears between inspection and creation is also validated before nested creation.
4. The socket parent must be owned by the effective UID and have exactly mode 0700, including rejection of special permission bits. Directory type is enforced by the descriptor open. Existing ordinary ancestor directories may be shared.
5. A second no-follow traversal checks device/inode identity and repeats final ownership/mode validation before returning.

All failures from directory validation identify the affected path/reason and direct the operator to choose a separate owned 0700 directory through ARC_SOCKET_PATH. Successful daemon/CLI output is unchanged. Canonical shared-root comparison also refuses alternate spellings of the same shared temporary root.

`prepare_socket_directory(&Path) -> io::Result<()>` remains compatible. `main.rs` retains provisioning, then instance-lock acquisition, then SQLite initialization. The socket bind still establishes 0600, and the existing locking, framing, peer checks and shutdown contracts remain intact.

## Regression matrix

| Case | Evidence |
| --- | --- |
| Existing 0755 and 0770 directories | Unit regression plus daemon/CLI cases for override and isolated default TMPDIR paths reject without mutation. |
| Final/intermediate links to 0700, 0755 and 0770 targets | Unit and daemon/CLI rejection; sentinel bytes, entries and metadata remain unchanged. |
| Dangling links | Unit and daemon/CLI rejection; link retained, missing target never created. |
| Missing prefix followed by `..` and an existing link | Unit and daemon/CLI rejection; even the missing prefix remains absent. |
| Existing non-directory parent | Daemon rejection before lock/database/socket initialization, preserved file bytes and identity. |
| Foreign UID | Deterministic numeric policy test exercises the same helper used with live metadata and geteuid, without privileged chown or invented filesystem ownership. |
| Special/nonprivate mode bits | Policy tests reject 0755, 0770, 1700, 2700, 4700 and 0600. |
| AlreadyExists | Direct invocation of the mkdir/open/validate path tests 0755/0770 rejection, 0700 acceptance and symlink rejection. |
| Entry appears between inspection and creation | Deterministic two-walk test supplies a 0755 directory between calls; no nested directory is created and all metadata/content stays intact. |
| New nested and existing 0700 directories | Unit tests plus 12 daemon startup combinations: default/override, new/existing and child umasks 000/022/077. All created components are 0700; existing identity/ownership/sentinel bytes are retained. |
| IPC and CLI compatibility | Real daemon hello, status and doctor; 0600 sockets; duplicate daemon rejected while the first remains responsive; SIGTERM exits successfully and removes the socket. Existing exact output assertions still pass. |
| Database failure remains reachable | The original migration-checksum failure retains its exact stderr, nonzero exit, zero GC rows and invalid checksum assertion with an explicitly private fixture. |

Every daemon rejection compares a recursive, sorted fixture snapshot before and after startup and again after CLI status/doctor. Snapshots include device/inode, full mode, UID/GID, every entry name, regular-file bytes and symlink text, without traversing links. They cover the separate database location and all symlink targets. Equality proves no lock, database, WAL, SHM or socket entry was added. Startup stdout must be empty, stderr must be one line with the stage/reason/recovery instruction, and both CLI commands must report DAEMON_UNAVAILABLE.

Environment overrides and umask changes occur only in child processes. Daemon guards kill and reap owned children on assertion failure; normal paths send SIGTERM and reap them. No process-global test environment or umask mutation is used.

## Historical engineer-run RED/GREEN evidence

This section preserves the original engineer's recorded commands, outcomes and diagnosis. It was not freshly reproduced during this report-only recovery. The retained bug report corroborates the RED command, exit 101 and mode change, but no raw RED or intermediate GREEN log or process-exit receipt was found among the four retained bug artifacts. The raw final Clippy and workspace-test output is embedded below. Reported exit values in this section remain historical claims, not independently recovered process records.

Before changing production provisioning, the original engineer added `permissive_existing_directories_are_rejected_without_mutation` and ran:

```text
cargo test -p arc-daemon --bin arc-daemon permissive_existing_directories_are_rejected_without_mutation
exit 101; 0 passed, 1 failed
assertion `left == right` failed: directory changed
left:  (16777232, 568877163, 16877, 501, 20)
right: (16777232, 568877163, 16832, 501, 20)
```

The unchanged inode/UID/GID with mode changing from 040755 to 040700 directly reproduced the defect. A second pre-fix daemon-unit run also failed this regression (9 passed, 1 failed).

The first attempted Python heredoc edit was denied by zsh because it could not create its temporary heredoc file. No Python edit ran. Applied the same scoped edit with the supported patch tool; the resulting daemon-unit run passed all 10 tests then present.

An intermediate `cargo test --workspace` run failed five resident tests because the first alias implementation expected absolute macOS symlink targets. Scoped `readlink` confirmed `/var -> private/var` and `/tmp -> private/tmp`; accepting those exact relative mappings corrected the failure. Then:

```text
cargo test -p arc-daemon --test resident --test startup
exit 0; 6 resident tests and 1 startup test passed
```

After adding policy, symlink, nested-creation and two-walk race coverage:

```text
cargo test -p arc-daemon --bin arc-daemon --test resident --test startup
exit 0; 15 daemon unit tests, 6 resident tests, 1 startup test passed
```

## Historical pinned-toolchain commands

The original engineer recorded use of the installed toolchain and cache with this PATH prefix and version output. This recovery did not invoke rustc, Cargo or any gate, and the retained log files do not independently record toolchain versions:

```text
/Users/gracefullight/.rustup/toolchains/1.95.0-aarch64-apple-darwin/bin
rustc 1.95.0 (59807616e 2026-04-14)
cargo 1.95.0 (f2d3ce0bd 2026-03-21)
```

`rust-toolchain.toml` has no diff against the accepted baseline. The original report records serial Cargo checks and no standalone build, installation, dependency change, browser suite or external delivery.

| Command | Historical evidence and limit |
| --- | --- |
| `cargo fmt --all` | Original report records exit 0 and applied formatting; no retained output or exit receipt. |
| `cargo fmt --all -- --check` | Original report records exit 0; retained formatter log contains zero bytes. Exit status is not established by that file. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Original report records exit 0; retained output records a successful dev-profile finish. No separate exit receipt. |
| `cargo test --workspace` | Original report records exit 0; retained output records 41 passing tests, zero failures and passing doc-test harnesses. No separate exit receipt. |
| `git diff --check -- crates/arc-daemon/src/server.rs crates/arc-daemon/tests/resident.rs crates/arc-daemon/tests/startup.rs` | Original report records exit 0; not rerun as a product gate during recovery. |
| Scoped diff against f697103 for manifests, Cargo.lock, toolchain, core/CLI/IPC/storage, daemon main/instance | Fresh recovery command returned exit 0 and empty output; these interfaces remain unchanged. |

The supplied mission limits remain unchanged. No historical timestamps or command exits have been inferred from log contents. Fresh pinned-Rust-1.95.0 execution of the exact required fmt, Clippy and workspace-test gates belongs to task-5; final verification and Chief acceptance remain mandatory.

## Review, limits and handoff

The configured code-intelligence provider is Serena; no Serena tools were exposed by this runtime. Used scoped reads of paths supplied by the assignment and compiler rather than recursive native code search. Similar-pattern inspection covered server.rs, instance.rs, main.rs and arc-storage/src/lib.rs: the socket's chmod 0600 is intentional; lock-file mode 0600 is intentional; storage's directory creation does not chmod existing directories and executes only after successful IPC preparation. A repository-wide semantic search was unavailable.

Historical engineer checks were recorded on macOS aarch64; this recovery ran only read-only evidence checks and report validation. Foreign-UID rejection is covered at the live numeric policy boundary, not by changing filesystem ownership. Concurrent replacement after provisioning returns and before later pathname-based operations is not modeled by this suite; the existing main/instance lifecycle was kept compatible. The implementation anchors creation with descriptors, uses no-follow opens and rechecks the final path identity during preparation. Independent QA and Chief final acceptance belong to their assigned Actors.

Acceptance checklist:

- [x] New ARC socket directories are created privately; existing entries are never repaired.
- [x] Ownership, exact mode, directory type and relevant symlink/identity checks precede normal lock/database initialization.
- [x] Unit and daemon/CLI regressions demonstrate rejected-directory and target preservation.
- [x] Explicit private fixtures preserve migration, startup, CLI, socket mode, single-instance and shutdown coverage.
- [x] Retained log output, its provenance and current source hashes are embedded in this report.
- [ ] Fresh independent integrated pinned-toolchain gates, task-5 QA evidence and Chief acceptance remain pending.

The implementation checklist above describes retained code and historical regression evidence; it is not independent integrated acceptance. The report-availability defect is corrected by embedding the evidence, without claiming that ignored logs crossed workspaces.

## Recovery observations and source identity

Only `docs/ipc-provisioning-implementation.md` is owned and changed in this recovery. The retained `-1` source bytes equal the current `-5` source bytes for each of the following files; SHA-256 was computed freshly with Python hashlib during this attempt:

| Path | SHA-256 |
| --- | --- |
| `crates/arc-daemon/src/server.rs` | `693b6b896f4b0229e9eb27b425db87717af68f7bf07c0198b89c22a2b87863e0` |
| `crates/arc-daemon/tests/resident.rs` | `cac6890f2aef752e47359f6a421f0b8f5a7cbfa8e17abb35f8e0e0a8b17d1da2` |
| `crates/arc-daemon/tests/startup.rs` | `e1b55d7ec35f39929c7341bd5d0dc81ba21ded8bea808aade93e1365699037e1` |

These hashes identify the retained code accompanying the historical evidence; the logs contain no source-hash receipt binding them to a process run. Task-5 must retain fresh output, actual exits, toolchain identity and final source hashes in its owned QA report.

Fresh noninteractive checks succeeded: scoped `git status --short --untracked-files=normal -- . ':!av.yaml'`; `git log -3 --format='%H %P %s'`; scoped baseline diffs; retained-workspace `git check-ignore -v`; and `python3 -c` reads, log-size/hash checks, embedded-log comparisons and source-byte comparisons. Each returned exit 0. Broad `git status --short --untracked-files=normal` returned exit 0 with `av.yaml: Operation not permitted`; no inference about that file's contents or status is made. An initial Python heredoc command failed before Python ran with `zsh:1: can't create temp file for here document: operation not permitted` (exit 1); `python3 -c` then performed the read-only checks successfully. The Python shim emitted a mise tracking-symlink warning; Python itself returned exit 0. No GUI workaround was used.

Out-of-scope observation for Chief reconciliation: `git diff f6971037ed1c3524f88e10732e8910909a4facd3 -- .qwen/settings.json` shows the addition of `"$version": 4` after the privacy object. This addition predates this recovery; the file was preserved without edits or reversion. Other Actors' reports, agent definitions, mission state, manifests, lockfile, migrations, toolchain settings and ignore rules were not changed.

Prior recovery validation recorded byte-for-byte embedded-log comparisons and a corrected trailing-blank-line failure. This attempt independently rechecked the retained files: both embedded blocks equal their retained files byte-for-byte, all log/source hashes match, and the three Rust files remain byte-identical to `-1`. Scoped `git diff --stat f697103 HEAD` shows only server.rs, resident.rs, startup.rs, the four docs and `.qwen/settings.json`. Final report assertions and `git diff --check -- docs/ipc-provisioning-implementation.md` are recorded below after this attempt's edit.

Report-only validation results for this recovery:

| Command | Exit | Observed result |
| --- | --- | --- |
| `python3 -c` evidence assertions | 0 | Both embedded nonempty logs match retained bytes; all three log hashes and Rust hashes match; Rust bytes equal `-1`; protected baseline diffs are empty; only this report differs from recovery HEAD. |
| `git diff --check -- docs/ipc-provisioning-implementation.md` | 0 | No output. |
| `git -C ../efcac0c89f4fc294-7afaa346b4bf92bf-1 check-ignore -v .agents/results/bugs/ipc-provisioning-fmt.log .agents/results/bugs/ipc-provisioning-clippy.log .agents/results/bugs/ipc-provisioning-test.log` | 0 | All three paths match `.gitignore:5:.agents/results/`. |

These commands verify the report and preservation of retained inputs. No Cargo command was executed in this recovery; historical test output below remains historical engineer-run evidence.

The required independent framework review must inspect the integrated report read-only and return the required review JSON. It remains separate from this report correction. Remaining risks are the verification gap described above and the retained lifecycle limit already documented; no new product defect was established during this recovery.

## Embedded historical engineer-run logs

These are the complete retained Clippy and workspace-test outputs, read during recovery without re-executing their commands. Command associations come from the original engineer report; the files have no command-line header or explicit process-exit record. The empty formatter file is described by size and hash rather than represented as proof of a successful check.

### ipc-provisioning-fmt.log

Retained bytes: 0. SHA-256: `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.

No output. An empty file does not establish command execution, success or exit status.

### ipc-provisioning-clippy.log

Retained bytes: 348. SHA-256: `91677eb7fadff386acd64f845078ef43574bf1497d861054b392f6d48f0649ea`.

```text
    Checking arc-daemon v0.1.0 (/Users/gracefullight/workspace/arc/.agent-valley/operations/0b22258d-d7c5-4fb0-bc7b-73945a671689/workspaces/fa572687-8b27-4ea3-8c17-867d5159fa1d/workspace/.agent-valley/parallel/workspaces/efcac0c89f4fc294-7afaa346b4bf92bf-1/crates/arc-daemon)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.88s
```

### ipc-provisioning-test.log

Retained bytes: 5247. SHA-256: `cf087e631c037361557d6b61d7770c2de30109d8881f0dc4782edba6128ff161`.

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.13s
     Running unittests src/main.rs (target/debug/deps/arcctl-38de850384e9723c)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/binary.rs (target/debug/deps/binary-9838723da50eec88)

running 2 tests
test arcctl_binary_is_built_next_to_the_test_harness ... ok
test no_arguments_names_both_commands_and_exits_nonzero ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s

     Running unittests src/lib.rs (target/debug/deps/arc_core-49713ac473ec0398)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/main.rs (target/debug/deps/arc_daemon-0c0e7c0845741242)

running 15 tests
test instance::tests::a_stale_inode_still_named_by_the_path_is_unlinked ... ok
test server::tests::a_shared_temporary_root_is_refused_instead_of_tightened ... ok
test instance::tests::a_live_duplicate_is_rejected ... ok
test instance::tests::a_stale_inode_never_unlinks_the_successors_lock_file ... ok
test instance::tests::a_dead_pid_is_recovered_without_killing_anyone ... ok
test instance::tests::a_mismatched_start_time_is_recovered_without_killing_anyone ... ok
test server::tests::foreign_uid_and_nonprivate_modes_are_rejected_by_the_live_policy ... ok
test server::tests::a_directory_that_appears_between_walks_is_validated_before_nested_creation ... ok
test instance::tests::dropping_a_stale_holder_preserves_its_successors_lock ... ok
test server::tests::already_exists_is_validated_without_repair_or_following_links ... ok
test server::tests::permissive_existing_directories_are_rejected_without_mutation ... ok
test server::tests::new_nested_and_existing_private_directories_are_usable_without_repair ... ok
test server::tests::final_intermediate_and_dangling_symlinks_are_not_followed ... ok
test server::tests::a_positional_array_is_rejected_and_the_connection_stays_usable ... ok
test server::tests::a_socketpair_serves_hello_and_survives_an_incompatible_version ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/resident.rs (target/debug/deps/resident-dba49166c3e84247)

running 6 tests
test commands_report_daemon_unavailable_when_absent ... ok
test nondirectory_parent_is_rejected_before_lock_or_database_initialization ... ok
test resident_contract_holds_until_sigterm ... ok
test permissive_directories_and_their_contents_survive_daemon_and_cli_rejection ... ok
test symlink_targets_and_dangling_links_survive_daemon_and_cli_rejection ... ok
test private_existing_and_new_nested_directories_support_default_and_override_startup ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.08s

     Running tests/startup.rs (target/debug/deps/startup-14b51a907450ac85)

running 1 test
test migration_failure_exits_nonzero_with_one_stderr_sentence ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running unittests src/lib.rs (target/debug/deps/arc_ipc-aaaac96664fdee2d)

running 13 tests
test tests::an_object_request_still_decodes_after_a_rejected_array ... ok
test tests::hello_and_health_do_not_require_the_database ... ok
test tests::oversize_declared_length_is_rejected_without_reading_the_body ... ok
test tests::a_framed_array_is_rejected_without_reaching_a_handler ... ok
test tests::non_object_roots_are_rejected ... ok
test tests::frame_round_trips_one_json_object ... ok
test tests::multiple_json_values_are_rejected ... ok
test tests::doctor_requires_ok_integrity_and_one_migration ... ok
test tests::protocol_version_99_returns_incompatible_and_a_later_hello_succeeds ... ok
test tests::unknown_operation_is_unsupported ... ok
test tests::responses_are_objects_too ... ok
test tests::writing_non_object_messages_is_rejected_before_emitting_a_frame ... ok
test tests::zero_length_is_rejected_without_reading_a_body ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/arc_storage-e9132bfe8c0fb324)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/nfr010.rs (target/debug/deps/nfr010-d4da681c436e96e0)

running 4 tests
test migration_failure_rolls_back_all_new_tables ... ok
test foreign_keys_and_unique_resource_identity_are_enforced ... ok
test nfr010_wal_migration_survives_close_and_reopen_without_cleanup ... ok
test mismatched_checksum_is_rejected_without_reapplying_migration ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

   Doc-tests arc_core

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests arc_ipc

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests arc_storage

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```
