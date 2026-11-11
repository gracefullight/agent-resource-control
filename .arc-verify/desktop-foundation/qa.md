# Independent desktop QA — task-10

Status: completed. Skill used: `oma-qa`, including its execution protocol and review checklist. **Review Result: PASS for the assigned automated macOS desktop foundation scope.** No reproducible CRITICAL, HIGH, MEDIUM or LOW product defect was established. Review completeness is completed for this source, assertion and receipt review; the manual, platform and historical limits below remain unverified. This is not a declaration of full Phase 0 completion or a replacement for framework final verification.

## Scope

Reviewed the integrated working-tree overlay against accepted and actual HEAD `a9e7a31566e833dceeb82d6d75cd7c6a399583ab`. Existing staged, unstaged and untracked delivery changes were inspected first and preserved. The review covered:

- Root workspace and lockfiles; prescribed desktop dependencies, basecn provenance, TS7/Vite/Tauri smoke and Biome configuration.
- `apps/desktop/src-tauri/src/{bridge,commands,dto,error,lib,lifecycle}.rs`, native configuration, registered-command/lifecycle tests, real-daemon bridge tests and their shared fixtures.
- `apps/desktop/src/bridge/`, the production Dashboard/refresh controller/selected panel, lifecycle subscriptions, credits, messages and deterministic DOM/contract tests.
- Extracted `crates/arc-ipc/src/client/` transport and CLI wrappers; unchanged protocol, migration and execution compatibility protections.
- Explicit eight-crate coverage roots, source parser, production denominators, freshness, raw profiles and historical/current provenance separation.
- Final task-9 receipts, completed native/Bun output, native executable, coverage exports and daemon/child/client process evidence.

Only `.arc-verify/desktop-foundation/qa.md` is authored by this reviewer. No product, tests, manifests, configuration, other evidence, staging or mission state was changed. No build, native test, native gate, coverage collection, install, browser, screen check, GUI workaround or external delivery was run. The Bun diagnostics below call read-only exported validators; LLVM diagnostics read existing binaries/profiles and export to memory. They do not run application/test binaries.

Serena tools were not available in the active tool inventory. Inspection used scoped reads of assigned/diff-listed paths and Git diffs, without substitute recursive source search or provider installation. A quick memory lookup found no relevant ARC entry; no historical memory supplies a review conclusion. Git's `av.yaml: Operation not permitted` diagnostic is retained as access denial, not evidence of deletion. The accepted contract's supplied no-observation-policy basis determines target applicability; hidden configuration or service health is not inferred.

The binding operator note at `2026-11-11T18:46:58.837Z` supersedes task-10's older task-7 traversal/in-flight wording. Task-7 is reviewed as eight-crate accounting plus one small checker of gates.json, its directly referenced target logs/hashes, and recorded gate outcomes. Recursive receipt traversal, registry semantics and in-flight exceptions are not required and were not reintroduced as acceptance conditions.

## Evidence

### Current identity and completed command receipts

`gates.json` SHA-256 is `26e9bc58b07c6f898ac72ceb68ec548d2dd36fd0e3ffcc6ce0f9155b6e82e803`. Independent Python SHA-256 readback verified all **130 unique directly referenced artifact paths**, including byte sizes where recorded, with zero missing/mismatched references. The small checker independently read **74 direct raw-log references**. These are reference counts, not counts of distinct executions.

The current 400-file input aggregate is `0f3fb3538812375c4e4fa444abadb863e445483ff094c9dda3f387583b57a63b`. Its manifest is `target/desktop-foundation/task-9-fixture-20261111T200733Z/frozen-install.after.inputs.json`, SHA-256 `130f92c298fb8e2a463348d614554e4d2d2851b8d3e3d47ec8ad6ad16fad887c`. Python independently checked each file and the sorted compact-JSON aggregate. `requireInputIdentity` also recomputed the complete current inventory, including new inputs rather than merely trusting that manifest. All 400 files matched.

