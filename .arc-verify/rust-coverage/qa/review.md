# Independent coverage review

## Review Result: PASS

**Completeness: completed for task-7's implementation and evidence review.** No CRITICAL, HIGH or MEDIUM product defects were reproduced. This review is independent of the implementation Actors. It does not mark the mission complete, execute task-8's frozen operator-verification runner, or approve missing final deliverables. The enclosing harness-timeout conflict below remains a Chief Director handoff.

Accepted HEAD: `fbe76a5f15101ad415f355ef06f1deeb1f97d568`.
Audited implementation-input SHA-256: `cf31468e23ac45fee3e5f04704e19d4b89f9b56365ef516b9daa9ebc6c9833d8`; **114 inputs**.
Host/target: `aarch64-apple-darwin`. Skill: **oma-qa**, installed qa-reviewer definition; shared execution policy, result contract and applicable checklist reviewed. No OMA run/session/claim path was injected; none was invented. No subagents, browser checks, installs, downloads, build commands, external effects or mission-state edits were performed.

## Findings by severity

- CRITICAL: none.
- HIGH: none in the audited coverage implementation.
- MEDIUM: none in the audited coverage implementation.
- LOW: none requiring a product repair.

No product blocker needs assignment back to task-1, task-3, task-4 or task-6. The Chief owns the separate final-verification/runtime handoff below.

## Required Chief handoff: frozen runner and final deliverables

The supplied immutable contract at `/mission/verificationContract/criteria/6/checks/0/timeoutMs` specifies **120000 ms** for `bun test scripts/coverage/harness.test.ts`. The current-source passing wrapper receipt records **227290 ms**, while its unchanged harness child took **225973 ms**. See `../integration/repair-4/acceptance.json:66` and `:149`, and the linked exact-command receipts below. `scripts/coverage/acceptance.ts:12` now allows 900000 ms and `scripts/coverage/harness.test.ts:13` allows 960000 ms, as directed by the operator note. The reviewer verified those values and preserved the frozen contract.

These receipts establish real command success; **they do not establish success under a 120-second enclosing runner**. Task-7 did not rerun that runner or weaken it. The Chief must reconcile the demonstrated duration with the saved execution allowance through the authorized mission mechanism, or retain final operator verification as incomplete. No operator decision or implementation method is requested here. There is no implementation patch assigned to QA for this immutable metadata conflict.

At inspection, `../final.md` and `../final-summary.json` did not exist. Their preparation and final judgment belong to task-8. These downstream obligations are outside this task-7 PASS; overall mission completion is **not claimed**.

## Coverage accounting and native evidence

The independent [reconciliation script](reconcile.py) reads raw LCOV, native LLVM JSON, input manifests, source inventories, profiles and all five final command receipts. It recomputes file/line unions and crate totals without calling the implementation's accounting function. It separately reconstructs every native line map from sparse LLVM segments, before canonical merging and test-only omission. Both raw formats agree for every record. A direct native re-export from the retained profdata/binaries has identical native counts, segments, functions and file sets. cargo-llvm-cov adds metadata and reserializes floating percentages; those presentation differences are recorded rather than represented as byte equality.

| Crate | Covered / production lines | Coverage | Result |
| --- | ---: | ---: | --- |
| arc-core | 64/64 | 100.00% | PASS |
| arc-detector | 577/578 | 99.83% | PASS |
| arc-storage | 226/227 | 99.56% | PASS |
| arc-ipc | 215/215 | 100.00% | PASS |
| arc-daemon | 1507/1613 | 93.43% | PASS |
| arc-cli | 203/213 | 95.31% | PASS |

The target is **80% per crate, a new engineering target**, not a baseline or SRS threshold. Counts use exact integer comparison. Both independent checks and implementation fixtures prove that exactly 80% passes, a rounded 80.00% below the boundary fails, and a high workspace average cannot conceal one failing crate.

All **29 production files** across all six crates are present. The live inventory contains 49 source files, including 20 files reachable only through test modules. The [omission audit](omission-audit.json) records all 54 omitted spans; inspection confirms test-only modules, injection helpers and observations guarded by cfg expressions false without `test`. Platform predicates remain unknown to the omission logic. Arbitrary filenames are not exclusions, and production modules recompiled through integration-test path attributes remain production.

