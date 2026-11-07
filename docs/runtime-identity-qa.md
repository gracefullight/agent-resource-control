# Independent macOS runtime-identity QA

## Review Result: WARNING

Task-5 fixed runtime acceptance: **PASS**. The three exact Cargo gates passed in this integrated macOS worktree, all required cases executed, and no required runtime finding remains. The warning concerns the separately executed repository architecture validator: it still fails and needs Chief disposition. This report does not declare architecture conformance or mission completion.

Status: assigned QA completed. Skill used: `oma-qa`, including its execution protocol and checklist. Product files, tests, manifests, lockfiles, migration, other reports and mission state were not edited. No agents were dispatched.

## Source and execution scope

Accepted baseline and current HEAD: `073802555b639aa2fec183d93f748d9437cbbcad`; baseline tree: `ce009646036415990a815170955421e1ab8d5c2f`. Initial scoped Git status showed the integrated repair and four prerequisite reports as uncommitted changes. They were preserved. The tested implementation is the working-file overlay, not the accepted commit's unchanged tree.

Final source fingerprint: `84110f4f1a4da327a8fabec09bb0cf642b1e4ea4ececbf173d2d8c40f00088f9`. The independently recomputed manifest covers all tracked files under `crates/`, root Cargo manifest/lock and toolchain file, plus `crates/arc-core/tests/runtime.rs` and `crates/arc-daemon/tests/support/mod.rs`: 23 files. Each entry contains its SHA-256 and permission mode; the fingerprint is SHA-256 of that sorted compact JSON map. It matches every delivered implementation entry and was unchanged before/after the Cargo gates. It is not a Git revision.

`crates/arc-storage/migrations/0001_phase0.sql` was compared directly with `git show 073802555b639aa2fec183d93f748d9437cbbcad:crates/arc-storage/migrations/0001_phase0.sql`. Baseline and final bytes are equal. Both SHA-256 values are `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`; the executed storage test also asserts that frozen digest.

Inputs consumed: [implementation and requirement map](runtime-identity-implementation.md), [technical review](runtime-identity-technical-review.md), [usage and recovery](runtime-identity-usage.md), and their linked source and fixtures. Prior reports supplied review leads; the acceptance below comes from this review's new executions.

## Exact gates and observed platform

Executed on 2026-11-07, beginning 10:21:09 UTC. `uname -srm` returned `Darwin 27.2.0 arm64`; Python independently identified Darwin/macOS. Installed versions: rustc `1.95.0 (59807616e 2026-04-14)`, cargo `1.95.0 (f2d3ce0bd 2026-03-21)`, clippy `0.1.95 (59807616e1 2026-04-14)`, rustfmt `1.9.0-stable (59807616e1 2026-04-14)`.

| Exact command | Exit | Duration | New evidence |
| --- | --- | --- | --- |
| `cargo fmt --all -- --check` | 0 | 0.174 s | [fmt.txt](../.arc-verify/runtime-identity/qa/fmt.txt) |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 17.186 s | [clippy.txt](../.arc-verify/runtime-identity/qa/clippy.txt) |
| `cargo test --workspace` | 0 | 22.380 s | [test.txt](../.arc-verify/runtime-identity/qa/test.txt) |

All argv were unchanged. The runner used the prepared `/Users/gracefullight/.cargo` cache with `CARGO_NET_OFFLINE=true` and an isolated `CARGO_TARGET_DIR` under the owned QA directory. Nothing was installed or downloaded. Commands ran serially, once each, with a 120-second timeout each. No standalone build was run. [commands.json](../.arc-verify/runtime-identity/qa/commands.json) records exact argv, timestamps, exits, environment and output locations.

The workspace run passed **53 tests**, with zero failed, ignored or filtered out: 2 CLI, 2 core unit, 3 core subprocess/probe, 17 daemon unit, 11 resident/probe, 1 startup, 13 IPC and 4 storage. The count includes probe entrypoints, not 53 distinct feature scenarios. The actual log names both macOS-only confstr unit tests, `native_default_is_independent_of_child_tmpdir` and `native_default_and_cli_ignore_unset_changed_and_unwritable_tmpdir` as executed and passing. Their platform guards did not skip the required behavior. Child-probe and storage-child branches are invoked by their passing parent tests; their top-level no-op entrypoints alone are not used as evidence.

## Acceptance matrix

