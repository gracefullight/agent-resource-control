# Runtime identity final decision

## Decision

Accept the macOS runtime-identity repair in this mission worktree. The fixed verification contract is satisfied by the delivered source and current, independently captured macOS QA evidence. No required runtime finding remains. This decision covers the integrated working files; the repair remains uncommitted. Repository architecture conformance remains failed in the additional harness check described below.

The accepted baseline and current HEAD are `073802555b639aa2fec183d93f748d9437cbbcad`, with baseline tree `ce009646036415990a815170955421e1ab8d5c2f`. The delivered source fingerprint is `84110f4f1a4da327a8fabec09bb0cf642b1e4ea4ececbf173d2d8c40f00088f9`. These identify different things: the baseline is the input commit; the fingerprint is SHA-256 of a sorted compact JSON map of 23 source-file hashes and modes, including the two new Rust fixtures. It is not a repaired commit or Git tree hash.

At 2026-11-07 10:35:34 UTC, this Chief independently recomputed the integrated source map, matched it against both implementation and QA manifests, verified all 17 recorded QA artifact hashes, and reconciled the 53 passing test names and 12 required case groups with the actual test log. No later product change invalidates the QA evidence. The implementation, technical, usage, value and QA reports separately identify their input baseline and inspection scope. Actor snapshot commits in those reports are provenance for their individual worktrees, not substitutes for the delivered fingerprint.

### Strategy and Director decisions

The Technical Director's reuse recommendation is accepted. The repair retains the five Rust crates, Rust 1.95.0, Tokio Unix IPC, bundled rusqlite, native file locks, process-start identity, acquisition serialization and inode-aware recovery. Arc-core adds only an edge to the existing workspace libc dependency; there are no package upgrades, new services or migrations. macOS runtime resolution uses `confstr(_CS_DARWIN_USER_TEMP_DIR)` without a TMPDIR fallback. A fixed `<native-root>/arc-instance-<effective-uid>/daemon.lock` provides admission independently of socket and database overrides.

Socket-specific locks cannot meet the differing-override requirement. A discovery service or registry would add an interface and another lifecycle without helping this acceptance contract. Silent fallback or permission repair would defeat identity consistency or preservation. The chosen implementation performs read-only socket preflight before admission writes, acquires the stable anchor before socket-directory creation and SQLite initialization, and holds admission through shutdown cleanup.

The Design Director's two-command CLI journey is retained: start the daemon, then use `arcctl status`; `doctor` remains optional. Explicit overrides still require matching client configuration. Duplicate diagnostics explain the per-user restriction, and readiness follows binding and permission setup. Default clients do not discover arbitrary overridden endpoints. The [usage report](runtime-identity-usage.md) distinguishes unavailable, duplicate, socket-directory and fixed-anchor failures and gives recovery guidance that preserves existing contents. No participant study or human usability improvement was measured.

The Marketing Director's commercial proposals remain outside this repair. The [value assessment](runtime-identity-value-scope.md) ties expected operator benefits to required behavior. Fewer failed launches or support requests are hypotheses; revenue, prices, acquisition and delivery costs, labor cost and ROI are unknown. No commercial experiment, outreach, publication or spending was needed for this decision.

## Verified results

The [independent QA report](runtime-identity-qa.md), [machine-readable evidence](runtime-identity-evidence.json) and [exact command records](../.arc-verify/runtime-identity/qa/commands.json) cover the same delivered fingerprint. QA executed the following unchanged commands serially on 2026-11-07, starting at 10:21:09 UTC:

| Command | Actual exit | Duration | Evidence |
| --- | --- | --- | --- |
| `cargo fmt --all -- --check` | 0 | 0.174 s | [Formatting log](../.arc-verify/runtime-identity/qa/fmt.txt) |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 17.186 s | [Clippy log](../.arc-verify/runtime-identity/qa/clippy.txt) |
| `cargo test --workspace` | 0 | 22.380 s | [Workspace test log](../.arc-verify/runtime-identity/qa/test.txt) |

