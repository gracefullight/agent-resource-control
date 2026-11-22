# Phase 1 resilience: contract, as delivered

Base `f4dfeb2` (tree `a53e900`). Wave 1 seam, wave 2 lanes A, B and C, wave 3 integration and QA. Paths under `crates/`. Row outcomes and measured figures: `docs/requirements/closing.md`, section "Phase 1 resilience: assessment".

## Decisions

- **D1 key.** `arc_ipc::traffic::hook_event::EventKey`, 64 lowercase hex. Only the installed hook computes it (`arc-cli src/hooks/event_key.rs`): SHA-256 of `DIGEST_DOMAIN` (`arc.traffic.hook-event.v1\0`), then for each of adapter (`claude`, `cursor`), `session_id`, `tool_use_id` and phase (`pre_tool_use`) its length as 8 big-endian bytes and its UTF-8 bytes. Raw ids never leave the hook process, enter SQLite or appear in diagnostics.
- **D2 hook.** For `cargo build`, `bun run build`, `cargo test` and `bun run test`, a payload with both ids returns `'<arcctl>' hook-exec --arc-event <key> -- <literal>`, the option directly after `hook-exec`. A payload missing either id, and the stage, container and gh rewrites, keep today's command.
- **D3 hook-exec.** Hidden `--arc-event <KEY>`, consumed like `--arc-request` (`exec/event_key.rs`). A malformed value prints `INVALID_REQUEST`, exits 2 and starts no child. `negotiated()` sends the key only when the daemon process that answered the run holds the advertisement lock (`project_key::holds`); the legacy `exec/traffic.rs` enqueue sends none.
- **D4 wire.** `JobRequest::Enqueue.event: Option<EventKey>`, omitted when `None` (protocol-1 bytes unchanged). Reply `JobResult::DuplicateDelivery { job_id }`, accepted only as the first reply to a keyed enqueue. Capability `traffic.hook-event.v1`: the daemon holds a POSIX write lock on `.traffic-hook-event-v1` beside the socket for the supervisor's lifetime (`arc-daemon src/lifecycle.rs`, `src/traffic_fairness_advertisement.rs`); no probe request.
- **D5 storage.** Migration `0008_hook_event`, expand-only: nullable `job_lifecycle.event_key` (CHECK 64 hex) and partial UNIQUE index `idx_job_lifecycle_event_key`; schema version 8. `arc_storage::hook_event::duplicate` runs in the admission transaction after owner verification and before any job row, queue entry or permit; `record` writes the key on the new job row, the index is the backstop. A duplicate is one `HOOK_EVENT_DUPLICATE` INFO audit row on the existing job, id `hook-event-duplicate-<job>-<registration>`, so a retried registration is recorded once. Retention: the key is deleted with its job row; no separate expiry.
- **D6 duplicate execution.** `traffic::Mode::Duplicate`: no job row, queue entry, reservation, `QUEUED` notice or wait; no stage link, launch fence, attach, completion, fallback, meter report or diagnostics. Its child runs once, unchanged, so its CPU is not in the existing job's summary. ARC never suppresses a vendor command.
- **D7 step back.** `tests/traffic_resilience/schema_step_back.rs::step_back(database, version)`, versions 7 and 6: undoes later migrations and their history rows, lowers `user_version`, asserts the schema of the first `version` migrations. The daemon must have exited; set `runtime.json` aside before a restart.
- **D8 registration.** arc-daemon `tests/*.rs` are auto-discovered; no registry under `scripts/` names a daemon target.
- **D9 turn lock (lane A).** `exec/traffic/cadence.rs`: a waiter tries the `.traffic-wait-turn` lock for at most `TURN_SLICES` = 64 slices of 40 ms (2.56 s), then polls without it; worst case about 5.1 s after its previous reply. No stderr text.
- **D10 attach (lane A).** Follow-up 1 is not a defect under the user-side rule (no `TRACKING_UNCERTAIN`, status shows the real outcome); no attach-recording file changed.
- **D11 Meter (lane B).** No reading was wrong; no `arc-meter`, `arc-platform` or `arc-core` change. Tests use containment bounds and the daemon's `sequence`, never elapsed time.

