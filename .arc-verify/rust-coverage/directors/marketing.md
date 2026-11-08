# Coverage repair: product claims and completion boundaries

Status: completed for task-2's scoped documentation assignment. Owner: Marketing Director.
Changed file: `.arc-verify/rust-coverage/directors/marketing.md` only.
Skill: `oma-docs`, using the installed `docs-curator` definition.

This report defines claims the Chief can use when assembling the final delivery.
It does not certify implementation, coverage percentages or independent QA.

## Source identity

Accepted committed baseline: `fbe76a5f15101ad415f355ef06f1deeb1f97d568`.
Verified baseline tree: `8db02719b5bd06d888729ca337f9bf8887991515`.
The inspected Actor-local HEAD and supplied snapshot commit are
`3e75ddb8da120ff60a8e1ddbf5193f7007555283`; its verified tree is also
`8db02719b5bd06d888729ca337f9bf8887991515`.
The accessible diff against the accepted baseline was empty before writing.
Nine inspected evidence/protected files were individually compared with accepted
Git bytes and matched. This report is a new local task edit, absent from those
commits; integration into the Chief's checkout is not established here.

The supplied snapshot and worker refs under
`refs/agent-valley/task-evidence/daa81f71b962cfea-78686d2704d4d690-1/`
both failed local `git rev-parse --verify` with exit 128. The snapshot commit
itself resolves; no other checkout's revision was substituted for either ref.

## Observations and delivery claims

| Evidence class | Supported statement | Source and limit |
| --- | --- | --- |
| Current source | ARC's workspace has six crates: arc-core, arc-detector, arc-storage, arc-ipc, arc-daemon and arc-cli. | [Workspace manifest](../../../Cargo.toml), lines 1–9. This inventory establishes scope, not coverage or release readiness. |
| Current source | The CLI exposes status, doctor and literal command classification through local IPC. | [CLI source](../../../crates/arc-cli/src/main.rs), lines 1–14 and 38–69. The source uses UnixStream. Classification is described as analysis without execution. |
| Current source | The detector entry point performs bounded syntax classification and returns typed results or uncertainty. | [Detector source](../../../crates/arc-detector/src/lib.rs), lines 1–32. Source inspection is not a fresh non-execution experiment or an exhaustive behavioral proof. |
| Inherited product description | The PRD describes Agent Valley issue-driven orchestration, worktrees and agent routing. | [PRD](../../../docs/prd.md), lines 15–51. Those claims must not be transferred to this ARC coverage repair. |
| Current input deficiency | The root package.json is absent in this Actor's input snapshot; the unchanged harness calls bun run test:coverage. | Direct file-existence check and [harness](../../../scripts/harness/validate.sh), lines 413–426. No current gate execution was performed by this documentation task. |
| Historical execution | A preceding module-conformance harness run exited 1. | [Receipt](../../module-conformance/implementation/harness.json): 2026-11-07, another mission workspace and source fingerprint. Its [claim ledger](../../module-conformance/evidence-scope.md), lines 41–45, records the missing coverage entry point and commercial unknowns. This is historical evidence, not a current coverage measurement. |
| Assigned target | At least 80% production-source line coverage is required independently for each of the six crates. | Supplied mission.goal, goalBrief.successCriteria and task-2 instructions. Eighty percent is a new engineering target, not a measured baseline or SRS threshold. A workspace average cannot establish it. |
| Hypothesis | A trustworthy gate could reduce mistaken success judgments and help contributors locate coverage gaps. | Proposed command, per-file uncovered lines and deterministic failure diagnostics support this rationale. Diagnosis time, defect reduction, engineering savings and user response have not been measured. |
| Unknown | Commercial demand, customers, willingness to pay, price, conversion, retention, revenue, acquisition/delivery/support costs, margin and ROI are unknown. | No customer study, payment ledger or complete cost accounting is supplied. Historical unknowns are corroborating context, not evidence that no such activity exists anywhere. Unknown costs are not zero. |

The immediate audience is repository contributors adding tests and reviewers
deciding whether verification supports a change. These roles follow from the
mission's commands and evidence requirements; no recruited participants or field
study validates their experience. Expected convenience is a hypothesis.

## Why fund the gate now

The immediate investment addresses a concrete repository failure: a required
command has no root entry point in the inspected input. Reusing existing Rust
tests and adding pinned coverage tooling gives the mission a bounded outcome:
the command must execute tests, account for each crate and reject invalid results.
This is the chosen engineering investment because the operator explicitly requires
it and the missing entry point is observable. It is not justified by a calculated
financial return; the data needed for that calculation is unavailable.

The supplied marketing consultation proposed a fixed-scope Rust coverage-gate
assessment, direct maintainer introductions, consultancy subcontracting,
symptom-led support discovery, technical content, sponsorship and later launches.
Those are untested commercial options. The supplied goalBrief defers them, and
task-2 authorizes an internal evidence report only. Buyer qualification, pricing,
permitted channel access and delivery economics have not been established.
Outreach, assessment offers, channel experiments and launch work would add work
without satisfying this mission's verification requirements, so they remain
outside this assignment. No channel-placement or payment forecast is adopted.

No publication, outreach, spending, new analytics, customer recruitment or
unrelated product change occurred in this task. No commercial experiment or user
simulation was run. Commercial uncertainty adds no new approval gate.