Zero-hit production lines remain in the denominator and per-file report. Examples include `crates/arc-daemon/src/db_worker.rs:45` through its error-format/source paths, `crates/arc-daemon/src/instance.rs:34`, and `crates/arc-cli/src/main.rs:131`. These actual uncovered paths were retained rather than removed to meet the target. Every per-file mapped/uncovered/omitted list matches the independent recomputation. The reports' upstream filters are confined to recorded dependency/compiler roots; the broad tests-directory filter is disabled (`scripts/coverage/run.ts:57-64`, `:87-89`).

The two separate same-source instrumentation runs are:

- Standalone: `.arc-verify/rust-coverage/baseline/runs/2026-11-08T00-43-49-396Z-e559836e-73d9-4c33-b202-cd7a23cb44cb`.
- Harness: `.arc-verify/rust-coverage/baseline/runs/2026-11-08T00-48-46-903Z-669089ee-9bd4-42bc-910e-4e1b9f574228`.

Each retains **851 raw profiles**. All 1702 profile hashes/sizes, the on-disk merge lists, report hashes and input hashes were independently checked. Every explicit CLI/daemon child profile is in its actual merge list. Native instrumented receipts show **275 passing tests, zero failed and zero ignored**, including unit, integration, real CLI protocol and resident-daemon cases. The five exact command receipts match the current input identity, cwd, argv, exits and available log hashes. Historical failures were not replaced: [the evidence catalog](evidence-catalog.json) identifies older/context-only records and excludes them from current acceptance proof.

The native text-view cross-check initially appeared to omit hits contributed through `tests/../src` aliases. The tool normalizes its source selector to the canonical spelling. For example, admission line 47 has zero hits in the canonical record and 100096 hits in the path-included production record; both map to the same source file. The canonical text view matches its exact record, while the required source union correctly retains either instance's hit. [Reconciliation](reconciliation.json) records this view limitation and zero remaining unexplained differences. Native `--dump` identifies one unmatched function as Tokio's dependency-only `sync::mpsc::block::start_index`; no workspace function mismatch was reported.

## Reproduced checks and intended failure branches

The current implementation's deterministic gate/accounting/preservation/evidence command passed **28 tests and 2667 assertions**, exit 0; see [receipt 00](receipts/00-command.json). Fixtures use subprocess exits and assert the intended diagnostic, so an unrelated missing prerequisite cannot count as the expected failure. `gate.test.ts:9` supplies a valid older LCOV report, and the driver has no production fixture mode or fallback path.

The additional [driver probes](driver-probes.py) execute byte-identical copies of every coverage script in isolated fixture roots. Fake tools are explicit controlled test inputs, never production measurements. Each case first runs a complete successful 80% fixture measurement in the same root. It then runs the intended failure while that valid older summary, reports, profiles, receipts and passing pointer remain available. Assertions require the expected diagnostic, nonzero exit, a new run ID and a current failed pointer; the positive control requires exit 0. [Probe outcomes](driver-probes.json) and `receipts/driver-complete-prior-*.json` retain stdout/stderr, argv, timestamps, copied-script hashes and the older passing run identity.

