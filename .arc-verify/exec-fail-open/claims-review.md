# Execution scope and claims review

Status: task-5 review completed; one wording finding for the Chief Director and
backend owner. This is a documentation assessment, not final implementation QA.
Skill used: `oma-docs`. Only this report was changed.

## Source scope

Accepted committed baseline: `00773c69a8e5e7fd417fd5cc78d03e222364f609`;
its verified product tree is `b98f996caa04fe13eeeeab1072c3c528f6d18c65`.
The inspected Actor-local HEAD and supplied input snapshot are
`9dc844262c2a327f8eb444c25705d1645ce03811`, tree
`e49f22a62e788d07ca92e255a0ce38e7cb5725ac`. These are different source scopes.
The delivered repair is present in this snapshot; it is not claimed present in
the accepted baseline. The supplied snapshot ref is
`refs/agent-valley/task-evidence/a546003e0df37906-e7c08de777760acd-1/snapshot`;
the supplied worker ref is
`refs/agent-valley/task-evidence/a546003e0df37906-e7c08de777760acd-1/worker`.

Initial accessible Git status, excluding `av.yaml`, was clean. The existing
12-line claims summary was inspected before replacement. Git's baseline-to-HEAD
Markdown inventory identified `docs/managed-exec.md` as the changed product
document. I reviewed its complete text and diff, the delivered exec help and
diagnostics, [implementation notes](implementation-notes.md), the
[design contract](design-contract.md), and the supplied Director advice.
Advice and contracts describe proposals or earlier observations; delivered source
and scoped execution records determine the assessment below.

These input bytes matched the inspected snapshot before this report changed:

| Delivered input | SHA-256 |
| --- | --- |
| [Exec help](../../crates/arc-cli/src/main.rs), lines 42–60 | `2cfe0e422bf0fd44c3ec0c193dd43ef87a97397300039068efb2b89e2f1e6b70` |
| [Execution documentation](../../docs/managed-exec.md), lines 1–29 | `01b1b04dea836712372cacb83daa5831b54cec2ff1dda4987115bddc7cee52cd` |
| [Execution owner](../../crates/arc-cli/src/exec.rs), lines 66–203 | `355e1b6c4fbd823a72d63cd1029da47343c372415dccac1132c1e7468be0f955` |
| [Preparation and diagnostics](../../crates/arc-cli/src/exec/preparation.rs), lines 12–94 | `08febe91f0b976559c669f7e3866976e4073a13d06834610f1daf0de107bb4fd` |
| [Transport decisions](../../crates/arc-cli/src/exec/transport.rs), lines 38–119 | `34a636d5a67c58a4bde633a15600095bcfac2d64dafe4f66d1b9714938514451` |
| Implementation notes | `c3efed2f47273523bde628fb1aa75a3982a6785916d0ba0d98a1f8bcb049ff1c` |
| [Retained native timing summary](implementation/repair8-native-timings.json) | `4ec1b69ad2cb807a1fea0bef4fbce6da052bff57701dd787d6f43e65f2aa5538` |

## Supported claims and their limits

| Claim | Delivered evidence and assessment |
| --- | --- |
| Tracking outages do not veto one native spawn attempt. | `docs/managed-exec.md:11,17` and `main.rs:51–54` state the benefit. `preparation.rs:31–54` maps unavailable or uncertain metadata to tracking states; `exec.rs:100–143` reaches one spawn expression and prints fallback copy only after success. This supports an execution attempt, not guaranteed program startup or success. |
| Exclusions remain non-launching. | `main.rs:53`, `docs/managed-exec.md:11,29`, `exec.rs:67–98`, and `preparation.rs:58–72,85–94` distinguish argument errors, cancellation, explicit rejection, invalid replies and local preparation failure. Native spawn failure has separate copy at `exec.rs:112–139`. The help summary needs the clarification recorded below. |
| Execution and persistence are separate. | `preparation.rs:74–82` says tracking is unavailable or recording is unconfirmed. The unavailable message allows an absent record; the uncertain reference is a lookup key. `main.rs:58` and `docs/managed-exec.md:17,21` expressly deny a persistence inference. Managed attachment/completion can also become uncertain at `exec.rs:146–197`. No guaranteed saved history or completion persistence is advertised. |
| Recovery does not execute work again. | `main.rs:56–59` and `docs/managed-exec.md:19–23` describe observational status and last-known records, not recovered execution. An absent record cannot establish whether the command ran. The repair does not promise to reconstruct a missing lifecycle history. |
| Native behavior remains the execution contract. | `main.rs:49–55` and `docs/managed-exec.md:13–15` preserve literal argv, cwd, environment, descriptors, streams, exit status, signals and terminal behavior. These are implementation claims supported by focused test records, awaiting final workspace and independent QA verification; they do not establish universal compatibility. |
| The metadata budget is shared and bounded. | `exec.rs:73,89–95` creates a 1,800 ms absolute deadline; `preparation.rs:21–48` passes it to both stages. `docs/managed-exec.md:27` separates that budget, a 200 ms engineering allowance, and the larger measured handoff interval. Neither help nor documentation promises a two-second command duration or a hard real-time host guarantee. |
| Fallback adds no resource management or platform promise. | `docs/managed-exec.md:17,25,29` excludes reservations and ARC cleanup and says fallback makes no post-spawn metadata requests. `main.rs:60` and `docs/managed-exec.md:25` leave desktop integration, native Windows acceptance, vendor hooks and scheduling outstanding. No delivery, cleanup, scheduler or desktop capability is marketed by these changes. |

Diagnostics inspected in `exec.rs` and `preparation.rs` use static guidance,
validated generated references and numeric outcomes rather than argv, cwd,
environment values or peer error text. Child-written streams remain separate
from this wrapper-copy assessment. Secret-leak and native-compatibility claims
still depend on the required execution checks.

