# Schema compatibility: customer value and claim limits

Status: completed scope assessment for task-3, attempt 1, 2026-11-08. Owner: Marketing Director. This report delivers the assigned analysis; it does not establish implementation acceptance.

Complete the migration reliability evidence before pursuing assessment services or promotion. Preserving existing records during supported upgrades and explaining incompatible schemas serve the current product journey. Revenue and support savings remain hypotheses. The supplied consultation's commercial proposals are deferred and add no prerequisite to this mission.

## Evidence and source scope

Accepted committed baseline: `bf28a35b594f5ae83ac97c216bd3b09e60a2dfa1`, product tree `cde83859dbe37796b62b134384627f657eeae010`. This Actor inspected local snapshot HEAD `99caf642c52558526bc9812b79e39622088576a3`, with that same tree. `git rev-parse` confirmed both trees. The snapshot is a distinct commit; its identity must not replace the accepted baseline. This report is a new, uncommitted task-local file and is not part of either input revision. Collection and integration must verify its delivered bytes in the receiving worktree.

The following documents were read locally and compared byte-for-byte with their paths at the accepted baseline. All three match. Their embedded reports about earlier missions remain historical evidence, not fresh execution results for schema compatibility.

| Source | Relevant evidence | SHA-256 of inspected bytes |
| --- | --- | --- |
| [Bounded IPC recovery](../../../docs/bounded-ipc-recovery.md), sections “Start, inspect and stop,” “Busy, unavailable and recovery” and “Interaction decision and untested assumptions” | Documents protocol 1, status as liveness, doctor as SQLite integrity plus a one-migration check, bounded database work and inferred personas | `d690a0432c86a418557a0dac6bc772e6cc68cded79245678f5ed167f616317b5` |
| [Bounded IPC value scope](../../../docs/bounded-ipc-value-scope.md), lines 7, 27 and 45–56 | Separates inferred needs from participants; treats assessment offers and channels as untested; identifies missing commercial evidence | `f0f6f0f932eeab477d9ba99339b302b4c57853f283d880a6eb7c490647fc0026` |
| [Runtime identity value scope](../../../docs/runtime-identity-value-scope.md), lines 7–14 and 20–30 | Connects startup/restart preservation to expected operator value; excludes commercial expansion and measured financial claims | `85ef668df693c92241f420bdc05a53440dfe2231d9a9bdae59db46a464c9f311` |

The supplied migration marketing consultation is advisory input. Its observations about `crates/arc-storage/src/lib.rs`, `crates/arc-cli/src/main.rs` and `crates/arc-ipc/src/lib.rs` at baseline describe single-migration assumptions. They were not independently re-inspected as code here: Serena was absent from the callable tool inventory. The recovery document independently supports the documented one-migration diagnostic constraint. No code-search restriction was bypassed, and no consultation or historical test pass is promoted to current acceptance evidence.

## Inferred users and technical value

| Inferred user | Job served by this migration mission | Evidence boundary |
| --- | --- | --- |
| Local developer | Start or reopen the daemon after a binary upgrade, retain existing records and avoid repeating completed migrations | Existing start/status/doctor journey is documented in recovery guidance. Ordered startup migration, additive upgrades and preservation need the current deterministic tests. |
| Operator | Distinguish a compatible schema from altered checksums, missing history or a database newer than the binary; choose a supported recovery action | Existing guidance distinguishes liveness, database errors and foreground stderr. Explicit schema reasons are required mission outputs, not observed behavior in this report. |
| Integration maintainer | Support mixed client versions with truthful diagnostics and stable protocol-1 behavior; recognize unsupported downgrade attempts | Recovery guidance documents protocol 1 and legacy count validation. Mixed-version behavior needs executable compatibility cases; adding JSON fields alone does not prove it. |

These are functional inferences, not interviewed customers, qualified leads or paying accounts. Clear failure reasons could reduce diagnosis time and support effort. Atomic upgrades could reduce failed upgrade recovery. Neither effect has been measured in customer use. No claim of improved conversion, retention or profit follows from these inferred jobs.

## Claims that require executable evidence

Use final integrated source identities, test names, commands, exits and logs under `.arc-verify/schema-compatibility/verification/`, with independent review under `.arc-verify/schema-compatibility/review/`. The table states evidence needed before making each claim; it does not report these checks as passed.

