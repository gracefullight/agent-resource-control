# Phase 1 entry: shared contract

Status: as delivered (waves 1 to 3). Base `393f2a3`, tree `f425c30`. Lists, measured values and row outcomes: `closing.md`, "Phase 1 entry: Traffic journey lists" and "Phase 1 entry: assessment".

## Decisions

- **D1 project key.** 64 lowercase hex: a domain-separated SHA-256 of the nearest ancestor of the working directory holding a `.git` entry (file or directory: each worktree is its own project), else the working directory; derived from the filesystem, no tool run. No path crosses the wire, enters SQLite or appears in diagnostics. The digest is opaque, not secret (unsalted). `arc-cli/src/exec/project_key.rs::negotiated` serves both governor-route enqueue sites; the legacy route sends no key. Open questions: should worktrees of one repository share a turn, and should a directory outside any repository count as its own project?
- **D2 storage.** Migration `0007_traffic_project`: nullable `traffic_entries.project_key` (CHECK 64 hex), partial index `idx_traffic_project_grant(project_key, granted_at_ms)`. Schema is version 7.
- **D3 wire.** `JobRequest::Enqueue.project: Option<ProjectKey>`, omitted when `None` (protocol-1 bytes unchanged). Capability `traffic.fairness.v1`. As delivered, no probe precedes the enqueue: it would change the request order three existing `arc-cli` governor tests script. The daemon advertises by holding a POSIX write lock on `.traffic-fairness-v1` beside its socket for its lifetime; `arcctl exec` reads the holder with `F_GETLK` and sends the key only when it is the authenticated peer. `traffic.fairness.capabilities` is still served. A relayed socket in another directory gets no key.
- **D4 reason.** `WaitReason` and `DecisionReason` are unchanged (desktop-exported). `arc_policy::FairnessReason::ProjectFairTurn` (`project_fair_turn`) is recorded on the grant of the job that got a fair turn; existing readers get its projection (`admit`, `admitted`). A waiter passed over for a later entry records `class_window`, which diagnostics and the desktop read; its wire `Queued` reply stays `FIFO` ("not the queue head"), which only sets the wrapper's poll cadence. Raw read: `traffic.fairness.decision`. `arcctl job diagnostics` prints `Last recorded decision: <reason>`; `--json` gains a top-level `last_recorded_reason` only for a fair turn. Desktop files untouched.
- **D5 head.** `arc-storage/src/jobs/traffic/fair_head.rs` is the one head query. The earliest queued entry decides the class; the head is the earliest entry of the project granted a permit of that class least recently, ties by sequence. A keyless entry passes nothing and is passed by nothing. No weights, virtual time or new queue.
- **D6 hook.** `arc-cli/src/hooks/test_literals.rs` holds exactly `cargo test` and `bun run test`; `vitest` is a TEST script leaf in `arc-detector`. AT-TRF-05 nested literal: `cargo test`.
- **D7 helper.** `tests/support/percentile.rs::nearest_rank`.
- **D8 AT-TRF-06 record.** The three-line `TRACKING_UNAVAILABLE` notice on the executed command's stderr: stdout byte-identical to the stub, stderr = that one notice then the stub's bytes, the stub's exit status.
- **D9 R-004 item 1.** Existing `two_real_builds_queue_fifo_and_bypass_remote_and_unknown_while_full` (`traffic_cli`, macOS-only): with no OS cap, admission holds the second real build at zero launches until the first releases. It inherits the runner's environment; FIXTURES binds tests this mission adds.
- **D10.** `native_probe_then_full_100000_attempt_workload` starts an isolated daemon and CLI; `native.rs` reads `uname -srm`, no git or host name.
- **D11 NFR-003 reference.** The asserted difference is against a no-op started through the same `/bin/sh -c` as the installed hook. The bare-spawn difference and the absolute hook P95 are printed and recorded in `closing.md`, not asserted.

## 101-waiter probe (failed in wave 1, repaired by lane C)

