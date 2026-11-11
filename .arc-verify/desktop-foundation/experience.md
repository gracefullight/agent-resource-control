# ARC desktop foundation: experience and acceptance contract

Status: task-2 design handoff, ready for implementation; product acceptance remains unverified. Owner: Design Director. This task owns only `.arc-verify/desktop-foundation/experience.md`. Implementation, test edits and native gates belong to the assigned backend-engineer Actors using `oma-backend` and `oma-frontend`; independent review belongs to the read-only `oma-qa` Actor.

## Evidence, revision and limits

The accepted committed source baseline is `a9e7a31566e833dceeb82d6d75cd7c6a399583ab`, product tree `c4506e64abaef97daf6f701cac2870c98cfeb91f`. The inspected Actor-local HEAD and initial snapshot commit are `b7e74b6566c909785eadd6b84b96a323f85e2a56`, with the same product tree. This is the task-2 workspace on branch `av-task/8bb32cc241dc9a3c-78686d2704d4d690-1`; the equal trees do not make its commit the accepted baseline. The supplied snapshot and worker reference names identify collection handles, not independently verified integration commits. This document is a new, uncommitted task deliverable until the mission integrates it.

Initial `git status --short --branch`, staged/unstaged diff summaries and scoped tracked/untracked listings showed no preceding desktop changes. `apps/desktop`, this evidence directory, `DESIGN.md` and `.design-context.md` had no tracked entries. Git also reported `av.yaml: Operation not permitted`; its index entry exists, and its working-file contents were not inspected. No deletion or fully clean-worktree claim follows from that denial. No mission state or preceding work was changed.

Primary requirements were inspected in `/Users/gracefullight/Downloads/Agent_Resource_Control_ARC_Requirements_SRS_2026-11-06.md`: §§15.1–15.4, 15.7–15.10, AT-GUI-01/04 and §26.1. Its SHA-256 is `a8ef945251f9f3fd7c3e337d4efc77ff9914f95114212ca9e6c90ae2ce3e3d9e`. It is an external local requirement input, not a file claimed to exist in the accepted Git tree. The supplied assignment provides the ARC developer personas and foundation scope; broader SRS navigation and policies are future scope.

The following known source paths were read at the Actor snapshot. `git diff --exit-code a9e7a31566e833dceeb82d6d75cd7c6a399583ab -- <these paths>` exited 0, confirming their inspected bytes match the accepted baseline. Historical reports were read only as historical evidence; no prior native pass is adopted as current desktop proof.

| Inspected source | Observation | SHA-256 |
| --- | --- | --- |
| `crates/arc-ipc/src/lib.rs:368–374` | Health reports daemon running, protocol version and Sweeper off; no resource measurements or daemon sample timestamp. | `17ff544a163bf7c831b8f3c84092c03047eb01498a9696c2004fe888229bdf46` |
| `crates/arc-ipc/src/jobs/types.rs:128–163` | `JobSnapshot` includes outcome and record/observation timestamps; `JobSummary` omits outcome. | `9459b9a22f0e3cd2ca2c921e5039117007adc928354587612759ad840380ff7b` |
| `crates/arc-cli/src/job.rs:219–300` | Recorded exit 0/nonzero, signals, spawn failure and cancellation are distinguished; completion without an exit remains unknown. | `c2024ede50b9f4073cb7979f5f6292b04365c2a9cba5ac98721240eed7e93ccb` |
| `docs/managed-exec.md` | Retrieval success is separate from child success; a missing record proves neither execution nor nonexecution; recovery never replays work. | `37d3fa4497330aec587477d6262b138aa894b744e84b6e10c1e78b983432ddce` |

Skill used: `oma-design`, resolved at `.agents/skills/oma-design/SKILL.md`, with its external-routing, execution-protocol, checklist and data-grid references. Operation: local contract documenting the supplied design decision, permitted by `resources/external-design-skills.md`; no external supplier or new visual-system artifact is needed. Serena discovery tools were unavailable in the tool catalog. Source inspection used direct reads of the exact paths supplied by the assignment; non-code requirement searches used `rg`. No code-search fallback, dependency installation, browser, screen check or GUI workaround was used.

