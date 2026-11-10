# Independent QA review — 2026-11-10 UTC

Review result: **PASS** for the reviewed cleanup and the recorded gate runs. Verification completeness is **partial**: this review did not rerun the full harness after task 7 or this QA update, and the configuration and scan limits below remain. This verdict does not claim that later framework verification has run. Skill used: `oma-qa`.

Index audit: PASS

All eight requested paths existed at accepted HEAD `63afa7d86be8f3516501bbf19668e5623ec1c935` and are absent from the current index: the three `budget`, `gate-proof` and `profile-preservation` mission tests, their three discovery wrappers, `harness-output.txt` and `baseline.json`. A case-insensitive scan of all 3,589 indexed paths found no `.profraw`, `.profdata` or `.lcov` paths. The staged change list contains those eight deletions and the four evidence files below.

Historical receipt: PASS

`gate-results.json` identifies the previous cycle as historical. The accepted-HEAD QA record describes a failed 04:15 UTC coverage collection with OS error 22 while setting a socket timeout, followed by successful unchanged runs; the cause of the failure was not established. The receipt's historical source identity, final run ID and log hash agree with the accepted-HEAD receipt. Its six percentages agree with the retained log.

The 1,487-byte retained log is ignored and untracked under `target/`. It is byte-identical to the accepted-HEAD blob and has SHA-256 `da5f1f6f1d7256fa52f22f28fe0b8cdd5e0d90c5b591acae77f3d0549cf612eb`.

Preservation audit: PASS

This is a scoped verdict. HEAD remains the accepted commit and its parent `e0875e7449c4e5d6752dfd1b6508c408ebd42ed3` is accessible. Staged and unstaged comparisons and untracked-file checks found no changes in the accessible protected paths: `.gitignore`, the checked manifests and lockfiles, `.cargo/`, `crates/`, `scripts/`, `docs/`, `.arc-verify/rust-coverage/` and `.arc-verify/evidence-hygiene/`. The index has no skip-worktree or assume-unchanged entries. The existing generic preservation test also passed.

The four profiles recorded in the prior cycle's baseline were checked directly in its historical workspace; each still matches its baseline size, mode and SHA-256. At 06:34 UTC, a case-insensitive walk found 5,980 coverage-extension files under this worktree's `target/`, four under `.arc-verify/`, and none elsewhere in the scanned mission worktree; the outer-checkout scan found none in its scanned scope. Both walks excluded Git metadata, did not follow symlinks (including 236 directory symlinks per walk), and reported no traversal errors. The outer scan also excluded its top-level `.agent-valley`, which contains nested workspaces. Those exclusions and symlink targets were not established by the walks.

Evidence budget: PASS

The complete indexed mission evidence is the four files below. Sizes are Git index blob bytes, refreshed after this report was staged; the other mission directory has no indexed files. The count and total are below 30 files and 5,000,000 bytes.

| Indexed evidence | Bytes |
| --- | ---: |
| `gate-results.json` | 2070 |
| `implementation.md` | 1890 |
| `integration.md` | 3632 |
| `qa.md` | 8825 |
| **Total: 4 files** | 16417 |

Overall: PASS

This verdict covers the cleanup, accessible preservation evidence, the independently rerun hygiene and coverage-evidence tests, and the recorded source-bound harness and coverage runs. The post-edit framework contract and the inaccessible or excluded scopes remain separate verification limits.

## Summary finding and correction

**Resolved historical LOW finding — owner: task 5; corrected by task 7.** The earlier QA review found that `implementation.md` and `integration.md` identified an older QA snapshot as the current linked report. The staged `implementation.md:3,5,17` and `integration.md:3,5,22` now direct readers to `qa.md` and the gate records by path and identify those files as authoritative for status and scope. The summaries no longer copy the disputed QA budget or ordering. This matches the operator's path-only correction; no further summary edit is requested to anticipate later QA or framework runs.

No reproducible CRITICAL, HIGH, MEDIUM or open LOW finding was identified in this review scope. This is not a product-wide security, performance or accessibility audit.

## Gate evidence

