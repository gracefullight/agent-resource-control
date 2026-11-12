# Host Meter: inline Dashboard experience and state contract

Status: task-3 design handoff. Owner: Design Director. This task writes only
`.arc-verify/host-meter/experience.md`. Backend-desktop implements the bridge,
controller, messages, Dashboard and tests using `oma-backend` and `oma-frontend`.
Backend-gates owns native integration and gate execution; independent `oma-qa`
reviews the integrated result. This document establishes no implementation pass.

## Evidence scope and provenance

The accepted committed baseline is `efedf0f4eb9b97eed641c82f3c4766bb16f2e3de`,
tree `64dd3d10ba1458a42a4dcae1fa42e6bcbee43b7e`. The inspected Actor-local HEAD
and supplied input snapshot commit are `b259d3f1c459afab178fa4f1a0296dfa785c7d9c`,
tree `962b8cd3085d2dce5b00292efbf001fed23cbc12`, on branch
`av-task/e0ce9b3a9c09523f-1724980aa084bbaa-1`. These identities were read from
Git; the snapshot is not the accepted baseline. Scoped `git ls-tree` confirms
the copied SRS, Meter contract and `crates/arc-core/src/meter.rs` are tracked in
this snapshot and have no entries at the accepted baseline. Their presence here
does not establish their presence in a review checkout. This report is a new
uncommitted deliverable until integrated; verify its delivered bytes there.

The supplied snapshot and worker reference names use prefix
`refs/agent-valley/task-evidence/e0ce9b3a9c09523f-1724980aa084bbaa-1/` and suffixes
`snapshot` and `worker`. Neither reference resolved locally during inspection.
The snapshot commit itself resolves and has the supplied tree. No collection
reference was created or repaired, and no integration revision is invented.

Initial scoped status and diffs showed no changes in the inspected assigned
inputs; `experience.md` did not exist. Root status emitted
`av.yaml: Operation not permitted`, so full cleanliness and container-target
configuration are unverified. `target/host-meter/task-1/handoff.md` is absent
in this isolated snapshot. Task-1's reported compilation/53-test success and
its independent review are supplied dependency evidence, not checks rerun here.

The SRS source at
`/Users/gracefullight/Downloads/Agent_Resource_Control_ARC_Requirements_SRS_2026-11-06.md`
and copied destination `docs/requirements/ARC_SRS_2026-11-06.md` were rehashed:
both match `a8ef945251f9f3fd7c3e337d4efc77ff9914f95114212ca9e6c90ae2ce3e3d9e`.
Inspected sections: 1.3, 10.1–10.9, 14.5 and 15.1–15.8. The supplied Chief's
goal brief chooses inline readings and defers other Meter surfaces and prompts.
The frozen task-1 contract supplies all wire semantics below.

| Inspected snapshot input | Source observation | SHA-256 |
| --- | --- | --- |
| `.arc-verify/host-meter/contract.md` | Frozen schema, availability, cadence, ownership and bounded-read contract. | `6f135aa7900e6d6061e28e83e3be1bf4008f1ad9067a3bf8c3f3b8fa26c031af` |
| `crates/arc-core/src/meter.rs` | Typed measurements, identities, intervals, provenance and accounting windows. | `568efc0d7ce38612c7571039d5cbc6b31b3889a07bfa7c584414c41e35c57385` |
| `crates/arc-core/tests/fixtures/meter-snapshot.json` | Synthetic first-interval/partial-availability fixture; not native observations. | `857fd6d59e71bdd2decfd7e4c5345190ecba0817127cb7c881d63e1ad1ef88e1` |
| `apps/desktop/src/dashboard/desktop-dashboard.tsx` | Existing health, metrics placeholders, recent list and selected details. | `18a5dd35387945242445a186af7b384de9d23a7935855a537eac801c1f081f2c` |
| `apps/desktop/src/dashboard/desktop-refresh-controller.ts` | One serialized owner, 5-second visible refresh, 10-second existing freshness, bounded reconnect. | `fa46e3f08ab125b229276cdb5a668b28e63a79edca628bdd26fcb774636e260d` |
| `apps/desktop/src/bridge/types.ts` | Health has no Meter data or measurement timestamp; `Received<T>` has a desktop wall-clock receipt. | `3f0ca9c0730e76e3a7d6b21f3522be57d082c10080c5cdcabc885a310e0bbae5` |
| `apps/desktop/src/components/recorded-time.tsx` | Keyboard-operable exact UTC/millisecond disclosures and explicit missing time. | `9c3de3b2814e732b1d5426008fe3d3ea0c3b82074c46bd010d6ed915fec421d2` |

