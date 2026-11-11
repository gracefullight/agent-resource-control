# Task-5 gate delivery, attempt 9

Local gate outcome: PASS. Framework acceptance remains pending its external rerun; no external pass is claimed. The operator's binding 13:37 active-suite scope applies. The assigned backend repair used **oma-debug** and **oma-backend**; the independent reviewer uses **oma-qa** and owns `qa.md` / `audit.json`.

## Result and diagnosis

The framework finally identified the two failures: `fallback_descriptors::fallback_preserves_closed_standard_and_extra_inherited_descriptors` and `scenarios::native_non_rust_child_observes_literal_closed_standard_descriptors`. Both failed in direct execution with descriptor 0 unexpectedly open. The saved diagnostic excerpt is under `target/terminal-outcomes/task-5-attempt-9/saved-verification-failure.txt`; its provenance is the supplied assignment, not a new external observation.

The same tests failed locally before this repair with **offline native pinned Cargo, ignored stdin, no TTY and a new process group**. The raw RED receipt and streams are `descriptors-red.json`, `descriptors-red.stdout.log` and `descriptors-red.stderr.log` under this attempt's target directory. The independent probe showed PATH `python3` resolves to a mise launcher which reopens closed stdin before Python starts: the launcher reported `[true,true,true]` while the resolved interpreter reported `[false,true,true]` for exactly the same closure. The previous tool-manager Cargo launch environment placed the interpreter ahead of that launcher and masked the fixture problem.

A shared test helper resolves `sys.executable` with all standard streams open, preserving its filesystem bytes with `os.fsencode`, and both direct and wrapped probes then execute that same interpreter. The two original tests passed GREEN in the formerly failing environment. No descriptor mask, inherited-fd data, exit-17 assertion, stream assertion, marker-count assertion, fault event, or direct/wrapped comparison was relaxed. Related descriptor restoration and signal fixture code was inspected through known-path reads; no additional affected pattern was found in that inspected scope. A complete code search was unavailable because Serena exposes no tools.

The first complete workspace run then passed all 59 managed-exec tests but exposed another launcher-dependent fixture failure, `semantics::real_bun_preserves_caller_environment_and_receives_missing_lineage`. Its strict environment maps differed in the launcher's process-specific `__MISE_SHIM_FALLBACK_CHAIN.pid`. A focused RED reproduced this. The repair reuses the existing canonical `bun_measurement::real_bun` helper, making it visible to its sibling test and prepending its native parent directory to the common caller PATH. Both commands still execute bare `bun test`; every environment, four-lineage-case, completion-count and permit assertion remains unchanged. Focused GREEN passed. The first workspace failure and both focused receipts are retained under target, and the required gates were refreshed after this additional two-file test repair.

The bug diagnoses and RED/GREEN evidence are consolidated into this assigned report, rather than duplicating them in another tracked bug report. There is no injected OMA claim/run identity; AV mission state and saved assignment were left untouched.

## Changed paths and installation

The accepted task-1 implementation, task-2/task-4 reports, and task-3 documentation were already installed. All **18 accepted task-1 source hashes still match**. The only source-tree changes in this attempt are the following test repair overlay, explicitly addressing the named verification failures:

- `crates/arc-daemon/tests/managed_exec.rs` — SHA-256 `a800d31275f508c798c3816a643bfb7705598a6607b4d5d1e8f7548b086420cf`.
- `crates/arc-daemon/tests/managed_exec/native_python.rs` — SHA-256 `cfef16f935f769d37d777325d819f92f6531b8f57418b293c174caf8d698158a`.
- `crates/arc-daemon/tests/managed_exec/fallback_descriptors.rs` — SHA-256 `c406da97101bf1c5a29cbf3dcadfb72c91c38e035a40e36783ea1c6a6cba245a`.
- `crates/arc-daemon/tests/managed_exec/scenarios.rs` — SHA-256 `3f5c1d3f16414aafce8656b2547aaee09e1b7c8dfa5074356c979f80219b8f89`.
- `crates/arc-daemon/tests/traffic_cli/bun_measurement.rs` — SHA-256 `676d94e956a0904367501a474ed13af6a688f388466dd5fbb9503c2ddd8a3a78`.
- `crates/arc-daemon/tests/traffic_cli/semantics/environment.rs` — SHA-256 `0d247bfa5922ab294fb0cd9f10bbd5f5440d35157862a2b891692dea51e9890d`.

