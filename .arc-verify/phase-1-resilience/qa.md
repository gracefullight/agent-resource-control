Snapshot: commit 74eff1a (tree 14194e4), lanes A, B, C over contract tree 19d78bf, base f4dfeb2

# Phase 1 resilience: QA

Read-only review against SRS 9.9, 9.10, 10.2, NFR-009, AT-TRF-09/10 and contract.md; nothing was built or run. Verdict: WARNING. No CRITICAL, HIGH or blocking defect; MEDIUM items are follow-ups. Migration 0008 and the seam files are committed here. Paths under `crates/`; `td` = `arc-daemon/tests`.

## Items

All ten named tests take a production path: built `arcctl` (exec, hook-exec or a read) or the installed hook, then a real `spawn_isolated` daemon.

- **TRF-009 item 4**: `trf_009_schema_migrating_restart_keeps_the_active_permit_and_running_job` (td/traffic_resilience/restart.rs:204).
  - Steps back to version 7: `UpgradeRequired`, same held row (:107-115).
  - The current binary boots `Compatible` (:121-140).
  - Same permit, generation, PID and start token (:142); `job status` says running (:146); one start (:157).
  - One release at child exit (:161-186); a further restart changes nothing (:193-198).
- **AT-TRF-10 item 3**: `at_trf_10_forced_daemon_kill_keeps_one_active_permit_and_one_start` (restart.rs:209). Same flow, after a SIGKILL asserted as the exit signal (traffic_resilience/fixture.rs:111-119).
- **NFR-009 item 2**: `nfr_009_seeded_fault_interleavings_never_duplicate_or_lose_a_permit` (traffic_resilience/faults.rs:411).
  - Seeds 1, 3, 5, 7 (:25), printed on failure (:138-145).
  - Checked after every step and at the end (:364, :378-401); every fault kind acted (:417-454).
  - Invariants, from invariants.rs unless noted:
    - window and budget (:130-140);
    - one permit per job (:93-96);
    - release immutability (:112-124);
    - live owner (:147-199);
    - at most one start (faults.rs:157-169);
    - final active count 0 (:204-222).
  - No product fault seam: managed_exec/fault.rs is included by `#[path]` (traffic_resilience.rs:5-7). Gaps: M1, M2.
- **AT-TRF-09 item 1 / ADR-014 item 2**: `at_trf_09_redelivered_{claude,cursor}_event_creates_one_job_and_one_permit` (td/hook_event_identity.rs:296, :301). Recorded payloads go through the installed hook (hook_event_identity/hook.rs:66-101) and the vendor shell.
  - Same key, no raw id in the command (:232-237).
  - One job, entry and permit (:266-267); high-water 1 (:284).
  - The duplicate: no notice, unchanged args, cwd, environment, stdin, stdout and exit status (:264, redelivery.rs:91-107).
  - One start per execution (:262, :276); duplicate recorded on the existing job (:270); release (:277).
  - No raw id in any fixture file (:289, :292).
  - Controls: distinct ids give two jobs (:309), no daemon fails open (:380), no advertisement sends no key (:419; M3).
- **MET-001/006/008 item 1**: `met_host_cpu_memory_and_swap_values_agree_with_os_readings` (td/meter_readings.rs:81). Path: `arcctl meter snapshot --json` with a RUNNING `arcctl exec` stub.
  - CPU from base+3 on, on a sequence-proven window (host.rs:157-158, observation.rs:109-121), in 0 to 100, busy and idle contained (host.rs:57-110).
  - Memory and swap inside the test's own readings, never above `hw.memsize` or total swap (:113-185).
  - Available only: anything else panics (:63-66, :128-130).
- **MET-024/025 item 2**: `met_daemon_cpu_seconds_and_peak_memory_agree_with_os_readings` (meter_readings.rs:90).
  - Core-seconds between `proc_pidinfo` before and after the sample (daemon.rs:44-81).
  - Peak >= RSS, never below an earlier RSS, never decreasing (:85-122).
  - Peak <= the wait4 lifetime peak, one-sided only (:150-153).

Follow-ups:
- 1: `short_lived_child_attach_is_reported_truthfully` (attach.rs:106) uses the user-side rule. NULL `child_pid` is printed, not asserted (:190); the cycle-33 NULL was not reproduced.
- 2: the turn-lock wait is bounded at 64 slices (exec/traffic/cadence.rs:37, :95-104), proved with the injected sleep (cadence/tests.rs:159).
- project_fairness.rs orders by `traffic_entries.granted_at_ms` (:51-53).

