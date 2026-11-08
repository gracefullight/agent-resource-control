> Latest handoff: attempt 4 repairs the option-like filename regression. All 79 differential cases and three candidate timing measurements pass. Acceptance remains partial because inherited hygiene gates fail; independent oma-qa is still separately assigned. The final results below supersede the earlier candidate measurements, which remain retained evidence.

# Harness implementation — task-3

Status: implementation delivered; mission acceptance remains blocked by inherited hygiene failures. Owner: Harness Engineer. Independent review and mission closeout belong to later Actors.

Skills used: oma-dev-workflow (existing task/tool inspection and verification) and oma-refactor (characterization seam and behavior-preserving transformation). The task's explicit performance and verification requirements take precedence over the generic refactor profile's performance/commit defaults. No mission state or historical receipts are changed.

Initial inspection: scoped status showed only the three dependency reports under harness-speed; product/index diffs were empty. Accepted oracle and task-1/task-2 reports read. Serena discovery returned no tools; used known-path reads only, with no native project code search. Memory quick pass found no relevant project history and was not used for implementation decisions.

Execution budget: mission duration limit 86,400 seconds, maximum three concurrent tasks, maximum 200 runs and three retries; this Actor runs one implementation process. New check wrappers permit 1,800,000 ms per child. No build, package installation, browser or external delivery is planned. Native Cargo test/Clippy compilation is explicitly requested verification.

Prerequisites: mise tasks --all succeeded and exposed only coverage:provision/test:coverage. package.json has no dependencies; explicit mission Bun/Cargo commands are used unchanged. Bash 3.2 and BSD grep support --null, verified with colon-containing filenames and binary input in external scratch. Existing realAcceptance/collector nested timeouts remain outside ownership and need Chief coordination if encountered; no operator command is changed.

Intermediate checks and introduced issues:

- bash -n scripts/harness/validate.sh passed after the first transformation.
- The first characterization invocation overlapped the transformation and is not valid baseline acceptance. It exposed a fixture PATH omission (cat needed by candidate); the fixture PATH was corrected and a fresh run started. All attempted results remain described here; no failed output is represented as passing evidence.
- A process inventory attempt (ps) was denied by the environment. No GUI or access workaround was attempted.

Changes/results and concrete acceptance receipts will be added before handoff. No pre-existing hygiene failure has yet been established.

## Verified intermediate results

- Full deterministic differential matrix passed: two Bun tests, zero failures, exact stdout/stderr/exit/coverage-call equality, including binary/colon framing. First complete passing run took 85.36 seconds. Receipt is regenerated against final sources before handoff.
- Native grep invocation scaling passed for 2 versus 80 files. The strengthened trace fixture also exercises matched-file reporting and will run again before handoff.
- Non-collecting accounting, gate and preservation tests passed. Hygiene test failed on its existing evidence-count guard: 932 indexed rust-coverage files must be <150. Three other hygiene tests passed. The serial inventory runner was adjusted to execute every listed test even when one fails, keeping all individual exits and failing the aggregate.
- Both accepted f85ae043 and HEAD list exactly 932 rust-coverage paths; the current index count is also 932. This failure predates implementation. See pre-existing.json for commands and bound diagnostics. No historical artifact was altered or removed.
- The new amendment-negative fixture initially passed an uncanonicalized /var temporary root to sourceInventory, which compares canonical paths. The fixture now uses realpathSync; all preservation tests pass with their original production logic intact.
- BSD awk truncates records at NUL, confirmed by a scratch probe. The first framing implementation lost matches; scan_frame now parses native grep framing in Bash before mapping rows in awk. Targeted tests then passed for secrets, force-push split, architecture overlap and binary/prefix ambiguity. Earlier failed comparisons are not acceptance evidence.

## Integration boundary

No manifests, lockfiles, Rust source, migrations, coverage accounting/collection code or historical receipts are changed. Historical baseline IDs and protected-reference hashes remain intact. Amendment 4 exempts only harness byte comparisons; checkPreservation still runs and now also calls a harness-only structural/path/mode guard. The existing Rust token/addition behavior is retained; exact Rust/migration guards remain in checkExactPreservation. All 84 protected entries remain inventoried. Existing Rust/migration negative tests are retained and new harness-byte acceptance, executable mode, symlink and migration-byte rejection tests are added.

The timing runner creates an isolated accepted Git tree, verifies every blob/mode/link and dereferenced scan identity before/after, and runs the candidate from an untracked sibling path. Its tracked harness remains the accepted excluded corpus entry. It records startup, Checks 1–4, Check 5, finalization, total and conservative process-start-to-Check-5 measurements. No mock or skip flag is used for timing.

