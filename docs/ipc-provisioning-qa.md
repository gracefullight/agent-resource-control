# Independent IPC provisioning QA

## Review Result: PASS

Status: completed. Actor: Independent QA Reviewer, task-5, attempt 1. Verification completeness: completed for the assigned source review and three required gates. No CRITICAL, HIGH, MEDIUM or LOW defect was established in the reviewed change. The limits below qualify the evidence; this report does not grant race-free operation under arbitrary concurrent filesystem replacement.

Used `oma-qa`, its execution protocol and checklist, and the installed `.agents/agents/qa-reviewer.md` definition. Read the implementation report and the technical/design assessments, then inspected the actual diff and tests. Their baseline FAIL verdicts describe the original defect, not the repaired files. No subagents, AV operations, browser checks, installations or external delivery were used.

Only `docs/ipc-provisioning-qa.md` is owned and written by this task. Product code, other Actors' reports, instructions, manifests, lockfile, migrations, toolchain and mission state were preserved. Cargo generated ignored `target/` artifacts; supplemental probes created and removed only their own temporary fixtures and reaped their child daemons.

## Revision and review boundary

Workspace: `/Users/gracefullight/workspace/arc/.agent-valley/operations/0b22258d-d7c5-4fb0-bc7b-73945a671689/workspaces/fa572687-8b27-4ea3-8c17-867d5159fa1d/workspace`.

Current HEAD and accepted baseline are both `f6971037ed1c3524f88e10732e8910909a4facd3`. The repair is an uncommitted integrated diff, so HEAD alone does not identify the tested implementation. Fresh SHA-256 values after the gates identify its three changed Rust files:

| Path | SHA-256 |
| --- | --- |
| `crates/arc-daemon/src/server.rs` | `693b6b896f4b0229e9eb27b425db87717af68f7bf07c0198b89c22a2b87863e0` |
| `crates/arc-daemon/tests/resident.rs` | `cac6890f2aef752e47359f6a421f0b8f5a7cbfa8e17abb35f8e0e0a8b17d1da2` |
| `crates/arc-daemon/tests/startup.rs` | `e1b55d7ec35f39929c7341bd5d0dc81ba21ded8bea808aade93e1365699037e1` |

These match the implementation handoff. Combined scoped baseline diff/status evidence identifies those Rust paths, the four preceding untracked reports, and the pre-existing `.qwen/settings.json` addition of `"$version": 4`; this task adds this QA report. The unrelated settings addition was present on entry and remains untouched for Chief reconciliation. It is not part of the IPC repair or a new ownership handoff.

Serena is configured in `.agents/oma-config.cue`, but tool discovery returned no Serena tools. Used Git evidence and scoped reads of already identified files, following the code-intelligence fallback. No substitute recursive source search was used. Broad Git status exited 0 but printed `av.yaml: Operation not permitted`; scoped status excluding that file succeeded. Its bytes were not inspected and no conclusion about that ignored configuration is made.

## Filesystem preservation

`server.rs:30-67` separates read-only inspection from private creation and validation. `walk_directory` opens components relative to a held descriptor. `server.rs:165-180` uses `O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC`; `server.rs:183-200` uses `mkdirat(..., 0700)` and validates the opened entry after success or `AlreadyExists`. No directory chmod, chown, unlink or recursive cleanup appears in production provisioning. The remaining `set_permissions` at `server.rs:244` applies to the socket and retains its 0600 contract.

The live validator at `server.rs:80-101` reads metadata UID, uses `geteuid()`, and rejects any mismatch or permission/special-bit combination other than 0700. Directory type comes from `O_DIRECTORY`. GID is preserved rather than restricted. Existing ordinary ancestors need not be 0700. Newly created components and entries that appeared at previously missing components are validated before proceeding into them. `server.rs:60-66` repeats a no-follow walk and compares final device/inode plus policy before returning.

The recursive integration snapshot at `resident.rs:387-434` records relative entry names, device, inode, full mode, UID, GID, regular-file bytes and symlink text. Children are sorted, and symlinks are not traversed. The target directories are separate real entries inside each fixture, so they are independently captured. Equality before startup, after rejected daemon exit and after both CLI calls establishes preservation of these fields and the absence of added lock, database, WAL, SHM, socket or descendant entries. It does not claim preservation of access times, which reads can affect.

`assert_preserved_rejection` at `resident.rs:451-511` requires a nonzero daemon exit, empty stdout, a one-line socket-directory error with reason/recovery guidance, unchanged snapshots, both CLI failures and absent database/socket paths. The database is a fixture sibling outside the rejected ARC directory or symlink target, so early database initialization cannot hide in an unobserved location. A supplemental check also used an absent database parent outside the rejected subtree and confirmed it remained absent.

### Requirement-to-evidence mapping

All named Rust tests below ran successfully in this task's fresh `cargo test --workspace`, not just the engineer's historical run.

