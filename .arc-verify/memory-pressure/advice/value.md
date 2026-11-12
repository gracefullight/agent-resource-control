# Task 7 — product value and commercial claim bounds

Status: completed advisory assignment. Marketing Director; 2026-11-12.
This completes the value report, not mission acceptance or a product launch.

The demonstrated benefit is controlled admission with an explained automatic
wait/recovery path for supported local workloads. Task 6 supplies production-path
integration evidence that fresh injected Critical pressure holds new roots,
preserves the running fixture's intended outcome and executes the held fixture
once after recovery. It establishes neither faster workloads nor financial value.

## Ownership and inspected provenance

Only `.arc-verify/memory-pressure/advice/value.md` is changed by task 7. The
2026-11-12 operator notes supersede the saved `target/.../advice/value.md` path:
this advisory report must be Git-visible, below 40,000 bytes, and is one of the
three advisory exceptions to the five canonical files/300,000-byte budget. The
combined evidence ceiling is 420,000 bytes. No saved assignment, acceptance
criterion, mission state, source, test, manifest or preceding report was changed.

- Accepted committed baseline: `338631893b51b2e7cd4045e636bb1301ab4db955`;
  verified tree `cb589a2dbb538f7d273e291858d939b00ffa62f7`.
- Inspected Actor input HEAD/snapshot commit:
  `01cbaf57939268e9ce68c5b7180a7eb6b84b3226`; verified input tree
  `18e39b08a1daf79a4cdf87c047537d56f5a41509`.
  Task-6 product/report bytes are present in this isolated input snapshot; they
  are not claimed to exist in the accepted committed baseline. This task-7
  report is a new local change, absent from that input snapshot.
- The supplied `refs/agent-valley/task-evidence/6c542d07702898e2-014e4ceaa5a9e511-1/snapshot`
  and `/worker` refs did not resolve at inspection. The supplied snapshot commit
  itself resolves. No missing worker revision is invented or replaced with HEAD.
- Reviewed handoff: [integration.json](../integration.json), 150,537 bytes,
  SHA-256 `6e37828971d65320702efa0d721de01cfadf83175399af9fa1d78ed7fbaa8394`.
  Its task-6 completion scope explicitly leaves final gates, coverage and QA to
  later tasks. The supplied dependency review also accepts task 6 within that scope.

The 29 hashes in `integration.json#/inputBinding/sourceFiles` all match this
Actor's inspected files. Task-6 raw files are absent from this isolated checkout
but available for read-only inspection at the supplied source workspace:
`/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/847d99d4-37bc-4c0d-9593-d56183093082/workspace`.
All 15 successful check receipts and their 15 logs match the recorded byte
lengths and SHA-256 values; matching receipt command/exit fields were inspected.
The four final Rust test receipts each bind the same 29 source hashes. The three
referenced raw captures for injection, SystemSource smoke and reserved launch
also match their hashes and lengths. Their selected values reconcile with the
compact report. These reads verify existing evidence, not fresh test execution.
Raw files remain ignored diagnostics, not an assumed separate handoff channel.
The Git-visible integration report carries the required evidence for later tasks.

No second aggregate mission input digest is created. Task 8 owns the single
final digest in `gates.json#/inputs`; the hashes above identify source/artifact
provenance only.

## Benefits supported by task-6 results

Evidence pointers below are relative to `integration.json`. Source references
are to the inspected snapshot, not the accepted baseline.