## Final differential and non-collecting checks

`bun test scripts/harness/equivalence.test.ts` passed (2 tests, 0 failures, 90.18 seconds). All 61 isolated cases compare stdout bytes, stderr bytes, exact exits and coverage invocation records. Raw per-case streams/traces are retained in the scratch directories listed in equivalence.json. The final matrix includes a tracked harness path, unreadable grep input, every rule and exclusion group, multiple files/lines/occurrences, binary records before/between/after text, filename-prefix ambiguity, symlinks, LF boundaries, staged additions/modifications/copies/deletions, unstaged repairs/violations, empty input, child success/failure, absent Bun and explicit skip equivalence only.

Native grep pass-through tracing on matching files: 2 files → original 38 / candidate 20 calls; 80 files → original 1,520 / candidate 20 calls. This covers reporting as well as detection. Native grep executes each traced call; no matching engine is mocked.

`bun test scripts/harness/coverage-verification.test.ts` ran all five non-collecting files serially. accounting.test.ts, gate.test.ts, hygiene-preservation.test.ts and preservation.test.ts exited 0. hygiene.test.ts exited 1 solely on the inherited rust-coverage count guard; its other three tests passed. The aggregate correctly exits 1. The discovered nine-file inventory matches the immutable accepted suite; coverage-cli, clean-checkout, harness and evidence tests are explicitly assigned separate live verification, not represented as executed by this aggregate.

`bash -n scripts/harness/validate.sh`, scoped `git diff --check -- scripts`, and independent accepted-corpus manifest reproduction passed. The manifest has 4,044 entries and reproduces both task-1 hashes. The snapshot mutation probe changes README.md in scratch and confirms that the corpus guard rejects it, then restores that deliberate probe before measurements. Coverage mutations are never restored or excluded to manufacture preservation.

The first full original run had exit 0, six coverage passes, Checks 1–4 146.505637 s, startup 0.761962 s, Check 5 79.935597 s, finalization 0.001100 s, total 227.204296 s. Strict after-run verification then failed on .arc-verify/rust-coverage/baseline/latest.json, a tracked accepted blob rewritten by the existing collector. attempts.json preserves this failed attempt and raw diagnostic binding. The final benchmark uses a fresh accepted snapshot for each full run and retains all four outcomes even if this inherited mutation makes the test fail.

Remaining collection constraint: running the root coverage CLI or full harness would rewrite that same tracked historical/runtime pointer, outside this Actor's file ownership and contrary to the required historical preservation. Those root collections require Chief coordination; no pointer cleanup or collector edit is performed here. Clean-checkout acceptance independently checks the inherited hygiene count before collecting. This is an observed path/content conflict, not a speculative safety restriction.

## Recovery attempt 3

The updated task allows 3,600,000 ms per wrapped verification command. `test-support.ts` now uses that allowance, reduced only by an explicitly supplied absolute `HARNESS_VERIFICATION_DEADLINE_MS`. This run starts at approximately 2026-11-08T06:54Z with a 3,600-second Actor limit; measurements and collections remain serial. Existing earlier receipts and fixtures are retained. Staged and working implementation changes were inspected and reconciled by staging only owned implementation paths; task-1/task-2 outputs are unchanged.

The retained profile measured 27.925 seconds in Check 1 out of a 30.238-second scan, with each secret expression taking roughly 2.7–3.4 seconds. That evidence justifies the recovery instruction's bounded three-pattern workers. Regex arguments, eligibility and native grep remain unchanged; parent-only replay preserves file/rule/line order. The saved goalBrief and task-1 contract are not edited.

Benchmark correction: the first original run revealed that the accepted collector rewrites its tracked `baseline/latest.json` at runtime. The corrected benchmark audits every accepted tracked path, blob, mode and dereferenced scan input before execution and at the Check 5 marker. It retains a complete post-run deviation list, permits only the collector's documented pointer change after the scan, and binds that pointer's before/after hashes to the actual run ID, source hash, summary and receipt. Symlink referent identities must remain unchanged. Each measurement uses a fresh snapshot. No mutation is restored or concealed after a measured run. Earlier `timing.json` bytes are retained as `timing-before-recovery.json`; failed historical attempts remain in `attempts.json`.

The prior statement that root collection required further permission is superseded by explicit recovery instruction 5: normal collector runtime outputs are authorized. No collector, historical contract or count assertion is amended. The inherited 932-path evidence-hygiene failure remains a failing acceptance prerequisite.

## Refreshed equivalence and timing