Also read at this snapshot: `desktop-messages.ts`, `tests/dashboard.test.ts`,
`tests/contracts.test.ts`, `tests/lifecycle.test.ts` and
`tests/fixtures/desktop-test-harness.ts`, all under `apps/desktop/` except the
messages file under its `src/`. These show externalized English copy, retained
exit-17 outcomes, independent keyboard selection, fake clocks and scripted bridge
replies. They establish test infrastructure and assertions, not current test passes.
`DESIGN.md` and `.design-context.md` are absent; no new visual system is required.

Skill used: `oma-design`, at `.agents/skills/oma-design/SKILL.md`, with execution,
external-routing, preservation and delivery-checklist references. Selected route:
**local OMA contract for an existing decision**; no external supplier was loaded.
Preserve the established React 19/Tauri 2, TypeScript 7, Bun, Tailwind 4,
basecn/Base UI, Phosphor, semantic tokens and English fallback messages. No font,
palette, navigation or dependency changes are specified. Serena is configured
but tool discovery exposed no applicable Serena tools. Inspection used scoped
reads of already located paths; only non-code requirements were searched with
`rg`. No native project-source search substituted for the missing provider.

Ownership note for Chief: the frozen contract's evidence inventory annotates
`experience.md` as “desktop”; the saved task assigns this exact file to Design
Director. This delivery follows the saved task and hands it to backend-desktop
as an input. No contract, assignment or other Actor file is edited.

**Evidence classification:** source facts above are inspected observations.
Personas and alternative comparisons are design judgments/hypotheses. Every
example and matrix row below is a proposed synthetic deterministic input, not
a measured host reading or an executed simulation. No field test, participant
reaction, screen check, accessibility interaction, conversion, retention,
churn, calibration or overhead result was produced in this task.

## People, alternatives and the chosen journey

The SRS audience suggests three **unvalidated persona hypotheses**:

1. First use: a macOS developer running several coding agents wants to interpret
   host pressure and ARC's own cost without configuring a new measurement policy.
2. Returning inspection: a developer investigating a recorded job wants current
   host context beside that job's actual outcome, without mistaking correlation
   for resource attribution or replaying the command.
3. Headless: a developer or script wants one bounded JSON snapshot without opening
   the desktop, plus intelligible unsupported/unavailable failures.

| Alternative | Proposed journey | Design judgment, not measured usability |
| --- | --- | --- |
| A: inline Dashboard | Open existing Dashboard → read Host and ARC daemon; select a recent job if needed. | Chief's selected option. Reuses the default destination, status control and one refresh owner. |
| B: Meter tab | Open Dashboard → activate Meter → read; return to Dashboard to inspect a job. | Could support later history/attribution, but adds navigation for this current-value slice. Deferred. |
| C: tray mini-meter | Open tray surface → read a compact subset → open Dashboard for details. | Could improve glancing, but adds a second freshness surface and lifecycle work. Deferred. |

These are structural action comparisons, not measured completion times. Inline
requires no new tab, tray view, onboarding, settings, thresholds, permission
dialog or analytics. The CLI path is exactly `arcctl meter snapshot --json`
through existing setup. A valid partial snapshot is still successful JSON with
per-field availability; operation/transport failures exit nonzero and cannot
produce a fabricated all-zero snapshot. This report does not define new CLI flags.

## Presentation contract

Preserve Dashboard order: health and last successful check; existing
**Resource availability** section; recent jobs and selected details. Within
Resource availability, use compact named **Host** and **ARC daemon** groups,
then retain unrelated unavailable metrics with their reasons. Use definition
lists and existing spacing/tokens; no nested card grid, chart or new landing
surface is needed. Meter updates must not remount job rows or steal focus.

Scope copy: **“Host readings cover this Mac. ARC daemon readings cover only the
daemon process.”** Group proximity to a selected job never attributes host totals
to that job, a vendor, project, session or agent. Retain Storage reclaimed, Active
agents, Global running jobs, Global queued jobs and Usage today as Unknown where
no corresponding interface supplies them. Keep “Recent history is not a live job
total.” Replace the old “Disk usage / written” label; filesystem space cannot
supply disk writes, footprint, reclaimed storage or savings. Replace the old
blanket metric reason only for the now-supported Meter readings.

### Field labels, units and definitions

Paths are relative to the validated snapshot. Never read `value` before checking
`reading.availability`. Wire units/provenance stay exactly as task-1 defines.
Display RAM/swap/filesystem bytes in GiB (2^30 bytes), RSS in MiB (2^20 bytes),
with two decimals; display CPU percent with one decimal and cumulative CPU with
two decimals and the text `core·s`. Exact-value disclosures retain original bytes
or numeric precision, unit and source. Do not silently label binary units GB/MB.
For a positive amount below the display unit's smallest shown increment, use
`<0.01 GiB`, `<0.01 MiB`, `<0.01 core·s` or `<0.1%` as applicable, keeping the
exact value in details. Reserve the ordinary zero display for an available zero.
Level fields use the words Normal, Warning or Critical; missing levels have no
healthy color or “Normal” fallback. A known zero remains a valid numeric reading.

