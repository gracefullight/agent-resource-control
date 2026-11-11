# Chief Director report: ARC desktop foundation

Status: **task-11 report completed; framework/operator final verification pending**. The integrated macOS foundation has six recorded passing gates and an accepted independent read-only QA report. This report does not declare the whole mission or full Phase 0 complete. Prepared on 2026-11-11 UTC / 2026-11-12 Australia/Sydney using `oma-pm` and its execution protocol.

Accepted baseline and actual mission-worktree HEAD are `a9e7a31566e833dceeb82d6d75cd7c6a399583ab`. The implementation is the integrated staged/unstaged/untracked overlay, not a new accepted commit. Existing changes were inspected first and preserved. This task owns and changes only `.arc-verify/desktop-foundation/final.md`; it changes no product, test, manifest, lockfile, QA report, assignment, acceptance criterion, mission graph, Actor identity or verification command.

## Decisions

The three accepted Director artifacts remain the strategy inputs: [contract.md](contract.md), [experience.md](experience.md) and [value.md](value.md). Their planning-time pending statements describe their original stage. Current implementation and execution claims come from [gates.json](gates.json), its completed receipts and [qa.md](qa.md), rather than promotion of a plan into runtime proof.

| Decision | Alternatives considered and reason | Evidence class |
| --- | --- | --- |
| Reuse the authenticated transport in `arc-ipc`, retaining CLI wrappers. | Copying the client would duplicate authentication/deadlines; shelling out to human CLI output would create another subprocess/output contract. Extraction preserves one transport and daemon execution owner. | Source/interface observation plus engineering judgment; compatibility tests and QA support the delivered result. No latency or monetary saving was measured. |
| Tray activation opens the existing Dashboard directly. | A second popover adds a handoff; a metrics landing adds navigation before useful job details and prioritizes unavailable measurements. One list/detail surface is the smallest useful inspection journey. | Design judgment. Proposed action counts in experience.md are not measured task times or usability results. |
| Show recorded outcomes, independent timestamps, Unknown metrics and explicit freshness. | `COMPLETED` alone cannot prove exit success, a successful lookup cannot prove child success, and a bounded recent page cannot establish live global totals. Preserve known results through observation failures. | Protocol/source observations, production state logic and deterministic executed assertions. |
| Complete foundation proof before commercial experiments. | A setup/reliability offer depends on trustworthy inspection and desktop-only exit. Building promotion, telemetry or a funnel now adds work without a validated buyer or economics. Keep the exact Try OMA credit in About. | Business judgment; demand, customer savings, conversion, CAC, LTV, revenue, ROI and usability effects remain unmeasured. |

ARC here means Agent Resource Control for local coding-agent/build users. The Agent Valley positioning in `docs/prd.md` is a documented mismatch, not authority to add orchestration controls. No unrelated documentation correction, publication, outreach, spending or distribution was added. The personas and future field/pilot proposals remain hypotheses, not participants or customer results.

Implementation, test edits and native gates were assigned to the existing installed backend-engineer Actors using both `oma-backend` and `oma-frontend`. Shared manifests and lockfiles retained one writer. Independent `oma-qa` followed integration; task-10's report and its accepted task review establish that dependency. This Chief task uses `oma-pm` to reconcile those artifacts and does not replace either specialist implementation or independent review.

### Bounded recovery and preservation

Task-6's never-indexed `desktop-scaffold.tsx` deletion/staging failure was identified by the binding operator note as a framework bug. Its installed-runtime fix required reintegration/review, not recreation of the deleted scaffold. Subsequent concrete UI findings were repaired in their existing ownership: a settled detail older than the latest selected summary stays Stale until a valid revision catches up; unchanged polling no longer mutates the selected-result live announcement. The final 44-test component receipt executes both regressions. Exit results and original timestamps remain visible during the stale interval. DOM mutation assertions establish announcement stability, not a human screen-reader result.

