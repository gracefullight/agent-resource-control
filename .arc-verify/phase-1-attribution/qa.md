Skill: oma-qa. Read-only. No product edit, build gate, npm audit, bandit, or Lighthouse. `av.yaml`: Operation not permitted (unread). `.agents/` untracked files untouched.

## Snapshot

Mission worktree, not a later commit. HEAD `6f63a3a04e34ab0d9e4422fccd002b557a753ee1`, tree `a679952f49c34d9742b21e72da1a4e169c03d107`. SHA-256 of `git diff HEAD` is `ddd9a295368bb1e3888d6d6fa1e303da2ecce47073cc33511020fd08b2ab2fff` (878,763 bytes). Untracked prefixes: `contract.md` `4699bfb8ed8cbb0c` (7,999 bytes), `meter_cadence.rs` `a16609cbce618aa7`, `meter/attribution.rs` `b2a978a0f0585d17`, `meter/footprint.rs` `286d6f77da48e2c5`, `meter/quality.rs` `51e9e1e12c08cbe1`, `0010_attribution.sql` `1dd54d10dd02985d`.

`traceability.md` still shows Phase 1 as 51 Complete, 16 Partial, 2 Pending, 2 Blocked of 71, and Phase 0 as 22, 1, 0, 17 of 40. The ten selected rows are still Partial or Pending. `closing.md` only rewords R-005 item 3 and still says Partial, Remaining: 2, 3. `docs/managed-exec.md` is unchanged. Counts, that guide, NFR-001, NFR-002, coverage, and the architecture harness are not established here.

## Production-path

Named tests: `process_attribution::native_process_attribution_separates_proven_estimated_and_unattributed`, `meter_quality::meter_quality_footprint_sessions_and_rollups_use_production_paths`, `detector_workspace::workspace_fanout_and_cache_follow_production_paths`, and `job_process_meter::mtr_002_unwaited_grandchild_cpu_memory_and_io_are_attributed_to_the_job` (`job_process_meter.rs:101`). No second copy. `script_resolution.rs` is outside the diff.

Lane A. Proven root: verified child, `RUNNING`, same-generation permit (`arc-storage/src/attribution.rs:110-123`). Descendant: `managed_child` (`job_ownership.rs:103-108`). Named test: root and descendant Proven under `claude-code`, agent child Estimated, outside burner Unattributed (`process_attribution.rs:173-191`); Proven drops after identity replace (`217-241`). Names `claude`, `cursor-agent`, `Cursor` (`arc-ipc/src/meter/attribution.rs:51-55`); fixture `claude` (`fixture.rs:22`). Observer is peer-only (`probe.rs:100-123`). `meter_cadence.rs:13-19` runs the job-tree sampler, then attribution. Envelopes wholly inside `[from,to)` (`arc-storage/src/attribution.rs:148-153`). `attribution/tests.rs:351-364` requires `zero_total`. `fence_tests.rs:256-285` injects the 120-day restart. `process_attribution.rs:490` waits in `first_baseline.rs:148-170`.

Old-peer v1 bytes are unverified. `crates/arc-cli/src/meter/usage.rs:28-38` selects v2 for an advertising daemon (`capability_advertisement.rs:24`). `crates/arc-daemon/tests/usage_summary.rs:118-154` and `295-303` therefore exercise the new summary path. `crates/arc-cli/src/meter/usage.rs:143-150` compares parsed JSON from `render`, not old-peer reply bytes. The named attribution read expects the advertised v2 section (`crates/arc-daemon/tests/process_attribution/summary.rs:64-65`). `UsageSummary` denies unknown fields (`crates/arc-ipc/src/meter/usage.rs:78-79`).

Lane B. Direct `arcctl exec` compares Disk Written, end allocated bytes, and delta, with 2 MiB already present (`quality.rs:27-59`, `179-201`). F3/F4: tree CPU versus waited-only, and partial when waited usage exceeds the sample (`342-424`). F2: `job_count` 2 after three nonempty passes (`meter_quality/rollup.rs:110-117`). `assert_all_flagged` checks six metrics on local and remote (`support.rs:203-220`). Sessions: two Claude digests, the same text under Cursor, and a missing session (`sessions.rs:61-98`). Original stdout and stderr are scanned after stop and before restart (`privacy.rs:144-173`, from `rollup.rs:62-64`).

