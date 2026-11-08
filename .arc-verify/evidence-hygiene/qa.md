# Independent evidence-hygiene QA

## Review Result: PASS

Status: completed. Completeness: completed for task-6. No confirmed CRITICAL, HIGH, MEDIUM or LOW defect. This review refreshes the integrated amendment's evidence and current task-4 claims binding. Chief acceptance and final managed verification remain with task-7.

Used the installed `.agents/agents/qa-reviewer.md` and `oma-qa`, its execution protocol, checklist and shared policy. Changed only `qa.md` and `qa.json`. No implementation, Git index, saved assignment, mission state, framework review or another Actor report was edited.

## Evidence

Review time: 2026-11-08T05:13:12.224694+00:00. Accepted/current HEAD: `aacb3b4359f13c31c0705c0cffc5bf1ac00f387e`. Accounting baseline: `fbe76a5f15101ad415f355ef06f1deeb1f97d568`. Reviewed stage-0 index SHA-256: `32cff37baa1d225c10a695474123051bbef31f2009729deeff9ac95ca5f6775c`. Collection snapshot: `6903568319eae73e7518909e54d410379d94dbc1`. Implementation identity: `260b6772a9d1fa3baf329befb8d957b3ed80f58116b6be8bb973372367c169fd`.

All 118 input hashes agree across receiving, retained final and nested clean checkouts. `qa.json` binds 159 reviewed files by exact path, bytes and SHA-256. Of these, 158 match the index. The authorized revised `claims.md` is unstaged; its current reviewed hash is `dc9c10697f2a27adc053ea84df138f460b934d4905ea70fa6ad47689adee18a5`. It describes a separate Actor snapshot's 932-file tree. That discrepancy does not reproduce in this receiving index: 24 coverage files and zero prohibited artifacts. Its request for refreshed downstream hashes is addressed by this review and remains for the Chief's final files. Prose changes do not alter collection inputs.

Prior QA and Chief files bind earlier report bytes. This refresh supersedes task-6's earlier input binding; the existing Chief verdict is not relabeled as current acceptance. Framework task reviews remain untouched.

### Immutable acceptance criteria

| Criterion | Result and inspected evidence |
| --- | --- |
| C1: bounded counts, zero prohibited artifacts, deterministic ignores | PASS. Independent Git tree/index/content scan. Fresh hygiene tests cover 13 positive and nine negative ignores, renamed/opaque native exports and exact 149/150 boundaries. |
| C2: required retention categories and historical/fresh distinction | PASS. All 24 destinations span accounting, summary, uncovered, receipt and review. All original Git source hashes match; 22 exact files and two Historical compact receipts verified. All 42 guide/index links resolve. Comparable checkpoint accounting independently matches. |
| C3: fresh clean-checkout collection, native/source/child evidence, six floors and failure checks | PASS. Nested result hash, checkout tree/source identity, empty precollection inventory and collection-before-verification chronology corroborated. Live evidence passes in both collecting checkouts; all 118 inputs match. Actual profile merges/native hashes checked for four collections. |
| C4: root command, verification, wrapper and standalone harness | PASS. All 12 required argv have successful same-source receipts. Rehashed 56 streams and 65 references. Both wrapper child bindings and actual receipts agree. Completed harnesses show zero violations, coverage met, six PASS labels and no skip flag. |
| C5: Cargo checks and exact protected preservation | PASS. Cargo fmt, Clippy and tests have successful same-source receipts. All 84 protected local/index paths, bytes and modes equal accepted HEAD. Fresh negative preservation tests pass. |
| C6: raw/history preservation, independent review and Chief assessment | PASS for task-6. All 456 original raw files rehashed; required commit/tree objects accessible. Independent review delivered. Chief task-7 must refresh final decision, bindings and accounting and retain final managed verification. |

### Current-tree accounting

| Scope | Before files | Before blob bytes | Current index files | Current index blob bytes |
| --- | ---: | ---: | ---: | ---: |
| Rust coverage | 927 | 260548113 | 24 | 475720 |
| All `.arc-verify/` | 2628 | 278635154 | 1740 | 18836277 |

