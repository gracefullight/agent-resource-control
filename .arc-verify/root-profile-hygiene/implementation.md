# Root profile hygiene

Status: the authorized harness and coverage run passed; the final section supersedes earlier gate and budget status.
Skills used: `oma-dev-workflow` and `oma-debug`.

- At HEAD `e0875e7449c4e5d6752dfd1b6508c408ebd42ed3`, the index contains only the four requested profile deletions. Each 383,272-byte worker file, task-1 backup, destination file and accepted-HEAD blob was byte-for-byte equal to the captured baseline; their modes were 0644. The destination has not been integrated.
- `.gitignore` differs from HEAD only by `/*.profraw`, `/*.profdata` and `/*.lcov`. A NUL-delimited scan of the full index found zero tracked files with those extensions. The existing ignore test now includes the three root probes and retains the existing .arc-verify and docs/fixtures cases.
- No changes remain in `scripts/coverage/hygiene.ts`; scoped Git checks found no changes to crates, the harness, the coverage runner or listed manifests and lockfiles. The separate task-1 profile backups remain at `.agents/results/root-profile-hygiene/profile-backups/`.

## Checks

| Command | Result |
| --- | --- |
| `bun test scripts/coverage/hygiene.test.ts` | Exit 0; 4 tests and 220 assertions passed. |
| `bun test scripts/coverage/hygiene.test.ts scripts/coverage/hygiene-preservation.test.ts .arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts` | Exit 0; 7 tests and 240 assertions passed across **two** files. Bun did not run the absent mission test. |
| `bun test .arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts` and `bun test .arc-verify/root-profile-hygiene/budget.mission.test.ts` | Exit 1 each: no matching test file. |
| `bun test .arc-verify/root-profile-hygiene/gate-proof.mission.test.ts scripts/coverage/evidence.test.ts` | Exit 1: only the existing evidence test ran; `.arc-verify/rust-coverage/baseline/latest.json` was absent. |
| `git diff --check -- .gitignore scripts/coverage/hygiene.test.ts scripts/coverage/hygiene.ts` and `git diff --cached --check` | Exit 0. |
| `mise tasks --all` | Exit 0; lists `coverage:provision` and `test:coverage`. |

The unchanged harness invokes the coverage runner, which compiles a fresh Cargo target. The task and `.agents/skills/_shared/core/execution-policy.md` prohibit that without an explicit build request. Neither gate was run or claimed as passed. Independent QA and destination integration remain with the Chief.

The operator note requires deleting the three mission-only tests and forbids adding receipts and gate proofs; the tests have been removed. The unchanged verification contract still refers to those paths and `gate-results.json`, so it cannot pass in this state. The existing task-1 `.arc-verify/root-profile-hygiene/baseline.json` was preserved because it is another Actor's required baseline. The instruction to retain only a short note in that directory conflicts with its ownership and dependency; the Chief must coordinate any change to it. The task-1 backup must remain accessible through integration.

`git status` and `stat av.yaml` reported `Operation not permitted`; global worktree cleanliness is not established. Serena was not exposed, so inspection used known paths and Git metadata.

## 2026-11-10 contract restoration

This update supersedes the earlier statements that the contract tests are absent and that the later operator note prohibits them. The corrected note requires the three mission tests and `gate-results.json`; all four now exist. Skills used: `oma-dev-workflow` and `oma-debug`.

