# Task 4: Python desktop contract and registered acceptance

The original handoff sections below record attempt 1. The final section, "Attempt 2 formatting repair", records the current retry's snapshot, changes and executed checks. Earlier local-HEAD, workspace and execution observations retain their original scope.

Status: completed for task-4 only. Actor backend-desktop / Desktop Contract Engineer executed as the installed backend-engineer with oma-backend. This is macOS deterministic code/CLI evidence for the Python TEST slice, not whole-mission acceptance.

## Provenance and scope

- Accepted committed source baseline: `6b94c0bd3b309ae7b5d7ce4c3ebef50b62b5e85c`; verified product tree `73ae372dc66a88b5a5a9488b8379e13ca9ac657a`.
- Actual initial snapshot and current local HEAD: `246890bce8ff3d01f818de612b5e6bad1f0d7eda`; verified tree `d41b523c72ba52635465706b923e9fc941cff405`. Baseline ancestry was checked. This snapshot contains the supplied task-2/task-3 overlay. Those files and this task's new working-tree edits are not attributed to the accepted committed baseline.
- Branch: `av-task/04608e0c24b60700-8f9ba36d8e59dd86-1`. Mission `0cf2ede2-1fe4-4ab1-88c3-c050e770f179`, task-4, attempt 1. Native thread identity from `CODEX_THREAD_ID`: `01a1dc84-e2fa-7772-969e-350c0a9b75fe`. No separate AV run ID or claim path was exposed, so none is invented.
- Supplied refs `refs/agent-valley/task-evidence/04608e0c24b60700-8f9ba36d8e59dd86-1/snapshot` and `.../worker` each returned `fatal: Needed a single revision` here. The supplied commit object/tree were verified directly; no refs or revisions were substituted or created. AV collection must bind this uncommitted delta to its delivery receipt.
- Initial scoped status/diff was empty excluding denied `av.yaml`. The denied file was neither read nor bypassed. No mission state, assignments, acceptance instructions, manifests, locks, generated declarations/JSON, shared native sources, product UI, harness, or `.arc-verify/python-tests/` evidence was edited.
- Serena is configured but has no callable tool in this run. Inspection used supplied/located paths and scoped reads. Searches of ignored logs/dependencies and outside-project memory did not substitute for project source discovery.

## Delivered changes and interface compatibility

1. `src-tauri/src/bridge/traffic_diagnostics.rs`: the existing registered read now probes `traffic.python-tests.diagnostics.capabilities` and queries `traffic.python-tests.diagnostics.job` only after the exact Python capability. An explicit unsupported probe falls back to the existing legacy diagnostics capability/query. Every probe/query uses the original shared absolute read deadline and read permit; no capability cache or execution/write retry was added. Malformed, busy, lost and unsupported-query responses retain their existing errors. A legacy-negotiated response cannot introduce Python reasons.
2. `src-tauri/tests/registered_commands_support/traffic_diagnostics.rs`: existing registered-router adversarial fixtures now model the Python-capable peer. They retain input/privacy/schema/ID/error/permit/shutdown/reprobe tests and add real protocol-1 fallback to legacy facts, legacy unresolved projections, and rejection of unnegotiated Python facts. All Rust-derived diagnostic fixtures are consumed unchanged.
3. `src-tauri/tests/traffic_python_tests.rs`: six thin registered-handler entrypoints import task-3's exact shared scenario module and the maintained MockRuntime production-handler router. The shared matching-interpreter readiness test also executes. There is no copied direct/desktop scenario implementation, custom scheduler, UI launch, browser or screen check.
4. `tests/fixtures/traffic-scripts-fixture.ts`: `pythonTraffic` selects an existing Rust-derived Python classification and validates the complete synthetic consumer fixture. It does not generate or hand-edit declarations or wire JSON.
5. `tests/traffic-scripts-contract.test.ts`: both Python reasons cross the fixed typed client for all eight admission states; malformed classification combinations, extra private fields, impossible queued permits and queued outcomes remain rejected. Existing exhaustive classification-fixture equality and no-write declaration compilation remain active.
6. `tests/traffic-scripts.test.ts`: both pytest/unittest fixtures cover TEST waiting, granted/start-unconfirmed, running, cancelled, abandoned, unconfirmed and unreserved distinctions; success/failure/signal outcomes; active shared/released permits; stale, unsupported and legacy projected reads; Unknown without fabricated success/zero; explicit refresh recovery; keyboard focus and selection; unchanged-poll/polite announcements; and confirmed pause meaning. No product UI change was necessary.

