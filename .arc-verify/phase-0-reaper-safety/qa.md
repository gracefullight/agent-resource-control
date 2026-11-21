Reviewed snapshot: commit `2b0eca1d8865ee2bf4950e32c988599ae7772182` (tree `5a58415d96c44c0f4f3c5463a635bb99a76cb528`): tasks 1-4 on baseline `86fac84ec77e216f2d86b8c072140ccdf91564b7` (tree `27635eb500955608140eb71ba557adc5bf5ae371`). Task-5 deliveries are not reviewed. QA skill `oma-qa`; no product file edited.

## Review Result: WARNING

No CRITICAL or HIGH finding and no reproducible defect that blocks tasks 2-4. In this snapshot `closing.md` and `traceability.md` equal the baseline (the four rows Partial, Phase 0 15/14/8/3); promotion is task-5's.

## Method and receipts

- Read in full: daemon `reaper.rs`, `reaper/*.rs`, `reaper_dispatch.rs`, `job_ownership.rs`; storage `reaper.rs`; wrapper `signals.rs`, `reaper_action.rs`; `reaper_identity.rs`; both fixture daemons; the Reaper, ownership and recovery tests; the shared-route diffs.
- I ran no build, test, lint, coverage or harness. Receipt: runner check `.agent-valley/integration-checks/2026-11-21T12-12-10-363Z.log` (arc-daemon, arc-storage). `crates/` and `docs/requirements/` of the worktree it ran in are byte-identical to this snapshot now (`diff -rq`).
- Full `managed_exec` run: 162 passed, 1 failed. Solo rerun (`target/av-runner/gates/cargo-test-scoped.rerun-2026-11-21T12-22-17-877Z.log`): 163 passed. All nine named tests are `ok` in both. Coverage, desktop, bun and the full workspace run were not in that check (final wave).
- arc-cli `reaper_action_tests::*` (2), arc-ipc `reaper::tests` (2), arc-platform `reaper_identity` (1) and 371 bun tests pass in check `2026-11-21T11-23-28-757Z.log`; those crates are unchanged.
- Nondeterministic, outside this wave: `capture_integration::closing_locality::forwarded_docker_socket_is_not_contacted_or_treated_as_local` (`docker_controls.rs:86`, ConnectionRefused) failed in the full run, passed alone and in the 11-23 check. Recorded, not a rejection.
- Task-4's one-off failure at `current_user_ownership.rs:232` does not match this snapshot: the relay read is a `let ... else` at line 161, no unwrap. The test passed in both runs.
- Serena did not connect and the guard blocks source search: absence claims cover only the files read. `av.yaml` unreadable, untouched; `Cargo.lock` unchanged.

## Row evidence

**SAFE-001** (items 1-2). Item 1: `managed_route_tracks_owned_child` (resident `arc-daemon`, built `arcctl`): the job row names the wrapper and the child's PID, start time and boot. Item 2: the daemon Reaper modules contain no `kill`; the only Reaper signal is `reaper_action.rs:262`, on the wrapper's own unreaped child under `WAIT_GATE`. `protected_candidates_receive_no_signal`: unknown ownership (`UNKNOWN_OWNERSHIP`), two forged registrations, a live session and a real daemon restart (`INSUFFICIENT_AUTHORITY`); both children exit 17. `owned_inactive_child_reclaimed_and_recorded`: SIGTERM outcome, exact `gc_operations` rows, four `audit_events`. Supports Complete.

**R-007** (items 1-2). `stale_recorded_identity_keeps_live_child_alive`: a stale start time at the daemon (`IDENTITY_MISMATCH`, no claim) and at the wrapper (`WRAPPER_IDENTITY_MISMATCH`, claim withdrawn); the SIGTERM-sensitive child stays alive and exits 17. `revalidation_precedes_every_signal`: ten conditions plus a wrapper refusal before SIGTERM and again before SIGKILL. `act()` rechecks PID, parent, both UIDs, start time, executable path, device and inode, and foreground under the gate before each single `kill`. This is injected stale-identity behaviour; no test observes the OS reusing a PID. Supports Complete.

**SAFE-007** (item 1 only). `built_cli_current_user_socket_and_job_ownership`: socket type, owner == euid, mode 0600, kernel UIDs of daemon, CLI and child, job bound to that CLI and child. `forged_or_mismatched_job_identity_is_rejected`: forged observer, unrelated or self child, and forged completion with and without a lying callback are rejected; an unchanged relay is accepted. Every process is the current user. Item 2 is EXTERNAL, not exercised, not claimed: no peer of another UID reaches the daemon. Supports Blocked with only item 2 remaining.