## Results available at review time

The six retained `implementation/commands/repair8-*.json` receipts record exit
zero and equal before/after source fingerprint
`b4dd903e5cd70b327392b67a9a364f4107f8e3c435d81c2ec35c606f6d536567`.
Before writing this report, I independently compared every listed input digest
in all six receipts to the local files: no mismatch. I also verified that the
timing summary's referenced managed-test receipt has SHA-256
`a3c89bd687dfd8dbac10a2fa70122a59f71cbd02546bca44580c20638eb4bcea`.

Implementation notes, lines 86–98, report 35 CLI tests, 35 native managed-exec
tests, four docs/preservation checks and six filtered validator regressions
passing, plus formatting and scoped warning-denying Clippy. Those are retained
backend results, not tests rerun by Marketing. The filtered validator run leaves
two production cases unselected and is not full no-skip acceptance.

The native summary contains 13 controlled macOS arm64 observations, each recording
one invocation, exit 17 and one total invocation after daemon recovery. Its
maximum is 1,845.304 ms for `shared-budget`, below the 2,000 ms engineering
target. Its boundary is the parent's instant before wrapper spawn through the
executed child's handshake, including startup and observer overhead. Internal
metadata and handoff subdivisions were not separately observed. These are
retained measurements, not a new timing run, customer experience or latency SLA.

All 13 referenced raw observation files and the ignored command logs are absent
from this isolated input. Their contents/output digests were not independently
reproduced here. The supplied task-3 review reports that the Technical Director
verified them in its own source scope. That review does not substitute for the
mission's independent oma-qa reproduction.

At this review's input snapshot, `implementation/gates.json`, `qa/review.md` and
`final.json` are absent. Full workspace gates, final coverage verification with
each crate's 80% production-line floor, the full harness and independent QA are
therefore not claimed passed. This report replaces an input hashed by the old
receipts; those receipts retain their original scope. Final collection must bind
the integrated report and any coordinated source corrections under the existing
contract. No historical receipt was rewritten.

## Finding for the Chief Director and backend owner

**C1 — low severity: the short fallback promise omits protocol-failure exclusions.**
At `crates/arc-cli/src/main.rs:51`, the help says tracking that cannot be confirmed
still leads to one attempt. Line 53 lists cancellation, arguments and explicit
authenticated rejection as exclusions, but not malformed or mismatched replies
or invalid local preparation. `docs/managed-exec.md:11` has the same broad summary.
A complete wrong-request-ID reply cannot confirm tracking, yet
`transport.rs:82–83` returns InvalidReply; `preparation.rs:34,67–69` stops with
IPC_ERROR and zero launch. The detailed document correctly explains that at
line 29. This is an ambiguity in the short description, not a request to change
the implementation's error policy.

Proposed correction: qualify the help/intro promise as unavailable tracking or
tracking left unconfirmed **by an outage**, and name invalid protocol replies and
local preparation failures alongside the non-launching cases. Keep native spawn
failure separate. Route the copy change through the Chief and backend owner:
`scripts/harness/exec-fail-open-docs.ts:10–16` requires exact agreement with the
accepted design copy. This Actor has changed neither shared copy, contracts nor
the checker. The finding does not waive or replace an acceptance check.

No other misleading persistence, recovery, cleanup or platform claim was found
in the delivered help, runtime copy and product document reviewed here.

## Unmeasured outcomes and deferred commercial work

Reduced interruptions, duplicate-launch reductions in actual workflows, support
savings, comprehension, adoption, retention, willingness to pay, revenue, CAC,
LTV and ROI remain unmeasured. Controlled execution fixtures establish neither
buyers nor money saved. No demand, price, paid conversion, customer acquisition
cost or commercial return is established by task-3 or this review.

The supplied Marketing advice's paid assessments, acquisition research and
partner/referral routes remain future hypotheses. They need a qualified buyer,
priced delivery/support effort and collected-payment evidence before a profit
claim. Outreach, publication and spending are outside this repair's authorized
effect scope. Loss-framed retention copy has no measured retention benefit and
could obscure the immediate execution/tracking distinction; it adds nothing
needed for technical acceptance. These proposals are deferred, add no gate to
this repair and were not executed. No market engine, promotional artifact,
contact, publication, purchase or commercial experiment was performed.

## Checks and ownership

- Git status, baseline/snapshot tree inspection, diff inventory and known-path
  reads completed. Serena tools were unavailable in the tool inventory; no
  substitute native project-code discovery search was used.
- `oma docs verify docs/managed-exec.md --json --no-urls` ran on an isolated
  temporary copy: exit 0, one document, zero references, zero broken references.
  Its generated index stayed outside the worktree. URL checking was disabled.
  Reference cleanliness does not establish semantic correctness.
- An existing `Bun.markdown.html` text check preserved the literal `<JOB_ID>`
  placeholder visibly as escaped text. The suspected missing-placeholder issue
  was rejected; no rendered-page or screen result is claimed.
- `bun test scripts/harness/exec-fail-open-docs.test.ts`: two passed, zero failed.
  This verifies copy/policy checks, not the execution matrix or final mission.
- `git diff --check -- .arc-verify/exec-fail-open/claims-review.md`: passed.
  Final accessible status was confined to this owned report. No product file,
  manifest, lockfile, registry, test, mission state or other Actor's work changed.

Acceptance assessment: delivered files and limits are cited; the concrete wording
defect has a source reference, counterexample and proposed correction; commercial
proposals remain separate hypotheses. Public interfaces are unchanged. The Chief
and backend owner receive C1 through this handoff. Final technical verification,
independent QA and any coordinated copy correction remain their assigned work.