**Evidence classification:** the source facts above are inspected observations. The journeys, personas, polling defaults and retention reasoning below are design judgments or hypotheses. All fixtures are synthetic deterministic test inputs, not daemon captures or simulations that were executed. There are no participant results, task-time measurements, accessibility interaction results, conversions, churn observations or revenue measurements in this task.

## People, alternatives and selected journey

Two requirement-derived personas guide the foundation. Both are **unvalidated hypotheses**:

1. A developer checking background build/test work wants to know whether ARC can answer and what the selected job actually recorded.
2. A returning developer investigating tracking after an outage wants the last reliable result, its time and a way to retry observation without launching another copy.

These are local coding-agent/build users. Actor rosters and Agent Valley orchestration controls do not belong in this ARC interface.

| Proposed alternative | Proposed path to a particular recent job | Judgment |
| --- | --- | --- |
| A: direct Dashboard | Activate tray icon → select recent row → details appear in the same window. | Choose A. Two explicit actions for a nondefault selection, without a surface handoff. |
| B: separate tray popover | Activate tray icon → open Dashboard → select row. | Three proposed actions; a compact glance surface may help later, but adds a handoff and another state owner now. |
| C: metrics-first landing | Activate tray icon → navigate to Jobs → select row. | Three proposed actions; unavailable metrics would occupy the first destination. |

These action counts describe the proposed designs, not measured usability or completion times. The mission already selects A; this contract makes it implementable. A newly opened Dashboard may automatically select the newest row and retrieve its details, reducing interaction for that particular case. This is not a measured first-value improvement.

The window has two destinations: **Dashboard** and **About / Credits**. Dashboard order is: daemon connection and time; compact resource-metric availability; recent-job list; selected-job details. At a wide desktop size, list and details sit beside each other. At narrower sizes, details follow the list in reading order with a visible “Selected job” heading; selecting does not navigate away or trap focus. Keep resource placeholders compact enough that jobs remain easy to reach. No separate popover, metrics landing, unused navigation or unwired policy controls are required for this foundation.

### Selection and refresh rules

1. Opening or reopening activates the existing window and starts one bounded refresh cycle. `daemon_health`, `recent_jobs` and `job_by_id` refer to the delivered typed Rust command interface; this document does not define a second wire schema. Rust retains authenticated protocol 1, same-user checks, frame bounds and transport deadlines. The frontend cannot send arbitrary operations or payloads.
2. Recent history is limited to **50** records. Show “Recent jobs” and “Showing {count} recent records · limit 50”; when the response explicitly says truncated, explain that older records are omitted. Neither a short list nor a count of 50 establishes global running, queued or total jobs. Recent observations are last-known records, not fresh process-liveness checks.
3. Each compact row identifies the opaque job ID, recorded phase and record update time. It contains no command/chat text, argv, working-directory path or token. Short IDs expose the full ID on focus and in details; identity stays keyed to the full ID. Use one line per row, consistent time formatting, aligned columns and no nested row actions. Details can wrap full values.
4. After the first successful nonempty list, select its first/newest returned row unless a prior selection exists. Selecting any other row starts only its bounded by-ID lookup. Immediately identify the new selected ID and show its loading state; never place the former job's outcome under the new ID. Do not fetch all 50 outcomes to make the list look resolved.
5. Details show job ID, recorded lifecycle/scheduling state when available, recorded outcome, exit code/signal/spawn reason where supplied, record timestamps and identity observation with its time. Raw reason codes can be secondary text with a plain-language label. No invented progress percentage, completion estimate, rerun action or process-control action is added.
6. Cache a validated result against its exact job ID. A recent row may then show that job's cached outcome with last-known freshness. An unresolved `COMPLETED` summary says “Outcome unknown”, never “Succeeded”. A more recent update makes the old cached result visibly last-known until refreshed; an older or late reply cannot overwrite a newer accepted revision for that ID.
7. Preserve selection through refresh, reconnect, hide/reopen and About navigation during this desktop session. If the ID falls outside the recent 50, retain the selected panel and label “Selected job is outside the recent list”; this alone is not a missing-record response. A fresh validated not-found response is a different state.
8. One refresh owner handles polling, manual Refresh status, selection changes and reconnect. Coalesce repeated refresh activation; at most one active lookup per kind and no timer multiplication. Tag results with connection generation and selected ID; discard late results from an older generation or selection. Unmount/hide cancels or retires pending refreshes. Reopening cannot create another window or another polling owner.