Task-7 retained the existing coverage gate while adding explicit desktop manifest/source/test roots. Its grouped `use` parser repair ends the import at its semicolon, preserving adjacent production lines and the missing-file, zero-hit and exact per-crate threshold protections. The operator's 2026-11-11T18:46:58.837Z clarification bounds the other deliverable to one small checker: require gates.json, verify directly referenced target raw-log existence/SHA-256 and require recorded gate outcomes. The in-flight exception and filename-driven external JSON traversal were deleted. Their absence is the accepted repair; hypothetical recursive-authentication requirements were not reinstated. Focused compilation/tests were accepted upstream, and the final active suite retains the parser/checker regressions.

Task-9's earlier architecture failure was real: the registered-handler test file exceeded the harness file-length rule. The backend owner split it into the delivered responsibility-specific support files, retaining all nine test bodies and executed root identities. The engineer then repaired the synthetic test-name fixture to consume those includes; the earlier 115-pass/2-fail Bun result remains a failure. That repair changed no assertions or validators, compiled successfully and passed 27 focused tests. All final gates were refreshed on the repaired 400-file identity. Relevant bindings are `gates.json` sections `failedAttempts`, `repair` and `fixtureRepair`, including the hash-bound `backend-repair-consumed.json`, preservation/identity checks and `fixture-repair.json` under target.

These were bounded repairs to actual review evidence within the existing tasks. Accepted task definitions, original receipts, failures, reviews, spent attempts and budgets were preserved. No replacement task ID, historical rerun, QA rewrite or external-effect replay was used to clear a blocker.

## Results

The following are recorded measurements on the integrated macOS inputs, independently assessed in qa.md. This prose task also rechecked the current file and receipt bindings below; it did not rerun builds or native gates. All receipt filenames in the next tables are relative to:

`target/desktop-foundation/task-9-fixture-20261111T200733Z/`

Their full paths, SHA-256, byte sizes, argv, cwd, timing and stdout/stderr references are in gates.json. The 22 deduplicated current completed command receipts have exit 0, null termination, a 7,200,000 ms per-check timeout and matching before/after input identities. Shared workspace-test references across compatibility, lifecycle and daemon-bridge roles denote the same execution, not additional runs.

### Stack, frozen installation and compatibility smoke

Resolved/installed evidence establishes React 19.3.1, actual TypeScript 7.0.2, Vite 8.3.8, Tailwind 4.3.5, Base UI 1.9.0, Phosphor 2.1.10, Biome 2.6.6, Bun 1.4.4 and Rust Tauri 2.12.5. Tauri JS API/CLI are 2.12.4; the official Rust opener is 2.7.3. `apps/desktop/basecn-provenance.json` binds the installed basecn Button and its license bytes; production consumers use that component. Installed/version receipts support these versions; no compiler substitution is claimed.

| Exact recorded command | Result | Receipt |
| --- | --- | --- |
| `bun install --frozen-lockfile` | Exit 0; frozen install made no lockfile change. | `frozen-install.receipt.json` |
| `bun run desktop:typecheck` | Exit 0; real TS7 checks app/Vite and tests/tooling without emission; no skipped library checking. | `smoke-typescript.receipt.json` |
| `bun run desktop:build` | Exit 0; production Vite compilation. | `smoke-vite.receipt.json` |
| `bun run desktop:smoke:tauri` | Exit 0; native debug compilation using `tauri build --debug --no-bundle`. | `smoke-tauri.receipt.json` |
| `bun run desktop:check` | Exit 0; Biome checked 36 files without fixes. | `biome.receipt.json` |

The frozen Bun lockfile SHA-256 before and after installation is `cbefa79204ee4db7148cb98ba5a5ce3810254b31c526b97fee98f531899aa7ef`. Root Cargo workspace/shared Cargo.lock include `apps/desktop/src-tauri`; QA found all 71 original Cargo name/version pairs retained. The compiled `target/debug/arc-desktop` hash is `9928403eedf8ff120ca010d67461ec70d8e4bcbfd17392fa93c0c0bad9bdfafc`. Compilation does not establish a GUI launch, native interaction, release packaging or distribution.

### All six mandated gates

