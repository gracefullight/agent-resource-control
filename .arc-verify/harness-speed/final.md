# Chief final acceptance

Status: passed. Task-6 changes only `index.md`, `final.md` and `final.json` in `.arc-verify/harness-speed/`. Accepted HEAD remains `f85ae0430038cc3d7a6125f650872dbfce6b5529`. Product implementation, preserved fixtures, independent review and the authorized index-only artifact removals were inspected before closeout. The original goalBrief, verificationContract, graph, task instructions and assignments were not edited.

## Acceptance decisions

1. **Behavior preserved.** [Equivalence](equivalence.json) contains 79 passing deterministic isolated cases. Chief inspection rehashed every retained original/candidate stdout and stderr file, confirmed equal exits and all recorded coverage-call comparisons, and checked all current source bindings. Fixtures cover every violation family, multiple matches and ordered replay, exclusions with positive controls, missing/unreadable files, staged working contents, empty input, coverage success/failure/no-Bun, explicit skip equivalence, newline boundaries, binary notices, symlinks and option-like filenames. The independent reviewer added four scratch-path cases. Native tracing records 38 → 20 grep calls for 2 files and 1,520 → 20 for 80 files. No skipped-coverage fixture substitutes for full acceptance.
2. **Speed target met.** All three candidate Checks 1–4 intervals are below 20 seconds on the accepted 4,044-entry corpus; even start-to-Check-5 intervals are below 20. Chief independently reproduced both corpus hashes and byte count using `acceptedCorpus()`. Complete original/candidate timings, distinct Check 5 and total intervals, tools/environment/host, before/boundary corpus audits, final deviations and stream hashes are in [timings.json](timings.json) and [timing.json](timing.json). Chief verified all four retained stream pairs against their receipt hashes.
3. **Coverage and preservation retained.** All nine existing coverage suites are covered by the five-suite serial aggregate plus four explicit live suites. Independent QA passed the unchanged accounting, native-report and profile checks, 84-entry exact preservation and Rust-token/migration protections. Only obsolete harness-byte equality changed under amendment 4; harness path, regular-file and executable/index-mode guards remain. All six exact per-crate 80% floors pass; current counts are listed below.
4. **Cargo checks passed.** Exact formatting, Clippy and workspace-test commands exited 0; retained workspace output records 275 passing tests. Chief rehashed all thirteen independent command logs and inspected their outcomes. No product input changed since those runs, so expensive suites were not repeated during report-only closeout.
5. **Full harness and fresh native evidence passed.** The inner source-bound command receipt proves exact argv `./scripts/harness/validate.sh`, exit 0, no skip flags, six real coverage passes and zero violations on the integrated index. Its unchanged collecting input hash is `91b009125526827cd072e922cb1cafcede1ec5d6b2a09ee16dba0ab98dbd0379`. Chief verified that receipt, its stdout/stderr hashes, clean-checkout receipt and current native summary/latest hashes. Chief then ran `bun test scripts/coverage/evidence.test.ts`: exit 0, 1 test, 2,580 assertions. Native artifacts remain in the receipt-linked ignored run directory, not copied into this evidence set.
6. **Independent review and compact evidence complete.** [Independent oma-qa](qa.md) returned PASS with no reproducible findings. [Final budget receipt](budget.json): 40 files, below 150; command exit 0; 141 assertions. Count includes all nested/untracked mission evidence, Director reports and these three closeout files. Historical failed receipts remain available. No mission-state or source repairs were needed during closeout.

## Timing and commands

| Run | Checks 1–4 s | Start to Check 5 s | Check 5 s | Total s | Exit |
|---|---:|---:|---:|---:|---:|
| original | 163.421057 | 164.162468 | 83.568649 | 247.733245 | 0 |
| candidate-1 | 14.616738 | 15.349835 | 82.110619 | 97.467000 | 0 |
| candidate-2 | 14.657121 | 15.172532 | 81.838186 | 97.016641 | 0 |
| candidate-3 | 14.675457 | 15.185321 | 81.483307 | 96.674869 | 0 |

Each benchmark was serial and ran real coverage. Checks 1–4 spans the Check 1 label to Check 5; Check 5 spans its label through `Coverage thresholds met`; startup and finalization/drain are separately recorded. These observed intervals include marker delivery and boundary-audit costs.

The following gates were executed by independent QA against the current implementation; Chief verified their retained receipt/log bindings. The framework's per-check allowance is 3,600,000 ms; the contract's 120,000 ms estimates did not terminate commands.

| Exact command | Exit | Seconds |
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
| `bun test scripts/harness/evidence-budget.test.ts` after closeout files exist | 0 | 0.940 |