| Group / concise label | Typed source; unit / provenance | Required explanatory meaning |
| --- | --- | --- |
| Host / **Host CPU** | `host.cpu_utilization`; percent / derived | “Host capacity · {elapsed} s interval · {N} logical CPUs.” Busy/total host tick delta × 100, normalized across all logical CPUs, 0–100%. Display the supplied percent; do not divide by N again or relabel it as one-core process CPU. |
| Host / **Available RAM** | `host.memory_available_bytes`; bytes / native | “OS reclaimable-page estimate.” Darwin free + inactive pages × native page size; not installed RAM minus process RSS and not Activity Monitor's Memory Used complement. |
| Host / **Memory pressure (native)** | `host.memory_pressure`; level / native | Native Normal/Warning/Critical only. Never infer it from RAM or swap percentages. |
| Host / **Swap used** | `host.swap_used_bytes`; bytes / native | Used swap across this Mac, not swap capacity or selected-job swap. |
| Local filesystem / **Capacity** | `host.filesystem.capacity_bytes`; bytes / native | Capacity of the identified filesystem, not total capacity of every physical disk. |
| Local filesystem / **Free space** | `host.filesystem.free_bytes`; bytes / native | All unallocated blocks including reserved space. |
| Local filesystem / **Available to ARC user** | `host.filesystem.available_bytes`; bytes / native | Free space available to this unprivileged caller; can be less than Free space. |
| Local filesystem / **Disk pressure (derived)** | `host.filesystem.pressure`; level / derived | “Based on available space / capacity.” Critical ≤5%, Warning >5% and ≤10%, Normal >10%. Display the sampler's judgment separately from native byte observations. |
| ARC daemon / **Cumulative CPU** | `daemon.cpu_core_seconds`; core_seconds / native | “User + system CPU since daemon process started.” Process-only core-seconds, not current CPU%, elapsed wall time, child CPU or job totals. |
| ARC daemon / **Current RSS** | `daemon.rss_bytes`; bytes / native | Current resident memory for this daemon process, excluding children; not whole ARC/desktop memory. |
| ARC daemon / **Observed peak RSS** | `daemon.observed_peak_rss_bytes`; bytes / derived | Maximum successful sampled RSS since the declared peak-observation start; not an OS lifetime high-water mark or peak across restarts. |

Under Local filesystem, visibly identify `mount_point` and `filesystem_type`,
for example `/ · apfs`. In accessible details show the observed `path`, full
`mount_point`, `filesystem_type`, native `volume_id` and local scope from
`is_local: true`. These are bounded filesystem identity fields authorized by the
Meter contract, not job cwd/argv. Wrap long paths and allow keyboard access to
full text. Never invent identity `/` when it is absent, combine filesystems,
request a user-selected path, or infer local identity from a display label.
Known space values require the contract's valid local identity. APFS sharing,
reservations and snapshots can explain differences from other tools; space
changes do not establish that ARC wrote or reclaimed bytes.

CPU interval details expose its own nullable UTC start/end and exact elapsed
milliseconds; the denominator is **all host logical CPU capacity**, unlike the
one-core denominator of the separate native idle-performance acceptance run.
Daemon CPU has `process_lifetime` accounting, identified by existing
`daemon_instance` internally. Do not convert its opaque start token into a UTC
start time. Peak details label `peak_observation_started_at_ms` as **“Peak
observation started”** and its measurement's `observed_at_ms` as **“Peak last
observed”**. That latter time confirms the running maximum; it is not necessarily
when the peak occurred. A failed RSS pass retains the earlier peak and earlier
observation time. A new successful RSS pass can confirm the same maximum at a
new time. Do not reset a peak because its value did not change.

### Times and freshness

Keep these sources distinct, using the existing keyboard-operable RecordedTime
pattern and externalized labels:

| Visible label / purpose | Source |
| --- | --- |
| **Meter sampled** | `sampled_at_ms`: completed sample time, not IPC-read time. |
| **{field label} observed** | That measurement's `observed_at_ms`; each field can have a different time. Unavailable readings can expose their observation-attempt time as such, without implying a value was obtained. |
| **Meter received by desktop** | `Received<MeterSnapshot>.received_at_ms`: successful desktop receipt only. |
| **Last successful check** | Existing health receipt; never substituted for Meter observation. |
| **Daemon health observation time** | Remains Unknown because this health DTO has no such time. |
| **Sampling: {Idle/Active/Pressure} · every {interval} s** | Declared snapshot cadence, not a frontend guess from job phase or visible-window state. |