Every row below passed in the new [workspace test log](../.arc-verify/runtime-identity/qa/test.txt). Full names and requirement mappings are also recorded in [runtime-identity-evidence.json](runtime-identity-evidence.json).

| Required behavior | Executed cases | Assertions independently inspected |
| --- | --- | --- |
| Native runtime resolver | `confstr_preserves_native_bytes_and_retries_a_growing_buffer`; `confstr_rejects_errors_malformed_paths_and_unstable_lengths` | Native bytes survive conversion; sizing/growth retry works; failure, empty/relative/malformed paths, embedded NUL and unstable lengths reject. Source uses native confstr without a TMPDIR fallback. |
| Default identity and configuration policy | `native_default_is_independent_of_child_tmpdir`; `nonempty_socket_override_and_empty_override_policy_are_preserved` | Child unset/changed/unwritable-path TMPDIR outputs agree; empty socket override selects default, nonempty override is honored; anchor is stable. macOS persistent database policy is retained. |
| Actual default endpoint and both CLI commands | `native_default_and_cli_ignore_unset_changed_and_unwritable_tmpdir` | A real 0500 TMPDIR fixture first rejects a write. Three daemon environments each serve three client environments for status and doctor: 18 successful invocations. Status names the same native default endpoint; changed/unwritable roots gain no `arc` directory. Default/anchor directories are owned 0700; socket/anchor are owned 0600. |
| Concurrent distinct overrides and losing-start preservation | `concurrent_distinct_overrides_admit_one_and_recover_after_shutdown_and_death` | Two child processes exist before either receives its launch gate. Socket names/directories, database locations and TMPDIR differ. Exactly one exits nonzero, without readiness, with the per-user duplicate reason. Its entire fixture snapshot is unchanged; database parent, DB/WAL/SHM and socket directory remain absent. Winner answers status and doctor. Either winner is accepted symmetrically. |
| Graceful and forced-death recovery | Same concurrent lifecycle case; `resident_contract_holds_until_sigterm` | SIGTERM exits successfully and removes the socket. Graceful restart answers both commands. The owned restarted process is killed/reaped; its stale socket remains. A further restart reacquires admission, answers both commands and shuts down cleanly. |
| Lock recovery and successor preservation | Six `instance::tests` duplicate/stale-inode/PID cases | Live duplicate rejected; dead PID and mismatched process-start token recover without killing anyone; stale-holder destruction and stale-inode cleanup preserve a successor. |
| Private new/existing directories | `private_existing_and_new_nested_directories_support_override_startup`; server private-directory cases | Shallow/nested and existing/new directories work under child umasks 000/022/077. Modes remain 0700/0600; existing directory identity and sentinel contents are retained. |
| Unsafe directories and symlinks reject without mutation | `permissive_directories_and_their_contents_survive_daemon_and_cli_rejection`; `symlink_targets_and_dangling_links_survive_daemon_and_cli_rejection`; `nondirectory_parent_is_rejected_before_lock_or_database_initialization` | Recursive no-follow snapshots compare names, device/inode, UID/GID, modes, file bytes and link targets. Final/intermediate/dangling links, missing-prefix plus `..`, permissive directories and nondirectory parents reject before DB/socket initialization. Both CLI commands fail without changing the fixture. |
| Overlong paths and unsafe socket entries | `overlong_socket_paths_and_existing_socket_entries_reject_without_mutation`; `socket_length_validation_counts_native_bytes_and_preserves_entries` | Native-byte capacity boundary is checked; 104-byte names and overlong existing entries reject. Anchor contents and fixture snapshots remain unchanged; missing prefixes are not created. Socket symlink targets and regular-file entries are preserved. |
| Unsafe anchor entries and owner policies | `unsafe_anchor_entries_are_rejected_without_repair_or_writes`; `foreign_uid_and_nonprivate_modes_are_rejected_by_the_live_policy` | Existing unsafe lock modes, symlink, directory and FIFO reject. Regular-file contents/inode/mode/owner are retained. Foreign UID and special-mode policies are directly exercised with attributes; this is not an actual foreign-owned-file claim. |
| Existing shared-root and raced-creation protections | Server shared-root, already-exists and between-walks tests | Native runtime root cannot be the final socket parent; appearing unsafe components and symlinks reject without repair or nested creation. |
| Cross-process fixture coordination | `fixture_coordination_is_shared_across_processes` | A separate test process reaches the ready marker, cannot record acquisition while the parent owns coordination, then succeeds after release. Both integration binaries use the same separate no-follow 0600 fixture-lock protocol. The marker check alone is not an exhaustive scheduler proof. |
| Retained CLI/protocol/storage behavior | `resident_contract_holds_until_sigterm`; `commands_report_daemon_unavailable_when_absent`; startup and nfr010 cases | Exact status/doctor stdout, unavailable code, continued service after incompatible protocol, no TCP listener, integrity/migration count, exact checksum-failure sentence, no migration reapplication and normal persistent WAL reopen remain asserted. |