- `profile-preservation.mission.test.ts` checks all four files in the mission worktree and original checkout, their separate backups, baseline sizes, modes and SHA-256 values, and accessible accepted-HEAD blobs. It also checks the index and compares scoped product, harness, coverage and configuration paths with accepted HEAD, excluding the intended hygiene-test change.
- `budget.mission.test.ts` compares the task-1 inventory with current mission evidence, the backups, discovery shims, coverage output locations, newly created root coverage files and native artifacts under `.arc-verify`. The latest completed budget checks were below 30 files and 5,000,000 bytes; repeat the fixed budget command after any evidence changes.
- `scripts/coverage/hygiene.test.ts` now asserts the whole index contains no `.profraw`, `.profdata` or `.lcov` path. Its existing isolated repository exercises the same assertion with an empty index and with prohibited root and nested files. Existing root, `.arc-verify`, docs and fixtures ignore probes remain.
- Bun 1.4.4 treats the contract's unprefixed `.arc-verify/...` paths as filters and skips hidden directories. Three small files under `mission-test-discovery.arc-verify/root-profile-hygiene/` expose the matching hidden tests only in the baseline's mission workspace. The exact contract commands report the shim paths and the underlying named tests; direct `./.arc-verify/...` runs report the hidden paths. A disposable ordinary-checkout simulation ran its sentinel test without the ignored profiles or backups.
- `gate-results.json` records the current source identity and the actual non-compiling checks. Harness and coverage are **unavailable**, with null exit codes: neither command was run. Both require a fresh Cargo compilation, which this task and the repository execution policy prohibit without an explicit build request. No historical receipt is presented as current evidence.
- The passive gate proof fails for each unavailable gate. The existing coverage evidence test also fails because `.arc-verify/rust-coverage/baseline/latest.json` is absent. The independent QA report predates this restoration and its missing-artifact finding is now stale; its owner must review the restored files and update the report.

| Command | Actual result |
| --- | --- |
| `bun test .arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts` | Exit 0; two named mission tests passed through the discovery shim. |
| `bun test scripts/coverage/hygiene.test.ts` | Exit 0; four tests and 224 assertions passed. |
| `bun test scripts/coverage/hygiene.test.ts scripts/coverage/hygiene-preservation.test.ts .arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts` | Exit 0; nine tests and 318 assertions passed across three reported files. |
| `bun test .arc-verify/root-profile-hygiene/gate-proof.mission.test.ts scripts/coverage/evidence.test.ts` | Exit 1; both gate-proof tests report unavailable and the existing evidence test reports missing `latest.json`. |
| `bun test .arc-verify/root-profile-hygiene/budget.mission.test.ts` | Exit 0; the mission evidence stayed within the stated count and byte limits. |
| `bun test ./.arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts ./.arc-verify/root-profile-hygiene/budget.mission.test.ts` | Exit 0; three tests passed across the two underlying hidden files. |
| `git diff --check -- .gitignore scripts/coverage/hygiene.test.ts` and `git diff --cached --check` | Exit 0. |

The current coverage-input identity is `61a94138b8f0a5696b123ba654e55241d1e23b2d8eee0172e41558690111543f` over 185 files. The harness SHA-256 is `6f3eacf963b2c9d7f026f2847f2d37baa861dd4bbf70a5bd33803bbf471d97b1`, and the coverage runner SHA-256 is `d32ec1940cafd539d10f259cc984ab019b90a54d17cbd49ce0613fc3815203da`. These identities are not a gate run.

For a future authorized actual run, the passive proof expects each gate's exact argument array, zero exit code, run start and finish times, `skipCoverage: false`, matching before/after source hashes, both script hashes, and the path and SHA-256 of its actual output. Coverage also needs the live run ID and runner receipt. The unchanged harness invokes the unchanged coverage command, so an actual harness output can support both records if it contains the corresponding six-crate result and live run directory. Count all outputs against the original inventory; do not use this expectation as a substitute for running either gate.


## 2026-11-10 authorized gate and budget update

This section supersedes the earlier statements that compilation was unauthorized, the gates were unavailable, and native coverage outputs count against the mission-directory budget. The operator authorized the Cargo build and limited that budget to `.arc-verify/root-profile-hygiene/`. Skills used: `oma-dev-workflow` and `oma-debug`.

The existing budget test failed on this worktree before the correction: the exact command reported 22,562 files and 3,218,996,424 bytes and timed out after scanning native outputs and build artifacts. Its scope was the cause. The owned test now walks only the mission evidence directory, retaining its existing regular-file and symlink handling and the same `< 30` and `< 5,000,000` assertions. No new test or harder limit was added.