This overlay was staged by explicit path so the existing all-tracked harness includes the new helper. Existing staged and unstaged Actor work was preserved. Current collector identity is `17fe50cbd7d5f9d332e16616723cc68037b5c111f772a73a0c4abed57afa01a4` over 249 files, from the unchanged `scripts/coverage/source.ts` worktree-byte collector. Its integrated extension additionally binds the docs, Bun discovery, harness and archived tools. Both manifests and the installation audit are hashed in `gates.json`; the before/after collector manifests match.

Protocol-1 wire definitions, all existing migration bytes/checksums, Cargo manifests and lockfile still match accepted HEAD `6b5218cf63b174b7976e52d81d56c7e3bf81112d`. The test-only repair adds no runtime interface or dependency. Protocol compatibility remains covered by the seven focused old-peer/current-peer status tests. Old-daemon same-database downgrade was not executed; the accepted decoder does not read new durable terminal names. The prior `JobError::UnsupportedStatus` exhaustive-match disclosure remains applicable to the earlier implementation.

## Commands and concrete outcomes

All commands ran on Darwin. Each replay/wrapper allowed 7,200,000 ms per contract command within the retained absolute deadline `2026-11-11T20:44:09.825567Z`. Native gate concurrency was one; no skip or instrumentation override was set. Current raw stdout/stderr and command receipts are under `target/terminal-outcomes/task-5-attempt-9/`, each referenced by path, bytes and SHA-256 in `gates.json`.

