# ARC desktop foundation: value, interface copy and commercial boundaries

Task-3 report for the Chief Director and task-6 interface owner. Prepared 2026-11-12. This is a source-based positioning and copy handoff; desktop implementation, runtime acceptance and commercial results are not established by this report. Only this file is owned and changed.

## Evidence and provenance

Accepted committed baseline: `a9e7a31566e833dceeb82d6d75cd7c6a399583ab`; its verified product tree is `c4506e64abaef97daf6f701cac2870c98cfeb91f`. Actor-local HEAD is the supplied snapshot commit `c97b745c55a2ac47903726d532c325637a1b5e11`, whose verified tree is also `c4506e64abaef97daf6f701cac2870c98cfeb91f`. All six repository sources below were byte-compared with their accepted-baseline Git objects and matched. The new `value.md` is an Actor-local task edit, absent from that baseline; it is not a claim about an integrated review checkout.

The supplied `snapshotRef` and `workerRef` under `refs/agent-valley/task-evidence/8bb32cc241dc9a3c-1724980aa084bbaa-1/` did not resolve in this Actor checkout (`git rev-parse --verify`, exit 128). The supplied snapshot **commit** resolved directly; no substitute revision was used. Initial staged, scoped unstaged and untracked checks found no preceding edits in the inspected scope. `git status` emitted `av.yaml: Operation not permitted`; that path is unverified, not reported deleted or inspected.

| ID | Inspected source and evidence class | Relevant evidence |
| --- | --- | --- |
| R1 | [README.md](../../README.md), lines 1–3; committed documentation | Names ARC and links contributor coverage checks; supplies no customer or commercial results. |
| R2 | [docs/prd.md](../../docs/prd.md), lines 1–19; committed documentation | Names Agent Valley and describes Linear-driven agent orchestration. This differs from the assigned ARC product. |
| R3 | [docs/managed-exec.md](../../docs/managed-exec.md), lines 12–34, 48–76; committed behavior documentation | Defines the direct-execution scope, up to 50 recent last-known records, recorded exit results, uncertain tracking and observational recovery without replay. Desktop integration, Windows and vendor hooks remain outstanding in this baseline document. |
| R4 | [crates/arc-ipc/src/jobs/types.rs](../../crates/arc-ipc/src/jobs/types.rs), lines 130–162; committed source inspection | Detailed snapshots contain optional outcome and observation/update times. Recent summaries contain phase and timestamps, without an outcome. |
| R5 | [crates/arc-ipc/src/lib.rs](../../crates/arc-ipc/src/lib.rs), lines 368–374; committed source inspection | Health replies identify a running daemon, protocol version and Sweeper off. These fields do not measure host CPU, memory, savings or vendor health. |
| R6 | [crates/arc-cli/src/job.rs](../../crates/arc-cli/src/job.rs), lines 219–300; committed source inspection | Exit 0 maps to succeeded; nonzero to failed. A completed phase alone remains outcome unknown. Signal, spawn failure and cancellation have separate labels. |
| S1 | `/Users/gracefullight/Downloads/Agent_Resource_Control_ARC_Requirements_SRS_2026-11-06.md`; external local requirements, version 1.1.0-draft in front matter | Lines 148–168 (§§1.3–1.5): local coding-agent/build users and exclusions. Lines 2047–2057, 2107–2119, 2178–2195, 2224–2234 (§15): private compact utility, lifecycle, freshness, unknown values and exact credits. Lines 2389–2399 (§17.5): privacy requirements. Lines 3121–3135 (§22): broader Phase 0 DoD. Lines 3410–3416 (§26.1): proposed license and vendor-name boundaries. Requirements describe intended behavior, not delivery proof. |
| M1 | Supplied `mission.marketingAdvice.review.findings`; prior advisory, not independently measured demand | Supplies pilot/referral/discovery hypotheses and measurement proposals. No new community or external-channel research was performed for this report. |

Source SHA-256 values, recorded from the inspected bytes:

```text
R1 20daf6024c582bcbeb2d64e4596a3124f44125a3cdecc641d3da4556f6afe5f6
R2 2a87a8090ae8d90a7b50d4eeb92e17d6c1e5fcdbf70853a21760faebab9b7a2a
R3 37d3fa4497330aec587477d6262b138aa894b744e84b6e10c1e78b983432ddce
R4 9459b9a22f0e3cd2ca2c921e5039117007adc928354587612759ad840380ff7b
R5 17ff544a163bf7c831b8f3c84092c03047eb01498a9696c2004fe888229bdf46
R6 c2024ede50b9f4073cb7979f5f6292b04365c2a9cba5ac98721240eed7e93ccb
S1 a8ef945251f9f3fd7c3e337d4efc77ff9914f95114212ca9e6c90ae2ce3e3d9e
```

