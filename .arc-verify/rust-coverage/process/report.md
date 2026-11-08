# Task 4: daemon and CLI coverage

Status: completed for the assigned process-test scope. Overall mission completion remains for integration, the unchanged harness and independent QA.

## Source provenance

Accepted committed baseline: `fbe76a5f15101ad415f355ef06f1deeb1f97d568`; exact tree: `8db02719b5bd06d888729ca337f9bf8887991515`.

Actor-local HEAD and initial snapshot: `6732fb6e33d3e58f208fca0807e3daf0407ffbf6`; input tree: `23e3dd57a4929d7c30e17472121d015b3ea95f83`. These identify the isolated task input, including task-1's tooling overlay. The six changed Rust files are uncommitted Actor-local edits, not changes already present in the accepted baseline. The new resident coverage module is not tracked at either the accepted baseline or initial snapshot. Actual file hashes and sizes are in `source-binding.json`.

The fresh measurement is `2026-11-07T23-55-08-154Z-6f5fd9eb-ae8f-402b-988a-10873cad858a`. Its full input SHA-256 is `9906da2dbf7e051b919b94a40267d8e985a602757ebe6232fde01b76f638947d`. `final-preservation.json` independently checks that current input bytes still match that measurement. `measurement.json` pins this immutable run rather than following a future latest pointer.

## Changes and interfaces

- `crates/arc-cli/tests/schema.rs`: reset accepted sockets to blocking mode before deadline-bounded framing, matching the existing classification fixture. Added the missing-schema-details error case.
- `crates/arc-cli/tests/classification.rs`: reject an error envelope carrying a success payload before mapping its error code.
- `crates/arc-cli/tests/binary.rs`: exercise legacy unknown-command/argument exits and version output with actual binaries.
- `crates/arc-daemon/src/runtime_report_tests.rs`: cover missing report parents, absent file names and embedded NUL names before file creation.
- `crates/arc-daemon/tests/resident.rs`: register one test module.
- `crates/arc-daemon/tests/resident/coverage.rs`: reuse the resident Fixture, Daemon, readiness, exit and CLI helpers and shared process/thread serialization. Exercise missing default database configuration, runtime-report startup rejection, recovery on the same database/endpoint, real status/doctor IPC and SIGINT shutdown. Check each child is reaped using ECHILD and record inherited instrumentation destinations when instrumentation is active.

No production function, protocol, manifest, lockfile, assertion or existing coordination code was changed. No production coverage exclusion, alternate process harness, container, service or browser suite was added. The migration and harness remain byte-identical to accepted HEAD. The preservation checker passed across all 77 existing Rust files and both protected-file hashes. Rustfmt only formatted the assigned edits.

## Measurements

The task-1 historical source-bound measurement reported daemon 1501/1613 (93.06%) and CLI 200/213 (93.90%). Its production-source bytes match this task input; the old report belongs to its recorded workspace and source hash, not this local HEAD. `measured-gaps.json` records both run identities and the exact newly covered lines.

| Crate | Covered/production lines | Coverage |
| --- | --- | --- |
| arc-core | 64/64 | 100.00% |
| arc-detector | 572/578 | 98.96% |
| arc-storage | 214/227 | 94.27% |
| arc-ipc | 178/215 | 82.79% |
| arc-daemon | 1507/1613 | 93.43% |
| arc-cli | 203/213 | 95.31% |

The 80% floor is a new engineering target, not an SRS requirement or an established baseline. Counts use task-1's canonical production-line union; test-only lines are omitted and zero-hit production lines remain.

Newly covered production lines: CLI classify.rs:35, doctor.rs:30 and main.rs:93; daemon main.rs:32 and runtime_report.rs:45,46,49,54,55. Detailed remaining zero-hit lines are in `uncovered.md`; the full run contains every crate's uncovered-line report. No missing production path was hidden to obtain these results.

## Instrumented child evidence

`instrumented/coverage-lifecycle.json` records three real daemon children and two real arcctl children. The inherited LLVM_PROFILE_FILE used `%p-%m`; each PID produced a distinct nonempty raw profile without a test override. Each child was reaped before profile inspection. SIGINT completed successfully; final daemon accounting confirms zero handler/gauge residue, joined database worker, closed connection, removed socket and deadline compliance.

`verify-child-profiles.py` checks all five profile hashes against the gate's fresh raw-profile inventory and their exact paths against the actual llvm-profdata merge input list. It then merges only these five profiles and exports both production binaries. The child-only export hits 1,594 canonical production lines across the linked workspace crates, including daemon and CLI files; every hit is present in the full canonical report. This establishes profile contribution, rather than inferring coverage from passing test counts. `child-profile-proof.json`, `child-only-commands.json`, `child-only.profdata` and `child-only.lcov` preserve the checks and output. Reproduce from this Actor worktree with `python3 .arc-verify/rust-coverage/process/verify-child-profiles.py`; raw inputs remain under the run-specific ignored target directory.