Paths above are relative to `apps/desktop/`. New Bun cases remain in two files already present in the exact retained active-22 command; that command and its manifests are unchanged. The existing strict TS decoder already received the two TEST-only reasons in task-2; this task verified its runtime behavior instead of rewriting that accepted interface. All 14 task-3 source bindings match its inherited README. See `../../../../../crates/arc-daemon/tests/traffic_python_tests/README.md` for the unchanged shared API and fixture implementation.

## Acceptance evidence

- [x] Strict generated/runtime agreement and only valid Python `TEST/HOST_LOCAL/NONE` combinations, with rejection of malformed data. Rust-generated declarations and JSON have no diff from this Actor's input snapshot.
- [x] Four required named registered tests execute real tools against production daemon fixtures in both arrival orders. Each second job has zero original launches and zero test-body entries before explicit predecessor release; each command/body enters exactly once; spare CPU/memory proves the TEST class-window cause. All complete persisted permit intervals are swept, with a TEST high-water of one and release before the successor grant.
- [x] Read-only SQLite, actual `arcctl job diagnostics/status`, registered `traffic_job_diagnostics`, actual Meter CLI and registered `meter_summary` reconcile the same IDs, classifications, native outcomes, roots, generations, resource reservations and active/released states. Wall-clock read timestamps are excluded from equality because each surface is read separately; immutable facts are compared.
- [x] Shared real-tool semantics run through registered callbacks: 18 direct/wrapped Python cases plus unresolved bypass; discovery/core selectors/dotted and file unittest forms; real console/module/venv paths; success, failure, exit 17, stdin/stdout/stderr, cwd/environment/argv/import behavior; and native signals. Tests construct their own offline projects/venv and retain ARC child coverage instrumentation.
- [x] Shared registered lifecycle coverage: global CPU contention, Governor pause/resume with fresh injected Critical pressure, queued cancellation with zero starts, crash/restart, nested permit/Meter lineage, real-daemon lost acknowledgments and unchanged legacy projections. Synthetic unsupported/hung peers remain explicitly distinct from production-daemon cases. Unknown/interrupted/unavailable Meter results are retained where tracking is uncertain.
- [x] Focused Bun, no-emission typecheck, native compilation/execution, focused Clippy, formatting and whitespace checks passed. No remaining task-4 failure or prerequisite blocker.

### Mixed TEST job receipts

| Registered scenario | First job                          | Second job                         | Starts before release | Starts / bodies | TEST high-water | Generations | Released / final active |
| ------------------- | ---------------------------------- | ---------------------------------- | --------------------- | --------------- | --------------- | ----------- | ----------------------- |
| Pytest-then-Cargo   | `43e34cb4b9a905902d510edc5bbdeb52` | `fd16a9f4c0c1c712e221041a6afd4074` | 0                     | [1, 1] / [1, 1] | 1               | [1, 2]      | 2 / 0                   |
| Cargo-then-Pytest   | `c4be7702fbbfd353c3e5aa458d8cb63f` | `454f337813721938606bc1ae464eb741` | 0                     | [1, 1] / [1, 1] | 1               | [1, 2]      | 2 / 0                   |
| Pytest-then-Bun     | `b7b05d1d0140ac155ff768837fa88744` | `d3345f78c1964d97d3993c132ec70fbb` | 0                     | [1, 1] / [1, 1] | 1               | [1, 2]      | 2 / 0                   |
| Bun-then-Pytest     | `330e9e40f1c235fd37776a4292f253d3` | `e50269d1d5861aa7be609b7407e6e7b3` | 0                     | [1, 1] / [1, 1] | 1               | [1, 2]      | 2 / 0                   |