Suggested deterministic defaults are a 5-second visible refresh interval, capped reconnect intervals of 1, 2, 4, 8, 16 and then 30 seconds, and a 15-second client-freshness threshold. These are unmeasured design defaults, not transport deadlines or a latency guarantee. Each exchange remains bounded by the bridge's delivered deadline; requests never overlap to chase a timer. A confirmed disconnect is shown immediately. The hidden window can suspend list/detail polling; on reopen, retained data is Stale if its freshness interval expired and refresh begins immediately. The minimal tray need not imply live health or totals while polling is suspended. Automatic retry uses the capped interval; “Retry connection” can request an immediate coalesced attempt. Test timers are injected, without real sleeps. The transport owner retains the existing IPC/retry semantics.

## Connection, retrieval and freshness states

Connection, each data section's retrieval state and each job's recorded outcome are independent. For example, “Disconnected” plus “Failed · Exit code 17 · last recorded …” is valid. A lost connection cannot erase or reclassify a previously recorded failure as success or outcome unknown.

| State / evidence | Visible copy and retained content | Action / deterministic requirement |
| --- | --- | --- |
| Loading, no prior response | “Checking daemon…”; “Loading recent jobs…” or “Loading job details…” for each pending section. Unknown metric placeholders remain readable. | Expose busy state on that section. Never show zero jobs or Connected before validated evidence. |
| Connected | “Daemon connected”, protocol 1 and “Last successful check: {desktop receipt time}”. | Means the authenticated health exchange succeeded; it does not claim every subsystem, adapter or job is healthy. |
| Empty successful history | “No recorded jobs yet.” Explain “ARC has no records to show in this recent-history response.” | No selected details and no false “No work ran” claim. Refresh status remains available. |
| Missing selected record | “No matching record.” Explain “A missing record does not establish whether the command ran.” | Preserve selected ID. If cached details exist, show them separately as last-known; do not present them as the current lookup result. Offer Refresh status, never replay work. |
| Section status unavailable | “Status unavailable” with a concise redacted cause, for example “Job details could not be retrieved.” | A successful health check may remain Connected while list/detail retrieval fails. Preserve that section's prior data and timestamp as Stale. |
| Disconnected, no prior data | “Disconnected”; “Daemon could not be reached.” Data availability is Unknown. | Offer Retry connection and bounded automatic reconnect. Do not imply the daemon or active child stopped. |
| Disconnected, prior data | “Disconnected” and “Stale · showing last recorded data”, with last successful check and each retained receipt time. | Keep rows, selection and known exit results. Never display “Live” or replace them with fabricated zeros. |
| Stale without a known transport failure | “Stale · last refreshed {time}”, for example after suspended polling exceeds the freshness threshold. | Do not claim a confirmed disconnect from age alone. A newly received list does not refresh a separately failed detail request. |
| Malformed / mismatched response | “Status unavailable · invalid daemon response.” Retained data stays Stale. | Reject malformed envelopes, unsupported shapes and a by-ID result with a different ID. No fallback to its payload, green success label or raw response/command text. |
| Incompatible protocol / capability | “Status unavailable · incompatible daemon response” or “This status is not supported by the connected daemon”, according to the typed error. | Do not mislabel a successfully contacted but incompatible daemon as healthy. Show last successful compatible check separately; preserve old records and offer retry. |
| Reconnected | Replace connection state only after a fresh validated health response. Refresh list and selected details independently. | Announce the connection change once. Keep last-known details until a matching fresh result replaces them; discard pre-disconnect late replies. |