Exact-time disclosures expose original UTC ISO and Unix milliseconds, respecting
the existing locale/timezone formatting. Null UTC reads **Unknown**; no epoch
zero, current receipt time or estimated timestamp is substituted. Safe integers
outside the JavaScript Date range use the existing numeric fallback. A missing/jumping wall clock
does not by itself invalidate monotonic measurement age or CPU elapsed time.
Observation source and availability must be readable without hover or color.

Implement freshness inside the existing refresh owner, with its injected
monotonic clock. The wire DTO need not gain a desktop clock field: store local
receipt monotonic time alongside the accepted section value.

```text
snapshot age now = snapshot.age_ms + (desktop monotonic now - receipt monotonic)
field age now = snapshot age now
              + (sampled_monotonic_ms - field.observed_monotonic_ms)
stale threshold = cadence.stale_after_ms = 2 * cadence.interval_ms + 1000
age is stale only when it exceeds that threshold
```

Offsets must be valid within the same sampler identity; negative/incomparable
offsets are not clamped into fresh zero-age data. After a local monotonic baseline
becomes unusable, keep retained data stale until a validated receipt establishes
a new baseline. Reconnect retires old local baselines. Within one identity and
sequence, a later receipt cannot reduce effective observation age or alter the
sample/field observation times; preserve an age floor or reject inconsistent
replies. Reads of the same cached sample never renew observation time. A newer
sequence may contain a retained old peak, so field freshness is still independent.

Default idle interval 15000 ms gives a 31000 ms stale threshold; active 2000 ms
gives 5000 ms; pressure 1000 ms gives 3000 ms. Use the validated declared cadence,
including allowed adjusted values, rather than these defaults as UI constants.
The existing health/job 10000 ms freshness rule is unchanged. A fresh health check
can coexist with stale Meter values, and a healthy idle Meter at age 11000 ms is
not stale. Show each stale field explicitly. If all available fields are fresh,
the section can say **“Current sample”**; partial availability still needs its
own status. Staleness alone never fabricates a transport disconnect.

### Availability and retained-value examples

Availability, sample freshness and connection are separate axes. The examples
below are synthetic copy specifications, not observed readings.

| State | Visible example | Required behavior |
| --- | --- | --- |
| Initial request pending | `Unknown · Loading Meter…` | No Current sample, zero or healthy pressure without a validated observation. Refresh loading does not erase an existing valid value. |
| Available, fresh | `Host CPU 25.0%` with `2.000 s interval · 8 logical CPUs`, observed time and Derived source. | Render the supplied normalized number and its own interval. Available zero, such as `Swap used 0.00 GiB`, is legitimate. |
| Available, stale | `Current RSS 40.00 MiB · Stale` with original observed time. | Keep value, unit, source and time; do not style as current or change its date on Refresh. |
| `first_interval` | `Host CPU Unknown · Waiting for next sample` | No CPU interval or 0.0% claim; other fields and job inspection remain usable. Only host CPU has this wire state. |
| `unsupported` field | `Memory pressure Unknown · Not supported` | No native level/color inferred from another field. No permission escalation or automatic repeated prompt. |
| `denied` field | `Swap used Unknown · Permission denied` | Describe OS measurement access only; do not mark the entire daemon disconnected or clear other readings. |
| `failed` field | `Free space Unknown · Measurement failed` | Show a static local explanation. Do not render raw native/peer diagnostic text; retain other successful fields. |
| Failure with an earlier success | `Current RSS Unknown · Measurement failed`; secondary `Last known 40.00 MiB · Stale`, with its earlier observed time. | Retain at most one prior successful value per field if shown; it is visibly stale immediately after failure, even before its time threshold. The current availability remains failed. Never merge its old time with the new failed attempt. |
| Mixed fields | `Some readings unavailable` plus available RAM/RSS, unsupported pressure and denied swap. | Status and timestamps remain per field; one failure cannot turn successful values into Unknown. |
| Whole snapshot unavailable | `Meter unavailable` plus separately labeled last-known stale fields if any. | Meter operation failure does not erase successful health/job responses or certify all native fields unsupported. |
| Old daemon rejects operation | `Meter not supported by this daemon` | Scope unsupported to Meter; preserve connected health, recent jobs and selected outcome. Do not manufacture a schema-1 snapshot. |

A retained peak still supplied as `available` by the sampler uses its own older
observation age and can be stale while current RSS is failed or fresh. An earlier
desktop fallback is clearly secondary to the latest field status. Keep this
bounded in-memory state, not a history array or a persisted sample store. The
contract persists no resource samples. Any later persistence must use the existing
writer and SRS 14.5 retention (raw 72 hours, minute aggregates 30 days, hour/day
aggregates one year); no history capability is implied by this design.

