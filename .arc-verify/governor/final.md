# Governor slice only — Chief Director final report

Status: **task-12 completed; first macOS Governor pause/resume slice delivered in the mission worktree**. Date: 2026-11-13. Actor: chief-director. Skill used: `oma-pm`. Only this report changes in task-12.

The scoped implementation has passing final-input gates and independent read-only QA **PASS**, with no unresolved Governor-slice blocker. The Chief reconciled the existing executable evidence without running native gates. This is **Governor slice only**; the framework's subsequent final verification/review is not claimed as already run. The original goalBrief, success criteria, verification commands, task assignments and task-8's completed effects remain unchanged.

## Measured results

### Final source and evidence identity

| Item | Exact identity |
| --- | --- |
| Accepted/current HEAD | `08e7e526cce7e03ca9264afc926ed2e878dfde6d` |
| Accepted tree | `d3cbd610653d0b78adeb2ffc26a9cab1790e877f` |
| Final maintained input SHA-256 | `e0a3d075c65e767a0959617129527a0252c8a3d2e8cc42aeb6deee5497b46a08` |
| Input inventory | 632 files in `gates.json#/inputs/manifest`; decoded hash/bytes, sorted-map digest, current worktree bytes and staged Git blobs all reconcile. |
| Separately bound traceability document | `2b3ab0b61b01aa6e3483ebd8641959289a54d651bbc9c673b9605e8e790b76db`; current/index bytes match. |
| contract.md SHA-256 | `4832ba276b4d1a1b05b65ee4a23acede7afce45b2cc3773bc88a4b03c76a30df` |
| integration.json SHA-256 | `0b07bf9876db923d19159b0e8b211f4ba0a5bd75989846a052d5a421d10c8877` |
| gates.json SHA-256 | `86a16984a31c1ed1be4d42ceac41f756e1462ab5c5ea996eaa2cda648cbf6917` |
| qa.json SHA-256 | `476ba2841949099a0e2dda068ae17f6ae9873bb258c3b99a534f03fe3be9862a` |

Workspace: `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/6dd4dd59-bec1-4e67-a1ba-65f73e0660ce/workspace`. Delivery consists of staged changes on that HEAD; no new implementation commit, push, installed-service change or release is asserted.

[Final gates](gates.json), [integration](integration.json), [QA](qa.json) and the [frozen contract](contract.md) are distinct evidence records. Generated Governor reports are excluded from collection identity; the frozen contract is included. Finalizing this report changes no compiled/tested input. QA's `precedingChiefRecord` hash `7d0e140b613c11188e77d0f501dea417c8195d0aee355f9a3af15935fc2132e2` refers to the task-4 record superseded here, not independent review of this later synthesis.

### Production execution and reconciliation

Final registered native checks pass **47 tests across three binaries**, zero failed/ignored/filtered. Workspace checks pass **1,218 tests across 68 binaries**, zero failed/ignored/filtered. Counts overlap and are not summed. Ten embedded native JSON texts retain exact original bytes and match their case hashes; executable selectors are recorded under `gates.json#/nativeAcceptance/executedTests`.

| Synchronized one-slot observation | Final-input result |
| --- | --- |
| Initial ON/revision 0 | A actually runs with its reservation; B queues with **zero** starts. |
| Pause OFF/revision 1 | B starts **once**, without a new permit. A remains alive through pause and B completion. |
| Further paused requests | C/D each start once with no permits, including explicitly injected fresh Critical; Meter and outcome tracking continue. |
| Isolated fixture restart | Same SQLite DB retains OFF/revision 1. No automatic job replay is asserted. |
| Resume ON/revision 2 | Existing one-slot queuing and fresh-Critical enforcement return. Reserved-resume cases advance generation 1 → 2. |
| Final children/outcomes | A–F each start exactly once and exit 17. A/E/F account for three RELEASED permits; **zero ACTIVE**. B/C/D mint none. |
| Authoritative reads | Direct SQLite, CLI and registered bridge agree on state/revision/timestamp: revisions **0, 1, 1, 2, 2** for initial, paused, persisted OFF, resumed and final. Child identity, outcomes and Meter reads independently reconcile. |

