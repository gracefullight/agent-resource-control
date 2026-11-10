# Root profile hygiene: integration check

For the cleanup's authoritative recorded verification status and scope, see the [independent QA review](qa.md), the [harness command record](../rust-coverage/tooling/acceptance/runs/harness-0049875b-a0d0-4bf2-98fc-3973be1bffeb/00-command.json) and its [source binding](../rust-coverage/tooling/acceptance/runs/harness-0049875b-a0d0-4bf2-98fc-3973be1bffeb/binding.json), the [harness gate records](../rust-coverage/tooling/acceptance/runs/), the [coverage gate records](../rust-coverage/baseline/runs/), and the [coverage pointer](../rust-coverage/baseline/latest.json). The gate records and pointer are ignored, workspace-local artifacts and may be absent from another checkout.

The operator explicitly authorized compilation for this mission, including the recorded harness and coverage runs. Consult the QA review and gate records above for their recorded outcomes and verification scope.

The [historical receipt](gate-results.json) is authoritative for the previous cycle, not the cleanup's current acceptance. It records a failed coverage collection when a saturation test reported OS error 22 while setting a socket timeout; the cause was not established. It also records harness passes with coverage enabled.

## Preservation and index

- At the integration snapshot, HEAD was `63afa7d86be8f3516501bbf19668e5623ec1c935`. The staged index contained none of the six former mission-specific tests and discovery wrappers, the prior full log or the prior per-file inventory. An index scan found no tracked `.profraw`, `.profdata` or `.lcov` files.
- The retained historical log has SHA-256 `da5f1f6f1d7256fa52f22f28fe0b8cdd5e0d90c5b591acae77f3d0549cf612eb`; its bytes match the accepted-HEAD blob, and Git reports the retained copy as ignored and untracked.
- Staged and unstaged comparisons with the accepted HEAD found no changes in `.gitignore`, the manifests and lockfiles checked, `crates/`, `scripts/`, `.arc-verify/rust-coverage/` or `.arc-verify/evidence-hygiene/`. This covers the product, harness, existing generic validators and coverage accounting within those paths.
- At that snapshot, workspace walks before and after the hygiene tests found no local coverage profiles. The walks excluded Git metadata and the outer checkout's top-level `.agent-valley` and did not follow directory symlinks. The four profiles in the earlier cycle's baseline were absent from this worktree; in the historical workspace named by that baseline, all four matched its recorded size, mode and SHA-256 after the tests. No profile was copied, changed or deleted by that task. See the QA review for its profile-scan findings; other excluded or symlinked locations were not established by those scans.
- `git status --short` and the broader unstaged comparison reported `av.yaml: Operation not permitted`. Global worktree cleanliness is therefore unverified. Serena was not exposed in the available tool inventory; no whole-repository content search is claimed.

## Checks at the integration snapshot

- `bun test scripts/coverage/hygiene.test.ts scripts/coverage/hygiene-preservation.test.ts` exited 0: 7 tests passed, 0 failed and 244 assertions, using Bun 1.4.4. The intended index changes were staged before the run.
- `git diff --cached --check` passed.
- `oma docs sync --json` exited 0. Candidate links were inspected in context.
- At the earlier integration snapshot, the harness and coverage gates were unverified and independent QA was pending. Those statements describe that historical snapshot; the QA review and gate records linked above are authoritative for their recorded results.