| Exact unchanged gate | Recorded result | Receipt |
| --- | --- | --- |
| `cargo fmt --all -- --check` | Passed, exit 0. | `format.receipt.json` |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed, exit 0; warning denial retained. | `clippy.receipt.json` |
| `cargo test --workspace` | Passed, exit 0; 614 passing native tests, no ignored or filtered tests. | `workspace-test.receipt.json` |
| `bun run test:coverage` | Passed, exit 0; eight individual production-line floors satisfied. | `coverage.receipt.json` |
| `./scripts/harness/validate.sh` | Passed, exit 0; 3,802 tracked files scanned, zero architecture violations; actual nested coverage passed. | `architecture.receipt.json` |
| `bun test` | Passed, exit 0; 117 passing tests, zero failures, all 13 active discovered test files. | `active-bun.receipt.json` |

The immutable verification contract retains one explicit `cargo test --workspace` entry. Coverage performs its own instrumented native executions; this is not a claim that tests ran only once in total. The architecture harness also actually ran `bun run test:coverage`, recorded separately in `architecture-nested-coverage.receipt.json`: 2026-11-11T20:25:56.052794–20:36:31.249225 UTC, within the outer harness interval 20:25:47.905898–20:36:31.347864. No skip flag or previous collection substituted for that execution. Receipt instrumentation preserved the exact nested command and PATH. Historical tests remain active; Bun discovery/ignore rules were not narrowed.

### Eight individual coverage results

Production-line accounting uses the existing gate with explicit roots for all eight crates, including `apps/desktop/src-tauri/src` and its tests root. The acceptance comparison remains integer `covered * 100 >= total * 80` for each crate. Display rounding and a high workspace average cannot satisfy a failing crate.

| Crate | Standalone covered / production lines | Percent | Harness-nested covered / production lines | Percent | Result in both |
| --- | --- | --- | --- | --- | --- |
| arc-core | 247 / 256 | 96.4844% | 247 / 256 | 96.4844% | Pass |
| arc-detector | 706 / 711 | 99.2968% | 706 / 711 | 99.2968% | Pass |
| arc-storage | 1811 / 1892 | 95.7188% | 1811 / 1892 | 95.7188% | Pass |
| arc-ipc | 868 / 905 | 95.9116% | 868 / 905 | 95.9116% | Pass |
| arc-daemon | 2242 / 2457 | 91.2495% | 2244 / 2457 | 91.3309% | Pass |
| arc-cli | 1608 / 1938 | 82.9721% | 1608 / 1938 | 82.9721% | Pass |
| arc-traffic | 111 / 111 | 100% | 111 / 111 | 100% | Pass |
| arc-desktop | 436 / 506 | 86.1660% | 436 / 506 | 86.1660% | Pass |

Standalone collection: `.arc-verify/rust-coverage/baseline/runs/2026-11-11T20-15-04-807Z-6a5df552-7793-4b1c-a1a6-f4b26c2da733/`. Harness-nested collection: `.arc-verify/rust-coverage/baseline/runs/2026-11-11T20-25-56-142Z-b374ae1f-48d9-43e7-865e-90ed1d65eade/`. Each retains its receipt, summary, raw LCOV/LLVM report and profiles; gates.json binds their hashes. QA independently recomputed both collections for 71 production files and authenticated 2,442 profiles per collection, including native child-profile evidence. Native startup/callback/handler production lines remain in the denominator.

QA's first read-only LLVM diagnostic timed out at its separate 90-second diagnostic limit. A shorter external-path filter completed exports from the same 45 existing objects and profiles; all 256 file maps and integer/function counts corroborated the stored accounting. LLVM emitted `warning: 22 functions have mismatched data`; QA found no resulting production-line discrepancy and only tiny summary percentage rounding differences. This failed diagnostic and warning remain disclosed, not relabeled as a failed required gate or an exact byte-for-byte export match.

### Delivered interface and lifecycle proof

The production Dashboard shows authenticated daemon-health observations, a bounded recent list and selectable matching ById details. The 50-record history page is never a live total. Valid detailed exit 0 is Succeeded, exit 17 is Failed, and absent recorded outcomes stay Outcome unknown. Record/observation times remain distinct from client receipt times. Known outcomes and selection survive failed refreshes; Disconnected, Stale, NotFound, empty history and status-unavailable states remain distinct. All eight unavailable resource/global metrics stay Unknown. A successful health exchange does not assert every subsystem is healthy.