Independently inspected **22 distinct current completed command receipts** across stack, smoke, tests, compatibility, gates and the completed small check. For each, exact argv/cwd/times/exit matched the gate entry; status was passed, exit was 0, termination was null, timeout was 7,200,000 ms, and before/after manifests matched the current aggregate. Referenced stdout, stderr and recorder bytes matched hashes and sizes. Shared workspace-test references in several roles were deduplicated, not counted as additional native runs. Failed prior attempts remain separately retained.

Current receipt directory: `target/desktop-foundation/task-9-fixture-20261111T200733Z/`. Recorded cwd is this mission workspace, except explicitly scoped desktop tool-version calls in `apps/desktop`. The material task-9 handoff and delivery path/hash inventory were consumed and checked against the integrated files; their conclusions alone were not used as execution proof.

### Acceptance traceability and native gates

| Acceptance area | Independently assessed evidence | Result and boundary |
| --- | --- | --- |
| Prescribed stack and frozen lock | Root Bun workspace; exact package pins; installed/version receipts; frozen-install before/after lock hash; TS7, Vite and Tauri completed smoke logs | Passed on integrated macOS inputs. Actual TypeScript is 7.0.2, React 19.3.1, Vite 8.3.8, Tailwind 4.3.5, Rust Tauri 2.12.5, Bun 1.4.4 and Biome 2.6.6. No compiler substitution. |
| Dashboard, exact credits and truthful states | Production components/client, shared fixtures and all required component identities in completed logs | Passed automated DOM/contract scope; native rendering and assistive technology remain unverified. |
| Authenticated typed protocol 1 and compatibility | Shared transport, fixed commands, full-snapshot validation before projection, CLI wrappers, current workspace execution identities and base-byte comparisons | Passed reviewed scope. No execution/SQL/arbitrary-operation proxy added. |
| Close-to-hide, reopen, desktop-only quit | Same registered router and lifecycle adapters as production; six lifecycle and nine registered-handler tests present in executed native output | Passed deterministic MockRuntime/port scope; no physical tray/menu interaction claimed. |
| Real-daemon bridge and survival | All seven required native bridge cases; isolated fixture code; current child sidecars, PID/birth identities and profile readback | Passed automated native fixture scope. Malformed/mismatched peers are injected against the production bridge; they are not claimed to be malformed real-daemon output. |
| Six gates and per-crate coverage | Exact completed receipts, parsed native/Bun summaries, both independently recomputed coverage collections, actual nested coverage interval | Passed; detailed results below. |
| Fresh evidence and historical separation | 400 current inputs, 130 direct artifact hashes, small checker, Git-object historical checks, preserved failed attempts | Passed current evidence scope. Historical unavailable outputs remain unavailable. |
| Independent QA and final reporting | This report supplies independent read-only QA, scope, findings and limits | QA delivered. Chief-owned final.md and immutable framework final verification remain downstream. |

| Required command recorded by the backend-engineer gate owner | Receipt | Outcome |
| --- | --- | --- |
| `cargo fmt --all -- --check` | `format.receipt.json` | Exit 0. |
| `cargo clippy --workspace --all-targets -- -D warnings` | `clippy.receipt.json` | Exit 0; warning-denying flags preserved. |
| `cargo test --workspace` | `workspace-test.receipt.json` | Exit 0; 614 passing native tests, no ignored/filtered tests. The contract lists this explicit command once; QA did not rerun it. |
| `bun run test:coverage` | `coverage.receipt.json` | Exit 0; all eight crates individually above 80%. |
| `./scripts/harness/validate.sh` | `architecture.receipt.json` | Exit 0; all-tracked scan of 3,802 files, zero violations and actual nested coverage. |
| `bun test` | `active-bun.receipt.json` | Exit 0; 117 pass, 0 fail across all 13 active discovered files. |

The architecture command ran from `20:25:47.905898` to `20:36:31.347864` UTC on 2026-11-11. Its nested `bun run test:coverage` ran from `20:25:56.052794` to `20:36:31.249225`, wholly inside that interval, and passed. The inspected BASH_ENV hook intercepts only the exact root `bun run test:coverage`, forwards to pinned Bun and retains PATH; it is receipt instrumentation, not a skip or replacement gate. Harness bytes match accepted HEAD. The backend owner's earlier 742-line test failure and subsequent fixture failure remain recorded as failures, outside current acceptance.

