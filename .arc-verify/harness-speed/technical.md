# Harness batching implementation contract

Status: task-1 complete; implementation and mission acceptance are unmeasured.
Skill: oma-architecture, Recommendation Mode. This report and `contract.json` are the only files owned by this Actor. The saved mission is the acceptance authority. No new ADR, workflow event or mission-state change is needed for this scoped contract.

## Source and existing changes

The accepted source is `f85ae0430038cc3d7a6125f650872dbfce6b5529`, tree `b7e6c535d7e0b963e6af9bc15d7645ad0f7cded2`. Actor-local HEAD is `5873a552333a07300bd102775d3f20eb23d1ef8c`, with that same tree. Its supplied snapshot ref is `refs/agent-valley/task-evidence/c79eadbc796a4c83-7afaa346b4bf92bf-1/snapshot`. These are distinct commits; neither the candidate nor these report files are already in the accepted baseline. The supplied snapshotRef and workerRef names do not resolve in this isolated Actor repository (individual `git rev-parse --verify --quiet` exits 1; initial combined resolution failed with unknown revision). Their names are preserved verbatim in the contract, with null local resolutions. Accepted/snapshot commit objects and trees were verified directly. Source/review repository ref availability is unverified; do not substitute local HEAD for a missing ref.

Initial scoped working/index diffs and status for scripts, crates, Cargo manifests and owned evidence paths were empty. The accepted harness and coverage directory also matched the local checkout. Global `git status --short --untracked-files=normal` reported `av.yaml: Operation not permitted`; global cleanliness is unverified. No denied-file workaround was used. The four promised acceptance test paths and harness-speed directory were absent initially.

Serena discovery returned no tools. Inspection used the supplied known paths and Git object reads, without native repository content search. Prior decision index, runtime/state-ownership proposal and Chief-decision-ownership ADR were read; their state and delivery boundaries do not require a new runtime or a change to this shell validator.

## Problem and approach

The accepted validator launches grep inside file/rule loops and wc/tr inside the eligible-file loop. The decision is how to reduce process creation while keeping its observable contract: eligibility, regex behavior, file/rule/line ordering, messages, counts, streams and exits.

| Approach | Implementation and operating cost | Maintenance and fit |
| --- | --- | --- |
| Sequential batches per unchanged expression, then ordered replay | Small shared scan helper; fewer grep launches; bounded cached results | Selected. Keeps existing Bash/native-tool stack and one reporting path. |
| Parallel pattern scans with buffered replay | More process scheduling, temporary output and synchronization | Adds ordering and failure-handling work before a measured need; do not start here. |
| Persistent cache or changed-files default | Cache invalidation and additional state; can avoid work on later runs | Cannot establish unchanged accepted-corpus performance or preserve default scope. Keep existing explicit staged mode. |

Use Bash 3.2 indexed arrays and native BSD grep/wc/awk. No associative arrays, GNU-only flags, regex-engine substitution, dependency installation or new runtime. Batch each expression over its eligible existing files; use deterministic chunks only when host argument limits require them. Avoid per-file grep during replay. Measure Check 4 before choosing any line-count optimization. A batch counter must keep wc LF-byte semantics, not count an unterminated final line or mistake a total row for a file.

The architecture and command interface remain the same, so no structural diagram is needed. The remaining risk is implementation-level compatibility; executable differential checks are its acceptance evidence.

## Accepted corpus identities

`contract.json` records both a raw `git ls-tree -r -z --full-tree` manifest digest and a length-framed path/mode/type/OID/content-SHA-256 digest. The encoding is explicit so other Actors can reproduce it without a tracked 4,044-row manifest.

| Accepted input | Identity |
| --- | --- |
| Tracked entries / blob bytes | 4,044 / 285,669,775 |
| Modes | 3,594 regular 100644; 25 executable 100755; 425 symbolic links 120000 |
| Raw Git tree manifest SHA-256 | `cc9e627ec4cc9f2ab8fa41b8be66e6ca285f64cf48729600d7aac587da955344` |
| Path/mode/content manifest SHA-256 | `5a4cc80b455f45901c8c70825972150937f2d6c22402f706562247a1b72733b3` |
| Harness blob / mode / bytes | `d90f4626868bd1c6ccaed84200a8f23ac83eef0e` / 100755 / 20,351 |
| Harness SHA-256 | `93140173df8b8bd28b067aff68301134b0398b643240b0b1e6a13da4487ebe33` |

The local harness hash matched the oracle. Symlink metadata in the initial Actor checkout showed 189 links to files, 236 links to directories and no missing referents. The shell's `-f` follows file links; the Git manifest hashes their link text. Benchmark receipts therefore also need before/after identities for the bytes actually read through links and the materialized file-state list. A tree hash alone does not prove dereferenced scan inputs.

