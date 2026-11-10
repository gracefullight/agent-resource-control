# Traffic: product and commercial scope

Status: product analysis for task-3, 2026-11-10. The paid assessment and field
test below are proposals; no outreach, sale or field test was performed.

## Source and evidence scope

The accepted committed baseline is `4d2f71c1a1482925f6c9918296d7ceb3435b7592`,
with tree `4455896fdf976244313a70a9fa38b730b344b411`. The Actor's inspected
HEAD and input snapshot are `5a3c7d0a66550768c454bc807b2dfa6def1da42e`, with
the same tree. This report is a new Actor-local worktree deliverable; it is not
in either of those trees. The relative repository links below refer to the
checkout in which the report is read; use `git show <baseline>:<path>` for the
specified baseline contents if that checkout has changed.

The ARC requirements source identified by
[bounded-ipc-targets.md](../../docs/bounded-ipc-targets.md) is the external
`Agent_Resource_Control_ARC_Requirements_SRS_2026-11-06.md`; its SHA-256 was
rechecked as `a8ef945251f9f3fd7c3e337d4efc77ff9914f95114212ca9e6c90ae2ce3e3d9e`.
Its multi-agent and worktree scenario describes a potential need, not observed
demand. This task read source and public references but ran no product tests.
Previous lifecycle receipts are historical evidence for their own scope and
are not results for Traffic at this snapshot.

## Intended user and product boundary

The first buyer hypothesis is a macOS Rust team whose developers or coding
agents run builds concurrently in separate local worktrees. A workload owner
may value controlled contention and knowing when work is queued or finished.
No such buyer, interruption cost, willingness to pay or user reaction has been
established here.

At the accepted baseline, [managed-exec.md](../../docs/managed-exec.md)
documents `arcctl exec -- <program> [args...]`, CLI-owned direct execution,
lifecycle tracking and outage fallback. The
[execution source](../../crates/arc-cli/src/exec.rs) classifies the argument
vector and spawns the program directly; the
[detector rules](../../crates/arc-detector/src/rules.rs) recognize direct
`cargo build`, `cargo test`, `cargo clippy` and `cargo fmt` forms when the
executable and subcommand are separate arguments. These are source and
documentation observations, not a fresh runtime pass. The same documentation
explicitly says this baseline has no Phase 1 reservations or scheduling.

The requested slice targets a healthy daemon coordinating **explicitly
wrapped**, locally classified Cargo build, test and lint jobs through a FIFO
queue, class windows and conservative CPU/memory reservations. Remote and
UNKNOWN jobs are intended to bypass reservations. An example of the intended
entry point is `arcctl exec -- cargo build`; unsupported or ambiguous forms
may classify as UNKNOWN. The mission also requires queue visibility, recorded
wait and outcomes, safe retry/restart behavior, nested-lineage checks and
preservation of native command semantics. These are acceptance targets until
verified against the integrated source; they are not results observed by this
report. ARC should not be described as intercepting unwrapped commands,
enforcing a whole-machine resource cap, reducing disk use or automatically
integrating with vendors on this evidence.

The mission's required macOS CLI experiment uses real Cargo builds in two
independent directories. Its targets are a peak of two builds with ARC bypassed
and one with a build window of one, one invocation per job in each condition,
and a second scheduled job that reports queued before the first releases and
starts only afterward. Queue wait and completion outcomes must be recorded.
These numbers are required test outcomes, not measurements made in this task.

Desktop integration, native Windows acceptance, vendor hooks, broader detection
and remaining scheduling policy are outstanding. Adaptive pressure control,
fairness and start-rate policy are outside this slice. Even a passing two-build
concurrency test would establish that test's admission behavior, not customer
savings or general responsiveness.

## Alternatives and acquisition hypothesis

| Alternative | What the source says | Comparison needed |
| --- | --- | --- |
| Existing workflow and Cargo job limits | Cargo's [`build.jobs` / `--jobs`](https://doc.rust-lang.org/cargo/reference/config.html#buildjobs) controls compiler-process parallelism for a Cargo invocation. | Compare the team's current settings and a disclosed job limit on its workload; do not assume ARC wins on time or memory. |
| Shared target directory | A [worktree-builds guide](https://github.com/laurigates/claude-plugins/blob/main/rust-plugin/skills/cargo-worktree-builds/SKILL.md) describes reuse and Cargo lock serialization, along with branch-related artifact risks. | Record target layout and cache state; the proposed test uses independent targets so a shared lock does not masquerade as ARC admission. |
| mr-boxington | Its [author describes](https://jdx.dev/posts/2026-09-05-introducing-mr-boxington/) shared compilation caching and memory-aware scheduling. | Treat those as the author's claims, not an independent comparison; include the tool if it is part of the team's existing setup. |

[Cargo issue 16804](https://github.com/rust-lang/cargo/issues/16804) reports
worktree isolation, cache and disk-exhaustion problems and was closed as a
duplicate. It is an author's problem report, not evidence of an ARC customer or
that this Traffic slice solves disk accumulation. Maintainers of worktree and
agent tools or Rust consultants could be referral channels, but no relationship
or referral has been established.

## Proposed paid-assessment test

Hypothesis: a team experiencing local parallel Rust-build contention would pay
for a fixed-scope assessment that compares its existing workflow, Cargo job
limits and the verified ARC slice, then reports outcomes and a configuration
recommendation. This is an assessment-service hypothesis, not proof of
software demand or recurring subscription value. Price, prospect, conversion,
delivery effort and time to payment are unknown. A quote would require a
separate estimate of delivery and support work.

After implementation and independent verification, a possible small field
test is one consenting team on its own macOS machine, using two independent
worktrees and target directories. Compare matched runs of its current
workflow, a stated Cargo job limit and ARC with the tested build window; rotate
order where feasible. Record machine and toolchain, inputs, flags, target and
cache state, configuration, run order and sample count. Record successful
completions and invocation counts, both per-job elapsed time and pair
makespan, queue wait, sampled peak memory or swap using a stated method and
cadence, and any directly observed interruptions. Keep failed or missing
measurements visible. A one-hour operator budget on existing hardware and no
ad spend is a proposed constraint, not a result; insufficient repetitions or
uncontrolled cache changes make the comparison inconclusive.

For this proposed experiment only, repeat with more teams if outcomes remain
correct, measured peak memory falls against the best viable tested
alternative, pair makespan worsens by no more than 10%, and a collected
assessment fee exceeds recorded acquisition, delivery and support costs.
Otherwise revise the offer or audience; a single positive case would still not
establish a general effect. Separately record whether a real quote is accepted,
an invoice is issued and payment is collected. No contact, spending or offer
is authorized by this documentation task, and these commercial criteria do not
alter the mission's technical acceptance gates.

## Economics and claims

Demand, actual savings, pricing, revenue, customer acquisition cost (CAC),
lifetime value (LTV) and return on investment (ROI) remain unmeasured. If an
assessment is later sold, record inquiry source, qualified inquiries, quotes,
accepted quotes, invoiced and collected amounts, refunds and payment fees,
acquisition hours, delivery hours and ongoing support costs. Use documented
labor cost assumptions. Calculate CAC as acquisition costs divided by paying
customers over a stated period; calculate contribution from collected
revenue less refunds, fees, delivery and support costs, then compare it with
acquisition cost. Payback is the point cumulative contribution covers CAC.
Do not infer recurring LTV from a one-off assessment or calculate ratios from
an empty denominator.

The commercial claim to test is that an assessment can produce useful,
measured guidance and a positive contribution for a particular workload.
Until source-bound integrated tests and field measurements exist, neither a
technical benefit nor a financial return should be represented as achieved.