Method: accepted `git ls-tree -r -z` and stage-0 `git ls-files --stage -z`, then `git cat-file --batch`; count each path and sum uncompressed blob bytes. Every indexed evidence blob was checked for prohibited directories, extensions, native JSON names/runtime pointer and parsed LLVM export structure regardless of suffix. Zero prohibited artifacts. `before-after.json` remains accurate for its labeled staged checkpoint.

The index has 15 new amendment files; adding this QA pair gives 17. Including/reserving the Chief's existing final pair gives 19, below 150. Coverage remains 24. Chief must recompute final delivery bytes after integrating revised claims and reports. QA did not stage files. Logical blob reductions do not measure transfer, disk-space or financial savings.

### Independently executed checks

| Command/check | Current result |
| --- | --- |
| `bun test scripts/coverage/hygiene.test.ts scripts/coverage/hygiene-preservation.test.ts scripts/coverage/gate.test.ts scripts/coverage/accounting.test.ts scripts/coverage/preservation.test.ts` | Exit 0; 33 pass, 0 fail, 312 assertions; 5.92 s. |
| `bun test scripts/coverage/evidence.test.ts` in final collecting checkout | Exit 0; 1 pass, 2580 assertions; 0.722 s. |
| Same live test in nested collecting checkout | Exit 0; 1 pass, 2580 assertions; 0.676 s. |
| `bun -e` using `inputIdentity` across three checkouts | Exit 0; 118 input hashes and aggregate identity equal `commands.json`. |
| Independent Python Git tree/index/content, protected/raw/history and receipt audit | Exit 0; counts above, 84 protected files, 456 raw files, 56 streams, 65 references and two child bindings corroborated. |
| Independent Python actual native/merge audit | Exit 0; four collections each have 851 actual profiles; actual merge list equals actual inventory and manifest. Native hashes and all six exact floors pass. |
| Independent Python retention/link/environment audit | Exit 0; 24 original source hashes, 22 exact files, two Historical derivations and 42 links verified. `GIT_LITERAL_PATHSPECS` unset. |
| `git diff --check --cached -- .gitignore scripts/coverage docs/rust-coverage.md` | Exit 0; no diagnostics. |
| Scoped `git diff --name-status HEAD` | Exit 0; eight expected workflow/document paths; protected/product/root configuration unchanged. |

The operator environment change was rechecked in this run: the unchanged ignore test passes and the earlier framework literal-pathspec failure does not recur. No implementation repair was needed. Positive probes cover root/nested runs/profiles, coverage extensions, native JSON names, runtime pointer, transient acceptance output and redundant generated files; negative probes retain durable JSON and unrelated fixtures.

Initial reviewer audit attempts exited nonzero because they assumed an immutable latest pointer, resolved outer logs/nested receipts in the executing rather than storing checkout, and required revised claims prose to match the index. Corrected those audit assumptions and completed all checks; no acceptance or test was changed. A report transport attempt failed on unavailable `btoa` before any write and was replaced with literal JSON. These attempts are recorded separately from defects and passes in `qa.json`.

### Reused required command evidence

Task-5 executed these commands; their inputs, streams and receipt references were independently corroborated here. They were not repeated solely for reviewer ownership.

| Exact command | Exit | Elapsed |
| --- | ---: | ---: |
| `bun run test:coverage` | 0 | 84.974 s |
| `bun test scripts/coverage/gate.test.ts scripts/coverage/accounting.test.ts scripts/coverage/preservation.test.ts` | 0 | 1.941 s |
| `bun test scripts/coverage/hygiene.test.ts` | 0 | 1.087 s |
| `bun test scripts/coverage/hygiene-preservation.test.ts` | 0 | 4.199 s |
| `bun test scripts/coverage/clean-checkout.test.ts` | 0 | 87.764 s |
| `bun test scripts/coverage/coverage-cli.test.ts` | 0 | 83.404 s |
| `bun test scripts/coverage/harness.test.ts` | 0 | 234.146 s |
| `cargo fmt --all -- --check` | 0 | 0.311 s |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 10.7 s |
| `cargo test --workspace` | 0 | 76.109 s |
| `./scripts/harness/validate.sh` | 0 | 211.329 s |
| `bun test scripts/coverage/evidence.test.ts` | 0 | 2.696 s |

