# Independent QA — endpoint capture

**AT-TRF-02 production-path gap:** `crates/arc-daemon/tests/managed_exec/remote_ci.rs:190-245` tests hook pass-through followed by separate simulated classification and stub execution. `capture_git.rs:349-385` tests direct `arcctl exec gh workflow run build.yml`: REMOTE_CI, once, no new queue entry or reservation with a held Build permit. Neither links agent execution. AT-TRF-02 remains Partial; Chief/integration owns the gap. Live vendor acceptance is not required.

**Verdict: FAIL; completeness: partial.**

## Scope and provenance

Baseline `b8a4da14537adcd44aca68f32ce47557f82f2eef`, tree `555dcfda17be04105c55dead4ad71bd904c5e824`. Reviewed snapshot `07981ab5cefe119e0a97e150062d8ec19015c592`, tree `279c014faf0d27e89b77dde3cd19a35e94db48a3`, has a delivery overlay. Correction inputs: `463a1c4c7b77f3b60bd683fdaeed9c32ea910215`, tree `3453a509e57e4f942ea7c0b30faba36fc7c046f9`; and `824eccf0e1a2dc421a03629224977da4996b5738`, tree `175ef324a371ea24ed9bcc3b3b61f2a4ba30e4b8`. Concurrent task-6 changes and final verification remain unreviewed. Snapshot ref resolves in parent; worker ref unavailable.

Reviewed contract and four lanes. Task 2–5 snapshot HEADs: `5cfe934fd141e2cf4a72b6d5f6f5869088dc86d0`, `f9e21b81f738023ba5950704b5bd325896378376`, `5098ed63c507ac6d7852d40f18acea2e075b15fe`, `645ead5436dc3c8059c2bac4a186f776f704c248`. Four patch hashes and 15 receipt entries matched, including a deletion. Task-4/5 delivery trees were unavailable; receipt blobs and snapshot diffs established repairs.

## Acceptance gaps and owners

**HIGH — deadline, Chief/shared-route owner.** `crates/arc-cli/src/exec/run.rs:26-47` starts a 1.8 s deadline (`preparation.rs:13-15`) before classification; `endpoint_context.rs:54-71` permits 5 s capture. If capture exceeds 1.8 s while fresh, `arc-ipc/src/client.rs:133-138` skips connection, even to a responsive daemon. This is static, not a measured delay or runtime reproduction. Rebalance capture/IPC budgets while preserving freshness; use an injected clock to test delayed registration, classification of the same child, exactly-once execution and genuine-outage fail-open.

**HIGH — isolation, endpoint/task-2; Chief coordinates.** SSH and Docker/Buildx fixtures (`crates/arc-daemon/tests/managed_exec/capture_endpoints.rs:54-86,217-255`) compare host interfaces (`endpoint_context/interfaces.rs:17-68`) with fixture targets (`arc-detector/src/remote_endpoints.rs:72-93`). Separately, `managed_exec/remote_endpoints.rs:438-461` launches SSH, Docker and kubectl through `cli.rs:25-28` without clearing environment or setting HOME. Capture can consult account/system SSH config (`endpoint_context/ssh.rs:76-94`), `DOCKER_CONFIG`/HOME Docker config (`docker.rs:138-146`) and `KUBECONFIG`/HOME kubeconfig (`kube.rs:87-102`); host-file reads were not observed. Isolation acceptance is unmet. Inject deterministic interfaces; use `env_clear()` and restore fixture socket, PATH, HOME, markers and Docker/Buildx/kube paths; pass SSH `-F <fixture>`. Rerun both routes and held-permit fixtures before runner verification.

1. **Both routes — integration/task-6.** `crates/arc-daemon/tests/managed_exec/capture_endpoints.rs:448-486` has 27 exec cases but only 14 UNKNOWN Docker/Kubernetes classify controls. Positive SSH, Docker and Buildx classify/precedence lack real-daemon proof. `capture_git/system.rs:19-57` describes a temporary 64-case comparison; `capture_integration.rs:1` is a placeholder. Add isolated proven, precedence, missing and ambiguous cases on both routes, including named bare pushes.
2. **AT-TRF-04 — integration/task-6.** `capture_endpoints/held.rs:75-91` asserts remote classification, child and unchanged scheduling under a held permit, but not separate local CLI consumption (`docs/requirements/ARC_SRS_2026-11-06.md:2648-2655`). Demonstrate it or leave the row incomplete; broken telemetry is not established.
3. **Runner/Chief.** Final-input verification, independent workspace run, >=80% per-crate coverage including Cursor, and final harness remain unobserved. DET-001 needs broader Bash/zsh proof; DET-005, SAFE-008 and NFR-007 need full remote/official-trigger scope.

