# Task 5: coverage and evidence validator repair

Status: **partial**. The owned validator and focused-fixture changes pass on this Actor's source. The historical suite, coverage gate, full harness and live coverage evidence check failed for the reasons below. This is not a Traffic mission pass.

## Source and ownership

The accepted committed baseline is `4d2f71c1a1482925f6c9918296d7ceb3435b7592`, tree `4455896fdf976244313a70a9fa38b730b344b411`. This Actor's initial snapshot and checked-out HEAD is `30087db5c4628c62beba3b13a6b0e0eae4af5d22`, tree `df5fd3f3f6f9ec77747dddc1ea28fb0e3387246c`. Its snapshot ref resolves in the mission source repository to that commit; the supplied worker ref was unavailable there at inspection. The source-input hash for the final checks, before and after each command, is `65b3054288fa76e38a6cc406591d0929f610f127ace17916b23dca3430096619`. The delivered edits are uncommitted Actor-local changes on the snapshot; verify their bytes and rerun after integration. They are not claimed to be in the accepted commit.

Changed paths: `scripts/coverage/current-preservation.ts`, `scripts/coverage/schema-adaptations.ts`, `scripts/coverage/traffic-evidence-contract.md`, `scripts/coverage/traffic-evidence.ts`, `scripts/coverage/traffic-measurement.ts`, `scripts/coverage/traffic-receipts.ts`, `scripts/harness/exec-fail-open-preservation.ts`, `scripts/harness/exec-fail-open-preservation.test.ts`, `scripts/harness/traffic-evidence.test.ts`, and this report. No product code, Cargo manifest, lockfile, `av.yaml`, historical receipt, or `scripts/harness/validate.sh` was edited. No third-party dependency was added. Used `oma-backend` and `oma-dev-workflow`; Serena was unavailable, so named paths were read directly. Git reported `av.yaml: Operation not permitted`; the file was not read or bypassed.

## Validator behavior

- The current inventory includes all seven crates and enforces 80% production lines per crate. Historical inventory remains the six-crate baseline. Focused tests reject missing and below-floor crates, malformed or stale reports, changed legacy assertions, unrelated manifest edits and migration changes.
- Exact schema-3 reference adaptations preserve the prior version-2 test assertions while accommodating the observed forward migration, including the retained permit-column and foreign-key checks. A new test-only module may use its local `use super::*` parent import; other wildcard or assertion bindings remain rejected by the guard. The original `0001_phase0.sql` and `0002_managed_exec.sql` SHA-256 values still match the accepted commit: `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384` and `464a1c88270f01a56ccab6610f87eff7648e0640f276ebce53b8a97026c50814`.
- The final Traffic validator retains the fixed summary requirements and requires a successful unfiltered CLI test with hashed command output. It no longer requires the earlier 23-event format, nested native-output snapshots, a particular directory below `target`, or the harness collection to remain the latest same-source collection. The old event parser is optional. Its summary fields are not independently derived from that parser; task 9 and independent QA must reconcile actual observed results with the summary and raw logs. No future measurement was fabricated.
- With the supplied mission's absent observation policy, the target check reports not applicable and does not assert any container was inspected. Configured-policy fixtures still reject missing or stale inputs, inaccessible observations, unhealthy targets and altered logs.

## Predecessors

The task-12 report `.arc-verify/traffic/predecessor-recovery.md` matched its recorded SHA-256 `84089fa14b68fe66cecab34b582c856bb28fa7292cdd43faf7b08539b893c554`. All 26 archived files, 13 frozen inputs and eight original-location copies were verified against that report before and after transfer from the retained prior worktree. No differing destination was overwritten. Transfer log: `target/traffic/task5-attempt6/predecessor-transfer.log`, SHA-256 `3f50971c5425060c576e312cc7951cd0118d6b54a6965299976373c9296c11ae`. These ignored files need hash-verified transfer to the integrated worktree.

The separate `target/exec-fail-open/task16-attempt3/nested-fixture-dependencies.json` remains unavailable; its recorded SHA-256 is `30eaa9b742c41d7b990ea0d9ae0b6a8e5dad62ac7b1ec9b389088694169c725f`. No replacement was fabricated.

## Checks

Every command below was captured with a 7,200,000 ms per-check allowance. Each listed receipt is under `target/traffic/commands/`, and contains the paths and SHA-256 hashes of its raw stdout and stderr. The final receipts all bind the Actor-local source hash above.

