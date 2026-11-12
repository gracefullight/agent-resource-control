# Task 5 handoff restoration — attempt 3

Status: report delivery restored. The document/code assessment is preserved below;
independent assessment of task-3 command execution remains **partial (E-01)**
because the backend receipts are unavailable. No product acceptance is claimed.

Owned changed path: `.arc-verify/memory-pressure/advice/design.md`.
Mission: `847d99d4-37bc-4c0d-9593-d56183093082`. Actor: design-director.
The operator note dated `2026-11-12T10:47:19.314Z` replaces the ignored handoff
path with this Git-visible path and permits this advisory file in addition to
the five canonical evidence files. The prior assessment is not repeated.
Its original attempt number, old output path, command history, inspected
revision and receipt claims below are historical scope, not this attempt's
execution or current committed-baseline claims.

## Restoration evidence

- Restored source: `target/memory-pressure/advice/design.md`, 28610 UTF-8 bytes.
- Original report SHA-256: `eda9eca8fb80fec82bcd85273b115c1a457c7743bec89388a03b0ce4f09980b0`.
- The complete original report after the preservation marker is byte-for-byte
  unchanged. Only this restoration cover is new. No report was deleted.
- Accepted committed baseline: `338631893b51b2e7cd4045e636bb1301ab4db955`;
  its verified product tree: `cb589a2dbb538f7d273e291858d939b00ffa62f7`.
- This repair checkout's actual HEAD/input snapshot:
  `0a8df64b40320a7d6a3062d486cb50c6ef6d681e`;
  verified input tree: `21fc8c5c0c2dd43bac18e8615b06b0e3d67f24c4`.
- The supplied snapshot ref resolves to that snapshot in the source workspace.
  The supplied worker ref is absent there at this inspection; no worker revision
  is inferred. These refs are not exposed in this child checkout; the initial
  combined ref command exited 128 before the bounded parent-ref read.
- All nine source/test/contract SHA-256 bindings in the preserved report match
  this repair checkout. That confirms retained finding inputs, not execution.
  Product changes in this snapshot remain distinct from the accepted baseline.

## Checks executed in this repair

| Action | Result |
| --- | --- |
| Scoped `git status --short` and `git diff --quiet 0a8df64b40320a7d6a3062d486cb50c6ef6d681e --` over the evidence and product paths | Empty status and exit 0 before restoration; no product edit. Unscoped status reported denied `av.yaml`, which was not accessed through another mechanism. |
| `git rev-parse` for current HEAD/tree and the accepted baseline tree; bounded source-workspace `git for-each-ref` for the supplied Actor refs | Current snapshot/baseline identities verified as above; worker ref unavailable. |
| Python standard-library reads, SHA-256 comparisons and exact-path existence checks | Original report intact; nine binding matches. Task-3 attempt-2 `report.md` and `receipt.json` remain absent in both Actor and source workspaces. Canonical `gates.json` and `integration.json` are also absent at this inspection. |
| `git check-ignore --no-index` for old/new report paths | Only the old `target/` path is ignored. |
| UTF-8 readback, preserved-body equality, report-size check, `git diff --check` and scoped stage inspection | Used to verify this handoff; final byte count and SHA-256 accompany delivery. |
| First report-marker validation | Failed on the review script's case-sensitive `Fallback` marker; the report uses lowercase `fallback`. Corrected the marker comparison without changing the assessment. |

Used `oma-design` with its local semantic-contract/document-restoration route,
execution policy, result contract, supplier-routing reference and checklist.
No new visual judgment or supplier download was needed. Serena is still absent
from the available tools. No implementation, test, contract, manifest, mission
state, historical evidence or design-system change was made. No build, gate,
test, GUI, browser, screen or assistive-technology check was run.

## Chief / task-6 handoff

