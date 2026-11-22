# Phase 1 meter and policy: contract

Base `9e29303` (tree `3b79428f`). Paths under `crates/`. Wave 1 stubs kept today's behaviour and were never asserted.

## Decisions

- **D1 desktop bytes.** v1 `meter.snapshot`, `meter.summary`, policy, diagnostics and protocol-1 replies are unchanged. New data travels only in new operations or an optional request field, sent after the daemon's advertisement (D3); only `arcctl` asks.
- **D2 migration `0009_meter_policy`**, expand-only, schema 9: nullable `jobs.vendor` (`claude-code`|`cursor`), `jobs.hook_overhead_ms` (>= 0), `jobs.project_key` (64 hex); `resource_samples.cpu_core_percent` (REAL >= 0); indexes `idx_resource_samples_time(timestamp_ns)`, `idx_resource_samples_job_time(job_id,timestamp_ns)`; `job_meter_summaries.process_tree_json` (json_valid, <= 16384 bytes, cleared with `report_json` on expiry); table `resource_rollups(granularity minute|hour|day, bucket_start_ms, cpu_core_seconds, peak_rss_bytes, disk_write_bytes, job_count, reclaimed_bytes NULL)`, PK `(granularity,bucket_start_ms)`. `timestamp_ns`/`cpu_time_ns` are decimal TEXT (19-digit wall ns); arithmetic uses `CAST(... AS INTEGER)`.
- **D3 capabilities**, each a file beside the socket write-locked for the daemon's lifetime: `meter.job-tree.v1`, `meter.usage-summary.v1` (also gates the registration fields), `meter.snapshot-traffic.v1`, `traffic.decision-history.v1`.
- **D4 operations** (arc-ipc): `meter.job_tree` {job_id}; `meter.usage_summary` {by vendor|project|period, from_ms, to_ms, bucket hour|day}; `meter.snapshot` payload `null` or `{"include_traffic":bool}`, `traffic` {queue_depth, observed_at_ms} only when requested; `traffic.decision_history` {job_id}. Lane `*_dispatch.rs` are called from `policy_dispatch::dispatch`.
- **D5 registration fields.** `Register.usage` and `Enqueue.usage: Option<UsageFields>` {vendor, hook_overhead_ms, project_key}, omitted when `None`. Raw session/tool-call ids are never sent or stored.
- **D6 vendor mapping.** Adapter `claude` maps to vendor `claude-code` only in `Vendor::from_adapter`; the event-key digest input stays `claude`.
- **D7 hook-exec.** Hidden `--arc-vendor <id>` and `--arc-hook-overhead-ms <n>` after `--arc-event`, before `--`; the overhead is the hook's own time to its response (not NFR-003). Rewrites, event key, permission evaluation unchanged.
- **D8 summaries.** No vendor groups under `none`, never Unattributed. REMOTE_CI/REMOTE_EXEC are counted in `remote`, local CPU `null` with `not_metered_locally`. CPU sums only `canonical_root` reports. Range `[from,to)`.
- **D9 tree report.** `arcctl meter summary --job --json` adds the `meter.job_tree` reply under `process_tree`. Each value has a basis (`waited_rusage`, `tree_samples`, `unobserved`); confidence root `A`, revalidated descendants `C`; a possibly exited descendant keeps `partial` + `unobserved_descendants`, never 0. Sampling runs on the Meter cadence.
- **D10 start rate.** Switch `traffic.start_rate_control` via `policy.set_switch`; startup names `ARC_TRAFFIC_START_RATE_PER_MINUTE`, `ARC_TRAFFIC_START_BURST`. `start_rate::hold` runs after the window/budget admit, before the permit; v1 replies project the wait onto `wait`/`fifo`; raw `start_rate` only in `traffic.decision_history`.
- **D11 audit.** `change_audit::switch` after a committed set_switch, pause or resume; `change_audit::configuration` once at startup. No learned policy value exists.
- **D12 retention.** `src/retention.rs` runs rollup on its own schedule; rollup and delete in one transaction. Raw 72 h, minute 30 d, hour/day 1 year.
- **D13.** arc-daemon `tests/*.rs` auto-discovered.

## Tests

Seams: `job_process_meter::seam_arcctl_exec_job_meter_summary_reads_waited_report`, `usage_summary::seam_installed_hook_keeps_keyed_cargo_build_rewrite`, `traffic_policy::seam_job_diagnostics_reads_last_recorded_decision`.

