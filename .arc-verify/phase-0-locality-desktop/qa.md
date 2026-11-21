Snapshot: commit 1a05f77d74f0b3e4c7de091ad939ee333005f012, tree 0cf6af957ffe732899b86e067caa82f07ae142a0 (baseline aabf278d, tree a2f228e8)

# Independent QA: Phase 0 locality and desktop

Review Result: WARNING. No CRITICAL or HIGH; three MEDIUM. Read-only, skill oma-qa. I ran no test: execution evidence is the runner log `.agent-valley/integration-checks/2026-11-21T17-24-41-173Z.log` (`integration check passed`; no tree hash recorded). Every test named below is `ok` there, with 0 ignored in `managed_exec`, `lifecycle` and `registered_commands`; Bun 372 pass, 0 fail.

## Evidence per HOST item

"PP": the built `arcctl`, hook or registered desktop command against a real private daemon with offline stubs. `L` = `apps/desktop/src-tauri/tests/lifecycle.rs`; `S` = `crates/arc-daemon/tests/managed_exec/capture_integration`; `C` = `S/closing_locality/local_container.rs`; `T` = `S/closing_shell/stages.rs`.

- **DEC-003.1, DEC-004.1, ADR-002.1, AT-GUI-01.1**: `close_request_hides_main_and_same_daemon_serves_bridge_and_cli` (L:233). PP: `configure_with_ports` commands, real `arc-daemon`, built `arcctl`. The close request is injected (L:245, F6). The daemon's self-reported PID is macOS-only (L:278).
- **AT-GUI-04.1, DEC-022.1**: `credits.test.ts:39` renders the production React component with a mock native client: icon-only Phosphor links, Try OMA, fixed URLs (SRS 2228-2230). `registered_opener_accepts_exactly_the_three_enum_destinations` (`registered_commands_support/input_validation.rs:36`) is PP for `open_credit_link`.
- **DEC-012.1, ADR-005.1**: the existing DET-005 tests under `S`, all PP. Row statuses are not set in this snapshot.
- **G-02.1**: `installed_claude_hook_executes_same_remote_child_with_held_build_permit` and `remote_docker_and_ssh_execute_while_local_build_permit_remains_held`. PP.
- **G-02.3, ADR-005.3-4** are AT-TRF-03 and DET-011.1-2; **G-02.4, ADR-005.5** are AT-TRF-12 and DET-016.1-2.
- **AT-TRF-03.1-3, DET-011.1-2**: `local_container_{buildx,docker_build}_via_{exec,hook_exec}` (C:75-229). PP on `arcctl exec` and `arcctl hook-exec` (F4): classify and stored LOCAL_CONTAINER, zero stub starts while a Build permit is held, one run, one released reservation equal to the held build's share. Matches SRS 2639-2645. macOS-only; an interposed socket inventory, not a live engine, proves the local endpoint.
- **AT-TRF-03.4, DET-011.4**: `local_container_classify_contacts_no_socket` (`local_container/readers.rs:160`). PP: 0 starts, 0 connections, no job row.
- **DET-011.3**: `local_container_other_endpoints_unchanged` (C:235). PP, both routes.
- **DET-011.5**: `docker_local_listener_controls_use_both_production_routes_and_admission`. PP on classify and `arcctl exec` only; the hook does not rewrite `act` (`hooks/container.rs:11-14`). macOS-only.
- **AT-TRF-12.1-2, DET-016.1-2**: `mixed_{bun,make}_stages_via_hook` (`S/closing_exec/stages.rs:16,22`) and `_via_exec` (T:32,38); body `stages/admission.rs:18-221`. Full PP: installed hook, vendor shell (`/bin/sh` for bun, `/bin/zsh` for make), `hook-exec`, daemon; and `arcctl exec -- bash -c`. Matches SRS 2725-2731. Not in closing.md: the daemon has `ARC_TRAFFIC_CPU_BUDGET_MILLIS=4000` (`stages/harness.rs:58-64`).
- **AT-TRF-12.3, DET-016.3**: `mixed_failing_test_stage_skips_gh_stage` (T:46). PP on sh, bash, zsh and exec; environment via `instrumentation::normalize_actual`.
- **AT-TRF-12.4, DET-016.5**: `mixed_classify_executes_nothing` (T:251). PP.
- **DET-016.4**: `mixed_unseparable_compounds_pass_through` (T:114): nine exec cases, six hook sources. PP. See F3.

## DET-011.6: desktop reader hops

