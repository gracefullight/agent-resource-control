# Runtime identity implementation

Status: implementation complete; required Cargo gates passed. Independent review and the Chief's integrated-worktree decision remain separate tasks.

## Source scope and ownership

Accepted committed baseline: `073802555b639aa2fec183d93f748d9437cbbcad`, tree `ce009646036415990a815170955421e1ab8d5c2f`.

This Actor started at isolated snapshot HEAD `7e46a56f0df04a0c0c4716e1cbb494e56d4ef4c5`, whose tree is also `ce009646036415990a815170955421e1ab8d5c2f`. The first scoped status inspection was empty. The saved `initial.json` was collected after adding the RED regression and therefore records that new test directory; it does not represent pre-existing work. All implementation observations below concern this Actor's uncommitted working files, not the committed baseline or a different review checkout.

The Runtime Engineer changed:

- `Cargo.lock` and `crates/arc-core/Cargo.toml`: connect arc-core to the existing cached workspace libc dependency; no version upgrades.
- `crates/arc-core/src/lib.rs` and new `crates/arc-core/tests/runtime.rs`: native runtime resolution, admission-anchor resolution and regression coverage.
- `crates/arc-daemon/src/{main,instance,server}.rs`: admission ordering, lock validation, socket preflight and native-root classification.
- `crates/arc-daemon/tests/{resident,startup}.rs` and new `tests/support/mod.rs`: native-root fixtures, cross-process serialization and lifecycle/preservation regressions.
- `crates/arc-storage/tests/nfr010.rs`: private native-root fixtures, child-only environment configuration and the fixed migration digest assertion.
- This report and `.arc-verify/runtime-identity/implementation/`: implementation evidence.

The CLI already calls `arc_core::socket_path()`, so its source and protocol need no changes. The root manifest, storage implementation and migration were not changed. There was no ownership handoff or shared-manifest conflict. Other Actors' reports and mission state were not edited.

## Cause and fix

The baseline default endpoint used `env::temp_dir()`, while admission appended `.lock` to the requested socket. Different launcher environments could select different defaults, and different explicit sockets admitted separate daemons for one user. The original directory classifier had a second TMPDIR dependency. Startup also initialized SQLite before discovering some invalid socket entries or socket-length errors.

`runtime_root() -> io::Result<PathBuf>` now queries `confstr(_CS_DARWIN_USER_TEMP_DIR)` on macOS. It sizes the buffer, retries growth, validates the returned length and final NUL, rejects embedded NULs, empty/relative paths and repeated growth, and preserves native bytes through `OsString::from_vec`. A failed native query returns an error; it never falls back to TMPDIR. Other platforms retain the existing temporary-root behavior for their default socket.

`socket_path()` keeps nonempty `ARC_SOCKET_PATH` precedence and otherwise returns `<runtime_root>/arc/arc.sock`. Empty overrides still select the default. Persistent database resolution is unchanged: macOS uses `HOME/Library/Application Support/arc/arc.sqlite3` unless `ARC_DB_PATH` is nonempty.

`instance_anchor() -> io::Result<PathBuf>` returns `<native-root>/arc-instance-<effective-uid>/daemon.lock` on macOS. Other Unix platforms use `/tmp/arc-instance-<effective-uid>/daemon.lock`. Neither socket/database overrides nor TMPDIR select the anchor. This is production behavior; no test bypass was added.

Startup order is:

1. Resolve database and socket paths without creating them.
2. Read-only socket preflight: platform `sockaddr_un.sun_path` byte limit, NUL rejection, no-follow parent traversal, private final parent if present, and existing socket-entry validation.
3. Validate an existing anchor entry, prepare its owner-only directory, and acquire `InstanceLock` on the stable anchor.
4. Prepare the requested socket directory, then open/migrate SQLite, bind and announce readiness.
5. Retain admission until server shutdown and database/socket cleanup finish.

The lock reuses the directory acquisition guard, OS file lock, PID/start token, and inode-aware stale recovery. Existing entries must be regular files owned by the effective UID with exactly 0600 permissions. Validation occurs before owner-record truncation; `O_NOFOLLOW` and `O_NONBLOCK` reject links and avoid FIFO blocking. The directory guard uses the existing descriptor-relative no-follow traversal and exact 0700 policy. The lock pathname stays after shutdown. Duplicate wording retains `socket already served` for compatibility and explains the per-user restriction, naming the anchor rather than claiming that the losing endpoint is live.