D-01 and D-02 below remain actionable source-based findings for backend-owned
repair: qualify retained stale/disconnected disclosure fields, and account for
elapsed read time before CLI copy asserts a current hold. C-01 remains an
advisory deterministic focus-test precision gap. These are preserved findings,
not newly measured defects or user reactions. The inline Dashboard/job-detail
journey, visual system, cancellation and automatic recovery remain the intended
interface. No loss-framed or dark-pattern experiment is added.

E-01 remains an unmet receipt dependency: restore backend-recorded task-3
results with explicit input bindings and referenced output, or provide
backend-recorded replacement results. A later task must receive any required
handoff through Git-visible files under the operator note; ignored raw logs
alone cannot supply cross-workspace evidence. Existing canonical gate/integration
reports may carry compact command outcomes, input/log bindings and relevant
output without adding another advisory file. Do not substitute foundation
results or source hashes for consumer execution. The Design Director has not
rerun gates to fill that gap. No operator choice or new approval is requested.

The path-restoration requirement is complete. Required independent task-3
execution-receipt assessment remains partial until the backend dependency is
available. Production acceptance and final gates remain task 6's responsibility.
The preserved report maps hold, fallback, stale, disconnected, recovery and
cancellation to delivered code/test assertions and states its verification limits.

---

<!-- Preserved original assessment begins; following bytes are unchanged. -->

# Task 5: memory-admission journey assessment

Status: completed document/code assessment with two semantic defects handed to task 6. This is advisory review evidence, not mission, gate, rendered-interface or accessibility acceptance.

Actor: design-director. Mission: `847d99d4-37bc-4c0d-9593-d56183093082`. Task: `task-5`, attempt 1. Owned output: `target/memory-pressure/advice/design.md`. Only this report was written. Product source, tests, shared contracts, manifests, mission state and canonical `.arc-verify/memory-pressure/` evidence were not changed.

The assigned report lives under ignored `target/`; it is explicitly staged as the sole file deliverable so workspace collection can retain it. No commit or external delivery was made.

## Inspected revision and delivery provenance

| Scope | Actual revision or evidence |
| --- | --- |
| Accepted committed source baseline | `338631893b51b2e7cd4045e636bb1301ab4db955` |
| Product tree of that exact baseline | `cb589a2dbb538f7d273e291858d939b00ffa62f7` |
| This Actor's inspected snapshot/HEAD | `3c36defe6ef30f05076ca7602dd9f20003db2f5b` |
| This Actor's input tree | `3f3f2f45513aca54d4b2f2d98c751e50267d8942` |
| This Actor's branch | `av-task/6c542d07702898e2-e7c08de777760acd-1` |
| Task-3 attempt-1 snapshot and worker HEAD | `d4d6ccb390a4e308932d64a010b2817f81db2c28` |
| Task-3 attempt-1 snapshot tree / delivered tree | `016e9d069be80292fdc1223bd88a4179cd38697c` / `0410fe625739d8954aa7b3ee44a1f0e12572db17` |
| Task-3 attempt-2 snapshot and worker HEAD | `5cc2978d2a2cef721d3f330baff0f0580a052321` |
| Task-3 attempt-2 snapshot tree / delivered tree | `c2f8b544c91974b3649f839d23b068ef889b4f4c` / `e8e739fde9056f065755251a3b9296fce2b605ea` |

All implementation observations below concern the delivered input in this Actor's snapshot, at the paths and line numbers cited. They do not describe changes already committed to the accepted baseline. The earlier task-3 worker commits omit their uncommitted delivery patches; this review checked the delivery blobs rather than treating those commits as the delivered implementation.

The source workspace supplied by the assignment is:

```text
/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/847d99d4-37bc-4c0d-9593-d56183093082/workspace
```

In that source workspace, both task-3 deliveries remain under `.agent-valley/parallel/deliveries/6c542d07702898e2-1724980aa084bbaa-{1,2}/delivery.json`. Their recorded status is `applied`. Both patch SHA-256 values match the actual patch bytes. Resolving successive changes by path gives 40 delivered paths; all 40 current `git hash-object` results match the latest delivery blob OID for that path. The two task-3 snapshot and worker refs resolve in the source workspace to the commits above. This Actor's snapshot ref resolves there to `3c36def...`; its worker ref is not available yet. No worker revision was invented.

