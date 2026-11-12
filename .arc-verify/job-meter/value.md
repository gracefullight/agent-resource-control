# Managed-job Meter value and commercial handoff

Task `task-3`, Marketing Director, 2026-11-12. This report owns only
`.arc-verify/job-meter/value.md`. The proposed value is a trustworthy explanation
of one local macOS managed command: CPU used, time running, time waiting and its
recorded outcome. Product acceptance remains with the backend owners and
independent QA. Pricing, revenue, acquisition cost, CAC, ROI, LTV, retention,
payback and customer savings are **unmeasured**, not established as zero.

The [SRS](../../docs/requirements/ARC_SRS_2026-11-06.md), sections 1.3, 10.9 and
21.3, identifies multi-agent and expensive build/test developers and requires
job attribution, CPU time, duration, outcome and queue/run separation.
[Managed execution documentation](../../docs/managed-exec.md) describes direct
execution, lifecycle lookup and recovery without replaying work. These are
inspected documentary sources, not new runtime verification or buyer research.
The supplied `mission.marketingAdvice.review` supplies the paid-diagnostic
hypothesis; its planKey is
`e486726e2a9d8b84a5dd4578dfebdd7f7ae821aefc1640e903929200ed228655`.
Its channel assumptions are not measured reach, demand or conversion.

The useful distinction from a generic CPU chart is job context: a developer
could distinguish expensive computation from admission delay, retain exit-17
or signal context, and inspect a saved explanation without executing again.
That differentiation is a product hypothesis, not a measured competitive win
or demonstrated reduction in diagnostic time. A host CPU chart alone cannot
attribute past consumption to the selected command or explain queue wait.

| Claim after current acceptance | Evidence and boundary required |
| --- | --- |
| CPU used for the identified managed execution | User-plus-system CPU core-seconds, process identity, attribution basis, scope and completeness. Native termination accounting must include short-lived reaped descendants within `native_waited_descendants`; this is not an arbitrary complete process-tree total. Exclude unrelated pipeline members and do not sum overlapping nested totals. |
| Run elapsed and Queue wait explain different intervals | Monotonic spawn-to-termination duration, including stopped time, separately from identified queue wait. Unknown or unrecorded timing is not zero; reservations are not CPU consumption. |
| A saved result supports later diagnosis | One idempotent summary per job under the SRS 90-day metadata policy must reconcile independently through SQLite, `arcctl meter summary --job <JOB_ID> --json` and the production desktop bridge/selected-job detail. Preserve exit and signal semantics, redaction and single execution. Retrieval failure is not permission to rerun. |
| The number is calibrated within the stated scope | Current AT-MTR-01: two CPU workers for approximately ten seconds, including short-lived reaped descendants, compared with an independent native reference of matching scope. Absolute error must be at most `max(0.1 core-seconds, 5% of reference)`; nominal twenty core-seconds is not the reference. |

Only current, matching-input backend receipts in `native.json`, `gates.json`
and independent `qa.json` can support acceptance; this report does not certify
them. At inspection, `.arc-verify/job-meter/` did not exist. No current job
calibration, reconciliation or gate result was available here. The retained
[host-meter report](../host-meter/value.md) is historical context, not current
job-accounting acceptance. Any later capability claim must cite the integrated
revision, paths and current receipts rather than treating this Actor snapshot
as the completed product. Unknown, legacy, interrupted and Partial accounting
must keep their limitations visible. Complete applies only to the proven stated
scope. CPU core-seconds are cumulative CPU, not CPU utilization, token charges
or dollar savings.

A later **opt-in diagnostic pilot is not run** and is outside this mission's
completion scope. After accounting acceptance and separate authorization, a
fixed-scope assessment could pair a developer's repeated local-job problem
with accepted evidence and a costed explanation. This directly tests payment
for a delivered service. It would not validate recurring software demand or
promise a vendor fix, faster jobs, lower bills or resource savings.

The supplied proposal is five willing macOS agent-workload developers with
repeated diagnostic pain and budget authority, a ten-business-day observation
window, up to six acquisition/preparation hours and zero advertising spend.
These are future planning hypotheses, not recruited people or authorized work.
Delivery and support need their own costed scope; their labor is not covered
by pretending the acquisition cap covers everything. Voluntary inquiries or
agreed referrals are candidate routes, not proven low-CAC channels.

