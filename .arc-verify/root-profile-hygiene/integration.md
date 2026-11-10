# Destination integration check

Status: partial. The profile and hygiene checks pass; the harness and coverage gate are unavailable. Reviewed with `oma-qa`.

The task-1 baseline names this workspace as the destination. Its HEAD is `e0875e7449c4e5d6752dfd1b6508c408ebd42ed3`. Each profile below is still present locally and in its task-1 backup, with mode `0644`, size 383,272 bytes, and the listed SHA-256. Each accepted-HEAD blob is accessible and has the same SHA-256. No restoration was needed.

| Profile suffix | SHA-256 |
| --- | --- |
| `29001.profraw` | `200253464f2de199a3319c5bca9e777a92ec857c3f2e5c318f9f268bd0d09ee6` |
| `29002.profraw` | `c73320b5c5ae0043057c8a3a480611885af6a8ad10699689f177dd47bccdd31b` |
| `29029.profraw` | `beb51c46d45de1b2c66c6b9bb2eb7f578d88eef29791ced1b39f7445585c1d42` |
| `29031.profraw` | `c5893510c8257c602de32f2f5b0a6b325c5cfa3444e7f5c8312bcfe9987fa568` |

All filenames have the prefix `default_16827093085728555556_0_`. The four backups remain in `.agents/results/root-profile-hygiene/profile-backups/`. The index has no `.profraw`, `.profdata`, or `.lcov` paths. The profile deletions are staged; `.gitignore` and `scripts/coverage/hygiene.test.ts` are unstaged. Scoped comparison with HEAD found no changes to `crates/`, `scripts/harness/validate.sh`, the coverage runner and hygiene protection files, or the checked manifests and lockfiles.

## Checks

- Baseline comparison: Python read the named destination, verified each local and backup file's size, mode and SHA-256, read each blob with `git cat-file blob`, and scanned the complete NUL-delimited `git ls-files` index. Exit 0; four profiles matched and zero coverage-extension paths were tracked.
- `bun test scripts/coverage/hygiene.test.ts scripts/coverage/hygiene-preservation.test.ts`: exit 0; 7 passed, 0 failed, 240 assertions. This includes the root, `.arc-verify`, docs, and fixtures ignore probes.
- `git diff --check -- .gitignore scripts/coverage/hygiene.test.ts` and `git diff --cached --check`: exit 0. The scoped `git diff --name-status HEAD` check for the product, harness, coverage and manifest paths listed above returned no paths.
- Coverage input identity computed by `scripts/coverage/source.ts`: `95e4deaa22b8163a2da2eea9f67c01e654f158f3bed1d13d6d85a4377e2a4687` across 185 files. Harness SHA-256: `6f3eacf963b2c9d7f026f2847f2d37baa861dd4bbf70a5bd33803bbf471d97b1`; coverage runner SHA-256: `d32ec1940cafd539d10f259cc984ab019b90a54d17cbd49ce0613fc3815203da`.

The unchanged harness invokes `bun run test:coverage`; the coverage runner creates a fresh Cargo target and compiles tests. This task and the repository execution policy require an explicit build request before compilation; none was provided. Neither gate was run. `.arc-verify/rust-coverage/baseline/latest.json` is absent, so no fresh coverage result is available. Final independent mission QA is not claimed here.

Evidence accounting, including this note: 7 new or changed mission evidence files, 1556078 bytes; the note directory contains 3 files, 22990 bytes. This includes four backups and all files newly created or changed under the baseline's coverage and target scopes. No new native coverage files were found. The existing task-1 baseline and task-2 note were preserved under the task's instruction not to delete evidence or preceding work.

The operator note calls for only a short evidence note and prohibits new mission-specific receipts and gate proofs. The task requests `integration.json` and `gate-results.json`, while the unchanged verification contract references `gate-results.json` and three deleted mission tests. Those interfaces cannot all be satisfied under the current instructions; no JSON receipt or replacement test was added. `av.yaml` still reports `Operation not permitted`, so global worktree cleanliness is not established.
