# Design Director delivery review — task-3

Status: completed. Review Result: PASS for the assigned navigation and CLI
preservation review. Verification completeness: completed within this scope.
No CRITICAL, HIGH, MEDIUM or LOW defects found. Mission-level independent QA
and final integration acceptance remain separate tasks.

Skill: oma-qa. Applied the installed `.agents/agents/qa-reviewer.md` definition,
execution protocol and checklist directly. Reviewed security, performance,
accessibility and code quality as relevant to this test-only change. No source,
implementation evidence, mission state, dependency or product changes made.
Only changed path: `.arc-verify/module-conformance/design-review.md`.

## Source scope and provenance

All resulting-source references below concern this Actor's inspected input
snapshot and matching working bytes, not the accepted committed baseline.

| Identity | Verified value |
| --- | --- |
| Accepted baseline commit | `a905653d557f4c5246a1f656042cb8d29e46c052` |
| Accepted baseline tree | `c72fead1fdc7262d4a297d18e74f89e2bbfd6157` |
| Actor-local HEAD / supplied snapshot commit | `5969a9b8ad18c25dd97931b0df36d40242b12521` |
| Actor-local HEAD tree / supplied snapshot tree | `a3dfc866c7ac3ae80a7e9f7fac36b799ce2bdacb` |
| Delivered source fingerprint, recomputed here | `0b82e06fc1588151af7ae61620539025e5e00038935e4f0fcd54fcda43b8e64a` |

Workspace: the assigned `av-task/9dd58b3e81e0c07c-1724980aa084bbaa-1`
isolated worktree. The delivered Cargo receipts record their original mission
workspace cwd ending in `904e38b9-fa2e-4313-aec9-19e903479c6a/workspace`.
They were not executed in this Actor checkout. Their recorded source identity
matches this checkout's 73 fingerprint records, including modes, missing-file
markers and raw byte hashes. Evidence files are outside that fingerprint and
were checked separately.

The supplied refs
`refs/agent-valley/task-evidence/9dd58b3e81e0c07c-1724980aa084bbaa-1/snapshot`
and `refs/agent-valley/task-evidence/9dd58b3e81e0c07c-1724980aa084bbaa-1/worker`
do not resolve here: `git rev-parse --verify` returns 128 for each. The supplied
snapshot commit itself resolves as local HEAD and has the supplied snapshot
tree. No replacement ref or hash was invented. This report does not establish
which edits have been integrated into the Chief's current checkout; verify
the delivery there before final acceptance.

Initial scoped `git status`, unstaged diff and staged diff were empty for
the reviewed source and report paths relative to the Actor-local HEAD.
The new modules are tracked in this snapshot. That observation makes no
claim about their tracking status in the baseline or another checkout.

## Observed module structure

Paths are relative to the repository. References in this section use the
Actor snapshot above.

| Inspected path | Lines | Responsibility and interface |
| --- | ---: | --- |
| `crates/arc-daemon/tests/resident.rs` | 421 | Explicit scenario declarations at 6–13; one fixture/helper implementation; root serialization probe at 413–421. |
| `crates/arc-daemon/tests/resident/lifecycle.rs` | 146 | Serving, protocol rejection/recovery, CLI status/doctor, duplicate instance, SIGTERM, database reopen and absent-daemon commands. |
| `crates/arc-daemon/tests/resident/provisioning.rs` | 166 | Directory modes, links, nondirectory parents, nested startup and invalid socket entries, with preservation assertions. |
| `crates/arc-daemon/tests/resident/identity.rs` | 149 | Native default independent of TMPDIR; concurrent override admission and recovery after shutdown/death. |
| `crates/arc-daemon/tests/resident/coordination.rs` | 32 | Cross-process fixture lock handoff. |
| `crates/arc-ipc/src/lib.rs` | 356 | Original production prefix at 1–354; `#[cfg(test)] mod tests` at 355–356. |
| `crates/arc-ipc/src/tests/mod.rs` | 8 | Explicit dispatch/framing declarations and one shared hello helper. |
| `crates/arc-ipc/src/tests/framing.rs` | 122 | Nine framing/decoding tests, including rejected object roots and length boundaries. |
| `crates/arc-ipc/src/tests/dispatch.rs` | 69 | Four version, health, doctor and unsupported-operation tests. |

