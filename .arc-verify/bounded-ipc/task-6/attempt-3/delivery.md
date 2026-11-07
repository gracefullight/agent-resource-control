# Task-6: bounded CLI documentation handoff

Status: assigned documentation completed; ready for Chief collection and independent review. Skills used: oma-docs and oma-design (local contract route; no visual-system change).

## Changes and acceptance

- [x] Guidance matches the integrated CLI, fixed caps and absolute deadlines. Updated `docs/runtime-identity-usage.md` and `docs/bounded-ipc-recovery.md`; successful output, optional doctor, plain errors and direct signals remain intact.
- [x] Liveness, integrity and complete transport saturation are distinguished. Status/hello bypass SQLite but have no reserved connection/request slots. Decoded DB_BUSY maps to IPC_ERROR only if its bounded response write succeeds; failed exchanges map to DAEMON_UNAVAILABLE. Four seconds is a suggested pause based on processing budgets, not a guarantee that persistent connections have expired.
- [x] Material design evidence identifies actual source and registered tests. Retained fail-fast behavior; retry and progress controls remain untested alternatives. Personas and usability benefits are assumptions. No participant, screen-reader, screen, retention, conversion or user-reaction result is claimed.

The source table in the recovery document now includes actual native CLI saturation/recovery, controlled slow-worker/SQL, both signals, transport units and identity/output assertions. Four missing evidence links from the resumed input were replaced. Historical runtime-identity documentation remains labeled historical.

## Provenance and compatibility

Accepted committed baseline: 42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0; tree c802f71ac4d91ed6b8a602d5c83bf9cf57116b76. Actual inspected Actor snapshot: eed6986a06f66dcfd09eb2d05f04a3b5ca9e5206; tree 87d951cecbcdf7236abf9af5da192e5e897096c0. Source-evidence.json preserves the supplied sourceEvidence record, including its attempt=2, separately from this task's attempt=3. The supplied snapshot/worker ref names each returned exit 128 locally; no other ref or HEAD was substituted. This is Actor-local evidence, not proof of integration into the review checkout or of committed baseline behavior.

Boundary.json checks 1,855 readable tracked paths: only the two owned documents changed. av.yaml was denied and remains excluded/unverified. No source, tests, dependency manifests, lockfile, frozen migration, harness, shared registry, Actor definitions, mission state, commit or ref was changed. CLI/protocol/configuration/ARC_RUNTIME_REPORT interfaces are compatible. There is no requested ownership handoff or implementation conflict.

## Concrete checks

| Command/check | Result |
| --- | --- |
| cargo test --workspace | Exit 0; 173 passed in 17 suites, 0 failed/ignored/measured/filtered; 80.46 seconds, Darwin arm64. Actual argv is unchanged. CARGO_NET_OFFLINE=true and worktree-local CARGO_TARGET_DIR are recorded in workspace-test.json. |
| Source hashes/modes before and after testing | All 50 product files identical. Both manifests and full-log SHA-256 are bound in workspace-test.json. |
| git diff --check -- docs/runtime-identity-usage.md docs/bounded-ipc-recovery.md | Exit 0. |
| Local link/heading resolution | 41 local references resolve; broken count fell from 4 to 0. references-before/after.json retain every target. |
| Accepted-baseline byte comparison | Migration SHA-256 046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384 and harness SHA-256 93140173df8b8bd28b067aff68301134b0398b643240b0b1e6a13da4487ebe33 match git show of the accepted baseline. |
| oma docs verify --help | Exit 0. Verify/sync scans not run because they regenerate the shared docs/generated/doc-refs.json registry without an ownership handoff. Manual reference inspection is explicitly labeled. |
| python3 .arc-verify/bounded-ipc/task-6/attempt-3/check.py finish | Final exit 0; scoped ownership, diff and local-reference assertions pass unchanged. |

The workspace suite includes actual CLI busy mapping, full transport saturation followed by status/doctor recovery, health with controlled slow DB work, non-reading-peer closure, SIGTERM/SIGINT accounting/reopen checks and 100,000-attempt fixtures. A passing suite does not replace the test/review owners' raw load accounting or native performance/SRS report. No measured performance number is inferred from structural caps or test runtime.

## Failed attempts and remaining limits

Initial evidence-helper attempts failed on an incorrect root, a tracked directory symlink and denied av.yaml access. Final link checking initially found its two not-yet-emitted output files; it passed after those actual files were emitted, with unchanged assertions. initial-check-failure.txt and initial-* JSON retain these failures; no product/test weakening occurred. Initial unscoped Git status also reported av.yaml denial. Mise tracking-symlink warnings did not prevent tests. Serena tools were unavailable; only supplied known-path source reads and scoped Git/hash checks were used. No restriction was bypassed.

The unchanged integration harness receipt remains exit 1 for oversized resident.rs, oversized IPC library and missing test:coverage. This task does not rerun or clear those findings, fmt/Clippy/final QA, independent review, performance/SRS comparison or mission-wide acceptance. Phase 0 features, Linux qualification and Windows runtime/Named Pipe/tray work remain open. av.yaml denial leaves configured external-target applicability unverified. No service health, credential or external-effect evidence was invented.

No OMA run ID or claim path was injected; none was invented, and no mission registration or resumption command was invoked. Task-specific evidence is in this directory; a supplemental result report is under .agents/results/. The test-created target cache remains in the Actor worktree, and Cargo exit was observed. No deployment, download, install, browser/GUI use or standalone build command occurred. Chief must verify delivered hashes in the integrated worktree; postCollectionVerified remains false here.