Delivery JSON SHA-256 values:

- Attempt 1: `d36e7177a13338676b2c3588346e1b327fd920738e8e60f4c9fcd482d4f8d31e`.
- Attempt 2: `259796fd9fa996c5fdc29e2b901b38a7b0ee8230d7c5e33e00a047f54a5a3db0`.

Initial scoped Git status was empty. Unscoped status reported `av.yaml: Operation not permitted`; that path was excluded from subsequent status reads and was not accessed through another mechanism. Consequently, a fully unqualified clean-worktree claim is not made.

## Method and design decision

Used `oma-design`, read from `.agents/skills/oma-design/SKILL.md`, with its execution protocol, supplier routing, preservation protocol and affected-result checklist. Route: local OMA semantic-contract assessment. The assignment freezes the inline journey and excludes visual/GUI judgment; no external design supplier, new design system or visual-direction change was needed. The separate Interface Design craft/render review was not performed. Frontend rules and the shared execution/code-intelligence policies were read as context, without invoking an implementation workflow.

Serena tools were unavailable in the active tool inventory. Review used scoped reads of supplied paths and their imports, Git metadata and ignored delivery artifacts. Native recursive search was confined to ignored scratch paths; there was no substitute native project-source search. No agent was spawned. No test, gate, compilation, build, browser, GUI, screen or assistive-technology session was run.

The incumbent desktop is React 19 / Tauri / Bun / Tailwind, confirmed in `apps/desktop/package.json`. The delivered panel reuses existing border, background, muted-text and focus-ring tokens. English messages follow the existing desktop fallback locale. No palette, font, navigation, component stack or language policy change is proposed. No root `DESIGN.md`, `.design-context.md` or `.interface-design/system.md` is tracked at the reviewed snapshot.

Continue with the inline Dashboard/job-detail journey. `desktop-dashboard.tsx:111–120` already places Meter beside recent jobs and selected detail; `desktop-refresh-controller.ts:281–282` selects the newest job only when no selection exists. `selected-job-panel.tsx:75–80` places memory admission before recorded outcome. These are observed structural facts, not measured discoverability or task time.

| Option | Structural journey and tradeoff | Assessment for this slice |
| --- | --- | --- |
| Inline Dashboard plus selected detail | Open Dashboard and read the current selection; select another existing row when needed. The reason and recorded outcome share the existing context. | Retain. Shortest structural path supported by the delivered product. No measured time advantage is claimed. |
| Separate Traffic screen | Navigate away to compare a queue, then correlate the job and return for its outcome. A useful future queue comparison would need additional bounded data/contracts. | Adds navigation for the immediate single-job question. No new screen is justified by current evidence. |
| Tray interruption | Notice an alert, open a destination and correlate its state with the current policy. Adds an interruption and another potentially stale surface. | No supplied user data establishes demand or useful alert frequency. Keep this outside this slice. |

SRS §1.3 lists developers using multiple coding agents/worktrees and costly local workloads. The following personas are hypotheses derived from that audience, not recruited participants: a first-use Mac developer asking why Cargo/Bun has not started; a returning developer checking recovery; and a headless maintainer distinguishing an outage from a hold. Their proposed successful journey is to identify the reason, wait for automatic recovery or cancel once, and avoid duplicate launch. Time to understand, mistaken rerun intent, reactions, retention, churn, conversion and revenue are unmeasured. No loss-framed copy or dark-pattern experiment is proposed for this slice.

## State-to-code and deterministic-test map

Paths in this section are relative to the reviewed snapshot. Test descriptions below describe assertions inspected in source. Task-3 execution counts are dependency-reported because its raw test receipts are unavailable here; they are not tests run by this review.