| Eventual technical claim | Required evidence and limits |
| --- | --- |
| Fresh initialization and existing-database reopen work | Deterministic initialization/reopen tests using the production compiled registry; exact expected schema and complete migration-history tuples. |
| Ordered additive upgrades preserve existing records and are idempotent | Isolated two-version additive fixtures through the same engine; before/after application records, complete history and repeat-open comparisons. Fixture version 2 is not a shipped production migration. |
| Concurrent upgraders serialize safely | Explicit lock/barrier or channel coordination proving the second upgrader revalidates history after admission; final records/history contain exactly the expected results. Timing sleeps alone do not establish ordering. |
| Failed or interrupted upgrades roll back | Deterministic SQL-failure and interrupted-execution cases with known interruption points, reopened database checks and comparison of application data, schema objects and history. Limit any claim to the interruption cases tested. |
| Invalid histories are rejected without mutation | Altered checksums, missing history and unsupported newer schema cases compare application data and complete history before/after rejection. Do not imply that history repair or downgrade execution is supported. |
| Protocol-1 and older-client compatibility are preserved | Actual old-client/new-daemon and new-client/old-daemon cases, including legacy reports lacking compatibility detail; accurate legacy fields and explicit downgrade rejection. An old count-based decoder can retain limitations on fixture version 2. Never falsify counts or infer checksum verification from absent fields. |
| Diagnostics expose compatibility without losing runtime guarantees | Daemon/IPC/CLI cases for explicit schema status and startup rejection; regression evidence for bounded single SQLite worker, responsive admitted health, runtime identity and shutdown. Startup rejection precedes serving, so doctor cannot be promised reachable after that rejection. Health remains subject to transport admission. |
| Production inputs and source constraints are preserved | Compiled migration registry inspection; `0001_phase0.sql` byte comparison with the accepted baseline and SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`; protocol version 1 and source files within 500 lines. The hash here is the required reference, not a fresh SQL measurement by this task. |
| Required checks produced the reported outcomes | Final-source `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and unchanged `./scripts/harness/validate.sh`, retaining actual exits and logs. Retain unresolved coverage failure if present; do not convert a Cargo pass into a harness pass. |

Passing these cases supports tested technical behavior within their stated conditions. It does not prove all future upgrades, universal data-loss prevention, production incident reduction, customer demand or financial return. This task does not replace implementation, final verification or independent oma-qa.

## Delivery and commercial choices

| Route | Evidence available now | Additional burden | Recommendation |
| --- | --- | --- | --- |
| Complete ordered migrations and compatibility evidence | Concrete one-migration diagnostic constraint in repository guidance and the saved acceptance contract | Assigned implementation, deterministic tests and source-bound independent review | Finish this mission first. It creates reviewable reliability evidence without adding a sales operation. |
| Launch a paid SQLite upgrade-readiness assessment | Supplied consultation proposes Rust local-tool teams as buyers; no qualified buyer or accepted offer is established | Qualification, delivery capacity, quote/payment terms, attributable acquisition and support costs | Deferred proposal outside this implementation scope. Do not assign outreach or treat expected receipts as mission value already realized. |
| Promote through articles, communities, integrators or marketplaces | Supplied consultation lists possible direct and indirect channels; this task has no current channel validation | Authorized publication, channel eligibility, working distribution where needed, attribution and follow-up | Deferred proposal outside this implementation scope. Existing Agent Valley material does not establish ARC demand or distribution readiness. |

The consultation also suggests incident-driven consulting around public migration failures. That remains a deferred service hypothesis: no incident, funded opportunity, partnership or accessible customer list was verified here. No external channel rules or licensing rights were researched in this task.

If separately authorized later, a small assessment-offer test could compare paid uptake and estimated contribution against preparation, qualification, delivery and support effort. The consultation's five-prospect/six-hour suggestion is a proposed test budget, not an assignment, completed experiment or forecast. A promotion test could compare attributable qualified inquiries and collected payments against channel and labor costs. Neither should claim causality from a tiny sample or use downloads, stars, clicks or successful doctor runs as revenue. No price or expected return is set here.

## Commercial unknowns

Actual customers, willingness to pay, pricing, collected revenue, refunds, fees, acquisition cost and attribution, delivery/support costs, margin, ROI, CAC, LTV, adoption, retention and churn remain unknown in the supplied evidence and inspected documents. Unknown does not mean zero. The supplied operations data also reports unknown costs; recorded token usage does not establish total spend or profitability.

Any later financial claim needs attributable payments and actual acquisition/delivery/support costs. A retention or support-savings claim needs customer cohorts or comparable before/after observations. No activity, payment, customer response or financial result is inferred from repository content or this report. Service-sales and promotion proposals remain outside scope; this task performed no external research, outreach, publication, spending, pricing change or product edit.

## Checks and handoff

Skill: read local `.agents/skills/oma-market/SKILL.md` and applied its evidence/fabrication limits. Its research engine was not invoked because the assignment explicitly excludes external research; this is supplied-evidence synthesis, not a market-research engine result. Shared execution policy and i18n guidance were read. Only this owned file is delivered; CLI/protocol interfaces, product sources, manifests, shared registries and other Actors' outputs are unchanged by this task.

Checks executed: scoped `git status` before edits (exit 0, no changes); `git rev-parse HEAD HEAD^{tree} bf28a35b594f5ae83ac97c216bd3b09e60a2dfa1^{tree}` (exit 0, identities above); Python document hash/baseline byte comparisons (exit 0, all three match). One invocation incorrectly passed two refs to `git rev-parse --verify` and exited 128 (“Needed a single revision”). Separate checks for supplied refs `refs/agent-valley/task-evidence/fdf57c1d58d8223e-1724980aa084bbaa-1/snapshot` and `refs/agent-valley/task-evidence/fdf57c1d58d8223e-1724980aa084bbaa-1/worker` also exited 128 each: those refs are unresolved locally. The supplied snapshot commit exists and matches HEAD; no missing ref is substituted. Report-only reference, content, whitespace and ownership checks are recorded in the task handoff. Python invocations emitted mise tracking-symlink permission warnings but completed; no restriction was bypassed.

No Cargo checks, harness execution, runtime measurements, browser checks, installations or commercial experiments ran in this prose task. Those mission-wide executable gates remain with the implementation and verification owners. Remaining risks are unverified implementation acceptance and unmeasured user/commercial outcomes. No ownership or interface conflict was found.