Every resulting source file is at most 500 lines by actual newline count.
`resident.rs` was 901 lines and IPC `lib.rs` was 551 at the accepted baseline.
The children use ordinary private Rust modules and `use super::*`; no public
visibility was widened. Fixture, Daemon, readiness, CLI and rejection helpers
remain defined once in the resident root. Shared process/thread serialization
remains in unchanged `crates/arc-daemon/tests/support/mod.rs`.

The unchanged daemon manifest has no new test target declarations. Delivered
baseline Cargo metadata lists the existing resident integration target; source
inventory places the new scenario files beneath its subdirectory, with no
`mod.rs` creating another integration root. The final workspace log has one
resident target with 11 tests. IPC discovery lists 13 tests with framing or
dispatch qualification. No numbered chunks or textual includes were used.

## Observed CLI and visible-state preservation

The baseline references below mean Git object bytes at `a905653d…`. Resulting
references mean Actor-local snapshot `5969a9b8…`. Full function source slices
were compared independently, including comments, literals, loops, declarations
and assertion order. IPC slices differ only by removed enclosing indentation.
All 51 mapped functions match: 24 tests and 27 helpers. All 28 attributes match,
including the macOS gate. The mapping accounts for 137 assertion sites;
whole-function equality also preserves helper assertions and their arguments.

In this table, `R` means `crates/arc-daemon/tests/resident.rs` at the baseline;
`D/` means resulting `crates/arc-daemon/tests/resident/`; `I` means baseline
`crates/arc-ipc/src/lib.rs`; `T/` means resulting `crates/arc-ipc/src/tests/`.

| Journey or state | Baseline → resulting evidence | Observed contract and limits |
| --- | --- | --- |
| Running status | R:325–335 → D/lifecycle.rs:63–73 | Successful exit; stdout after `trim_end()` equals `running. Socket: <path>. Protocol: 1. Sweeper off.`; excludes `eleven`. |
| Doctor | R:337–345 → D/lifecycle.rs:75–83 | Successful exit; stdout after `trim_end()` retains owner-only IPC, tray not connected, vendors not detected, classification not checked and Sweeper off; excludes `eleven`. |
| No-argument help | Unchanged `crates/arc-cli/tests/binary.rs:9–24` at both revisions | Non-success exit; combined stdout/stderr names status and doctor and contains no ANSI escape prefix. It does not assert a particular help stream or separately exercise `--help`. |
| No daemon | R:396–408 → D/lifecycle.rs:134–146 | Both commands fail and stderr contains DAEMON_UNAVAILABLE. Assertions do not require an exact numeric nonzero code or empty stdout. |
| Rejected directory/socket | R:472–533 → resulting root:336–397, called from D/provisioning.rs | Daemon non-success, empty readiness stdout, one stderr line with reason, path, ARC_SOCKET_PATH and owned-0700-directory guidance. Both CLI commands fail with DAEMON_UNAVAILABLE on stderr; fixture snapshots and database/socket state remain unchanged. |
| Valid override startup | R:594–669 → D/provisioning.rs:62–137 | Both commands succeed across existing/new, nested/non-nested and umask cases. Duplicate instance fails, reports socket already served, and original daemon remains usable until SIGTERM. |
| Native default identity | R:687–739 → D/identity.rs:5–57 | macOS-only attribute preserved at D/identity.rs:3. Both commands succeed across unset/changed/unwritable daemon and client TMPDIR combinations; status stdout contains the default socket. |
| Concurrent admission and recovery | R:742–831 → D/identity.rs:60–149 | Losing daemon announces no readiness on stdout and reports only-one-daemon on stderr. Both CLI commands succeed for winner, graceful restart and stale-socket recovery after forced death. |
| Direct shutdown | R:363–392 → D/lifecycle.rs:101–130; R:671–677 → root:399–405 | Owned daemon receives SIGTERM, exits successfully, socket disappears and database reopens with integrity ok and one migration. No exit confirmation or extra command introduced. |
| IPC capability state | I:501–540 → T/dispatch.rs:20–59; R:202–209 → root:211–218 | Hello/health do not invoke database handler; protocol and Sweeper off remain asserted. Doctor retains integrity ok, one migration, owner-only IPC, false tray/vendors and Sweeper off; zero migrations returns integrity failure. |
| Protocol failure recovery | I:485–498 → T/dispatch.rs:4–17; R:286–322 → D/lifecycle.rs:24–60 | Version 99 remains incompatible, later hello succeeds; oversized frame returns the same error and daemon remains usable. |