Other completed recorded commands include `bun install --frozen-lockfile`, `bun run desktop:typecheck`, `bun run desktop:build`, `bun run desktop:smoke:tauri`, `bun run desktop:check`, and the four-file component command. Frozen installation reports no changes; lock hash before and after is `cbefa79204ee4db7148cb98ba5a5ce3810254b31c526b97fee98f531899aa7ef`. TS smoke invokes the actual `typescript/bin/tsc` for both app/Vite and tests/tooling without emission or skipped library checking. Tauri smoke is native debug compilation without bundling/GUI launch. Vite reports 422.10 kB JS / 130.75 kB gzip and 21.55 kB CSS / 4.70 kB gzip; these are recorded output sizes, not startup or memory measurements. Biome checked 36 files without fixes. Basecn's installed Button and license bytes match their provenance hashes; all three declared consumers use it. Phosphor is imported by the production interface. Cargo.lock retains all 71 original name/version pairs and adds desktop dependencies; none of those original pairs was removed.

`desktopSectionsEvidence(..., true)` verified the completed role-bound smoke, components, registered handlers, daemon bridge and compatibility receipts. `activeBunEvidence` verified the exact unfiltered root `bun test`, completed output and every currently discoverable active test file. The 44 required component/contract/lifecycle tests all executed, including both task-6 repair regressions. The current small evidence test also has its own completed exit-0 receipt, after active Bun completed. Synthetic validator fixtures are marked synthetic and do not substitute for any native/component result.

### Production bridge, authentication, lifecycle and privacy

`src-tauri/src/lib.rs` installs one router through `install`, used by production `configure` and test `configure_with_ports`. Tests replace OS ports/tray creation, not the router, handlers, state machine or bridge. Registered commands use `tauri::test::get_ipc_response` against that installed router. `on_window_event`, `on_menu_event`, `on_tray_icon_event` and `on_run_event` are wired in the production builder/run path.

`bridge.rs` exposes only health, Recent and ById data reads; native-only socket/clock constructors cannot be selected from JS. It validates the 32-lowercase-hex job ID before I/O, full status object shapes and protocol semantics before private fields are dropped, unique/bounded Recent entries, safe times, correct result variant and returned job ID. Recent is capped at 50 and remains `liveness_fresh=false`. Socket work uses `spawn_blocking`; an atomic permit is acquired before queueing and held until blocking work ends. Cancellation remains attached to that native operation even if the caller drops its future.

The shared client authenticates same-user peers before sending lifecycle bytes, preserves protocol/request correlation and error-envelope checks, uses the existing 1 MiB framing limit, and applies the two-second desktop absolute read deadline with bounded attempts, cancellation polling and DB-busy backoff. Peer diagnostics are mapped to fixed error codes. JS rejects extra/missing shape fields and normalizes raw rejections rather than stringifying them. Projection contains no command/chat text, argv, cwd, socket path, process birth identity or classifier payload. CSP, event-only capability and native enum opener provide the inspected local boundary; no telemetry or external service was added.

Accepted-base comparison confirmed byte-identical `peer.rs` and `endpoint.rs` after relocation. Exchange loop and DeadlineStream code remain token-equivalent after necessary module/borrow spelling changes. Continuity changes are visibility/formatting only; CLI wrappers retain request construction, commit tracking, retry policy and call deadlines. All three SQL migrations, `arc-ipc/src/jobs/types.rs`, `bunfig.toml` and the harness are byte-identical to accepted HEAD. Current executed native identities include CLI exit-17/restart, exit-0/restart, shared-deadline, protocol privacy and migration rollback/checksum cases. No daemon production implementation or execution ownership was moved into the desktop.

Close prevention calls the same `close_requested` adapter used by the installed window callback; its prevention closure is injected because Tauri's CloseRequestApi constructor is private. Tests verify prevention, hiding the existing main window, same-window reopen, show/unminimize/focus calls, one local quit, no daemon socket action/deletion and cancellation. Production TauriShell's actual native methods remain compiled; exit only calls the desktop AppHandle. This proves deterministic callback/handler behavior, not that a human clicked a native tray or observed a window hiding.