This uses production daemon startup/writer, actual CLI wrappers/target children and registered Tauri routing. Child markers/PIDs/native identities and persisted outcomes establish execution, not a READY result. Final observations are `integration.json#/finalGateAttempt/nativePrimarySequence` and `gates.json#/nativeAcceptance/rawJsonFiles`, particularly `GV-N02-06.json`. Task-8 observations remain historical and are not substituted for final-input proof.

All fifteen frozen obligation groups have passing mapped checks. These combine native execution with deterministic storage/writer/transport and DOM/contract tests; they are not fifteen studies or additional unique test totals.

| Case | Required behavior covered |
| --- | --- |
| GV-N01 | Same-user controls, default ON/0, durable mutation, no pre-COMMIT ACK, receipt readback after lost waiter. |
| GV-N02 | A-running/B-queued pause; B runs once unreserved and A survives. |
| GV-N03 | Subsequent paused local work bypasses admission, including injected fresh Critical. |
| GV-N04 | Actual child/Meter/outcome accounting, root/descendant ownership, released reservations and no tested permit leak. |
| GV-N05 | OFF/revision and original receipts survive controlled fixture restart. |
| GV-N06 | Resume restores window/fresh-Critical rules and fresh post-backlog observations. |
| GV-N07 | Duplicate/conflicting identity, CAS, no-op time/revision, historical replay, safe bounds, capacity/eviction. |
| GV-N08 | Real writer/SQLite backlog, read after admission, cancellation versus post-COMMIT ambiguity. |
| GV-N09 | Reserved-before-READY pause releases known-unspawned reservation and launches once. |
| GV-N10 | Pause/resume versus reserved/unreserved launch, healthy/fallback source, expiry/UNSPAWNED and fresh generation. |
| GV-N11 | Queued/converted/reserved cancellation, pre-enqueue tombstone and terminal replay cannot revive work. |
| GV-N12 | Lost mutation/launch ACK/body, dropped waiter, ambiguous DB_BUSY and witness recovery without blind retry. |
| GV-N13 | Cross-client writes, disconnect/reconnect, stale/conflicting reads and navigation during uncertainty converge. |
| GV-N14 | Strict legacy/additive protocol 1, command semantics, locality/nesting, fail-open, Meter and original migrations. |
| GV-N15 | Rust-derived TS/fixtures/compiler/runtime bounds and deterministic G01–G13 accessibility/recovery cases. |

Selectors map through `integration.json#/nativeAcceptance/cases` and `/testSelectors` to `gates.json#/checks`. QA inspected assertions under `apps/desktop/src-tauri/tests/governor/`, `tests/memory_pressure/` and registered-command tests, as well as storage, daemon, IPC and CLI regressions.

### Six mandated gates and affected desktop checks

All six exact recipes pass. The Chief audited **16 final receipts**: argv/cwd, actual exit, stdout/stderr hashes and before/after input identities agree; all postdate the binding 2026-11-12T22:31:20.410Z repair note.

| Gate | Final outcome |
| --- | --- |
| `cargo fmt --all -- --check` | Exit 0; `gates.json#/checks/fmt`. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0; `/checks/clippy`. |
| `cargo test --workspace` | Exit 0; 1,218 pass, zero failed/ignored; `/checks/workspace`. |
| `bun run test:coverage` | Exit 0; all eleven independent production floors pass; `/checks/coverage`. |
| `./scripts/harness/validate.sh` | Exit 0; validated `ARC_COVERAGE_RECEIPT` reused without collection; `/checks/harness`. |
| Explicit allowed Bun argv below | Exit 0; 221 pass, zero failures; `/checks/activeBun`. |

~~~sh
bun test apps/desktop/tests/bridge-client.test.ts apps/desktop/tests/contracts.test.ts apps/desktop/tests/credits.test.ts apps/desktop/tests/dashboard.test.ts apps/desktop/tests/lifecycle.test.ts apps/desktop/tests/meter.test.ts scripts/coverage/accounting.test.ts scripts/coverage/desktop-foundation.test.ts scripts/coverage/gate.test.ts scripts/coverage/hygiene.test.ts scripts/coverage/terminal-outcomes.test.ts scripts/coverage/traffic-accounting.test.ts scripts/harness/equivalence.test.ts scripts/harness/traffic-targets.test.ts scripts/harness/coverage-reuse.test.ts
~~~

