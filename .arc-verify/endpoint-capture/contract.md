# Endpoint capture contract

Wave 1 proves transport only; capture is environment-only and Git discovery stubbed. Facts are synthetic.

## Interfaces

- `arc_detector::capture_shell_argv(&ClassifyRequest) -> Option<Vec<Vec<OsString>>>` parses bounded literal shells; `None` is uncertainty.
- `exec::capture_argv(&[OsString]) -> Option<EndpointContext>` calls `endpoint_context::capture(&[OsString]) -> Option<EndpointContext>`, then `ci_context::extend` and `endpoint_context::finish`, each `(&[OsString], &mut EndpointContext) -> Option<()>`; finish binds SSH after Git. `exec::capture_shell(&ClassifyRequest) -> Option<EndpointContext>` merges facts, rejects conflicts/unproved leaves and keeps the earliest timestamp. Both enforce five-second/8 KiB/32-entry bounds. Use caller cwd/environment; no tool execution, network, raw logging or persistence.
- Explicit Git destinations use `EndpointKind::GitRemote` and the literal selector; bare push uses `GitPushDefault` and `@bare-push`. See [git-config](https://git-scm.com/docs/git-config) and [git-push](https://git-scm.com/docs/git-push).
- Classify uses `capture_shell` and `job.classify.locality.v1`, conservatively falling back for old protocol-1 peers. Preserve `job.classify` and four output fields.
- Nonlocal/unknown `is_endpoint_locality()` reasons use `job.start.locality.v1`. Fresh-proven gh results (`GH_WORKFLOW_RUN`/`GH_RUN_WATCH`/`GH_RUN_VIEW`) and SSH (`SSH_REMOTE_COMMAND`) retain their reasons and use protocol-1 `job.start`; raw facts never cross registration. Missing/conflicting facts yield `ENDPOINT_AMBIGUOUS`; stale/failed capture yields legacy `UNKNOWN/OBSERVE`. Only authenticated unsupported before a possible commit permits contextual fallback to `job.start` with `UNKNOWN/OBSERVE` (`UNRESOLVED_COMMAND`); lost ACK cannot. `job.status.locality.v1` exposes storage; legacy replies project non-legacy reasons.
- `LOCAL_ENDPOINT_BUILD` uses `traffic.endpoint.capabilities`/`traffic.endpoint.job` and Governor Build policy. Scheduled endpoint children inherit the caller's environment; lineage uses the sidecar. Unsupported peers get a legacy projection; remote/unknown bypass admission. Kubernetes API endpoints do not prove pod placement. No cache, refresh or revalidation service.
- Preserve arguments, cwd, environment, streams, exit status, fail-open, hook rewrites and vendor approval.

## Dependencies

Docker uses `serde_json`; `serde_norway` 0.9.42 parses kubeconfig/gh YAML; `gix-config` 0.62.2 handles Git quoting, continuation and multivalues without running Git. SHA-1/SHA-256 enabled; 35 packages added. Bound reads; reject unsupported includes, YAML aliases/merge/tags/multiple documents, conflicts and unsupported SSH forms. No implicit file/network access. Wave 2 does not edit manifests/lockfiles.

## Ownership

One writer per file per wave; other files need Chief coordination, subject to runner-failure repair.

1. **Shared:** `Cargo.lock`, `crates/arc-cli/Cargo.toml`, `crates/arc-cli/src/exec.rs`; under `crates/arc-cli/src/exec/`: `capture.rs`, `entry.rs`, `metadata.rs`, `lifecycle.rs`, `lifecycle/contract_tests.rs`, `preparation.rs`, `run.rs`, `governor.rs`, `traffic.rs`, `transport.rs`; `crates/arc-cli/tests/fixtures/child.rs`; under `crates/arc-detector/src/`: `lib.rs`, `locality.rs`; under `crates/arc-ipc/src/`: `jobs.rs`, `traffic.rs`, `traffic/endpoint.rs`; under `crates/arc-daemon/`: `src/connection.rs`, `src/db_worker/jobs.rs`, `src/governor_dispatch.rs`, `tests/managed_exec.rs`, `tests/managed_exec/shared_execution_contract.rs`; `crates/arc-traffic/src/lib.rs`; this contract.
2. **Endpoint**, under `crates/`: `arc-cli/src/exec/endpoint_context.rs` and `endpoint_context/**`; `arc-detector/src/remote_endpoints.rs` and `remote_endpoints/**`; `arc-daemon/tests/managed_exec/capture_endpoints.rs` and `capture_endpoints/**`.
3. **Git/CI**, under `crates/`: `arc-cli/src/exec/ci_context.rs` and `ci_context/**`; `arc-detector/src/remote_ci.rs` and `remote_ci/**`; `arc-daemon/tests/managed_exec/capture_git.rs` and `capture_git/**`.
4. **Classify:** `crates/arc-cli/src/classify.rs`, `src/classify/**`, `tests/classification.rs`, `tests/classification/**`.
5. **Test-only:** `crates/arc-daemon/tests/traffic_native_tools.rs` and `traffic_native_tools/{mod.rs,scenarios.rs,witnesses.rs,swift.rs,swift/**,go/owned.rs,go/owned_tests.rs}`; `apps/desktop/src-tauri/tests/memory_pressure/{native_launch_races,native_sequence,native_support}.rs`; `crates/arc-cli/tests/agent_hooks_cursor.rs` and `crates/arc-cli/tests/fixtures/agent_hooks/cursor/{installer_concurrency,process}.rs`. No requirement row.
6. **Integration:** `crates/arc-daemon/tests/managed_exec/capture_integration.rs` and `capture_integration/**`; `docs/requirements/traceability.md`, `docs/managed-exec.md`, `README.md` when affected. It may edit lane-owned files only for runner-reported failures and updates the six rows and actual counts.
7. **Independent QA:** `.arc-verify/endpoint-capture/qa.md` alone, one report for the reviewed snapshot; put an agent production-path gap first if present. Corrections after review are permitted; no refresh for later deliveries.

Wave 2: four ready tasks, at most three Actors; integration and QA follow in parallel. Named tests use observed state or injected clocks, without longer timeouts, retry loops or suite serialization. Child watchdogs are at least 60 seconds. The fixture lock reflects one daemon per user.

## Reviews and runner

First review lists all blockers. Later rejection requires unresolved earlier findings, defects in lines changed since that task's previous review, or failed runner steps caused by owned files; other findings are FOLLOW-UP. A full-run failure passing alone is recorded and blocks only after two consecutive failed checks or both isolated runs fail. Runner executes the workspace once per integration check; final verification supplies the independent run. Rerun a failed final command once; a pass is `NONDETERMINISTIC`. Runner owns desktop, >=80% per-crate coverage and harness. Timeout/check: 7,200,000 ms. Actors do not run workspace tests, coverage, harness, SwiftPM-backed tests, browser E2E or screens.

Runner commands:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
bun test apps/desktop/tests/bridge-client.test.ts apps/desktop/tests/contracts.test.ts apps/desktop/tests/credits.test.ts apps/desktop/tests/dashboard.test.ts apps/desktop/tests/governor-contract.test.ts apps/desktop/tests/governor.test.ts apps/desktop/tests/lifecycle.test.ts apps/desktop/tests/meter.test.ts apps/desktop/tests/traffic-scripts-contract.test.ts apps/desktop/tests/traffic-scripts.test.ts scripts/coverage/accounting.test.ts scripts/coverage/desktop-foundation.test.ts scripts/coverage/evidence.test.ts scripts/coverage/gate.test.ts scripts/coverage/hygiene.test.ts scripts/coverage/terminal-outcomes.test.ts scripts/coverage/traffic-accounting.test.ts scripts/harness/coverage-reuse.test.ts scripts/harness/equivalence.test.ts scripts/harness/traffic-targets.test.ts scripts/requirements/traceability-parser.test.ts scripts/requirements/traceability.test.ts
```

## Focused evidence

Current offline/locked: fixture build; daemon `shared_execution_contract::` (4); CLI `exec::lifecycle::contract_tests` (2), `exec::native_environment::tests` (3); scoped Clippy and rustfmt. Local regression failed on added lineage, then passed for absent, empty and supplied values with real-daemon permit and lifecycle. Synthetic gh/SSH tests cover protocol and missing/conflicting/stale facts. Prior attempt (not rerun): metadata (11), capture (1), endpoint compatibility (3), detector capture (1), CLI Clippy and CI01–CI20/EP01–EP22 (42 remote, 0 UNKNOWN, 0 false-local per shell/argv route; injected pod placement). Real discovery, built-`arcctl` integration, linked gh and agent hooks remain downstream.