`components.receipt.json` records this exact exit-0 command, with 44 passing tests:

```sh
bun test apps/desktop/tests/dashboard.test.ts apps/desktop/tests/credits.test.ts apps/desktop/tests/contracts.test.ts apps/desktop/tests/lifecycle.test.ts
```

The registered native suite uses the production router, commands and callback adapters through Tauri MockRuntime. The workspace receipt records six lifecycle tests and nine registered-handler tests, including `close_request_hides_existing_window`, `reopen_reuses_window` and `quit_desktop_never_stops_daemon`. Close prevention hides the existing main window; reopen shows/unminimizes/focuses that same window; quit cancels local bridge work and exits only the desktop. Tests replace OS ports rather than handlers or the state machine. Component tests cover hide/reopen/quit cleanup and visible local-action failures. These establish deterministic lifecycle behavior, not a physical tray click.

About/Credits retains the exact SRS content: **Agent Resource Control**, **Created by Eunkwang Shin**, icon-only Phosphor `GithubLogo` to **https://github.com/gracefullight**, icon-only `LinkedinLogo` to **https://www.linkedin.com/in/gracefullight**, and **Try OMA** to **https://github.com/first-fluke/oh-my-agent**. Executed assertions cover accessible names, tooltips, keyboard focus and fixed-destination opener requests. Local metadata supplies version, available build commit, OS architecture and notices independently of daemon connectivity. Authoritative adopted product-license metadata remains unavailable and the UI says so; the proposed SRS Apache-2.0 value was not invented as adoption. Bundled dependency notices are separate from the product license. No live default-browser opening or assistive-technology session is claimed.

### Authenticated bridge, real-daemon outcomes and survival

Fixed typed Rust commands `daemon_health`, `recent_jobs` and `job_by_id` reuse protocol 1, same-user authentication before request bytes, the existing 1 MiB framing bound and a two-second absolute read deadline. Socket work is off the UI thread and native permits bound queued work. Full responses and selected identity are validated before projection; diagnostics are redacted. No arbitrary operation, SQL, socket path or execution proxy is exposed to JS. The refresh controller retires late generations/selections, serializes native reads, preserves last-known data and bounds reconnect backoff at 30 seconds.

The current workspace receipt records all seven `apps/desktop/src-tauri/tests/daemon_bridge.rs` cases:

| Executed native case | Established result |
| --- | --- |
| `records_exit_zero` | Production bridge retrieves a recorded normal exit 0. |
| `records_exit_seventeen` | Production bridge retrieves recorded failure with exact exit 17. |
| `disconnect_reconnect` | Disconnection and subsequent authenticated recovery work without replaying execution. |
| `rejects_malformed_response` | Production bridge rejects an injected malformed peer response. |
| `rejects_mismatched_job_id` | Production bridge rejects a response for another selected ID. |
| `daemon_survives_desktop_exit` | Actual desktop-client subprocess exits; the owned daemon retains PID plus birth/start identity and remains queryable. |
| `active_child_survives_desktop_exit` | Desktop-client exit preserves daemon, wrapper and active-child identities; releasing the child records exit 17 with launch count one. |

The fixture starts an isolated real daemon with private runtime paths, owner-only permissions, existing cross-process fixture serialization and owned-child cleanup. Malformed/mismatched peers are separately injected against the production bridge; they are not falsely described as real-daemon malformed output. The desktop client re-executes the native test binary with the production registered router and a headless OS exit adapter. It actually exits that process; it is not the Wry GUI executable or live tray interaction.

Current survival sidecars are `target/desktop-foundation/task8/13039/{active_child_survives_desktop_exit/active-child-survival.json,daemon_survives_desktop_exit/daemon-survival.json}` and the equivalent paths under `task8/27452/`. QA's hash table binds these four artifacts and their matching child/client profiles. The standalone active case binds daemon 13056, wrapper 13057, child 13058 and exited client 13059; the nested case binds 27509/27510/27511/27512. These are recorded fixture identities at execution time, not assertions that those PIDs are live now. Profile readback corroborates both survival functions and all five other bridge cases.

