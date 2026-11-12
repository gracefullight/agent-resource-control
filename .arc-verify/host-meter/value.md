# Host Meter value and commercial evidence

Task: `task-4`, Marketing Director. Deliverable: this report only. The current
priority is trustworthy Meter evidence for macOS multi-agent developers. Price,
revenue, ROI, CAC, LTV, payback, retention and channel contribution are unmeasured.
There is no measured financial basis for spending on acquisition or expanding
this slice into a commercial platform. Native acceptance remains a prerequisite
to demonstrating the proposed diagnostic value.

The intended benefit is to help a developer inspect local resource pressure and
ARC's own cost while retaining the context of recorded job outcomes. This is a
value proposition derived from requirements and the task-1 interface contract;
it is not validated purchase intent, a customer outcome or a savings result.

The sources and their evidence boundaries are:

| Source | What was actually available to this task |
| --- | --- |
| [Authoritative SRS](../../docs/requirements/ARC_SRS_2026-11-06.md), sections 1.3, 1.5, 10 and 18 | Inspected requirements for multi-agent developers, resource accounting, headless operation and performance validation. Requirements describe intended behavior, not observed customers or completed features. |
| [Task-1 contract](contract.md) | Inspected the frozen Meter interface, host/daemon semantics, availability, freshness, ownership and required acceptance evidence. The document explicitly leaves native implementation, CLI, Dashboard and final acceptance to dependent tasks. |
| Supplied `dependencies[task-1].output` and independent Technical Director review | Reported locked compilation of four owned crates, 53 passing library tests and a successful contract review. These are attributed dependency results; this task did not rerun them. |
| Supplied `mission.marketingAdvice.review` | Prior Marketing Director advice supplies the three commercial routes compared below. Its planKey is `1273f31a4fddd5aee8e5eef6bbca312e053410f4fdbf41f4afdcf6a1af65d837`. It reports no commercial experiment, payment or outreach. No fresh market research was performed here. |

Provenance: the accepted committed baseline is
`efedf0f4eb9b97eed641c82f3c4766bb16f2e3de`, tree
`64dd3d10ba1458a42a4dcae1fa42e6bcbee43b7e`. This Actor's inspected local HEAD is
the supplied initial snapshot commit `602576f769e936cb5619a8ea9b4201e21e89fd5a`,
tree `962b8cd3085d2dce5b00292efbf001fed23cbc12`.
Git tree inspection found the SRS and contract tracked at that snapshot, and
neither path at the accepted baseline. Their presence here does not establish
their integration into another checkout. This report is an Actor-local task
delivery and was not committed by this task.

Both the Downloads source and repository SRS were independently hashed during
this task and match
`a8ef945251f9f3fd7c3e337d4efc77ff9914f95114212ca9e6c90ae2ce3e3d9e`.
The inspected contract's SHA-256 is
`6f135aa7900e6d6061e28e83e3be1bf4008f1ad9067a3bf8c3f3b8fa26c031af`.
The referenced `target/host-meter/task-1/handoff.md` is absent in this workspace;
raw compilation/test receipts were therefore unavailable for independent review.
The supplied snapshot ref name did not resolve locally; the actual commit and
tree above resolved by object ID. No replacement revision is asserted.

For the intended customer, the proposed slice offers these concrete uses:

| Proposed capability, subject to native acceptance | Possible customer use | Limit on the claim |
| --- | --- | --- |
| `arcctl meter snapshot --json` through authenticated, bounded `meter.snapshot`, with daemon-owned sampling independent of the GUI | Obtain a scriptable current reading without opening the Dashboard. | Contract delivery alone does not prove a working production CLI, release package or native measurement. No new telemetry or history export is promised. |
| Host CPU over an identified interval, available RAM, native memory pressure and used swap | Investigate whether the local machine is under pressure while multiple tools run. | Host totals include other local activity. They do not identify the vendor, managed job or cause of a slowdown. |
| Capacity, free and caller-available bytes for an identified local filesystem | See which filesystem was observed and whether space is low. | Derived disk pressure is a policy judgment, separately labeled from native observations. These are not disk writes, worktree footprint, reclaimed storage or savings. |
| Daemon cumulative CPU core-seconds, current RSS and observed peak RSS | Include the monitor's own resource cost in a diagnosis. | CPU has a process-lifetime window; peak RSS is a sampled maximum over its declared window. Neither measures all child processes nor proves an OS lifetime high-water mark. |
| Readings inline beside existing recorded job outcomes | Inspect current machine state without losing selected-job context. | Adjacency supplies context, not job attribution, a retrospective reading at job completion or evidence that ARC improved the outcome. |

Units, observation times, intervals and availability make those readings useful.
An initial CPU interval must remain pending; unsupported, denied or failed fields
remain unknown. Fresh, stale and unavailable states must remain distinguishable;
fetching an unchanged cached sample cannot refresh its observation time. A real
zero requires a successful measurement. These are contract obligations, not
completed usability or component-test results in this report. The contract uses
bounded current state and persists zero resource samples, so historical trends
cannot be inferred from this slice.

