# ARC bounded IPC: customer value and evidence limits

Status: task-7 value-scope report, attempt 4, 2026-11-08. Owner: Marketing Director. Only this document is delivered by this task. The [task-1 contract](bounded-ipc-contract.md), [target-source report](bounded-ipc-targets.md), task-1 attempt-4 handoff and supplied marketing consultation inform this assessment. Technical acceptance belongs to the implementation, independent review and final mission report.

## Customer need and delivery priority

Local-tool developers need a daemon that responds, rejects work predictably when busy and recovers without manual cleanup. Operators investigating a stalled tool need to distinguish liveness from database integrity and stop their daemon promptly. Integrators need stable framing, errors and resource limits. These needs and audiences are inferred from ARC's functionality and the assigned failure cases; no customer interviews, participant observations or buyer commitments were collected.

At accepted committed baseline `42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0`, `crates/arc-daemon/src/server.rs:307–316` starts an untracked OS thread per accepted connection; its connection loop uses blocking frame reads/writes without server deadlines and runs doctor under a SQLite mutex. A stalled client can retain work without a documented server expiry. This was confirmed with `git show` of that exact revision. It is source evidence, not a measured incident rate, support bill or production outage.

Predictable failure and recovery therefore take priority over promotion or new features. The selected repair bounds admission before allocating bodies or spawning handlers, tracks Tokio tasks, applies absolute deadlines and sends database work to one worker through a finite queue. This attempt's input snapshot already contains that implementation. Direct inspection of `admission.rs` confirms nonwaiting permits and a tracked JoinSet; `connection.rs` applies frame/write deadlines and routes only doctor through the database adapter. These observations describe local snapshot source, not fresh runtime results or code in the accepted baseline. Fewer hangs, retries, abandoned sessions and support requests are plausible benefits; their frequency and financial value remain unmeasured. Passing a load test would establish tested behavior, not customer demand or profit.

## Product boundary

The [workspace manifest](../Cargo.toml) lists five Rust crates: core, storage, IPC, daemon and CLI. The [client](../crates/arc-cli/src/main.rs) exposes `status` and `doctor`; it does not open SQLite. The [protocol dispatcher](../crates/arc-ipc/src/lib.rs) supplies hello/health and doctor responses. Successful doctor text explicitly reports tray not connected, vendors not detected, command classification not checked and Sweeper off. These are inspected source capabilities and limitations, not fresh executable results from this task.

The [Agent Valley PRD](prd.md) describes a different, broader orchestration product: issue intake, agent sessions, parallel worktrees, dashboards and cost routing. Its personas, feature tables and business targets do not establish delivered ARC features, customers or returns. ARC claims must stay within this local daemon/status/doctor slice. Remaining Phase 0 features, Linux qualification and Windows runtime/Named Pipe work remain obligations identified by task-1; this report does not complete them.

## Scope choices

| Option | Plausible value | Evidence and added work | Decision for this mission |
| --- | --- | --- | --- |
| Repair bounded IPC and recovery | Predictable status, diagnostics and shutdown under stalled or excessive local clients | Concrete source defect and an existing acceptance contract; operational and customer effects still require evidence | Deliver the saved repair and preserve existing CLI behavior |
| Add CLI retries, commands or progress controls | Could make waiting or recovery easier | No observed need or usability result; adds behavior, compatibility tests and possible retry traffic | Defer; retain the two-command journey and documented fail-fast recovery |
| Test a paid daemon-hardening assessment | Could serve teams maintaining local tools with a matching reliability problem | No identified buyer, agreed price, payment, delivery estimate or acquisition result | Defer; an optional later experiment, never a repair prerequisite |

The supplied consultation suggests a scoped assessment followed by an optional integration engagement. That is an offer hypothesis. Before a separately authorized test, establish a matching problem, budget owner, delivery/support estimate and attributable payment evidence. Proposed channels such as technical articles, direct inquiries, partner referrals, registry distribution and sponsorship are untested routes here. Neither those routes nor unrelated product, packaging or channel work belongs in this delivery. No outreach, publication, spending, recruitment, packaging or commercial experiment was performed.