Candidate SHA-256: `fbde643dce5ab9af3041bc33eb895114cba3f5190c4f64abc64920503cacffd3`. Oracle SHA-256: `93140173df8b8bd28b067aff68301134b0398b643240b0b1e6a13da4487ebe33`. Final relevant source/helper bindings are embedded in the receipts.

`bun test scripts/harness/equivalence.test.ts`: exit 0, 2 tests, all 61 differential cases passed; exact stdout/stderr/exit/coverage-call comparisons. Total 93.74 seconds. Matching-file native invocation traces remain 38→20 calls at 2 files and 1,520→20 calls at 80 files.

`bun test scripts/harness/performance.test.ts`: exit 0, 1 test, 50 assertions, 533.18 seconds. One original plus three candidate full commands ran serially, without mocks or skip flags. Every command exited 0, passed all six 80% crate gates, preserved all scan inputs through the Check 5 boundary, and had only its bound runtime pointer change after collection.

| Run | Startup s | Checks 1–4 s | Start to Check 5 s | Check 5 s | Finalization s | Total s |
|---|---:|---:|---:|---:|---:|---:|
| original | 0.946586 | 145.793088 | 146.739674 | 83.620222 | 0.002155 | 230.362051 |
| candidate-1 | 0.716026 | 13.702007 | 14.418033 | 81.910536 | 0.007276 | 96.335845 |
| candidate-2 | 0.867203 | 13.319978 | 14.187181 | 81.183949 | 0.006661 | 95.377791 |
| candidate-3 | 0.645169 | 13.375995 | 14.021164 | 81.335947 | 0.005585 | 95.362696 |

Every candidate scan and conservative process-start-to-Check-5 measurement is strictly below 20 seconds. These measurements establish elapsed-time improvement on this host and corpus; no financial or usability outcome is inferred.

The accepted 4,044-entry corpus remains bound to tree-manifest `cc9e627ec4cc9f2ab8fa41b8be66e6ca285f64cf48729600d7aac587da955344` and path/mode/content-manifest `5a4cc80b455f45901c8c70825972150937f2d6c22402f706562247a1b72733b3`. Candidate execution from the sibling scratch path is the substitution interface already recorded by task-1; the accepted excluded harness entry stays unchanged.

## Final implementation handoff

Status: partial acceptance. Batching, deterministic equivalence, invocation scaling and the three required speed measurements pass. The inherited evidence-hygiene state still prevents complete acceptance. Independent oma-qa has not been performed by this Actor.

Owned product paths changed:

- `scripts/coverage/config.ts`
- `scripts/coverage/hygiene-preservation.test.ts`
- `scripts/coverage/hygiene.ts`
- `scripts/coverage/preservation.ts`
- `scripts/harness/benchmark.ts`
- `scripts/harness/coverage-verification.test.ts`
- `scripts/harness/equivalence.test.ts`
- `scripts/harness/evidence-budget.test.ts`
- `scripts/harness/fixtures.ts`
- `scripts/harness/performance.test.ts`
- `scripts/harness/test-support.ts`
- `scripts/harness/validate.sh`

The four acceptance test files are executable Bun tests. `coverage-verification.test.ts` discovers the complete nine-file existing suite, executes five named non-collecting files serially, and inventories the four separately invoked live checks. `evidence-budget.test.ts` enforces recursive file count, size, native-artifact exclusion and relevant current source/helper bindings before retaining the existing hygiene audit. No assertions or thresholds are bypassed.

Compatibility: production remains a self-contained Bash 3.2 script using the existing native grep/wc/awk/xargs utilities. The regex arguments, exact eligible lists, exclusions, messages, ANSI bytes, streams, file/rule/line ordering and counts are preserved by the passing differential matrix. Force-push triggers remain separate from reporting; within-domain filtering applies only to its original rule. Check 4 keeps native wc newline counting, ceilings and exclusions. Its measured original cost was 1.915 seconds, so no additional line-count transformation was needed.

Amendment 4 changes only harness byte-equality obligations. Harness path/type/executable mode checks and `checkPreservation` remain active. Rust and migration local/index byte/path/mode guards, existing negative cases, accounting logic and six independent exact 80% floors remain. New tests prove authorized harness byte changes pass and mode, symlink and migration mutations fail.

| Required command | Exit | Seconds | Outcome |
|---|---:|---:|---|
| `bun test scripts/harness/coverage-verification.test.ts` | 1 | 19.966 | inherited hygiene count failure |
| `bun test scripts/coverage/coverage-cli.test.ts` | 0 | 82.266 | passed |
| `bun test scripts/coverage/clean-checkout.test.ts` | 1 | 1.676 | inherited hygiene count failure |
| `cargo fmt --all -- --check` | 0 | 0.304 | passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 8.974 | passed |
| `cargo test --workspace` | 0 | 63.879 | passed |
| `bun test scripts/coverage/harness.test.ts` | 0 | 96.850 | passed |
| `bun test scripts/coverage/evidence.test.ts` | 0 | 0.397 | passed |
| `bun test scripts/harness/evidence-budget.test.ts` | 1 | 1.714 | inherited hygiene count failure |