Lane names: A `job_process_meter::mtr_002_unwaited_grandchild_cpu_memory_and_io_are_attributed_to_the_job`; B `usage_summary::mtr_004_vendor_project_and_period_summaries_match_recorded_jobs` (+ `met_017_snapshot_reports_traffic_queue_depth_while_build_permit_is_held`); C `traffic_policy::dec_013_start_rate_bucket_queues_then_grants_without_lost_or_duplicated_permit`, `trf_015_job_diagnostics_reads_every_decision_in_order_and_schedule`, `pol_004_every_policy_change_writes_before_and_after_audit`, `mtr_014_raw_samples_roll_up_then_delete_and_rows_stay_bounded`.

## Wave 2 ownership (one writer per file)

A lane owns new files in a directory named after its file.

- **A:** `arc-platform src/process_tree.rs`; `arc-meter src/job_tree.rs`; `arc-ipc src/meter/job_tree.rs`; arc-daemon `src/job_process_meter.rs`, `src/db_worker/resource_samples.rs`, `src/job_tree_dispatch.rs`, `tests/job_process_meter.rs`; `arc-cli src/meter/job_tree.rs`.
- **B:** `arc-ipc src/jobs/usage.rs`, `src/meter/usage.rs`; arc-cli `src/hooks/usage_fields.rs`, `src/exec/usage_fields.rs`, `src/meter/usage.rs`, `src/meter/snapshot_traffic.rs`; arc-storage `src/jobs/usage_fields.rs`, `src/usage_summary.rs`; `arc-meter src/snapshot.rs`; arc-daemon `src/usage_summary_dispatch.rs`, `tests/usage_summary.rs`.
- **C:** `arc-traffic src/start_rate.rs`; `arc-ipc src/traffic/decision_history.rs`; arc-storage `src/jobs/traffic/{start_rate,decision_history}.rs`, `src/policy/change_audit.rs`, `src/rollup.rs`; arc-daemon `src/db_worker/start_rate.rs`, `src/decision_history_dispatch.rs`, `src/retention.rs`, `tests/traffic_policy.rs`, `tests/hook_event_identity.rs`; arc-cli `src/job/diagnostics/history.rs`, `src/exec/traffic/cadence.rs` and its tests.
- **No lane edits:** migrations, manifests, `Cargo.lock`, `tests/support/**`, other arc-ipc files, `arc-cli src/meter.rs`, `main.rs` and the call-site files. **Wave 3:** integration may edit any file except `qa.md`.

## As delivered

- Delivered as written; `arc-meter src/snapshot.rs` unchanged. Lane C also edited `db_worker/traffic_configuration.rs`, `job/diagnostics.rs`.
- Defaults: 120 starts/min, burst 120 per class; a burst below its window fails startup. Rollup 2 s after start, then every 10 min, skipped while the writer is busy.
- Superseded-test repairs (task-2) strip and check only the wave's added key or options: arc-daemon helpers `hook_event_identity/hook.rs`, `agent_test_journey/stage.rs`, `meter_readings/snapshot.rs`, `support/meter_partial.rs`, `traffic_bun_scripts/mod.rs`; 4 arc-desktop supports.
- Integration: `hook_event_identity.rs` compares event keys (overhead differs); added `resource_samples::tests::written_samples_roll_up_to_their_growth_then_are_deleted`. QA B2/B3 (task-2): RSS, peak >= 48 MiB held; read >= 4 MiB.
- Full run fail, alone pass: `hook_event_identity` cursor redelivery (fixed by follow-up 1); arc-platform `process_tree::tests::ppid_descendants_are_read_with_their_identity_and_usage` (0 RSS after exec; waits); `traffic_resilience attach::short_lived_child_attach_is_reported_truthfully` (11:27Z, 12:59Z: exit window `Indeterminate` or refused capture; now waits, retries).
- Failed again alone: arc-cli governor `owned_tree_initial_capture_failure_cleans_a_separately_grouped_descendant`, `owned_tree_unwind_during_initial_capture_still_cleans_the_descendant` (11:43Z, alone, coverage 2): a stop sent during the child's exec was dropped; the fixture re-stops it. Test-only; new test `owned_tree_restops_a_process_whose_stop_was_lost_to_an_exec`; 150/150 loops loaded.
- `traffic_native_tools` failed 3 checks, alone pass: coverage 1 (12:04Z); `cmake_ordered_pairs` (12:59Z: enqueue unconfirmed in 1800 ms under swap, ran `TRACKING_UNCERTAIN`; witness fails on it at once); `go_owned_child_counts_are_isolated_between_parallel_cases` (14:11Z, 60 s watchdog, run 1205.8 s): a concurrent fork kept case 0's stdin pipe in case 1's release-gated `cat`; children now pipe-free `/usr/bin/true`. Test-only.
- Outcome: 22 rows Complete; MTR-003 Blocked on EXTERNAL item 3. Phase 1 51/16/2/2 of 71; Phase 0 22/1/0/17 of 40. No customer or revenue effect claimed.
