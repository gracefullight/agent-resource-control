# Phase 1 lane ownership

The lane wave starts from product tree `e68c481795bb551c20703dfb309ac3274e4aae45`. Each path or scope below has exactly one writer in this wave. Integration may change any of these paths after the wave; QA alone writes `qa.md` for its reviewed snapshot.

| Owner | Exclusive wave paths or scope |
| --- | --- |
| Lane A | `crates/arc-cli/**`; `crates/arc-ipc/src/jobs/**` and `crates/arc-ipc/src/meter/usage*`; `crates/arc-storage/**` except `src/rollup.rs` and `src/rollup/**`; `crates/arc-daemon/src/capability_advertisement.rs`, `crates/arc-daemon/src/process_attribution.rs`, `crates/arc-daemon/src/process_attribution/**` and `crates/arc-daemon/src/usage_summary_dispatch.rs`; `crates/arc-daemon/tests/agent_test_journey.rs`, `crates/arc-daemon/tests/agent_test_journey/**` and `crates/arc-daemon/tests/support/**`; `crates/arc-daemon/tests/process_attribution.rs` and `crates/arc-daemon/tests/process_attribution/**`; `README.md`; this `contract.md`; shared manifests, lockfiles and migration registration. |
| Lane B | `crates/arc-daemon/tests/meter_readings.rs` and `crates/arc-daemon/tests/meter_readings/**`. |
| Lane C | `crates/arc-meter/**`; `crates/arc-storage/src/rollup.rs` and `crates/arc-storage/src/rollup/**`; `crates/arc-ipc/src/coalescing.rs`; `crates/arc-daemon/src/retention.rs`, `crates/arc-daemon/src/db_worker.rs`, `crates/arc-daemon/src/db_worker/execution.rs`, `crates/arc-daemon/src/db_worker/outstanding.rs`, `crates/arc-daemon/src/lifecycle.rs`, `crates/arc-daemon/src/connection.rs` and `crates/arc-daemon/src/connection/coalescing.rs`; `crates/arc-daemon/tests/traffic_policy.rs` and `crates/arc-daemon/tests/traffic_policy/**`; `docs/managed-exec.md`. |

## Shared registration

`crates/arc-ipc/src/lib.rs` is not a lane scope. Its only wave change is the one-line `pub mod coalescing;` registration of Lane C's module, and Lane C is its single wave writer. No other lane edits that file in this wave.

## Superseded expectations

Migration `0011_codex_attribution` (Lane A) raises the supported schema from 10 to 11. Lane A alone updates exactly the earlier delivered expectations this replaces, and nothing else in those files:

- current-version and migration-count values in `crates/arc-daemon/src/db_worker/tests.rs`, `crates/arc-daemon/src/database_failure_tests.rs`, `crates/arc-daemon/src/server_tests.rs`, `crates/arc-daemon/tests/classification.rs`, `crates/arc-daemon/tests/resident/lifecycle.rs`, `crates/arc-daemon/tests/resident/schema.rs` and `crates/arc-daemon/tests/startup.rs`, where each future-schema case moves from version 11 to 12;
- the 0011 undo entry in `crates/arc-daemon/tests/traffic_resilience/schema_step_back.rs`.

## Boundaries

Scopes for coalescing and retention do not include the Lane A named files. The shared fixture interface stays backward compatible: `Sandbox::new` is unchanged, and `Sandbox::provisioned` is additive. A newly needed file outside these scopes, or any file claimed by two lanes, requires Chief coordination before editing. `docs/requirements/closing.md` and `docs/requirements/traceability.md` belong to integration, not to a lane.

## Lane A repair observations

The inspected local snapshot is commit `2f8a6f0e830beac3acd2b2f3d17d720766e8b888`, tree `3fd16b357176d1b19a6c5359cf1f172a8735cae1`; it includes earlier lane overlays absent from baseline commit `257e4ad1c05956727900296ecae87df693b41842`. The repairs described here are uncommitted changes in this task workspace.

The watchdog previously killed and reaped the wrapper before `Run::drop` could discover a separately grouped target. The timeout now invokes identity-checked tree cleanup before the wrapper group is reaped; normal `Owned` callers retain their existing behavior. The controlled timeout regression first failed because the target survived, then passed after the repair. It also proves that the target survives EOF and SIGTERM, and has its own cleanup guard. Normal command watchdogs remain 60 seconds. The isolated doctor assertion now expects schema version 11 without changing its other checks.

Focused local checks passed: the installed Claude/Codex Build-slot journey; both direct-drop and watchdog cleanup cases; `native_probe_then_full_100000_attempt_workload` (100,000 completed); the CLI Codex old-peer vocabulary test; five storage Codex migration, attribution and summary tests; and `native_process_attribution_separates_proven_estimated_and_unattributed`. Focused Clippy and formatting checks also passed. The installed-hook journey uses offline contract-derived fixtures and does not establish a live vendor invocation or trust decision. Workspace verification, other lanes, integration and independent QA remain outside this lane's local evidence.

## Correction to the frozen QA snapshot

Keep `qa.md` unchanged as the write-once review of commit `63e554fd39de354b149078cb36d42fd4a22d1051`. In that snapshot its item 3 sentence saying the sustained-load test fails if "clients stall or error" overstates the assertions. The eight connected doctor clients share one success counter: the test requires aggregate progress during retention and another 20 successes afterward, not progress by each client. `Failure::Busy` is tolerated; other returned failures are counted and asserted to remain zero. The test separately requires all 42,000 rows to roll in one run with admitted client work continuously outstanding, and checks the rollup totals and preserved audit rows. See `crates/arc-daemon/tests/traffic_policy/retention.rs` at the reviewed commit, `Load::start` and the `r_014_scheduled_rollups_progress_during_sustained_load` assertions.

For acceptance, read the frozen report together with this explicitly attributed correction and independently review that combined evidence against the same snapshot. The correction does not change the report's other observations or claim that every client progressed without `Busy`.

A fresh independent `qa-reviewer` accepted the report and correction together against that snapshot, including the earlier production-path gaps and both evidence size limits. This was a read-only review; it did not run tests or assess the later integrated product.