The test run passed 53 tests, with zero failures, ignored tests or filtered tests: 2 CLI, 2 core unit, 3 core subprocess/probe, 17 daemon unit, 11 resident/probe, 1 startup, 13 IPC and 4 storage. The total includes probe entrypoints; it does not represent 53 independent feature scenarios. Required macOS-only tests appear as executed in the log. QA used the installed toolchain and dependency cache with offline Cargo and a separate target directory. This report-only pass did not repeat the suite on unchanged bytes.

| Required behavior | Executed evidence and result |
| --- | --- |
| Native runtime identity | Confstr unit tests pass for sizing, growing buffers, native bytes and malformed/error responses. Subprocess tests preserve the default endpoint under unset, changed and unwritable-path TMPDIR values. |
| Default endpoint and CLI reachability | The resident matrix first proves a 0500 TMPDIR rejects a write, then runs three daemon environments against three client environments for both status and doctor: 18 successful calls to the same native default endpoint. Changed and unwritable roots gain no `arc` directory. |
| Exactly one daemon across overrides | Gated children with different socket directories, database paths and TMPDIR values admit exactly one winner. The loser exits nonzero without readiness; its snapshot is unchanged, and its database parent, DB/WAL/SHM and socket directory remain absent. The winner answers status and doctor. |
| Lifecycle recovery | SIGTERM removes the socket and permits restart. Forced death of an owned, reaped child leaves a stale socket; the next start reacquires admission, answers both commands and shuts down cleanly. Unit cases cover dead PID, reused PID/start token and successor-inode preservation. |
| Permissions and preservation | Executed fixtures assert owned 0700 directories and 0600 sockets/locks. Unsafe directories, final/intermediate/dangling symlinks, nondirectory parents, unsafe anchor entries and overlong socket paths reject without changing protected snapshots. Overlong cases also compare anchor contents and absent database initialization. |
| Fixture coordination | Both daemon integration binaries hold a separate cross-process fixture lock throughout their owned daemon lifecycles. A child-probe test demonstrates exclusion and subsequent acquisition. Production admission has no test bypass; contenders remain concurrent inside fixture coordination. |
| Configuration and existing contracts | Nonempty socket overrides, empty-override defaults and persistent macOS database policy are preserved. Exact CLI output, unavailable errors, protocol recovery, no TCP listener, storage reopen and the exact migration-checksum failure remain tested. |

The independent QA host was `Darwin 27.2.0 arm64`. Native confstr and `getconf DARWIN_USER_TEMP_DIR` agreed on `/var/folders/r9/0qvpcx794n385zwgzwfvt55h0000gn/T/`; an owned QA write probe succeeded there. Metadata records UID 501 and 0700 for the runtime/default/anchor directories, and 0600 for the anchor and fixture locks. Socket permissions were checked while fixtures were live. Tests used disposable database overrides, including the actual-default-endpoint case.