Serena tools were unavailable in the tool catalog. Inspection used scoped reads of the already supplied paths, plain-document searches and Git object comparisons; no replacement project-source search was used. The configured `oma-market` skill was read and its evidence boundaries applied. Its community-signal engine was not run: this assignment synthesizes supplied advice and repository/SRS evidence, owns one report, and makes no fresh market-research claim. No engine badge, audience estimate or validated channel ranking is implied.

## Positioning and current value hypothesis

ARC means **Agent Resource Control**. For this foundation, position it as a compact desktop companion for developers inspecting local ARC-managed work. The intended users are developers running concurrent coding agents and costly local build/test jobs [S1 §§1.3–1.4]. The narrower initial audience of macOS Rust/Bun developers matches the documented direct-command slice [R3 lines 18–34]; automatic coverage of every language or vendor is not established.

**Hypothesis V1 — reliable job inspection and recovery visibility:** selecting a recent job, reading its recorded outcome and seeing the last successful check when tracking is unavailable will help developers interpret background work with less investigation. Reduced investigation time, fewer mistaken rerun intentions and willingness to pay are unmeasured. The proposed first-value journey is Open Dashboard → select a recorded job → identify its recorded result and freshness. This is a proposed journey, not a timed user study.

The foundation's intended capability is bounded inspection: actual daemon connectivity, recent last-known records, selected-job details, explicit timestamps and unknown/stale/disconnected states [R3–R6; S1 §15]. Health-query success proves only that exchange, not every subsystem's health or child success. An empty or missing record does not establish that no command ran [R3 lines 48–74]. Recent history is not a global running/queued count. Missing CPU, memory, reclaimed-space or agent-count metrics remain **Unknown**, never zero or estimated savings [R5; S1 §15.8].

`docs/prd.md` is a positioning mismatch [R2], not authority to add Actor rosters, Linear issues, autonomous coding or merge controls. Leave it unchanged in this task and flag it to the Chief for any separately assigned documentation correction. ARC excludes agent orchestration, remote CI management, token/subscription management and remote chat deletion [S1 §1.4]. Do not promise vendor-leak repair, browser containment, resource savings, universal plugin coverage or full Phase 0 delivery from this desktop slice. Existing documented scheduling is not new desktop functionality [R3].

## Copy handoff for task-6

These are proposed English strings for the delivered interface, not statements that the desktop already exists or passes acceptance. Task-6 should bind them to actual bridge data and the shared freshness contract; this report adds no DTO, refresh interval or business feature.

| Surface / condition | Suggested wording | Evidence / binding |
| --- | --- | --- |
| Product name | **Agent Resource Control** | Exact SRS §15.9 label. |
| Dashboard description | **Inspect recorded local jobs.** | Bounded inspection positioning [R3–R4]. |
| First-use help | **Select a recent job to review its last recorded state. Refresh checks tracking; it does not run the command again.** | Observational recovery [R3 lines 64–74]. |
| Fresh successful health exchange | **Daemon connected** · **Last successful check: {client receipt time}** | Use only after validated health; the check time is not a daemon observation time [R5]. |
| Recent list | **Recent jobs (up to 50)** | History limit, not live totals [R3 line 48]. |
| No selection | **Select a job to view recorded details.** | Does not infer a result from a summary [R4]. |
| Validated detailed exit 0 / exit 17 | **Succeeded · Exit code 0** / **Failed · Exit code 17** | Recorded outcome required [R6]. Other nonzero codes retain their actual value. |
| Missing outcome, including bare completed phase | **Outcome unknown** · **No recorded exit result is available.** | Do not turn a completed phase or successful lookup into success [R4, R6]. Preserve separate signal/spawn-failure/cancellation states when the delivered contract supplies them. |
| Detail timestamps | **Record observed: {daemon observation time}** · **Record updated: {record update time}** · **Received: {client receipt time}** | Separate source observation/update from receipt [R4]. Use Unknown for unavailable times; expose the exact time alongside a readable local rendering. |
| Validated empty recent reply | **No recorded jobs yet.** | Only after a successful empty query; not during initial loading or an outage [R3]. |
| Validated missing selected ID | **No matching record. A missing record does not show whether the command ran.** | No implied replay or failure [R3 line 72]. |
| Refresh pending / lookup failure | **Loading recorded jobs…** / **Status unavailable. Refresh status.** | Neither condition changes a recorded job result. Malformed/incompatible replies must follow the bridge's error state, not become an empty list [R3 lines 78–82]. |
| Connection lost, retained data | **Disconnected** · **Stale data — last successful check: {time}** · **Retry connection** | Retain known results and their timestamps; do not label them current or erase exit 17 [S1 §15.7; R3]. If no earlier check exists, say **No successful check yet.** |
| Unavailable metric | **Unknown** · **Not available from this daemon response.** | Applies to missing resource/global totals [R5; S1 §15.8]. |
| Window and tray help | **Closing this window hides ARC Desktop.** · **Quit Desktop (Daemon continues)** | Intended lifecycle [S1 §§15.4, 15.10]; runtime/survival proof belongs to engineering and QA. |