| Bounded current claim | Actual evidence and practical meaning | Limit |
| --- | --- | --- |
| Fresh Critical pressure controls admission even with capacity available. | `/injected`: explicit Normal → Critical → Normal through the production startup/Meter/coordinator path. Zero new root permits and zero new root child starts across 21 held checks, including 20 after the running child exits. Raw Critical sequences 2–4 remain fresh; maximum captured age is 994 ms against 3,000 ms. `native_sequence.rs:103` starts Test and holds Lint, whose separate windows and combined reservations otherwise fit L0. | Injected pressure establishes control logic. It is not an OS Critical event, an OOM experiment or a throughput benchmark. |
| A reservation alone cannot bypass a later Critical hold. | `/injected/reservedRace`: real `arcctl`, 20 held checks, zero target executions/new permits, unchanged existing reservation, then one target execution after Normal. Assertions are in `apps/desktop/src-tauri/tests/memory_pressure/native_launch_races.rs:57`. | One controlled reservation/launch race; not a population rate or a guarantee for unmanaged commands. |
| Existing intended execution survives this admission change, and the queued fixture resumes without resubmission. | `/injected/runningChildOutcomeUnchanged` and `/injected/recovery`: both observed exits remain `EXITED`, code 17. The held job executes exactly once. Independent PID markers, SQLite child identities/permits/decisions, CLI status and registered production DesktopBridge agree. `/grantBoundary/expiredReadyRecovery` separately records one execution after confirmed unspawned expiry recovery. | Exit 17 is the fixture's deliberately nonzero outcome, not a successful build. `native_support.rs:70` links the `cargo` command to `arc-exec-fixture`; these are real owned processes with fixture workloads. No real project's result quality or build performance was measured. |
| A waiting developer can inspect a reason and the intended automatic recovery path. | `/injected/held/cliStatus` displays “Waiting for memory” and automatic-start copy qualified by other queue rules; the registered bridge reports `memory_pressure` with the same decision metadata. `/surfaces` and `display-bun-final` record deterministic Dashboard hold/recovery, stale/disconnected/unsupported and focus/history cases. | Available explanation and automatic behavior are demonstrated. Human understanding, discoverability, accessibility in a real webview and reduced duplicate-launch intent are unmeasured. The bridge uses registered production commands with MockRuntime shell ports, not a screen test. |
| Unavailable observations do not invent Normal or indefinitely reuse an old Critical hold. | `/grantBoundary`, `/edgeCases/cases/missingStaleFailedUnsupportedDeniedFallback` and `/surfaces/freshness`: deterministic tests cover grant-time identity/age, backlog, exact boundaries, restart and clock changes; explicit `macos-memory-l0-v1` fallback preserves unavailable/stale/failed/unsupported reasons. | Fallback uses conservative static scheduling limits. It does not guarantee low pressure or OOM prevention. A disconnected display cannot infer that the daemon selected a new fallback. |
| Decision persistence is bounded in the tested cases. | `/audit`: unchanged polling leaves four rows before/after; 20 repeated checks append no transition. Tests cover per-job/global transition caps, bounded expiry and terminal-anchored 90-day retention. | Controlled SQLite/clock tests, not a 90-day field study, disk-saving measurement or proof of zero overhead. |
| Real macOS pressure observations reach policy and read surfaces. | `/nativeSmoke`: SystemSource, separate from injection; two actual available Normal readings, sequences 1 and 2, reconcile with CLI and registered bridge. | Normal only. No native Critical observation was forced or recorded; no savings follow from this smoke. |

The supported audience is local multi-agent/worktree developers already routing
direct Cargo build/test/clippy/fmt and supported one-shot Bun tests through ARC.
`contract.md` defines Build/Test/Lint with positive memory reservations;
`/edgeCases/cases/allSupportedClassesWithFreeSlots` records deterministic coverage
for these classes. The production sequence itself exercises Cargo-classified
fixture commands, not every Cargo/Bun workload. Remote, UNKNOWN and unsupported
workloads retain bypass behavior; verified nested reuse retains its existing
reservation. Existing FIFO, windows and budgets still govern eligibility, and
the separate bounded outage fail-open is preserved. This is not host-wide
interception or an unconditional promise to block every new process.

## Wording audit and Chief handoff

Scoped wording inspected: the integration report, contract, existing
technical/design advisories, `apps/desktop/src/dashboard/policy-messages.ts`,
`job-policy-panel.tsx` and `crates/arc-cli/src/job/policy.rs`. Git metadata shows
no README or `docs/` change between the accepted baseline and this input snapshot.
This is a claim audit, not independent source QA or exhaustive repository search.

The inspected new product copy states a memory wait and qualifies automatic
start with other queue rules. Current/retained policy, historical job decisions
and recorded outcomes have separate labels. `job-policy-panel.tsx:83` guards
confirmed wait copy against stale/error states; CLI `policy.rs:25` retains the
“Last received policy” qualification on expired reads. The task-6 receipts include
the corresponding expiry/recovery cases. No unsupported savings, customer or
revenue promise was found in this inspected wording.

The following unsupported extensions are handed to the Chief as claim limits;
they are not assertions that a contributor published them:

| Avoid in synthesis or later promotion | Evidence permits |
| --- | --- |
| “Prevents OOM”, “stops Mac freezes” or “handles native Critical successfully.” | Tested admission under injected Critical; actual native smoke observed Normal only. |
| “Faster builds”, “improved throughput”, “saves CPU/tokens/money”, or “negligible overhead.” | No matched workload benchmark or overhead/cost comparison in these receipts. |
| “Builds succeed unchanged” or “all commands execute exactly once.” | Specific fixture exits and execution counts preserved, including intentional exit 17; supported scope and bypass/outage rules remain explicit. |
| “Users understand the wait”, “retention improves”, or “avoids lost conversions.” | Deterministic explanatory UI/CLI behavior, without a human study, cohort or funnel. |
| “POL-001/003/004 fully accepted”, “release-ready”, or “all gates pass.” | Task-6 integration acceptance for the first macOS slice; task-8 final gates/coverage, independent QA and Chief synthesis remain pending here. |

