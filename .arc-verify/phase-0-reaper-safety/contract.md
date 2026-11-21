# Phase 0 Reaper safety: shared contract

Status: route, lanes, integration delivered. See `closing.md` and `traceability.md`: Phase 0 is 18 Complete, 10 Partial, 8 Pending, 4 Blocked. QA and runner results are not claimed.

## Delivered route

- **Capability.** The daemon never signals a job child; the `arcctl exec` wrapper, its direct parent and sole reaper, does. `exec/signals.rs` holds `WAIT_GATE` across each consuming wait until `CHILD` is cleared, and `signals/reaper_action.rs::act` holds it from its checks through `kill`: the PID cannot be reaped or recycled in between. `act` requires the requested PID to be `CHILD`, the daemon's record to equal the birth evidence, `reaper_identity::matches` (parent, UIDs, start time, executable path, device, inode) and a non-foreground group; otherwise no signal and a `Refusal`. One request, at most one signal.
- **Registration.** After Attach on the directly managed `job.start` route the wrapper sends additive `reaper.register.v1` (`arc_ipc::reaper`): job, observer, `ChildEvidence`, session, private endpoint, token. The daemon accepts only if the direct peer is the live observer, `job_ownership::managed_child` holds, its own fresh capture equals the evidence, the session is the observer's live parent (not PID 1), the endpoint's kernel peer is the observer and the job row names that observer and child. The wrapper serves only that daemon instance and token; both are memory-only. Scheduled (Traffic, Governor) jobs are not registered yet.
- **Pass.** `src/reaper.rs`: inventory, `validate::owner`, `validate::safety`, `executor::request`. A fenced `TERMINATING` claim commits before each request. After 5,000 ms on the Reaper clock every condition is evaluated again before a separate `KILL` request.
- **Record.** `arc_storage::reaper` through the single writer: `resource_objects` (`MANAGED_PROCESS`, `PROVEN` or `UNKNOWN`), `resource_leases` (session lease `ACTIVE` to `ENDED`), and one `gc_operations` plus one `audit_events` row (`reaper.decision`) per changed decision, with reason and observation time. States: `BLOCKED`, `SKIPPED`, `RETRY_LATER`, `TERMINATING`, `RECLAIMED`, `PARTIAL_FAILURE`. `RECLAIMED` needs a wrapper-reported delivery and the outcome only the wrapper's consuming wait writes; an unread reply (`TERMINATION_UNATTRIBUTED`) or lost capability is `PARTIAL_FAILURE`. A refused claim that sent nothing returns to `TRACKED`. Fresh evidence against the record is `IDENTITY_MISMATCH`; disagreeing stored records are `RECORD_MISMATCH`. Decision rows carry no executable path. No migration.
- **Switch.** Off unless the daemon starts with `ARC_REAPER=process`: no pass, no row, registration answers `UNSUPPORTED_CAPABILITY`. "Sweeper off" is unchanged; when selected the ready line adds "Process Reaper on."

## Definitions

- **Owned:** `PROVEN` from a verified registration, a live capability in this daemon, and fresh evidence equal to the record. Legacy, relayed, forged or absent registration, and any job after a daemon restart, is never signalled.
- **Owner session:** the process that launched the wrapper. **Session ended:** its recorded identity is observed missing or replaced and the wrapper is observed reparented. Indeterminate is not ended.
- **Active reference:** any `ACTIVE` unexpired `resource_leases` row, including the session's own lease.
- **Inactive:** session end recorded, no active reference, and 600,000 ms elapsed since the recorded end.

## File ownership (one writer per file)

- **Reaper lane:** `arc-daemon/src/reaper.rs`, `src/reaper/**`, `arc-storage/src/reaper.rs`, `reaper/tests.rs`, `arc-core/src/process_identity.rs`, `tests/managed_exec/reaper_safety.rs` (492 of 500 lines).
- **Ownership lane:** `src/job_ownership.rs`, `tests/managed_exec/current_user_ownership.rs`.
- **Recovery lane:** `src/db_worker/execution.rs` (inert seam), `tests/managed_exec/crash_recovery.rs`, `tests/fixtures/recovery_daemon.rs` (`--fixture-commit-barrier`).
- **Shared, read-only for lanes:** everything else, including manifests, `arc-ipc`, `arc-platform`, `arc-cli`, `tests/managed_exec.rs`, `reaper_fixture.rs`, `reaper_escalation.rs`, `tests/fixtures/reaper_daemon.rs`, `tests/support/**`.
- Integration owns runner-reported repairs, `closing.md` and `traceability.md`. QA alone writes `qa.md`, once.