## Independent implementation and test review

Security and reliability inspection found no required correction. `arc-core::runtime_root` calls `confstr(_CS_DARWIN_USER_TEMP_DIR)` on macOS and preserves native bytes. `socket_path` keeps the existing override policy; `database_path` retains the persistent macOS application-support path. The only dependency delta is arc-core's use of the existing workspace libc package; no package versions or workspace members changed.

`main.rs::prepare_instance` performs read-only socket preflight, validates/prepares the fixed anchor and acquires admission before preparing override socket directories or opening SQLite. The instance guard remains live through serving and cleanup. Socket length and existing-entry validation precede mutation and repeat at bind. Directory traversal uses descriptor-relative no-follow opens, exact final UID/0700 validation, narrow macOS system aliases and raced-component checks. Existing anchor entries are checked before owner-record writes; opening uses `O_NOFOLLOW|O_NONBLOCK`, exact UID/0600 and regular-file checks. The existing directory acquisition lock, process-start identity and inode-aware stale recovery are reused.

No production test escape hatch was found. Probe environment variables are confined to tests. Daemon-owning resident/startup fixtures hold `support::Serial` throughout their complete lifecycle; its cross-process lock is separate from production admission. Each owned resident child has a cleanup guard; startup's bounded wait also reaps its owned child. Contenders remain concurrent within fixture serialization. Neither the CLI nor the daemon receives a test-only uniqueness bypass. Environment and umask changes are child-local. No `#[ignore]` or early platform-success return hides a required macOS case in the inspected scope.

The CLI source/protocol, nonempty socket override and production persistent-database interface remain compatible. The intended semantic change is one admission slot per user across differing endpoints/databases. Usage guidance correctly states that the default client does not discover arbitrary socket overrides and distinguishes fixed-anchor rejection from socket-directory rejection. GUI, accessibility and browser tests do not apply to this scoped Rust daemon/CLI task.

## Fixture roots and permissions

An independent native `confstr` sizing/query and `getconf DARWIN_USER_TEMP_DIR` both returned `/var/folders/r9/0qvpcx794n385zwgzwfvt55h0000gn/T/`. The root is UID 501, mode 0700. An exclusively owned QA directory and file were successfully created beneath it; that probe was removed after verification. The tests create their short private fixtures under this supported native root.

Post-test metadata confirms 0700 on the native root, default `arc` directory and `arc-instance-501` directory, and 0600 on `daemon.lock` and `arc-fixtures-501.lock`, all UID 501. Their pre/post inodes agree. Live socket modes are asserted during the executable fixtures; no live socket is expected after successful shutdown. Exact root, probe and metadata evidence is in [environment.json](../.arc-verify/runtime-identity/qa/environment.json) and [runtime-after.json](../.arc-verify/runtime-identity/qa/runtime-after.json).

## Additional repository check and Chief handoff

### CRITICAL

None found.

### HIGH

None found.

### MEDIUM

- `crates/arc-daemon/src/server.rs:1`, `crates/arc-daemon/tests/resident.rs:1`, `crates/arc-ipc/src/lib.rs:1` — the additional validator rejects lengths 762/901/549 against 500. Direct baseline reads show 690/655/549, so every file already exceeded the cap and the first two grew. Remediation: Chief assigns a separate preservation-focused split to the owners, such as moving directory provisioning into a server submodule and resident scenarios into modules sharing the existing support helper. Keep the exact Cargo gates and all assertions. This reviewer did not implement or waive the split.
- `scripts/harness/validate.sh:420` — coverage validation executes `bun run test:coverage`, which fails because this Rust workspace has no package.json or Vitest configuration. Remediation: Chief coordinates the validator owner and the Rust verification owner to supply an applicable coverage contract and executable check. Do not set `SKIP_COVERAGE`, add a dummy success script or replace the fixed Cargo commands to suppress this result. No numerical coverage result is available.