### Dashboard races, task-6 repair and credits

The production refresh controller serializes native calls, retains at most one recent page and selected detail, retires stale generations/connection epochs/selections, cleans timers on hide/unmount/quit, and caps reconnect backoff at 30 seconds. Deferred-promise assertions cover A/B selection races and hide/reopen before the retired read settles, with maximum in-flight reads equal to one. Recorded outcomes and original times survive disconnection/detail errors; NotFound is distinct from empty history and unavailable status. Health confirms only the last authenticated health exchange. All eight unavailable resource metrics remain Unknown; a 50-row page is never substituted for a live global total.

Task-6's repaired lookup compares a settled detail's `updated_at_ms` with the latest accepted selected summary before declaring it fresh (`desktop-refresh-controller.ts`). `dashboard.test.ts`'s case “a newer selected summary keeps an older settled detail stale until a valid revision catches up” holds the production lookup pending, settles an older valid reply, verifies Stale in both selected row and panel, preserves exit 0 and the original update/observation times, then clears Stale only after revision 4 catches up. The completed final component log contains that executed case; the original Actor-local RED artifact is not asserted to be available here.

`selected-job-panel.tsx` keeps the selected-result live region's text tied to selected ID/outcome rather than its loading phase. The completed “unchanged polls never mutate the selected-result announcement; a new selection announces its result” test observes childList/characterData/subtree mutations, verifies the same region node and zero mutations during two pending-and-settled unchanged polls, then verifies mutations and the correct exit-17 announcement on a new selection. Connection changes have a separate polite status region. These are DOM assertions, not a screen-reader session.

Observed, registered, updated, completed and native receipt times retain separate labels; exact ISO/Unix instants are accessible through details/summary. Monotonic receipt age controls Stale. Completed summaries alone remain Outcome unknown; only a matching validated detail displays exit 0 as success or exit 17 as failure. Signal, spawn failure and unavailable outcome retain distinct meanings. Keyboard listbox movement/selection, navigation focus restoration, tooltip focus, local-action failure and StrictMode cleanup have executed component assertions.

SRS §15.9 was read directly from `/Users/gracefullight/Downloads/Agent_Resource_Control_ARC_Requirements_SRS_2026-11-06.md`. Production credits and tests preserve Agent Resource Control, Created by Eunkwang Shin, icon-only Phosphor GithubLogo/LinkedinLogo with accessible names/tooltips/focus, and Try OMA. Exact destinations are `https://github.com/gracefullight`, `https://www.linkedin.com/in/gracefullight` and `https://github.com/first-fluke/oh-my-agent`. Both JS and Rust accept only the three enum choices; the official native opener receives those fixed URLs. Metadata comes from the local native command independently of daemon health. Missing build/license/notices stay explicitly unavailable; the SRS's proposed Apache-2.0 value is not invented as an adopted product license. Notice text is rendered as text, with an executed injection-sentinel assertion.

### Coverage, native binary/profile and child evidence

Independently recomputed both stored collections using current `sourceInventory`, raw LCOV, full LLVM JSON and `account`; recomputed file accounting and all crate results exactly match their stored summaries. There are 71 production files. Explicit roots include `apps/desktop/src-tauri/src` and its separate tests root. Missing crate/file, zero-hit, malformed LF/LH/DA, exact threshold, duplicate/alias accounting, test-only parsing and configuration-freshness protections remain active. The grouped-use parser regressions check its semicolon boundary, nested/multiline imports, adjacent production retention and genuinely ambiguous shared lines.

| Crate | Standalone covered / production lines | Percent | Harness-nested covered / production lines |
| --- | --- | --- | --- |
| arc-core | 247 / 256 | 96.4844% | 247 / 256 |
| arc-detector | 706 / 711 | 99.2968% | 706 / 711 |
| arc-storage | 1811 / 1892 | 95.7188% | 1811 / 1892 |
| arc-ipc | 868 / 905 | 95.9116% | 868 / 905 |
| arc-daemon | 2242 / 2457 | 91.2495% | 2244 / 2457 |
| arc-cli | 1608 / 1938 | 82.9721% | 1608 / 1938 |
| arc-traffic | 111 / 111 | 100% | 111 / 111 |
| arc-desktop | 436 / 506 | 86.1660% | 436 / 506 |