## Reaper lane seams

`arc-reaper-daemon-fixture --fixture-control <dir>` shares production startup; its seams live in `src/reaper/fixture.rs` and `arc-daemon` never installs them. Files: `clock` (ms), `pass` (requested sequence), `done` (finished sequence), `fault-<name>` (`fixture::fault(name)`). Child mode `ignored-term` survives SIGTERM.

- Unknown ownership: a protocol job with no registration.
- Active reference: a registered job whose launcher is alive, or another `ACTIVE` lease row.
- Mismatched identity: injected `fault-recorded-identity` = `daemon` (stale start time at daemon validation), `pid`, `executable`, `uid`, or `request` (the wrapper gets the stale record and refuses). Stale-identity behaviour, not observed OS PID reuse.
- Indeterminate identity: injected `fault-child-evidence` or `fault-session`. Also `fault-foreground`, `fault-action-reply`.
- Insufficient authority: restart the daemon on the same database (capability lost), or injected `fault-authority`.

## Named HOST cases

- SAFE-001: `managed_route_tracks_owned_child`, `owned_inactive_child_reclaimed_and_recorded`, `protected_candidates_receive_no_signal`.
- R-007: item 1 `managed_route_tracks_owned_child`; item 2 `stale_recorded_identity_keeps_live_child_alive`, `revalidation_precedes_every_signal`, over the arc-cli gate tests in `signals/reaper_action_tests.rs`.
- SAFE-007 item 1: `built_cli_current_user_socket_and_job_ownership`, `forged_or_mismatched_job_identity_is_rejected`. Item 2 is EXTERNAL; not selected or claimed.
- NFR-010: `forced_daemon_death_before_commit_rolls_back_and_integrity_ok`, `forced_daemon_death_after_commit_survives_and_integrity_ok`. AT-SWP-10 stays open.

## Focused checks (offline)

All pass: formatting; workspace all-target Clippy; `managed_exec` 163/163 twice in succession; daemon unit 142, fixture daemons 127 each; arc-cli unit 184; arc-storage 172; arc-ipc 128; arc-platform 15. See `closing.md`.

## Rules

Fixtures: offline, cleared environments, temporary HOME, explicit configuration, `runtime::instrumentation::forward`; observed state or the injected clock; 60 s child watchdogs. Actors never run the workspace test command, coverage, the architecture harness, SwiftPM-backed tests, Playwright or screen checks. Review: the first names every blocker; later only an unresolved earlier finding, a defect in changed lines or an owned-file runner failure rejects. Fail in a full run but pass alone: recorded; rejects on two consecutive failures or a failure alone. A passing runner integration check is complete evidence; final fail-then-pass is NONDETERMINISTIC.

## Immutable runner commands

Each occurs once; timeout 7,200,000 ms.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
bun test apps/desktop/tests/bridge-client.test.ts apps/desktop/tests/contracts.test.ts apps/desktop/tests/credits.test.ts apps/desktop/tests/dashboard.test.ts apps/desktop/tests/governor-contract.test.ts apps/desktop/tests/governor.test.ts apps/desktop/tests/lifecycle.test.ts apps/desktop/tests/meter.test.ts apps/desktop/tests/traffic-scripts-contract.test.ts apps/desktop/tests/traffic-scripts.test.ts scripts/coverage/accounting.test.ts scripts/coverage/desktop-foundation.test.ts scripts/coverage/evidence.test.ts scripts/coverage/gate.test.ts scripts/coverage/hygiene.test.ts scripts/coverage/terminal-outcomes.test.ts scripts/coverage/traffic-accounting.test.ts scripts/harness/coverage-reuse.test.ts scripts/harness/equivalence.test.ts scripts/harness/traffic-targets.test.ts scripts/requirements/traceability-parser.test.ts scripts/requirements/traceability.test.ts
```