`crates/arc-cli/src/main.rs` and its binary tests are byte-identical to the
accepted baseline. Lines 21–29 retain color Never and no-argument help;
43–50 retain success versus failure exit handling and stderr error output;
62–93 retain stdout status/doctor text. Lines 96–112 retain connection/exchange
failure mapping to DAEMON_UNAVAILABLE and decoded protocol/integrity/IPC error
mapping. Those unchanged branches were inspected; the listed assertions do
not dynamically cover every possible error or exact numeric exit code.
Successful resident CLI cases do not assert empty stderr. The no-argument
test checks combined streams; it does not prove all terminal/environment
configurations. These are preserved coverage limits, not new regressions.

## Observed subprocess discovery and fixture lifetime

Baseline R:863–870 → root:414–421 preserves the exact root name
`fixture_serialization_probe`. Baseline R:878 → D/coordination.rs:9 still invokes
the current executable with `--exact fixture_serialization_probe`. Delivered
baseline and final discovery outputs each contain that root name exactly once.
The coordinator itself becomes `coordination::fixture_coordination_is_shared_across_processes`.

The probe writes ready before `support::serial()` and acquired only after
acquiring it. D/coordination.rs:16–31 waits for ready, rejects early child exit,
asserts acquired absent while the parent owns the process lock, releases that
lock, waits for successful child exit and requires acquired present. Therefore
the checked pass exercises the handoff assertions; it is not inferred solely
from a child exit code or a no-op probe without the environment variable.
The hash-verified exact coordination run selects one test and passes it.

`support/mod.rs:11–23,26–65` retains the mutex plus UID-specific file lock.
`release_process_lock()` takes only the file; the thread guard remains alive.
The coordinator declares serial, fixture, then owned child in that order
(D/coordination.rs:5–15). Reverse local drop order reaps the child before fixture
removal and guard release, including assertion unwind. Other scenario bodies
likewise retain guard-before-fixture-before-child declaration ordering.
Identity's children array, restart and recovery handles stay within that guard.

Root:95–114 retains gated `exec`, preserving the owned-child PID. Root:134–140
retains try_wait, kill if still running, then wait, including unwind cleanup.
Root:143–146 removes only its owned fixture directory after child cleanup.
No module-local locks or broad process-kill cleanup were added. All these
function bodies and the support file were compared directly to baseline bytes.

## Existing executable evidence consumed

No Cargo or harness command was rerun in this review: no concrete runtime gap
required a focused native check. The following delivered receipts and complete
ordered output parts were hash-verified here. Every final implementation
receipt has matching before/after source fingerprint `0b82e06f…`, reports no
timeout and records reaped command processes. Recorded environment uses
offline Cargo with rustup auto-install disabled.