## Recovery, ownership and accessibility

Extend `DesktopRefreshController` and the bounded typed bridge; do not add an
independent Meter poller, event subscription, timer loop or native-worker owner.
Meter is a separately handled read within the existing serialized cycle. Health
and job reads must complete even if Meter is unsupported, unavailable or malformed;
a Meter-only failure cannot switch off their automatic refresh/reconnect policy.
Manual Refresh status/Retry connection remain the existing coalesced controls,
not a per-field button grid. Meter reads use the shared absolute deadline and
permit; refreshes do not initiate native sampling or alter cadence.

On a confirmed disconnect, show the existing connection state immediately and
mark retained Meter values last-known/stale. Keep the selected ID, keyboard focus,
recorded outcome and all original times. Reuse existing capped reconnect delays
(1, 2, 4, 8, 16, then 30 seconds); no rapid Meter-specific retry chain. On reconnect,
health success establishes connectivity only. Meter returns to current only after
a validated snapshot and per-field age checks. Until its identity is reconfirmed,
old readings describe the previous connection, not verified current daemon cost.

Accept ordering within the current connection/generation and `daemon_instance`.
A lower sequence cannot replace a higher one; same-sequence reads only update
receipt/age bookkeeping consistently. A newly confirmed identity, including PID
reuse with a different start token, begins a new accounting window and may have
sequence 1. Discard old CPU intervals, sampled peaks and field fallback values
at that transition. Show **“Daemon restarted · new accounting window”** once.
Do not combine epochs or turn decreasing lifetime counters into negative usage
or savings. Reject late old-generation replies, even with a numerically greater
sequence. If no new identity is available, do not infer a restart from counters.

For an old daemon, remember unsupported capability only within the current
connection epoch; reconnect, manual retry or reopen can make one bounded probe.
Normal health/job refresh continues. A field's denied/failed state is not an
IPC retry instruction; a future scheduled sample/read may recover naturally.
No new service installation, upgrade prompt, credentials or OS permission flow
is part of this slice.

Hide/close pauses desktop reads and retires pending UI replies, retaining the
existing window and selection. Reopen expires old Meter values before presenting
them, coalesces one fresh cycle and reuses that window. About navigation and
Credits remain intact. Quit Desktop (Daemon continues) stops only desktop/tray
and desktop timers. Sampling continues in the daemon while GUI is hidden or
absent; component call counts alone cannot prove this native lifecycle behavior.
Keep execution, protocol-1, fail-open and recorded-outcome semantics unchanged.

Use named groups/headings and semantic definition lists. Pair pressure color and
icons with the words Normal/Warning/Critical and source labels; stale/unknown
pressure has an explicit status, without an implied healthy level. Use existing
foreground, muted, destructive and focus-ring tokens, retaining WCAG AA as the
implementation target; no contrast result is claimed here. Exact times and full
filesystem identity must be keyboard accessible. Preserve arrow/Home/End focus
movement, Enter/Space selection and stable job IDs; Meter arrival changes neither
selection nor focus. Wrap long identities without truncating their accessible text.
Use existing externalized messages and locale/timezone behavior; retain CJK-capable
fallback policy if localized, without introducing fonts.

Keep numeric rows outside live regions. Use one deduplicated polite status message
for meaningful Meter transitions (unavailable, stale, recovered, or native pressure
entering/recovering from Warning/Critical). Do not announce every value, sample,
timestamp, cadence tick or unchanged failure, and do not move focus to a warning.
Preserve existing connection/selected-outcome announcements and credits links.
Add no decorative animation; existing reduced-motion behavior remains usable.

## Deterministic component-test handoff

These cases are **unexecuted specifications** for backend-desktop, using existing
`FakeClock`, `NativeFixture`, deferred replies, `mountDesktop`, `within` and
semantic DOM/keyboard assertions. Extend the fixture's daemon-read accounting
to include the typed Meter command so `maxInFlight === 1` remains meaningful.
Keep new source/test files below 500 lines; use the reserved
`apps/desktop/tests/meter.test.ts` plus existing contract/dashboard/lifecycle suites.
No additional active Bun entry file or browser E2E is required.