“Stale” describes the freshness of the desktop's observation or cached section. An old completed job is not stale merely because it completed yesterday: its recorded exit remains a recorded exit. A fresh receipt also cannot turn an old process-identity observation into proof that the process is running now.

### Outcome mapping

Use typed validated evidence. A query's successful return means retrieval succeeded, not that the child exited successfully. Keep lifecycle state and outcome as separate fields when needed.

| Available record / outcome | User-facing result | Evidence limits |
| --- | --- | --- |
| Registered / launching, no terminal outcome | “Starting · last recorded” | Do not claim the child has started. |
| Explicit scheduling queued / reserved, if supplied by the bridge | “Queued” / “Starting”, with supplied queue detail only | Recent lifecycle summaries do not by themselves prove these scheduling states. |
| Running phase / identity observation | “Running · last recorded”; identity “Verified at {observation time}” if supported | Not a current liveness or whole-system count claim. |
| Recorded normal exit, code 0 | “Succeeded · Exit code 0” | Requires a matching detailed record with that exit, not `COMPLETED` alone. |
| Recorded normal exit, code 17 or another nonzero code | “Failed · Exit code 17” or the actual code | Retain the exact supplied code; status retrieval success does not change failure. |
| Recorded signal termination | “Terminated by signal”; actual signal and core-dump flag when recorded | A signaled phase can identify the phase; absent signal details stay Unknown. |
| Recorded spawn failure | “Spawn failed”; supplied reason and optional OS code | No invented exit code or claim that target work ran. A missing optional code is Unknown. |
| Explicit confirmed cancellation from scheduling evidence | “Cancelled” | Lifecycle `JobSnapshot` has no cancellation outcome variant. If the delivered typed bridge cannot retrieve explicit cancellation, retain Unknown rather than infer cancellation. Report this interface gap to the Chief. |
| Historical completion without a recorded exit | “Outcome unknown” | `COMPLETED`, missing processes and a successful query do not establish an exit code. |
| Unconfirmed / abandoned reservation, if explicitly supplied | “Outcome unknown”; “Reservation retained” only with that evidence | Do not infer cancellation, success or released capacity. |
| Outcome absent, identity missing/mismatched/indeterminate | “Outcome unknown”, with recorded observation/reason separately | Identity evidence is not an exit outcome. Unknown is not Failed. |
| New lookup fails after known terminal outcome | Prior outcome plus “Stale · last recorded” | Distinguish the retrieval failure from the retained child result; never silently discard it. |

### Times and unknown metrics

Use distinct labels and original semantic fields: **Registered**, **Record updated**, **Completed** when recorded, **Identity observed** from `observation.observed_at_ms`, and **Status received** from the desktop clock after a validated response. Observer/child verification times, when shown, remain explicitly verification times. A protocol-1 health response has no daemon measurement time: its last successful check is a client receipt time, and a daemon sample time remains Unknown. Do not copy the desktop receipt time into any missing daemon field.

Render readable local absolute times including date and timezone. Provide the full ISO-8601 instant with offset or UTC and original millisecond value on keyboard-accessible exact-time disclosure; a hover tooltip alone is insufficient. Relative age may supplement the absolute time but never replace it. Missing timestamps say Unknown; no epoch-zero display. Different local clock orderings do not justify synthesizing or rewriting source times.

| Dashboard field | Foundation value / concise reason |
| --- | --- |
| CPU | **Unknown** — “No CPU measurement available.” |
| Memory | **Unknown** — “No memory measurement available.” |
| Disk written / usage | **Unknown** — “No disk measurement available.” |
| Storage reclaimed / Reclaimed today | **Unknown** — “No reclamation measurement available.” |
| Active / connected agents | **Unknown** — “Agent count is not reported.” |
| Global Running / Queued jobs | **Unknown** — “Recent history is not a live job total.” |
| Usage today | **Unknown** — “Usage totals are not reported.” |

The current health payload supplies none of these measurements. IPC request counters, historical benchmarks, recent-list length and placeholder zeros are not substitutes. Only future authenticated measurements with defined units, scope and sample time can replace Unknown. “Sweeper off” can describe the explicit reported field, not successful cleanup or reclaimed space.