All eight pair outcomes are recorded exit 0; all eight Meter summaries are recorded and reconcile through registered handlers. The additional global CPU pair used BUILD then Python TEST, zero second starts, one TEST high-water, two released permits. Across the entire registered native suite, **36 permits released** and all **12 owned daemon shutdowns** satisfied deadline, gauges-zero, worker-join and socket-removal assertions. Crash recovery reaped the owned prior daemon; no permit rows were manually repaired.

### Execution parity receipts

| Case                        | Registered managed job             | Native / wrapped exit |
| --------------------------- | ---------------------------------- | --------------------- |
| pytest-console-discovery    | `eecdc68a6d353c469a6aad5f1754f085` | 0                     |
| pytest-module-discovery     | `36524e4ee50f1a91a374aca95b632e14` | 0                     |
| pytest-file-node            | `0df5eef4f8ce8f6eca73266d0dbdc082` | 0                     |
| pytest-core-selectors       | `1a580e94c9ee67704627ccb421ce2b39` | 0                     |
| unittest-default            | `53b15f8d8fab3d9ea5f7719b43d2495e` | 0                     |
| unittest-discover           | `5cc75dac3b25a48c87501db828e5c8fc` | 0                     |
| unittest-discover-selectors | `aa0d9b65a4bb198f822ede660be927da` | 0                     |
| unittest-dotted-selector    | `b7975bc56b0e9dea1e37a740595b4b26` | 0                     |
| unittest-file-selector      | `2807b5c3e0674049c37c8e47adc43299` | 0                     |
| Pytest-failure              | `d832186c8e6eb7e4cca17fbab874b9f9` | 1                     |
| Pytest-stdin                | `38948dbf5a8262cf5c96f33e314020c9` | 0                     |
| Unittest-failure            | `b08d27937dc6c6742e66140073c66fb3` | 1                     |
| Unittest-stdin              | `ba23f3e6d041ed42f0ae193b5fdfc344` | 0                     |
| Pytest-exit17               | `fe81c1dfd2f9e87b97d5c64afcfd573c` | 17                    |
| Unittest-exit17             | `952dd363f47aa66fcf4f0b7d441be63a` | 17                    |
| venv-console-alias          | `c50a47238299298c525c9178e95aec28` | 0                     |
| venv-pytest                 | `eb90011c76166566f04c09ffa0f99df1` | 0                     |
| venv-unittest               | `8c7f5cd1a7b17a6710e733df5b444fb9` | 0                     |

### Recovery and compatibility receipts

- Fresh Critical Python job `02f35c8d140b51a2d6fdbf2b81d36d0d` remains unstarted until pause and then runs without a permit. OFF survives controlled restart; resumed unittest `93fff0b6a464f1bedc7f96a163db360f` receives/releases the shared reservation. The pressure source is explicitly injected into the production Meter worker, not a measurement of host stress.
- Restart: running `7075956d1987f62aff3f91928d9d3ce9`, cancelled `8ef423f840c6314a3f97199f80437dd4` (zero starts), survivor `c7f89ac4d991eb40d8ad0f9b05bb9cff`. The survivor retains its job ID and executes once. Cancelled Meter stays `unknown/interrupted`; peer-change Meter stays `unknown/unavailable` while the observed native exit remains stored.
- Signals: forwarded SIGTERM `fbc9b2b57989688bcea51a34f2009855`, child-originated SIGTERM `c0d305ae69ac3497175fca12812e7124`, pytest SIGINT `36f264ad630f06dc5ff8575d693562d9`. Native/wrapped SIGTERM is 15; pytest SIGINT is exit 2 with exact output parity. All release their permits.
- Nested root `774f65e3fb4417cbbff3dcaf149eed29` and child `efa24bf7b9c02ff152dd871cdb4531f7` share one permit. The child's recorded Meter is included in the canonical root; completion while the parent lives does not release the shared permit early.
- Lost ACK job `1234567890abcdef1234567890abcdef` starts/body-enters once and releases its permit. Enqueue, launch/start, attach and completion replies are intentionally not consumed by the authenticated raw client; callback diagnostics use registered handlers. Legacy diagnostics/lifecycle project Python to unresolved without modifying stored facts. Raw-client Meter remains explicitly unknown.
- Synthetic compatibility/outage: unsupported and hung peers started exactly one original Python body, each exit 0; first-body elapsed observations were 452 ms and 2096 ms respectively. Absent-socket unittest exited 0. These timings are observations, not substituted deadline proofs. Task-2's accepted injected-time fallback test supplies the unchanged absolute-deadline proof. Production fixture DB remained empty for these synthetic transport cases.