The non-collecting inventory individually recorded accounting **12 pass**, gate **14 pass**, hygiene-preservation **3 pass**, preservation **1 pass**, and hygiene **3 pass / 1 fail**. The hygiene failure is `hygiene.test.ts`’s “final index has bounded evidence and no prohibited artifact anywhere under .arc-verify” at `hygiene.ts:70`. Clean-checkout fails at its initial root audit (`clean-checkout.test.ts:15`), so its fresh-collection assertions remain unexecuted. Evidence-budget passed its first 89 assertions and fails at its inherited `assertHygiene` call (`evidence-budget.test.ts:36`). These are failed checks, not waived acceptance.

The refreshed audit confirms **932** indexed Rust coverage paths, byte/mode/OID-identical to accepted HEAD (manifest `220c4fe94bc7183e56701454e9b8bae4d7d2dd3edbb0616816e12cd0e74ee934`). The complete audit additionally inventories **457** prohibited indexed paths. `git diff --cached --exit-code f85ae0430038cc3d7a6125f650872dbfce6b5529 -- .arc-verify` exits 0, so these indexed hygiene inputs predate task-3. No artifact was filtered from the audit, deleted, relabeled or rewritten to obtain a pass.

Root coverage CLI and integrated harness collections wrote their documented runtime latest pointer normally. `recovery-checks.json` retains its before/after hashes, actual run IDs, source hashes, summary bindings and command diagnostics. The latest integrated native-evidence test passed after the final harness collection. The index retains the historical pointer bytes; the working pointer is a normal authorized runtime output.

Scope verification: Cargo manifests/lockfile, package.json, mise.toml and all crate sources/migrations are unchanged against HEAD. Owned working product bytes equal their staged bytes. Scoped cached whitespace checks pass. Existing task-1 and task-2 reports are unchanged; their hashes are in `recovery-observations.json`. Global cleanliness is not claimed because the earlier denied av.yaml status access remains outside this scope. No browser, package download, production build, external delivery or mission-state mutation was performed. All owned subprocess sessions completed; generated scratch fixtures, snapshots, logs and native artifacts are retained at receipt paths.

Evidence: **15 files** currently under harness-speed, including untracked and nested files, below 150. Compact durable receipts: `equivalence.json`, `invocations.json`, `timing.json`, `coverage-verification.json`, `recovery-checks.json`, `recovery-observations.json`. Earlier failed/interrupted evidence remains in `attempts.json`, `timing-before-recovery.json`, `profile.json`, `pattern-profile.json` and `pre-existing.json`. Raw logs/native exports remain in existing target or external scratch locations.

Readability verdict: acceptable for this scope. One shared scan cache serves Checks 1–3, with parent-only ordered reporting and explicit native binary/NUL framing. The harness grows from 444 to 577 lines; that buffering cost buys pattern/batch invocation scaling and measured scans of 13.320–13.702 seconds, versus 145.793 seconds originally. Further restructuring is deferred.

Chief handoff: continue the existing independent oma-qa task using these current source-bound receipts. Mission completion still requires coordinated resolution of the inherited `.arc-verify` hygiene inputs and successful replays of the failed frozen checks. Task-3 does not own that cleanup, historical deliverable changes, final `qa.md`, `index.md` or `final.json`; none is fabricated here. No ownership transfer or operator decision is requested.

## Option-like filename repair

Attempt 4 uses oma-dev-workflow for the assigned optimization and oma-refactor's characterization and behavior-preservation method. This is a repair within the existing performance assignment; no refactor commit, new mission or ownership transfer was created. Serena is absent from the current tool catalog, so inspection uses supplied known-path reads and Git metadata, without native repository code discovery. `mise tasks --all` exposes only `coverage:provision` and `test:coverage`; the frozen acceptance commands therefore run directly as assigned. Installed pinned tools are reused.

The retained staged implementation was inspected before changes. Only `scripts/harness/validate.sh` and `scripts/harness/fixtures.ts` require further product edits for the reported regression. The old receipts are retained in `before-option-repair.json`. `option-regression.json` records the extended matrix failing against the unrepaired candidate, including invalid options mixed with ordinary violations. This establishes the regression before production repair.