The supplied upfront Marketing Director advice prioritized credible admission
proof before promotion. Task 6 now supplies the bounded logic proof above. Its
explicit native/performance limits are retained; historical missions and earlier
Normal readings are not recertified as evidence for this slice. Earlier advisory
findings are dated inputs: task 6 records repairs/replacement results under
`/directorFindings` and `/repairRound`; their former blockers are not presented
as still-open product defects without current evidence.

## Commercial unknowns and optional future hypotheses

Demand, willing buyers, price, collected revenue, acquisition costs, costed
delivery/support labor, refunds, referral shares, CAC, LTV, payback, retention
and ROI remain unknown. No customer, payment, cost or cohort dataset accompanies
this assignment or task-6 acceptance evidence. Quotes/invoices would not alone
prove collected cash. Passing checks, stars, uptime or completed fixture jobs
cannot supply those missing denominators. Mission Actor/tool/subscription and
engineering costs are also unpriced here; a zero known estimate in supplied
operation metadata does not establish zero total cost or positive mission ROI.

Two earlier ideas remain optional future hypotheses, outside this mission's
tasks and acceptance:

- A fixed-scope paid macOS contention assessment might serve a consenting team
  with repeated local Cargo/Bun contention. Buyer demand, access to representative
  workloads, an agreed price/payment, costed acquisition/delivery/support time
  and a matched comparison against the team's existing configuration are unmet.
  Positive contribution after those costs would need observation, not inference
  from this admission test. No assessment was sold or delivered.
- A voluntary referral from an interested tool maintainer or Rust consultant
  might identify the same audience. No partner, reach, placement, referral
  agreement, share or attributable payer is established. Comparing collected
  contribution per costed acquisition hour could test it only with later scope
  and authorization. No relationship or referral experiment exists here.

Neither hypothesis authorizes outreach, promotion, collection of customer data,
publication, ad buying or spending. No competitor research, growth experiment,
contact, distribution, customer reaction or financial result was performed or
invented by task 7. There is no additional approval or acceptance gate.

## Checks, completion and remaining risks

Skill used: `oma-pm`, for matching the assigned deliverable to acceptance and
separating evidence, assumptions and later scope. The task-specific report
instruction takes precedence over its generic plan-output template; no new plan,
task graph or implementation was created. Serena tools were unavailable after
tool discovery. Fallback was scoped reads of already located paths, Git metadata
and evidence-file parsing/hashing; no native recursive project-code search.

| Commands/checks executed by task 7 | Outcome |
| --- | --- |
| `git status --short`, then scoped status/diff excluding `av.yaml` | Initial command reported `av.yaml: Operation not permitted`; it was not bypassed. Scoped input status/diff were empty. No claim of unrestricted whole-worktree cleanliness. |
| `git rev-parse` for HEAD, baseline/input trees and supplied refs | Commit/tree values above verified. Combined command exited 128 on absent worker ref; separate snapshot-ref verification also exited 128. Exact commits remain available. |
| Scoped `git ls-files --stage` and baseline-to-HEAD path metadata | Contract, integration and prior advisories tracked in this snapshot; preceding work preserved. No README/`docs/` changes in the checked comparison. |
| Python standard-library reads, JSON assertions and SHA-256/byte checks | Integration digest, 29 source bindings, 15 receipt/log pairs and three captures verified. Four final Rust log totals are 34 + 53 + 64 + 16 = 167. Recorded display/tooling totals are 109 + 86 = 195 Bun passes, all scoped; these are task-6 executions, not this Actor's tests. |
| Whole native-capture equality probe, then field projection check | Initial assertion failed because compact `cliMeter` projects native `host.memory_pressure` and omits unrelated fields. Explicit policy/bridge and projected-meter equality checks passed for both samples. This was a review-script assumption, not a failed product test. |
| Report transport/size and whitespace checks | Final handoff records the report's hash/size, Git visibility and `git diff --check` outcome. |

Task 7 ran no builds, software tests, native gates, coverage collection, browser
or GUI checks. The existing backend commands and their full argv remain in
`integration.json#/checks`; earlier failed attempts and resolutions remain in
`/failedAttempts`. Scoped results do not replace the six mandated final gates,
the 80% production-line floor in all eleven crates, affected desktop checks,
configured-target reconciliation or independent read-only QA. Those remain
assigned later work, with the existing verification contract unchanged.

- [x] Each current benefit is tied to task-6 results and bounded by what was tested.
- [x] Unmeasured commercial assumptions and optional future ideas are separated
  from engineering results and add no mission requirement.
- [x] No outreach, publication, spending, customer reaction, savings or financial
  outcome is represented as performed.

Remaining risks: native Critical, real-workload comparative value, production
overhead and human comprehension remain unmeasured. Overall mission acceptance
is pending the assigned final verification/review. Outstanding product scope:
CPU/disk pressure control, statistical learning, vendor hooks, native Windows
and full policy acceptance.