| Requirement | Actual source and executable evidence |
| --- | --- |
| Existing 0755/0770 directories rejected unchanged | `server.rs:442` unit test `permissive_existing_directories_are_rejected_without_mutation` compares device/inode/mode/UID/GID, sentinel bytes and sole-entry count. `resident.rs:514` integration test `permissive_directories_and_their_contents_survive_daemon_and_cli_rejection` covers both modes with both override and isolated default resolution, using the complete recursive snapshot. |
| Final/intermediate links to private and permissive targets rejected | `server.rs:506` unit test and `resident.rs:533` integration test cover targets at 0700, 0755 and 0770. They check final and intermediate links, target metadata/bytes/entries and link text. Override coverage includes missing descendants; default resolution covers its fixed `arc/arc.sock` final directory. |
| Dangling links and missing-prefix redirection rejected without creation | The same unit/integration tests preserve dangling links and keep targets absent. `new/../link` or `new/../arc` cases require even the new prefix to remain absent. Inspection does not canonicalize away that evidence. |
| Existing non-directory parent rejected before initialization | `resident.rs:572`, `nondirectory_parent_is_rejected_before_lock_or_database_initialization`, snapshots the file and its bytes and uses the same no-side-effect assertions. |
| Unsafe ownership rejected before lock/database use | `server.rs:600`, `foreign_uid_and_nonprivate_modes_are_rejected_by_the_live_policy`, deterministically passes owner 42/effective UID 43 to the exact policy used by live metadata validation. It also covers 0755, 0770, 1700, 2700, 4700 and 0600. Call order connects that policy to pre-initialization rejection. No privileged chown or actual foreign-owned daemon fixture is claimed. |
| Creation loses an AlreadyExists race | `server.rs:611`, `already_exists_is_validated_without_repair_or_following_links`, directly executes the mkdir/open handler against existing 0755, 0770, 0700 and symlink entries, with preservation checks. It deterministically exercises the error path, not a timing-dependent race loop. |
| Entry appears between inspection and creation | `server.rs:562`, `a_directory_that_appears_between_walks_is_validated_before_nested_creation`, inserts a 0755 directory between the two walks; the second rejects it, preserves identity/ownership/content and creates no nested child. This proves this specific interleaving. |
| New nested directories are owned/private; valid 0700 directories remain usable | `server.rs:465` unit test verifies exact full permission bits, effective UID and existing identity/content. `resident.rs:581` integration test covers 12 combinations: default/override, new/existing, child umasks 000/022/077. Both new components and the socket mode are checked; existing device/inode/UID/GID and sentinel bytes survive startup. |
| Default and ARC_SOCKET_PATH resolution preserved | `arc-core/src/lib.rs:11-17` is byte-identical to baseline. The 12-case startup matrix runs daemon and CLI against matching isolated child configuration. Supplemental daemon/CLI checks verified unset and empty overrides select `TMPDIR/arc/arc.sock` with exact status output. |
| Platform temporary-root aliases remain usable | `server.rs:203-214` exempts only exact macOS `/tmp -> private/tmp` and `/var -> private/var` mappings, including equivalent absolute target text. Live `readlink` returned those relative mappings. The suite and supplemental successful startup traverse this host's `/var/folders/...` temporary root. Arbitrary ARC links remain rejected. Direct `/tmp` startup was not verified because its fixture creation was denied; see failed attempts. |
| Shared temporary root refused without tightening | `server.rs:656`, `a_shared_temporary_root_is_refused_instead_of_tightened`, passes. The guard at `server.rs:216-225` also compares canonical identities; it performs no mutation. |
| Rejection precedes instance-lock/database initialization | Unchanged `main.rs:35-40` calls prepare before `InstanceLock::acquire`; `main.rs:20-26` acquires the instance before `open_and_migrate`. Database-path resolution at line 15 only reads configuration. Live recursive rejection snapshots show no lock/storage artifacts. Ownership rejection is supported by the live-policy unit test plus this call order, not by a fabricated foreign-owned integration run. |
| Valid startup, 0600 socket, duplicate rejection and SIGTERM | `resident_contract_holds_until_sigterm` verifies socket type/modes, hello, incompatible-version recovery, oversized frames, exact status/doctor output, second-instance rejection, continued first-daemon response, SIGTERM success, socket removal and clean SQLite reopen with zero GC rows. The 12-case matrix repeats successful CLI use, duplicate rejection and shutdown. Existing six instance tests verify duplicate and stale-holder behavior. |
| Fixtures explicitly private; no runner-global environment/umask race | `server.rs:393`, `resident.rs:46` and `startup.rs:26` use `DirBuilderExt::mode(0700)`. Rejection modes are set explicitly in fixtures. Environment settings/removal use child `Command` configuration; `resident.rs:78-87` changes umask only in child `pre_exec`. None of the changed tests changes the parallel runner's environment or umask. Daemon guards reap children on failure. |
| Original migration-failure purpose preserved | `migration_failure_exits_nonzero_with_one_stderr_sentence` passes with the private fixture: exact checksum-failure stderr, empty stdout, zero GC rows and unchanged invalid checksum. It reaches storage failure rather than failing directory policy first. |
| Plain rejection/recovery text and sanitization | `directory_error` reports path/reason and selects a separate owned 0700 directory through the existing override. Unchanged `main.rs:43-58` sanitizes CR/LF and retains nonzero stderr failure behavior. Supplemental live CR/LF fixture check passed: one terminal newline, no CR, sanitized path, absent separate database parent, unchanged snapshot, and exact `DAEMON_UNAVAILABLE\n`/empty stdout for status and doctor. |

