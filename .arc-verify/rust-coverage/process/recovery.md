# Task-4 delivery recovery

Status: completed for process-test delivery repair; independent task review remains required. This does not approve mission completion.

The first delivery included the shared `baseline/latest.json` update made by its isolated coverage command. This collided with the library Actor's later pointer. Task-4 now delivers only its six Rust edits, process artifacts, and the explicitly retained immutable measurement directory. The current shared pointer was never replaced, restored, staged or selected by this repair. Task-6 owns serial integrated verification and pointer updates.

## Reconciled inputs and preserved work

Accepted baseline: `fbe76a5f15101ad415f355ef06f1deeb1f97d568`.

The retained original workspace is `.agent-valley/parallel/workspaces/daa81f71b962cfea-8f9ba36d8e59dd86-1`; its input snapshot is `6732fb6e33d3e58f208fca0807e3daf0407ffbf6`. Only its latest pointer was restored to that snapshot's bytes, SHA-256 `6d5ee0c4dbe567133cbfa625243d0e03b3d8eea8b2669f8de9807851fe3a98b3`. A scoped Git comparison confirms no pointer delta in that retained workspace.

The assigned worktree already contained accepted task-3 library tests and task-5 documentation. Its actual pre-recovery input tree was captured as `670e7d793ff12ac83476680a39fa7496243d6d4b` using a temporary Git index. Existing shared index entries were preserved. Each existing target Rust file matched its retained input version before copying, and the new resident module was absent. All six recovered files now match their retained measured hashes and sizes. No patch was replayed over peer changes. The recovery checks preserve 213 peer files and 161 existing baseline-run files byte-for-byte.

The assigned pointer remains the input pointer for run `2026-11-07T23-59-16-087Z-c03595ea-3a4f-4616-abe8-82f405ed9781`, whose recorded pass is false. Its unchanged worktree SHA-256 is `5f0233ab2a4c06577965d7959770a6299b697dd27d3aa182e2524bbe8fcd280c`. Its preexisting index bytes are also unchanged. The delivery-tree comparison and patch explicitly exclude this pointer, rather than choosing a passing historical run.

## Retained measurement and child evidence

`measurement.json` remains pinned to run `2026-11-07T23-55-08-154Z-6f5fd9eb-ae8f-402b-988a-10873cad858a`, source hash `9906da2dbf7e051b919b94a40267d8e985a602757ebe6232fde01b76f638947d`.

The complete append-only deliverable is `.arc-verify/rust-coverage/baseline/runs/2026-11-07T23-55-08-154Z-6f5fd9eb-ae8f-402b-988a-10873cad858a/`. All 22 files retain their original bytes, paths, commands, workspace paths and source identity. `report.md`, the original inventories, command outcomes and child exports remain unchanged. `recovery-artifact-inventory.json` extends delivery accounting; it does not rewrite the historical command outcomes.

Every one of the retained measurement's 110 source/configuration input hashes still matches the original workspace. A fresh call to the existing Bun inputIdentity and checkPreservation functions confirms the same source hash and accepted protected hashes. Recovery did not change any measured source or configuration in that workspace; its same-source command receipts are reused. The current assigned input additionally contains four accepted library coverage modules and a test-module declaration in detector/lib.rs. Therefore this measurement remains isolated evidence, not a measurement of the combined integrated source. Task-6 must collect that measurement.

The read-only recovery check validates all 842 nonempty raw profiles against recorded hashes and the actual merge input list. All five lifecycle child profiles have distinct PID destinations inherited from `%p-%m`, were reaped, and appear in that full merge. Retained child-only export contains 1,594 covered canonical production lines, all included in the canonical full report. Canonical path resolution merges tests/../src aliases; no binary/test-instance averaging is used. Full LCOV covered counts are recomputed per canonical production file and matched against the retained summary.

| Isolated measured crate | Covered / total | Percent |
| --- | --- | --- |
| arc-core | 64 / 64 | 100.00% |
| arc-detector | 572 / 578 | 98.96% |
| arc-storage | 214 / 227 | 94.27% |
| arc-ipc | 178 / 215 | 82.79% |
| arc-daemon | 1507 / 1613 | 93.43% |
| arc-cli | 203 / 213 | 95.31% |

