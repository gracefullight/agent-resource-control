# IPC provisioning final decision

Status: completed task-6. Chief decision: accept the integrated directory-provisioning repair identified below. The exact Cargo gates and filesystem-preservation tests passed in this worktree. This decision does not claim that the managed framework's subsequent verification or final review has already executed.

Used `oma-pm` with the installed `.agents/agents/pm-planner.md` definition for scope, dependencies and acceptance reconciliation, and `oma-qa` for the required final checks. The engineer records `oma-debug`; independent QA records `oma-qa`. This task writes only `docs/ipc-provisioning-final.md`. The saved assignment, original goalBrief, acceptance criteria, Actor ownership, mission graph and verification contract remain unchanged. No new plan or mission state was written.

## Accepted revision and scope

Current HEAD is the accepted baseline `f6971037ed1c3524f88e10732e8910909a4facd3`. The repair is an uncommitted diff, so acceptance attaches to these actual file bytes, not HEAD alone. SHA-256 checks before and after the final gates matched the implementation and independent QA handoffs:

| Changed Rust path | Accepted SHA-256 |
| --- | --- |
| [server.rs](../crates/arc-daemon/src/server.rs) | `693b6b896f4b0229e9eb27b425db87717af68f7bf07c0198b89c22a2b87863e0` |
| [resident.rs](../crates/arc-daemon/tests/resident.rs) | `cac6890f2aef752e47359f6a421f0b8f5a7cbfa8e17abb35f8e0e0a8b17d1da2` |
| [startup.rs](../crates/arc-daemon/tests/startup.rs) | `e1b55d7ec35f39929c7341bd5d0dc81ba21ded8bea808aade93e1365699037e1` |

The three Rust changes cover provisioning, its unit/daemon/CLI regressions and explicit private fixtures. The six report deliverables are listed below. A fresh baseline comparison returned exit 0 with no diff for Cargo.toml, Cargo.lock, rust-toolchain.toml, arc-core, arc-cli, arc-ipc, arc-storage, the daemon manifest, main.rs, instance.rs and platform.rs. Dependencies, migrations, socket resolution, startup ordering and existing interfaces are preserved.

Ownership exception: `.qwen/settings.json` already contained an unrelated addition of `"$version": 4` on entry. A fresh diff confirms that exact addition. It is excluded from the IPC repair acceptance and was neither approved as part of this fix nor reverted. It needs separate owner attribution before any later commit that includes it; it does not block the passing Rust checks. Broad Git status also reported `av.yaml: Operation not permitted`; that file was not inspected or modified. No authority or configuration claim is inferred from it.

## Decisions and evidence

Accept the Technical Director's existing-stack recommendation: retain the five Rust crates, Rust 1.95.0, Tokio Unix sockets, declared libc integration, native locks and SQLite lifecycle. No dependency or general filesystem framework was added. Cost savings are unmeasured.

Accept private creation plus non-mutating validation. The current server uses descriptor-relative no-follow traversal, `mkdirat(0700)`, effective-UID and exact-mode checks, validation after AlreadyExists, and a final device/inode check. It rejects ordinary ARC symlinks, including intermediate and dangling links. Only the established macOS `/tmp` and `/var` root mappings are exempted. The remaining production chmod applies to the socket at 0600. Existing ordinary ancestors may be shared. Preparation still completes before instance-lock acquisition and database initialization.

Accept the Design Director's recovery direction: startup errors identify the path and condition and direct the operator to a separate owned 0700 directory through ARC_SOCKET_PATH. The override still identifies a socket file. Successful CLI output and single-line startup sanitization remain compatible. Reject silent chmod/chown repair and automatic fallback because they violate preservation and resolution. Reject extra prompts and pressure tactics because they add no demonstrated value to this repair.

Accept the Marketing Director's scope memo: operator value here is preserving existing directories while restoring reliable startup. Defer paid pilots, publication, partners, registry delivery and sponsorship. Demand, revenue, ROI and usability improvements remain unmeasured. No commercialization requirement or external delivery was added.

| Requirement | Passing final evidence |
| --- | --- |
| Existing 0755/0770 directories remain unchanged after rejection | Unit preservation test plus daemon/CLI rejection cases for both override and isolated default resolution. Recursive snapshots compare device/inode, full mode, UID/GID, names and file bytes. |
| Symlink targets remain unchanged; redirection rejects before initialization | Unit and daemon/CLI cases cover private/permissive targets, final/intermediate/dangling links and missing-prefix `..` paths. Snapshots retain link text and target contents; no lock/database/WAL/SHM/socket or nested entry appears. |
| Unsafe ownership rejects | The deterministic foreign-UID test calls the same numeric policy as live metadata validation. Current main.rs calls provisioning before lock acquisition and SQLite opening. No actual foreign-owned daemon fixture is claimed. |
| New directories are private; valid 0700 directories work | Unit tests and the 12-case resident matrix cover default/override, new/existing and child umasks 000/022/077. New components are 0700; existing identity, ownership and sentinel bytes survive. |
| Creation races validate without repair | AlreadyExists handler tests and the deterministic between-walks insertion test pass without changing the encountered unsafe entry or creating a nested child. |
| Resolution, sockets and single-instance behavior remain compatible | The resident matrix and existing lifecycle test pass: default/override startup, status/doctor, 0600 sockets, duplicate rejection, continued first-daemon service and successful SIGTERM cleanup. |
| Fixtures explicitly private; migration failure still reachable | Private DirBuilder fixtures are present; the checksum-failure integration test reaches its original exact database error. Child configuration and umask avoid runner-global mutation. |

