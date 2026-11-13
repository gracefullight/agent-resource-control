# Governor interaction handoff

Status: task-2 ready; implementation pending. Governor slice only. Task-7 owns implementation/tests; this task owns this document.

## Evidence and decision

Actor-local snapshot HEAD `6e652f028bee3efd4c29356bdceb29ff7df6ee95` has tree `d3cbd610653d0b78adeb2ffc26a9cab1790e877f`, equal to accepted baseline `08e7e526cce7e03ca9264afc926ed2e878dfde6d`'s tree. This new document is absent from committed baseline. No accessible preceding edits; `av.yaml` denied/uninspected.

Observed in `apps/desktop/src/`: `desktop-application.tsx` owns navigation/refresh; `dashboard/desktop-dashboard.tsx` shows connectivity; `dashboard/desktop-refresh-controller.ts:108` retires reads; `desktop-messages.ts:25` calls refresh paused; `dashboard/job-policy-panel.tsx:93` shows holds. Native `apps/desktop/src-tauri/src/error.rs` separates rejected/uncertain/busy; `bridge.rs` serializes calls. Package/button sources establish React/Base UI/focus tokens. Source observations only.

SRS §§1.3, 7.3, 15.5 inform **inferred personas**: releasing queued work while a build survives; restoring admission; reconciling a headless disconnected change. No field study, simulation, timing, conversion or churn measurements.

| Alternative | Proposed journey from tray; actions | Decision |
| --- | --- | --- |
| Dashboard | Open Dashboard → switch; 2 | Select: existing destination, recovery space. |
| Settings | Dashboard → new Settings → switch; 3 | Extra navigation/surface. |
| Tray | Open tray → Governor item; 2 | Another state surface, little recovery context. |

Estimated counts. Place one switch beside daemon status; wrap at narrow widths. Preserve tokens, typography, Base UI and navigation; no Settings/tray addition.

## State rules

Accessible name: **Agent Resource Control**. Checked = `governor.enabled=true`. Authoritative text: **Active**, **Paused**, or **Governor unavailable** without current evidence. Initial: **Checking Governor…**. Label connectivity/refresh separately: daemon connection cannot establish Governor state; hidden-window refresh never pauses Governor.

Separate confirmed value/revision/receipt time from requested value/identity/base revision. Checked follows confirmed state; applying text shows intent. Absent saved state defaults ON/revision 0 in daemon, never client guess. Saved OFF survives reopening/restart.

One application-level mutation owner outlives About and retired refresh generations. Use bridge serialization; coalesce activation during write/readback. Hiding/abandoning responses cannot undo dispatched mutations. Use Rust-derived contracts; no wire fields prescribed here.

## Deterministic recovery matrix for task-7

Synthetic prescriptions. Start Active/revision 4; assert UI, call count and accepted revision per event.

| ID / event | Display/control | Recovery/assertion |
| --- | --- | --- |
| G01 Initial pending, no value | Checking Governor…; labeled placeholder | No actionable/falsely unchecked switch or mutation. First OFF/7 renders unchecked Paused/7; no Active flash. |
| G02 Fresh ON/4 or OFF/5 | Checked Active or unchecked Paused; operable | Only validated Governor data establishes state; health/refresh cannot change it. |
| G03 Pause; delayed writer | Applying pause…; checked, last confirmed Active; activation blocked, focus retained | One OFF mutation, stable identity/base revision. No success before committed ACK. Resume: Applying resume… symmetrically. |
| G04 Committed ACK OFF/5 | Unchecked Paused/5; operable | Clear pending once. Duplicate delivery reuses identity; no repeat transition. No-op retains revision. |
| G05 Confirmed pause rejection | Checked Active; “Could not pause Governor.” plus safe reason | Restore prior confirmed state unless newer authoritative evidence supersedes it. Explicit retry; no automatic overwrite. |
| G06 Lost ACK/timeout/disconnect | “Change not confirmed. Checking Governor…”; retained value, activation blocked | Start readback when transport permits; retain identity. No success/rollback claim. Generic busy/cancellation/timeout cannot prove rejection. |
| G07 Readback after G06 | Current state; request unconfirmed while unresolved | Old ON/4 cannot settle delayed write. State reads confirm present state, not request outcome. Reconcile before settlement; failed readback uses G10. Reconnect reads back, never blindly resends. |
| G08 Cross-client conflict | “Governor changed elsewhere. Review current state and try again.” | Read latest revision; then allow new explicit intent. Never automatically rebase/replay obsolete intent. |
| G09 Reordered/invalid reply | Retain newest valid state; invalid evidence becomes unavailable | OFF/5 → ON/6 → late OFF/5 stays Active/6. Lower revision cannot regress; equal revision/different value triggers readback. |
| G10 Initial failure/stale/unsupported | Governor unavailable; retained “Last confirmed: Active/Paused · {check time}” | Unknown: placeholder. Retained switch keeps value, activation blocked until fresh evidence. Never guess ON/OFF. |
| G11 About/hide during G03/G06; reopen | Retain intent/state in owner; silence while hidden | Completion survives retired read generations. Reopen reads before enabling; stale reads cannot replace newer ACK. No extra mutation/default reset. |
| G12 Reconnect after CLI change | Checking/readback → fresh Active/Paused | Compare all retained revisions; preserve unresolved writes. Health reconnection cannot settle a mutation. |
| G13 Backlog/response cancellation | Applying → G04/G05/G06 | Distinguish proven pre-dispatch rejection from unknown outcome. Cancelling a wait cannot undo committed effects. |

Keep revisions in diagnostics; use actual desktop check times, not invented daemon times.

## Work explanations and accessible operation

Fresh Paused supersedes historical memory-hold/recovery copy: **“Scheduling paused. Waiting jobs can proceed without resubmitting. Running jobs and usage measurement continue.”** Scope to supported local jobs; retain old holds as history. Keep actual Meter readings, Critical and freshness visible. Paused proves neither memory recovery nor child launch. Stale/unknown Governor evidence means current admission unconfirmed, not a live hold.

Active help: **“Scheduling enabled. Jobs start when admission rules allow.”** Resume restores window/fresh-Critical rules, not immediate starts. No rerun or live counts inferred from history. Native owners prove exactly-once queued/reserved execution and child survival.

Use a semantic switch with known boolean state. Tab focuses; Space/Enter each request one change through Base UI; pointer/label is equivalent. Retain visible focus through pending/rejection/readback; `aria-disabled=true` blocks activation with a reason while retaining focus. Associate help/errors via `aria-describedby`, applying region via `aria-busy`. Unknown placeholders never imply OFF.

Use words beyond color and existing focus/contrast tokens. One polite region announces transitions once, including rejection/uncertainty; no polling repeats, competing alerts, focus jumps or hidden announcements. Help is not hover-only. Preserve wrapping, zoom/reduced motion. Contrast/keyboard/VoiceOver checks unrun.

## Retention decision and handoff checks

Prominent Resume plus dismissible reminder per paused session is an **unmeasured nagging hypothesis**: more resumes, possibly more accidents/annoyance/churn. Select neutral feedback, equal pause/resume and durable OFF. Defer reminders until resumed-job conversion improves without worse pause success/time, accidents or reuse/churn. Default ON/ON time proves no retention.

Skill: `oma-design`, local route (`resources/external-design-skills.md`); no supplier. Serena unavailable; named-file reads. Source/scope/size/Git checks only. No code edits, tests, gates, screen/browser checks or studies. Task-7 verifies behavior; QA reviews implementation.