The unchanged `./scripts/harness/validate.sh` ran with `SKIP_COVERAGE` unset from 2026-11-10 04:24:41.596119 UTC to 04:27:29.065254 UTC and exited 0. It invoked `bun run test:coverage` as its child; this was not a separate standalone invocation. The coverage collection inside that child ran from 04:24:49.296 UTC to 04:27:14.947 UTC and exited 0. The enclosing times in `gate-results.json` bound the child run. The complete combined harness output is in `harness-output.txt`; its identical `harness-output.log` original is retained locally and ignored by the pre-existing `*.log` rule.

The source identity was `61a94138b8f0a5696b123ba654e55241d1e23b2d8eee0172e41558690111543f` over 185 coverage inputs before and after the run. The unchanged harness SHA-256 was `6f3eacf963b2c9d7f026f2847f2d37baa861dd4bbf70a5bd33803bbf471d97b1`; the unchanged coverage runner SHA-256 was `d32ec1940cafd539d10f259cc984ab019b90a54d17cbd49ce0613fc3815203da`. The output SHA-256 is `da5f1f6f1d7256fa52f22f28fe0b8cdd5e0d90c5b591acae77f3d0549cf612eb`. Run ID: `2026-11-10T04-24-48-674Z-c4014030-b332-4718-92ba-1f71581058d7`; the live runner receipt and its hash are recorded in `gate-results.json`.

| Crate | Production lines | Coverage |
| --- | ---: | ---: |
| arc-core | 247/256 | 96.48% |
| arc-detector | 609/610 | 99.84% |
| arc-storage | 653/674 | 96.88% |
| arc-ipc | 387/393 | 98.47% |
| arc-daemon | 1629/1746 | 93.30% |
| arc-cli | 968/1100 | 88.00% |

All six exceed the existing 80% requirement. The fresh native coverage run directory contains 22 regular files and 24,949,525 logical bytes; its fresh target directory contains 10,593 regular files and 1,647,430,675 logical bytes, including 1,494 raw profiles totaling 544,343,520 bytes. No symlinks were counted. These pre-existing gate output locations are outside the operator's mission budget. The mission directory itself contains 10 files and 106,446 logical bytes at this report write. The live `baseline/latest.json`, run directory and target are intentionally ignored by existing coverage protections and must remain available in this workspace for the live evidence test.

| Command | Result |
| --- | --- |
| `mise tasks --all` | Exit 0; existing coverage tasks listed. |
| `bun test .arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts` | Exit 0; 2 tests, 74 assertions; mission tests executed through the discovery shim. |
| `bun test scripts/coverage/hygiene.test.ts` | Exit 0; 4 tests, 224 assertions. |
| `bun test scripts/coverage/hygiene.test.ts scripts/coverage/hygiene-preservation.test.ts .arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts` | Exit 0; 9 tests, 318 assertions across all 3 intended files. |
| `./scripts/harness/validate.sh` | Exit 0; all 5 checks passed, with coverage enabled. |
| `bun test .arc-verify/root-profile-hygiene/gate-proof.mission.test.ts scripts/coverage/evidence.test.ts` | Exit 0; 3 tests, 4,550 assertions across both intended files. |
| `bun test .arc-verify/root-profile-hygiene/budget.mission.test.ts` | Exit 0 after the scope correction; 1 intended test, 2 assertions. |
| `git diff --check -- .gitignore scripts/coverage/hygiene.test.ts` and `git diff --cached --check` | Both exit 0. |

The four destination profiles and their backups matched the baseline and accessible accepted-HEAD blobs in the preservation test. The existing hygiene tests also checked the empty coverage-extension index, the root and `.arc-verify` ignore probes, and the docs and fixtures examples. No product file, manifest, lockfile, harness, runner, other mission test, or discovery shim was changed in this task.

The independent `qa.md` report predates this correction and the current gate receipt; its findings about the budget and gate proof need an independent refresh by its owner. Global worktree cleanliness is still unverified because Git reported `av.yaml: Operation not permitted`. A separate process-list command was also denied; the launched harness process itself completed and returned its exit status.