The audience comes from SRS section 1.3, not recruitment: developers using
multiple local coding agents or expensive build/test workflows on macOS. The
commercially interesting subset might have repeated disruption, inadequate
current diagnosis and a budget owner. That narrower segment and its willingness
to pay are assumptions requiring validation.

The supplied marketing advice cites [iStat Menus](https://bjango.com/mac/istatmenus/)
as an existing monitoring alternative, and user reports in
[Claude Code issue 63318](https://github.com/anthropics/claude-code/issues/63318)
and [Codex issue 32024](https://github.com/openai/codex/issues/32024) as possible
diagnostic themes. Those are prior-advice citations, not pages refreshed or
independently reproduced here. Product features, prices and thread status are
not asserted as current facts. The reports do not establish prevalence,
representative demand, permission to contact their authors or purchase intent.
The paid-value question is whether calibrated observations and interpretation
help enough beyond a user's existing tools to justify a fee.

The future commercial alternatives are qualitative hypotheses from that advice:

| Route | Concrete future offer or path | Economic hypothesis and uncertainty |
| --- | --- | --- |
| Paid diagnostic pilot | A fixed-scope assessment of an agreed local workload, using accepted CLI/Dashboard evidence, independent calibration and explicit ARC overhead; deliver an explanation of observations and limits. Quote labor plus an intended margin after costing the work. | The shortest proposed path from a diagnostic need to a paid deliverable, if a willing buyer exists. Sales, delivery and support consume labor; repeatability, price, willingness to pay and contribution are unknown. Do not promise to fix vendor leaks or reduce resource use. |
| Referral and support | With a participating maintainer's agreement, offer a troubleshooting recipe or referral to a separately scoped assessment/support engagement. The advice's OMA adjacency is a possible audience connection. | A relevant introduction might reduce acquisition effort, but access, reach, partner participation and buyer conversion are unverified. Referral shares, partner coordination and support load can erase margin. Existing credits links are not an approved campaign or a measured funnel. |
| Evidence-led technical content | After acceptance and separate publication authorization, explain a reproducible headless diagnosis with dated calibration, an honest capability table and a way for interested readers to inquire. | Useful evidence might attract developers already seeking a diagnosis. Writing, maintenance and response labor have a cost; views or repository stars do not establish qualified inquiries or paid contribution. No audience size, conversion rate or ROI ranking is available. |

Delivering trustworthy Meter evidence is the current priority across all three
routes. It is the common prerequisite to showing what the product can observe
and what the observation costs. A paid pilot is a candidate for the first later
test because it connects an offer to a costed deliverable; that is a sequencing
hypothesis, not a measured superiority claim. No route is launched by this task.

The commercial evidence gaps must remain explicit:

| Metric | Current evidence status and what would be needed |
| --- | --- |
| Price and purchase intent | Unmeasured. Need an actual dated offer, agreed currency/price and a buyer's acceptance or refusal. A complaint about another tool is insufficient. |
| Revenue and customer count | Unmeasured, not asserted as zero. Need attributable invoices, actual payment/refund receipts and distinct paying customers. Quotes and expressions of interest are not receipts. |
| Delivery cost, channel contribution and ROI | Unmeasured. Need fees, refunds, partner shares, direct costs and costed acquisition/preparation/delivery/support labor, allocated once. Passing technical tests is not profit evidence. |
| CAC | Unmeasured. Acquisition cash plus acquisition labor cost divided by new paying customers requires a recorded cohort and a nonzero paying-customer denominator. With no buyers in a future cohort, it is not a valid numeric estimate. |
| Retention and LTV | Unmeasured. Need defined cohorts, elapsed observation windows, voluntary reuse, repeat receipts and continuing costs. A running hidden daemon is not voluntary return; non-return alone is not proven churn. |
| Payback | Unmeasured. Need dated cumulative contribution covering recorded acquisition cost. Receipt timing without delivery/support costs cannot establish payback. |
| Customer savings | Unmeasured. Need a suitable comparative workload design and actual resource/productivity outcomes. Host observations and low daemon overhead alone do not establish savings. |

A small future validation hypothesis, unstarted and outside this delivery, is:
five qualified macOS multi-agent developers reached through an authorized opt-in
referral route will evaluate the same accepted evidence packet and a costed,
fixed-scope diagnostic offer; at least one paid, delivered assessment might yield
positive contribution after acquisition cost. Five participants, ten business
days, at most six preparation/sales hours and no advertising spend are proposed
planning bounds from the prior advice, not booked participants, elapsed work,
promised revenue or current authorization. Delivery/support effort also needs a
costed scope before any future offer; the six-hour bound does not erase that cost.

Any later authorized test would require the following actual evidence:

1. A consented, access-controlled ledger outside product telemetry: participant
   identifier, qualification and budget authority, referral source, consent and
   timestamps. Record denominators separately for eligible, opted in, shown the
   packet, offered, accepted, paid, delivered and refunded; do not assume all
   five reach every stage. Record refusals and missing observations explicitly.
2. A dated, identical evidence packet and offer: accepted native receipts,
   environment and limitations, the participant's current diagnostic method,
   quoted scope/currency/price and any changes. Observe whether the participant
   can distinguish host totals, daemon cost, stale readings and unknown values.
   An interpretation result is separate from an offer acceptance or payment.
3. Actual invoice/payment/refund references, fees, referral share, direct costs
   and preparation, acquisition, delivery and support minutes. Apply a disclosed
   labor rate, including owner time; do not treat unpaid effort as free or count
   it twice. Record repeat purchases and voluntary reuse only when observed.
4. Reconciliation of cash received less refunds, fees, referral shares, direct
   costs and all allocated labor. Compare channel contribution after acquisition
   cost, not clicks or gross receipts. CAC needs its own paying-customer count;
   LTV, retention and long-run payback stay unknown beyond the observed window.

The proposed decision rule is to fund a further cohort only after a delivered,
paid assessment has positive fully costed contribution and the diagnostic is
understood. Otherwise revise the audience, offer or price before further spend.
A missing qualified cohort is inconclusive. One positive case does not prove
scalable CAC, retention, LTV or a profitable channel; there is no numerical ROI
ranking for the three routes. This report creates no ledger, product telemetry,
funnel UI, subscription, licensing change or marketing feature.

The required idle check is a separate engineering observation: at least 300
seconds after documented warm-up, a noninstrumented daemon, desktop actually
hidden, independent observer and recorded environment. CPU is aggregate daemon
user-plus-system CPU-time delta divided by monotonic elapsed time, multiplied by
100, with one core as denominator; it must not be divided by logical CPU count.
The mission requires less than 0.5% and RSS below 50000000 bytes. At this snapshot,
`native.json` is absent and this task ran no idle measurement or calibration, so
no achieved value or pass is claimed. Even a later passing bounded idle result
cannot establish customer savings, long-term performance, active-job overhead,
battery benefit or a broadly representative user experience. SRS sections 1.5
and 18.1 call for outcome/resource comparisons; credible savings would need
matched ARC OFF/ON workloads, equivalent successful outcomes and measured
variation, rather than converting monitor readings into avoided cost.

Task-4 consulted `oma-market` and the shared execution policy. The saved task
restricts this work to existing evidence and this file; the market engine,
external source collection and its separate brief-output workflow were not run.
No fresh research result, engine badge, outreach, publication, solicitation,
experiment, spending or external delivery is claimed. No code, tests, manifests,
shared registries, task instructions, mission state or other Actor files changed.
The contract and seven-file evidence reservation remain compatible; `value.md`
uses its existing reserved slot. Native gates and all implementation remain with
the assigned backend owners; independent QA remains a separate delivery.

Executed inspection and document checks, with their limits:

| Command or read | Outcome |
| --- | --- |
| `git status --short --untracked-files=all` | Reported `av.yaml: Operation not permitted`; no full-clean assertion. Authoritative container configuration is not established by this report. |
| Scoped `git status` and `git diff` for `.arc-verify/host-meter` and requirements | No preceding changes in those scoped paths; `value.md` did not exist before this task. |
| `git status --short --untracked-files=all -- . ':(exclude)av.yaml'` after writing | Listed only the new, untracked `.arc-verify/host-meter/value.md`; the denied configuration path was excluded, not read. |
| `git rev-parse` and scoped `git ls-tree` at the baseline and snapshot | Resolved the actual hashes above and the two snapshot document paths. The supplied snapshot ref name was unresolved; this did not prevent reading the actual snapshot commit. |
| `shasum -a 256` on Downloads SRS, copied SRS and contract | Both SRS hashes matched the required hash; contract hash recorded above. |
| Scoped `sed`/document reads | Inspected the cited SRS sections and contract. Reading `target/host-meter/task-1/handoff.md` failed with `No such file or directory`; no receipt verification is inferred. |
| `git diff --no-index --check /dev/null .arc-verify/host-meter/value.md` | No whitespace diagnostics; exit 1 reflects the new-file difference under `--no-index`. |
| Inline Python document inspection | Passed whitespace, UTF-8, local-link, byte-budget and unchanged-input checks. These are document checks, not the mission's reserved evidence checker or product tests. |

No unit, integration, CLI, native, coverage, component, build or other gate was
executed by this task. Product acceptance, calibration, overhead and configured
container prerequisites require the assigned owners' current receipts. Remaining
scope includes Managed-job/vendor attribution, Native Windows, Vendor hooks and
Full Meter/Phase 1 acceptance, even if this host slice subsequently passes.