All floors remain exactly 80% **per crate**, not a workspace average or rounded display threshold. Collections are:

- Standalone: `.arc-verify/rust-coverage/baseline/runs/2026-11-11T20-15-04-807Z-6a5df552-7793-4b1c-a1a6-f4b26c2da733/`.
- Harness-nested: `.arc-verify/rust-coverage/baseline/runs/2026-11-11T20-25-56-142Z-b374ae1f-48d9-43e7-865e-90ed1d65eade/`.

Each collection has 2,442 hashed raw profiles and an exact authenticated merge-input list. Child-proof records include both owned arcctl and arc-daemon probe PIDs and their nonempty matching profiles. The nonzero daemon probe exit is an intentional separate probe, not a passing application run. The current native desktop executable's SHA-256 `9928403eedf8ff120ca010d67461ec70d8e4bcbfd17392fa93c0c0bad9bdfafc` matches its gate reference.

Read-only LLVM 22 export from the **45 existing objects and existing workspace.profdata in each collection** corroborated all 256 stored file maps. All segments, integer data and function name/count mappings match; only summary percentage rounding differs, by at most `1.4210854715202004e-14`. Feeding these fresh in-memory exports back into accounting against the stored LCOV produced the same eight crate results in both collections. Both survival functions have recorded entry count 2 (parent plus re-executed client), and the other five required bridge case functions have positive count 1. This assesses compiled mapping/profile evidence as well as receipt strings.

The first diagnostic replay used the exact long recorded external-root exclusion expression and timed out at its 90-second diagnostic limit; it produced no acceptance result. Recovery used the same objects/profile with a shorter external-path exclusion expression, keeping exports in memory, and completed in about 5.6 seconds per collection. LLVM emitted `warning: 22 functions have mismatched data` during this diagnostic readback. The full file-map/name-count comparison and production-line recomputation above showed no resulting integer/line-coverage discrepancy. This diagnostic warning and the failed attempt are disclosed; they are not relabeled as a native gate failure or hidden behind a byte-identical-export claim. No stored report/profile was changed or merged again.

The real-daemon fixture uses private 0700 runtime directories, 0600 sockets, shared cross-process fixture serialization and owned-child cleanup. It retains the production instance anchor and never kills a PID merely discovered from an existing daemon. The desktop client is an actual subprocess re-executing the native bridge test binary, running the production registered router with a headless OS exit adapter; it is not a launch of the Wry GUI executable. That adapter records bridge cancellation and actually exits the process through the registered quit path. Parents assert client disappearance and verify daemon/wrapper/child PID **plus boot/start identity**, then release the held child and verify recorded exit 17 and launch count one.

Fresh standalone profile case directory `target/desktop-foundation/task8/13039/` binds active-survival daemon 13056, wrapper 13057, child 13058 and exiting client 13059. The separate daemon-survival case verifies daemon 13060 after client 13063 exits. Equivalent nested cases under `task8/27452/` bind daemon 27509, wrapper 27510, child 27511 and client 27512; the separate daemon case uses daemon 27513/client 27516. Their referenced client/wrapper/child raw profiles match the current collection's recorded hashes and byte sizes, not just a filename pattern.

Independently read and hashed supporting sidecars (their assertions are corroborated by current executed native cases and profiles):

| Path below target/desktop-foundation/task8 | SHA-256 |
| --- | --- |
| `13039/active_child_survives_desktop_exit/active-child-survival.json` | `e360803ae1562b72abde7f3dfbe5aee3a68cfa0837817b1de36ad97dc4c5347c` |
| `13039/daemon_survives_desktop_exit/daemon-survival.json` | `3042d26e10284abd6a490523aecc410446f03fa64c6e49d7c0f5ca21dc224cf0` |
| `27452/active_child_survives_desktop_exit/active-child-survival.json` | `1c7b7f3d465edce5d01a834a2d4ed3f03e72cc04d5757d30f91dd0b826b3f847` |
| `27452/daemon_survives_desktop_exit/daemon-survival.json` | `52cb850135270727a9e41a25981c2f717327a6f0d18e433511a30a1520fd2fb9` |