| Additional check | Final outcome |
| --- | --- |
| `bun test apps/desktop/tests/governor.test.ts apps/desktop/tests/governor-contract.test.ts` | Exit 0; **18 passed**, zero failures. |
| `bun test scripts/requirements/traceability-parser.test.ts scripts/requirements/traceability.test.ts` | Exit 0; **56 passed**, zero failures. |
| `bun scripts/requirements/traceability.ts` | Exit 0; inventories, row fields, mappings and phase counts valid. |
| `cargo test -p arc-desktop --test governor --test memory_pressure --test registered_commands -- --nocapture` | Exit 0; **47 passed**, zero failed/ignored/filtered. |
| `cargo test -p arc-daemon --bin arc-daemon lifecycle_tests::committed_bun_grant_lost_before_old_peer_replacement_recovers_original_store -- --exact --nocapture` | Exit 0; one selected test passes; 118 unrelated tests filtered by explicit selector. Completion/cancellation branches recover original store. |
| `bun scripts/coverage/run.ts --preflight-only` | Exit 0; native/tool readiness passes, no coverage collected. |
| `bun run desktop:typecheck` | Exit 0. |
| `bun run desktop:check` | Exit 0. |
| `bun run desktop:build` | Exit 0. |
| `bun run desktop:smoke:tauri` | Exit 0; debug/no-bundle smoke, not rendered interaction or release packaging. |

Task-10 ran **alone in the integrated mission workspace**: `gates.json#/execution` records root-only Actor observation, serial intervals, no dispatches, unchanged inputs and settled owned native processes. Final native checks began 2026-11-12T23:54:58.944Z; the last selected traceability check ended 2026-11-13T00:22:42.363Z; assembly finished 00:23:30.935637Z. Environment retained `CARGO_BUILD_JOBS=2`, offline Cargo, `ARC_COVERAGE_OUTPUT_ROOT=target/governor` and **7,200,000 ms** per-check allowance. Mission limits were not reset.

### Eleven-crate production coverage and receipt reuse

Recorded tools: aarch64-apple-darwin, Bun 1.4.4, Rust 1.95.0, cargo-llvm-cov 0.6.21, LLVM 22.1.2. `gates.json#/coverage` retains numerators/denominators, native profile proof, receipt and report hashes. Independent QA recomputed each floor with the maintained read-only reuse validator.

| Crate | Covered / production lines | Percent |
| --- | ---: | ---: |
| arc-core | 612 / 625 | 97.92% |
| arc-detector | 706 / 711 | 99.30% |
| arc-storage | 3256 / 3413 | 95.40% |
| arc-ipc | 1763 / 1823 | 96.71% |
| arc-daemon | 3408 / 3760 | 90.64% |
| arc-cli | 2823 / 3236 | 87.24% |
| arc-traffic | 111 / 111 | 100.00% |
| arc-desktop | 663 / 742 | 89.35% |
| arc-platform | 464 / 502 | 92.43% |
| arc-meter | 634 / 641 | 98.91% |
| arc-policy | 202 / 210 | 96.19% |

All eleven exceed **80% independently**. There is **one successful collection for final inputs**, run `2026-11-13T00-04-54-679Z-5d22d95c-10d4-419d-a645-9109f9328f6e`, bound to the final input SHA-256 above. Instrumented workspace: 1,218 pass, zero failed/ignored; all **3,314** profiles are merge members. Harness uses `target/governor/coverage/runs/2026-11-13T00-04-54-679Z-5d22d95c-10d4-419d-a645-9109f9328f6e/reuse.json`, with validated reuse and no recollection.

An earlier collection on different inputs failed because LLVM omitted a constants-only facade. Backend-runtime moved unchanged constants into the executable wire module, preserving exports, accounting policy and floors, then repeated final checks. That failure remains history; it is not another successful collection for final inputs. Required handoff facts and native observations are embedded in tracked reports; ignored raw logs/coverage paths are supplemental diagnostics.

## Decisions