## Commands and outcomes

All commands ran from `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/0cf2ede2-1fe4-4ab1-88c3-c050e770f179/workspace/.agent-valley/parallel/workspaces/04608e0c24b60700-8f9ba36d8e59dd86-1` on Darwin, Rust/Cargo 1.95.0 and Bun 1.4.4. Native Cargo used `CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2`; no dependencies were added or downloaded. Real pytest 8.4.1 was verified with its matching `/opt/homebrew/opt/python@3.13/bin/python3.13`; the actual native Python image is the installed 3.13.18 framework binary. Resource inspection showed 8 logical CPUs, 16 GiB RAM and about 75 GiB free. No agents were spawned. AV's 3-task/172800-second/400-run/3-retry bounds were retained; no absolute deadline or cancellation override was exposed or reset here. Checks had no wrapper timeout shorter than the 7200000-ms contract allowance.

| Command                                                                                                                                                                               | Actual outcome                                                                                                                                                          |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo build -p arc-daemon -p arc-cli --bins`                                                                                                                                         | Exit 0; native daemon, pressure fixture and CLI available; 15.98 s                                                                                                      |
| `cargo test -p arc-desktop --test registered_commands --test traffic_python_tests -- --nocapture --test-threads=1`                                                                    | Exit 0; registered commands 29/29 (2.49 s), Python registered target 7/7 (43.79 s)                                                                                      |
| `bun test apps/desktop/tests/traffic-scripts-contract.test.ts apps/desktop/tests/traffic-scripts.test.ts apps/desktop/tests/bridge-client.test.ts`                                    | Exit 0; 50 passed, 0 failed, 2930 assertions (6.78 s). A later test-only typing correction changed the contract file; the final check below verifies that changed file. |
| `bun test apps/desktop/tests/traffic-scripts-contract.test.ts`                                                                                                                        | Exit 0; final contract-file input, 8 passed, 2495 assertions (147 ms)                                                                                                   |
| `bun run desktop:typecheck`                                                                                                                                                           | Exit 0; both configured `tsc --noEmit` passes                                                                                                                           |
| `cargo clippy -p arc-desktop --lib --test registered_commands --test traffic_python_tests -- -D warnings`                                                                             | Exit 0; focused native targets, 12.27 s                                                                                                                                 |
| `bunx --no-install biome check apps/desktop/tests/fixtures/traffic-scripts-fixture.ts apps/desktop/tests/traffic-scripts-contract.test.ts apps/desktop/tests/traffic-scripts.test.ts` | Exit 0; 3 files checked, no changes                                                                                                                                     |
| `cargo fmt --all` and focused `bunx --no-install biome format --write ...`                                                                                                            | Exit 0 after repairs; only assigned source paths differ                                                                                                                 |
| `git diff --check -- . ':!av.yaml'`                                                                                                                                                   | Exit 0                                                                                                                                                                  |
| `git diff --exit-code HEAD -- apps/desktop/src/bridge/traffic-scripts.generated.ts apps/desktop/tests/fixtures/traffic-scripts-wire.json Cargo.toml Cargo.lock package.json bun.lock` | Exit 0; generated and shared manifest inputs unchanged                                                                                                                  |

No matching successful task-4 AV receipt was supplied to reuse. Accepted task-2/task-3 integration evidence was consumed for their unchanged dependencies; their direct daemon suite was not rerun. The new registered target exercises a distinct required transport. The contract-file rerun followed an actual input change, not an unchanged successful-check duplicate. Final workspace/active-22/six gates, desktop build/native smoke, coverage collection/reuse and QA remain downstream; these focused checks do not replace them.

## Failed attempts and repairs

1. First Bun invocation: exit 1 before executing tests, missing `@tauri-apps/api/core` because this isolated worktree had no `node_modules`. The mission workspace's root/desktop manifests and `bun.lock` matched byte-for-byte. Local ignored dependency directories now point to that existing installation's entries. No package installation, download, manifest or lockfile change occurred. The initial whole-directory symlink was visible to Git, so it was replaced with ordinary ignored directories containing links; no dependency link is delivered.
2. First executable Bun run: 48 passed/2 failed. The new test tried an active permit on the Rust-generated reservation-free COMPLETED fixture. The strict decoder correctly rejected it. The repaired test completes the existing RUNNING fixture, preserving its root/generation/resources while checking active shared and released permits. Acceptance/validators were not weakened.
3. First native compile: exit 101, E0521 in the new helper because maintained `session_exchange` requires a static operation. The helper now accepts the existing static operation constants with `&'static str`. Final compilation/tests/Clippy pass.
4. First no-emission typecheck: TS2345 in a deliberately malformed fixture passed to the typed fixture helper. The test now returns an unknown wire envelope to the client instead of claiming malformed data is a `DiagnosticResult`. A transient formatter syntax error during that edit was fixed before final checks. Final typecheck and contract test pass.
5. Provided symbolic source refs were unavailable in the isolated checkout; verified supplied objects/tree and current local HEAD are reported above without inventing refs.