| Check | Outcome | Receipt directory | Receipt SHA-256 |
| --- | --- | --- | --- |
| Focused accounting, preservation and targets | 43 passed; exit 0 | `task5-attempt6-focused-final-d7d3e349-f341-4756-92e7-7301a4ba7518` | `583a7b2f0da950c9091aba0ed56f9685ca4cc32843d7396fb14dc2231fe6d223` |
| Traffic evidence fixtures only | 8 passed, 1 live test filtered; exit 0 | `task5-attempt6-evidence-fixtures-final-05b85a95-4f49-4f05-a688-fa35e0895fc5` | `5e36a053dbc0e332d4614fc02841a251135cc25f190079aaad3457b0829651d5` |
| Full historical evidence suite | 18 passed, 4 failed; exit 1 | `task5-attempt6-historical-2ee166e5-20f0-4871-b072-89d4d10eb39e` | `08ae9b432591068f133f14146ffe997b3ce0bb686032f7c0a54bd935fa4ccfe3` |
| Existing coverage gate | failed on daemon tests; exit 1 | `task5-attempt6-coverage-efbd49fd-4034-486d-9e84-1aafc8702380` | `7d2482f0946df71313fa33d5838d7cfbb10bcea9f778b69ae37931dd66c03523` |
| Full harness without skips | failed on coverage; exit 1 | `task5-attempt6-harness-ac0f3b98-a15f-4173-95f6-1dec8ab9276e` | `78429fdb380c966f5c48bbf18ff55171017fda286d9fadf54c71fd6fbf14b91d` |
| Live coverage evidence after harness | failed latest.pass check; exit 1 | `task5-attempt6-live-coverage-66d0b328-8135-45b7-8c5e-3b1d2226ddba` | `5591c3ee24221c497abafc9015a2dc6c49faf3af04c1bd5a8b86e732b39ddd03` |

Commands, in the same order:

1. `bun test scripts/coverage/traffic-accounting.test.ts scripts/coverage/accounting.test.ts scripts/coverage/gate.test.ts scripts/coverage/preservation.test.ts scripts/harness/exec-fail-open-preservation.test.ts scripts/harness/traffic-targets.test.ts`
2. `bun test scripts/harness/traffic-evidence.test.ts --test-name-pattern fixture` (fixture-only; the final live Traffic check belongs to tasks 9 and 11)
3. `bun test scripts/harness/exec-fail-open-evidence.test.ts` (unfiltered)
4. `bun run test:coverage`
5. `./scripts/harness/validate.sh` (no skip flags)
6. `bun test scripts/coverage/evidence.test.ts`

The first focused attempt had 32 passes and four preservation failures because schema-3 expectations were absent from the old reference. Intermediate probes identified the test-only parent import and three schema-3 reference adaptations. After the narrow fixes, the complete focused run above passed. The initial failed receipt is `target/traffic/commands/task5-attempt6-accounting-94ea35ab-5bde-47f5-9632-f70ed4ba0af7/receipt.json`, SHA-256 `85b791aaa90577521e870a2db7e6a1e7282921eeac3776e09705f584c301f021`.

The full historical suite's three missing-manifest failures were `correction rotation retains the staged predecessor with a different inventory`, `correction rotation preserves the previously rotated predecessor`, and `production gates require the current combined archive and authenticated predecessors`. `sealed current evidence binds independent QA and final decision without requiring a terminal sink` failed with `EXEC_FAIL_OPEN: stale receipt inputs/git`. The retained historical stderr is `target/traffic/commands/task5-attempt6-historical-2ee166e5-20f0-4871-b072-89d4d10eb39e/stderr.log`, SHA-256 `45b5ee4b6248c5c3f00c1be9a341f604d0dd99785ef0b4bd192ca8f8437479f8`.

Both coverage collections stopped at the daemon unit tests: 45 passed and 16 failed there, with no ignored or filtered tests in that harness. The failures show schema 3 where tests expect 2, including `crates/arc-daemon/src/db_worker/tests.rs:35`, `crates/arc-daemon/src/db_worker/tests.rs:219`, `crates/arc-daemon/src/database_failure_tests.rs:351`, and `crates/arc-daemon/src/server_tests.rs:82`. These product/test updates belong to task 7; no production-line percentages were generated. The first collection's native stdout is `target/traffic/native-coverage/cf8fe862-5b2c-418c-ba72-2ea605fa306f/stdout.log`, SHA-256 `3e90456a371e063cba588798b7c3542b9f687ef5fb932e1b1798a00365bc6655`. The harness collection's native stdout is `target/traffic/native-coverage/080e8966-abd6-4090-b3cb-496857ccfdc4/stdout.log`, SHA-256 `88d1214d0bacbb2478f978838bc39b2473e2c4aea643a3884ce3bee362e2ea06`. The harness passed its first four structural checks and failed its enabled coverage check.

## Integration handoff

Task 7 must repair the daemon's schema-3 expectations and preserve their assertions. Coordinate any corresponding exact reference adaptations in the owned preservation helpers; do not suppress the guard. Recover the exact missing historical manifest from an authenticated source and refresh any required live historical receipts, then rerun the entire historical suite on integrated inputs. After product integration, rerun the focused fixtures, coverage, the full no-skip harness and live source-bound evidence. The macOS two-directory measurement, final Traffic receipts and independent `oma-qa` remain assigned to later tasks.

The saved mission has no observation policy; task 5 did not create a target-health report or claim target health. Desktop, native Windows, vendor hooks, broader detection and remaining scheduling policy remain outstanding.

Budget after this report: `00015` evidence files, `0000159692` decimal bytes. Budget capture: `target/traffic/task5-attempt6/budget.json`, SHA-256 `d1ecbf72fa1dfb6cff3e117e02e3507e4490bf002a40928e01baea4377d183b4`.