| Decision | Delivered behavior and judgment |
| --- | --- |
| One maintained authority | Reuse Rust/Tokio/rusqlite, existing bounded writer/IMMEDIATE transactions, authenticated IPC, launch coordination/witness and Tauri/React/Base UI/Bun. No competing database/configuration/cache authority. |
| Durable desired-state setter | `policy_states` holds `governor.enabled`, default ON/revision 0 only when absent. Effective changes advance a JavaScript-safe revision; no-ops retain revision/time. Stable identity, CAS and bounded 1,024-receipt retention distinguish duplicate/conflicting/uncertain writes. ACK follows COMMIT. |
| Serialized execution | Toggle, grant and final launch share coordination, including fallback sampling. Negotiated queued/known-unreleased reserved work converts once to unreserved execution; running, possibly spawned and descendant-owned work retains reservation/witness obligations. Resume restores admission and fresh generations. |
| Compatible additive boundary | `daemon.pause`, `daemon.resume` and `policy.set_switch` share the owner-authenticated setter. Capability negotiation adds current-client unreserved execution; strict legacy shapes and protocol version 1 remain. Preserve one spawn gate, command semantics, fail-open and Meter. Migrations 0001–0005 match accepted bytes; 0006 is additive. |
| Headless parity | `arcctl daemon pause`/`resume` and `arcctl daemon governor` provide desired-state control and authoritative readback with identity/CAS support. Unknown acknowledgments do not trigger inverse or blind repeated mutations. |
| Truthful, accessible UI | One Dashboard **Agent Resource Control** switch beside daemon status; Active/Paused derive from confirmed state, independently of connectivity/refresh. Initial/unavailable, last-confirmed and Applying states remain distinct. Rejection restores prior state unless superseded; uncertainty retains identity and reads back. Stale/conflicting revisions cannot silently regress state. |
| Recovery and interaction | App-level mutation ownership survives About/hide/reopen. G01–G13 assert one request, Space/Enter, retained focus, help/errors and polite announcements. Paused explanation preserves measured pressure and running-work continuity. DOM checks establish those assertions; no rendered/VoiceOver result. |
| Rust-derived TS | Pinned dev-only `ts-rs = 12.0.1` derives eight declarations and Rust wire fixtures. Normal Rust tests compare in-memory regeneration/bytes; Bun tests compiler structure and runtime bounds. Delivered `governor.generated.ts` is not a manual parallel schema. |

The [Technical Director contract](contract.md) supplied transaction/compatibility/gate boundaries; [Design's handoff](../../docs/requirements/governor-experience.md) supplied Dashboard/G01–G13; [Marketing's handoff](../../docs/requirements/governor-value.md) prioritized trust and deferred commercial hypotheses. Their initial planning status is historical; later implementation and final receipts establish delivered behavior.

Task-5 froze shared interfaces before runtime/client consumers. Backend-engineer owners used `oma-backend` with `oma-db`/`oma-frontend` support for implementation, tests, repairs and native gates. Backend-runtime owns state/runtime/shared contracts; backend-surfaces owns CLI/bridge/UI. Task-13's bounded repair and task-9's dedicated matrix feed the final gate task; task-11 independently used `oma-qa`. Chief synthesis uses `oma-pm` and changes only this file.

## Alternatives

| Alternative | Chosen tradeoff |
| --- | --- |
| Separate state file/database/writer | Reuse one transaction/writer so switch and reservation effects commit together. |
| Enqueue-only bypass, fake zero-resource permit or blanket release of unattached work | Use negotiated unreserved mode and one-shot release/witness semantics to preserve queued identity, possibly spawned children and nested ownership. |
| General RPC/schema migration or manual TypeScript mirror | Keep strict protocol 1 plus additive capability; incur only the pinned development exporter needed for Rust agreement. |
| Settings destination or tray toggle | Use existing Dashboard and recovery space. Proposed action counts/personas remain unmeasured. |
| Resume nagging, reminder or commercial launch | Defer experiments; preserve durable OFF, equal operability and truthful state. Retention, conversion and financial effects are unknown. |

## Recovery disposition and Lessons

Task-8's id, original definition, effect scope and completed integration effects were preserved. Task-13 did not replay Governor integration. Its bounded validator repair rejects false/null/failed/unexecuted and specification-only completion, enforces inventory-specific cross-references/explained unmapped values, and accepts valid selectors containing words such as `unknown` without accepting contradictory prose. It remains a format/inventory check. Final regression RED: 17 failures; GREEN: 55 passes. The historical two-file command still failed for the then-missing task-9 matrix; task-9 later delivered it, and task-10's final command passes **56/56**. No historical failure was relabeled passed.