## Final source bindings

The six code/test paths below are the actual uncommitted task-4 delta against snapshot `246890bce8ff3d01f818de612b5e6bad1f0d7eda`. Their source content was final for the passing native/Clippy/Bun/type checks, subject only to the documented final contract-test typing correction and its final rerun. This report is written afterward and is not included in its own binding table.

| Path                                                                              | SHA-256                                                            |
| --------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| `apps/desktop/src-tauri/src/bridge/traffic_diagnostics.rs`                        | `9d1a609ebe8d5bf5e88ffbd6dc9d08fa8cb824b52402dfd807ff460dafe7b715` |
| `apps/desktop/src-tauri/tests/registered_commands_support/traffic_diagnostics.rs` | `b2ce9a230be78d48c7e05212c84b3eebb99b022426db8e0d30b4a3a0350dbeab` |
| `apps/desktop/src-tauri/tests/traffic_python_tests.rs`                            | `1caabd117a01832274deae4fb06f5219cc6fd743af68830edd227929603f8074` |
| `apps/desktop/tests/fixtures/traffic-scripts-fixture.ts`                          | `f5b838f3fe006865e761a762c30da8b1c7c93fdd02e8e422f014a567bbc3b59c` |
| `apps/desktop/tests/traffic-scripts-contract.test.ts`                             | `2d3ac8893fbe77192ae0ef1ce5889e37545e3540fdbed819dd9367ef17b09a0a` |
| `apps/desktop/tests/traffic-scripts.test.ts`                                      | `f13bc56e5ca5d330054473fcbff92a0ad2c9d3b57257c0209d09605570342a7a` |

### Unchanged consumed interface bindings