### LOW

No additional required runtime finding.

The actual additional command `./scripts/harness/validate.sh` exited **1** after 43.775 seconds. Secrets, dangerous-command and architecture-layer checks passed; three size violations and the missing coverage script failed. `WORKSPACE_ROOT` pointed to a QA-owned scratch directory; no skip flag was added. See [harness.txt](../.arc-verify/runtime-identity/qa/harness.txt), [harness.json](../.arc-verify/runtime-identity/qa/harness.json) and [baseline-comparison.json](../.arc-verify/runtime-identity/qa/baseline-comparison.json).

These findings do not change the fixed task-5 Cargo/runtime outcomes. They leave repository architecture conformance failed and belong in the Chief's final decision. Ownership changes outside this assignment require Chief coordination. This report is not a waiver, repair or substitute for the final Chief report.

## Sandbox limits

- This run's shell and noninteractive subprocess tools worked. Mise repeatedly emitted `Operation not permitted` while creating its tracking symlink outside the workspace; it did not prevent any gate. No GUI fallback or permission bypass was used. The earlier reviewers' `grantpt` failure is historical, not this run's execution result.
- Serena exposed no tools. The configured provider is Serena; review used direct reads of the assignment-located files and scoped Git evidence, without substituting native repository code search. Missing optional YAML/local OMA configuration files were not created; the existing CUE configuration was read.
- Actual foreign-UID files, real setuid-file preservation and non-UTF-8 filesystem creation were not exercised by this QA. Executed tests directly cover UID/special-mode policy and native-byte conversion/length; supported filesystem entries cover content and inode preservation. The engineer separately reported setuid stripping and non-UTF-8 creation denial. Those reports are not promoted to new executions here.
- Daemon tests used owned disposable children and isolated database overrides. They did not open a user's production database or terminate an unrelated daemon. A competing unrelated daemon would cause admission failure rather than being displaced; this suite completed successfully.
- Non-macOS execution and adversarial same-UID namespace replacement are not established by these results. Gated contenders test real concurrent starts, with winner choice left to kernel scheduling.
- No configured container target was supplied in the saved assignment; `av.example.yaml` has only a commented example. No container health, availability or recent-error claim is made. No deployment/restart was attempted.
- No OMA claim/run/session identity was injected. The assigned AV output paths were used directly; no new mission/OMA run, saved task, graph, assignment or state was created or changed.

No required macOS acceptance case was denied, unavailable or left unexecuted. There were no failed Cargo attempts in this QA run. An early read of final capture files occurred while the runner was still active and found them absent; their completed versions were subsequently read and verified. Historical failed attempts remain in preceding Actors' artifacts and were not substituted for this execution.

## Deliverables and acceptance checklist

Changed paths owned by this task: `docs/runtime-identity-qa.md`, `docs/runtime-identity-evidence.json`, and new QA capture scripts, manifests/logs plus a local generated-target ignore rule under `.arc-verify/runtime-identity/qa/`. Compiler outputs remain in the ignored QA target directory. Existing implementation logs and preceding reports were preserved.

Commands executed include the three exact Cargo gates above; `uname -srm`; `getconf DARWIN_USER_TEMP_DIR`; tool version queries; scoped `git status`, `git rev-parse`, baseline `git show`, `git diff --check`; the independent capture/assembly scripts; and the additional harness validator. Platform/version/Git/root commands and source diff check exited 0. The combined optional OMA YAML/local-config read exited 1 for absent files. `git check-ignore` initially confirmed the generated target was not ignored (exit 1); the task-local ignore rule now excludes it. Read-only inspection commands otherwise succeeded. Full gate output and hashes are retained, not only summary stdout.

- [x] Exact formatting, Clippy and workspace-test commands pass on the integrated macOS source.
- [x] Required unit, subprocess and CLI cases actually execute, with inspected assertions and no production bypass or weakened acceptance.
- [x] Acceptance matrix, source fingerprint, unchanged migration, fixture roots, actual command outcomes and Sandbox limits are recorded.
- [x] JSON evidence records the required keys, native platform and actual zero Cargo exit codes.
- [x] No required runtime blocker remains. Additional repository-conformance failures are reported to the Chief for disposition; no blanket repository pass is claimed.