1. Daemon reply: `local_container_projection_for_earlier_peers` (`readers.rs:22`, diagnostics at 72-83). PP.
2. Registered command: `registered_traffic_diagnostics_return_the_local_container_projection` (`registered_commands_support/traffic_diagnostics.rs:400`). A fake peer replays the wire fixture.
3. TypeScript reader: "local container build diagnostics are read as the HOST_LOCAL projection and LOCAL_CONTAINER is rejected" (`traffic-scripts-contract.test.ts:212`), same fixture record.

No single test crosses all hops. Only the `classification` object is identical across the three; the fixture record of hops 2 and 3 differs from the daemon's reply (F5). The TypeScript reader still rejects LOCAL_CONTAINER (`traffic-scripts-contract.ts:106-111`).

## Other checks

- **Fixture rules**: built binaries get `env_clear`, a temporary HOME, explicit paths and `LLVM_PROFILE_FILE`; waits have 60 s diagnostic deadlines, except F7. Lane A asserts fixture removal; lanes B and C rely on `Fixture::drop`.
- **Readers**: `transport::write_response` (`arc-daemon/src/transport.rs:171-183`) projects every reply. The unprojected writer serves only the two negotiated operations (`connection/response.rs:20-24`). `readers.rs` finds no LOCAL_CONTAINER in protocol-1, locality, diagnostics or endpoint replies.
- **Ownership**: `git diff --stat e678629a HEAD` lists 28 files, all inside the lane and task-7 assignments; none over 500 lines; no manifest changed.
- **Security**: no finding. The request id enters shell source only as 32 hex characters (`hooks/stages/compound.rs:38-50`).

## Findings

- **F1 MEDIUM, owner task-5.** `crates/arc-daemon/tests/managed_exec.rs`: the binary ran 130.37 s against the 60 s rule (log line 1631; contract.md:41 records 112 s). Fix: move the `stages` and `local_container` modules to a new `[[test]]` target.
- **F2 MEDIUM, owner task-5, known.** `crates/arc-cli/src/exec/governor/admission.rs:172` writes `QUEUED\nWaiting to start.\n` with no `Reference:`. Reproduce: `local_container/fixture.rs:246-274` asserts that exact text. Fix: `write_notice(&format!("QUEUED\nWaiting to start.\nReference: {}\n", id.as_str()))`, strip the line in `harness.rs:373-381`, update C:153-157.
- **F3 MEDIUM, owner task-5 (lane B files).** `crates/arc-cli/src/hooks/stages.rs:23-36` rewrites the two exact texts without reading the Makefile or the endpoint. Reproduce: the unit test at `stages.rs:77-112` gets the rewrite with no Makefile or remote. With recipe `$(TEST_RUNNER) test` or an unproven remote the hook route is therefore still staged, which DET-016.4 does not state; no test runs that case. Fix: add a named hook-route test for both cases and restate DET-016.4.
- **F4 LOW, owner task-5.** C:86-98 asserts the installed hook's rewrite, then runs `hook-exec` directly; the vendor shell never runs it for Docker. Fix: record this in AT-TRF-03.2 and DET-011.1-2.
- **F5 LOW, owner task-5 (task-7 files).** `crates/arc-ipc/src/traffic/diagnostics/typescript.rs:192-203` builds the fixture from `results[3]`, so `admission.reservation` is null, while the daemon reply has a reservation (`readers.rs:81-82`). An ad-hoc probe of `isTrafficDiagnosticResult` accepted that shape. Fix: derive the fixture from a record with a Build reservation.
- **F6 LOW, owner task-5.** `apps/desktop/src-tauri/src/lifecycle.rs:131-137`: no test executes the `CloseRequested` arm of `on_window_event`; Tauri's `CloseRequestApi` cannot be constructed. Fix: state this beside the cited test.
- **F7 LOW, owner task-5.** `daemon_bridge_support/mod.rs:339-361` (`wait_running`, used at L:288) loops with no deadline. Fix: assert a 60 s deadline.

Stale, already assigned to task-5, not new: contract.md:3, :12 (D6), :27 (`Reference:`); closing.md DET-005 items 9 and 13 and lines 100-101, 198-199, 221-222; `docs/managed-exec.md:32,59` omit the Docker literals.

## Not checked

- I ran no test, coverage, harness, fmt or clippy.
- Content search was unavailable: whether a reader outside the diff compares `locality` with `HOST_LOCAL`, and which other tests assert the QUEUED text.
- An older daemon with the new `arcctl exec`; non-macOS compilation; the Cursor adapter; desktop rendering of a MIXED request record; `traceability.md`.
- No live Docker engine, GitHub dispatch, vendor runtime, tray, browser or screen. No customer or revenue effect claimed.