| Path                                                     | SHA-256                                                            |
| -------------------------------------------------------- | ------------------------------------------------------------------ |
| `apps/desktop/src/bridge/traffic-scripts-contract.ts`    | `fa3512c2488f11fa63f336eec67b3c1550ff0e73efc26bf8fa11a943ab5cdfb8` |
| `apps/desktop/src/bridge/traffic-scripts.generated.ts`   | `20dbcbb726a90b590e126a42734320eef1de58796810daad39fb0218ab32c43a` |
| `apps/desktop/tests/fixtures/traffic-scripts-wire.json`  | `9cc3dac10309b2ec8b472e0720a4b27e5466254e372461af38a06a867dc44633` |
| `crates/arc-daemon/tests/traffic_python_tests/README.md` | `68348a804b3f4e23534d21df007d968e8957e11b02b150a3ee800582653884af` |
| `crates/arc-daemon/tests/traffic_python_tests/mod.rs`    | `e46f9ef498b9ce4aec4eee9680894adace7b4309b6c53e5287ec9538f0202c4a` |
| `.arc-verify/python-tests/contract.md`                   | `aea1e29eaf18b40cb223a1fc1a8620e8fcfb00919ced2ac23ae89152d327d045` |

### Local output digests

Raw logs and scenario JSON are retained in ignored `target/` for this local run only; they are not the handoff transport. The Git-visible job/assertion tables, command outcomes and source hashes in this report are the durable handoff. Later tasks must verify the integrated files and AV delivery receipt rather than assume these ignored logs move between workspaces.

| Local log under `target/python-desktop-checks/` | SHA-256                                                            |
| ----------------------------------------------- | ------------------------------------------------------------------ |
| `native-build.log`                              | `001780b6245b3ce2b1a1171012b3f005bab6d5078d8bdae10532d8404710bd52` |
| `native-tests.log`                              | `95ddf061e8474b8658279bbbe96e277d766695e169b449516c510ce2c31bb6b0` |
| `native-tests-attempt1.log`                     | `4ef5cd1c10d6e5616000b78a225f9b176ab9e97d6c597ceb3acac8485eac86d9` |
| `bun-desktop.log`                               | `48888e4889d255419a19ffc6f1a5f04bcaa1fb60de2b4fc0bda715df75db9eca` |
| `bun-contract-final.log`                        | `2e3af384a7e04eb4c262613fbf11d2de6adb79c190d2e8618dc3062d4842c136` |
| `typecheck.log`                                 | `7c3cb6045e4508ad4ca264a0bdbb3207275c78410b843909c0ea071a97268c0f` |
| `typecheck-attempt1.log`                        | `3f59183fc007a9061c1927daa0c20d21e8c6edf05e129cbb0833a93c19d362ec` |
| `biome.log`                                     | `5b7ddf307cfbdc66638fd86875c61dd9cdbbf294be200ef2b26b99c1625b5971` |
| `clippy.log`                                    | `6a037231185d116c3251cc5eda95e6124a1d9dd96a6c0a516ce43f5dbe3cb0b0` |

## Handoff and remaining limits

Canonical material artifact: `apps/desktop/src-tauri/tests/traffic_python_tests/README.md` (this non-ignored regular file). A local `.agents/results/` mirror follows the installed result filename convention, using the exposed native thread identity rather than an invented AV run ID.

No task-4 blockers or ownership/interface conflicts remain. The registered bridge change stays within the assigned desktop boundary; no production repair outside that boundary was required. The exclusive backend gate Actor must integrate this Git-visible delta, consume applicable current-input AV successes, complete the remaining documentation/traceability and all final gates, collect final-input coverage once and reuse it in the unchanged harness, then hand final inputs to independent oma-qa. Those final gates, 80% per-crate coverage, desktop packaging/smoke and independent QA are not claimed here. No production deployment, publishing, browser E2E, GUI or screen-based check occurred.

Skills used: installed oma-backend, with its execution protocol, checklist, code-intelligence fallback and shared execution/result contracts. Authentication/schema/DB-migration checklist items were not changed by this read-only diagnostics integration; existing authenticated sessions and the single maintained writer remain authoritative.

## Attempt 2 formatting repair