| Required state | Actual delivered presentation and source | Inspected test evidence and boundary |
| --- | --- | --- |
| Confirmed fresh Critical pressure hold | `job-policy-panel.tsx:10–27,87–107` requires current `memory_hold`, a recorded `memory_pressure` reason, and no stale/error state before displaying “Waiting for memory” with automatic-recovery copy. Running/terminal detail suppresses a job-specific hold through `selected-job-panel.tsx:75–79`. | `policy-interaction-cases.ts:75–111` installs a synthetic queued job and validates headline/recovery copy. `policy-read-cases.ts:14–77` covers delayed reads. This is synthetic presentation, not native Critical evidence. |
| Current L0 with missing/failed/denied/unsupported/invalid reading | `policy-messages.ts:18–29` names each availability reason; `policyAnnouncement` combines L0 with that reason. “Memory reading not supported” is distinct from a daemon lacking the policy capability. | `policy-read-cases.ts:232–264` supplies null pressure/age and checks L0 without invented Normal. `policy-contract-cases.ts` validates shape and reason consistency. |
| Authoritative stale Critical → L0 fallback | A new daemon result can be current policy `l0` / `observation_stale` while its underlying pressure value remains stale Critical. This differs from merely expiring an old desktop reply. `PolicyReadState` accepts that explicit fallback; the panel labels an older memory wait historical. | `policy-read-cases.ts:96–132` checks fresh at 3000 ms, stale at 3001 ms, then accepted L0/stale policy while recorded decision age stays 0. `policy-interaction-cases.ts:143–174` checks explicit stale fallback and retained outcome. |
| Locally stale retained Critical, with no new authoritative fallback | `policy-read-state.ts:77–107` ages the field and marks the policy stale after the threshold, receipt age of 10000 ms or invalid clock. `policyAnnouncement` says current admission is unconfirmed; it does not fabricate L0. | `policy-read-cases.ts:96–178` checks unchanged fields, duplicate receipts, new host sequences and failed fallback reads cannot rejuvenate pressure. `policy-interaction-cases.ts:112–141` checks the stale headline/live text. Expanded details remain misleading: D-01 below. |
| Disconnected with retained data | `desktop-refresh-controller.ts:233–269` retires policy on failed health reads. `policy-read-state.ts:68–75,215–224` retains value separately from error/staleness. Headline says last known policy retained and current admission unconfirmed. | `policy-interaction-cases.ts:143–174` retains independent recorded success after disconnect; `policy-read-cases.ts:214–230` tests retirement. Expanded details need D-01. |
| Initial unavailable versus unsupported capability | With no accepted value, `job-policy-panel.tsx:14–16` uses unknown, not retained. Explicit capability unsupported gets its own message. `policy-read-state.ts` does not turn malformed/failed reads into unsupported. | Repaired `policy-interaction-cases.ts:46–74` checks initial unknown, later L0, then retained failure. `policy-contract-cases.ts:119–160`, Rust `consumer_reads.rs:111–149` and `registered_policy.rs:193–220` distinguish capability errors. |
| Current policy versus last job decision | `policy_job` returns one correlated result; the panel uses `result.effective` through `PolicyReadState`, not a separately refreshed Meter reading. Last decision retains its original timestamp, version and observation age in the disclosure (`job-policy-panel.tsx:124–133`). Normal/L0 does not rewrite an old memory wait into an admission. | `policy-read-cases.ts:14–77,96–132` keeps recorded age fixed. `consumer_reads.rs:72–108` and `registered_policy.rs:132–170` preserve decision metadata through production bridge/router code with socket fixtures. |
| Automatic recovery and established start/outcome | When fresh Normal arrives, the panel removes current waiting/recovery instructions and labels any old wait historical. `exec/traffic.rs:274–385` continues polling the same job; `exec/launch.rs:71–141` waits for Held→Ready. `exec.rs:207–214,365–375` emits Started after native spawn succeeds. | `policy-interaction-cases.ts:75–111` simulates recovery and later independent success. `exec/launch/tests.rs:60–139` contains a socket-fixture recovery test with one target marker and exit 17. Neither establishes production-daemon acceptance for task 6. |
| Running, reused and completed work | The panel avoids telling an already running/finished selected job that it will start automatically. Its global policy headline may still describe restrictions on new jobs. `job.rs:243–246` limits job-specific CLI waiting to Queued/Reserved; launch Reused preserves the existing reservation. | Presentation test keeps recorded success independent of stale/disconnected policy. Launch fixture assertions concern the client seam; task 6 still owns actual running-child/nested outcomes. |
| Cancellation while queued/held | Existing route remains the launching terminal's Ctrl-C. `exec/traffic.rs:339–343,372–375,412–438` keeps cancellation and reports unconfirmed cleanup. `exec/launch.rs:71–75,138` returns Cancelled; `exec.rs:184–204` finishes the unspawned witness and cancels. Desktop remains an inspection surface. | `exec/traffic/admission_tests.rs:28–42` tests cancellation during busy admission; `exec/launch/tests.rs:194–221` tests cancellation after Held without outage fallback. These use deterministic cancellation callbacks, not a real-user Ctrl-C trial or production-daemon cancellation reconciliation. |
| CLI current/fallback/history view | `job/policy.rs:28–50` displays policy version/profile/mode/reason/availability/age and a separately labeled last decision. `exec/traffic.rs:323–335,401–409` reports semantic queue notices; `exec/policy.rs:45–50` identifies confirmed pressure. | `tests/memory_pressure/status_reads.rs:129–152` checks current Critical versus stale L0, historical age 5, old-daemon unsupported and malformed reply. It lacks an in-flight observation-age boundary: D-02 below. |