## Native lifecycle and informed persistence

| Trigger | Required behavior / visible label |
| --- | --- |
| Activate macOS tray icon or **Open Dashboard** | Show, unminimize if necessary and focus the existing Dashboard window. Reuse its identity. No second popover or duplicate window. |
| Window close request | Prevent desktop exit and hide the same window; tray and daemon continue. Retire pending UI refreshes safely. |
| Reopen after hide | Reuse that window, restore selection and refresh expired sections. Retained old data is labeled Stale during recovery. |
| **About / Credits** | Show the credits destination in the same window; preserve job selection and return focus appropriately. |
| **Quit Desktop (Daemon continues)** | Exit the desktop UI and tray only. Send no daemon-stop, cancellation or child signal; do not remove records or claim daemon ownership. No exit interception or extra confirmation. |

Show brief first-use copy in the Dashboard, without a modal gate: “Closing this window keeps ARC in the menu bar. Quit Desktop closes the desktop app; the daemon and running jobs continue.” Keep the explanatory quit label discoverable in the normal native menu. If a platform's conventional Quit action is supported, it has the same desktop-only semantics. Do not add startup enrollment or notifications to retain users.

Deterministic lifecycle tests must exercise registered Tauri commands and actual close/reopen/menu callback wiring, including `close_request_hides_existing_window`, `reopen_reuses_window` and `quit_desktop_never_stops_daemon`. Component tests alone cannot prove native callbacks. The integration owner must additionally measure daemon identity and active-child identity survival after a desktop-client subprocess exits through the production bridge. Mock callback invocations do not establish live tray interaction or process survival.

## Exact About / Credits contract

| Element | Required content / destination | Accessible behavior |
| --- | --- | --- |
| Application label | **Agent Resource Control** | Visible heading. |
| Creator credit | **Created by Eunkwang Shin** | Visible text with the exact spelling. |
| Personal GitHub | Icon-only Phosphor **`GithubLogo`** → **https://github.com/gracefullight** | Accessible name “Eunkwang Shin on GitHub”; tooltip “GitHub”; visible keyboard focus. No visible text substitutes for the icon-only requirement. |
| LinkedIn | Icon-only Phosphor **`LinkedinLogo`** → **https://www.linkedin.com/in/gracefullight** | Accessible name “Eunkwang Shin on LinkedIn”; tooltip “LinkedIn”; visible keyboard focus. |
| OMA call to action | Visible text **Try OMA** → **https://github.com/first-fluke/oh-my-agent** | Ordinary focusable link/action, keyboard activation and accessible name matching its text. |

The personal GitHub destination is `gracefullight`; the OMA repository owner is `first-fluke`. Preserve the exact destinations, without tracking parameters or substitutions. Use the official Tauri opener through a narrow fixed-destination interface, on user activation only. The app must not automatically open links or fetch remote profile content. If opening fails, show a concise inline error and retain focus. Deterministic opener assertions prove the requested destination, not that a live default browser opened.

Metadata fields are **Version**, **Build commit**, **OS architecture**, **License** and **Open-source notices**. Derive version and architecture from authoritative application/build metadata; expose a build commit only when actually collected. Absent metadata is explicitly “Unavailable”. Do not use this report's accepted HEAD as the application build commit without build evidence. SRS §26.1 calls Apache-2.0 a proposed default, so it is insufficient to claim an adopted product license. The implementation owner must use an authoritative license/notices source or show “License unavailable in this build” / “Open-source notices unavailable in this build” and record the unresolved delivery metadata. Notices, when available, must be readable and attributable to the actual dependencies; a heading alone is not notices content.

Keep Try OMA within About / Credits as a quiet utility baseline. There is no paid funnel, persistent Dashboard banner, interruption, fabricated social proof or automatic outbound navigation in this foundation.

## Keyboard, announcements, language and layout

