# IPC provisioning: value and scope decision

Status: completed task-4 decision memo. Owner: Marketing Director. Method: oma-pm using the installed pm-planner definition. Inputs: the immutable repair goal, supplied marketing advice, current source and the implementation report.

The immediate operator benefit is non-destructive startup: ARC creates missing socket directories privately, accepts valid owned 0700 directories, and rejects unsafe existing directories without changing their permissions, ownership or contents. Rejection must precede lock and database initialization. Preserving an operator's existing directory avoids collateral changes to other software; preserving default and ARC_SOCKET_PATH resolution lets valid configurations keep working. Reduced support effort and improved adoption are hypotheses, not measured outcomes.

Complete the focused repair and its required regression evidence first. This is the shortest authorized path to value because it fixes the supplied failure within the existing daemon, CLI and fixtures. The mission already defines success: unchanged rejected 0755/0770 directories and symlink targets, ownership rejection, private creation, valid 0700 startup, 0600 sockets and single-instance behavior, plus the exact pinned-toolchain gates. This memo adds no implementation requirements, interfaces or commercial completion gates. Fresh technical acceptance remains with the assigned reviewers and Chief Director.

The inspected [daemon startup](../crates/arc-daemon/src/main.rs) prepares the socket directory before the instance lock and SQLite opening; [provisioning](../crates/arc-daemon/src/server.rs) separates creation from validation. The [CLI](../crates/arc-cli/src/main.rs) exposes status and doctor over local Unix IPC. Its diagnostics explicitly report that tray connection, vendor detection and command classification are unavailable and the Sweeper is off. These are source observations; this task did not execute the daemon or rerun tests. The [implementation report](ipc-provisioning-implementation.md) records historical regression evidence and its verification limits.

The [Agent Valley PRD](prd.md) describes agent orchestration, Linear workflows and associated personas. It does not establish ARC customers, ARC demand or ARC revenue. Unix developer-tool maintainers and teams integrating local daemons are prospective audiences inferred from ARC's functionality; no buyer research or willingness to pay is established here.

The following remain optional future hypotheses requiring separate evidence and authorization:

| Route | Evidence needed before pursuing it |
| --- | --- |
| Paid integration or hardening pilot | A qualified buyer's problem, willingness to pay, scoped delivery estimate and actual payment evidence. |
| Technical article | A verified reproducible example, a relevant audience and a way to attribute qualified demand. |
| Integration partners | A willing partner, qualified demand, delivery responsibilities and referral economics. |
| Registry distribution | Publication rights, package readiness, name availability and evidence of integrator demand. |
| Sponsorship | Eligibility, a willing sponsor and sustainable maintenance obligations. |

Revenue, CAC, LTV, ROI and acquisition, delivery and support costs are unknown. The supplied advice contains proposed routes and assumptions, not observed financial returns. Unknown values are not zero, and technical test passes do not establish profit.

Acceptance review: directory preservation and reliable startup are the stated value; source observations, prospective audiences and commercial hypotheses are separated; no outreach, publication, spending, telemetry, pricing change or market research was performed. Only this memo was written. Verification is document inspection and scoped Git checks; no Cargo, browser or GUI checks are claimed by this task. The remaining risk is unproven commercial demand, which does not block the repair.