Define a valid synthetic fixture F from the shared Rust Meter fixture with D1 as
its synthetic daemon identity, sequence 7, `sampled_monotonic_ms = 30000`,
`sampled_at_ms = T` (existing harness wall-clock T), `age_ms = 0`, idle cadence
15000/31000, and each current field observed at monotonic 30000 / wall T. CPU is
25%, with interval 28000→30000, elapsed 2000 ms, UTC T−2000→T, 8 logical CPUs and
`host_capacity` normalization. Supply native RAM 8589934592 bytes (8 GiB), Normal
memory pressure, zero used swap, filesystem capacity 549755813888 / free
137438953472 / available 128849018880 bytes (512/128/120 GiB), and a synthetic local
`/` apfs identity. Disk pressure is derived Normal. Daemon CPU is 12.5 core-seconds,
RSS 41943040 bytes (40 MiB), sampled peak 46137344 bytes (44 MiB), with peak-window
start monotonic 0 / UTC T−30000. All other fields/unit/provenance follow schema 1.
Keep fixtures internally valid; invalid cases explicitly test boundary rejection.
These numbers are not accuracy, performance-limit or savings evidence.

| ID / suite | Input or transition | Concrete required assertion |
| --- | --- | --- |
| M01 meter/dashboard | Meter pending, then valid F; selected job B already records exit 17. | Pending fields are Unknown/loading, never invented zero. After arrival Host/ARC daemon groups and F values appear; B's Failed · Exit code 17 and focus remain. |
| M02 meter | Shared first-interval fixture; later validated CPU interval. | Waiting for next sample, no CPU 0.0% or interval on first sample; legitimate available RAM zero still renders. Later CPU interval replaces waiting without clearing jobs. |
| M03 meter/dashboard | F with no further Meter result; advance 11000 ms, separately allow fresh health. | Idle Meter remains current under 31000 ms despite the existing 10000 ms job rule; fresh health does not overwrite Meter time. Preserve existing job-staleness assertions. |
| M04 meter | Freeze/defer new Meter results; advance F to 31000, then 31001 ms age. | Fresh at exactly 31000; Stale at 31001. Numeric value and observation time stay fixed; staleness alone does not say Disconnected. |
| M05 meter | Same D1/sequence reread at age 30000 ms with a newer receipt; advance 1000 then 1 ms. | Observation times/interval stay fixed; age 31000 is fresh and 31001 stale. A later same-sequence reply claiming age zero cannot rejuvenate it. |
| M06 meter | Valid active 2000/5000 and pressure 1000/3000 cadence fixtures; include another allowed interval. | Their exact declared threshold is used with the same strict boundary; no hard-coded 10-second or 31-second Meter rule and no cadence inferred from job phase. |
| M07 meter | Health/recent/detail refresh successfully while Meter is stale, then reverse with detail stale and fresh Meter. | Section states remain independent; successful requests do not erase stale evidence or relabel recorded failure. |
| M08 meter | CPU first_interval; pressure unsupported; swap denied; filesystem failed; RAM/RSS available. | Waiting, Not supported, Permission denied and Measurement failed are distinguished per field; other fields still render and no missing value becomes zero/Normal. |
| M09 meter | New D1 snapshot has failed RSS, retains earlier successful peak with old observed time; optional last-known RSS fallback. | Current RSS's latest status is failed. Any fallback is secondary Last known · Stale; peak retains its own prior time and becomes stale by field age even with fresh sample/health. |
| M10 meter | Later successful RSS below the unchanged running peak, now confirming it at a new observation time. | Same peak number, updated Peak last observed, unchanged Peak observation started; no “peak occurred at” claim or reset. |
| M11 meter/contracts | F's distinct free/available/capacity; separate identity-unavailable invalid/failed cases. | Show 128/120/512 GiB under correct labels and full synthetic identity details. Unknown identity is not `/`; contradictory known values or invalid nonlocal identity are rejected at the typed boundary. |
| M12 meter | Fixtures carry policy-consistent derived disk levels at 5%, 10% and >10%; separate unavailable/zero capacity case. | Derived label and threshold explanation are readable; bytes retain Native source. Unavailable judgment has no Normal fallback. UI consumes supplied judgment, not a second derivation from rounded numbers. |
| M13 meter | F's CPU 25%, N=8, 2000 ms; daemon 12.5 core-seconds. | Host displays 25.0%, interval and all-host denominator, neither 3.125% nor 200%. Daemon displays 12.50 core·s with process-lifetime scope, never 12.5% or job attribution. |
| M14 meter | Available swap zero versus unsupported/denied/failed swap; small and large exact byte values. | Known zero renders 0.00 GiB; unknown variants render reasons. GiB/MiB divisors are correct; a positive amount below displayed precision uses the specified less-than label and retains exact bytes in details. |
| M15 meter/dashboard | Distinct field, sample, receipt and health times; null field/interval UTC, then a safe integer outside the Date range. | Each Exact time control uses its actual source. Missing UTC says Unknown, never receipt/epoch zero; health observation remains Unknown. Numeric fallback does not crash. |
| M16 meter | Wall receipt jumps back/forward one hour while fake monotonic advances; separate unusable local monotonic baseline. | Valid monotonic ages/CPU elapsed do not jump with UTC. An unusable monotonic baseline marks retained data stale pending new validation, never fresh negative age. |
| M17 contracts/bridge-client | Wrong schema/unit/provenance, missing tagged value, extra unknown field, unsafe integer, invalid CPU interval or unbounded result. | Typed bridge rejects invalid Meter data and raw diagnostics; earlier validated Meter is stale and successful health/job reads continue. Health remains its original exact shape with no invented metric fields. |
| M18 meter/contracts | Explicit old-daemon UNSUPPORTED_CAPABILITY; independently test METER_UNAVAILABLE and transport failure. | Meter unsupported, Meter unavailable and confirmed disconnect remain distinct; no synthetic snapshot. Unsupported does not disable later health/recent/detail reads; one coalesced retry/reconnect/reopen may probe again. |
| M19 dashboard/meter | Select B/exit17, then disconnect; repeated Retry, existing 999+1 ms reconnect boundary. | Preserve B, outcome, focus and original Meter/job times; old values stale. Retry requests only reads and is coalesced; health recovery alone does not refresh Meter. |
| M20 meter | D1 sequence 9 → confirmed D2 sequence 1, same PID/different start token; D2 CPU first_interval and smaller peak. | Accept new epoch, reset Meter counters/fallback/peak window, keep selected job/outcome. One restart notice; no negative CPU, cross-epoch peak, inferred savings or D1 carryover under D2. |
| M21 meter/lifecycle | Delayed D1 or retired-generation reply arrives after D2; separate same-D1 lower sequence. | Old reply cannot replace accepted state even if its sequence is larger; lower same-instance sequence cannot roll back values. Loading flags settle without adding a new read owner. |
| M22 dashboard/meter | Deferred Meter/read, repeated refresh, selection A→B, late A detail. | Combined daemon reads have maxInFlight 1, no queue growth; B remains selected, late A ignored and no command execution/shutdown call is sent. Meter failure cannot starve job reads. |
| M23 lifecycle/meter | Hide with a pending Meter reply; advance beyond stale threshold; repeated show/reopen before retired read settles. | Hidden read count/timers stop, late result cannot publish, same window/selection retained; reopen ages retained values first and queues one serialized fresh cycle. Component evidence makes no claim to prove daemon sampling. |
| M24 lifecycle | Quit Desktop with Meter timer/read pending; send later visibility events. | Exactly existing desktop-only quit request, no new reads after quit, no daemon action and no leaked timers/subscriptions under StrictMode/unmount. |
| M25 dashboard/credits | Meter updates while B focused, About navigation and credits/opener failure cases. | Existing keyboard selection, outcomes and all Credits assertions remain; social links/CTA and lifecycle actions keep their exact semantics and error messages. |
| M26 meter/dashboard | Tab to field exact-time/full-identity disclosures; script repeated numeric samples then stale/unavailable/recovery/native-pressure transitions. | Semantic names, keyboard disclosure and unchanged job focus; numeric rows are not live. Deduplicated polite transitions do not announce each sequence, timestamp, value or repeated identical error. |