## Final-delivery wording contract

The Chief may report a repaired coverage gate only after integrated-source
evidence confirms the root Bun command, exactly pinned tooling, all six separate
crate results and the 80% floor. Report the actual covered/total production lines,
per-file uncovered lines, executed platform/target, source/configuration identity,
tool versions and command exits. Unit, integration and CLI child-process execution
must contribute actual measurement; passing test counts alone do not establish
that subprocess profiles were collected. Difficult production paths stay in scope.

Deterministic checks must show nonzero exits for failed tests, missing tools,
missing/stale reports and a below-threshold crate. Successful old reports cannot
rescue a failing current run. Independent oma-qa review must remain identifiable
as independent review; this Director report is not that review.

The following exact final checks remain implementation and Chief obligations:

- `bun run test:coverage`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `./scripts/harness/validate.sh`, unchanged, with zero violations and no skip flags

Production behavior and existing assertions must be preserved. The harness and
`crates/arc-storage/migrations/0001_phase0.sql` must remain byte-identical to the
accepted baseline. This task's direct byte comparisons passed; they do not
certify the later integrated tree. Final coverage and review evidence belongs
under `.arc-verify/rust-coverage/` and must be checked in that delivered tree.

Inspected protected-file SHA-256 values, matching the accepted baseline:

| Path | SHA-256 |
| --- | --- |
| scripts/harness/validate.sh | `93140173df8b8bd28b067aff68301134b0398b643240b0b1e6a13da4487ebe33` |
| crates/arc-storage/migrations/0001_phase0.sql | `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384` |

Do not describe this repair as delivering orchestration, scheduling, managed
execution, cleanup, a dashboard, desktop integration, signing or packaging.
Do not equate classification with running, scheduling or governing the analyzed
command. Do not claim faster diagnosis, lower costs, increased conversions or
profit from test passes. Existing capabilities are context; the new deliverable
is the coverage gate and its verified evidence.

## Platform and unfinished work

- desktop: incomplete
- native Windows: incomplete
- remaining Phase 0: incomplete

The [prior bounded-IPC report](../../../docs/bounded-ipc-final.md), lines 118–126,
retains native Windows Named Pipe transport, desktop/tray integration, managed
execution and wider platform/runtime obligations. It predates later classification
work, so its older statement that classification is undelivered is not adopted.
The current CLI and detector source above establish the narrower present slice.
Host coverage must identify inactive platform branches as unmeasured; a passing
host result cannot certify those branches, native Windows or full Phase 0.

## Checks and handoff

Manual inspection — `oma docs verify` did not run. `oma docs verify --help`
confirmed that verification regenerates `docs/generated/doc-refs.json`, outside
this task's sole owned file. No index was generated and no unrelated document
was synchronized. CLI drift counts before/after are unavailable. The report did
not exist on entry; local Markdown references and required claim boundaries were
checked directly after writing. No external URL verification is claimed.

Commands executed and outcomes:

- `pwd`, scoped source/document reads and `command -v oma`: exit 0; CLI installed.
- `git rev-parse HEAD HEAD^{tree} <baseline>^{tree} <snapshot>^{tree}`: exit 0; identities above verified.
- `git diff --stat <baseline> -- . ':!av.yaml'`: exit 0, empty on entry.
- `git status --short --untracked-files=normal`: exit 0, but printed `av.yaml: Operation not permitted`; global cleanliness is unverified.
- Scoped Git status of the owned/evidence paths: exit 0, clean on entry.
- `git ls-files --error-unmatch package.json`: exit 1, root manifest not tracked; Python also confirmed it absent on disk.
- Snapshot/worker ref resolution: exit 128 for each, as recorded above.
- `oma docs verify --help`: exit 0; printed nonfatal mise tracked-config permission warnings. No denied path was bypassed.
- Python hashlib/subprocess comparison: exit 0; all nine evidence/protected paths match accepted Git bytes.
- Post-write Python reference/content and source-preservation checks: exit 0; eight unique local Markdown targets resolve, 17 required boundary terms are present and all nine inspected evidence/protected paths remain byte-identical to accepted Git bytes.
- `git diff --check`: exit 0. `git diff --no-index --check /dev/null .arc-verify/rust-coverage/directors/marketing.md`: exit 1 for the new-file difference, with no whitespace diagnostics.
- Final `git status --short --untracked-files=all -- . ':!av.yaml'`: exit 0; only the new owned report is listed. The tracked diff excluding the denied av.yaml path remains empty.

Serena tools were unavailable in the discovered tool inventory. Known source
paths were read directly; no recursive native code search or GUI workaround was
used. No Cargo/test suite, harness, build, installation or screen check was run
for this prose-only assignment. Product execution and independent QA remain
with their assigned owners.

Acceptance checklist:

- [x] Repository observations and supplied consultation are cited; hypotheses and commercial unknowns are explicit.
- [x] Final-delivery claims distinguish the current ARC slice, the new target and incomplete capabilities.
- [x] Only the owned report was written; no distribution, spending, analytics or product change occurred.

No ownership or interface change is proposed. The Chief must verify integration
of this new report and bind any final success claim to the actual delivered
source and required command receipts.
