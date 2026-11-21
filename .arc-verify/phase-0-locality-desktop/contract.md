# Phase 0 locality and desktop shell: shared contract

Status: integrated. The seam, lanes A, B and C and task-7's desktop reader are delivered; integration assessed the thirteen rows in `docs/requirements/closing.md`, section "Phase 0 locality and desktop".

## Decisions as delivered

- **D1 hook stages.** `hooks.rs::rewrite_literal` and `literal_argv` delegate to `hooks/stages.rs`: `rewrite(command, context, surface) -> Option<String>`, `literal_argv(argv) -> bool`. It rewrites exactly `bun test && gh workflow run ci.yml` and `make test && gh workflow run ci.yml` to `<arcctl> hook-exec --arc-request <id> -- <stage 1> && <arcctl> hook-exec --arc-request <id> -- <stage 2>`, one fresh id per rewrite, and admits exactly `bun test`, `make test`, `gh workflow run ci.yml`. The shell keeps `&&`; arcctl schedules nothing.
- **D2 exec stages.** Form: argv exactly `bash -c <one of the two sources>`. `exec/run.rs` builds its child through `exec/stages.rs::command(argv, request: &JobId, classification) -> Command`, which replaces only the source with the D1 rewrite when `classification` is MIXED with no uncertainty; the request id is that run's own job id, so the bash child stays the request record. `sh -c`, `zsh -c` and every other form keep today's result.
- **D3 linkage.** Carrier: `hook-exec --arc-request <32 lowercase hex>`, an option before `--` that never reaches the child; a malformed value exits 2 with no child. `exec::run_hook(argv, request: Option<String>)` scopes it; `stages::link` sends, after registration and before the launch fence, additive `job.request.v1` (`arc_ipc::request`): `StageRequest::Link { request_id, job_id, observer }` or `Stages { request_id }`, reply `StagesResult { request_id, jobs }` in link order, at most 16. The daemon authorizes the observer like `job.start`; `arc_storage::request::apply` writes one `audit_events` row per stage (`event_type` `REQUEST_STAGE`, `entity_id` the request), only for the wrapper that registered the job. No migration. A lost or unsupported link never delays a stage. A stage is linked when its admission returns; read a waiting stage through job status.
- **D4 vocabulary.** `Locality::LocalContainer`, serialized `LOCAL_CONTAINER`. `arc_detector::compute_location::of(class, reason) -> Locality` alone decides a location (`Classification::new`) and returns `LocalContainer` for `(Build, LocalEndpointBuild)`, so `act -j build` moves with Docker. `Locality::is_local_compute()` replaced every "local compute" comparison.
- **D5 projection.** `Classification::host_local_projection()` and `canonical()`. `arc_ipc::compute_location`: `OPERATION_CLASSIFY` `job.classify.container.v1`, `OPERATION_JOB_STATUS` `job.status.container.v1`, `negotiated`, `project`, `canonical`, `project_reply`. Requests carry the projection; the daemon stores the canonical form. Every daemon reply goes through `transport::write_response`, which projects; only those two operations use `write_negotiated_response`. Readers: `job.start` and `job.status` keep `legacy()` (UNKNOWN); `*.locality.v1`, `traffic.endpoint.v1`, diagnostics and the desktop get HOST_LOCAL; the two container operations get LOCAL_CONTAINER.
- **D6 desktop.** No copy, UI or migration change. Task-7 added the four endpoint reasons to the diagnostics reason vocabulary: generator `arc-ipc/src/traffic/diagnostics/typescript.rs`, regenerated `traffic-scripts.generated.ts` and `traffic-scripts-wire.json`, validator `traffic-scripts-contract.ts`. The desktop reads the HOST_LOCAL projection and rejects `LOCAL_CONTAINER`.
- **D7 host binary.** `apps/desktop/src-tauri/tests/lifecycle.rs` registers `daemon_bridge_support`. Lane A's test is there.

## Seam tests

- A: `registered_app_reads_private_real_daemon`.
- B: `stage_seam_links_hook_exec_records_and_leaves_unseparable_compound_alone`, `stage_seam_installed_hook_leaves_unseparable_compounds_unrewritten`.
- C, one per reader: `container_seam_remote_context_is_unchanged_for_every_reader`; ipc `projected_local_container_record_is_host_local`; daemon `only_a_negotiated_reply_names_local_container`.