### Task-7 clarification and historical evidence

`scripts/coverage/desktop-evidence.ts` is one 30-line checker. It requires gates.json, verifies that all six gate outcomes are present, and authenticates each directly referenced target raw log. It has no in-flight exception, filename-driven recursive JSON loading or registry authentication. Focused active tests cover missing/malformed gates, absent/tampered logs even while the active-Bun outcome says running, nested direct logs, invalid target paths and hashes. The earlier task wording requesting a restricted exception/traversal ordering is superseded by the operator's instruction to delete that mechanism; its absence is the correct result. No new recursive traversal requirement or hypothetical tampering condition is a blocker.

Historical terminal evidence remains bound to source base `6b5218cf63b174b7976e52d81d56c7e3bf81112d` and delivery commit `a9e7a31566e833dceeb82d6d75cd7c6a399583ab`. Independent `terminalHistory` execution verified 24 delivered source/repair overlay hashes and unchanged committed historical reports. It reports **0 available and 76 unavailable historical outputs**, `currentAcceptance=false`. Original aggregate/runtime/coverage cannot be recertified without those outputs. New desktop dependencies are checked against fresh current inputs, while original manifest relationships are checked against their original Git objects. Archived tests and Bun discovery remain preserved. No historical pass was promoted into current acceptance or reconstructed.

There were four desktop-foundation evidence files at review entry; this report makes five, below seven. The recorded gate-time count of four describes its recording time. QA/final generated reports are deliberately outside the 400-file source digest, avoiding self-reference. The existing small check and final framework verification remain intact.

### Commands performed by this reviewer and reproducibility

All commands ran from the mission workspace and were noninteractive. No required native gate or test suite was rerun by QA.

| Reviewer command/action | Outcome |
| --- | --- |
| `pwd`; `git status --short`; `git diff --stat a9e7a31566e833dceeb82d6d75cd7c6a399583ab`; scoped `git diff`/`git show` | Inspected integrated changes and base preservation. Git reported av.yaml access denial; it was not treated as deletion. |
| Scoped `cat`, `sed`, JSON reads and direct SRS read | Reviewed the source, assertions, policy and receipts named above; no search provider or GUI workaround was installed. |
| `python3 -B -` hash/receipt audit | 400 current input hashes/aggregate, 130 direct references and 22 deduplicated completed receipts verified; zero mismatches. |
| Read-only `bun -e` diagnostics invoking the functions below | Current inventory, completed role evidence, full active-Bun discovery, small checker and both coverage collections passed recomputation. |
| `bun -e` calling `terminalHistory(process.cwd())` | 24 historical overlay hashes verified; 76 unavailable outputs disclosed, no current acceptance. |
| Read-only Python/LLVM export to memory, using existing object/profile argv extracted from stored verbose receipts | Exact-filter diagnostic timed out at 90 seconds. Short-filter readback succeeded for both runs; comparisons/accounting passed with the diagnostic warning documented above. |
| Python current child-sidecar/profile and base-lock comparisons | Current survival identities/profile hashes matched; protected base files match; all original Cargo package pairs retained. |
| Final report/input/scope readback | Confirms only this report was authored and the original current source/evidence bindings remain unchanged. |

The core read-only accounting/role diagnostic is reproducible without a build or test runner:

```sh
bun -e '
import { readFileSync } from "node:fs";
import { requireInputIdentity } from "./scripts/coverage/source";
import { desktopSectionsEvidence, activeBunEvidence } from "./scripts/coverage/desktop-operations";
import { coverageEvidence } from "./scripts/coverage/coverage-receipts";
import { desktopEvidence } from "./scripts/coverage/desktop-evidence";
const root = process.cwd();
const g = JSON.parse(readFileSync(".arc-verify/desktop-foundation/gates.json", "utf8"));
const m = JSON.parse(readFileSync(g.inputIdentity.manifest.path, "utf8"));
const id = requireInputIdentity(root, m);
desktopSectionsEvidence(root, g, id.sha256, true);
activeBunEvidence(root, g.gates.activeBun, id);
console.log(desktopEvidence(root).verifiedLogs.length);
for (const c of [g.coverage.collection, g.gates.architecture.nestedCoverage.collection])
  console.log(coverageEvidence(root, c, id.sha256).crates);
'
```