Keep this copy out of outcome values and diagnostics that have a shared canonical label; adapt presentation without changing their meaning. Do not add a rerun, cleanup, signup, payment or referral-tracking action. Command/chat text is not needed to inspect job identity, state and timestamps [R3 line 50; S1 §15.2]. Avoid a blanket “nothing is ever uploaded” promise: S1 §17.5 separately describes proposed aggregate telemetry, and this assignment neither implements nor verifies it.

Preserve the fixed About/Credits contract [S1 §15.9]: `Agent Resource Control`; `Created by Eunkwang Shin`; Phosphor `GithubLogo` icon-only to `https://github.com/gracefullight`; Phosphor `LinkedinLogo` icon-only to `https://www.linkedin.com/in/gracefullight`; and exactly `Try OMA` to `https://github.com/first-fluke/oh-my-agent`. The icon links need accessible names, tooltips and keyboard focus. Version, available build commit, OS architecture, license and open-source notices must reflect authoritative metadata; the SRS's proposed Apache-2.0 value is not proof of an adopted product license.

**Try OMA is a required outgoing credit.** It establishes no incoming OMA placement, referral agreement, traffic, lead or sale. Leave the exact destination intact; do not add attribution parameters or instrumentation. Any future reciprocal placement needs its own evidence and authorization.

## Future commercial opportunities — all hypotheses

| Opportunity | Proposed route and value hypothesis | Prerequisites and cost uncertainty |
| --- | --- | --- |
| **Hypothesis C1 — focused setup/reliability pilot** | Qualified macOS Rust/Bun developer → scoped quote → paid installation and local-job tracking assessment → optional support. Buyers with an immediate inspection problem may value a narrowly supported setup. | Verified foundation, reproducible supported environment, authoritative installation/notices, an agreed scope and a collected payment. No price, payer or delivery duration is established. Manual acquisition, delivery and support labor can erase margin. Service payment would validate that service, not recurring software demand. |
| **Hypothesis C2 — OMA-context referrals** | Separately agreed contextual guide/referral → voluntary ARC trial → the same paid setup offer. Parallel-workflow users may fit the intended audience. | Permission and actual placement, qualified users and supported compatibility. None is established here. Cash spend might be low while content, integration and support labor remain material; the outgoing credit supplies no incoming reach. |
| **Hypothesis C3 — troubleshooting-intent discovery** | Relevant, affiliation-disclosed diagnostic guidance/reproducer → voluntary trial → scoped paid diagnosis/setup. An existing local tracking problem may create stronger intent. | Independently verified matching cases, relevant contribution rules, explicit authority for any outreach/publication and an accurate supported-scope demonstration. No prospect list, current channel access or demand count was validated. Triage time is acquisition cost. Do not imply ARC fixes a vendor memory leak or controls unsupported work. |

Proposed priority: test one narrow C1 offer, treating C2 and C3 as acquisition-source hypotheses for that same offer. Compare collected contribution per acquisition hour instead of building three funnels or ranking channels by imagined reach. This is an execution judgment for a later authorized experiment, not measured channel superiority. No payment-date forecast is supported.

Delivery proof precedes commercial experiments because the paid promise is trustworthy inspection. A green badge based on missing exit evidence, a stale result shown as live, or a desktop quit that stops work would invalidate that promise. Before a pilot, obtain fresh foundation evidence: frozen-stack compatibility; accurate health/detail/unknown states; deterministic component and registered lifecycle-handler tests; production-bridge tests against an isolated real daemon for exit 0/17, reconnect, invalid replies and daemon/active-child survival; the six mandated gates and per-crate coverage; and independent read-only QA. This restates the supplied mission dependency, not an added acceptance gate. This task ran none of those engineering checks and claims none passed.

Live native tray interaction, Windows acceptance and **full Phase 0 completion remain unverified by this report**. S1 §22 requires two-OS daemon/tray/CLI proof and other work beyond a macOS foundation. Vendor integrations and broad resource metering/cleanup remain separately scoped; they cannot be advertised from fixture existence or future SRS features.

## Later pilot proposal and measurement definitions