`crates/arc-storage/migrations/0001_phase0.sql` remains byte-for-byte equal to the accepted baseline. This Chief repeated the baseline comparison and obtained SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`, matching the executed storage assertion and QA evidence.

### Review reconciliation

The [implementation report](runtime-identity-implementation.md) records the fix and earlier attempts. The [technical review](runtime-identity-technical-review.md) found no required product correction after inspecting admission ordering, resolver failures, lock recovery and directory protections. The independent QA then executed the fixed gates in the integrated worktree. Supplied cross-vendor reviews of tasks 1–5 each report passed with empty findings. Those review verdicts were supplied with this assignment; the command logs and their current hashes provide executable acceptance evidence. The cross-vendor review environment's reported `grantpt` failure is not represented as a successful command execution or attributed to QA's working shell.

Earlier failures remain disclosed. The initial RED regression demonstrated TMPDIR-dependent resolution, but its wrapper did not preserve Cargo's individual exit code. Engineer fixture attempts encountered setuid stripping and denied non-UTF-8 filename creation; final tests retain explicit attribute/native-byte policy checks and supported filesystem preservation cases. The usage Actor's package-only resident attempt exited 101 with a missing sibling client binary and a coordination timeout during overlapping execution. Its later full workspace run and the independent serial QA run passed without assertion or timeout changes. The package-only attempt is not promoted to a pass or a general package-only test guarantee.

### Additional harness disposition

`./scripts/harness/validate.sh` exited **1** in independent QA; its [log](../.arc-verify/runtime-identity/qa/harness.txt) and [command record](../.arc-verify/runtime-identity/qa/harness.json) remain unchanged. Secrets, dangerous-command and architecture-layer checks passed. The failures are:

| Path or check | Baseline | Delivered | Disposition |
| --- | --- | --- | --- |
| `crates/arc-daemon/src/server.rs` | 690 lines | 762 lines | Existing 500-line violation grew; a preservation-focused module split remains a repository follow-up. |
| `crates/arc-daemon/tests/resident.rs` | 655 lines | 901 lines | Existing violation grew with regression coverage; split scenarios while retaining shared fixtures and all assertions in a follow-up. |
| `crates/arc-ipc/src/lib.rs` | 549 lines | 549 lines | Unchanged pre-existing violation outside this repair's product ownership. |
| Harness coverage check | No root package.json or Vitest configuration | `bun run test:coverage` unavailable | Validator and Rust verification owners must establish an applicable coverage check; no coverage percentage is available. |

Chief judgment: retain these as repository-conformance follow-ups rather than expanding this fixed runtime assignment into a structural refactor and coverage-tool migration. The fixed verification contract names the three Cargo gates and report/evidence checks; it does not name this additional validator as a runtime acceptance gate. This records its failure and does not waive its rules or claim repository conformance. No skip flag, dummy coverage script, reduced assertion or altered verification command was introduced. The technical and QA reviews leave this disposition to the Chief.

## Remaining limits

No required macOS runtime case or fixed-contract deliverable remains incomplete. No container or metric completion target is supplied in the immutable assignment; the inspected example's container configuration is commented. This pass could not read ignored local `av.yaml`: the supported file read returned `PermissionError: Operation not permitted`. No deployed-target health claim is made, and no unspecified external target is inferred from the example. No sandbox restriction was bypassed.

Actual foreign-UID file creation, real setuid preservation and non-UTF-8 filesystem creation were not established in QA. Attribute-policy and native-byte tests passed under the disclosed sandbox limits. Non-macOS execution and adversarial same-UID namespace replacement remain unverified. Existing inode checks do not make pathname operations atomic against an owner or administrator replacing its own namespace. A real unrelated daemon can cause fixture admission failure; tests do not displace one.

The usage review also identifies a diagnostic limitation: the directory-error helper can suggest changing `ARC_SOCKET_PATH` when used for a fixed-anchor directory error. That suggestion cannot relocate the anchor. The usage report explains the distinction. No required identity or preservation assertion depends on that advice; refining the diagnostic belongs to the product owner in a follow-up.

The repair is delivered as uncommitted work, including two new Rust files. Preserve them when collecting the worktree. Future product, dependency, test or toolchain changes require fresh affected checks and an updated source fingerprint. Repository module/coverage follow-ups should retain the frozen migration and exact Cargo commands. No deployment, external-service restart or distribution occurred.

### Lessons and task delivery

Runtime identity must use the native per-user authority at every relevant boundary, including directory classification. Admission must precede persistent side effects. Tests should coordinate complete daemon lifecycles through a separate lock while exercising real production uniqueness. Source manifests and preserved logs allow final review to reuse current evidence without redundant full-suite runs.

Skills used: `oma-coordination` for dependency/review reconciliation and `oma-pm` for checking the saved scope and acceptance coverage. The frozen plan was reused; no new plan, Actor assignment, mission state or OMA run was created. Serena tools were unavailable; source inspection used direct reads of assignment-located paths and scoped Git evidence.

This task changes only `docs/runtime-identity-final.md`. Product and test interfaces are unchanged by this pass. Chief checks used scoped `git status`, `git ls-files`, `git rev-parse`, baseline `git show`, `git diff --check`, direct file reads and non-mutating Python SHA-256/contract checks. Source/artifact reconciliation exited 0; the optional local-config read was caught and reported as denied. Mise's denied tracking-symlink warning did not prevent those checks. The final report's required sections, contract file/JSON fields, local links and preservation of all recorded inputs are checked before handoff.

The first report-check wrapper exited 1 because it incorrectly required exit 0 from `git diff --no-index --check /dev/null` against this new file. Git returned 1 with no whitespace diagnostics. The corrected wrapper accounts for added-file differences and independently checks line endings and trailing whitespace; no product or acceptance test changed.