Socket validation measures native bytes against the platform structure's capacity, including room for the terminator: 103 pathname bytes on this macOS host. It runs before directory creation, lock-content writes, SQLite or stale-socket removal and is repeated at bind. Existing non-sockets, links, foreign-owned entries and sockets with non-0600 modes are rejected without repair. Native-root classification replaces the macOS TMPDIR comparison. Existing ordinary ancestors, narrow `/var` and `/tmp` system aliases, symlink rejection and content-preservation rules remain in use.

## Fixtures and requirement map

All affected scratch helpers use `arc_core::runtime_root()` and short, privately created 0700 directories. Daemon-owning tests in both integration binaries hold a shared `support::Serial` guard for their entire fixture lifecycle. It combines a thread mutex and a separate owner-only file lock at `<native-root>/arc-fixtures-<uid>.lock`. Child cleanup guards kill only their own children when necessary and reap them before releasing coordination. Contenders remain concurrent inside the shared fixture guard: both gated child processes exist before either is released through stdin into `exec` of the daemon. Production uniqueness is unchanged during tests.

Former TMPDIR-relocated “default” directory matrices now use explicit socket paths for isolated unsafe/shallow/nested cases. All their permission, sentinel, symlink, dangling-link, intermediate-link, `..`, umask and inode assertions remain. A separate test exercises the actual native default endpoint, removing `ARC_SOCKET_PATH`, across all three daemon environments and all three client environments. It does not relocate the default or modify unrelated runtime directories.

| Requirement | Regression evidence in final workspace run |
| --- | --- |
| confstr sizing, growth, native bytes and malformed/error responses | `confstr_preserves_native_bytes_and_retries_a_growing_buffer`; `confstr_rejects_errors_malformed_paths_and_unstable_lengths` |
| Default endpoint independent of unset/changed TMPDIR; persistent database policy | `native_default_is_independent_of_child_tmpdir` |
| Nonempty and empty override policies; stable anchor across overrides | `nonempty_socket_override_and_empty_override_policy_are_preserved` |
| Real default endpoint; demonstrably unwritable TMPDIR; status and doctor across different child environments; 0700 directories and 0600 socket/anchor | `native_default_and_cli_ignore_unset_changed_and_unwritable_tmpdir` |
| Concurrent distinct socket names/directories, database paths and TMPDIR; one winner; loser has no readiness, database parent, database/WAL/SHM or socket directory; winner status/doctor; graceful restart; forced-death stale recovery | `concurrent_distinct_overrides_admit_one_and_recover_after_shutdown_and_death` |
| Existing directory permissions, contents, inodes and owners preserved; symlink and dangling-link targets unchanged | `permissive_directories_and_their_contents_survive_daemon_and_cli_rejection`; `symlink_targets_and_dangling_links_survive_daemon_and_cli_rejection`; `nondirectory_parent_is_rejected_before_lock_or_database_initialization` |
| New and existing private shallow/nested directories under child umasks 000/022/077 | `private_existing_and_new_nested_directories_support_override_startup` |
| Overlong paths reject before mkdir, lock writes, database or existing-entry modification; socket symlinks/files preserved | `overlong_socket_paths_and_existing_socket_entries_reject_without_mutation`; `socket_length_validation_counts_native_bytes_and_preserves_entries` |
| Unsafe existing anchor modes, links, directories and FIFO; foreign UID and special mode policy | `unsafe_anchor_entries_are_rejected_without_repair_or_writes`; `foreign_uid_and_nonprivate_modes_are_rejected_by_the_live_policy` |
| Dead PID, PID reuse/start mismatch, live duplicate and stale-inode successor preservation | Existing six `instance::tests` recovery/duplicate tests, all passing |
| Cross-process fixture coordination without disabling test concurrency globally | `fixture_coordination_is_shared_across_processes`, using the separately spawned `fixture_serialization_probe` |
| Resident protocol, exact CLI output, unavailable failures, no TCP listener and SIGTERM cleanup | `resident_contract_holds_until_sigterm`; `commands_report_daemon_unavailable_when_absent` |
| Exact migration-failure diagnostic and no reapplication; fixed frozen migration digest; storage reopen/integrity | `migration_failure_exits_nonzero_with_one_stderr_sentence`; `nfr010_wal_migration_survives_close_and_reopen_without_cleanup`; existing storage failure/constraint regressions |

Only child processes receive environment or umask changes. Readiness, rejection, coordination and shutdown waits are bounded. The actual default endpoint test leaves the persistent production anchor and private default directory intact, while removing its owned socket through graceful shutdown.

## Executed checks