| Future commercial measure | Receipts needed before making a claim |
| --- | --- |
| Price and willingness to pay | Dated scope, currency and quote, accepted/refused offers and changes. Complaints, stars and clicks do not establish purchase intent. |
| Revenue and contribution | Actual payments, refunds, fees, partner shares, direct costs and acquisition/preparation/delivery/support minutes at a disclosed labor rate, including owner time. Allocate every cost once. |
| CAC and ROI | A consented cohort with source/referrer and separate eligible, opted-in, offered, accepted, paid, delivered and refunded counts. CAC is acquisition cash plus acquisition labor cost divided by new payers; it is undefined with no payers. ROI requires net receipts and all attributable costs. |
| Retention, repeat demand and LTV | Dated cohorts, elapsed observation windows, voluntary reuse, repeat paid deliveries, refunds and continuing costs. A resident daemon is not voluntary retention; one service sale does not establish recurring demand or LTV. |
| Customer savings | Repeated comparable workloads with equivalent successful outcomes and measured CPU, elapsed time and diagnostic effort against the user's existing method. A calibrated reading alone does not show avoided expenditure. |

For a future pilot, contribution before acquisition equals receipts minus
refunds, fees, partner shares and delivery/support costs; subtract acquisition
and preparation once to assess fully costed contribution. CAC uses the same
cohort's acquisition costs, not a second deduction. ROI equals
`(net receipts - total attributable costs) / total attributable costs` when
costs are positive; payback requires dated contribution covering acquisition.
No numeric estimate is supported today.

A future, separately consented research ledger would record qualification,
permission to participate, source, offer exposure, receipt references and labor;
product telemetry is not a substitute. Workload-evidence sharing needs explicit
permission and redaction. SRS section 17.5 restricts telemetry to aggregate
anonymous values and excludes paths, argv, project names and PIDs. This task
creates no ledger or telemetry. A proposed continuation rule is positive fully
costed contribution from a paid, delivered diagnosis plus demonstrated ability
to interpret scope, CPU/run/queue and Unknown/Partial states without rerunning.
Otherwise revise segment, scope or quote before another cohort. Failure to
recruit is inconclusive; one positive case proves neither scalable acquisition
nor retention. The six engineering gates remain unchanged by this hypothesis.

Provenance: accepted `baselineHead`
`18091eeb1ac0232dfdda4a937113fa3bd149b220`, `baselineTree`
`c1ed103170701e587991486e318eea47d42d709b`. Inspected Actor-local HEAD and
supplied `snapshotCommit` are `3c792e2af68e9a37800e9048a869f10d556e8c60`;
its inspected tree equals supplied `snapshotTree`
`c1ed103170701e587991486e318eea47d42d709b`. The SRS, managed-exec documentation
and historical value report are tracked at the accepted baseline. The supplied
snapshot ref did not resolve locally; the commit object did. This new report
is an uncommitted Actor-local deliverable, not an accepted-baseline change.

Skill used: `oma-market`, with the shared execution policy and supplied evidence.
`oma market detect-trap` exited 0. This task synthesizes existing inputs; fresh
market collection, engine execution and its additional output files were not
run under the one-file assignment. No fresh community-research result is
claimed. There was **no outreach**, publication, solicitation, external
experiment or spending. No product, test, manifest, positioning, assignment,
mission state or other Actor file was changed. Native and desktop checks remain
with the assigned backend owners. Full `git status`/unstaged inspection reported
`av.yaml: Operation not permitted`; scoped evidence status and staged inspection
showed no preceding changes. No whole-worktree clean claim is made.

Vendor/project aggregation, live process-tree metering, Windows, vendor hooks
and full Meter acceptance remain outstanding. This commercial handoff adds no
requirement to launch a pilot before the accounting mission can complete.

Document checks passed for UTF-8, required claims-boundary terms, local links,
whitespace and the evidence file/byte budget. The new-file
`git diff --no-index --check` emitted no whitespace diagnostics; exit 1 denotes
the file difference. Scoped status lists only this new report in job-meter.
These are document checks, not the mission's reserved evidence test or any
unit, integration, CLI, native, desktop, coverage or build gate.