### Original-defect evidence

Direct `git show f697103...:crates/arc-daemon/src/server.rs` confirms the original `create_dir_all(parent)` followed by unconditional directory chmod, with no ownership validation. The new preservation test necessarily rejects that behavior: it captures 040755 before provisioning and compares against the later full mode.

Read the retained engineer bug report at `.agent-valley/parallel/workspaces/efcac0c89f4fc294-7afaa346b4bf92bf-1/.agents/results/bugs/ipc-provisioning.md`. It records the focused regression command exiting 101 before the fix, with mode changing 040755 to 040700 and device/inode/UID/GID unchanged. The integrated implementation report preserves the same RED command and tuples. This is corroborating historical evidence; no raw RED process receipt was available and this task did not revert product code to replay it. Fresh GREEN execution of that same test is present in the 15-test daemon unit run below.

## Pinned toolchain

`rust-toolchain.toml` remains baseline-identical: channel `1.95.0`, minimal profile, rustfmt and clippy. The default shell resolves Cargo through a mise shim; each gate explicitly prepended the already-installed toolchain directory and set offline mode:

```sh
export PATH=/Users/gracefullight/.rustup/toolchains/1.95.0-aarch64-apple-darwin/bin:$PATH
export CARGO_NET_OFFLINE=true
```

Fresh `command -v cargo` and `command -v rustc` resolved to that directory. Fresh versions:

```text
cargo 1.95.0 (f2d3ce0bd 2026-03-21)
rustc 1.95.0 (59807616e 2026-04-14)
host: aarch64-apple-darwin
LLVM version: 22.1.2
```

All gates ran directly from the mission workspace root, serially, without a command wrapper or changed argv. Existing `/Users/gracefullight/.cargo/registry` cache was used. No cache copy, download, toolchain installation, pin change, manifest change or standalone build command was needed. Compilation output came from the explicitly required Clippy/test commands. `target/` was absent before these gates.

Resource observations before the compilation gates: 8 logical CPUs, 17,179,869,184 bytes RAM and approximately 109 GiB free disk. Assignment limits remain 86,400 seconds, maximum 3 concurrent Actors, 200 mission runs, 3 retries, with 120,000 ms per required gate. This task dispatched no other Actor and ran one Cargo process at a time. No monetary/provider usage estimate is asserted. All required commands completed well within their per-command limit. Verification was complete by `2026-11-07T03:07:49Z`.

## Command results

These are current tool-returned exits. Empty formatter output is paired with its actual exit, rather than being treated alone as proof. The historical engineer logs are not substituted for these results.

| Command | Exit | Observed result |
| --- | --- | --- |
| `cargo fmt --all -- --check` | 0 | No output; completed in about 0.37 s. No formatting applied. |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | All workspace targets checked; finished dev profile in 9.07 s, no warnings/errors. |
| `cargo test --workspace` | 0 | 41 tests passed; 0 failed/ignored. Test compilation finished in 6.83 s; all harnesses below passed. |
| `git diff --exit-code f6971037ed1c3524f88e10732e8910909a4facd3 -- Cargo.toml Cargo.lock rust-toolchain.toml crates/arc-core crates/arc-cli crates/arc-ipc crates/arc-storage crates/arc-daemon/Cargo.toml crates/arc-daemon/src/main.rs crates/arc-daemon/src/instance.rs crates/arc-daemon/src/platform.rs` | 0 | Empty diff: all workspace manifests, lockfile, storage/migrations and listed unchanged interfaces preserved. |
| `git diff --check -- crates/arc-daemon/src/server.rs crates/arc-daemon/tests/resident.rs crates/arc-daemon/tests/startup.rs` | 0 | No whitespace diagnostics. |
| `/usr/bin/python3 -c <supplemental daemon/CLI assertions>` | 0 | Unset/empty override startup through `/var`, 0700 directory/0600 socket, exact status, SIGTERM cleanup, CR/LF rejection sanitization, exact unavailable CLI output and recursive non-mutation passed. This supplements the exact Cargo gates. |
| `git rev-parse HEAD`; scoped baseline diffs/status; `/usr/bin/shasum -a 256 <reviewed files>`; scoped `cat`/`sed`/`nl` reads | 0 | Revision, diff, source identity, reports and source evidence acquired. |
| Pinned `cargo --version`, `rustc --version`, and `command -v` | 0 | Both resolve to installed Rust 1.95.0. |