## Cross-checks

- **FIXTURES.** Built binaries get env_clear, a temporary HOME and XDG and an explicit socket (td/support/journey.rs:113-137, runtime.rs:45-57). LLVM_PROFILE_FILE is forwarded (instrumentation.rs `forward`). Environment comparison goes through instrumentation (agent_test_journey/stage.rs `assert_environment`). Waits are on observed state with 60 s watchdogs (journey.rs:176-182, observation.rs:22). No `#[ignore]`, no asserted duration. Fixture removal: native_fixture.rs:72-83.
- **Raw ids.** Only the hook reads them (hooks/event_key.rs:53-56); they are framed and hashed (:33-49) and only the 64-hex key leaves (:70-84). Debug output is opaque (arc-ipc traffic/hook_event.rs:41-45). SQLite stores the key only (migration 0008:6-7); the audit row has no payload (arc-storage hook_event.rs:78-87).
- **Protocol 1.** Bytes pinned without the key (arc-ipc hook_event.rs:86-94). The legacy route sends `event: None` (exec/traffic.rs:325); a key goes only to the advertising holder (exec/event_key.rs:57-61). `DuplicateDelivery` is accepted only as the first reply to a keyed enqueue (governor/admission.rs:194).
- **Storage.** The duplicate check comes after owner verification and before the switch, job row, entry and permit (jobs/traffic/admission.rs:161). A retried first enqueue takes the existing-entry path (:121-139); a retried duplicate is `INSERT OR IGNORE`.
- **Size and ownership.** Every file is at most 499 lines. All lane edits are in their own lane's contract scope, except E1.

## Findings

### Security
- LOW S1, arc-storage src/hook_event.rs:64-70: a key matches for the job row's whole life, completed jobs included, so a re-run of a copied keyed command runs without a permit. This was decided (D5, D6). Fix (integration): document it in managed-exec.md.

### Performance
- LOW P1, cadence.rs:95-104: while a stopped holder keeps the lock, every waiter pays the full 64-slice bound on every poll (cadence/tests.rs `delay + TURN_SLICES`). Fix (lane A): skip the lock after an expiry until a non-blocking attempt succeeds.

### Code Quality
- MEDIUM M1, fault_steps.rs:189-240 and :245-265: the proxied arcctl wrappers run the unscheduled `tool run` with permits == 0 (:197, :238). Retried and lost replies on a permit holder come from the test's socket client (protocol.rs:1-7), not arcctl. Fix (integration): state this scope for NFR-009 in closing.md.
- MEDIUM M2, invariants.rs:112-124: a RELEASED permit may come back ACTIVE at a higher generation without counting against "released exactly once" (named in the lane A review). Fix (lane A): reject a generation rise from RELEASED.
- MEDIUM M3, hook_event_identity.rs:419-430: the "older daemon" control is an unlocked file, not a pre-capability binary (named in the lane C review). Fix (integration): record it in closing.md.
- LOW L1, meter_readings/os.rs: the readers share sources with arc-platform, so MET-006 has no second source; the target is not cfg-gated for macOS. Fix: `#![cfg(target_os = "macos")]`.
- LOW L2, arc-cli src/event_digest.rs:3-8: path-compiled hook copies use a digest stand-in that emits no key. Fix: state it in closing.md.
- LOW L3, restart.rs:107: held-permit step-back covers 8 to 7 only; 6 only with a finished job (traffic_resilience.rs:131). Fix: record it.
- LOW L4, traffic_resilience.rs:5-7: fault.rs unit tests run twice. Fix: move the proxy to support.
- LOW L5, docs/managed-exec.md:92 still says 2.56 s. Fix (integration): about 5.12 s; a stopped wrapper no longer blocks waiters.
- LOW E1, arc-cli src/event_digest.rs: new lane C file absent from the ownership table; one writer. Fix (integration): list it.

## Limits

- No execution results; pass, fail and durations rest on the runner log.
- Lane B calibration figures are Actor-measured and unverified; deviations are printed (observation.rs:175-177).
- Not traced: `job status` rechecking PID and start time, and the daemon mapping of the duplicate reply.
- No live vendor session; keyed-command approval prompts untested. Serena down: named-file reads.