| Check | Observed result |
| --- | --- |
| `bun test scripts/coverage/hygiene.test.ts scripts/coverage/hygiene-preservation.test.ts` | Rerun by this reviewer with Bun 1.4.4: exit 0; 7 passed, 0 failed, 244 assertions. These tests create, modify and remove temporary Git repositories. |
| `bun test scripts/coverage/evidence.test.ts` | Rerun by this reviewer: exit 0; 1 passed, 0 failed, 4,509 assertions. At 06:34 UTC, a separate pointer inspection identified the fixed coverage run below. The pointer can change. |
| `bun test scripts/coverage/harness.test.ts` and post-edit framework contract | Not independently rerun: the test launches the compiling full harness, which is outside this task's execution instructions. The fixed recorded harness run below passed before the task-7 summary and this QA edits. Post-edit contract verification remains pending. |

The [recorded harness command](../rust-coverage/tooling/acceptance/runs/harness-3dbe0d6b-dd60-4843-bdc2-5f6cdede5209/00-command.json) ran `./scripts/harness/validate.sh` in this worktree from 06:01:16 to 06:04:01 UTC, exited 0, included coverage, listed all six crate passes and reported zero violations. Its [binding](../rust-coverage/tooling/acceptance/runs/harness-3dbe0d6b-dd60-4843-bdc2-5f6cdede5209/binding.json) records matching before/after source identity and exit 0. The runner deletes `SKIP_COVERAGE` from its environment; the recorded output shows the coverage step. The operator explicitly authorized the recorded compilation and future mission runs.

The harness output names coverage run `2026-11-10T06-01-23-116Z-ae63ac14-8f74-43cc-b85c-bdc7af4fcc85`. Its [receipt](../rust-coverage/baseline/runs/2026-11-10T06-01-23-116Z-ae63ac14-8f74-43cc-b85c-bdc7af4fcc85/receipt.json), summary and the pointer observed at 06:34 UTC agree on that run, exit 0, pass and source SHA-256 `61a94138b8f0a5696b123ba654e55241d1e23b2d8eee0172e41558690111543f`. The current identity independently recomputes to that hash across 185 inputs. This identity covers the coverage inputs, not the QA summaries or harness script; those accessible paths were checked separately against accepted HEAD where applicable.

The recorded coverage percentages are 96.48%, 99.84%, 96.88%, 98.47%, 93.24% and 88.00% for `arc-core`, `arc-detector`, `arc-storage`, `arc-ipc`, `arc-daemon` and `arc-cli`, respectively, against the 80% floor. The evidence test recomputed accounting, validated the raw report hashes and the merge of 1,494 recorded profiles, and checked child-profile bindings. The recorded `arcctl` and `arc-daemon` child exits are 0 and 1; the test checks binding consistency, not that both exits are zero.

The preceding QA report stated that the framework passed its contract at 05:39 UTC and that earlier 05:16 and 05:25 attempts failed QA-file assertions. This reviewer did not independently inspect those framework result records. Neither that statement nor the fixed 06:01 harness record establishes the post-edit framework result.

## Checks and limits

- `git ls-files`, `git show`, `git cat-file` and a Python index audit checked all eight removals, all indexed profile extensions, the four evidence blob sizes, and the staged change list. The four staged evidence files contain no forbidden local-results citations.
- `git check-ignore -v`, `git ls-files --error-unmatch`, SHA-256 and a byte comparison against the accepted-HEAD blob established the retained log's ignored, untracked identity. The expected `--error-unmatch` result was exit 1 because the log is not indexed.
- `git diff` and `git ls-files --others --exclude-standard` over the protected paths returned no changes; `git diff --cached --check` passed. Direct baseline comparisons checked the four historical profiles, and the coverage-extension walks have the exclusions stated above.
- `git status --short` printed `av.yaml: Operation not permitted` despite exiting 0; `ls -ld av.yaml` also failed with that error. Global worktree cleanliness and that configuration are unverified. No container target was supplied in the mission input; configured targets, health, availability and recent errors are unverified because the configuration is inaccessible.
- Serena was not available in the tool inventory. This review used known-path reads, Git metadata and existing tests; it makes no whole-repository content-search claim. No browser, GUI, dependency installation, compiling build or harness was launched by this reviewer.