## Evidence that could support later claims

The following claims are conditional. This document does not mark the repair, measurements or checks complete. Use the final integrated report and its exact source/artifact identity before making any delivery claim.

| Completed evidence needed | Claim that evidence could support | Limit to retain |
| --- | --- | --- |
| Deterministic unit, subprocess and CLI tests reconciling 100,000 attempts and all connection/request/queue/task high waters | Configured application caps held throughout the tested workload and rejection stayed bounded | Test profile, concurrency, outcomes and retained-task definition; not unlimited capacity or a measured efficiency gain |
| Stalled-header, partial-body, trickle-input and non-reading-peer tests, followed by status/doctor recovery | Capacity was released within documented deadlines and the tested CLI recovered | State actual deadlines and observed results; rejection is not successful request completion |
| Controlled slow database work with available transport slots and a successful health exchange | Admitted health work did not wait for SQLite in that case | Complete connection/request saturation can still reject health; no unconditional availability claim |
| Separate SIGTERM/SIGINT measurements below five seconds, all handlers reaped, worker joined, database reopened and integrity checked | Tested graceful shutdown accounted for its work and preserved database integrity | Graceful shutdown does not establish every forced-crash or platform case |
| Protocol/CLI, same-user authentication, TMPDIR-independent identity, single-daemon and filesystem regression checks; byte comparison of `0001_phase0.sql` | Tested compatibility and preservation boundaries held | Identify inspected integrated revision and platform; keep the migration unchanged |
| Native idle RSS, lifetime peak RSS, sampled native threads, instrumented task high waters and successful small-message round-trip P95 | Actual resource use and latency under disclosed conditions | Structural caps are separate evidence; task-1 records SRS NFR-002 idle RSS <50 MB and NFR-004 P95 ≤20 ms, not a saturation peak-RSS threshold |
| Exact Cargo gates and unchanged harness execution with retained failures | The named checks produced the reported outcomes | Report unresolved harness failures and remaining Phase 0/Linux/Windows work; no blanket completion claim |