Compatibility evidence is the same completed unfiltered workspace receipt plus QA's accepted-base comparisons: all three SQL migrations, protocol job types, Bun discovery configuration and architecture harness remain byte-identical to baseline; CLI wrappers preserve request construction, uncertain-commit tracking, retry/deadline and native execution semantics. Authentication/endpoint logic was reused, not replaced. CLI exit/restart, privacy and migration regressions executed. Projection excludes command/chat text, argv, cwd, socket paths and process birth identity; no telemetry or external service was added.

### Target applicability, evidence and independent QA

`targets.satisfied=true` means **not applicable** under the supplied accepted no-observation/container-policy context, with `observationPolicy=null`. It does not establish service health or configuration absence. `av.yaml` readback still fails with Operation not permitted; Git status emitted the same access diagnostic in this task. It was not interpreted as deletion, bypassed with GUI tools, or used to invent a target inventory. No deployment or production service restart occurred.

Current identity is 400 files, aggregate SHA-256 `0f3fb3538812375c4e4fa444abadb863e445483ff094c9dda3f387583b57a63b`, bound by `target/desktop-foundation/task-9-fixture-20261111T200733Z/frozen-install.after.inputs.json` (SHA-256 `130f92c298fb8e2a463348d614554e4d2d2851b8d3e3d47ec8ad6ad16fad887c`). This task independently rehashed every listed input and its sorted compact-JSON aggregate: zero mismatches. QA additionally recomputed complete current discovery, rather than trusting only the listed manifest.

This task independently verified all 130 unique directly referenced artifact paths against recorded SHA-256/byte sizes and compared the 22 current completed receipt records with their gate metadata: zero mismatches. gates.json itself remains SHA-256 `26e9bc58b07c6f898ac72ceb68ec548d2dd36fd0e3ffcc6ce0f9155b6e82e803`. The independent qa.md consumed here is SHA-256 `3d7b9bd68023c5d6354e7527d4c67d738732f7e423119e78d69b3518a9ff0e7a`. Native raw stdout/stderr logs remain hashed under target, including empty logs. The existing small checker assessed 74 directly written raw-log references; these counts are references, not independent execution counts.

There are **six evidence files after this delivery**, below the seven-file ceiling: contract.md, experience.md, value.md, gates.json, qa.md and final.md. Exactly one small checker remains, the 30-line `scripts/coverage/desktop-evidence.ts`, with the existing active entry `scripts/coverage/desktop-foundation.test.ts` and focused tests. This report adds no checker, runner, manifest or raw-log copy. gates.json's recorded evidenceFileCount of four describes its gate-time state; QA made five and this report makes six. Its older `unverified.independentQa` field likewise records the task-9 stage. The accepted task-10 report now supplies QA; neither historical field was rewritten.

Historical provenance stays separate: source base `6b5218cf63b174b7976e52d81d56c7e3bf81112d`, delivered historical evidence at accepted HEAD, 24 verified source/repair overlay hashes, zero available and 76 unavailable predecessor outputs, `currentAcceptance=false`. Missing old aggregate/raw runtime evidence is not recertified. Original evidence remains unchanged; fresh current receipts establish this foundation's results.

Independent task-10 `oma-qa` reported PASS for its automated macOS foundation scope, with no reproducible product blocker or CRITICAL/HIGH/MEDIUM/LOW finding. Its subsequent task review was accepted in the supplied dependency. No repair dispatch is needed on this evidence. Native/manual/platform/historical limitations stay explicit. This report does not clear a defect by editing QA, replace framework independent task reviews, or claim that framework final verification has run.

## Lessons

Authenticated transport and outcome semantics already existed; preserving their boundaries was more useful than adding another execution owner. The direct Dashboard could therefore focus on the actual user question: what was recorded, when, and how reliable is this observation? Unknown measurements and stale outcomes are useful information when presented truthfully.

Freshness and accessibility needed behavioral regressions, not just labels. Task-6's delayed-result test protects the selected summary/detail relationship; live-region mutation assertions protect users from repeated unchanged announcements. Deterministic assertions do not eliminate the remaining native/assistive validation gap.