| Command | Outcome (exit) | Elapsed | stdout / stderr SHA-256 |
| --- | --- | --- | --- |
| `cargo test -p arc-daemon --test managed_exec closed_standard -- --nocapture` | FAIL (101) | 0.942 s | `5feae15af8eb3042631b287d25918b9abe452ee9dd09fe972579d17a0915cdce` / `d5e958f3d0c08bae8106f3f332aeaf053d0c4f52cf7809a6a4133fe17702e836` |
| `cargo test -p arc-daemon --test managed_exec closed_standard -- --nocapture` | PASS (0) | 62.155 s | `c9b140302073e5c77270f624cf9810f5bf7ac294a33058ebcd2f496deda1a40d` / `ba61fec4715d09371424308372082e14eb62d2397b88be27eed8e3abd4886651` |
| `cargo build --workspace --bins` | PASS (0) | 0.197 s | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` / `fecede7c2157efe2f16c8c8b9dce94b0fb4dd384dca8207046c8bf7636eb0ffe` |
| `cargo fmt --all -- --check` | PASS (0) | 0.617 s | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` / `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS (0) | 1.064 s | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` / `59fcb6b74d1095b13aff3d6fec6c4b60b024563df97a38caa62dddb33bb520bd` |
| `cargo test -p arc-daemon --test traffic_cli terminal_outcomes` | PASS (0) | 2.027 s | `9ede67c4c8cb6b488687532112386001b5eb4b84158661fc63286102a595ee9c` / `14be661449cbc2e7f7ad2905bc5b84f8cbf43202ed701b9bdca3e50adb06a1fc` |
| `cargo test -p arc-daemon --test traffic_cli status_compat` | PASS (0) | 3.703 s | `1ab6835b2b1c8a235628ec666971e544682bb6d00fc92b5010d00b720ea64191` / `7bfc0055cad81b705c4089f0819a34667b2001200c40abfa68979af7c2b3d00e` |
| `cargo test --workspace` | FAIL (101) | 197.444 s | `32ea5651904460779ab46e28529afdc6066fddef35a7717f2e5449666be897fd` / `68352a620cb9e0447ccb9baf7b6c1a0dde68ba4fb7b19681c3af9ecf26a12fd8` |
| `cargo test -p arc-daemon --test traffic_cli real_bun_preserves_caller_environment_and_receives_missing_lineage -- --nocapture` | FAIL (101) | 0.870 s | `ec914e4140e0214b7eff71aa4d2ee59390265ce983d7d513a8a846fb0f0f6780` / `9ba06aa70f53b667ade2ae96e6371b51af91541a21ed720df804e7a8ee7a3339` |
| `cargo test -p arc-daemon --test traffic_cli real_bun_preserves_caller_environment_and_receives_missing_lineage -- --nocapture` | PASS (0) | 2.787 s | `9e090d68a27694418fbb986b945a1ed539fdf98ce0c560b5dcbbfc2f379f759a` / `2fc64df29d92131b6fc4c37f529897882cb58a9eb5e3470cc480e3c9ca5b49fb` |
| `cargo build --workspace --bins` | PASS (0) | 0.261 s | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` / `d7dae192715495fa89ccbe279af3efb9792c1dae1768228e369035e68a1df673` |
| `cargo fmt --all -- --check` | PASS (0) | 0.730 s | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` / `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS (0) | 1.395 s | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` / `60bdff372191fb52a5a312b9e1a7e68f09b8ef92d2d8080317ca13c25ab6419b` |
| `cargo test -p arc-daemon --test traffic_cli terminal_outcomes` | PASS (0) | 2.484 s | `345b68216db48de06b08a5716f043ba06ebdf9141f5e264cf6b90b6511df1096` / `14be661449cbc2e7f7ad2905bc5b84f8cbf43202ed701b9bdca3e50adb06a1fc` |
| `cargo test -p arc-daemon --test traffic_cli status_compat` | PASS (0) | 3.701 s | `b976b265a81b18b3f92c43f8f98fa51ba8ac89e7f6129ed4bb35ccbe552754eb` / `7bfc0055cad81b705c4089f0819a34667b2001200c40abfa68979af7c2b3d00e` |
| `cargo test --workspace` | PASS (0) | 203.274 s | `b4558b2032503b822f6d65db8f2446b5b76bf2267983bf120325abdf36ed8c41` / `24f7b72405c13d87230895d0cca207142b211111df689bdfd918be4b9a3fce05` |
| `./scripts/harness/validate.sh` | PASS (0) | 261.212 s | `8ac1008436339acd5e13f5662380e23b32fb1ce3adf1794b58c62a446e27facc` / `bc5967d975ba228cabd93d8a140bce15cff6f86eb0c2c1aa191ea6b600dd6233` |
| `bun test` | PASS (0) | 53.055 s | `d4d16539b9c124d2391dbf77305b2591ef74bec1fa5452918e2458c2038f4f13` / `53e99172cf3a97a8db7393a2887c9d50497cb07e293fd35a1febdac12d668472` |

`cargo test --workspace` passed **591 tests**, with zero failed, ignored or filtered tests. The 10 focused terminal-outcome tests passed, including both named macOS exit-17 restart cases; each verifies one fixture invocation, exact stderr and wrapper exit 17, independently persisted FAILED, restart durability and both status views. Exit 0, retries, permit release, historical COMPLETED records and unavailable outcomes retain executable coverage. The focused status-compatibility selection passed seven tests, including transient DB_BUSY and known/unavailable outcome cases.

The complete `./scripts/harness/validate.sh` ran all five checks without skips and executed the existing `bun run test:coverage` runner successfully. Its fresh run ID is `2026-11-11T15-22-04-297Z-d667dff5-6781-493e-9fea-edce3b6585b4`, bound to the current source hash and native unit/integration/child-binary logs. Coverage counts were taken from its LCOV and LLVM JSON accounting, with every crate above the unchanged 80% production-line floor:

| Crate | Covered / total production lines | Coverage | Result |
| --- | --- | --- | --- |
| `arc-core` | 247/256 | 96.48% | PASS |
| `arc-detector` | 706/711 | 99.30% | PASS |
| `arc-storage` | 1811/1892 | 95.72% | PASS |
| `arc-ipc` | 569/576 | 98.78% | PASS |
| `arc-daemon` | 2244/2457 | 91.33% | PASS |
| `arc-cli` | 1895/2255 | 84.04% | PASS |
| `arc-traffic` | 111/111 | 100.00% | PASS |