Backend-runtime resolved the binding repair note in its assigned workspace:

- Traffic-only fixtures explicitly reject `governor.capabilities` with protocol-1 `UNSUPPORTED_CAPABILITY`, retaining original dropped-reply, cancellation, failed-spawn, owner-death, exactly-once and permit assertions.
- The Bun lost-ACK seam reaches Governor enqueue; product recovery retains uncertainty after earlier send across old-peer replacement. Completion executes **one** target, cancellation **zero**; both recover the original store with zero ACTIVE permits.
- The abandoned fixture's ACTIVE reservation became RELEASED and job became OUTCOME_UNKNOWN after original processes exited. This was pending reconciliation rather than a confirmed permanent leak; no missing exit outcome was invented.
- Captured `policy.current` returned typed `POLICY_UNAVAILABLE` while READY held the fence. Explicit HELD/QUEUED barriers and durable child-attachment ordering now synchronize reads/releases. The unavailable boundary is asserted; no sleep, retry or swallowed failed RPC hides the race.
- Explicit blocking checkpoint reads with deadlines and a target/Attach handshake repair observed macOS timing assumptions while retaining actual identity, outcome and once-only assertions. Module splits satisfy unchanged harness length checks; coverage accounting/floors remain intact.

`gates.json#/history` retains **88** prior/superseded command rows, including **18** failures, in compact command/exit/cause form. Raw receipts remain under `target/governor`. Old successes on other input identities are not final acceptance.

Lessons: committed state, acknowledgment and actual child execution are separate facts. Stable mutation identity/readback handles ambiguity; actual native child/witness evidence prevents duplicate execution claims. Launch fences also affect diagnostic policy reads, so test synchronization needs explicit handshakes. Freeze source/test/config inputs before the final collection, preserve failed attempts and deliver review facts in Git-visible files.

## Traceability and completion counts

The dedicated amendment-13 [matrix](../../docs/requirements/traceability.md) covers **165 requirements + 47 acceptance-test IDs + 104 catalog entries = 316 formal primary rows**, plus **30 toggles**. Every primary row records source, phase, status, evidence and cycle `governor-2026-11-13`; unmapped relationships have reasons. Missing/duplicate/extra formal IDs are empty. Final checks and independent QA reconcile SRS inventory, mappings, fields, toggles and arithmetic. SRS SHA-256: `a8ef945251f9f3fd7c3e337d4efc77ff9914f95114212ca9e6c90ae2ce3e3d9e`.

| Phase | Requirements complete / total | Acceptance complete / total | Catalogs complete / total | Formal complete / total | Partial | Pending |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Phase 0 | 2 / 19 | 0 / 4 | 7 / 17 | **9 / 40** | 22 | 9 |
| Phase 1 | 7 / 34 | 1 / 10 | 2 / 27 | **10 / 71** | 48 | 13 |
| Phase 2 | 0 / 24 | 0 / 7 | 0 / 12 | **0 / 43** | 5 | 38 |
| Phase 3 | 0 / 55 | 0 / 22 | 0 / 31 | **0 / 108** | 5 | 103 |
| Phase 4 | 0 / 29 | 0 / 4 | 0 / 11 | **0 / 44** | 15 | 29 |
| Phase 5 | 0 / 4 | 0 / 0 | 0 / 6 | **0 / 10** | 1 | 9 |
| Total | **9 / 165** | **1 / 47** | **9 / 104** | **19 / 316** | **96** | **201** |

Blocked rows: **0**. Pending = **26 Unverified + 175 Outstanding**. Toggles/supplemental cases do not inflate formal totals. These are current revalidated assessments, not invented promotions from a baseline without a row-level matrix. Fifteen Governor groups and thirteen experience cases are overlapping Phase 1 inventories, not 28 unique tests or completed requirements.

The full master switch including cleanup/Sweeper and `AT-GUI-02` remain Partial. Vendor coverage, GC/deletion, learning, Windows, broader tools, signing/distribution and calibration/measurements remain outstanding or unverified. Historical task-9 pre-gate statements and task-13's missing-document failure are preserved; this report supplies downstream results. No phase or full SRS is declared complete.