**NFR-010** (items 1-2). `forced_daemon_death_before_commit_rolls_back_and_integrity_ok` and `forced_daemon_death_after_commit_survives_and_integrity_ok`: SIGKILL while the writer is held at a socket barrier; restart on the same database; the daemon's doctor and a direct `PRAGMA integrity_check` return `ok`, `journal_mode` is `wal`; 700, 702 and 703 survive, 701 does not; 702 is shown WAL-only before the kill. AT-SWP-10 is not demonstrated and stays open. Supports Complete.

Isolation: cleared environments, temporary HOME, explicit socket and DB, forwarded `LLVM_PROFILE_FILE`, 60 s watchdogs, injected clock.

## Findings

MEDIUM, Reaper lane
- `crates/arc-daemon/src/reaper.rs:336-343,257-262`: an unread SIGTERM reply is timed as delivered, so a fully revalidated SIGKILL can follow 5,000 ms later when no SIGTERM arrived. Fix: in the `Some((Some(term), None))` arm request `Signal::Term` again while `accepted` is false.
- Test gap, `reaper_safety.rs:409-415`: the pre-SIGTERM stage uses a child that ignores SIGTERM. For lease, protected, foreground, altered PID, executable or UID, and unreadable evidence, "no SIGTERM" rests on decision rows; liveness proves them only before SIGKILL. Fix: run `refused(.., "SIGTERM_REQUESTED")` on an `echo` child and assert exit 17.

LOW
- `reaper.rs:229`: the capability is removed before the outcome decision is written; a failed write turns a confirmed reclaim into `TERMINATION_UNATTRIBUTED`. Record first, then remove.
- `reaper.rs:230-236`: `RECLAIMED/TERMINATION_CONFIRMED` is recorded for any outcome after an accepted signal, including the child's own exit code.
- No test read reaches `RECORD_MISMATCH`, `OWNERSHIP_UNVERIFIED`, `PROCESS_MISSING`, `SUPERVISOR_MISSING`, `CLAIM_REJECTED` or `ENDPOINT_*`; `validate.rs` has no unit test.
- `reaper_safety.rs:263,271`: the second forged registration also names a non-parent session, so the inode check is not isolated.
- `arc-cli/src/main.rs:255-258` and `doctor.rs:59` print only "Sweeper off." while the daemon runs the Reaper.
- `contract.md` is stale: `reaper_safety.rs` is 492 lines, not 296; it omits `RECORD_MISMATCH`, `TERMINATION_UNATTRIBUTED`, claim withdrawal to `TRACKED` and the faults `foreground`, `action-reply`, `recorded-identity=pid|executable|uid`; it lists `db_worker/execution.rs` as shared although task-4 changed it.
- `current_user_ownership.rs:349-351`: the PID 1 error code is asserted only if an error returns; `:223` needs a temp root <= 100 bytes.

## Production-path gaps (unverified, not failures)

- Every Reaper test drives `arc-reaper-daemon-fixture`: production sources and `entry::run`, plus an injected clock, pass trigger and faults. No test runs resident `arc-daemon` with `ARC_REAPER=process`, its 30 s interval or its wall clock. The resident daemon is shown only with the switch off: zero ownership and GC rows.
- Foreground protection is exercised only by the injected fault. Lease and protected rows are written by test SQL; no production writer exists, so a deliberately detached job cannot be shielded after its launcher exits.
- Scheduled (Traffic, Governor) jobs are never registered; inventory is capped at 64 of each kind per pass.
- Safe but never reclaimed, untested: a child that execs after its birth capture (permanent mismatch) and a child whose wrapper died (`SUPERVISOR_MISSING`). Descendants are never signalled.
- The Reaper does not read the Governor OFF state (SAFE-013, Phase 3, not selected); `ARC_REAPER` is its only gate.
- NFR-010: crash and restart both use `arc-recovery-daemon-fixture`, never the `arc-daemon` binary. The before-commit case loses an in-memory transaction; uncommitted frames spilled to the WAL are not exercised. The main-file-only assertion assumes no checkpoint right after commit. The `execution.rs` seam runs only in the fixture binary, which labels stages by alternation.