Wave 1, default limits (64 connections, 16 requests, DB queue 8), Build window 1, one held permit: in three runs 16, 43 and 19 children started while the permit was held, with `db_rejected` 20,187 / 32,511 / 14,210. Repair: a waiter behind the head backs off from 40 ms to 2.56 s, such waiters poll one at a time through an advisory lock on `.traffic-wait-turn` beside the socket, and expiry runs once per admission request. `at_trf_08_...` now holds 101 waiters and asserts zero connection, request and DB rejections. Not covered: a memory hold keeps the 40 ms cadence at depth.

## Tests

Seam: A `seam_real_daemon_classification_round_trips_feed_the_percentile_helper`; B `seam_installed_hook_rewrites_cargo_build_to_the_managed_route`; C `seam_two_queued_builds_keep_fifo_order_through_built_arcctl`; arc-ipc `traffic::fairness::tests`; arc-storage `tests::project_key`.

Lane, fixed names. A, `phase0_targets.rs`: `nfr_001_idle_daemon_cpu_below_half_percent_of_one_core`, `nfr_003_installed_hook_bypass_p95_overhead_within_50_ms`, `nfr_005_cached_ordinary_classification_p95_within_50_ms`; evidence: `bounded.rs::native_probe_then_full_100000_attempt_workload`. B, `agent_test_journey.rs`: `at_trf_05_nested_hook_test_shares_the_root_test_permit`, `at_trf_06_installed_hook_cargo_test_fails_open_exactly_once`. C, `project_fairness.rs`: `at_trf_08_project_b_is_granted_after_at_most_one_project_a_job`.

## File ownership

One writer per file per wave; paths under `crates/`. Wave 3: integration (any file but `qa.md`) and QA (`qa.md` only, once).

- **Lane A:** `arc-daemon/tests/phase0_targets.rs`, `tests/phase0_targets/**`, `tests/support/{measurements,native,percentile}.rs`.
- **Lane B:** `arc-cli/src/hooks.rs`, `src/hooks/**`, `src/exec/{entry,lineage}.rs`, `arc-cli/tests/{binary,fixtures/agent_hooks}/**`, `tests/agent_hooks_test_literals.rs`; `arc-detector/src/scripts/**`, `arc-detector/tests/**`; `arc-daemon/tests/agent_test_journey.rs`, `tests/agent_test_journey/**`; `README.md`, `docs/managed-exec.md`.
- **Lane C:** `arc-traffic/**`; `arc-storage/src/jobs/traffic/**`, `src/policy/fairness.rs`; `arc-ipc/src/traffic/fairness.rs`; `arc-daemon/src/**`; `arc-cli/src/exec/{project_key,traffic,governor}.rs`, `src/exec/{traffic,governor}/**`, `src/job/diagnostics.rs`, `src/job/diagnostics/**`; `arc-daemon/tests/project_fairness.rs`, `tests/project_fairness/**`.
- **Shared, read-only for lanes:** everything else.
- **Wave 3 integration edits:** `arc-daemon/tests/support/native_fixture.rs` and new `native_root.rs` (a leftover `b<pid>-<n>` directory under a reused pid no longer fails fixture creation), one regression test in `tests/traffic_python_tests.rs`, `docs/managed-exec.md` (queue order), `closing.md`, `traceability.md`, this file.

## Rules

FIXTURES: a built binary gets a cleared environment, temporary HOME, explicit paths and `LLVM_PROFILE_FILE` (`support/journey.rs::Sandbox`); exact environment comparisons use `support/instrumentation.rs`; no correctness from elapsed time (lane A's SRS targets only, at five times the worst of five loaded runs); 60 s child watchdogs; nothing ignored; a passing test removes its fixture directory; files at most 500 lines; no new binary over 60 s; no new Bun file. Actors run `cargo test -p` only.

REVIEW BOUNDARY: a review rejects only an unresolved earlier finding, a defect in lines the task changed since its last review, or a failed runner step caused by a file the task owns. A test failing in a full run but passing alone rejects only on two consecutive failures.

Not claimed: a live vendor session; a queued wait against a vendor tool timeout; any EXTERNAL item; any customer or revenue effect.

## Runner commands

Each once: `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace --no-fail-fast`; and the fixed 22-file `bun test` command of the verification contract (ten `apps/desktop/tests` files, seven `scripts/coverage`, three `scripts/harness`, two `scripts/requirements`), unchanged.