The active `bun test` suite status is **PASS**; counts: `{"failed": 0, "files": 7, "passed": 43, "skipped": 0, "tests": 43}`. It includes the unchanged current mission validator, which independently checks source identity, native logs, fresh coverage accounting, compatibility and evidence/archive hygiene. No retired historical validator or fixture was restored or repaired. The retained authorized archive inventory follows below.

## Evidence, independent review and remaining limits

Previous gate/QA/audit reports were copied byte-for-byte to immutable `before-*` snapshots under this attempt's target directory and hashed. Earlier failures and their provenance remain available there; current native checks supersede prior source/freshness claims. Historical dependency logs retain the operator-accepted Actor-workspace provenance; their absence locally is not treated as new proof.

Independent QA issued **PASS** after reviewing both fixture repairs and authenticating all required local gates. Its immutable verdict is `target/terminal-outcomes/task-5-attempt-9-independent-qa/independent-verdict.json` (SHA-256 `bc061f6704da770125f10e36c6a38c0e194a234f2e1691669831d678297149df`). The reviewer independently executed `bun test scripts/coverage/terminal-outcomes.test.ts`: four passed, zero failed, 439 expectations. All 117 direct phase-one gate references, 249 collector inputs, 18 accepted implementation hashes and 20 baseline interface bindings matched. Final verdict bindings and exact evidence count/byte audit are in the reviewer-owned `qa.md` and `audit.json`. These gate files do not sign a verdict for that reviewer. The seven mission evidence files remain below 30 files and 5,000,000 bytes, with raw logs under target. Exact final bytes belong to `audit.json` after the gate reports freeze.

The binding operator inventory establishes **zero configured container/service targets**: `configurationVerified=true`, `allTargetsHealthy=true`, `targetCount=0`, `targets=[]`. Health is true over that verified empty set; no runtime observation or restart is fabricated. The previously denied `av.yaml` was not reread or bypassed, and no complete global worktree-cleanliness claim is made. No AV lifecycle operation, mission graph/instruction change, dependency installation, browser/GUI/Playwright action, publication or other external delivery occurred.

The named framework defect is now reproduced, diagnosed and repaired with local passing evidence. **The framework's own final rerun remains pending.** No additional local replay is presented as its external acceptance result.

## Authorized archive inventory retained from earlier installation

The binding 13:37 decision retired historical artifact-authentication validators. The original inventory retains all 69 files, including 28 historical tests, with original/archived hashes and bytes. No archive file changed this turn. `scripts/archive/**` remains excluded by Bun; seven current suites remain active.