Fresh workspace-test summary:

| Harness | Passed | Failed | Notes |
| --- | ---: | ---: | --- |
| arc-cli binary integration | 2 | 0 | Binary location and no-argument command help. |
| arc-daemon unit | 15 | 0 | Six existing instance tests and nine server tests, including all provisioning cases named above. |
| arc-daemon resident integration | 6 | 0 | All preservation, private-startup matrix, unavailable and resident lifecycle cases passed; 1.43 s. |
| arc-daemon startup integration | 1 | 0 | Original migration-checksum failure remains reachable. |
| arc-ipc unit | 13 | 0 | Framing, request validation and protocol behavior preserved. |
| arc-storage nfr010 integration | 4 | 0 | Migration atomicity, checksum, WAL/reopen and constraints. |
| Other unit/doc harnesses | 0 | 0 | All completed successfully; no ignored tests. |

### Failed attempts and access limits

An initial supplemental Python probe requested a fixture directly under `/tmp`; `tempfile.mkdtemp` failed with `PermissionError: [Errno 1] Operation not permitted: /tmp/arcqa-...`, exit 1, before starting a daemon or creating a fixture. No `/tmp` startup result is claimed. Repeated the supported checks using the host's normal temporary directory under `/var/folders/...`; all assertions passed, exit 0. No GUI, permission changes or tool installation was used to bypass the denial. This optional direct `/tmp` probe does not replace or invalidate the successful required suite, which already exercises the supported platform alias.

The first combined version/environment diagnostic ended with exit 1 because `printenv` found none of the requested Cargo/managed-run environment variables. Its preceding pinned version calls succeeded; the later individual toolchain-path/version checks also succeeded. `ls` reported missing `target/`, `.arc-verify` and `.cargo/config.toml` during inspection; those paths were not required prerequisites. Broad Git's ignored `av.yaml` diagnostic is recorded above. No required gate failed, and no failed attempt was counted as a pass.

## Compatibility and remaining risks

The public preparation signature, socket resolution, instance acquisition, protocol/peer handling, storage schema/migrations and successful CLI output remain compatible. The unchanged baseline comparison and existing runtime assertions support this conclusion. No added dependency, permission/feature reduction or unrelated behavior change was found.

The ownership evidence is a deterministic test of the exact live numeric policy, combined with verified startup ordering; it is not an actual foreign-owned filesystem startup. The two deterministic race cases establish AlreadyExists handling and an entry appearing between walks. They do not establish immunity to arbitrary concurrent rename, chmod or replacement after the final validation. In particular, preparation returns `()` and closes its descriptors; later lock/socket operations still use pathnames (`main.rs:38-40`, `instance.rs:123-126`, `server.rs:241`). This is the preserved lifecycle limit documented in the implementation handoff, not a tested race-free guarantee. No concurrent exploit was reproduced or invented in this review.

This execution covers macOS aarch64. Linux behavior was not run. Standard child masks 000/022/077 passed; masks that remove owner permission bits were not tested and are not silently repaired by the implementation. Special modes reject at policy level. Direct `/tmp` alias startup and a fresh original-baseline RED replay remain unverified as described above. These are bounded evidence limits, not passing results for those additional scenarios.

Framework independent task review, operator verification and Chief final acceptance remain separate stages. No saved task, mission graph or verification contract was changed.

## Acceptance checklist

- [x] Every required provisioning/lifecycle behavior mapped to actual source and current unit/integration/CLI evidence.
- [x] Existing 0755/0770 directories and symlink targets preserved on rejection, including metadata, entries and sentinel bytes; no lock/database side effects observed.
- [x] Ownership/race conclusions limited to the tests and call-order evidence actually available.
- [x] All three exact Cargo gates passed using installed Rust 1.95.0.
- [x] Manifests, lockfile, toolchain, migrations and unrelated product interfaces unchanged from accepted baseline.
- [x] Only the owned QA report written; pre-existing settings change and other Actors' work preserved.

Report validation through `/usr/bin/python3 -c` exited 0: required sections, UTF-8 decoding, terminal newline, no trailing whitespace, and all three Rust SHA-256 values verified. Final scoped Git status showed only this QA report added to the entry status; the three reviewed Rust hashes remained unchanged after all execution. Product `git diff --check` also exited 0.

Chief handoff: no product correction or interface handoff is requested. Retain the unrelated settings observation for reconciliation, and keep final framework verification separate from this task's PASS.