| Condition | Expected branch / result | Evidence |
| --- | --- | --- |
| Exact 80% | exit 0, six PASS rows | positive-80 probe; gate suite |
| Failed test process | TEST_FAILURE, exit 1; no old report rescue | failed-tests probe; gate suite's real exit-23 subprocess |
| Missing/mismatched tools | MISSING_TOOL / TOOL_VERSION, exit 1 | missing-tool and wrong-tool-version probes; nonexistent-executable unit fixture |
| Failed child execution | CHILD_FAILURE, exit 1 | bad-child-exit probe; `run.ts:77` |
| Missing/empty child profile | CHILD_PROFILE, exit 1 | missing-child-profile / empty-child-profile probes; `run.ts:79-80` |
| Missing/incomplete merge inputs | CHILD_PROFILE, exit 1 | missing-merge-list / merge-mismatch probes; `run.ts:93-95` |
| Export subprocess failure | REPORT_FAILURE, exit 1 | failed-export probe |
| Missing LCOV or JSON | MISSING_REPORT, exit 1 | missing-lcov / missing-json probes |
| Stale LCOV or JSON | STALE_REPORT, exit 1 | stale-lcov / stale-json probes |
| Changed configuration during run | STALE_REPORT, exit 1 | changed-config probe; gate suite's changed identity |
| Below-target crate | BELOW_THRESHOLD, exit 1 | one-crate-below probe; passing-average gate fixture |
| Malformed reports | nonzero; MALFORMED_REPORT or JSON parse error | malformed-lcov / malformed-json probes |
| Missing crate/file, zero denominator, unknown file | nonzero for intended validation error | gate/accounting suite |
| LF/LH corruption, duplicate/out-of-range/unsafe data | reject before merging records | gate/accounting suite |
| Alias/repeated compilation and test-only omissions | canonical union; exact expected denominator | accounting suite plus independent native reconciliation |

For real child execution, the reviewer reran the retained instrumented resident test with `--exact coverage::startup_failures_release_admission_then_sigint_reaps_instrumented_children --nocapture`, using review-owned profile/evidence paths. It passed in about 0.2 seconds. Its receipt establishes **five distinct reaped children** (three daemon, two CLI), their nonempty inherited profiles and graceful SIGINT shutdown. Merging only those five child profiles and exporting only the actual daemon/CLI objects produces positive production counters for both crates, independent of a passing test count. See [native test receipt](receipts/real-children.json), [child merge/export receipt](receipts/real-child-merge.json) and [lifecycle evidence](real-children/coverage-lifecycle.json). Original measured profiles were not modified.

## Five exact command receipts

These are inspected, source-bound **reused native executions**, not new full-suite runs by QA. Task instructions explicitly permit reuse of trustworthy same-source receipts. The deterministic suite, focused native integration/CLI case, independent native export and failure probes were reproduced by this reviewer.

| Exact command | Exit | Duration | Retained receipt |
| --- | ---: | ---: | --- |
| `bun run test:coverage` | 0 | 80.164 s | `.arc-verify/rust-coverage/tooling/acceptance/coverage-cli-f8af459c-a9cf-49ed-9c31-34138816c24e/00-command.json` |
| `cargo fmt --all -- --check` | 0 | 1.116 s | `.arc-verify/rust-coverage/integration/runs/repair-4-fmt-2026-11-08T00-45-17-805Z` |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 0.376 s | `.arc-verify/rust-coverage/integration/runs/repair-4-clippy-2026-11-08T00-45-18-928Z` |
| `cargo test --workspace` | 0 | 40.064 s | `.arc-verify/rust-coverage/integration/runs/repair-4-workspace-tests-2026-11-08T00-45-22-579Z` |
| `./scripts/harness/validate.sh` | 0 | 225.973 s | `.arc-verify/rust-coverage/tooling/acceptance/harness-c4da9be1-0b54-46d4-ba48-5c247c503f88/00-command.json` |

The harness output contains `All checks passed (0 violations)` and all six crate PASS rows, and no skip flag or skip diagnostic. Its nested measurement is separate from the standalone run. The source fingerprints match both before and after all five commands. These command successes do not remove the enclosing-runner issue above.

## Pins, preservation and compatibility

Rechecked installed Bun **1.4.4**, cargo-llvm-cov **0.6.21**, and Rust 1.95.0's installed LLVM tools; [version receipts](receipts/02-command.json), [components](receipts/03-command.json), [Bun](receipts/04-command.json). Native run versions show Rust/Cargo 1.95.0 and matching LLVM **22.1.2**. The operator's prerequisite change was honored: earlier permission denials were treated as historical, and current tools/measurements were checked. No new installation was needed.

`mise.toml`, `rust-toolchain.toml`, `scripts/coverage/config.ts` and `provision.sh:4-6` agree on exact versions and `cargo install --version 0.6.21 --locked`. Provisioning is separate; the gate never installs tools, and Cargo measurement is locked/offline. Cargo manifests and Cargo.lock are unchanged from accepted HEAD. No Bun package dependency or new package lockfile is introduced.

