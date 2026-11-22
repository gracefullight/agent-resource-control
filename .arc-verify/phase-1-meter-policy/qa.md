# Phase 1 meter and policy: QA

Snapshot `c925712a` (tree `cc901c2f`), base `9e29303`: task-1, task-2 (through attempt 4), task-3, task-4 and task-5. Replaces the attempt-1 review of `0de315b`; since then only TA and `tree.c`, `hook_event_identity.rs`, `resource_samples/tests.rs`, managed-exec.md, closing.md, traceability.md and contract.md changed, so product-code verdicts carry over. Read-only; no cargo test run here (standing user instruction), so the runner's integration pass is the only execution evidence.

## Verdict: PASS (0 blockers; follow-ups below)

Tests: TA = `job_process_meter::mtr_002_unwaited_grandchild_cpu_memory_and_io_are_attributed_to_the_job`; TB = `usage_summary::mtr_004_vendor_project_and_period_summaries_match_recorded_jobs`; TQ = `usage_summary::met_017_snapshot_reports_traffic_queue_depth_while_build_permit_is_held`; and in `traffic_policy::`, `dec_013_…`, `trf_015_…`, `pol_004_…`, `mtr_014_…`. Each drives built `arcctl exec`, the installed hook or a built `arcctl` read against a real daemon or its schedule.

| Row | Item | Verdict |
|---|---|---|
| MTR-002 | 2 | Covered by TA: grandchild in the root's group, rows `C`, root `A`; outside and recycled-identity controls never sampled (`:275-283`) |
| MTR-003 | 2 | Covered by TA: tree CPU > waited, = waited + unwaited (`:201-208`). Item 3 EXTERNAL: Blocked |
| MET-002 | 2 | Covered by TA: delta/interval×100, in (0, 100·cores], first null (`:242-252`) |
| MET-003 | 2 | Covered by TA, as for MTR-003 |
| MET-004 | 2 | Covered by TA: grandchild RSS >= the 48 MiB held (`:159`, `:217-218`). Resolves B2 |
| MET-005 | 1 | Covered by TA: peak = max per-sample sum, >= grandchild RSS and held (`:211-219`) |
| DET-012 | 2 | Covered by TA: a non-ARC grandchild is linked to the job |
| MET-009 | 1 | Covered by TA: `tree.c:108-116` reads 4 MiB back uncached; report and grandchild reads >= it (`:237-238`). Resolves B3 |
| MET-010 | 1 | Covered by TA: report = sum of the last readings, >= the 4 MiB written |
| MTR-009 | 1, 2 | Covered by TA: the read and write floors above |
| MTR-004 | 1, 2 | Covered by TB: buckets equal an independent SQLite GROUP BY |
| MTR-008 | 2 | Covered by TB: REMOTE_CI in `remote`, CPU null `not_metered_locally`, not in local totals |
| MTR-018 | 3 | Covered by TB: JSON by vendor, project and period |
| MET-013 | 2 | Covered by TB, but only for jobs whose report was recorded; see F1 |
| MET-015 | 2 | Covered by TB: count by class and outcome |
| MET-017 | 2 | Covered by TQ: depth 2 = SQLite QUEUED while a Build permit is held, then 0 |
| MET-026 | 1 | Covered by TB: stored = hook's option; NULL for direct and gh |
| DEC-013 | 4 | Covered by dec_013: QUEUED with start-rate key, later granted, no lost or duplicated permit |
| POL-001 | 2 | Covered by dec_013: window 1, the per-class estimates, and 120/min with burst 120 |
| TRF-015 | 3 | Covered by trf_015 (every decision in order, queue position) and by dec_013 (token schedule) |
| POL-004 | 3 | Covered by pol_004: set_switch, pause, resume, startup config diff; cell states no learned value exists |
| MTR-014 | 1, 2 | Covered by mtr_014: minute/hour/day sums, raw > 72 h deleted, Meter record expired by the daemon |
| NFR-023 | 1 | Covered by mtr_014: bounded across three seeded batches (not a soak) |

Counts: in traceability.md 22 selected rows are Complete and MTR-003 Blocked; Phase 1 reads 51/16/2/2 of 71 (line 59), Phase 0 22/1/0/17 of 40 (line 58), consistent with 29/32/9/1 before.

## Earlier blockers: all resolved in this snapshot