## Focus, disclosure and status announcements

Observed source: `recent-jobs-list.tsx:31–72,149–189` keeps focus separate from selection, uses stable job keys and roving tab stops, supports arrows/Home/End plus Enter/Space, and restores focus only on navigation return or removal of the previously focused row. `PolicyReadState.select()` clears the old job projection, and controller generation checks discard late replies for a prior selection. No policy update calls focus. This supports preservation of the established interaction model; it is not rendered keyboard verification.

`job-policy-panel.tsx:114–135` uses native `details`/`summary`, an explicit summary label and existing focus-ring tokens. Policy/version/profile, availability, age, sampler identity, sequence and recorded decision metadata remain in a disclosure rather than primary task instructions. Inspecting native semantics supports an accessible implementation approach. Contrast, visible focus appearance, keyboard operation in the macOS webview and actual screen-reader announcement behavior remain unverified.

`selected-job-panel.tsx:150–164` uses the existing polite status region. `policyAnnouncement` excludes exact ages, sequence numbers and receipt timestamps, so routine refreshes need not rewrite the live message. `policy-interaction-cases.ts:112–141` inspects a MutationObserver and asserts no changes on equivalent polls, with a semantic change on expiry; `dashboard.test.ts:563–625` also checks stable outcome announcements. These are deterministic DOM assertions, not an assistive-technology test. A hold→stale, hold→fallback or disconnected transition should remain meaningful; no ETA, progress percentage or invented success is added.

Coverage note C-01: the test named “preserve keyboard focus/outcome” (`policy-interaction-cases.ts:75–111`) checks row focus before clicking the disclosure, then checks disclosure openness and row selection after recovery. It does not assert the focused element after recovery or operate the summary with Tab/Enter/Space. This is a verification gap, not evidence of a focus defect. Task 6 can strengthen that existing case with explicit keyboard activation and active-element assertions through hold, stale, fallback and recovery. Do not add a new screen review or acceptance gate for this note.

## Concrete findings for task 6, backend-owned repair

### D-01 — P2: stale/disconnected disclosure still presents a fresh effective hold

Observed static defect in `apps/desktop/src/dashboard/job-policy-panel.tsx:109–123`, using `policy-messages.ts:19–21`.