The repair separates leading-dash paths from shared native batches. These entries use the accepted single-file trigger/report arguments at their original position in ordered replay. Replay reads the original FILES here-string so grep options that consume stdin retain the oracle's traversal behavior; adding `--` globally would change that behavior. Ordinary entries continue to use unchanged native expressions over bounded xargs batches. The shared eligibility predicate is used for preparation and replay with the original exclusions. Force-push trigger short-circuiting and its separate reporting expression remain intact, as does the domain rule's exclusive filter.

The matrix adds 18 cases: invalid options, quiet mode, option terminator, stdin, filename-prefix/list/invert/expression/file options, help/version output, include options, shell/domain paths, staged selection and excluded suffixes, each alongside ordinary violations. Existing fixtures remain. Both stdout and stderr are compared as bytes, with exact exit and coverage-call equality.

Validation is complete for this Actor; the acceptance result is partial. Updated command receipts are `option-repair-equivalence.json`, `option-repair-performance.json`, and `option-repair-checks.json`; raw diagnostic streams remain under `target/harness-speed/option-repair/` or external scratch. No historical coverage receipts, manifests, lockfiles, Rust sources, migrations, collector logic, accounting or thresholds were changed. A scoped Git comparison confirms those files still match HEAD, and the indexed `.arc-verify` tree remains identical to accepted HEAD. The inherited hygiene failures remain required failures until separately resolved within coordinated ownership.

### Final attempt-4 results

The extended characterization reproduced 15 mismatching cases before repair (`option-regression.json`). After repair, all **79** cases pass exact stdout/stderr/exit/coverage-call comparison. `bun test scripts/harness/equivalence.test.ts` exits 0 with 2 passing tests and 10 assertions in 122.711 seconds. Real grep invocation tracing stays at **20** candidate invocations for 2 and 80 eligible inputs, while the oracle grows from 38 to 1,520. A separate retained trace assertion confirms nine invalid-option native calls use exactly their original `-qE` arguments and never include an ordinary file argument (`option-repair-trace.json`). The original stdin-consuming behavior of `-q`, `--` and `-` also matches byte-for-byte.

`bun test scripts/harness/performance.test.ts` exits 0 with 1 passing test and 50 assertions in 557.129 seconds. The one original and three candidate full commands ran serially, with real native coverage, fresh accepted snapshots and no skip flags. All four exited 0, passed six per-crate 80% floors, preserved accepted scan inputs through the Check 5 boundary, and had only their bound runtime latest pointer change afterward.

| Run | Startup s | Checks 1–4 s | Start to Check 5 s | Check 5 s | Finalization s | Total s |
|---|---:|---:|---:|---:|---:|---:|
| original | 0.723503 | 159.002406 | 159.725909 | 94.632839 | 0.001746 | 254.360494 |
| candidate-1 | 0.780635 | 14.476503 | 15.257139 | 79.759842 | 0.007143 | 95.024124 |
| candidate-2 | 0.516966 | 14.485945 | 15.002911 | 80.953849 | 0.005566 | 95.962326 |
| candidate-3 | 0.535555 | 14.733389 | 15.268944 | 80.745241 | 0.006057 | 96.020242 |

Every candidate scan is below 20 seconds, as is every conservative process-start-to-Check-5 measurement. Check 4 remains unchanged; its candidate measurements are 1.983–2.011 seconds, so no line-count transformation was needed. The earlier measured Check 1 bottleneck still justifies the retained three-pattern worker limit and parent-only ordered replay, as authorized in recovery clarification 3. The original sequential planning assumption and accepted task-1 artifacts have not been rewritten.

| First implementation check | Exit | Seconds | Outcome |
|---|---:|---:|---|
| `bun test scripts/harness/coverage-verification.test.ts` | 1 | 16.949 | inherited hygiene failure |
| `bun test scripts/coverage/coverage-cli.test.ts` | 0 | 89.241 | passed |
| `bun test scripts/coverage/clean-checkout.test.ts` | 1 | 2.137 | inherited hygiene failure |
| `cargo fmt --all -- --check` | 0 | 0.354 | passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 0.350 | passed |
| `cargo test --workspace` | 0 | 43.572 | passed |
| `bun test scripts/coverage/harness.test.ts` | 0 | 96.927 | passed, full unskipped script invoked |
| `bun test scripts/coverage/evidence.test.ts` | 0 | 0.408 | passed after final collection |
| `bun test scripts/harness/evidence-budget.test.ts` | 1 | 1.852 | inherited hygiene failure |