## Lane tests (delivered)

- A, `lifecycle.rs`: `close_request_hides_main_and_same_daemon_serves_bridge_and_cli`. Existing, `registered_commands`: `registered_opener_accepts_exactly_the_three_enum_destinations`. About: `apps/desktop/tests/credits.test.ts`.
- B, `closing_exec/stages.rs`: `mixed_bun_stages_via_hook`, `mixed_make_stages_via_hook`. `closing_shell/stages.rs`: `mixed_bun_stages_via_exec`, `mixed_make_stages_via_exec`, `mixed_failing_test_stage_skips_gh_stage`, `mixed_unseparable_compounds_pass_through`, `mixed_classify_executes_nothing`, `mixed_stages_run_once_without_a_daemon`.
- C, `closing_locality/local_container.rs`: `local_container_buildx_via_exec`, `local_container_buildx_via_hook_exec`, `local_container_docker_build_via_exec`, `local_container_docker_build_via_hook_exec`, `local_container_other_endpoints_unchanged`, `local_container_projection_for_earlier_peers`, `local_container_classify_contacts_no_socket`.
- Task-7: `registered_traffic_diagnostics_return_the_local_container_projection`; Bun "local container build diagnostics are read as the HOST_LOCAL projection and LOCAL_CONTAINER is rejected".

QUEUED notice: a waiting stage's or build's wrapper writes `QUEUED`, `Waiting to start.`, `Reference: <its job id>` once to its own stderr (the vendor shell's on the hook route); the gh stage writes none. Stream comparisons exclude it.

## File ownership

One writer per file per wave. Wave 2: lanes. Wave 3: integration (any file except `qa.md`) and QA (`qa.md` only, once).

- **Lane A:** `apps/desktop/**`.
- **Lane B:** `arc-cli/src/hooks/stages.rs`, `src/exec/stages.rs`, new modules under them; `capture_integration/closing_shell/{stages,actual}.rs`, `closing_exec/stages.rs`; `docs/managed-exec.md`.
- **Lane C:** `arc-detector/src/compute_location.rs`, new modules under it; `arc-cli/src/classify.rs`, `src/classify/**`; `arc-cli/src/hooks/container.rs` (`literal_argv`, `rewrite`); `capture_integration/closing_locality/{local_container,docker_controls}.rs`.
- **Shared, read-only for lanes:** everything else, including manifests, `Cargo.lock` and mod parents (`closing_*.rs`, `capture_integration.rs`, `hooks.rs`, `exec.rs`, `child.rs`).
- Integration owns `closing.md` and `traceability.md`.

## Rules

Offline fixtures; a built binary gets a cleared environment, temporary HOME, explicit paths and `LLVM_PROFILE_FILE`; no correctness from elapsed time; 60 s child watchdogs; no test binary over 60 s, which `managed_exec` does not meet (112 s for its 163 baseline tests before this mission, 134.79 s in the runner after the lanes); nothing ignored; a passing test removes its fixture directory; files at most 500 lines. Actors run only `cargo test -p`; never the workspace tests, coverage, the harness, SwiftPM-backed tests or screen checks. No new Bun file.

## Runner commands

Each once; timeout 7,200,000 ms.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
bun test apps/desktop/tests/bridge-client.test.ts apps/desktop/tests/contracts.test.ts apps/desktop/tests/credits.test.ts apps/desktop/tests/dashboard.test.ts apps/desktop/tests/governor-contract.test.ts apps/desktop/tests/governor.test.ts apps/desktop/tests/lifecycle.test.ts apps/desktop/tests/meter.test.ts apps/desktop/tests/traffic-scripts-contract.test.ts apps/desktop/tests/traffic-scripts.test.ts scripts/coverage/accounting.test.ts scripts/coverage/desktop-foundation.test.ts scripts/coverage/evidence.test.ts scripts/coverage/gate.test.ts scripts/coverage/hygiene.test.ts scripts/coverage/terminal-outcomes.test.ts scripts/coverage/traffic-accounting.test.ts scripts/harness/coverage-reuse.test.ts scripts/harness/equivalence.test.ts scripts/harness/traffic-targets.test.ts scripts/requirements/traceability-parser.test.ts scripts/requirements/traceability.test.ts
```