Status: completed for the task-4 repair. The Chief-supplied AV integration failure was `cargo test --workspace` -> `arc-desktop` / `traffic_bun_scripts` / `registered_bridge_repository_desktop_build_typecheck_lint`: `bun run desktop:check` rejected six unformatted tables in this README. The entire README was formatted with the installed Biome 2.6.6. No formatter scope, test, acceptance criterion, generated contract, manifest, lockfile or implementation was changed.

### Current source provenance

- Accepted committed source baseline remains `6b94c0bd3b309ae7b5d7ce4c3ebef50b62b5e85c`, tree `73ae372dc66a88b5a5a9488b8379e13ca9ac657a`.
- This attempt's actual inspected snapshot/current local HEAD is `37cdc08523d7d374694cb30de18ad1789bbffaf4`, tree `440f99605fa868d75ffdadbd01e66e642ecb29af`; baseline ancestry passed. Branch: `av-task/04608e0c24b60700-8f9ba36d8e59dd86-2`. This isolated snapshot includes previous task overlays; those changes are not attributed to the committed baseline or presumed present in a later review checkout.
- Mission `0cf2ede2-1fe4-4ab1-88c3-c050e770f179`, task-4, attempt 2. Exposed native thread identity: `01a1dc96-8c76-7281-aa08-fff9ba28b378`. No separate AV run ID, claim path, absolute deadline or cancellation override was exposed. Existing mission duration/run/retry limits were retained.
- Supplied `refs/agent-valley/task-evidence/04608e0c24b60700-8f9ba36d8e59dd86-2/snapshot` and `.../worker` were unavailable (`fatal: Needed a single revision`). The supplied commit and tree were verified directly; refs were not created or substituted. The pending AV delivery receipt must bind the actual local README delta.
- Initial scoped status was clean excluding denied `av.yaml`; it was not read or bypassed. This README is the only Git-visible file changed in this retry. The installed oma-backend skill and execution/result/checklist references were used. Serena had no callable tools; reads stayed on supplied paths, Git objects, plain-text reports and ignored artifacts.

The snapshot README SHA-256 was `77baf7fe72709f830a32b82f9b1240476849d4942c3ba75a473872733d2c0221`; immediately after full-file formatting, before this addendum, it was `2c71996d31a45f24cc8e59ecee7dde771fdfb09d36ae1ca8b89072bfc12318c6`. A deterministic comparison stripped table-cell padding and separator widths and proved all original evidence content equal. All 12 implementation/interface/dependency hashes in the original source-binding tables also matched current files. Only historical-scope clarification and this repair evidence were then added, with the entire final file formatted again.

### Executed checks and repaired prerequisite

Commands ran from this attempt's workspace:

`/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/0cf2ede2-1fe4-4ab1-88c3-c050e770f179/workspace/.agent-valley/parallel/workspaces/04608e0c24b60700-8f9ba36d8e59dd86-2`.

Host readiness was 8 logical CPUs, 16 GiB RAM and about 75 GiB free; Bun 1.4.4 and Cargo 1.95.0. Cargo ran serially with `CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2`; wrappers allowed 7200 seconds per check, matching the mission's 7200000-ms allowance. No agents, installs or downloads were used.

This isolated retry lacked `node_modules`. The root and desktop manifests plus `bun.lock` matched the existing mission installation byte-for-byte. Ordinary ignored local dependency directories were populated with links to that installation. The first native test compiled but exited 101 before reaching the formatting check: Vite could not write to the linked `.vite-temp` cache (`EPERM`). Only this retry-created cache symlink was replaced with an ordinary local ignored cache directory; the source installation/cache and project configuration were preserved. The identical failed-test command then passed. This was a local fixture prerequisite repair, not a product/test change.