The independent byte comparison and preservation test confirm:

- `scripts/harness/validate.sh`: `93140173df8b8bd28b067aff68301134b0398b643240b0b1e6a13da4487ebe33`.
- `crates/arc-storage/migrations/0001_phase0.sql`: `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`.
- All 77 pre-existing Rust files retain their original ordered tokens/assertions. Production tokens are unchanged; the only new production-file declaration is the cfg(test) detector test module. Manual inspection checked that added tests/helpers do not disable or weaken existing assertions. The schema fixture's added `set_nonblocking(false)` repairs accepted-socket blocking behavior before the retained I/O assertions (`crates/arc-cli/tests/schema.rs:51`).

No production permission, protocol, behavior or interface changed. All 793 preceding integrated paths and all 114 measured inputs were rehashed after review; none changed. [Ownership receipt](ownership-preservation.json). Review output is confined to this QA directory, including isolated runtime fixtures, receipts, reports and copied source snapshots. No shared manifest, lockfile, threshold, saved instruction or mission state was edited.

Security review found no added user-controlled shell interpolation: commands use argv arrays and explicit executables. Existing runtime dependency choices and product access rules are preserved. Dependency-vulnerability audits, web accessibility and browser performance tooling are not applicable to this dependency-free local gate change. Terminal results use text labels and actionable paths rather than color alone. No screen or browser suite was run.

## Platform and evidence limits

Inactive macOS-host source is still inventoried: `arc-core/src/lib.rs:21-24` (non-macOS runtime), `:71-72` (instance root), `:95-104` (Windows/XDG database bases); `arc-daemon/src/platform.rs:11-38` (Linux peer credentials), `:99-121` (Linux /proc process-start), and unsupported-platform fallbacks. These branches are compiler-inactive and **unmeasured**, not covered or coverage-driven exclusions. Linux gate admission is not a Linux measurement. Native Windows and desktop were not tested; stable Rust doctest coverage is outside this collection.

Serena was absent from the exposed tool inventory. Inspection used supplied-path reads, Git metadata/diffs and plain evidence processing, without native repository code search. Global status reports `av.yaml: Operation not permitted`; no bypass was attempted and global cleanliness is not asserted. Source-bound evidence is local and assumes honest local tools/receipts; it is not external delivery or a tamper-proof attestation.

Review-only failed diagnostic attempts are recorded, not product failures: an initial LLVM show invocation supplied a source as an object (exit 1; corrected with the documented local --sources form); an initial reconciliation expected captured libtest output (corrected by reproducing the focused test with --nocapture); the text-view alias limitation and JSON metadata/float serialization differences were investigated and resolved as described above. Final reconciliation exits 0; earlier failure logs remain in receipts.

## Acceptance checklist and Chief delivery

- [x] Review artifacts contain verdict, scope, completeness, reproducible checks, source hashes and concrete references.
- [x] Required failure paths, accounting rules, preservation and all five command outcomes assessed.
- [x] Independent deterministic reproduction and actual native child execution completed.
- [x] Missing evidence/downstream work separated from product findings; no fabricated final approval.
- [x] Ownership respected; no implementation or immutable contract edited.

Changed paths: **`.arc-verify/rust-coverage/qa/**` only**. Main deliverables are `review.md`, `review.json`, `reconciliation.json`, `driver-probes.json`, `evidence-catalog.json`, `omission-audit.json`, the two reproducible Python audit scripts, `ownership-preservation.json`, and linked receipts. Runtime fixture/raw-profile data remains local and is excluded by this directory's own `.gitignore`; its hashes and reproduction commands are retained. No preceding Actor files were altered.

Chief next actions: consume this scoped PASS, preserve the frozen-runner timing handoff, prepare task-8 final deliverables and report actual operator-verification outcome. Do not represent this review as a completed frozen 120-second harness invocation.

- desktop: incomplete
- native Windows: incomplete
- remaining Phase 0: incomplete