## Tests

Seams: `traffic_resilience::seam_stepped_back_database_migrates_at_daemon_start`, `meter_readings::seam_real_daemon_meter_snapshot_json_parses_through_built_arcctl`, `hook_event_identity::seam_installed_hook_keeps_todays_command_for_a_payload_without_event_ids`, `hook_event_identity::seam_hook_exec_consumes_the_event_option_and_rejects_a_malformed_key`.

Fixed lane names, all delivered and passing:

- A, `traffic_resilience`: `restart::trf_009_schema_migrating_restart_keeps_the_active_permit_and_running_job`, `restart::at_trf_10_forced_daemon_kill_keeps_one_active_permit_and_one_start`, `faults::nfr_009_seeded_fault_interleavings_never_duplicate_or_lose_a_permit`; follow-ups `attach::short_lived_child_attach_is_reported_truthfully` and arc-cli `exec::traffic::cadence::tests::held_turn_lock_cannot_stop_a_waiter`.
- B, `meter_readings`: `met_host_cpu_memory_and_swap_values_agree_with_os_readings`, `met_daemon_cpu_seconds_and_peak_memory_agree_with_os_readings`.
- C, `hook_event_identity`: `at_trf_09_redelivered_claude_event_creates_one_job_and_one_permit`, `at_trf_09_redelivered_cursor_event_creates_one_job_and_one_permit`; controls `at_trf_09_distinct_tool_use_ids_create_two_jobs`, `at_trf_09_redelivered_event_fails_open_without_a_daemon`, `at_trf_09_a_daemon_that_does_not_advertise_receives_no_key`.

Unit: arc-ipc `traffic::hook_event`, arc-storage `jobs::traffic::tests::{event_key,hook_event}`, arc-cli `exec::event_key`, `hooks::event_key`.

## Changed

- **Wave 1.** 44 request-literal sites (one `event` field each, found with `cargo check --workspace --all-targets`, including `apps/desktop/src-tauri/tests/memory_pressure/native_reserved.rs`), 19 schema-version pins 7 to 8, the seam modules above, and a repair in arc-daemon `tests/classification*` (one connection per case; the daemon closes an idle connection after its 1 s header timeout).
- **Lane A.** arc-cli `src/exec/traffic/cadence.rs`, `cadence/tests.rs`; arc-daemon `tests/project_fairness.rs` (start order from `traffic_entries` grant times), `tests/traffic_resilience.rs` and `traffic_resilience/{fixture,restart,faults,fault_steps,invariants,protocol,attach}.rs`.
- **Lane B.** arc-daemon `tests/meter_readings.rs` and `meter_readings/{os,observation,host,daemon}.rs`.
- **Lane C.** arc-cli `src/hooks/event_key.rs`, `src/exec/event_key.rs`, the `exec` admission and run modules, `agent_hooks/{claude,cursor}.rs`, `event_digest.rs`; arc-ipc `traffic.rs`; arc-storage `hook_event.rs`, `jobs/traffic/admission.rs` and tests; arc-daemon `src/lifecycle.rs`, `src/traffic_fairness_advertisement.rs`, `tests/hook_event_identity*`; the existing tests that compare the whole returned command now expect the keyed route.
- **Integration.** `docs/managed-exec.md`, `docs/requirements/closing.md`, `docs/requirements/traceability.md` and this file. No integration repair was needed; every listed crate and arc-daemon target passed on the merged tree.

Cargo.lock, manifests and desktop TypeScript are unchanged; no Bun test file was added; no test is ignored; every touched file is at most 500 lines.

## Ownership

Wave 2 had one writer per file: lane A the cadence, attach-recording and `traffic_resilience`/`project_fairness` files; lane B `meter_readings`, `src/meter*`, `arc-meter`, `arc-platform`, `arc-core/src/meter.rs`; lane C the hook, exec, IPC, storage and other daemon sources and the returned-command tests. Migrations, `tests/support/**` and `Cargo.lock` were read-only for lanes. Wave 3: integration any file but `qa.md`; QA only `qa.md`.
