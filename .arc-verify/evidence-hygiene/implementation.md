# Task-2 implementation

Status: completed for task-2 implementation and preliminary checks. Mission acceptance and independent review belong to later managed tasks.

Accepted HEAD: `aacb3b4359f13c31c0705c0cffc5bf1ac00f387e`.

## Boundaries and interfaces

Used `oma-dev-workflow`, the installed backend-engineer guidance, the architecture-reviewer implementation authorization and task-1's accepted baseline/retention contract. No skill, profile, mission instruction, graph or acceptance criterion was changed. Serena was absent from callable tools; inspection used supplied paths, direct reads and Git metadata. No browser, dependency installation, unrelated build, external delivery or agent dispatch ran.

The existing Bun/Rust/LLVM stack is sufficient. A separate archive or replacement evidence schema would add conversion work without improving the required fresh native verification. The implementation keeps native runtime evidence local and retains the selected historical files. No product architecture changes are needed.

Owned implementation changes:

- `.gitignore`: the exact required runs/profiles rules, scoped coverage extensions, actual raw JSON names and runtime latest pointer. An explicit retained-file boundary also ignores nonselected local generated outputs, so an ordinary add cannot resurrect removed inventories/logs. Required durable summary/accounting files remain trackable through explicit negations.
- `scripts/coverage/acceptance.ts`: retains `realAcceptance(name, argv)` and moves future transient command receipts below `tooling/acceptance/runs/`.
- `scripts/coverage/coverage-cli.test.ts`: increases the existing real-coverage test's internal timeout to 960 seconds; its root command and assertions are unchanged.
- `scripts/coverage/hygiene.ts` and `hygiene.test.ts`: inspect every stage-0 evidence blob regardless of suffix, recognize native LLVM exports by type or file/segment/branch structure, reject runtime directories/coverage payloads/runtime pointers, enforce both exclusive 150-file bounds, verify selected source bytes/provenance, and test ignored/unignored probes in a fresh repository with global excludes disabled. Synthetic index fixtures verify renamed exports outside rust-coverage and exact count boundaries.
- `scripts/coverage/hygiene-preservation.test.ts`: verifies all 84 accepted protected files, including all 82 crate Rust files, migration and harness, against accepted HEAD. It checks index/local path sets, byte identity, modes and regular-file state; isolated fixtures exercise additions, removals, newline changes, mode changes and symlinks. The older historical token-preservation check remains separate and unchanged.
- `scripts/coverage/clean-checkout.test.ts`: creates a separate Git checkout from the current index tree, retains required history through local shared objects, checks no prior target/run/profile/raw/latest inputs exist, requires identical input maps, collects real coverage, then executes `evidence.test.ts` there. It asserts all six exact thresholds, current identity and native hashes. It neither copies ignored inputs nor invokes itself. The checkout and raw command receipts remain local for inspection.

`package.json`, the root coverage command, `run.ts`, `evidence.test.ts`, gate/accounting logic, historical accounting baseline, harness, Rust sources, migration and lockfiles are unchanged. Existing native LCOV/JSON recomputation, source binding, profile hashes, complete merge membership and explicit child PID checks remain required.

## Retention and local preservation

Consumed task-1 files without changing their bytes. Staged their existing deliverables to make the isolated checkout include the accepted dependency contract.

Applied the 24 selected destinations: 19 original byte-preserved files, three exact historical copies and two compact derived command receipts. Compact receipts keep original source/storage identity, actual argv/PID/timestamps/exits and output hashes. Missing metadata is marked unavailable. Historical storage HEAD is not asserted as execution HEAD. Bound original reports are unchanged; the documentation owner must label them through the maintained index and convert removed-path links to explicit Git-history references.

Removed 908 index entries using `git rm --cached --quiet --ignore-unmatch --pathspec-from-file=- --pathspec-file-nul`. No local artifact was removed by this operation and no history was rewritten. `retention-receipt.json` binds the operation to the original ignored manifest SHA-256 `c7aa81c97561770a8317efaa23ea2f89c4143660f4f558bd0012a899916c41c0`. All 456 original raw files, totaling 252,272,711 bytes, were rehashed before and after cleanup. Both required baseline commit objects remain available.

The final local recheck also verified every one of the 908 removed files against task-1's recorded size/hash: 260,184,384 bytes preserved. The compact retention receipt records current staged counts/bytes, including this report and fresh-checkout receipts. Seven new evidence-hygiene files are staged at this task boundary, including task-1's three unchanged inputs; only four are task-2 outputs.

Comparable Git blob accounting at the preliminary index checkpoint:

| Scope | Before files | Before bytes | After files | After bytes |
| --- | ---: | ---: | ---: | ---: |
| rust-coverage | 927 | 260,548,113 | 24 | 475,720 |
| all .arc-verify | 2,628 | 278,635,154 | 1,730 | 18,644,070 |

The all-evidence checkpoint included five new evidence-hygiene files and preceded this implementation report/final receipt updates. Final reporting must recalculate totals from the final staged/committed tree. Counts include each blob path once and sum logical uncompressed blob bytes; they do not measure pack, clone, disk or monetary savings. The exhaustive tracked-blob audit reported zero prohibited artifacts.

## Preliminary commands and results

`task-2-checks.json` records exact commands, timestamps, exits and hashes; full local logs are below `target/evidence-hygiene/task-2/runs/`.

- `git rev-parse HEAD`: exit 0, accepted HEAD matches. Scoped pre-existing implementation diffs were empty. Unrestricted diff emitted `av.yaml: Operation not permitted`; global cleanliness is not asserted and no bypass was attempted.
- `mise tasks --all`, `bun --version`, `rustup run 1.95.0 cargo llvm-cov --version`: exit 0; existing tasks and Bun 1.4.4/cargo-llvm-cov 0.6.21 confirmed. No pins changed and no provisioning was run.
- Focused command: `bun test scripts/coverage/hygiene.test.ts scripts/coverage/hygiene-preservation.test.ts scripts/coverage/gate.test.ts scripts/coverage/accounting.test.ts scripts/coverage/preservation.test.ts`. Initial exit 1: 32 passed/one failed because verbose Git output reports unignoring negations. Corrected the test to assert actual unignored semantics with `--quiet`, while inspecting each verbose result. Retry exit 0: 33 passed. After adding the retained-file ignore boundary, final focused run exit 0: 33 passed, 312 assertions, 6.19 seconds. No acceptance condition was removed.
- `rustup run 1.95.0 cargo fetch --locked --offline`: exit 0; cached locked dependencies available.
- `bun test scripts/coverage/clean-checkout.test.ts`: exit 0; one test passed, 31 assertions, 89.92 seconds. The isolated checkout began with no target, runs, profiles, raw reports or latest pointer. Its real `bun run test:coverage` and subsequent `bun test scripts/coverage/evidence.test.ts` both exited 0. Source hash `260b6772a9d1fa3baf329befb8d957b3ed80f58116b6be8bb973372367c169fd` matched the originating checkout before and after collection. `task-2-clean-checkout.json` retains the snapshot tree, absence checks, checkout path, run/host, six crate results, native hashes, explicit child proof and compact child-command receipts. Full reports/profiles remain in that isolated checkout. This is fresh preliminary proof, not the final integrated suite.
- `git diff --cached --check` on owned implementation/receipt paths: exit 0.
- Final `auditTrackedEvidence`/`assertHygiene`/`checkExactPreservation` invocation: exit 0; 24 rust-coverage files/475,720 bytes, zero prohibited artifacts and 84 exact protected files. `git diff --exit-code HEAD -- crates scripts/harness/validate.sh Cargo.toml Cargo.lock package.json mise.toml rust-toolchain.toml .agents/ .codex/agents/`: exit 0, empty. Nonignored untracked-file scan over coverage/evidence ownership paths: empty. Required standalone native Cargo/harness suites were not executed by this task; downstream verification remains required.

Resource observations: 8 logical CPUs, 16 GiB RAM, approximately 71 GiB free storage before collection. Execution stayed serial for collection-dependent work; existing mission concurrency 3, deadline/retry budgets and framework verification commands were not changed. Test internal timeouts are bounded; external contract timeouts remain unchanged.

Task-2 acceptance evidence: (1) executable hygiene, preservation and clean-checkout checks passed; (2) the manifest-bound retention receipt proves local raw preservation across index-only removals; (3) all 24 selected historical destinations are tracked with required categories and zero prohibited artifacts; (4) runtime/acceptance paths are ignored while the unchanged live verifier requires real native data; (5) this report and compact receipts record the initial test failure, corrected checks and actual successful fresh collection. No unresolved prerequisite was observed in these checks.

## Handoff

Keep the original raw-bearing mission workspace and `target/evidence-hygiene/baseline-local-manifest.json`. Integration must preserve index-only deletions; do not checkout/reset those paths. Rehash the original manifest records after integration before publishing final local-preservation claims. A raw-free checkout cannot prove preservation of the original local files.

Later owners still need documentation/index updates, final comparable accounting, required root-workspace collection/evidence and Cargo/harness receipts, managed independent task review, integrated oma-qa and Chief acceptance. Do not interpret this implementation report or any Historical receipt as mission completion. Serialize root coverage/harness collection and run root evidence verification after its last collection; each UUID run owns its target and profiles. Source-script changes invalidate previous collection identities.