- Use a labeled, single-selection listbox for recent rows, with `aria-selected` bound to full job ID. Arrow Up/Down moves focus; Home/End moves to the first/last row; Enter/Space selects and loads details. Use one roving tab stop. Options contain no nested interactive controls. Their accessible names include identity, recorded phase/outcome and update time. An equivalent native single-selection primitive must preserve these semantics and be covered by the same tests.
- Tab/Shift+Tab reaches navigation, Refresh status / Retry connection, the list, details' exact-time controls, credits links and notices in reading order. Selection does not steal keyboard focus into details. On reopen or navigation return, restore the prior meaningful focus; if its row disappeared, use the selected-detail heading or list label. Do not reset focus every poll.
- Announce Disconnected and recovery once per transition through a polite live region; announce a newly selected detail result concisely. Do not announce every unchanged poll, timer tick or metric placeholder. Busy and error states are associated with their sections, and no native alert steals focus.
- Status always has text plus an optional Phosphor icon. Color alone, spinner alone, hover alone and unexplained abbreviations are insufficient. Use the implementation's shared semantic tokens, visible focus and WCAG AA contrast targets; this handoff claims no measured contrast or screen-reader compatibility.
- Default product strings are externalized English strings, including loading, errors, Unknown reasons, tooltips and plural/count forms. Use named placeholders and locale date formatting, without concatenating sentence fragments. Required proper names and URLs stay exact. Korean-ready layout is required, not a claim of a completed Korean translation.
- Retain system font fallbacks with Hangul coverage, natural Korean line breaking and flexible text width/height. Avoid fixed-width labels, text baked into icons and clipped translated status text. Full IDs and exact times remain accessible when compact rows truncate. At a narrower window and enlarged text, details stack and all actions stay reachable; no responsive or glyph inspection has been executed here.
- Keep the prescribed React 19, TypeScript 7, Vite, Bun, Tailwind 4, basecn/Base UI, Phosphor, Biome and Tauri 2 stack. Use existing primitives and semantic tokens; this contract adds no state framework, data-grid dependency, font download, animation system or separate design-system file. Reduced motion must preserve feedback without decorative motion.

## Synthetic state fixtures and deterministic assertions

These are **proposed test fixtures**, not captured traffic, performance results or user evidence. They describe the presentation model after the bridge has validated a response; they are not a wire DTO specification. The delivered Rust/TypeScript contract owns serialized field names and error types. Use a fake clock `T = 2026-11-12T00:00:00.000Z`, locale `en-US` and timezone `UTC` in component tests. Separate recorded update `T−10s`, identity observation `T−20s` and desktop receipt `T`. Full fixture IDs are 32 lowercase hexadecimal characters: A = `0000000000000000000000000000000a`; B = `0000000000000000000000000000000b`.