## Commands and outcomes

Receipts and complete logs use the filenames below; commands ran in this isolated worktree.

| Command | Outcome | Receipt |
| --- | --- | --- |
| cargo test -p arc-cli --test schema (before fix) | 101; reproduced WouldBlock at schema.rs:57 in two cases | schema-before.json / .txt |
| cargo fmt --all | 0; formatted assigned edits | final fmt check below confirms result |
| cargo test -p arc-cli --test schema (after fix) | 0; eight cases pass | schema-after.json / .txt |
| cargo test -p arc-cli | 0 | cli-focused.json / .txt |
| cargo test -p arc-daemon --test resident coverage:: -- --nocapture | 0; native lifecycle/reaping evidence | resident-focused.json / .txt |
| cargo test -p arc-daemon --bin arc-daemon runtime_report::tests::invalid_report_names | 0 | unit-focused.json / .txt |
| bun run test:coverage | 0; all six crates pass, 87.12 seconds | coverage.json / .txt; immutable full run |
| python3 .arc-verify/rust-coverage/process/verify-child-profiles.py | 0; profile hashes, merge inclusion and canonical hits pass | child-profile-proof.json; child-only-commands.json |
| cargo fmt --all -- --check | 0 | fmt.json / .txt |
| cargo clippy --workspace --all-targets -- -D warnings | 0 | clippy.json / .txt |
| cargo test --workspace | 0; native schema failure resolved | workspace-tests.json / .txt |
| bun test scripts/coverage/preservation.test.ts scripts/coverage/gate.test.ts scripts/coverage/accounting.test.ts | 0; 27 checks pass | gate-preservation.json / .txt |
| current inputIdentity plus checkPreservation | 0; same measured bytes and accepted preservation hashes | final-preservation.json |

Tools checked by the real gate: Rust/Cargo 1.95.0, cargo-llvm-cov 0.6.21, matching LLVM 22.1.2, Bun 1.4.4. Exact outputs are in the immutable run's versions.json and crate-summary.json. The run's receipt.json includes argv, PID, timestamps and exits for instrumented tests, binary probes, reports and tool checks. `source-binding.json` records task attempt and inherited mission limits; verification was serial, with no new Actors, installations or downloads.

The first child-proof script attempt assumed the serialized OsString was a string and failed its template assertion. The receipt actually preserves native Unix bytes; the corrected analysis decodes those bytes. Product sources and measurement were unaffected. A read-only ps probe was denied with Operation not permitted; it was not bypassed. Initial global Git status also reported av.yaml access denied; scoped status and source/preservation checks succeeded. Serena was not exposed in the available tool inventory; inspection used supplied paths and direct reads, without a repository code-search fallback.

## Acceptance and handoff

- [x] Material deterministic unit/integration/CLI tests cover measured gaps and preserve existing assertions.
- [x] Real child-profile creation, distinct destinations, raw hashes, full-merge inclusion, child-only canonical production hits and reaping are verified.
- [x] Daemon and CLI each exceed 80% in a fresh source-bound shared coverage run.
- [x] No behavior change, production exclusion, duplicate harness, container or GUI dependency.

Chief integration must retain the six Rust edits, this process directory and the generated immutable measurement directory `.arc-verify/rust-coverage/baseline/runs/2026-11-07T23-55-08-154Z-6f5fd9eb-ae8f-402b-988a-10873cad858a`. The shared command also updated baseline/latest.json as designed; no shared tooling was edited manually. Re-run final integrated evidence/acceptance after the library Actor's edits; an Actor-local passing report does not establish coverage for different integrated bytes.

There is no remaining reproducible blocker for this assigned task. The unchanged ./scripts/harness/validate.sh was not run in this process-test task; task-1's earlier timeout remains historical, not a fresh result here. The Chief owns that required final command and independent oma-qa. This report does not approve mission completion or alter the verification command. No injected OMA run/claim identifiers were present, so none were invented and no mission state was changed.

Measured host: aarch64-apple-darwin. Linux inactive branches need their own native measurement; native Windows is unsupported by the current Unix transport. No platform path was excluded for the target. Stable-toolchain doctest coverage is outside the supplied collector; native workspace doctests passed.

- desktop: incomplete
- native Windows: incomplete
- remaining Phase 0: incomplete

Skill used: oma-backend, with the installed backend-engineer definition, execution protocol and checklist. API/auth/database-schema checklist items do not apply to this test-only change. Independent review remains assigned to the separate reviewer.