| Recorded command / receipt | Recorded outcome |
| --- | --- |
| `cargo fmt --all -- --check` / implementation/fmt-final.json | Exit 0; empty output. Earlier fmt.json retains exit 1 on a different pre-format fingerprint. |
| `cargo clippy --workspace --all-targets -- -D warnings` / implementation/clippy.json | Exit 0. |
| `cargo test --workspace` / implementation/workspace-test.json | Exit 0; 183 successful test executions, including 2 CLI binary, 11 resident and 13 IPC tests; none failed. |
| Resident and IPC `--list` / implementation/*-discovery.json | Exit 0; names independently reconciled to all baseline/resulting mapped tests. |
| Exact qualified coordination test / implementation/coordination-handoff.json | Exit 0; one passed, ten filtered out; ready/acquired assertions are in the unchanged selected body. |
| `./scripts/harness/validate.sh` / implementation/harness.json | Exit 1; No oversized files found; one coverage-stage failure, Script not found test:coverage. Harness bytes unchanged. |

The passing baseline CLI, resident and IPC execution logs were also verified
against receipt output hashes. Earlier IPC discovery lacks processReaped and
per-command fingerprint fields, as baseline/fingerprint.md documents; it was
used only as a baseline name list. Current final discovery and workspace
receipts provide the complete source-bound execution evidence.

## Judgments and inferred users

Judgment: lifecycle, provisioning, identity and coordination names provide
useful scenario entry points while shared helpers remain single. Identity
combines endpoint identity with admission/recovery, a coherent boundary for
one-instance-per-user behavior. Framing versus dispatch separates decoding
mechanics from operation semantics. The 421-line resident root remains sizable,
but its common helpers and deliberately root-level probe have a clear purpose.
These are maintainability judgments, not measured navigation improvements.

Inferred personas are a developer starting ARC, a returning developer checking
liveness, and an operator recovering a failed/conflicting instance. Source
and tests preserve start/readiness → status, optional doctor, and direct signal
shutdown. No participant study, field test, simulation, retention experiment,
conversion/churn measurement or customer reaction was supplied or conducted.
No claim of improved task time, usability, retention or revenue follows from
this review. No reminders, extra journeys or exit friction were introduced.
Color-independent textual help is covered as above; screen-reader usability
and general WCAG conformance were not assessed. Screen checks are inapplicable
to this test-only split and were not performed.

## Commands executed here, limits and acceptance

- `git rev-parse HEAD HEAD^{tree} <baseline>^{tree}`: exit 0, identities above.
- Scoped `git status --short`, `git diff --stat`, `git diff --cached --stat`:
  exit 0, no pre-existing changes relative to this snapshot in reviewed paths.
- Known-path cat/nl/sed reads: inspected rules, skill/agent, baseline,
  implementation and source. `.agents/oma-config.yaml` was absent; the installed
  `.agents/oma-config.cue:381` specifies Serena. Tool discovery found no Serena
  tools. Used scoped reads, Git object/path inventories and explicit mapped
  paths; no recursive native project-source search or provider setup changes.
- `python3 -` read-only preservation/receipt audit: initial exit 1 on the old
  IPC discovery schema's missing processReaped field. Corrected the audit's
  schema assumption without editing evidence; repeat exit 0. Verified all 51
  full function slices, 28 attributes, 58 protected files against Git objects,
  IPC production prefix, 73 fingerprint records, 13 receipt/output pairs,
  discovery mappings, 183 passing executions and all nine source line counts.
- `python3 -` configuration/metadata/provenance read: printed existing daemon
  targets and test paths, then exited 1 when the supplied snapshot ref failed
  resolution. Explicit `git rev-parse --verify` checks confirmed both supplied
  refs return 128 and HEAD/baseline commits return 0. No refs were created.
- Mise emitted tracked-config symlink permission warnings during shell startup;
  reads/audits still ran. Delivery records av.yaml access denial during broad
  status, so global cleanliness is not claimed. No bypass was attempted.

Acceptance checklist:

- [x] Inspected modules, fingerprint and responsibility boundaries recorded.
- [x] CLI journeys, streams, exit semantics, color and truthful state compared
  against baseline, with exact existing coverage limits.
- [x] Exact subprocess name, ready/acquired evidence, guard ordering and
  owned-child cleanup independently checked.
- [x] Report only; fewer than 500 lines; no product/source changes.

Interface compatibility: no changed public IPC interface, CLI journey or
fixture interface found. Remaining mission risk: the harness coverage stage
still fails; final Chief integration and independent mission QA must retain
that outcome and verify the delivered bytes in the integrated worktree.

Evidence integrity anchors (SHA-256, paths under this evidence directory):

| Artifact | SHA-256 |
| --- | --- |
| baseline/baseline.json | `93356d14b07cb8dbcdaf65dcd1fe9a9eeef1fddca8b7378b1511fb36b7221577` |
| implementation/mapping.json | `d78bf55b5527df4eaa36ae1d493734faa16bb2ab400f201f2c746354afeecfdf` |
| implementation/fingerprint-files.json | `3944dcbb0f4477680b0c559c0b5db35c3a15683f259c62595545b59b204b535a` |
| implementation/workspace-test.json | `95bb50e4f42ada3ba219efdb883fca740d564d43f2a0ed8fd0ee1d17aa478482` |
| implementation/coordination-handoff.json | `d8a1390017bd4bee7b6dc8da4edf8ce1ce5cc03607d6f7a65441113cd21e2734` |