Materialize the accepted Git corpus with its original modes and links in isolation. Execute the candidate from an untracked scratch path outside that corpus, using the corpus as cwd. Leave its tracked `scripts/harness/validate.sh` at accepted bytes: it is excluded from scanning anyway. Keep new tests and reports outside the benchmark tracked list. Verify corpus and materialized inputs before and after. Record the final integrated source separately, including uncommitted bytes and modes; do not relabel it as the unchanged benchmark corpus.

## Eligibility and observable order

The exact ordered regex strings and case-glob lists are in `contract.json`; the accepted script hash binds all literal messages. Their source is the accepted harness, not a redesigned policy.

- Selection uses `git ls-files`, or `git diff --cached --name-only --diff-filter=ACM` only when the first argument is `--staged`. Git stderr is suppressed and failures tolerated as originally. Preserve quoted/line-delimited path behavior and the original printed mode. The displayed file count includes selected missing paths. Empty selection exits 0 before coverage.
- Check 1 scans every selected existing `-f` path except its six exact exclusions and nine image/font globs. Its nine secret rules retain their source order. Check 4 exclusions do not apply to this check.
- Check 2 scans shell extensions and workflow YAML, excluding the validator itself. Rule order is absolute-root removal, force-push to protected branch, bare force-push. The protected-branch rule has two OR triggers and a broader reporting expression. A trigger in one file must never activate reporting in another; a second line in the same triggering file can match only the reporting expression and still be printed.
- Check 3 scans `*/domain/*` and `domain/*`, regardless of extension. Order is non-domain-layer import, legacy infrastructure import, Python infrastructure import, Go infrastructure path, Python ORM/SDK import. Only the first rule removes matches containing a `from ./` import. Keep its post-line-number grep filter exactly; legacy and other overlapping matches remain independently reportable.
- Check 4 uses its existing nine code-extension globs and exact exclusions for generated/vendor/test/declaration/managed/canvas paths. The cap is 500 LF bytes, strict greater-than; the grandfathered list is currently empty and ceiling behavior remains intact.
- Check 5 retains `bun run test:coverage`, WORKSPACE_ROOT handling, missing-Bun and nonzero-command messages, and the exact skip warning when SKIP_COVERAGE=1. It still runs after prior violations. The full gate and performance acceptance use no skip flags.

Replay Check 1 through Check 5; within a check, replay files first, then rules, then original matching-line order. Increment once per file/rule with nonempty report matches, not per occurrence or line. Keep overlapping-rule counts. Preserve native grep binary-file output and filename framing. The unused `check_pattern` helper's Perl fallback does not describe the active five checks' extended-regex engine.

Compare stdout and stderr independently as raw bytes, including colors, spaces and blank lines. Preserve each final exit, success suppression and total. Original output is the oracle even where a regex or inherited message looks unusual. A theoretical pattern that BSD grep does not recognize is not permission to repair the policy.

## Amendment 4 and coverage interfaces

The obsolete harness-byte obligations are in `scripts/coverage/config.ts` / `preservation.ts` and `hygiene.ts`, with expectations in `hygiene-preservation.test.ts`. The engineer may replace only those harness-byte equality assertions with executable behavioral equivalence and update their expectations together. Retain harness tracked-path presence, local/index presence, regular-file/non-symlink checks and executable mode 100755. Retain all Rust and migration checks, negative tests and historical baseline identities.

Do not change the historical token-preservation baseline `fbe76a5f15101ad415f355ef06f1deeb1f97d568` or exact-hygiene baseline `aacb3b4359f13c31c0705c0cffc5bf1ac00f387e` to the new mission baseline. Do not rewrite historical receipts. Keep `checkPreservation` in `harness.test.ts` and `checkExactPreservation` in clean-checkout acceptance.

`package.json` routes coverage to `bun scripts/coverage/run.ts`. The collector checks pinned tools and source identity, creates a fresh instrumented target, exercises workspace and explicit child binaries, merges every child profile, validates fresh LCOV plus corroborating LLVM JSON, enforces each crate and publishes a current ignored pointer. Keep all accounting/source-inventory, malformed/stale/missing evidence, profile inclusion and denominator checks. The six crates remain arc-core, arc-detector, arc-storage, arc-ipc, arc-daemon and arc-cli. Each requires its own exact covered × 100 >= total × 80 comparison; no rounded or workspace-wide substitute.

The existing coverage `inputIdentity` hashes Rust/manifests/configuration and `scripts/coverage`; it omits the validator and new `scripts/harness` tests. Retain it and add a separate full integrated-source/candidate binding in mission receipts. It cannot alone prove which optimized harness passed.

## Executable acceptance and evidence handoff

The engineer supplies these exact interfaces; this task creates the contract, not their implementation:

| Interface | Required evidence |
| --- | --- |
| `bun test scripts/harness/equivalence.test.ts` | Isolated deterministic original/candidate fixture matrix; equal stdout bytes, stderr bytes and exits. |
| `bun test scripts/harness/performance.test.ts` | Candidate Checks 1–4 strictly below 20 seconds on unchanged accepted corpus; original/candidate identities and separate Check 5/total. |
| `bun test scripts/harness/coverage-verification.test.ts` | Retained coverage, accounting, preservation and hygiene assertions, including amended harness protections. |
| `bun test scripts/harness/evidence-budget.test.ts` | Fewer than 150 final evidence files, including reports and receipts; no tracked raw exports or runtime artifacts. |

Fixtures cover clean files, every rule and layer, trigger/report differences, overlapping rules and multiple files/lines/occurrences, exclusions with positive controls, missing files, staged ACM/omitted deletions/unstaged content, empty selection, LF boundaries, symlinks, binary-like data and deterministic coverage success/failure/missing-Bun. Generate violation payloads and the original harness at runtime in scratch only. Controlled coverage stubs belong only to equivalence tests and never replace real gates.

Receipts carry accepted commit/tree/corpus, candidate hash, applicable source scope/revision and before/after source identities; exact argv/cwd/effective relevant environment; host and tool paths/versions; UTC labels and monotonic clock; raw stream hashes/byte lengths; exit; status; and diagnostic paths/hashes. The contract lists fields and no measured values are invented.

An external observer concurrently drains both streams and timestamps immediately before spawn, receipt of the complete Check 5 label, and completion. Checks 1–4 is the observed prefix, including startup; Check 5 is the remaining observed interval, explicitly including the unchanged final summary and process/stream-drain overhead. Record label offset, clock/method and boundaries so this is not misrepresented as an instrumented inner coverage duration. Total is the sum. A missing label yields null plus explanation. Run benchmarks and coverage collections serially, record all attempts and run order, and keep expensive competing suites out of timing runs.

The remaining mission checks are coverage CLI and clean-checkout acceptance; `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace`; and full harness acceptance followed by current native-evidence verification. `harness.test.ts` invokes the required exact `./scripts/harness/validate.sh` with SKIP_COVERAGE unset, so its source-bound invocation can supply that same full-command receipt. The independent oma-qa report and final index/final.json remain other Actors' deliverables. Historical passes cannot replace them.

Retain compact results and review under harness-speed, with large native artifacts in existing ignored coverage/target or scratch locations and referenced accurately. Count all final evidence files, including untracked deliverables, and record the tracked count separately. The final index covers Equivalence, Checks 1–4, Check 5 and Independent review. QA covers Verification completeness and Pre-existing hygiene failures. Report inherited failures separately; this task did not execute hygiene or establish a pre-existing failure.

## Prerequisites, verification and handoff risks

Observed tools: Darwin arm64; Bash 3.2.57 at `/bin/bash`; BSD grep 2.6.0-FreeBSD, `/usr/bin/wc` and `/usr/bin/awk`; Git 2.56.0; Bun 1.4.4; Rust/Cargo 1.95.0; cargo-llvm-cov 0.6.21. Rustfmt, Clippy and LLVM tools are installed. Offline dependency availability and matching LLVM binary versions are not yet exercised. No installation is needed for this contract.

A timeout interface mismatch needs Chief/engineer coordination: the framework allows 1,800,000 ms per check, while existing `commands.ts` defaults to 120,000 ms and realAcceptance/clean-checkout use 900,000 ms internally; existing Bun declarations include 960,000 and 1,860,000 ms. New wrappers/replays must allow at least the framework limit, without altering the operator command. This Actor cannot edit helpers outside its two owned files. Do not treat the mission's 120,000 ms planning estimates as termination authority.

Commands performed: scoped `git status` and working/index diffs; accepted/local `git rev-parse`; `git show` for the accepted harness; `git ls-tree` and binary-safe `git cat-file --batch` to compute corpus identities; known-path reads of preservation, accounting, driver, source identity, acceptance and historical architecture interfaces; tool version/path and installed-component queries; symlink metadata inspection. The initial global status attempt had the access failure noted above; all scoped diff/identity queries succeeded. No expensive suite, benchmark, coverage collection, Cargo check, full harness, browser, build or package operation ran.

Deliverable checks passed (Python verifier exit 0): JSON parsing and required-interface assertions; independent reproduction of both complete 4,044-entry corpus manifests and the accepted tree/oracle hash; comparison of every recorded detection/report/filter expression against the accepted shell arguments; and evidence-file count 2, below 150. Scoped final status showed only the two new owned files. `git diff --check -- .arc-verify/harness-speed`, working-source `git diff --exit-code -- scripts Cargo.toml Cargo.lock crates`, and the corresponding cached diff each exited 0. Because these reports are untracked, a direct content check also verifies their trailing whitespace and terminal LF; Git diff alone does not inspect their bytes.

Implementation, timing, full coverage and independent review remain unperformed by this Actor, as assigned. Chief should hand this contract to the engineer and later require review against the actual integrated bytes, not just the Actor's snapshot HEAD.