After the first check sequence, inspection found an evidence-counter defect: concurrent wrapper writes can leave adjacent newline separators, which the old `.filter(Boolean)` count dropped. The raw trace contains 20 native invocations, not 19. The test now counts every separator for its fixed newline-free scaling arguments; no assertions were removed. `trace-count-correction.json` retains the earlier receipts. The refreshed equivalence matrix passes all 79 cases and reports 20 invocations at both sizes. Production candidate and benchmark helper/test bytes remain unchanged from the passing performance run, verified by their per-file bindings.

The following final checks ran serially after that test-only correction and staging. Cargo and coverage CLI results above remain current for their unchanged inputs.

| Final staged check | Exit | Seconds | Outcome |
|---|---:|---:|---|
| `bun test scripts/harness/coverage-verification.test.ts` | 1 | 22.941 | inherited hygiene failure |
| `bun test scripts/coverage/clean-checkout.test.ts` | 1 | 1.990 | inherited hygiene failure |
| `bun test scripts/coverage/harness.test.ts` | 0 | 106.025 | passed |
| `bun test scripts/coverage/evidence.test.ts` | 0 | 0.428 | passed |
| `bun test scripts/harness/evidence-budget.test.ts` | 1 | 1.813 | inherited hygiene failure |

`option-repair-finalchecks.json` preserves these exact command receipts and the final collection pointer transitions. The native-evidence pass follows the final integrated harness collection.

The non-collecting inventory covers all nine existing coverage test files through five explicitly executed files and four separate required live checks. Individual current results: accounting **12 pass**, gate **14 pass**, hygiene-preservation **3 pass**, preservation **1 pass**, hygiene **3 pass / 1 fail**. The failed hygiene case is “final index has bounded evidence and no prohibited artifact anywhere under .arc-verify” (`scripts/coverage/hygiene.test.ts:11`), failing at `scripts/coverage/hygiene.ts:70`. Clean-checkout fails its initial root audit (`scripts/coverage/clean-checkout.test.ts:15`), so its fresh-collection assertions remain unexecuted. Final evidence-budget reaches **109 assertions** before the same inherited audit fails (`scripts/harness/evidence-budget.test.ts:36`); the harness-speed count, compactness and current relevant source bindings have already been checked at that point. These failures remain failed requirements.

The final audit reproduces **932** Rust coverage evidence paths in both accepted HEAD and the index, with identical path/mode/OID manifests, and **457** prohibited indexed evidence paths. Indexed `.arc-verify` bytes remain identical to accepted HEAD (`git diff --cached --exit-code f85ae0430038cc3d7a6125f650872dbfce6b5529 -- .arc-verify`, exit 0). No historical artifacts were deleted, filtered, relabeled or rewritten. The prerequisite for full mission acceptance remains Chief-coordinated authorized resolution of those inherited hygiene inputs and successful unchanged required replays. No ownership handoff or cleanup was performed by this Actor.

The final root integrated harness passed all original checks with zero violations and no skip flags. The subsequent native-evidence test passed against its fresh actual collection. `option-repair-checks.json` records each root runtime pointer's before/after hashes and actual collection IDs; `option-repair-observations.json` binds the final pointer, summary and raw native evidence hashes. The pointer remains an authorized generated working output; it was not manually rewritten or staged.

### Final source, ownership and evidence

Candidate SHA-256: `0e66d222504f469cc9be910e1c7a6e33544440f017809e25cc255f6dc90f29a8`. The accepted oracle SHA-256 remains `93140173df8b8bd28b067aff68301134b0398b643240b0b1e6a13da4487ebe33`. All current equivalence, invocation, timing and coverage-verification receipts bind this candidate and their relevant helper/test sources. `option-repair-observations.json` records the complete integrated source identity and final indexed/working equality.

Attempt-4 product edits are `scripts/harness/validate.sh`, `scripts/harness/fixtures.ts`, and `scripts/harness/equivalence.test.ts`. The full retained task-3 owned change set remains the twelve paths listed in “Final implementation handoff” above; all are staged and their working bytes match the index. The four harness acceptance test files and preservation amendments remain intact. No shared manifest, lockfile, Rust source, migration, accounting/collection logic, exact threshold or historical coverage receipt changed. Accepted task-1 and task-2 deliverables retain their initial hashes.

New material evidence: `before-option-repair.json`, `option-regression.json`, `option-repair-equivalence.json`, `option-repair-performance.json`, `option-repair-trace.json`, `option-repair-checks.json`, `option-repair-observations.json`, `trace-count-correction.json`, and `option-repair-finalchecks.json`. The mutable implementation report and four current acceptance receipts are refreshed; prior versions remain in the retained pre-repair receipt. Raw command streams, fixture traces, native outputs and the executable command runner remain in the recorded external scratch or ignored target locations. The evidence set contains **24 files**, including untracked and nested files, below 150. No `qa.md`, `index.md`, or `final.json` was fabricated by this Actor.

