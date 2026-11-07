# Task-2 execution context

Skill: oma-refactor. Role: installed refactor-engineer. Ownership is the two
assigned test targets, their new child modules and this implementation directory.
No OMA claim/run/session path was injected. Do not create or mutate mission state.
Independent QA and final mission synthesis belong to subsequent assigned Actors.

Use task-1 baseline artifacts unchanged. Accepted HEAD is
`a905653d557f4c5246a1f656042cb8d29e46c052`. Scoped staged/worktree diffs were empty
before editing; global status reports `av.yaml: Operation not permitted`.
Serena tool discovery returned no applicable tools. Use known-path reads and Git
path metadata only. No source discovery fallback or provider changes.

Mission budget: 86400 seconds from mission start; supplied operations began
2026-11-07T18:10:03Z, giving an upper deadline of 2026-11-08T18:10:03Z UTC.
No cancellation is present. Runtime suites run serially with a 120-second command timeout and bounded
termination/reaping. Toolchain reads and resident --list overlapped the workspace
test; they did not execute scenarios or modify source. Each check owns its group.
Mission maxima: parallelism 3, runs 200, retries 3. No installs or downloads.
CPU, address-space and file limits and host context are recorded in each receipt;
these describe existing host limits, not additional resource enforcement.
Local monetary costs are unknown. Existing workspace target/ is reused.

The baseline safety net passed 13 IPC, 11 resident and 2 CLI tests. Assertion
inventory and immutable Git-byte comparison supplement unchanged existing tests.
Coverage, mutation strength and flakiness rates have not been measured. These
moves address the two assigned 901/551-line hotspots; no broader churn analysis
or unrelated cleanup is needed. Production is frozen while tests are moved.

Transformations: move resident scenarios into lifecycle, provisioning, identity
and coordination child modules; retain root fixtures/helpers and the root probe.
Then move IPC tests into framing and dispatch modules with one shared hello helper.
Explicit module wiring and imports are the only intended source differences.
Staging only owned source paths is authorized for tracked harness coverage;
commits are left to Chief integration to preserve the accepted HEAD-bound baseline.