Coverage and historical receipt issues required bounded accounting/provenance work, not weakened tests or restoration of old manifests. The operator's small-check clarification prevented an evidence-authentication subsystem from displacing the desktop product. Actual gate failures still required owner repairs and new receipts; an accepted planning report or an unrelated previous pass could not substitute.

Recorded pass counts, bundle sizes and routing samples establish no customer demand, retention, saved time or profit. The commercial proposals remain proposals. Background tray persistence is not voluntary return, and the outgoing OMA credit is not an acquisition agreement.

## Next actions

1. Return this sole-owned report to the supervising framework for its existing independent task review and unchanged final verification contract. **Framework/operator verification: pending**; the empty legacy verifyCommand was not replaced, and no fabricated overall verification result is recorded. Preserve the exact focused commands and the single explicit workspace-test entry in that contract.
2. **Live native tray interaction: unverified.** MockRuntime lifecycle/callback tests and a real headless client exit do not prove a native tray click, actual window hide/reopen or macOS menu behavior. Any relevant screen-based/native interaction assessment belongs only to the permitted final-review pass after code verification, using the allowed existing tools. No such pass was performed by this task.
3. **Windows acceptance: unverified.** Native receipts are for aarch64-apple-darwin; the current non-Unix bridge returns Unsupported. No Windows implementation or test result follows from this macOS foundation.
4. **Full Phase 0 completion: unverified.** The SRS's wider two-OS/full-phase obligations are not established by these foundation receipts. Native accessibility, rendered contrast/layout, startup/RSS, human task success and broader integrations also remain unmeasured. Adopted product-license metadata remains unavailable; this task initiates no distribution.
5. Retain original artifacts if historical recertification is later required; do not recreate old logs as past results. Any commercial field test, publication, outreach, spending or broader product change requires separately assigned scope. No such work was started here.

The saved execution limits remain three concurrent workspaces, one absolute 172800-second mission duration, 400 total runs, mission `maxRetries=3` and the existing attempt/repair limits across recovery. The retained native execution-context artifact records deadline `2026-11-13T16:21:27.747Z`; current time was checked as `2026-11-11T20:57:03Z` before this report was written. The supplied mission checkpoint records 43 measured runs; it is not a reset or a newly measured final total. No counter, deadline, Actor route/vendor/model or mission state was changed. This task starts no agents, native verification wave or external effect. Cancellation or deadline exhaustion requires stopping new admissions and an accurate partial handoff, not creating replacement tasks or resetting spent budgets. Host resource/cost enforcement beyond the recorded context is not claimed.

### Task-11 checks and acceptance handoff

Commands/actions executed for this documentary task: `git status --short` (exit 0 with av.yaml access-denied diagnostic); `git rev-parse HEAD` (exit 0, accepted HEAD); scoped `cat`/`sed` and JSON reads of assigned artifacts, policy, receipts and manifests (exit 0); read-only inline `python3 -B` SHA-256/input/receipt audits (exit 0, 400 inputs, 130 direct artifacts and 22 completed receipts matched); current UTC readback; and inline Python document content/coverage-table/link/whitespace/scope checks (exit 0). `git diff --no-index --check -- /dev/null .arc-verify/desktop-foundation/final.md` exited 1 with no diagnostics for the new-file difference; the independent Python whitespace check passed. These are report checks, not newly executed product tests. No build, native gate, coverage collection, browser E2E, Playwright download, GUI workaround or publication was performed by task-11.

- [x] Decisions, alternatives, measured results, lessons and next actions are written with receipt references and evidence boundaries.
- [x] QA and all six exact gates are reconciled; prior failures, missing historical evidence and unavailable metadata remain explicit.
- [x] Live native tray interaction, Windows acceptance and Full Phase 0 completion each retain a separate truthful status; framework/operator final verification remains pending.
- [x] Original scope, permissions, Actor identities, checks and spent budgets are preserved; six evidence files and one small checker remain within the limits.

Changed path: `.arc-verify/desktop-foundation/final.md`. Shared interfaces are unchanged by this task. No unresolved reproducible product blocker was supplied by accepted QA; the manual/platform/full-phase and downstream framework-verification limits above remain open.