Deterministic trace, not executed here: accept the valid Critical fixture and expand its details; advance beyond its 3000 ms freshness limit with no new reading. `PolicyReadState` sets `section.stale=true` but correctly retains the original reply. The headline becomes “Stale policy · current admission is unconfirmed,” while the same visible disclosure still prints “Effective mode: memory_hold” and “Effective reason: Fresh memory reading: Critical.” `Observation availability` still says `available` because that is the retained wire field. A disconnect or explicit unsupported-capability error after an earlier successful read similarly retains these unqualified detail labels. The top-level caution does not qualify the adjacent detail fields themselves.

Effect: two parts of one panel give incompatible temporal descriptions. Confusion or duplicate-launch intent is a hypothesis; no user incident is claimed. This is a presentation finding and does not establish an admission bypass.

Repair handoff: keep the retained bytes/history; qualify this group as “Last received policy” when stale or errored, explicitly label its pressure/availability as retained, and avoid calling that reading currently fresh. Keep the current headline truthful. Do not synthesize L0 from local expiry; show L0 only after an authoritative fallback result. Keep last recorded job decision and its original age separate. Preserve tokens, summary expansion, focus and the independent recorded outcome.

Backend verification target: extend the existing Dashboard fixture cases with an open disclosure at exactly 3000/3001 ms, followed by disconnected and unsupported responses after a successful Critical read. Assert unconfirmed/retained qualifiers throughout the disclosure, no active fresh-Critical claim, no automatic-start instruction while unconfirmed, stable focus/selection and unchanged recorded decision age/outcome. These checks were not run by this reviewer.

### D-02 — P2: CLI pressure display does not age the observation across its read

Observed static gap in `crates/arc-cli/src/exec/policy.rs:13–43,45–50` and `crates/arc-cli/src/job/policy.rs:9–37`; corroborating transport/decoder paths are `exec/transport.rs:34–53` and `arc-ipc/src/policy.rs:100–111`.

`policy::job` returns a decoded wire result without a monotonic elapsed-read adjustment. `confirmed_hold` checks mode and last decision but no local freshness. The once-off status read permits two seconds; the queue-display probe permits 150 ms (`exec/traffic.rs:325–330`). Both can receive a valid result computed before a delay.

Deterministic trace, not executed here: the daemon computes Critical with `age_ms=2500`, `stale_after_ms=3000`, and a recorded wait; delivery takes another 1000 ms within the status deadline. The decoder still accepts the wire age and `render` prints a current memory hold / “Wait reason: Waiting for memory” with observation age 2500, although the field is now at least 3500 ms old. The desktop's repaired `PolicyReadState.accept` handles precisely this kind of elapsed-read uncertainty; CLI display does not. A similar boundary exists within the shorter notice deadline near expiry.

Effect: the headless user can receive a current-hold statement from an observation that aged out during the read. This finding concerns display freshness; final launch has its own fence and is not authorized by this display helper.

Repair handoff: carry monotonic elapsed-read information into the CLI display projection; conservatively account for read/transport time before asserting a current hold. When freshness is unconfirmed, display stale/last-received policy and retain the historical decision without claiming recovery or a newly selected L0. Preserve raw recorded decision metadata, authenticated bounds, launch coordination, cancellation and old-daemon behavior.

Backend verification target: use a controllable clock or bounded socket-fixture barrier for fresh-at-boundary and expired-during-read cases, including a short notice probe. Check no confirmed-current wait wording after expiry, unchanged historical age, a genuine fresh recovery, and an authoritative stale→L0 result. Keep these in the existing CLI unit/integration paths. No new timing experiment or test was run here.

### E-01 — receipt availability limits independent execution claims

The dependency reports that task-3 attempt 2 passed 84 Rust tests, 114 Bun tests, formatting, scoped Clippy and desktop type/style/build/Tauri checks. Its independent technical-review summary reports matching receipt bindings. Those supplied summaries were read as reported evidence.