| Fixture | Input / transition | Required assertion |
| --- | --- | --- |
| F01 initial-loading | All requests pending, no cache. | Checking/loading states appear; Connected, zero totals and Succeeded do not. |
| F02 empty-history | Valid health, valid empty Recent. | Connected + No recorded jobs yet; no selected result or claim that no work ran. |
| F03 summary-only | A summary has phase `COMPLETED`; by-ID pending. | Outcome unknown in the row; details load for A. Summary completion is never success. |
| F04 exit-zero | Matching A detail, `EXITED` code 0. | Succeeded and Exit code 0; query success is not independently used as the outcome. |
| F05 exit-seventeen | Matching B detail, `EXITED` code 17. | Failed and Exit code 17, not Succeeded; record and receipt times stay distinct. |
| F06 historical-unknown | Completed record has no recorded outcome/code. | Outcome unknown; no fabricated 0, signal or completion estimate. |
| F07 signal | Matching detail carries a recorded signal, for example 15, and recorded core-dump flag. | Terminated by signal with the actual values; absent optional details stay Unknown. |
| F08 spawn | Matching detail carries SpawnFailed and supplied reason, OS code absent. | Spawn failed; reason visible, OS code Unknown, no claimed child exit. |
| F09 cancellation | Explicit typed cancellation evidence, or its unsupported-capability counterpart. | Cancelled only in the explicit case; unsupported case remains unknown/unavailable. A missing process is never sufficient. |
| F10 running-and-reservation | Running with old identity observation; optional explicit queued/reserved/unconfirmed variants. | Running is last-recorded; reservation uncertainty remains Outcome unknown, with retention only if explicit. |
| F11 disconnect-retains-failure | Load F05, then transport disconnects. | Disconnected + Stale; preserve B, Failed, code 17 and their old timestamps. Retry does not invoke execution or shutdown. |
| F12 age-and-sections | Advance beyond 15 seconds without health confirmation; separately test healthy health/list with failed detail. | First is Stale, not a fabricated confirmed disconnect; second retains section-specific stale detail without changing its receipt time. |
| F13 reconnect | After F11, valid health then list then matching B detail arrive independently. | Connection recovers once; cached failure stays until replaced; old-generation replies cannot restore an older state. |
| F14 not-found | By-ID explicitly returns no matching record, with and without cached A. | Selected ID persists; no execution inference. Cached A is separately last-known, never presented as a fresh match. |
| F15 invalid-replies | Malformed payload, incompatible version and A request answered with B. | Appropriate unavailable/invalid/incompatible state; reject B under A; retain only previously validated cached data. |
| F16 selection-race | Select A then B; delayed A returns after B. | Selected ID and details remain B. An older revision of B also cannot replace a newer accepted B result. |
| F17 recent-boundary | Return 50 rows with truncation, then selected A falls outside list. | Limit/truncation disclosed; no global job-total claim. Retain A detail without inventing not-found. |
| F18 independent-times | Distinct observation/update/receipt times; another case omits daemon sample/completion time. | Each label uses its own source; exact instants accessible by keyboard; missing fields Unknown. |
| F19 unknown-metrics | Healthy daemon but no metric data, under every connection state. | CPU, memory, disk, reclaimed space, agents and global job counts stay Unknown with reasons; no numeric zero stand-ins. |
| F20 keyboard-and-language | Navigate options, select B, switch About and back, render long externalized strings. | Selected state, focus order and restoration are stable; connection announcements are transition-only; required strings come from localization resources. No participant or rendered-layout pass is implied. |
| F21 credits | About with real metadata or missing values, plus opener failure. | Five required credit elements, three exact destinations, icon-only social controls with names/tooltips/focus; no invented license or commit. Each activation calls only its allowlisted destination. |
| F22 lifecycle | Close callback, repeated reopen, then Quit Desktop action. | Hide preserves window identity; reopen reuses it; quit exits only desktop. Pending refreshes cannot mutate a hidden/replaced generation. |

Frontend handoff targets are the mission's `apps/desktop/tests/dashboard.test.ts`, `credits.test.ts`, `contracts.test.ts` and `lifecycle.test.ts`. Use deterministic component assertions with injected bridge results/timers and semantic DOM interactions; do not assert only that strings exist in source files. Cover loading, selection, retrieval failures, cached outcomes, time sources and native-command requests. Contract tests should reject mismatched IDs and unbounded/arbitrary operations at the correct boundary.

Backend and gate-owner handoff targets are the mission's registered Tauri-handler lifecycle tests and `apps/desktop/src-tauri/tests/daemon_bridge.rs`. Isolated real-daemon production-bridge tests must exercise exit 0/17, disconnect/reconnect, malformed responses, mismatched IDs and daemon/active-child survival after desktop-client exit. A malformed peer fixture may exercise rejection but cannot by itself establish successful authenticated real-daemon integration. Reuse the daemon's existing isolation/serialization and owned-child cleanup. Preserve CLI semantics, protocol/migrations and privacy. This document neither edits tests nor substitutes synthetic fixtures for those obligations.

## Retention judgment and a future field test

**Hypothesis:** a quiet, understandable utility earns voluntary return when another real job needs inspection. Close-to-hide can make the utility available, but background uptime is not voluntary engagement. Keeping Quit visible and explaining daemon persistence supports an informed exit and makes retention data interpretable.