Additional executed checks: `bash -n scripts/harness/validate.sh`, scoped `git diff --check`, scoped cached whitespace checks, working/index equality, unchanged excluded-source comparisons and dependency-deliverable hash checks all passed. `mise tasks --all` passed and confirmed the frozen verification commands have no corresponding mise tasks. No tools were installed or downloaded. Host-wide process inventory was unavailable because `ps -Ao pid,ppid,etime,command` was denied by the shell (exit 127); this Actor launched no competing verification, and every observed owned command session exited. No browser, production build, external delivery or mission-state mutation was performed.

Readability verdict: acceptable within the required shell interface. The repaired script is 612 lines versus the accepted 444 and retained pre-repair 577. The additional 35 lines isolate option parsing and retain original stdin traversal; eligibility is shared rather than duplicated between preparation and replay. No runtime dependency or second regex engine was added.

Chief handoff: the reported option-path regression is repaired and staged with refreshed passing behavior and speed evidence. Acceptance remains **partial** due to the three inherited hygiene failures and the separately assigned independent oma-qa review. Continue the existing review using the current receipts; do not treat this implementation report as independent acceptance.


## Attempt-6 authorized recovery

Status: task-3 recovery passed; ready for the saved independent review. Mission-wide completion remains with the Chief and tasks 4–6. No independent verdict, qa.md, final.json or mission-state change is written by this Actor.

Skills used: oma-dev-workflow for recovery and exact serial command execution; oma-refactor for source and behavior preservation. Serena tools were unavailable; all source inspection used supplied known paths. No browser, subagent, package installation, production build or external delivery was used. The required Bun commands have no matching mise task; mise tasks --all confirms the existing two-task configuration.

## Changes and authorization

The operator authorization recorded at 2026-11-08T07:55Z supersedes attempt-5's coordination request. The actual enumerated index had 932 Rust coverage paths and 457 prohibited artifacts. The whitelist was derived from retained[].destination in .arc-verify/evidence-hygiene/retention.json, with 24 unique destinations verified present.

Nine bounded explicit git rm --cached --ignore-unmatch -- invocations untracked exactly 908 non-whitelisted paths. All prohibited paths were among those removals; none outside rust-coverage required a further removal. No file was deleted from disk. The exact removed path/mode/OID manifest is retained at target/harness-speed/attempt-6/removed-manifest.json with SHA-256 5a400edd202c5b2e554a0745ede26e8bcfab7ea4641c713b0a5cfe6c2ba7dbcc. Its full identity and before/after audits are in attempt-6-recovery.json.

After verification, all 908 removed local files still match their pre-operation sizes, modes and SHA-256 hashes. All 24 retained index entries and local files are unchanged. Comparing the full index to the saved pre-operation index minus exactly the enumerated removals confirms unrelated index changes were preserved. The final audit reports 24 Rust coverage files, 475,720 bytes and zero prohibited indexed artifacts. Exact Rust/migration preservation and harness structure checks pass for all 84 protected entries.

The immutable accepted commit f85ae0430038cc3d7a6125f650872dbfce6b5529 and its 4,044-entry corpus remain unchanged (Git manifest SHA-256 cc9e627ec4cc9f2ab8fa41b8be66e6ca285f64cf48729600d7aac587da955344). Index cleanup affects delivery only; no benchmark was rerun against a smaller corpus. Historical receipts and accepted task-1/task-2 deliverables were not edited.

This attempt changes only the evidence index and recovery documentation. The twelve existing staged implementation files remain unchanged:

- scripts/harness/validate.sh
- scripts/harness/benchmark.ts
- scripts/harness/coverage-verification.test.ts
- scripts/harness/equivalence.test.ts
- scripts/harness/evidence-budget.test.ts
- scripts/harness/fixtures.ts
- scripts/harness/performance.test.ts
- scripts/harness/test-support.ts
- scripts/coverage/config.ts
- scripts/coverage/hygiene.ts
- scripts/coverage/preservation.ts
- scripts/coverage/hygiene-preservation.test.ts

Working/index bytes agree for all twelve. New durable outputs are attempt-6-recovery.json, attempt-6-checks.json, attempt-6-handoff.md and the generated budget.json; implementation.md is appended with this recovery. Prior report/coverage-verification receipt bytes and full manifests/diagnostics remain in target/harness-speed/attempt-6/. Previous failed receipts and attempt-5-handoff.md remain intact.

## Current command evidence

Each wrapper permits 3,600,000 ms per check. The following commands ran serially, with no benchmark or competing coverage collection:

| Command | Exit | Seconds |
|---|---:|---:|
| `bun test scripts/coverage/hygiene.test.ts` | 0 | 0.815 |
| `bun test scripts/harness/coverage-verification.test.ts` | 0 | 15.080 |
| `bun test scripts/coverage/clean-checkout.test.ts` | 0 | 86.445 |
| `bun test scripts/harness/evidence-budget.test.ts` | 0 | 0.725 |

The hygiene suite passed 4 tests and 214 assertions. The non-collecting aggregate passed and ran every listed child: accounting, gate, hygiene-preservation, hygiene and preservation. Its nine-file inventory still includes the four separately required coverage CLI, clean-checkout, integrated harness and native-evidence files. The budget test passed 119 assertions.

Clean-checkout passed 31 assertions. It materialized index tree f25af4f19afa14a9208ad109795490a6cf0cfaf1, verified no inherited targets/runs/profiles/latest pointer or raw exports, and matched source identity 91b009125526827cd072e922cb1cafcede1ec5d6b2a09ee16dba0ab98dbd0379. It then executed bun run test:coverage and bun test scripts/coverage/evidence.test.ts in the isolated checkout; both exited 0. All six crate results passed their independent exact 80% floors. Current native LCOV/LLVM hashes, crate accounting, child receipts and diagnostics are linked in attempt-6-recovery.json. Fresh checkout: /var/folders/r9/0qvpcx794n385zwgzwfvt55h0000gn/T/arc-clean-coverage-KO8gci. Full child proof: /Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/92ab1985-9682-4e04-a416-1f3ab9a6bc3a/workspace/target/evidence-hygiene/clean-checkout/runs/f59c0ac9-443b-4537-a66a-00b4ee63dc42/result.json.

One scratch report-finalizer invocation exited 1 with a parse error before executing; its extra closing parenthesis was corrected, then the unchanged final checks and report generation completed. No product/index mutation occurred in the failed invocation. Additional checks passed: bash -n scripts/harness/validate.sh; scoped git diff --cached --check; final complete index comparison; local file hash/mode comparison; protected-source preservation; working/index equality; current source binding checks for equivalence.json, invocations.json and timing.json.

## Preserved equivalence and timing evidence

No production/test/helper input changed in this attempt. Current relevant source bindings were verified before reusing the passing 79-case exact stdout/stderr/exit fixture receipt, native grep scaling receipt (38/20 calls at 2 files; 1,520/20 at 80 files) and full accepted-corpus timing receipt. Candidate SHA-256 remains 0e66d222504f469cc9be910e1c7a6e33544440f017809e25cc255f6dc90f29a8.

| Run | Checks 1–4 s | Check 5 s | Total s | Exit |
|---|---:|---:|---:|---:|
| original | 159.002406 | 94.632839 | 254.360494 | 0 |
| candidate-1 | 14.476503 | 79.759842 | 95.024124 | 0 |
| candidate-2 | 14.485945 | 80.953849 | 95.962326 | 0 |
| candidate-3 | 14.733389 | 80.745241 | 96.020242 | 0 |

The three candidate scans remain below 20 seconds. Startup, conservative process-start-to-Check-5 and finalization remain separately recorded in timing.json. Real native coverage ran without skip flags in every measurement. Bash 3.2/native grep interfaces, ordering/counting, original patterns/exclusions, option/error/binary behavior and the separate force-push expressions remain covered by the unchanged matrix. No dependency, Rust source, migration, manifest, lockfile, accounting/collection logic or threshold changed. Earlier Cargo formatting, Clippy, workspace tests, coverage CLI and integrated unskipped harness passes remain in option-repair-checks.json and option-repair-finalchecks.json for their unchanged inputs; they are not represented as freshly rerun here.

## Pre-existing hygiene failures and remaining work

The prior failures were caused by the inherited snapshot's 932 indexed coverage artifacts, including 457 prohibited artifacts. They are preserved as historical failed evidence in earlier receipts and are now resolved by the authorized index removals and fresh passing checks. This attempt reproduced no remaining check failure or ownership/interface conflict.

The evidence directory contains 30 files including untracked and nested evidence, below 150. The standalone budget receipt counts the complete current set. All observed owned command sessions exited.

Readability verdict: the existing batching implementation is unchanged; this recovery adds no scanning complexity. Metric delta for this attempt: indexed Rust coverage evidence 932 → 24; prohibited indexed artifacts 457 → 0; retained local historical files deleted 0.

Chief handoff: task-3's inherited hygiene blocker is resolved. Continue the existing independent oma-qa and tasks 4–6 with their original checks and 3,600,000 ms allowance. Remaining acceptance risk is the unperformed independent/integrated final review; this implementation handoff does not claim that review or mission completion.
