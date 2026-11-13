# Governor slice: customer value and commercial judgment

Marketing Director handoff, task-3, attempt 2; 2026-11-13.

**Decision:** complete trusted scheduling control first: let a macOS developer release supported queued work while an existing build continues, then reliably restore admission rules. Commercial routes remain future experiments, with no additional implementation dependency or acceptance gate.

## Evidence and provenance

Accepted committed baseline: `08e7e526cce7e03ca9264afc926ed2e878dfde6d`. Inspected Actor snapshot HEAD: `c8c510598553881e6039b58906f152c82511e9ba`. Both resolve to product tree `d3cbd610653d0b78adeb2ffc26a9cab1790e877f`. This document is a new Actor-local change, absent from both commits; it does not prove an integrated implementation.

- **Repository observations:** [SRS](ARC_SRS_2026-11-06.md) §§1.1–1.5 describes contention and costly builds/tests; §7.3 specifies OFF behavior; §15.5 names default-ON `governor.enabled`. Inspected SRS SHA-256: `a8ef945251f9f3fd7c3e337d4efc77ff9914f95114212ca9e6c90ae2ce3e3d9e`. These are requirements, not demand measurements.
- **Historical evidence:** the [memory-pressure final report](../../.arc-verify/memory-pressure/final.md) records supported direct Cargo build/test/clippy/fmt and one-shot Bun test paths, fixture execution and pressure admission. Its passes are predecessor evidence; underlying receipts were not revalidated here.
- **Supplied external reports:** Marketing advice cites [Claude #101193](https://github.com/anthropics/claude-code/issues/101193) for reported Mac contention and a homemade semaphore, and [Codex #56441](https://github.com/openai/codex/issues/56441) for reported Linux linker exhaustion. These supplied reporter claims establish no independently verified ARC outcome, prospect consent or willingness to pay. Linux is outside this slice.

Method: loaded `oma-market`; synthesized supplied advice and scoped documents without a new market-engine run. Serena was unavailable; no substitute code search was performed. Initial Git status reported `av.yaml: Operation not permitted`; access was not bypassed.

## Specific customer problem and time to value

The customer context is a macOS developer routing supported Cargo or one-shot Bun jobs from several agents/worktrees through ARC. SRS personas are design targets, not recruited participants. With A running and urgent B queued, stopping A or resubmitting B risks wasted work and confusing outcomes. Effects on churn or lost sales are unknown.

Pause must let queued and reserved-but-unspawned current-client work proceed unreserved exactly once; new supported local requests bypass admission, including memory-pressure holds. Running children continue, permits reconcile and Meter keeps measuring. Resume restores window and fresh-Critical enforcement without promising immediate starts. Universal hooks, Windows, Sweeper and token savings remain outside this slice.

Trust requires owner-authenticated, revisioned SQLite state shared by CLI/desktop, acknowledgment after commit, durable OFF across restart and Active/Paused/unavailable feedback. Confirmed rejection restores prior UI state; uncertain acknowledgment triggers readback without claiming success or rollback. These requirements await Governor implementation evidence here.

**Judgment:** reliability is the present bottleneck. Lost state or duplicate work undermines any paid offer. Finish implementation, acceptance, gates and independent QA first. One usable control creates evidence for later claims; lower support burden and earlier credible offers remain hypotheses, not measured savings.

## Commercial alternatives: future experiments only

SRS §26.1 gives an open-source direction, with no pricing evidence. Routes below assume later authorization, voluntary participants and accessible channels. This handoff includes no outreach, publication, spending, telemetry or product-positioning change.

| Route after slice acceptance | Proposed test and assumptions | Evidence that would disconfirm expansion |
| --- | --- | --- |
| Paid macOS assessment/setup | Same fixed scope to five qualified teams. Supplied planning assumptions: six acquisition hours total, up to four delivery hours/buyer, no media spend. Within 14 days of qualified offers, seek two quote requests and one collected payment with positive contribution after acquisition. Access, price and effort are unverified. | No payment/quote interest or negative fully costed contribution: revise audience/offer. An invoice is not cash. |
| OMA/AV referrals or Rust-consultant introductions | Attribute one paid assessment to a voluntary referral; cost its share and support time. Supplied advice identifies [OMA](https://github.com/first-fluke/oh-my-agent) as an adjacent workflow source. No traffic, partner or agreement is verified. | No attributable payer or contribution below direct acquisition after referral costs: do not scale. |
| Unofficial troubleshooting/search guide | Affiliation-disclosed comparison with semaphores/concurrency limits via relevant, permitted community placements. Assume 4–6 labor hours, no media spend; supplied 2–6 weeks to payment is a hypothesis. Track qualified readers through quotes and cash. | Visibility without qualified trials/payments or acquisition cost exceeding contribution: stop/narrow. No unproven OOM-cure claim. |
| Catalog or manual adapter installation | Separately verify adapter and current vendor rules; compare installs-to-payers and maintenance cost with services. SRS §20.2 separates submitted, approved, available and local-install-only. Supplied advice proves no approval, distribution capability or traffic. | Ineligible listing, failed install or maintenance exceeding attributable contribution: defer. Views justify no expansion. |

No route has a known cash advantage. Services consume delivery labor; referrals add shares; content needs discovery; catalogs add engineering/review costs. Reliability comes first; this comparison proves no ROI.

## What acceptance measures

The required synchronized production-daemon/CLI/registered-bridge workload must show one-slot A-running/B-queued; pausing starts B once without a new permit while A survives; later paused requests bypass; permits release; OFF survives restart; resume restores window and fresh-Critical rules. SQLite, CLI and bridge reads must reconcile revisions, actual child counts, outcomes and permits. Required cases include duplicate/conflicting writes, backlog, reserved-launch races, cancellation, lost acknowledgments and reconnect, plus deterministic accessible-switch and Rust-derived TypeScript agreement checks.

Tests establish exercised-path correctness; readiness alone proves no child execution. Fixtures/injected pressure measure neither real build speed, native Critical/OOM prevention, editor responsiveness, human comprehension, retention nor commercial value. This task ran no Governor software checks and claims no acceptance result.

SRS G-04 needs a later matched ON/OFF and existing-workaround study: consenting teams, equal inputs/cache, alternating order, completion, Meter queue/run resources and foreground latency. Predeclare acceptable latency/throughput tradeoffs; reject the performance claim if it fails to beat the workaround or loses/duplicates work. This adds no release gate.

Demand, pricing, development/Actor/tool cost, CAC, LTV, payback, revenue and ROI are **unknown**: no customer, payment, costed-labor or cohort records were supplied. Unknown cost is not zero. Later evaluate cash minus refunds, fees and acquisition/delivery/support costs. CAC needs payers; payback needs cumulative contribution; LTV needs repeat-contribution history. Stars, passes and ON time cannot fill these gaps. Future records must respect SRS §17.5; this slice adds no telemetry.