| Original path | Archived path |
| --- | --- |
| `scripts/harness/exec-fail-open-preservation.ts` | `scripts/archive/harness/exec-fail-open-preservation.ts` |
| `scripts/harness/exec-fail-open-collection.ts` | `scripts/archive/harness/exec-fail-open-collection.ts` |
| `scripts/harness/exec-fail-open-recipe.test.ts` | `scripts/archive/harness/exec-fail-open-recipe.test.ts` |
| `scripts/harness/exec-fail-open-preservation.test.ts` | `scripts/archive/harness/exec-fail-open-preservation.test.ts` |
| `scripts/harness/exec-fail-open-receipts.ts` | `scripts/archive/harness/exec-fail-open-receipts.ts` |
| `scripts/harness/exec-fail-open-reads.ts` | `scripts/archive/harness/exec-fail-open-reads.ts` |
| `scripts/harness/exec-fail-open-recipe.ts` | `scripts/archive/harness/exec-fail-open-recipe.ts` |
| `scripts/harness/exec-fail-open-docs.ts` | `scripts/archive/harness/exec-fail-open-docs.ts` |
| `scripts/harness/managed-exec-test-output.ts` | `scripts/archive/harness/managed-exec-test-output.ts` |
| `scripts/harness/exec-fail-open-docs.test.ts` | `scripts/archive/harness/exec-fail-open-docs.test.ts` |
| `scripts/harness/exec-fail-open-compact-flow.test.ts` | `scripts/archive/harness/exec-fail-open-compact-flow.test.ts` |
| `scripts/harness/exec-fail-open-evidence.test.ts` | `scripts/archive/harness/exec-fail-open-evidence.test.ts` |
| `scripts/harness/exec-fail-open-coverage.ts` | `scripts/archive/harness/exec-fail-open-coverage.ts` |
| `scripts/harness/exec-fail-open-inputs.ts` | `scripts/archive/harness/exec-fail-open-inputs.ts` |
| `scripts/harness/exec-fail-open-traversal.ts` | `scripts/archive/harness/exec-fail-open-traversal.ts` |
| `scripts/harness/managed-exec.test.ts` | `scripts/archive/harness/managed-exec.test.ts` |
| `scripts/harness/exec-fail-open-history.ts` | `scripts/archive/harness/exec-fail-open-history.ts` |
| `scripts/harness/exec-fail-open-gates.test.ts` | `scripts/archive/harness/exec-fail-open-gates.test.ts` |
| `scripts/harness/exec-fail-open-assemble.ts` | `scripts/archive/harness/exec-fail-open-assemble.ts` |
| `scripts/harness/exec-fail-open-fixture.ts` | `scripts/archive/harness/exec-fail-open-fixture.ts` |
| `scripts/harness/exec-fail-open-fixture-recipe.ts` | `scripts/archive/harness/exec-fail-open-fixture-recipe.ts` |
| `scripts/harness/managed-exec-evidence.test.ts` | `scripts/archive/harness/managed-exec-evidence.test.ts` |
| `scripts/harness/managed-exec-evidence.ts` | `scripts/archive/harness/managed-exec-evidence.ts` |
| `scripts/harness/exec-fail-open-gates.ts` | `scripts/archive/harness/exec-fail-open-gates.ts` |
| `scripts/harness/exec-fail-open-native.ts` | `scripts/archive/harness/exec-fail-open-native.ts` |
| `scripts/harness/exec-fail-open-receipts.test.ts` | `scripts/archive/harness/exec-fail-open-receipts.test.ts` |
| `scripts/harness/exec-fail-open-graph.ts` | `scripts/archive/harness/exec-fail-open-graph.ts` |
| `scripts/harness/exec-fail-open-container.ts` | `scripts/archive/harness/exec-fail-open-container.ts` |
| `scripts/harness/performance.test.ts` | `scripts/archive/harness/performance.test.ts` |
| `scripts/harness/exec-fail-open-cross-workspace.test.ts` | `scripts/archive/harness/exec-fail-open-cross-workspace.test.ts` |
| `scripts/harness/exec-fail-open-compact.test.ts` | `scripts/archive/harness/exec-fail-open-compact.test.ts` |
| `scripts/harness/exec-fail-open-compact-fixture.ts` | `scripts/archive/harness/exec-fail-open-compact-fixture.ts` |
| `scripts/harness/managed-exec-coverage-fixture.ts` | `scripts/archive/harness/managed-exec-coverage-fixture.ts` |
| `scripts/harness/exec-fail-open-compact-archive-fixture.ts` | `scripts/archive/harness/exec-fail-open-compact-archive-fixture.ts` |
| `scripts/harness/exec-fail-open-native-extra.ts` | `scripts/archive/harness/exec-fail-open-native-extra.ts` |
| `scripts/harness/coverage-verification.test.ts` | `scripts/archive/harness/coverage-verification.test.ts` |
| `scripts/harness/exec-fail-open-native-extra.test.ts` | `scripts/archive/harness/exec-fail-open-native-extra.test.ts` |
| `scripts/harness/benchmark.ts` | `scripts/archive/harness/benchmark.ts` |
| `scripts/harness/exec-fail-open-inventory.ts` | `scripts/archive/harness/exec-fail-open-inventory.ts` |
| `scripts/harness/exec-fail-open-compact-historical-fixture.ts` | `scripts/archive/harness/exec-fail-open-compact-historical-fixture.ts` |
| `scripts/harness/exec-fail-open-compact.ts` | `scripts/archive/harness/exec-fail-open-compact.ts` |
| `scripts/harness/evidence-budget.test.ts` | `scripts/archive/harness/evidence-budget.test.ts` |
| `scripts/harness/exec-fail-open-evidence.ts` | `scripts/archive/harness/exec-fail-open-evidence.ts` |
| `scripts/harness/traffic-evidence.test.ts` | `scripts/archive/harness/traffic-evidence.test.ts` |
| `scripts/harness/managed-exec-coverage-identity.test.ts` | `scripts/archive/harness/managed-exec-coverage-identity.test.ts` |
| `scripts/harness/managed-exec-immutability.test.ts` | `scripts/archive/harness/managed-exec-immutability.test.ts` |
| `scripts/coverage/traffic-bun-qa.ts` | `scripts/archive/coverage/traffic-bun-qa.ts` |
| `scripts/coverage/hygiene.test.ts` | `scripts/archive/coverage/hygiene.test.ts` |
| `scripts/coverage/traffic-measurement.ts` | `scripts/archive/coverage/traffic-measurement.ts` |
| `scripts/coverage/traffic-evidence.ts` | `scripts/archive/coverage/traffic-evidence.ts` |
| `scripts/coverage/preservation.test.ts` | `scripts/archive/coverage/preservation.test.ts` |
| `scripts/coverage/current-preservation.ts` | `scripts/archive/coverage/current-preservation.ts` |
| `scripts/coverage/traffic-bun-measurement.ts` | `scripts/archive/coverage/traffic-bun-measurement.ts` |
| `scripts/coverage/traffic-bun-qa.test.ts` | `scripts/archive/coverage/traffic-bun-qa.test.ts` |
| `scripts/coverage/traffic-bun-evidence.ts` | `scripts/archive/coverage/traffic-bun-evidence.ts` |
| `scripts/coverage/hygiene.ts` | `scripts/archive/coverage/hygiene.ts` |
| `scripts/coverage/clean-checkout.test.ts` | `scripts/archive/coverage/clean-checkout.test.ts` |
| `scripts/coverage/hygiene-preservation.test.ts` | `scripts/archive/coverage/hygiene-preservation.test.ts` |
| `scripts/coverage/historical.ts` | `scripts/archive/coverage/historical.ts` |
| `scripts/coverage/preservation.ts` | `scripts/archive/coverage/preservation.ts` |
| `scripts/coverage/traffic-accounting.test.ts` | `scripts/archive/coverage/traffic-accounting.test.ts` |
| `scripts/coverage/evidence.test.ts` | `scripts/archive/coverage/evidence.test.ts` |
| `scripts/coverage/acceptance.ts` | `scripts/archive/coverage/acceptance.ts` |
| `scripts/coverage/traffic-bun-evidence.test.ts` | `scripts/archive/coverage/traffic-bun-evidence.test.ts` |
| `scripts/coverage/traffic-evidence-contract.md` | `scripts/archive/coverage/traffic-evidence-contract.md` |
| `scripts/coverage/harness.test.ts` | `scripts/archive/coverage/harness.test.ts` |
| `scripts/coverage/coverage-cli.test.ts` | `scripts/archive/coverage/coverage-cli.test.ts` |
| `scripts/coverage/traffic-receipts.ts` | `scripts/archive/coverage/traffic-receipts.ts` |
| `scripts/coverage/schema-adaptations.ts` | `scripts/archive/coverage/schema-adaptations.ts` |

## Acceptance checklist

- [x] Accepted artifact installation verified against actual worktree bytes.
- [x] Requested compilation and deterministic native/Bun outcomes retained with commands and hashed streams.
- [x] Seven 80% coverage floors and unskipped full harness authenticated.
- [x] Verified empty container inventory represented as instructed.
- [x] Actual independent bounded PASS obtained; reviewer owns final QA/audit refresh.
- [x] Other Actors work, product/tests, instructions and mission state preserved.
- [ ] Final mission acceptance: unchanged external workspace rerun remains pending for the framework.