Platform: macOS/Darwin 27.2.0, arm64. Installed rustc `1.95.0 (59807616e 2026-04-14)` and cargo `1.95.0 (f2d3ce0bd 2026-03-21)`. Fixture root: `/var/folders/r9/0qvpcx794n385zwgzwfvt55h0000gn/T/`. Existing cached dependencies were sufficient; no installation, download, standalone build or browser suite was used.

The RED subprocess test was added before the resolver fix. `cargo test -p arc-core --test runtime native_default_is_independent_of_child_tmpdir` reported failure: an unset TMPDIR selected the native path, while `/changed/runtime/root` selected a different socket. Its shell wrapper printed the log and therefore did not capture Cargo's individual exit code. The failure text is retained in `red.txt`. The post-fix core run passed in `core-green.txt`.

Focused daemon/resident checks and the first full workspace attempt identified two fixture-only sandbox limits: setuid is stripped from actual files, and creation of a non-UTF-8 filename is denied. The final tests retain direct policy/native-byte boundary coverage and use filesystem-supported modes and ASCII names for independent inode/content assertions. Earlier failures and their source manifests are retained rather than presented as passes.

Final commands, executed without changing their arguments:

| Command | Exit | Duration |
| --- | --- | --- |
| `cargo fmt --all -- --check` | 0 | 0.179 s |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 1.037 s |
| `cargo test --workspace` | 0 | 5.440 s |
| Scoped `git diff --check` | 0 | checked after Cargo gates |

The final workspace test run passed 53 tests, including child-probe entrypoints, with zero failures or ignored tests. Exact argv, timestamps, exit codes and log locations are in `commands.json`; command stdout/stderr is in `fmt.txt`, `clippy.txt` and `test.txt`. `run_checks.py` records the unchanged source manifest before/after checks. Earlier gate attempts are archived in `gate-attempt-1/` and `gate-attempt-2/`; the second archive is a passing run before making the storage fixture directory explicitly 0700.

Final source fingerprint: `84110f4f1a4da327a8fabec09bb0cf642b1e4ea4ececbf173d2d8c40f00088f9`. Its scope is all tracked files under `crates/`, root Cargo manifest/lock and toolchain file, plus both new Rust test/helper files. `source.json` lists each native file's SHA-256 and mode. The fingerprint is SHA-256 of the sorted compact JSON file map, not a Git commit/tree hash. The source was unchanged during the final gates. Reviewers must verify these working-file hashes after integration; this report does not claim the baseline or snapshot commit contains the edits.

`crates/arc-storage/migrations/0001_phase0.sql` remains byte-for-byte unchanged at SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`.

## Evidence limits and handoff

The OMA debug definition, skill, execution policy, result contract, code-intelligence capability contract and debug rule were read. No OMA workflow was self-started and no agents were dispatched: this is the supplied AV-managed Actor assignment. Serena exposed no available tools; navigation used scoped reads of supplied paths. The read audit covered the resolver, CLI caller, startup, server classification, instance implementation and all affected fixture helpers.

The sandbox denied a zsh temporary here-document, `ps`, access to unrelated ignored `av.yaml` during an unscoped diff, and mise tracking symlinks. These were recorded and avoided using supported noninteractive reads/patches and scoped Git checks. None of those restrictions was bypassed. Test-owned native-root creation, Unix sockets, locks, subprocess execution and reaping worked. Real foreign-UID ownership changes and actual setuid files were not claimed; their rejection policies are unit-tested with explicit attributes. Native-byte conversion and path-length validation are tested; this sandbox did not permit creating non-UTF-8 filenames. Non-macOS execution was not performed.

The repository's additional `./scripts/harness/validate.sh` invocation exited **1**, recorded separately in `harness.json`/`harness.txt`. Secret, dangerous-command and layer checks passed. It reports server.rs at 762 lines, resident.rs at 901 lines, and arc-ipc/src/lib.rs at 549 lines against its 500-line cap, then `Script not found "test:coverage"`. The accepted baseline already has those three files at 690, 655 and 549 lines respectively; this repair adds 72 and 246 lines to the two owned files. `baseline-comparison.json` records those exact comparisons. The IPC file and legacy coverage setup are outside this Actor's ownership. A structural split of the owned modules should be coordinated with any harness follow-up; it was not mixed into the functional debug patch. No harness definitions, coverage settings, acceptance commands or migration bytes were changed to suppress findings. The Chief must retain this additional validation failure in the mission-level assessment.

No required Rust implementation case remains unresolved. The Chief still owns integration, independent technical/acceptance review, usage/value/final reports and the mission-level evidence file. Those reports must use the accepted baseline above and verify the delivered uncommitted source rather than substituting this Actor's snapshot HEAD.