| Proposed retention option | Expected effect, unmeasured | Decision / future evidence |
| --- | --- | --- |
| Quiet utility with discoverable quit and the mandated About CTA | May support repeat successful inspection with low interruption. | Chosen baseline. Measure successful inspections and deliberate returns if a later study is authorized. |
| More prominent Try OMA placement | May increase outbound clicks and distract from job inspection. | Hypothesis for a later comparison; retain the exact destination. Clicks alone prove neither conversion nor revenue. |
| Bury Quit, repeatedly intercept exit, or treat continued tray uptime as return | May inflate persistence while increasing exit time and misunderstanding. | Excluded from this foundation because informed exit and task success are explicit requirements. No retention or churn benefit is established. |

**Proposed, unrun field test:** four local developers spanning first use and return after an outage compare A with B using controlled exit-17, unknown-after-disconnect and close/reopen/quit tasks. Record unaided outcome correctness, actions, time, mistaken rerun intent and whether participants distinguish desktop exit from daemon/child termination. A proposed directional rule is that all four must distinguish failure from unknown and understand quit before selecting a faster journey; otherwise revise and repeat. Four participants would not establish a population conversion or retention effect, and this future study is not an added mission acceptance gate.

Future private aggregate measurements, only if separately authorized, could distinguish deliberate Dashboard opens, successful selected-detail reads, reconnect results and explicit quits from automatic uptime. D7/D28 voluntary return, disable/uninstall reports and task/exit success provide context; non-return alone is not proof of churn. Omit commands, chat, paths, tokens and persistent job identifiers. No new analytics or tracking is requested by this handoff, and no conversion, churn, profit or user reaction has been measured.

## Task-2 acceptance evidence and handoff limits

- Complete: the selected row/details journey, state and outcome matrices, independent times, Unknown metric reasons and deterministic fixtures are specified above.
- Complete: exact credits/destinations, metadata availability, keyboard/accessibility expectations, Open Dashboard and Quit Desktop (Daemon continues), close-to-hide and reuse on reopen are specified.
- Complete: personas, action counts, polling defaults, retention expectations and future field tests are labeled as judgments/hypotheses; there are no invented field results.

Executed checks for this artifact:

| Command / check | Actual outcome |
| --- | --- |
| `git rev-parse HEAD HEAD^{tree} a9e7a31566e833dceeb82d6d75cd7c6a399583ab^{tree} b7e74b6566c909785eadd6b84b96a323f85e2a56^{tree}` | Exit 0; HEAD is the snapshot commit and all three trees equal the supplied `c4506e64abaef97daf6f701cac2870c98cfeb91f`. |
| `git status --short --branch`, staged/unstaged diff summaries and scoped tracked/untracked listings | Exit 0; no initial desktop task edits. `av.yaml` access denial limits the whole-worktree observation as recorded above. |
| `git diff --exit-code a9e7a31566e833dceeb82d6d75cd7c6a399583ab -- docs/managed-exec.md crates/arc-ipc/src/jobs/types.rs crates/arc-ipc/src/lib.rs crates/arc-cli/src/job.rs` | Exit 0; inspected source files match the accepted baseline. |
| `shasum -a 256` on the SRS and four source paths | Exit 0; hashes recorded in Evidence above. |
| Inline `python3` document-conformance check | Exit 0; all required state/credit/metadata/provenance/limit phrases and F01–F22 fixtures present; minimum evidence size, final newline, balanced fences and no trailing whitespace checked. This is a document check, not a component test. |
| `git diff --no-index --check -- /dev/null .arc-verify/desktop-foundation/experience.md` | Exit 0; no whitespace errors in the new untracked document. |

Only this file is changed. This task contributes one of the maximum seven desktop-foundation evidence files and adds no checker, raw-log file, manifest, lockfile or product code. Downstream owners retain the mission's six mandated gates and raw-log hashing obligations; this report is not their gate receipt.

No implementation suite, build, typecheck, native handler, daemon integration or human study was executed by this documentation task. No screenshots, browser E2E, Playwright installation or screen-based checks were performed. Live native tray interaction, assistive-technology interaction, rendered contrast/layout, Windows acceptance and Full Phase 0 completion remain unverified. The independent QA and final-review owners must evaluate the integrated implementation and fresh gate receipts; this Actor snapshot and uncommitted report do not establish integrated completion.
