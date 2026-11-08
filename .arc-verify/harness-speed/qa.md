# Independent harness QA — task-4

## Review Result: PASS

No reproducible CRITICAL, HIGH, MEDIUM or LOW findings. All assigned command gates passed. This is the independent review deliverable; final integration and Chief acceptance remain with tasks 5–6.

Skill used: `oma-qa`, including its execution protocol and checklist. Scope: shell correctness, command safety, performance, fixture coverage, coverage accounting, preservation amendments and evidence hygiene. This CLI change has no browser/UI surface; no browser, GUI, installation, external delivery, delegated task, product repair or mission-state change was performed.

## Source bindings and existing changes

Accepted HEAD: `f85ae0430038cc3d7a6125f650872dbfce6b5529`, tree `b7e6c535d7e0b963e6af9bc15d7645ad0f7cded2`. Oracle SHA-256: `93140173df8b8bd28b067aff68301134b0398b643240b0b1e6a13da4487ebe33`. Candidate SHA-256: `0e66d222504f469cc9be910e1c7a6e33544440f017809e25cc255f6dc90f29a8`. HEAD still resolves to the accepted commit; implementation is staged. Index tree: `f25af4f19afa14a9208ad109795490a6cf0cfaf1`.

The existing twelve implementation/test changes and 908 staged coverage-artifact removals were inspected before execution. Current working/index source bytes agree. A before/after inventory confirms all 126 selected source entries retained their bytes and modes throughout this review; no product/test/index change was made by this Actor. Task-1 contract and task-2 checklist were read, and preceding report bytes remain unchanged. The suites refreshed their declared equivalence, scaling, timing and coverage-verification receipts; earlier static reports and reviews were preserved.

Serena discovery returned no tools. Inspection used known-path reads, source diffs and immutable Git objects without replacement repository source search. A `ps` process inventory was denied, so only this reviewer's serial scheduling is established; host-wide process absence is not claimed. The optional YAML config was absent; the configured CUE file was read instead. These were inspection limits, not failed acceptance commands.

## Behavioral review

- All 79 deterministic isolated fixtures match original stdout bytes, stderr bytes, exits and coverage calls. Cases include clean/empty input; each secret, shell and architecture family; layer and SDK variants; file/rule/line ordering and overlapping matches; force-push trigger/report differences and cross-file isolation; exclusions with positive controls; missing/unreadable files; staged selection and working contents; no-Bun, coverage success/failure/skip behavior; LF boundaries, binary notices, symlinks, filename framing and option-like paths.
- The force-push OR triggers remain distinct from the broader reporting expression. Only the first architecture rule applies the within-domain filter. Replay is file-first, then rule, then matching line; counting stays once per nonempty file/rule.
- Native grep tracing reproduced 38 versus 20 invocations for 2 files and 1,520 versus 20 for 80 files. Ordinary paths are batched with `xargs`; option-like paths retain the accepted native interpretation. The measured implementation uses at most three concurrent secret-pattern batches, then ordered parent replay. It adds no runtime dependency and ran with Bash 3.2/BSD grep.
- Independent byte assertions confirm the secret-pattern block, layer variable and every notification call retain their accepted values/order; Check 4 through final exit is byte-identical. The exact scan eligibility and unchanged expressions were checked against the accepted diff and task-1 contract.
- The integrated unskipped harness scanned its indexed additions and passed with zero violations. New indexed source paths are listed in `qa-audit.json`.
- An additional `awk -v` scratch-path concern was tested rather than reported as a defect. Four controlled TMPDIR cases (ordinary, spaces, literal backslash-t and backslash-n) retained identical streams/exits, coverage calls and three expected violations. No defect was reproduced; see `qa-edgecases.json` and the retained runnable probe.

## Checks 1–4, Check 5 and total

Fresh independent benchmark: one original then three candidate full runs, each with real coverage and no skip flag. All use the unchanged 4,044-entry accepted corpus (285,669,775 blob bytes). Raw Git manifest SHA-256: `cc9e627ec4cc9f2ab8fa41b8be66e6ca285f64cf48729600d7aac587da955344`; path/mode/content manifest SHA-256: `5a4cc80b455f45901c8c70825972150937f2d6c22402f706562247a1b72733b3`. The candidate executes outside the accepted corpus, leaving its tracked original harness unchanged.