However, `target/memory-pressure/task-3/attempt-2/report.md` is absent from this Actor checkout, the supplied source workspace and the former task-3 workspace location. The source workspace's ignored parallel-workspace directory retains this Actor and another current checkout, not the former task-3 checkout. The retained task-3 deliveries contain `delivery.json`, `workspace.json` and `changes.patch`, without raw command receipts/logs. The code/delivery bindings are verified above; they do not prove command execution. This review cannot independently confirm the 84/114 counts or their input/receipt bindings.

Chief/task-6 handoff: preserve or recover the backend-owned raw receipt/log artifact dependency if available, and bind final affected-interface results to task 6's actual integrated inputs. Do not ask the operator to choose a recovery method, invent successful commands, substitute task-1 foundation results for task-3 behavior, or rerun native gates from the Design Director role. E-01 limits this report's verification claim; it does not negate the dependency's reported successful review.

## Receipt inspection and commands actually executed

Available current-mission task-1 evidence was inspected at the supplied source workspace. `consumer-foundations.json` records exit 0 for `cargo test --locked --offline -p arc-cli -p arc-daemon -p arc-desktop --test memory_pressure`; it predates consumer implementation and contains foundation checks only. `contracts-verification-retry.json` records exit 0 for the focused policy/IPC/storage command. All four referenced stdout/stderr byte lengths and SHA-256 values match. These checks establish receipt readability/integrity within that scope; they are not fresh passes for this snapshot.

| Actual review action/command | Outcome |
| --- | --- |
| `pwd`; `git rev-parse HEAD 'HEAD^{tree}' 338631893b51b2e7cd4045e636bb1301ab4db955 '338631893b51b2e7cd4045e636bb1301ab4db955^{tree}'` | Reviewed Actor snapshot and accepted baseline/tree resolved as above. |
| `git status --short` | Reported denied `av.yaml`; no claim of full unscoped inspection. |
| `git status --short -- . ':!av.yaml'` | Empty before report creation. |
| `git diff --name-status 338631893b51b2e7cd4045e636bb1301ab4db955 HEAD -- . ':!av.yaml'` | Located integrated input changes; did not treat them as accepted-baseline content. |
| `git rev-parse` of supplied snapshot/worker refs; `git for-each-ref --format='%(refname) %(objectname)' refs/agent-valley/task-evidence/` in source workspace | Task-3 refs and this Actor's snapshot resolve; this Actor's worker ref unavailable. Child checkout does not expose these parent refs. Failed combined ref reads exited 128; bounded source-workspace reads recovered the available provenance. |
| Scoped `cat`, `nl -ba` and `sed -n` reads of the listed contract, UI, CLI, bridge and test paths | Produced the state map and code findings. No code execution. |
| `cat target/memory-pressure/task-3/attempt-2/report.md` in Actor/source workspaces, plus exact former task-3 path | Failed: no such file. Bounded ignored-directory inspection confirmed the raw dependency artifacts were not retained there. |
| `git check-ignore target .agent-valley` in source workspace | Both scratch locations confirmed ignored before native scratch-artifact discovery. |
| Python standard-library reads plus `git hash-object -- <delivered-path>` | Both retained delivery patches match their recorded SHA-256; all 40 latest task-3 delivered path blobs match this snapshot. |
| Python standard-library receipt/log hashing | Two task-1 receipts read; four log digest/byte bindings match. Task-3 raw logs remain unavailable. |
| `git ls-files DESIGN.md .design-context.md .interface-design/system.md` | No tracked design-context artifact at these paths. |
| `apply_patch` for the assigned report, followed by UTF-8/readback, SHA-256 and scoped Git-status inspection | The report is the sole intended output; final artifact digest is provided with delivery. |
| `git add -f -- target/memory-pressure/advice/design.md`; final staged-path inspection | Only the assigned ignored report is staged for collection. Product and canonical-evidence paths remain unchanged. |