The successful harness wrapper and standalone ran after the timeout clarification with 1020-second outer allowances; the framework's operator allowance is 1800 seconds. Original harness exits -15 after 120.016 and 120.007 seconds remain historical failures. They are not counted as passes. The later environment correction is covered by this run's fresh hygiene check.

Final collecting checkout: `/var/folders/r9/0qvpcx794n385zwgzwfvt55h0000gn/T/arc-final-coverage-g9ku0kyt`; receipt run `2026-11-08T03-39-01-852Z-54eb07ec-10c6-4817-8cd9-f9b44bd88848`. Nested clean checkout: `/var/folders/r9/0qvpcx794n385zwgzwfvt55h0000gn/T/arc-clean-coverage-hF5HU5`. Its retained local result hash, checkout-specific native reports, no-artifact inventory, snapshot tree, input map and command chronology were corroborated. Live tests ran in those collecting checkouts; no absolute-path exports were copied.

| Crate | Covered / total | Exact 80% gate |
| --- | ---: | --- |
| arc-core | 64 / 64 | PASS |
| arc-detector | 577 / 578 | PASS |
| arc-storage | 226 / 227 | PASS |
| arc-ipc | 215 / 215 | PASS |
| arc-daemon | 1507 / 1613 | PASS |
| arc-cli | 203 / 213 | PASS |

Core run/accounting/source/gate/evidence code and existing negative tests are byte-identical to accepted HEAD. They preserve unique targets, before/after input hashes, LCOV/LLVM corroboration, profile-byte hashes, actual merge membership and child command/PID proof. The daemon exit 1 is its expected invalid-socket probe. Tests still reject failed subprocesses, missing/mismatched tools, missing/stale/malformed reports, changed inputs, absent crates/files, workspace-average masking, rounded-below-80 values and inflated summaries. No threshold or failure assertion was weakened.

### Preservation and interface compatibility

All 82 Rust files, `crates/arc-storage/migrations/0001_phase0.sql` and the harness equal accepted HEAD byte-for-byte with equal path sets and modes. Negative tests reject added/deleted files, byte edits, modes and symlinks. Manifests/lockfiles, product interfaces and harness are unchanged. Acceptance output now lives under ignored `tooling/acceptance/runs/`; root commands, six-crate order, accounting baseline and failure behavior remain compatible.

All 456 original raw files / 252272711 bytes still match the baseline manifest. Of 908 removed paths, 907 static files / 260184102 bytes equal their accepted Git blobs now. The remaining path is deliberately mutable ignored `baseline/latest.json`, now naming `2026-11-08T04-44-29-279Z-d02f52c0-8b96-4c60-a68a-638d0ad271c3` with the same input identity and pass=true. Original pointer bytes remain accessible in accepted history. Task-5's earlier 908-file equality receipt is timestamped evidence, not a promise that a runtime pointer never changes. No original raw file was lost. Both accepted and historical-baseline commit/tree objects are accessible.

## Findings

### CRITICAL

None.

### HIGH

None.

### MEDIUM

None.

### LOW

None.

No reproducible defect requires an implementation repair. The current unstaged claims/report boundary is an explicit Chief integration item. No task-6 verification gap remains.

## Completeness and handoff

Task-6 completed independently. Changed only `qa.md` and `qa.json`; all immutable criteria map to actual reproductions or corroborated same-source receipts. Completeness is separate from the PASS quality verdict. Chief task-7 must integrate current claims and QA artifacts, refresh final decision/bindings and complete-delivery byte totals, retain framework task reviews and run the unchanged managed verification contract.

Serena discovery exposed no callable provider; used supplied/known paths, scoped direct reads, Git metadata and deterministic checks. No substitute project source search ran. Global worktree cleanliness is not asserted because `av.yaml` has a previously observed permission boundary; no bypass was attempted. Workflow-only changes need no screen check. No browser, GUI, Playwright, installation, download, publication, external action or extra coverage collection was performed. Native receipts establish macOS measurements; no unsupported platform, certification or savings claim is made.