| Run | Checks 1–4 s | Start → Check 5 s | Check 5 s | Total s | Exit |
|---|---:|---:|---:|---:|---:|
| original | 163.421057 | 164.162468 | 83.568649 | 247.733245 | 0 |
| candidate-1 | 14.616738 | 15.349835 | 82.110619 | 97.467000 | 0 |
| candidate-2 | 14.657121 | 15.172532 | 81.838186 | 97.016641 | 0 |
| candidate-3 | 14.675457 | 15.185321 | 81.483307 | 96.674869 | 0 |

All three candidate scans and the stricter process-start-to-Check-5 measurements are below 20 seconds. Timers use an external monotonic stdout-marker observer. Checks 1–4 spans the Check 1 to Check 5 labels; startup is separately recorded and included in the stricter prefix. Check 5 spans its label through `Coverage thresholds met`, including the boundary corpus audit and output delivery. Finalization and stream draining are separately recorded. These are observed phase intervals, not an instrumented inner collector duration.

Before-scan and Check-5-boundary corpus/dereferenced-input audits agree in every run. Post-run changes are confined to the source-bound collector's explained latest-pointer write. All runs exited 0 and passed six real crate gates. Exact commands, environment/tool/host identities, hashes, phase markers and retained scratch locations are in `timings.json`, bound to the complete `timing.json` receipt. No competing benchmark or coverage collection was started by this reviewer.

## Verification completeness

Complete for the assigned executable gates. Commands ran serially in the requested order, with the native Cargo commands executed directly. Latest mission/operator allowance is 3,600,000 ms per check; the 120,000 ms contract values were not used to terminate checks. Existing test/collector internal limits were left unchanged. No command was interrupted or timed out. The first equivalence command duration is the Bun-reported suite time; later receipts use a monotonic command interval.

| Command | Exit | Seconds |
|---|---:|---:|
| `bun test scripts/harness/equivalence.test.ts` | 0 | 120.210 |
| `bun test scripts/harness/performance.test.ts` | 0 | 555.672 |
| `bun test scripts/harness/coverage-verification.test.ts` | 0 | 16.088 |
| `bun test scripts/coverage/coverage-cli.test.ts` | 0 | 84.443 |
| `bun test scripts/coverage/clean-checkout.test.ts` | 0 | 86.923 |
| `cargo fmt --all -- --check` | 0 | 0.410 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 0.413 |
| `cargo test --workspace` | 0 | 38.286 |
| `bun test scripts/coverage/harness.test.ts` | 0 | 88.422 |
| `bun test scripts/coverage/evidence.test.ts` | 0 | 0.557 |
| `bun target/harness-speed/independent-qa/edgecases.ts` | 0 | 7.208 |
| `bun target/harness-speed/independent-qa/audit.ts` | 0 | 5.538 |

The coverage-verification aggregate executed these five existing suites serially; all four remaining coverage files ran separately as required. No existing coverage suite is omitted.

Final evidence-budget command: `bun test scripts/harness/evidence-budget.test.ts` — exit 0, 0.650 s command interval, 133 assertions. All eleven required command gates, including this final budget check, passed.

| Non-collecting suite | Passing tests | Assertions | Exit |
|---|---:|---:|---:|
| accounting.test.ts | 12 | 43 | 0 |
| gate.test.ts | 14 | 41 | 0 |
| hygiene-preservation.test.ts | 3 | 20 | 0 |
| hygiene.test.ts | 4 | 214 | 0 |
| preservation.test.ts | 1 | 3 | 0 |

The aggregate's separately required files are `coverage-cli.test.ts`, `clean-checkout.test.ts`, `harness.test.ts` and `evidence.test.ts`. Cargo workspace tests reported 275 passing tests and zero failures. The final native-evidence test passed 2,580 assertions after the final harness collection in the collecting checkout.