Scope component queries to the actual Resource availability/Host/ARC daemon
groups. Replace the old “exactly eight Unknowns” placeholder assertion with
truthfulness assertions about the named unsupported fields; do not drop its
no-invented-zero/outcome intent. Retain the existing tests for fake native replies,
keyboard selection, disconnect/reconnect, recorded outcomes, Credits and lifecycle.
Existing command-allowlist assertions add only the delivered bounded Meter read;
they must continue rejecting arbitrary execute/stop operations.

Backend-gates must separately verify production-daemon CLI/bridge behavior,
at least ten successive native sample sequences, disconnect/reconnect, restart,
unsupported old-daemon behavior and headless/adaptive sampling. Fake fixture
responses or ten reads of one cached sequence are not that evidence. Native
calibration must compare independent references and explain actual discrepancies.
The warmed noninstrumented hidden-desktop idle run measures at least 300 seconds:
aggregate daemon user+system CPU delta / monotonic elapsed × 100, **one core**,
below 0.5%; RSS below 50000000 bytes. A UI sample, self-report or synthetic number
cannot certify those limits, long-term performance or customer savings.

## Retention ideas and deferred learning

The supplied advice proposed a neutral summary versus a salience/dark-pattern
variant foregrounding real pressure and repeating a dismissible return-check
prompt. Possible conversion lift, interruption and later abandonment are all
**unmeasured hypotheses**. Immediate opens could rise while correct interpretation,
task completion or voluntary reuse worsens; no direction or magnitude is known.