For native readback, select the `--json` report command in each stored coverage receipt, parse its verbose `Running` argv for `llvm-cov export`, retain all 45 `-object` arguments and the existing `-instr-profile` path, and export to captured stdout. The successful diagnostic used `-ignore-filename-regex=^/Users/gracefullight/\.(cargo|rustup)/|^/rustc/`; it is a separate diagnostic, not a change to the operator's verification command or stored coverage gate. Compare the 256 stored file records by filename, requiring exact segment/integer values and allowing only the documented tiny `/percent` rounding difference; compare every function name/count. Passing that in-memory JSON to `account(storedLcov, sourceInventory(root), root, undefined, json)` must reproduce the eight-crate table above.

## Findings

### CRITICAL

None established.

### HIGH

None established.

### MEDIUM

None established.

### LOW

None established. The diagnostic warning, denied configuration access and unavailable manual/platform/historical checks are verification limits, not invented product defects. No source remediation is requested. No reproducible product blocker requires dispatch to an implementation owner.

## Verification gaps

| Unverified area | Reason and evidence boundary | Owner / next action |
| --- | --- | --- |
| Live native tray interaction | Compilation, MockRuntime callbacks and the real headless client subprocess do not establish physical tray click, close-to-hide, native reopen or macOS menu behavior. No screen check occurred in this individual review. | Chief/final-review owner: perform only the separately authorized final native interaction pass and record actual observations. |
| Windows acceptance | All current native receipts identify aarch64-apple-darwin. The bridge explicitly returns Unsupported on non-Unix platforms; this is not Windows implementation/acceptance proof. | Chief and future platform owner: obtain Windows implementation and native acceptance evidence before claiming that platform. |
| Full Phase 0 completion | This is the first desktop foundation; the SRS's wider two-OS/full-phase acceptance is not established by these receipts. | Chief: report separately and retain unverified status. |
| Native accessibility, rendered layout/contrast and performance | DOM tests establish labels, keyboard interactions, focus, live-region mutation behavior and state semantics. They do not establish VoiceOver operation, physical contrast, resizing, startup time, RSS or real-user task success. No new vulnerability audit or human/commercial study is authenticated by these gates. | Final-review/product owners: measure only within the appropriate later acceptance scope; do not relabel hypotheses or bundle size as those measurements. |
| Historical runtime evidence | 76 predecessor raw outputs, including original aggregate evidence, are unavailable locally. Current gates and coverage are fresh and independently assessed instead. | Chief/archive owner: retrieve original artifacts if historical recertification is needed; do not recreate them as historical passes. |
| Active av.yaml and external targets | Direct configuration read remains access-denied. The supplied accepted contract explicitly establishes no observation/container policy; targets.satisfied is applicability, not service health or verified configuration absence. | Chief: retain that stated basis and access limitation. No target health, deployment or recovery effect is claimed. |
| Product license and distribution | Authoritative adopted product license metadata remains unavailable and the UI says so. Dependency notices are present; an SRS proposal is not adoption. No release/distribution was part of QA. | Product owner before a future distribution decision; no fabricated license claim or unrequested distribution step. |
| Final mission delivery | Chief-owned final.md and unchanged framework contract verification follow this QA dependency. Existing gates.json still records QA as downstream; this reviewer is not authorized to rewrite it. | Chief: consume this report, complete the final reporting/verification dependency and preserve the manual/platform distinctions. |

Acceptance of task-10 concerns the accuracy and completeness of this report. All assigned review acceptance items are addressed: integrated source/assertions, final full-discovery output, native binary/profile/child bindings, eight individual coverage floors, historical separation, explicit limits and report-only ownership. Changed path: `.arc-verify/desktop-foundation/qa.md`. No mission-state mutation or product repair was performed.