Clean-checkout evidence materialized the intended index tree `f25af4f19afa14a9208ad109795490a6cf0cfaf1` and matched root input identity `91b009125526827cd072e922cb1cafcede1ec5d6b2a09ee16dba0ab98dbd0379`. Its precollection checks prove no inherited target, coverage runs or latest pointer and no prohibited indexed artifacts. Its actual native coverage and evidence commands both exited 0; the generated LCOV/LLVM files still exist in `/var/folders/r9/0qvpcx794n385zwgzwfvt55h0000gn/T/arc-clean-coverage-lAsPth`. The root sources and index remained unchanged.

The full-harness inner receipt proves exact argv `./scripts/harness/validate.sh`, collecting cwd, exit 0, unchanged input identity and zero violations. Its source-bound test clears SKIP_COVERAGE and retains all real-coverage assertions. Receipt and binding paths/hashes are in `qa-audit.json`. Final root run: `2026-11-08T08-18-56-766Z-93dd6ce1-259e-4313-b343-79541cb3977e`. Current LCOV/LLVM hashes, all collected profiles and both explicit child PIDs were verified by the post-collection evidence test. Bulky native reports remain at their real ignored runtime locations, referenced from `qa.json`.

## Coverage and preservation

Coverage accounting, source inventory, native-profile merge verification, gate logic, manifests/lockfile and six 80% floors are unchanged. The gate still uses exact `BigInt(covered) * 100n >= BigInt(total) * 80n` independently for each crate. Negative tests retain malformed/stale/missing reports, denominator/source mismatches, rounded-below-boundary failures, per-crate failure despite high averages and all existing preservation cases.

| Crate | Covered/total production lines | Coverage | Exact floor |
|---|---:|---:|---|
| arc-core | 64/64 | 100.00% | PASS |
| arc-detector | 577/578 | 99.83% | PASS |
| arc-storage | 226/227 | 99.56% | PASS |
| arc-ipc | 215/215 | 100.00% | PASS |
| arc-daemon | 1508/1613 | 93.49% | PASS |
| arc-cli | 203/213 | 95.31% | PASS |

Amendment 4 only exempts harness bytes. Harness path, regular-file/non-symlink and executable/index-mode protections remain. Exact preservation passes for all 84 entries; Rust bytes/modes and migration bytes remain protected, and historical Rust-token/production-token checks pass. Existing negative tests were retained, with added harness-structure and migration rejection cases. Baselines remain `fbe76a5f15101ad415f355ef06f1deeb1f97d568` and `aacb3b4359f13c31c0705c0cffc5bf1ac00f387e`; no historical receipt was rewritten.

## Pre-existing hygiene failures

Independent Git-object audit reproduces the accepted snapshot's 932 indexed Rust coverage files and 457 prohibited artifacts. These were inherited failures, resolved by the operator-authorized task-3 untracking; they are not introduced scanner regressions. Fresh hygiene, clean-checkout and coverage-verification checks now pass. Exactly 24 whitelisted coverage files remain indexed, totaling 475,720 bytes; no prohibited indexed artifact remains.

All 24 retained files match their saved bytes/modes and accepted-HEAD content. All 908 removed files remain on disk; 907 retain their pre-removal bytes/modes. The sole intentional runtime change is the ignored latest pointer updated by this review's fresh collections. Historical run receipts and static reports remain unchanged. Earlier failed attempts remain in their original evidence files.

## Deliverables, budget and remaining risks

Directly produced: `qa.md`, `qa.json`, `timings.json`, `qa-commands.json`, `qa-audit.json`, `qa-edgecases.json`, all under `.arc-verify/harness-speed/`. Test-generated updates: equivalence/scaling/timing/coverage-verification receipts and the final budget receipt. Full streams and runnable supplemental checks are retained under `target/harness-speed/independent-qa/`; native reports remain in their linked collecting/scratch locations. No index staging or mission-state mutation was performed.

Evidence count including these reports is 36, strictly below 150; every evidence file is below 1,000,000 bytes. Final `bun test scripts/harness/evidence-budget.test.ts` passed after these reports were written: exit 0, 0.532 s suite time, 133 assertions. Its command receipt is included in `qa-commands.json`. Remaining limits: timings describe this host; host-wide process enumeration was unavailable; Chief integration and final acceptance are separate tasks. There is no reproduced product blocker or ownership/interface conflict to assign back to task-3.