All six meet the new 80% engineering target on the recorded macOS arm64 target. This is not an SRS threshold. Per-file remaining gaps are in `uncovered.md` and the immutable run's uncovered.md and summary.json. Inactive Linux branches and native Windows are not certified by this host measurement.

## Concrete checks

`python3 .arc-verify/rust-coverage/process/recovery-checks.py` exits 0 after source, immutable-artifact, protected-byte, peer, pointer, profile and coverage checks plus the following serial commands. Full argv, cwd, PID, times, exits and output hashes are in `recovery-commands.json`; corresponding `recovery-*.txt` files preserve logs.

| Command | Result |
| --- | --- |
| bun -e existing inputIdentity and checkPreservation, in original workspace | 0; original identity exactly matches immutable inputs.json |
| cargo test -p arc-cli | 0; real binary, classification and schema suites pass |
| cargo test -p arc-daemon --test resident coverage:: -- --nocapture | 0; startup failure, admission recovery, real status/doctor IPC, SIGINT and reaping pass |
| cargo test -p arc-daemon --bin arc-daemon runtime_report::tests::invalid_report_names | 0; invalid-path unit case passes |
| cargo fmt --all -- --check | 0 |
| bun test scripts/coverage/preservation.test.ts scripts/coverage/gate.test.ts scripts/coverage/accounting.test.ts | 0; 27 pass, zero failures |

Historical same-source receipts additionally record successful `bun run test:coverage` (87.12 seconds), `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`. These were not relabeled as new integrated executions. The original schema RED receipt records WouldBlock before the fixture's blocking-socket repair; its GREEN receipt remains preserved. Existing assertions, production behavior, all manifests/lockfiles, migration `0001_phase0.sql` and `scripts/harness/validate.sh` are unchanged by this delivery.

The first new recovery-check attempt failed when its own LCOV comparison used lexical paths for repeated production-module instances (`admission.rs`). The check was corrected to resolve canonical paths, matching the existing accounting interface. `recovery-failed-attempts.json` records the failure and correction. No gate code, pin, exclusion, threshold, assertion, timeout or verification command was changed. Serena was not available in the tool inventory; supplied paths and direct reads were used without a repository code-search fallback. Initial global Git status reported `av.yaml: Operation not permitted`; it was not bypassed. Scoped Git checks succeeded.

## Delivery and handoff

The six Rust paths are listed with sizes and hashes in `source-binding.json` and `recovery-reconciliation.json`. Their tests add CLI usage/version and invalid-envelope errors, schema-detail errors, runtime-report invalid-name cases, and resident startup/recovery/SIGINT coverage using the established fixtures and shared coordination. The schema socket change is test-fixture-only. No production interface changes, duplicate harness, external services, containers or GUI checks were added.

The supported binary Git patch and full tree/hash verification are produced under `.agents/results/process-task-4-recovery.patch` and `.agents/results/process-task-4-delivery.json`. A temporary input tree and delivery tree make the delta explicit. Every owned delivered file is checked against actual index/tree blobs and a clean patch-application result. The resident module must have its full nonempty 5,572-byte body, rather than an intent-to-add marker. The mutable pointer must be absent from the delta. Shared manifests, pins, tooling and registries remain outside task-4 ownership.

- [x] Six measured-gap test edits recovered with preserved assertions and production behavior.
- [x] Real instrumented child creation, distinct inherited profiles, full-merge inclusion and reaping established by retained and freshly checked evidence.
- [x] Isolated daemon and CLI each exceed 80%; exact original source binding revalidated.
- [x] Complete immutable run retained append-only and delivery pointer collision removed.
- [x] Accepted library tests, documentation, prior baseline runs and current pointer preserved.

Framework independent task review is the next step. Task-6 must measure the combined source and execute all five exact required commands, including standalone coverage and the unchanged full harness, with zero violations and no skip flags. Task-7 independent oma-qa and final executable verification remain required. This task does not modify the mission graph, instructions, assignments, acceptance criteria or spent budgets. No new Actors, installations, browser suites, external actions or mission-state edits were used.

- desktop: incomplete
- native Windows: incomplete
- remaining Phase 0: incomplete

Skill used: oma-backend with the installed backend-engineer definition, execution policy, execution protocol and checklist. API/auth/database-schema checklist entries do not apply to this test-only delivery repair. No OMA run or claim identity was injected; none was invented.