Lane C. Named test: Bun `--filter`, npm `--workspaces` and `-w`, pnpm `-r` and `--filter`, yarn `workspaces foreach`, and an excluding selector (`detector_workspace.rs:199-250`). Unicode `?` is `cases.rs:151-175`. npm trailing whitespace is `directories.rs:332-348`. Cache hit, identical mtime, script edit, manifest replace, and member add/remove are `detector_workspace.rs:112-183`. A non-fan-out `classify` without `--diagnostics` keeps the daemon result (`classify/workspace.rs:34-36`).

Task-7. `accept_running` waits for `RUNNING` for that child and treats `RESERVED` as unfinished (`project_fairness.rs:131-144`). Grant order still requires the previous release (`189-192`). The 101-waiter and `project_fair_turn` checks remain (`299-415`). A prior runner reported both tests passed in 13.68 s. Not re-run here.

## Findings

1. Blocker, task-2. Same-PID identity change can be skipped and the named test still passes. `assert_same_pid_replacement` returns `None` when `reuse` is 0 or the 35 s budget from test start has elapsed (`phases.rs:187-198`). The caller checks the new identity only inside `if let Some` (`process_attribution.rs:284-289`). Fail unless one PID and two start tokens are read on consecutive scans and the first replacement reading is uncounted. The task-2 review log did not show that observation.

2. Blocker, task-6. `after_traffic` is never called. `footprint.rs:18-21` and `#[allow(dead_code)]` at line 205 leave grant and completion readings unwired; `db_worker/traffic.rs` is outside lane B. The footprint fixture is an unclassified `arcctl exec` (`quality.rs:36-40`). A classified BUILD or TEST has no Current Footprint here. Call `after_traffic` after a committed grant and completion, and cover that path, before AT-MTR-02 is Complete.

3. Follow-up, task-2. `assert_aggregates_only` prints and continues when no interval has a zero total (`summary.rs:257-272`) and does not insert one. `attribution/tests.rs:351-364` requires `zero_total` and was not run here. No live zero-total interval is established.

4. Follow-up, task-3. `job diagnostics` may fail and omit the job id while privacy still passes (`privacy.rs:95-98`). Status and the footprint summary must succeed and name the job. The log scan requires `arc-daemon ready.` and runs before restart. Require a successful diagnostics response that names the job, then scan it.

5. Follow-up, task-6. `contract.md` D6 still says whole UTC minutes overlapping the request. The store and `assert_envelopes` include only measured envelopes wholly inside `[from,to)`. Correct D6 to that envelope rule. The summary was not executed here.

6. Follow-up, task-4. If the native name is rejected and the volume cannot be created, `non_utf8_directory` writes `UNVERIFIED` and returns (`directories.rs:202-208`). Task-4's review observed that classify/exec path. Not re-run here, so the `VERIFIED` line (`directories.rs:255`) is not re-established.

7. Recorded, not a confirmed defect. An earlier combined run failed `agent_test_journey` and `hook_event_identity` because `daemon_record` reads every database-prefix name with `fs::read` (`stage.rs:342-351`). The store is `.{db}.attribution` (`location.rs:3-6`, `27-30`), lacking that prefix. A leftover `{db}.attribution` directory would still panic the helper. Not run here.

8. Recorded for task-6; not re-run. Failed together and passed alone: `mixed_failing_test_stage_skips_gh_stage` (`TRACKING_UNCERTAIN`) and `go_offline_success_failure_and_source_barriers` (`BrokenPipe`). `gh_and_buildx_same_execution_bypass_held_permit_and_buildx_cli_is_metered` failed once, then passed alone. Earlier `project_fairness` `RESERVED` failures match the wait this tree now uses.

9. Task-6 still owns phase counts, Complete rows, `docs/managed-exec.md`, and the pre-migration rollup limit (`rollup.rs:26-29`: a job on both sides of migration 0010 can be counted again; no membership is invented). NFR-001, NFR-002, coverage, and the architecture harness were not run. No customer or revenue effect is claimed.

10. Unverified production-path gap, task-6. Old-peer v1 byte compatibility (ISO/IEC 25010) is missing evidence, not a defect. No test exchanges `meter.usage_summary` with a peer that lacks `.meter-usage-summary-v2` and compares those reply bytes. Add that comparison before v1 preservation is treated as shown.