- `bunx --no-install biome format --write apps/desktop/src-tauri/tests/traffic_python_tests/README.md`: exit 0; entire README formatted with installed Biome.
- `bun run desktop:check`: exit 0; 84 files checked, no fixes; 2026-11-15 00:24:34.766786-00:24:34.967657 UTC, 0.201 s. Log SHA-256: `29db8629f77d03b36becd7d0ca5b3b6bdd8f565f00ce1bfd673e321331357f81`.
- `cargo build -p arc-daemon -p arc-cli --bins`: exit 0; local native fixture prerequisites compiled offline; 00:24:53.079559-00:25:07.577496 UTC, 14.498 s. Log SHA-256: `1d26cf0565c78958f0255debe94a916a388f4e75a24dfb9df58bb63e54bb24e6`.
- `cargo test -p arc-desktop --test traffic_bun_scripts registered_bridge_repository_desktop_build_typecheck_lint -- --exact --nocapture --test-threads=1`: first exit 101 due to the cache prerequisite above; 00:25:26.923027-00:25:54.035803 UTC, 27.112 s. Preserved failure-log SHA-256: `cddb2f1e8812c606ce0918710a4d7c76bf75ba8c74efd932f89aad548dfe68ed`.
- The identical native test command after cache repair: exit 0, **1 passed, 0 failed, 16 filtered out**; 00:26:16.250791-00:26:21.219099 UTC, 4.968 s (test body 4.35 s). Log SHA-256: `61446f18f339e8fcd38f851177770bed7db373cd1b1efcec8a16a9e1ff6e1ecb`.
- `git diff --check -- . ':!av.yaml'` and the original generated/shared-input `git diff --exit-code HEAD -- ...` comparison (also including `biome.json`): exit 0.

The native test used unchanged production handlers and the production daemon. Its successful scenario reconciled these exact jobs through SQLite, CLI status/diagnostics, registered desktop diagnostics and Meter:

| Native argv                 | Job ID                             | Class / reason                   | Original starts | Permit generation / final state | Outcome / Meter   |
| --------------------------- | ---------------------------------- | -------------------------------- | --------------- | ------------------------------- | ----------------- |
| `bun run desktop:build`     | `475465a24d9200ea2f2b5e7caed46dfe` | BUILD / BUN_SCRIPT_BUILD         | 1               | 1 / RELEASED, inactive          | exit 0 / recorded |
| `bun run desktop:typecheck` | `7431209113802c4985537f134d491821` | TYPECHECK / BUN_SCRIPT_TYPECHECK | 1               | 2 / RELEASED, inactive          | exit 0 / recorded |
| `bun run desktop:check`     | `bc1d88e0e854af528847fd400c699365` | LINT / BUN_SCRIPT_LINT           | 1               | 3 / RELEASED, inactive          | exit 0 / recorded |

The nested typecheck executed both configured `tsc --noEmit` passes; the nested check covered all 84 desktop files. Meter correctly retained partial descendant completeness, rather than claiming full resource attribution. Shutdown recorded `deadline_met`, `gauges_zero`, `worker_joined`, `handlers_reaped` and `socket_removed` all true. The fixture's local observations/logs remain under ignored `target/python-desktop-repair-checks/`; this README carries the durable handoff facts, so delivery does not depend on ignored artifacts moving between workspaces.

### Acceptance and handoff

- [x] The entire handoff README is formatted without dropping or changing its original evidence.
- [x] The full existing desktop check passes without changing its scope.
- [x] The exact previously failed registered Rust test compiles and passes after the local cache prerequisite repair; all three real nested commands execute once and release their permits.
- [x] All inherited Python implementation, compatibility, fixture and generated-interface hashes remain unchanged. The earlier passing Python pair/semantics/lifecycle, registered command and focused Bun receipts are retained with their original attempt-1 scope; they were not redundantly rerun for this Markdown repair.

No task-4 blocker or interface/ownership conflict remains. AV must replay its failed integration check against the integrated repair. This retry does not claim a successful full workspace run, final six gates, final-input coverage, standalone desktop native smoke or independent QA; those remain assigned downstream. The nested desktop build above belongs to this required regression test. No mission state, shared manifest, lockfile, source contract, harness or `.arc-verify/python-tests/` file was edited, and no external delivery or screen/browser checks occurred.