## Independent review and remaining risks

Task-11's `oma-qa` assessment is **PASS**: `reviewComplete`, `requiredEvidenceComplete`, `inputsMatchFinalGates` true; unresolved Governor blockers/findings empty, zero Critical/High/Medium/Low findings. It inspected source/assertions/contracts, sixteen final receipts, native observations, worktree/index inputs, eleven floors, maintained receipt reuse and traceability. It wrote only `qa.json` and ran no native gate, coverage collection or screen check. The supplied Technical Director review accepted task-11.

QA's pending Chief-synthesis/final-mission fields describe its preceding stage; this authorized task now supplies the report without rewriting QA. No relevant reproducible product blocker remains in recorded Governor acceptance. Limits:

- Critical is explicitly injected through production daemon/Meter; separate actual SystemSource evidence records Normal sampling. No native OS Critical event, responsiveness/throughput benefit, savings or idle-SLO result is established.
- Keyboard/focus/help/announcement assertions have deterministic DOM coverage. Rendered contrast, VoiceOver and participant usability remain unverified.
- `av.yaml` reports **Operation not permitted** and remains uninspected; Serena is unavailable. Known-path reads/diffs and noninteractive audits were used without GUI workaround. No immutable deployment/container target was supplied; target health, availability, recent errors and service recovery remain unverified. Denial does not prove whether target configuration exists.
- Controlled local daemon fixtures establish supported local Cargo/one-shot Bun behavior, not installed daemon recovery, universal vendor hooks, Windows, deployment or distribution.

## Next actions

Return the preserved staged deliverables and this report for the supervising Chief/framework's saved final verification/review, without changing its contract. No implementation repair or new coverage collection is required for unchanged inputs. A later reproducible defect/input change must go to existing backend owners with renewed affected final-input evidence and independent QA within the saved limits.

Broader work follows the matrix. Any permitted final-review visual pass would establish additional rendered facts only if performed; none is claimed. Design's proposed four-person study/reminder comparison and Marketing's paid assessment, referral, guide/catalog and matched ON/OFF value tests remain **unmeasured future work**. No participant result, retention, conversion, revenue, Actor/tool cost or ROI is known here. No commercial publication, spending or outreach was added.

## Task-12 ownership, checks and delivery

Changed path/material artifact: **`.arc-verify/governor/final.md` only**. Other product/test/configuration bytes and index entries remain unchanged. Public interfaces and compiled inputs match final gates. No task instructions, acceptance criteria, mission graph, Actor assignments or state changed; no Actors or AV operations were dispatched.

| Chief command/audit | Outcome |
| --- | --- |
| `pwd`; `git status --short`; `git rev-parse HEAD`; `git diff --stat`; `git diff --cached --stat` | Inspected prior changes/accepted HEAD; recorded `av.yaml` denial. |
| `cat`/`sed` named policy, skill, report/handoff paths; `rg` on non-code docs | Read required instructions and used `oma-pm`; no native project-source discovery substitute. |
| Inline `python3` + `git cat-file --batch` | Manifest/digest, 632 current/index inputs plus matrix, QA bindings and 16 check argv/cwd/exits/log hashes/snapshots pass. |
| Inline `python3` archive/count audit | Ten native JSON text hashes, repair/history records, QA disposition and phase arithmetic reconcile. |
| Properly quoted `python3 -c` report write | Only owned report updated; original charter/criteria retained verbatim. |
| Scoped `git diff --check -- .arc-verify/governor/final.md` and `git diff --cached --check -- .arc-verify/governor/final.md` | Pass; only this report restaged by Chief. |
| `git check-ignore --no-index --stdin` / `git ls-files` and final inline document/preservation audit | Expected ignore exit 1/no ignored handoffs; required artifacts Git-visible. Other readable tracked bytes/index entries match intake; denied `av.yaml` unread. Sections, counts, input identity and evidence budget pass. |

One snapshot output exceeded the tool limit and could not be parsed; a bounded digest-only audit succeeded. An oversized draft was rejected before writing and shortened to the compact budget. Neither changed product files or supplied acceptance evidence. Task-12 ran **zero native gates, tests/builds, coverage collections or screen checks**.

Acceptance checklist:

- [x] Final source identity, executable evidence, independent review and phase counts reconcile.
- [x] Decisions, Alternatives, Measured results, Lessons and Next actions included; **Governor slice only**.
- [x] Scoped completion supported by required final evidence; no unresolved Governor blocker.
- [x] Broader product/SRS/commercial obligations and unmeasured experiments remain distinct.
- [x] Only assigned report changed; necessary deliverables non-ignored/Git-visible.


## Preserved charter and success criteria

The supplied `mission.goalBrief` (interpretation, assumptions and successCriteria) and version-1 `mission.verificationContract` remain unchanged. Their saved checks, exact argv, JSON pointers/values, file assertions and acceptance criteria govern execution. This decision record neither replaces those objects nor authorizes edits to them. An empty top-level `verifyCommand` does not remove the structured verification contract.

Original mission goal, retained verbatim:

> At accepted HEAD 08e7e526cce7e03ca9264afc926ed2e878dfde6d, deliver the first macOS Governor pause/resume slice required by SRS §7.3 and §15.5. Assign implementation, test edits, repairs and native gates to backend-engineer Actors using oma-backend, oma-db and oma-frontend, followed by independent read-only oma-qa review. Provide owner-authenticated daemon.pause/resume and the governor.enabled policy switch, headless CLI controls, and an accessible desktop master switch displaying Active, Paused or unavailable state. Reuse one durable, revisioned SQLite state and the existing writer; default ON and acknowledge mutations only after commit. OFF bypasses scheduling for supported new local jobs, including memory-pressure holds, while running children continue. Already queued or reserved-but-unspawned current-client jobs must transition to unreserved execution exactly once, with reservations reconciled. Resume restores existing admission rules. Serialize toggles with grant and launch decisions. Success requires synchronized production-daemon CLI and registered desktop-bridge tests demonstrating: with a one-slot window, job A runs and B queues; pausing allows B exactly once without a new permit while A survives; subsequent paused requests bypass admission; existing permits release without leaks; OFF survives daemon restart; and resuming restores window and fresh-Critical enforcement. Independently reconcile switch revisions, child counts, outcomes and permits through SQLite, CLI and bridge reads. Cover duplicate/conflicting writes, writer backlog, reserved-launch races, cancellation, lost acknowledgments and disconnect/reconnect. Confirmed write rejection restores the prior UI state; an uncertain acknowledgment triggers readback without falsely claiming success or rollback. Preserve protocol-1 compatibility, command semantics, fail-open behavior, Meter measurements and existing migration bytes. Derive the new TypeScript switch contract from Rust and test structural agreement. Pass the six mandated gates, retain 80% production-line coverage per crate, and run affected desktop checks. Use the existing coverage preflight and harness receipt reuse, with one successful collection per final input set. Run the final gate task alone in the mission workspace. Include amendment 13's dedicated documentation task creating docs/requirements/traceability.md with every requirement and acceptance-test ID, status, evidence and cycle; report completion counts by phase. Follow the compact evidence and allowed verification-command policies. Limit completion claims to this Governor slice.

The five saved success criteria, retained verbatim:

1. Production daemon, CLI and registered bridge tests prove committed, durable Governor control, exactly-once pause/resume scheduling, required recovery cases and preserved compatibility.
2. The accessible desktop master switch and Rust-derived TypeScript contract pass deterministic interaction, uncertainty, reconnect and structural-agreement checks.
3. All six mandated gates and affected desktop checks pass, with at least 80% production-line coverage in each of eleven crates and one reusable successful coverage collection for the final inputs.
4. Amendment 13 traceability enumerates every SRS requirement and acceptance-test ID with status, evidence, cycle and reconciled completion counts by phase.
5. Independent read-only QA and the Chief's final report reconcile final evidence and limit completion claims to this Governor slice.


## Compact evidence budget

The five canonical files under `.arc-verify/governor/` are contract.md **67,635 bytes**, integration.json **81,693 bytes**, gates.json **106,401 bytes**, qa.json **13,409 bytes** and final.md **30,734 bytes**: **299,872 / 300,000 UTF-8 bytes**, leaving **128 bytes**. The four preceding reports retain their hashes; no sixth report or raw-log handoff is added. This file is the task-12 material deliverable.