Final evidence belongs in `.arc-verify/bounded-ipc/qa.json`, `.arc-verify/bounded-ipc/performance.json`, `.arc-verify/bounded-ipc/performance.md`, `.arc-verify/bounded-ipc/harness.txt` and `docs/bounded-ipc-final.md`. This task does not independently verify their reported results. The required commands remain `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and `./scripts/harness/validate.sh`; this prose task does not rerun or replace the assigned technical verification.

## Commercial evidence boundary

| Item | Current evidence status |
| --- | --- |
| Price and willingness to pay | Unknown; no quoted or accepted ARC offer established |
| Demand and paying customers | Unknown; inferred audiences are not buyers or leads |
| Acquisition volume, attribution and cost | Unknown; no campaign or inquiry-to-payment record collected |
| Engineering-time savings and support reduction | Unknown; no before/after customer workload or cost study |
| Conversion, adoption, retention and churn | Unknown; no observed cohort or experiment |
| Revenue, delivery/support costs, margin and ROI | Unknown; no attributable financial result |

Unknown inputs are not zero. Neither acceptance passes nor more retries establish retention, savings or revenue. A later commercial assessment would need collected payments, refunds/fees, acquisition labor/expense and delivery/support cost before calculating contribution or ROI. No return, price or time-to-revenue forecast is asserted here. Commercial uncertainty does not block the authorized reliability repair.

## Provenance and document checks

Accepted committed source baseline: `42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0`, tree `c802f71ac4d91ed6b8a602d5c83bf9cf57116b76`. Attempt 4 inspected local snapshot HEAD `32d305405c70711a56482fc55e8b5b0972970199`, tree `2ee8c1b555aceeaed149c7315e4feb32a8e79e5e`; both trees were recomputed with `git rev-parse`. This snapshot contains the task-1 documents, prior value-scope report and bounded runtime implementation beyond the accepted baseline. The cited CLI and PRD files match the accepted baseline; the server and protocol files differ. Do not attribute snapshot additions or this attempt's uncommitted edit to the accepted baseline or assume they are tracked in another checkout. The task-1 handoff describes its source workspace at the accepted HEAD with uncommitted implementation; that is a different checkout from this Actor's committed input snapshot.

The initial worktree already had an uncommitted edit to this report, SHA-256 `890d6c3602976705bf7e4c748316000c39a2799894222bbd2a2c27d160cda49d`. This resumption preserves its substantive content and corrects the attempt and verification records. No other accessible path was changed on entry.

Inspected task-1 input SHA-256 values:

- `docs/bounded-ipc-contract.md`: `989a8d0fcfbffc427d2c952494771065248358f3211f0044fa764dd4810ad8d0`.
- `docs/bounded-ipc-targets.md`: `991aa83a4c7d721a3b4eec303b635aa5df88cd7830918aa5309b1ef3cff41ddd`.

Both supplied sourceEvidence refs, `refs/agent-valley/task-evidence/d6c6b45f4a6c3769-014e4ceaa5a9e511-3/snapshot` and `refs/agent-valley/task-evidence/d6c6b45f4a6c3769-014e4ceaa5a9e511-3/worker`, were unresolved here (`git rev-parse --verify` exit 128 each). The supplied snapshot commit itself exists and equals inspected local HEAD. No missing ref is substituted with another checkout's revision. Initial unscoped status reported `av.yaml: Operation not permitted`; status excluding that path worked. No denied read was bypassed. Serena tools were absent from the available inventory, so source inspection used direct reads of known paths and scoped Git comparisons.

Skill used: `oma-docs`. Manual inspection — `oma docs verify` did not run because its generated `docs/generated/doc-refs.json` is outside this task's sole owned file. Local Markdown targets, required report content, whitespace and the single-file change boundary were checked. The supplied `oma-market` instructions were considered for applicability; its research engine was not invoked because this assignment consumes supplied evidence and defers new market/channel work. No fresh market research, Cargo gate, runtime measurement or screen check is claimed. The owned report itself is the material deliverable; no shared registry or other Actor's receipt was modified.

| Executed check | Outcome |
| --- | --- |
| `git status --short -- . ':(exclude)av.yaml'` before edits | Exit 0; inherited modification to the owned report; no other accessible path changed |
| `git diff --exit-code 42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0 HEAD -- crates/arc-cli/src/main.rs docs/prd.md` | Exit 0; CLI and PRD match the accepted baseline |
| `python3 -B` local reference, newline/whitespace, required-content and input SHA-256 checks | Exit 0; six local references resolve and both task-1 hashes match |
| `git diff --exit-code 32d305405c70711a56482fc55e8b5b0972970199 -- <path>` for task-1 inputs, `admission.rs` and `connection.rs` | Exit 0 for each; inspected local source and dependency documents match the supplied snapshot |
| `git diff --check -- docs/bounded-ipc-value-scope.md` | Exit 0; no whitespace diagnostics |
| Scoped Git status and `git diff --exit-code HEAD -- . ':!av.yaml' ':!docs/bounded-ipc-value-scope.md'` after edits | Exit 0; only the owned report changed |

No installation, GUI fallback or permission change was used. The supplied final-review findings about missing task-6 receipts and inaccessible configured-target evidence remain Chief Director coordination items; this task neither repairs other Actors' files nor treats those obligations as complete.

Acceptance: the report ties plausible developer/operator needs to the concrete defect and selected repair; it distinguishes source observations, technical targets, future evidence and commercial hypotheses; it adds no mission prerequisite. CLI/protocol interfaces, shared manifests and other Actors' files are unchanged by this task. Remaining risks are unproven customer and financial outcomes and the still-required integrated technical acceptance.