**Hypothesis E1 — paid pilot experiment, not authorized execution:** after technical acceptance and explicit commercial authorization, consider six qualified macOS Rust/Bun multi-agent developers, with three sought from each of C2 and C3 if access is actually secured. Qualification means an immediate supported local-job inspection problem and willingness to evaluate a scoped setup. Use one clearly quoted scope and fee over 14 days, no ad spend and a proposed ceiling of six acquisition staff-hours; record delivery/support hours separately. These sample, time and budget choices are planning assumptions, not a forecast, established buyer pool or statistically conclusive test. This report sets no price and starts no recruitment.

Suggested directional continuation rule: at least two collected payments, total pilot contribution covering acquisition cost, and at least one payer deliberately returning by day 7. Report all denominators and revise audience/offer if the rule is unmet; do not relabel quotes or free trials as payers. The thresholds come from M1 and are proposed decision rules, not observed results.

| Measurement | Definition for a later authorized manual pilot |
| --- | --- |
| Activation / first value | Participant correctly identifies a selected recorded outcome and its freshness. Report activated/attempted users and time from Dashboard open to correct interpretation; keep failed and unknown trials. Compare the current CLI and desktop on matched exit-17 and disconnected/unknown tasks, accounting for order effects. Saved time requires that comparison and is currently unmeasured. |
| Paid conversion | Unique payers with collected payment / qualified participants offered the same scoped service. Report quotes, trials, collections and refunds separately, by voluntarily supplied source and unknown attribution. |
| CAC | (Channel cash + acquisition labor cost) / new unique payers. Include research, content, referral and triage time; disclose labor-rate assumptions. With zero payers CAC is undefined, not zero. |
| Contribution | Collected fees − refunds − payment fees − delivery/support labor and direct service costs. Record acquisition separately to compare contribution per acquisition hour. Invoiced but uncollected fees are not revenue evidence. |
| LTV | Cohort evidence of repeat purchases/renewals and lifetime contribution. A setup fee or day-7 return cannot establish LTV. No perpetual-retention assumption. |
| ROI / payback | Net attributable return subtracts acquisition and attributable development cost from contribution. ROI needs a defined cost base and observation window; payback requires cumulative contribution to cover CAC. Missing cost/token-price data is unknown, not free work. Clicks and successful test fixtures are not profit. |
| Deliberate return | Voluntary later inspection of another job, with day-7 follow-up and denominator stated. Auto-start, tray uptime and close-to-hide persistence are not voluntary reuse; non-return alone does not establish churn. |

Demand, saved time, prices, paid conversion, CAC, LTV and ROI are **unmeasured** in the evidence inspected here. Source code, SRS targets, historical gate passes and Actor routing statistics cannot establish them. No defensible monetary savings or ROI ranking follows.

A later business owner could keep a consented manual ledger of source category, qualification, activation, quote, collected payment/refund, costed acquisition/delivery/support hours and deliberate reuse. Aggregate reporting needs no command/chat text, argv, paths, project names, tokens or persistent job identifiers [S1 §17.5]. This is a measurement definition only: no ledger, telemetry, instrumentation, outreach, publication, spending or new business feature was created.

## Task acceptance and handoff

- Evidence-referenced ARC positioning and copy are above for task-6; the Agent Valley mismatch is recorded without changing `docs/prd.md`.
- Every proposed commercial route is labeled Hypothesis; unsupported economics remain unmeasured. Candidate product copy is distinct from accepted runtime capabilities and commercial experiments.
- This task changes no shared interfaces, manifests, tests, mission state or other Actor-owned files. Engineering retains the bridge/state contract and all native gates; independent QA retains acceptance review.
- Commands and outcomes: scoped `git diff --name-status`, `git diff --cached --name-status` and `git ls-files --others --exclude-standard` exited 0 with no initial changes; `git rev-parse` for the accepted/snapshot trees exited 0 with the equal hashes above. Python byte comparisons and SHA-256 collection exited 0. `git diff --check -- . ':(exclude)av.yaml'` exited 0. A no-index whitespace check of this new file against `/dev/null` exited 1 with no diagnostics (new-file difference); the Python report check independently passed content, links, whitespace, evidence-file count and sole-owned-path assertions with exit 0. The unresolved-ref exit 128 and denied-path diagnostic are recorded above. No test file or evidence checker was added, and no build, native gate, browser or GUI check ran for this prose-only assignment.
- Remaining evidence limits: denied `av.yaml` inspection, locally unresolved supplied ref names, no current implementation receipts, no participant study, no validated distribution access and no commercial observations. These limits do not prevent the report handoff; they prevent broader product, revenue or completion claims.