The initial YAML config read also reported `.agents/oma-config.yaml` absent. The existing CUE config and explicit project instructions supplied sufficient context; no config repair was attempted. Read-only commands were sometimes grouped, so a missing optional path or unavailable ref can make that grouped command nonzero despite successful preceding reads. No failed tool prerequisite was bypassed.

No Cargo/Bun test, lint, typecheck, build, native-source smoke, coverage, architecture harness, Playwright, screen check or GUI command was executed by this review. No installed daemon was started/restarted, no external delivery occurred and no historical mission was recertified.

## Source bindings for actionable findings

These are per-file evidence bindings, not a second mission input digest. The canonical final mission digest remains owned by task 6 in `gates.json.inputs`.

| Reviewed path | SHA-256 |
| --- | --- |
| `.arc-verify/memory-pressure/contract.md` | `08c0b81c0f2a506b9a9265daebb729a6177b3cb3be0c0667a6bc9619939c0f19` |
| `apps/desktop/src/dashboard/job-policy-panel.tsx` | `1cd0d0be90a01edea5606b8675b4acfc040b4a36702a6d287a73118ac164aaf9` |
| `apps/desktop/src/dashboard/policy-messages.ts` | `c84582bf0eeac8b7affd04a7585441fc3163862bb601d14547addadc61199445` |
| `apps/desktop/src/dashboard/policy-read-state.ts` | `722ba0d268bac5a8edc7d7bb3ecf3862a22504d659fbc3ae8f908f54111eb99e` |
| `crates/arc-cli/src/exec/policy.rs` | `78fba5986db6e3fa16a9e42d1fb599faa9a7f27b9fb788d554a9d61149968c64` |
| `crates/arc-cli/src/job/policy.rs` | `e5439326a1179f91e36a40431dfc82207a5b6699bcfeba02e861d3816e6a1ae7` |
| `apps/desktop/tests/fixtures/policy-interaction-cases.ts` | `fd680a5ab8428c4e7b05c28c508e5d4a60c0294f9521a4f6cc21387b58b29066` |
| `apps/desktop/tests/fixtures/policy-read-cases.ts` | `d67dd3ecf41229a2b497e776dc05470941c9d475f12b5bc31c61df37be8c5ed8` |
| `crates/arc-cli/tests/memory_pressure/status_reads.rs` | `3ca69a7f8f732d9f84c6f689a3fbb7b35866e0dcc4643d37adef53cc5b52ba72` |

## Acceptance and remaining work

- [x] Hold, fallback, stale, disconnected, recovery and cancellation map to actual delivered source and deterministic test assertions, with execution-evidence limits explicit.
- [x] Inline journey assessed against Traffic-screen and tray alternatives using existing structure; visual system, automatic recovery, cancellation and scope preserved.
- [x] Concrete temporal-copy/freshness defects include paths, causal traces, proposed backend-owned correction and existing-suite verification targets. No user-test outcome or measured reaction invented.
- [x] Focus/selection, disclosure and semantic announcement behavior assessed from code/tests; rendered and assistive-technology checks explicitly unrun.
- [x] Report provides task-6 handoff and artifact/source provenance. No implementation or mission-state edits.

Task 6 owns D-01/D-02 repair, the E-01 receipt handoff/reconciliation and all actual integrated tests/gates. C-01 is an advisory test-precision note. This report does not require the product to be defect-free to complete task 5, and adds no gate or permission step.

Production-daemon Normal→Critical→Normal reconciliation, zero new permits/starts throughout Critical, unchanged running outcome, exactly-once recovery, full edge-case coverage, a separate SystemSource smoke, six final gates, per-crate coverage and affected desktop checks remain backend integration acceptance. Native Critical, usability/time-to-understand, retention and commercial effects are not established by synthetic fixtures or this report. Outstanding product scope remains CPU/disk pressure control, statistical learning, vendor hooks, native Windows and full policy acceptance.