## Evidence and review

- Shared capture: `exec/capture.rs:2-26`; freshness/size bounds: `arc-detector/src/locality.rs:13-16,113-157`. Unproved, stale or unsupported facts yield UNKNOWN/OBSERVE. `endpoint_context/kube.rs:147-154` emits API facts, not placement. Git push URL/rewrite precedence: `ci_context/git.rs:188-255`; real Git preflight is separate from offline stubs.
- `crates/arc-cli/src/classify.rs:54-103,147-188` preserves protocol-1 fallback and validates confident replies against fresh local classification. Lost-ack and old peers are tested (`exec/lifecycle/contract_tests.rs:37-136`); text/JSON retain four fields. UNKNOWN is not an execution receipt.
- **Simulated:** CI01–CI20 + EP01–EP22: 42 cases; focused runs found 42 remote, 0 UNKNOWN and 0 false-local per shell and argv route. EP20–EP22 inject pod placement; neither discovery nor full official-trigger coverage is proven.
- **Built CLI/daemon/offline fixtures:** endpoint exec 27: 10 remote, 17 UNKNOWN, 0 false-local; classify has 14 UNKNOWN. Named Git/gh exec 32: 16 remote, 16 UNKNOWN; system-selection exec 64: 28 remote, 36 UNKNOWN; GH transport 39: 9 remote, 30 UNKNOWN; 0 HOST_LOCAL each. Sets overlap; held-permit Buildx and gh passed subject to isolation. Stubs cover semantics and fail-open; hook controls retain both rewrites and vendor approval.
- **Test-only lane:** inspected Swift exit/retirement, Go per-case counters, Cursor locks and desktop clock injection. Runner records all five named tests, including `swift_ordered_pairs`, passing, but not Cursor under coverage.
- **Runner:** `2026-11-19T19-53-03-547Z.log` and `target/av-runner/gates/{cargo-test-workspace,bun-tests}.json`: workspace passed (893.785 s), 371 Bun tests in 22 files; input `0bcef080ef6b437e47b317a5ce0570dd976a56a04769adf75e177bb5fc31731e`; log sizes/SHA-256 match. Preliminary architecture validation skipped coverage. Earlier matrix and two admission failures failed both isolated reruns, then passed on changed inputs.

First review lists all blockers. Reject only unresolved earlier findings, defects in lines changed since prior review, or failed runner steps caused by owned files; others are FOLLOW-UP. A full-run failure passing alone rejects only after two consecutive failed checks or both isolated runs fail. A failed final command passing its one rerun is NONDETERMINISTIC; none is available here.

## FOLLOW-UP and limits

- Task-3 reported 28 `ENDPOINT_AMBIGUOUS` exec versus `UNRESOLVED_COMMAND` classify differences in its temporary 64-case comparison (`capture_git/system.rs:27-57`); integration should assess them.
- Without `GIT_CONFIG_NOSYSTEM` or `GIT_CONFIG_SYSTEM`, the compiled system path is unproved and capture returns UNKNOWN (`ci_context/git/reader.rs:21-32`). Default SSH calls `getpwuid_r` before rejecting isolated HOME (`ssh.rs:76-119`); audit that dependency.
- The runner's zero-target observation is for a **different** hard-coded mission (`scripts/harness/traffic-targets.test.ts:138-144`); logs are untrusted. This mission's container configuration, availability, health and recent errors are unverified; Git could not inspect `av.yaml` (`Operation not permitted`).
- Snapshot: **0/6 Complete**. DET-001, DET-005, SAFE-008, NFR-007 and AT-TRF-02 are Partial; AT-TRF-04 Pending (`docs/requirements/traceability.md:99,103,236,255,287,289`).

## QA commands

On `07981ab5`, `git diff --check` and focused `cargo test --offline --locked` passed: CLI classify unit (10), classification integration (7), endpoint route (1); daemon managed-exec endpoint (3), Git (9), CI01–CI20 (1), EP01–EP22 (1). Denominators used `-- --nocapture`. Corrections: static snapshot inspection and report checks only. Skill: `oma-qa`; Serena unavailable. No QA browser, screen, workspace, coverage or harness run.