- **B1 (task-4/task-5):** `hook_event_identity.rs` now compares the event keys, then the commands with only the overhead digits removed. `hook.rs:136-166` accepts the optional vendor and overhead suffix but validates it strictly (vendor in {claude-code, cursor}, overhead parses as u64). Nothing depends on elapsed time any more.
- **B2 (task-2):** the RSS floor is asserted at `job_process_meter.rs:218-219`. `tree.c:125-130` touches every held page on each pass.
- **B3 (task-2):** `tree.c` now calls read(2) and publishes `read`, and the read floors are asserted at `:237-238`.

## Checks that pass

- **v1 bytes:** `meter.snapshot` is intercepted only for `include_traffic:true`; tree, usage and history are new operations read only by arcctl after the advertisement lock. No diff in `apps/desktop/src`, `apps/desktop/tests`, `scripts`, `Cargo.lock` or manifests. Desktop Rust repairs remove only the added key and keep the strict v1 decode.
- **Privacy:** no raw session or tool-call id is stored. `record::assert_no_raw_ids` scans the fixture, and `hook_event_identity` asserts that neither id appears in the command.
- **Remote and `none`:** both covered. CPU and duration are summed only over `canonical_root` reports, both in storage and in the recompute.
- **Start rate:** tokens are derived from `permits.reserved_at` inside the admission transaction, so a hold writes no permit.
- **Audit:** before and after rows exist for both switches and for the startup configuration. No learned value is claimed.
- **Rollup:** each raw row is counted once. Rollup and delete run in one IMMEDIATE transaction with a deleted == rolled check (`rollup.rs:149`). Retention is 72 h / 30 d / 365 d. `written_samples_roll_up_to_their_growth_then_are_deleted` covers lane A rows through lane C's rollup.
- **Rule F:** all three targets use a cleared environment and a temporary HOME through the fixtures. `cc` runs with `env_clear` plus `instrumentation::forward`. Watchdogs are 60 s, waits are on daemon state, and none of the three targets has an `#[ignore]`.
- **Ownership:** every file has one writer per wave; the deviations are listed in contract.md "As delivered".
- **500 lines:** the largest changed source file has 499 lines (`traffic_bun_scripts/mod.rs`).
- **Follow-up 1:** `journey::eventually` waits for the start marker at `hook_event_identity.rs:270-273`.
- **Follow-up 2:** the stuck turn lock came from a flock copy that a sibling test's fork held until it exec'd. The product opens the descriptor with `FD_CLOEXEC` (`the_turn_descriptor_is_closed_on_exec`), and `Cadence` is dropped before the spawn (`exec/traffic.rs:370-386`). Product code is unchanged.

## Production-path gaps (not blocking)

- **F1:** TB tolerates a lost best-effort wrapper report (`tests/usage_summary.rs:276`; `REPORT_WINDOW` 500 ms, `exec/accounting.rs:13`); cause unknown, so MET-013 rests on at least one report.
- **F4** (`resource_samples.rs:346-352`): if only the root is ever sampled, the report says `complete` with unwaited 0. A descendant that lives entirely between two samples is therefore reported as 0 without a label. Prefer `partial` or a lower-bound label when the waited rusage exceeds the sampled root.
- **F12:** TA does not exercise a ppid-only descendant that left the process group.

## Follow-ups

- **F13 (task-2):** TA's RSS and read floors are exact; calibration ran beside four `yes` processes, not another test binary (rule F). On a failure apply the flake rule; never add a silent tolerance.
- **F14 (closing.md owner):** MET-004, MET-005, MET-009, MTR-009 cells are true but do not name the new floors (48 MiB held, 4 MiB read back).
- **F2** (`rollup.rs:23`): hour and day `job_count` count a job once per rollup pass, and a pass runs every 600 s.
- **F3:** rollup CPU and write totals are lower bounds. The usage-summary CPU is waited-only (`usage_summary.rs:104`).
- **F5** (`hooks/usage_fields.rs:70`): `rfind` could insert the options inside a literal that itself contains ` hook-exec --arc-event <hex> -- `.
- **F6:** the start-rate switch keeps no mutation receipt, and pause and resume accept any key.
- **F7** (`decision_history.rs:93`): queue position uses the global sequence, not fair-head order.
- **F8** (`cadence/tests.rs:117,241`): the real-slice tolerance has no five-run deviation evidence.
- **F11** (`retention.rs:60`): a rollup pass waits for `DbOutstanding == 0`, so sustained writer load postpones it.