Chief also executed scoped Git status/diff and immutable-object reads, a read-only Python receipt/source/stream audit, corpus reproduction via `bun -e`, and the fresh native-evidence command above. All exited 0. Serena discovery returned no tools; inspection used known paths, diffs and Git objects without replacement source search. The memory quick pass found no task-relevant history and supplied no acceptance evidence. No plan, mission-state, product file, manifest, lockfile or index entry was changed by Chief. Only the explicitly requested budget test refreshes its generated receipt.

| Crate | Covered / total production lines | Exact 80% floor |
|---|---:|---|
| arc-core | 64 / 64 | PASS |
| arc-detector | 577 / 578 | PASS |
| arc-storage | 226 / 227 | PASS |
| arc-ipc | 215 / 215 | PASS |
| arc-daemon | 1508 / 1613 | PASS |
| arc-cli | 203 / 213 | PASS |

## Decisions and compatibility

The existing Bash/native grep stack remains; no dependency, caching layer or narrower default scan was added. The original sequential batching plan was tested: [phase profile](profile.json) measured a 30.238 s scan, including 27.925 s in Check 1. [Pattern profiles](pattern-profile.json) and the retained recovery record justify at most three concurrent secret-pattern batches, followed by parent-only ordered replay. This resolves the Director's advice against measured need while retaining immutable acceptance. Other checks remain ordered, and Check 4 through final exit is byte-identical. Force-push triggers/reporting and within-domain filtering remain distinct. Special option-like paths preserve their original native grep interpretation without affecting ordinary batched inputs. Check 4's roughly two-second cost did not justify changing newline semantics.

The interface remains the same validator command, modes, output messages, order, counts and exits. Accepted oracle SHA-256 is `93140173df8b8bd28b067aff68301134b0398b643240b0b1e6a13da4487ebe33`; candidate SHA-256 is `0e66d222504f469cc9be910e1c7a6e33544440f017809e25cc255f6dc90f29a8`. Chief verified all 30 current harness/coverage source entries and the reviewed index tree `f25af4f19afa14a9208ad109795490a6cf0cfaf1`. Rust sources, migration, coverage collection/accounting, manifests and lockfile remain unchanged. Product paths changed by the implementation are enumerated in `final.json`; closeout does not claim ownership of those changes.

## Pre-existing hygiene failures and failed attempts

Accepted HEAD inherited 932 indexed Rust coverage files, including 457 prohibited artifacts. This caused real hygiene, clean-checkout and budget failures in preceding attempts. The operator's 2026-11-08 authorization resolved that prerequisite through `git rm --cached`: 908 index entries removed, disk files retained, exactly 24 whitelisted durable files/475,720 bytes left indexed and zero prohibited artifacts. Fresh post-authorization hygiene, clean-checkout and evidence-budget checks passed. Independent QA verified all 24 retained bytes/modes and all 908 disk paths; only the normal ignored latest pointer changed during subsequent fresh collections. See [recovery proof](attempt-6-recovery.json), [independent audit](qa-audit.json) and [initial failure](pre-existing.json).

Other retained attempts record the former 1,800-second Actor deadline termination, the benchmark's overly strict post-coverage latest-pointer assertion, BSD awk NUL framing, an option-like filename regression and a trace separator count correction. Those attempts are not used as passing evidence. Corrections and the fresh 79-case matrix remain in [implementation history](implementation.md), [option regression](option-regression.json), [trace correction](trace-count-correction.json) and [current QA commands](qa-commands.json). Current required gates have no unresolved failures. Stale partial conclusions in earlier reports are preserved as history; the current source-bound review supersedes them.

## Payoff and remaining limits

Mean scan time fell from 163.421 to 14.650 seconds: 148.771 seconds saved, 91.04% lower, 11.16× faster. Mean full-harness time fell from 247.733 to 97.053 seconds: 150.680 seconds saved, 60.82% lower, 2.55× faster. Check 5 now occupies 84.30% of mean total; its small timing difference does not establish a coverage optimization. [Value report](value.md) separates measured elapsed time from unknown economics and contributor reactions.

Measurements cover one Darwin arm64 host with Bash 3.2/BSD grep, one baseline and three candidate runs in fixed order. Cache warmup and host load may affect comparisons; host-wide process enumeration was denied, although reviewer commands were serial. Scratch/native artifacts are retained locally at receipt-linked paths and are not portable durable exports. Financial return, user adoption and recovery-task performance were not measured. No remaining required implementation work, ownership conflict or blocked prerequisite was found. Framework final verification is separate and has not been represented as already executed by this task.

Skill used for Chief acceptance: oma-pm, scoped to requirements/evidence mapping. Implementation used oma-dev-workflow and oma-refactor; independent review used oma-qa. No browser, visual check, package installation, external delivery or additional agent dispatch occurred.