This slice adds no prompts, manufactured urgency, savings claims, analytics,
exit friction, hidden quit control or pressure-driven navigation. Native pressure
is an observation, not a claim that ARC prevented harm. Close-to-hide and a
continuing daemon preserve existing utility behavior; their uptime is not proof
of voluntary retention. The Chief already deferred the experiment.

If separately authorized later, compare identical truthful readings under neutral
and repeated dismissible salience treatments, with explicit eligible-user
denominators. Record correct-interpretation conversion, inspection task success/
time, interruptions, dismissals, voluntary seven-day reuse and observed uninstall
or reported abandonment. A prompt click is not purchase; non-return alone is not
churn. Prefer a treatment only after actual data show useful interpretation/reuse
without worse usability or exit understanding. No such experiment, recruitment,
instrumentation, simulation or field test was conducted or added to acceptance.

## Acceptance evidence and handoff limits

- Supplied: implementable labels, unit conversions, scope copy, field-state
  examples and M01–M26 deterministic assertions for the existing Dashboard.
- Supplied: independent observation/receipt/health times, cadence-aware field
  age, cached-read protection, host normalization and daemon/peak windows.
- Supplied: the Chief's inline choice, alternatives, inferred personas and
  retention ideas labeled as judgments/hypotheses, with no claimed user results.
- Compatibility: consumes schema 1 inside unchanged authenticated protocol 1;
  requests no contract/manifests/migration change or new refresh/sampling owner.
  Implementation and executable verification remain with backend Actors.

Executed inspection commands and actual outcomes:

| Command / check | Outcome |
| --- | --- |
| `git rev-parse HEAD HEAD^{tree} efedf0f4eb9b97eed641c82f3c4766bb16f2e3de^{tree} b259d3f1c459afab178fa4f1a0296dfa785c7d9c^{tree}` | Exit 0; actual identities recorded above. |
| `git ls-tree <accepted-or-snapshot-commit> -- .arc-verify/host-meter/contract.md docs/requirements/ARC_SRS_2026-11-06.md crates/arc-core/src/meter.rs` | Both exit 0; entries absent at accepted baseline, present at inspected snapshot. |
| `git status --short` and scoped `git status --short -- <inspected paths>` / `git diff --stat -- <assigned input areas>` | Exit 0; no initial scoped changes. Root command emitted av.yaml access denial; whole-worktree cleanliness is unverified. |
| `shasum -a 256 <SRS source, copied SRS and seven tabled input paths>` | Exit 0; both SRS hashes match the required digest and snapshot-input hashes are recorded above. |
| Scoped `cat` / `sed -n` reads and non-code SRS `rg -n` searches | Required contract, types, UI/controller/test infrastructure and SRS sections inspected. No source test was executed. |
| `git rev-parse --verify <supplied snapshot-ref>` and initial combined revision query including worker-ref | Exit 128; references unresolved locally. The successful commit/tree query is recorded separately. |
| `cat target/host-meter/task-1/handoff.md`, `cat .arc-verify/host-meter/experience.md` before writing, and reads of DESIGN/context files | Exit 1 for missing files as recorded above; no raw task-1 receipt or existing design system was claimed. |
| `git diff --exit-code HEAD -- <frozen Meter inputs and apps/desktop>` | Exit 0 after document delivery; inspected contract, SRS, Meter types/fixture and desktop inputs remain unchanged. |
| Final scoped `git status --short -- <evidence, source, manifests and generated result/state areas>` | Exit 0; only the new untracked experience.md is reported in those areas. No other Actor delivery is changed. |
| `git diff --no-index --check -- /dev/null .arc-verify/host-meter/experience.md` | Exit 1 with no whitespace diagnostics; the new file differs from /dev/null. This comparison is not counted as a passing gate. |
| Inline read-only `python3` document inspection; `wc -lc .arc-verify/host-meter/experience.md` | Exit 0; UTF-8, trailing whitespace, final newline and fenced blocks are clean, and this file is below 5 MB. This inspects prose only, adds no checker file and is not a product test or the backend's mission evidence check. |

Only this one reserved mission evidence file is added; no second checker, raw
log, result file or product/test file is introduced. The existing reservation
remains at most seven tracked files / 5 MB with one backend-owned small evidence
check; this task does not certify the eventual integrated inventory or gates.

No builds, native gates, unit/integration/CLI/component suites, typechecks,
screen checks, browser E2E, Playwright downloads, GUI actions or field tests ran
in this documentation task. Rendered layout/contrast, assistive technology,
native correctness, actual overhead and integration are pending their assigned
owners. Missing raw dependency logs and unresolved collection references limit
local provenance verification; av.yaml access limits configuration inspection.
Independent read-only QA must review the actual integrated inputs and fresh
backend receipts. Remaining scope: Managed-job/vendor attribution, Native Windows,
Vendor hooks and Full Meter/Phase 1 acceptance.