The [independent QA report](ipc-provisioning-qa.md) supplies the detailed requirement-to-test mapping and additional sanitization/default-path probes. This task inspected the actual implementation, recursive snapshot assertions, startup matrix and initialization order, and reran the required workspace suite. It did not create a duplicate lifecycle suite.

## Final command results

Checks ran serially from the mission workspace on 2026-11-07, starting at 03:14:31 UTC and completed before 03:14:45 UTC. Each shell prepended `/Users/gracefullight/.rustup/toolchains/1.95.0-aarch64-apple-darwin/bin` to PATH and set `CARGO_NET_OFFLINE=true`. Fresh version output was `cargo 1.95.0 (f2d3ce0bd 2026-03-21)` and `rustc 1.95.0 (59807616e 2026-04-14)`. The installed cache was reused; no installation or download was needed.

The assignment's 86,400-second duration, maximum three concurrent Actors, 200-run limit, three retries and 120-second gate limits were retained. This task dispatched no Actors and ran one Cargo process at a time. Before checks the host reported eight logical CPUs and about 116 GB free disk. No required gate needed a retry.

| Exact required command | Actual exit | Result |
| --- | ---: | --- |
| `cargo fmt --all -- --check` | 0 | Empty output, paired with the tool-returned exit; no formatting applied. |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | Finished dev profile in 0.25 s; no warnings or errors. |
| `cargo test --workspace` | 0 | 41 passed, zero failed or ignored; all doc-test harnesses passed. |

Test totals: two CLI integration, fifteen daemon unit, six resident integration, one startup integration, thirteen IPC unit and four storage integration tests. Resident tests completed in 1.42 s. The fresh preservation, private-startup and lifecycle test results establish acceptance on the hashed implementation. Earlier engineer logs and prior mission outcomes are historical evidence only; neither substitutes for these checks.

Other current checks: `git rev-parse HEAD`, scoped Git status/diffs, direct known-path reads and Python SHA-256 checks all returned exit 0. The protected baseline comparison above returned exit 0 with empty output. Serena tool discovery exposed no search/navigation tools; inspection used known paths and Git rather than substitute recursive code search. There was no shell prerequisite failure in this task. No browser, GUI, standalone build, service deployment/restart, publication or outreach was performed; the Cargo suite owns its test daemons.

## Deliverables and preserved reviews

All six task owners' durable outputs are present. Existing reports retain their bytes and historical status statements; this decision supplies the later acceptance without rewriting them.

| Task owner | Delivered artifact | Review evidence supplied by the framework |
| --- | --- | --- |
| IPC Repair Engineer | [Implementation](ipc-provisioning-implementation.md), plus the three Rust paths above | task-1 independent qwen review `9de3a0e0-d2b4-4012-bad6-09978ecf414b` passed. |
| Technical Director | [Technical assessment](ipc-provisioning-technical-review.md) | task-2 independent reviewer run `0be6a80d-a39b-43b7-adb5-1198c3ae770a` passed. Its baseline FAIL diagnoses the original code. |
| Design Director | [Design assessment](ipc-provisioning-design-review.md) | task-3 qwen review `53d96d87-84cb-4cab-88a2-c13f10fba7db` passed. Its baseline FAIL is not a verdict on this repair. |
| Marketing Director | [Value/scope memo](ipc-provisioning-value-scope.md) | task-4 Technical Director review `1f8388ec-1174-4431-928f-9080b307670f` passed. |
| Independent QA Reviewer | [QA](ipc-provisioning-qa.md) | task-5 work `218a4a38-4524-483d-9af4-a35a208c34ad` and qwen review `8d80d7c4-a586-4ac1-a660-53d64dc88b7f` passed. |
| Chief Director | This final decision | Task-6 review and managed final verification follow this deliverable. No result for them is invented here. |

These review IDs/outcomes come from the supplied managed execution record. This task did not rerun or rewrite framework reviews. Earlier review timeouts, invalid JSON and the rejected read-only-stage mutation remain recorded failures; later passes do not erase them. Current report availability, source hashes and fresh Cargo exits were checked directly.

## Limits and handoff

The accepted evidence covers macOS aarch64. Linux was not executed. Foreign ownership is tested at the live policy boundary rather than by privileged chown. Deterministic race tests cover the stated interleavings; arbitrary concurrent rename/chmod/replacement after preparation returns remains unverified because subsequent lifecycle operations reopen pathnames. Acceptance makes no race-free guarantee beyond the tested provisioning behavior.

Independent QA records a denied optional direct-/tmp fixture attempt and successful supported /var-based checks. No direct-/tmp pass is claimed. Masks removing owner permissions and a fresh replay of the original baseline RED failure were not tested. These limits do not replace any required command or preservation check.

Next action: the supervising framework runs the unchanged verification contract against this integrated worktree, including this sixth report, then performs its final review. If a required check fails or source bytes change, acceptance must be reevaluated; product correction belongs to the engineer's existing ownership with independent re-review. No unresolved required product defect was found in this task.

Acceptance checklist:

- [x] Accepted baseline plus exact repaired-source identity, decisions, outcomes and evidence links recorded.
- [x] Original goal, exact Cargo commands, independent review and filesystem-preservation requirements retained.
- [x] All required Cargo gates and preservation tests passed on the integrated Rust revision; remaining limits and later framework stages distinguished from completed checks.
- [x] Only the owned final report written; other Actors' work and saved mission instructions preserved.
