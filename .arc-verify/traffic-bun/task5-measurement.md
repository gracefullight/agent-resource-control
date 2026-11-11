# Task 14: source-bound workspace and mixed-test refresh

Status: the task-14 captures and failure handoff are complete. The mixed-test measurement and its validator passed. The unfiltered workspace test failed and requires backend attention. Skill used: `oma-backend`. Mission verification remains incomplete.

## Attempt 2: receipt-reference repair

The task-11, task-5 attempt-2 and earlier task-5 receipt links below now point to their three distinct retained files and matching hashes. The task-5 attempt-2 copy in the task-11 directory has the same bytes as its original. On this attempt, `bun target/traffic-bun/task14/attempt2-revalidate.ts` verified those links and hashes, the failed workspace receipt and its raw outputs, and the current measurement through `validateMeasurement`. Source identity was `cac648d93c9222589275832eaa9138d8f4fbe7241842cc65d570de8bc754d3ec` before and after validation, matching the task-14 captures. No capture was rerun or relabeled.

The revalidation script is `target/traffic-bun/task14/attempt2-revalidate.ts`, SHA-256 `83025d7a29de981a06e15adbd9d5d921926d8431a0d1eb32054ae4c12830429e`; its passing output is `target/traffic-bun/task14/attempt2-revalidate.log`, SHA-256 `cd9f6771e1f77163f52bdffea2f57404e338676fd66012f1cf9cc1fbc8b130e5`. Its first run failed because the check expected the shorthand `Err(Deadline)` instead of the literal panic text in Cargo's log; `target/traffic-bun/task14/attempt2-revalidate-initial-failure.log` has SHA-256 `6e1fbded085ff9948e73118f6085086025b258d171581dcdfcf3d2060b04c71c`. The check was corrected to match the retained output and then passed. The workspace test failure itself remains unchanged and is described below.

## Source and execution workspace

The execution workspace was `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/59981caa-53a9-40cb-b3a1-3c3dfbb22b09/workspace`, at accepted HEAD `16772fe2aa350413225f3fb2bb9227d14ae5a6c4`, tree `1834e0cccb50a12fea299618f9a360f4ca10e03f`, with the task-13 transport repair integrated. The maintained `missionInputs` inventory contains 319 paths and hashes to `cac648d93c9222589275832eaa9138d8f4fbe7241842cc65d570de8bc754d3ec`. Each capture recorded that same hash before and after execution. The inventory and context are in `target/traffic-bun/task14/source-inputs.json`, SHA-256 `7f5df153940fecd9f49741efd1b8678f20b9b3d6c3508f7af7de5a115a13372b`.

Host: Darwin 27.2.0 arm64, Cargo and Rust 1.95.0, Bun 1.4.4; 8 CPUs, 16 GiB RAM, approximately 49 GiB free at capture start. The three Cargo commands ran serially with `CARGO_BUILD_JOBS=4`; each maintained capture allowed 7,200,000 ms. Serena was not exposed; known files were read directly. Git reported `av.yaml: Operation not permitted`, so this report makes no whole-worktree cleanliness claim. No GUI, browser or standalone build was used.

## Four real Cargo–Bun cases

| Arrival order | Bypass peak | Scheduled peak | Visible queue | Scheduled wait | Launches and bodies per job | Exits |
| --- | ---: | ---: | --- | ---: | --- | --- |
| Cargo first | 2 | 1 | yes | 97 ms | `[1, 1]` each | `[0, 0]` |
| Bun first | 2 | 1 | yes | 52 ms | `[1, 1]` each | `[0, 0]` |

The launch, body and exit columns apply to both modes. Each case used separate canonical directories `target/traffic-bun/commands/task14-native-e2755f61-17da-4ce2-a553-1b4d8a97d022/observations/<case>/cargo` and the sibling `bun` directory. Their absolute paths, source and executable hashes, launch and body markers, process identities, queue notices and outcomes are in the hashed observations. All outcomes were `EXITED:0`.

| Case | Cargo tool / test PID | Bun tool / test PID | Committed release row, Unix ms | Second tool kernel start, Unix µs |
| --- | --- | --- | ---: | ---: |
| Cargo first, bypass | 22673 / 22710 | 22715 / 22715 | — | — |
| Cargo first, scheduled | 22349 / 22383 | 22390 / 22390 | 1794392184802 | 1794392184842860 |
| Bun first, bypass | 22274 / 22309 | 22266 / 22266 | — | — |
| Bun first, scheduled | 22612 / 22647 | 22502 / 22502 | 1794392188148 | 1794392188181977 |

The second tool kernel start followed the end of the release millisecond by 39,860 µs and 32,977 µs, respectively. The maintained validator uses the macOS kernel process start identity and the release timestamp in the committed row. The stored timestamp is sampled before commit; it does not independently measure when the commit returned. The retained pre-exec witness also observed the committed released row. The negative fixture rejected a second kernel start that was early or ambiguous.

## Commands, outcomes and raw evidence

All artifact paths are relative to the execution workspace. Receipt hashes bind the raw stdout and stderr hashes shown below. The receipts record exact argv, actor, process, timing, host, executable, source identities and exit. `target` is ignored and is not automatically transferred to another workspace.

| Command or check | Outcome | Receipt path and SHA-256; raw stdout / stderr SHA-256 |
| --- | --- | --- |
| `CARGO_BUILD_JOBS=4 bun scripts/coverage/traffic-bun-evidence.ts capture measurement-engineer task14-workspace cargo test --workspace` | **Failed**, exit 101. Completed summaries show 411 passed, 1 failed, 0 ignored and 0 filtered across 19 suites before Cargo stopped; remaining suites were not run. | `target/traffic-bun/commands/task14-workspace-1417e880-5406-4596-b058-e264ced43508/receipt.json` `5aca89dc91bc028e3c4766b399d9429c83046fd4bc89e31f33f4e00dbb8a0dae`; stdout `ad56e233802acb13d9782789ed0b880608adb27d83d22ff0bcdc30b7978d5701`; stderr `b664b740dfbd4ebe3be41e2273d6b6df1107edc6015b51442bcb99ad3477484d`. |
| `CARGO_BUILD_JOBS=4 bun scripts/coverage/traffic-bun-evidence.ts capture measurement-engineer task14-binary cargo test -p arc-cli --tests --message-format=json` | Passed; 68 passed, 0 failed, ignored or filtered across 5 suites. | `target/traffic-bun/commands/task14-binary-fd71dd00-00ec-4f21-af9f-7a2b2ca7963d/receipt.json` `8dc88026db0d7336a0b304297d5f4afa95708a065b632f01b97cad7df06cb95b`; stdout `ad1e0a0df4623b2b5f15d06502c183263e8e07b095972efe7cbbb7ddfd7e3f4b`; stderr `59181effa8d6a5a531a90e3df2984fcf52bc04c8ad704241913767bd9bb4d1e6`. |
| `CARGO_BUILD_JOBS=4 bun scripts/coverage/traffic-bun-evidence.ts capture measurement-engineer task14-native --binary-preparation <binary receipt path> <binary receipt SHA-256> cargo test -p arc-daemon --test traffic_cli --message-format=json -- --nocapture` | Passed; 63 passed, 0 failed, ignored or filtered. | `target/traffic-bun/commands/task14-native-e2755f61-17da-4ce2-a553-1b4d8a97d022/receipt.json` `6dea42852afc5af6cd9242fcb2a11b850b55997b68d9419ac3450758105689a3`; stdout `1ad069ff637f32250543ce63b341b453e3bfd9b95aceeb3b4f97cbf5063c59b6`; stderr `d5e52e970f687cd7f540c3a559ac49753e0d2d6aa0d11ce4394e03b370d1e678`. |
| Maintained `measurement` derivation and `validateMeasurement` | Passed, recomputing all four cases and source/process bindings from the native and binary receipts. | `target/traffic-bun/commands/task14-native-e2755f61-17da-4ce2-a553-1b4d8a97d022/observations.json` `28fff71e71dab0c30368c740530df08e76aee1077d31d44cb4896f8273c7c75d`; `target/traffic-bun/task14/measurement-validation.log` `f5c74c2771ef8403da5b0f37010ce62db8144b3bc9e705dbda0086e79b01defb`; compact `case-summary.json` `3151819bc1a2526d544b4dfdd8e82464c03057c44cdd49baf27b576f54b17a80`. |
| `bun scripts/coverage/traffic-bun-evidence.ts capture measurement-engineer task14-kernel-negative bun test scripts/coverage/traffic-bun-evidence.test.ts -t 'kernel start identity rejects'` | Passed: 1 selected test, 13 filtered, 12 assertions, 0 failures. This was a targeted negative fixture, not the full evidence suite. | `target/traffic-bun/commands/task14-kernel-negative-da9e75dd-8c0a-45b9-bbfb-b15512161b4d/receipt.json` `01d7fb5535364a28d5a7a90adc9ac14631528b4e80fcb423414dcf51972edd80`; stdout `bbe6a94c29cc8d3ff5a1792a5abae1f1122aed0642111b52036ab7e0b2b2ca47`; stderr `ae45bf950b7dcbe4765867c09e3b3ee60ce36111165290536a125488e513b677`. |
| Maintained `validateReceipt`, `validateFullCargoOutput`, `parseCargoExecution` and `validateMeasurement` | Passed for the CLI preparation and native receipts, with current source identity. | `target/traffic-bun/task14/receipt-validation.log` `e8df4824519de89d05cedba42ba844b9141ebef08c34c58890c7f00180b783c6`. |
| Staged receipt `validateMeasurement` and `evidenceBudget` | Passed against current source; 19 evidence files and 360,909 bytes at that check. | `target/traffic-bun/task14/final-validation.log` `f99ad0f719cd0d63404cdbfc7eb782d5b7af001c278c4ac5b8a257134b447981`. |

The maintained `measurement` command was invoked with the exact native and binary receipt paths and SHA-256 values in this table. It produced `.arc-verify/traffic-bun/mac-acceptance.json` with SHA-256 `1bf95078039c7cab81c0bbb778f595701109844f03c4eb048c48628d42e97778`. The `orders.cargo_first` and `orders.bun_first` schema is unchanged. The source-bound `arcctl` SHA-256 is `5ecc4787ae0e274289fba23ce61e078bb79c7413b641609b69fa8607a54093ea`, and `arc-daemon` is `73334f7973adc3e91f8ff2fbe37ee38fc279989fc6becdb8f460fe7639a69399`.

## Exact capture and derivation commands

The native capture reused the exact CLI preparation receipt and hash shown here:

```sh
CARGO_BUILD_JOBS=4 bun scripts/coverage/traffic-bun-evidence.ts capture measurement-engineer task14-workspace cargo test --workspace
CARGO_BUILD_JOBS=4 bun scripts/coverage/traffic-bun-evidence.ts capture measurement-engineer task14-binary cargo test -p arc-cli --tests --message-format=json
CARGO_BUILD_JOBS=4 bun scripts/coverage/traffic-bun-evidence.ts capture measurement-engineer task14-native --binary-preparation target/traffic-bun/commands/task14-binary-fd71dd00-00ec-4f21-af9f-7a2b2ca7963d/receipt.json 8dc88026db0d7336a0b304297d5f4afa95708a065b632f01b97cad7df06cb95b cargo test -p arc-daemon --test traffic_cli --message-format=json -- --nocapture
bun scripts/coverage/traffic-bun-evidence.ts measurement target/traffic-bun/commands/task14-native-e2755f61-17da-4ce2-a553-1b4d8a97d022/receipt.json 6dea42852afc5af6cd9242fcb2a11b850b55997b68d9419ac3450758105689a3 target/traffic-bun/commands/task14-binary-fd71dd00-00ec-4f21-af9f-7a2b2ca7963d/receipt.json 8dc88026db0d7336a0b304297d5f4afa95708a065b632f01b97cad7df06cb95b > target/traffic-bun/task14/mac-acceptance.json
bun scripts/coverage/traffic-bun-evidence.ts capture measurement-engineer task14-kernel-negative bun test scripts/coverage/traffic-bun-evidence.test.ts -t 'kernel start identity rejects'
```

## Workspace failure and handoff

`db_worker::tests::execution_deadline_interrupts_sql_before_response_deadline` failed in the `worker_runtime` suite. The retained stdout records an `Err(Deadline)` unwrapped by the shared recovery helper at `crates/arc-daemon/src/db_worker/tests.rs:134`. The log does not establish why the deadline expired. The stderr also recorded `fixture cleanup observation failed: permission denied during observation` while the separate Traffic CLI suite itself reported 63 passing tests. No product code was changed and the failing command was not rerun by this task. The backend owner should investigate the failure; the final-gate owner needs a passing current-source workspace receipt after any repair.

- [x] Captured an unfiltered workspace attempt and reported its actual failure.
- [x] Recomputed and validated the four real mixed-tool cases, including kernel-start ordering, queue visibility, single launches and body executions, and successful exits.
- [x] Preserved task-11's prior receipt at `target/traffic-bun/task14/historical-task11-mac-acceptance.json`, SHA-256 `23f029bbeeb37dc7495dafa26dfd6a8b53873a646c49799d99373caedadd8e6c`; its report remains below as historical evidence.
- [ ] Independent review of the replacement measurement, a passing unfiltered workspace test, coverage, harness, final evidence and QA rebinding remain with their assigned owners.

The above raw artifacts exist in this execution workspace. No transfer or installation in another workspace is claimed. Task 8 must verify every referenced file and hash in its own workspace or recollect there; any relevant source change requires new source-bound receipts and independent review of the replacement measurement. The old gate and QA artifacts are not rebound by this task.

---

# Historical task 11: refreshed workspace and Cargo–Bun measurement

Status: the workspace test, source-bound CLI preparation and native Traffic CLI capture passed; the refreshed measurement validator passed. Skill used: `oma-backend`. This is a measurement handoff, not final mission verification.

## Source and workspace

The assigned workspace was `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/59981caa-53a9-40cb-b3a1-3c3dfbb22b09/workspace`, at accepted HEAD `16772fe2aa350413225f3fb2bb9227d14ae5a6c4` and tree `1834e0cccb50a12fea299618f9a360f4ca10e03f`. Its working-tree overlay includes task 10's test repair. `missionInputs` recorded 319 source paths, modes and hashes as `f7f8da187251f11c9a06169fd3f0743a40a5892b603202bd6c80b283683d0724`. Each capture receipt records that exact source hash both before and after its command. The source inventory is `target/traffic-bun/task11/source-inputs.json`, SHA-256 `9fb6b6ee7b7494d3e5c051e1b6d321d88ea4637506025b9af09d0bae57716571`.

The host was Darwin 27.2.0 arm64, Cargo/Rust 1.95.0 and Bun 1.4.4. The host reported 8 CPUs, 16 GiB RAM and approximately 54 GiB free; Cargo used four jobs. Each maintained capture allowed 7,200,000 ms. Git status reported `av.yaml: Operation not permitted`; Serena was not exposed, so known paths were read directly. `ps` was also denied with `operation not permitted`. An exploratory read of `scripts/coverage/traffic-capture.ts` failed because that file is absent; the existing `traffic-bun-evidence.ts` provided the maintained capture instead. No GUI, browser or standalone build was used.

## Four real test-process cases

| Arrival order | Bypass peak | Scheduled peak | Queue visible | Queue wait | Invocations | Exit codes |
| --- | ---: | ---: | --- | ---: | --- | --- |
| Cargo first | 2 | 1 | yes | 97 ms | `[1, 1]` | `[0, 0]` |
| Bun first | 2 | 1 | yes | 47 ms | `[1, 1]` | `[0, 0]` |

The invocation and exit columns apply to both cases in each row. Each real test had one launch marker and one body marker and both recorded `EXITED:0`. The Cargo and Bun jobs used separate canonical working directories under `target/traffic-bun/commands/task11-native-8561f5f6-7950-4273-8c76-a0649ef05702/observations/<case>/<tool>`.

| Case | Cargo tool / test PID | Bun tool / test PID | Committed release row Unix ms | Second tool kernel start Unix µs |
| --- | --- | --- | ---: | ---: |
| Cargo first, bypass | 58198 / 58234 | 58250 / 58250 | — | — |
| Cargo first, scheduled | 57861 / 57895 | 57903 / 57903 | 1794389594696 | 1794389594749044 |
| Bun first, bypass | 57801 / 57835 | 57797 / 57797 | — | — |
| Bun first, scheduled | 58118 / 58156 | 58109 / 58109 | 1794389598266 | 1794389598301283 |

The second tool's kernel start was 52,044 µs and 34,283 µs after the end of the respective release millisecond. The comparison uses macOS kernel process start identity and the release timestamp stored in the committed row, as the operator specified. The timestamp is sampled before the transaction commits; it is not an independent measurement of the instant commit returned. A targeted negative fixture rejected early or ambiguous kernel starts.

## Checks and raw evidence

All paths are relative to the assigned workspace. The maintained command receipts record argv, actor, host, timing, executable identity, source hash before and after, exit, and hashes of stdout and stderr. Raw data is under ignored `target` and does not transfer automatically to another workspace.

| Command or check | Outcome | Receipt or artifact and SHA-256 |
| --- | --- | --- |
| `CARGO_BUILD_JOBS=4 bun scripts/coverage/traffic-bun-evidence.ts capture measurement-engineer task11-workspace cargo test --workspace` | Exit 0; 569 passed across 37 suites, zero failed, ignored or filtered. | `target/traffic-bun/commands/task11-workspace-05effae0-89fa-42bf-8f2c-33729d685062/receipt.json` `044ba04c107b8203327a87f063c3c3536569ec9b133947890a7545fe6afd486c`; stdout `8de636c17c7741fc8d8815477ef024d102bedf10cc2fd65e8f4f9b43137a47ef`, stderr `a6d24704021afcb32f25ebbff8688188525111d349d2146bcff1cfb17e4d3c72`. |
| `CARGO_BUILD_JOBS=4 bun scripts/coverage/traffic-bun-evidence.ts capture measurement-engineer task11-binary cargo test -p arc-cli --tests --message-format=json` | Exit 0; 67 passed, zero failed, ignored or filtered. | `target/traffic-bun/commands/task11-binary-297d637b-0db9-4482-82ef-7a3b75291399/receipt.json` `556b48a096fc5fe1d3c7a828d0612107c05c7ecaa38d94aba6646b9bf2b3062e`; stdout `d30389eb5f9e22ef422a7687a1e65415347d743540dbe9ca5c09f455eb1463b2`, stderr `fee0e799c1c34cf20eb11d0a8b23d08cbf5b508d595393b3dda38afd5c9fbf0e`. |
| `CARGO_BUILD_JOBS=4 bun scripts/coverage/traffic-bun-evidence.ts capture measurement-engineer task11-native --binary-preparation <binary receipt> <binary SHA-256> cargo test -p arc-daemon --test traffic_cli --message-format=json -- --nocapture` | Exit 0; 63 passed, zero failed, ignored or filtered. | `target/traffic-bun/commands/task11-native-8561f5f6-7950-4273-8c76-a0649ef05702/receipt.json` `6d51974a5d5038474cfc8ef31685d9c77aa1d8bd8def4eea8603fb033ffb97a0`; stdout `458c71b0145b2137c7e501929714d99af0dd742d9367d1a5b4ebc5de55f6262b`, stderr `f2f3c258990ae085ee70b3a7855d99901cbb57c76657a535874245cdad0718b4`. |
| Maintained `measurement` derivation and `validateMeasurement` | Passed using the two exact preparation and native receipts above. | Observations `target/traffic-bun/commands/task11-native-8561f5f6-7950-4273-8c76-a0649ef05702/observations.json` `2bfb97fcb2ba993e1d7738a1323044c069f6eda04d6fe9c76e41cb95ddbf3c27`; validator output `target/traffic-bun/task11/measurement-validation.log` `01612e29e577d685b94d719d38e3d75db20811b3acb171aec9eae5ba42c27744`; [retained task-11 receipt](../../target/traffic-bun/task14/historical-task11-mac-acceptance.json) `23f029bbeeb37dc7495dafa26dfd6a8b53873a646c49799d99373caedadd8e6c`. |
| `bun test scripts/coverage/traffic-bun-evidence.test.ts -t 'kernel start identity rejects'` via maintained capture | Exit 0; one selected test passed, 13 filtered, 12 assertions. | `target/traffic-bun/commands/task11-kernel-negative-ce590851-f36f-4e3e-87df-22aaa01f3bb9/receipt.json` `779303c6b35eae6193bd15c84b533f3be9275929e301b0b80e8d4f5c8d1401d5`; stderr `042da7c591af7bc4c6f9bc9db72a2209794c78daf93bac1bc2adcf632e7b81b8`. |

The observation file binds individual marker files, test sources, logs, working directories and process identities by SHA-256. A compact extraction is `target/traffic-bun/task11/case-summary.json`, SHA-256 `c5077319da1b149c31954cdcc476cc3dbd826b6aadf5fdfbea6de04a201e09f2`. The source-bound `arcctl` SHA-256 is `23010b107c2b6e8d19eeea0a2f07813912bc833ab284425b517646794e589d96` and daemon SHA-256 is `b8d4665e2fdcb41dcc6d8051ed3ae883e4f8c39edab3cf7bcb34c99f9e0cdd7c`. Actual Cargo and Bun tool SHA-256 values are `c512bff73c86143b557463f021d0c3d5b0490d97d65040ba59ea2b3427784758` and `0a2237cdcfa0d16ef501d9acca9f0026309ef348bea7393a64e8d1e3609565cc`, respectively. Each Cargo fixture test executable and its hash is recorded in the observations.

Receipt validations for the workspace, preparation and native captures are retained at `target/traffic-bun/task11/workspace-validation.log` (`b07e863d104f6bc18c393466b588b775fafa6c3e417fcae540f3f4a2644a4063`), `binary-validation.log` (`9bdbb7cd8ff508c1b4ab9c95a59bd1943d878bbbbfc2cc250f9f9f4e56ce0ba0`) and `native-validation.log` (`3cf7b1cf8fd590ccfcc713418fd817643c5a7b3b2e181ea7e000f371eb21f1a4`) in the same directory.

## Exact capture and derivation commands

The three captures ran serially, with the workspace command first. The preparation receipt and its hash were passed unchanged into the native capture and measurement derivation:

```sh
CARGO_BUILD_JOBS=4 bun scripts/coverage/traffic-bun-evidence.ts capture measurement-engineer task11-workspace cargo test --workspace
CARGO_BUILD_JOBS=4 bun scripts/coverage/traffic-bun-evidence.ts capture measurement-engineer task11-binary cargo test -p arc-cli --tests --message-format=json
CARGO_BUILD_JOBS=4 bun scripts/coverage/traffic-bun-evidence.ts capture measurement-engineer task11-native --binary-preparation target/traffic-bun/commands/task11-binary-297d637b-0db9-4482-82ef-7a3b75291399/receipt.json 556b48a096fc5fe1d3c7a828d0612107c05c7ecaa38d94aba6646b9bf2b3062e cargo test -p arc-daemon --test traffic_cli --message-format=json -- --nocapture
bun scripts/coverage/traffic-bun-evidence.ts measurement target/traffic-bun/commands/task11-native-8561f5f6-7950-4273-8c76-a0649ef05702/receipt.json 6d51974a5d5038474cfc8ef31685d9c77aa1d8bd8def4eea8603fb033ffb97a0 target/traffic-bun/commands/task11-binary-297d637b-0db9-4482-82ef-7a3b75291399/receipt.json 556b48a096fc5fe1d3c7a828d0612107c05c7ecaa38d94aba6646b9bf2b3062e > target/traffic-bun/task11/mac-acceptance.json
bun scripts/coverage/traffic-bun-evidence.ts capture measurement-engineer task11-kernel-negative bun test scripts/coverage/traffic-bun-evidence.test.ts -t 'kernel start identity rejects'
```

## Final-gate handoff

- [x] The unfiltered workspace run passed with the actual `measurement-engineer` actor identity.
- [x] All four cases, the kernel-start ordering and marker counts were rederived and validated. The existing `orders.cargo_first` and `orders.bun_first` receipt interface is unchanged.
- [x] Previous task 5 results remain below as historical observations. The task-5 attempt-2 receipt was preserved as `target/traffic-bun/task11/historical-mac-acceptance.json`, SHA-256 `f4a8c1c4bbd46c147b1d7f587f75a55b6b0ec44ace6b9b092d7cc31566c28d5a`.
- [ ] The final-gate owner must use the exact workspace, CLI preparation and native receipt references above in the final gate summary when the recorded source and all raw-file hashes still match. The old `gate-summary.json`, `gates.json` and `qa.json` refer to source `b80c9452c4ab6011fc6dc66af609d822b31aadf2875bb08784a22e78593ba590` and require the responsible owner's current-source verification and rebinding. Coverage, harness and final evidence checks were not run by this task.

These raw files exist in the assigned workspace named above. No installation or transfer to a different workspace was performed. Before reuse, the final-gate owner must confirm the files and hashes in its execution workspace and re-collect locally if they are missing or the source changes. The three historical measurements have distinct retained receipts: task 11 at `target/traffic-bun/task14/historical-task11-mac-acceptance.json` (`23f029bbeeb37dc7495dafa26dfd6a8b53873a646c49799d99373caedadd8e6c`); task-5 attempt 2 at `target/traffic-bun/task5-attempt2/measurement.json` (`f4a8c1c4bbd46c147b1d7f587f75a55b6b0ec44ace6b9b092d7cc31566c28d5a`), also copied to the task-11 path above; and the earlier task-5 run at `target/traffic-bun/task5/measurement.json` (`83dd8990cb4195f2bddfb1734220d93945057e9721f93b2d1ea99ff0577a6df2`). “Current” in the historical text means current at that earlier capture.

---

# Historical task 5: integrated Cargo–Bun measurement

Status: the assigned fixture repair, measurement and source-bound measurement validator passed. Skill used: `oma-backend`. This result is for the current working-tree source, not a claim that the mission-wide gates or current-source independent QA passed.

## Source and fixture

The real workspace was `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/59981caa-53a9-40cb-b3a1-3c3dfbb22b09/workspace`. HEAD was the accepted `16772fe2aa350413225f3fb2bb9227d14ae5a6c4`, tree `1834e0cccb50a12fea299618f9a360f4ca10e03f`. The measured source identity was `b80c9452c4ab6011fc6dc66af609d822b31aadf2875bb08784a22e78593ba590`; the 319 source input hashes and modes are in `target/traffic-bun/task5-attempt2/source-inputs.json`, SHA-256 `7c646a6dda9c3c31c81b3b8f77d8efd79577e93c4243aff824228992e7149879`.

`historical_integrity.rs` referenced a removed `src/exec/transport/tests.rs`. The original bytes were restored to `crates/arc-cli/tests/fixtures/historical_transport_tests.rs` from Git blob `41902733a5c1b741ac2097dcccb1ad7fd439e9ca`, and both includes were redirected there. `git hash-object` returned that exact pinned blob. The original-byte and bounded-test-body assertions remain in place, and the complete CLI preparation passed. Both the fixture and revised source were staged. No migration changed relative to the accepted baseline.

The host was Darwin 27.2.0 arm64, Cargo 1.95.0 and Bun 1.4.4. The source-bound commands used four Cargo jobs on the host reporting 8 CPUs, 16 GiB memory and about 56 GiB free disk. Each capture allowed 7,200,000 ms. Full Git status reported `av.yaml: Operation not permitted`; scoped status worked. Serena tools were not exposed, so identified paths were read directly. No GUI or browser was used.

## Results

| Arrival order | Bypass peak | Scheduled peak | Visible queue | Queue wait | Invocations | Exit codes |
| --- | ---: | ---: | --- | ---: | --- | --- |
| Cargo first | 2 | 1 | yes | 95 ms | `[1, 1]` | `[0, 0]` |
| Bun first | 2 | 1 | yes | 55 ms | `[1, 1]` | `[0, 0]` |

Invocation and exit columns apply to both cases in each row. Each actual test had one launch and one body marker; both completion outcomes were `EXITED:0` in all four cases. Each case used separate canonical Cargo and Bun working directories under `target/traffic-bun/commands/task5-attempt2-native-0499c80c-ea28-40bb-b526-b22e720d1a37/observations/<case>/<tool>` in the workspace above. The hashed observations bind the directories, tool and test executable hashes, process identities, launch and body markers, queue notice, and completion records.

| Case | Cargo tool PID / test PID | Bun tool and test PID | Committed release timestamp (Unix ms) | Second tool kernel start (Unix µs) |
| --- | --- | ---: | ---: | ---: |
| Cargo first, bypass | 91541 / 91575 | 91579 | — | — |
| Cargo first, scheduled | 91345 / 91379 | 91386 | 1794385383763 | 1794385383799879 |
| Bun first, bypass | 91416 / 91450 | 91412 | — | — |
| Bun first, scheduled | 91480 / 91515 | 91474 | 1794385388743 | 1794385388776257 |

The second kernel start followed the end of the release millisecond by 35,879 µs and 32,257 µs, respectively. The stored release timestamp is sampled before the transaction commits; the validator compares the kernel identity with the timestamp in the committed row, as specified by the operator. The kernel-order negative fixture also rejects an early or precision-ambiguous start.

## Checks and evidence

All paths are relative to the real workspace above and SHA-256 hashes refer to actual local bytes. `target` is ignored and is not automatically transferred to another workspace.

| Command or check | Result | Evidence |
| --- | --- | --- |
| `CARGO_BUILD_JOBS=4 bun scripts/coverage/traffic-bun-evidence.ts capture measurement-engineer task5-attempt2-binary cargo test -p arc-cli --tests --message-format=json` | Passed: 67 tests, zero failed or ignored. | Receipt `target/traffic-bun/commands/task5-attempt2-binary-fb0724ba-8c96-4454-b1b7-dc22148c97d6/receipt.json` `d4d2f290c952434012375267bca1f4c2c7a96d4abf49dd6515ab486bd2a7c7e9`; stdout `cae570416c305144fec4fb9234517613b20bc4fb089d645f0650ee7a80eb92bd`; stderr `31dac23ea56d17b3fc333d8c7c2716c504f4560cfa70da4949d8db6933870147`. |
| `CARGO_BUILD_JOBS=4 bun scripts/coverage/traffic-bun-evidence.ts capture measurement-engineer task5-attempt2-native --binary-preparation <binary receipt> <binary SHA-256> cargo test -p arc-daemon --test traffic_cli --message-format=json -- --nocapture` | Passed: 63 tests, zero failed or ignored, with no test filter. | Receipt `target/traffic-bun/commands/task5-attempt2-native-0499c80c-ea28-40bb-b526-b22e720d1a37/receipt.json` `04d3f0457b1cb579412ff0cc04361ceed0153ec6ba1b093094d066fae006007b`; stdout `9273b91ceff9e4e8a5090c19c329f1a46ffb7f79529cf39a1829c51bea96fdd1`; stderr `940e417326987e24e7e792d989085e33dfb1e1be14e90ef4bcb3c980d8167da3`. |
| `bun scripts/coverage/traffic-bun-evidence.ts measurement <native receipt> <native SHA-256> <binary receipt> <binary SHA-256>` | Passed and derived the maintained `orders` contract from the native output. | `target/traffic-bun/commands/task5-attempt2-native-0499c80c-ea28-40bb-b526-b22e720d1a37/observations.json` `f97f6fd5f2e21669e6336f85bacf7dad80492a777804b0a774c5bf614c877e88`; [retained task-5 attempt-2 receipt](../../target/traffic-bun/task5-attempt2/measurement.json) `f4a8c1c4bbd46c147b1d7f587f75a55b6b0ec44ace6b9b092d7cc31566c28d5a`. |
| Exported `validateMeasurement` against the staged receipt and current `sourceContext` | Passed; it rechecked source, executable, native-output, raw-file and derived-order bindings. | `target/traffic-bun/task5-attempt2/measurement-validation.log` `f349c476287eb4afa3e8dd18f5996dd8195cae25a852cd2c1d58a25f8b6e1fb9`; compact process summary `target/traffic-bun/task5-attempt2/process-summary.json` `ed812367e6a1b92062539bf2281e316a19d1cc58cd4d27966f8c00f205a00628`. |
| `bun test scripts/coverage/traffic-bun-evidence.test.ts -t 'kernel start identity rejects'` | Passed: one selected test, 13 filtered, 12 assertions. | `target/traffic-bun/task5-attempt2/kernel-test.log` `5e3efb8662e7f10a9cc08e6638479f23cee2bcfdda0b9a93b12a2a34273df29f`. |
| `cargo fmt --all -- --check` | Passed. | `target/traffic-bun/task5-attempt2/fmt.log` `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`. |
| `git diff --exit-code 16772fe2aa350413225f3fb2bb9227d14ae5a6c4 -- crates/arc-storage/migrations`; scoped cached diff checks | Passed. | Command exit 0. |

The source-bound `arcctl` SHA-256 was `cde951f5eb01a24a33b154fa8c11820e622f4599dd0d70de6ddad4a328cf08ed` and `arc-daemon` SHA-256 was `155ba888e576c5660ec18b63dba6f09f23de6fb544d9fd7fbe1477a958397e33`. Cargo's executable SHA-256 was `c512bff73c86143b557463f021d0c3d5b0490d97d65040ba59ea2b3427784758`; Bun's was `0a2237cdcfa0d16ef501d9acca9f0026309ef348bea7393a64e8d1e3609565cc`. Individual test binaries and all process start tokens are in the hashed observations.

## Exact retained capture commands

The following commands use the retained preparation and native receipts from this run. A fresh capture creates a new run directory and hash; use the newly printed reference for subsequent commands.

```sh
CARGO_BUILD_JOBS=4 bun scripts/coverage/traffic-bun-evidence.ts capture measurement-engineer task5-attempt2-binary cargo test -p arc-cli --tests --message-format=json
CARGO_BUILD_JOBS=4 bun scripts/coverage/traffic-bun-evidence.ts capture measurement-engineer task5-attempt2-native --binary-preparation target/traffic-bun/commands/task5-attempt2-binary-fb0724ba-8c96-4454-b1b7-dc22148c97d6/receipt.json d4d2f290c952434012375267bca1f4c2c7a96d4abf49dd6515ab486bd2a7c7e9 cargo test -p arc-daemon --test traffic_cli --message-format=json -- --nocapture
bun scripts/coverage/traffic-bun-evidence.ts measurement target/traffic-bun/commands/task5-attempt2-native-0499c80c-ea28-40bb-b526-b22e720d1a37/receipt.json 04d3f0457b1cb579412ff0cc04361ceed0153ec6ba1b093094d066fae006007b target/traffic-bun/commands/task5-attempt2-binary-fb0724ba-8c96-4454-b1b7-dc22148c97d6/receipt.json d4d2f290c952434012375267bca1f4c2c7a96d4abf49dd6515ab486bd2a7c7e9
```

The measurement command's first run created the observation artifact in that native run directory. Its validation used the task-5 attempt-2 receipt and source context at that capture; the current source context has since changed.

## Handoff to tasks 7 and 8

- [x] All four cases satisfy their peak requirements and both scheduled cases show queuing and kernel-start ordering after release.
- [x] Both tests launch once, execute their bodies once and exit successfully in every case; waits and observations are retained by hash.
- [x] The required `orders.cargo_first` and `orders.bun_first` interface is unchanged and validated against the current source.
- [ ] Independent current-source QA, workspace-wide gates, coverage, full harness and final Bun evidence check remain with their assigned owners. Existing `qa.json` has source hash `b37a639c46d64dfc71068635603699180ba5dcdd58d38107ff0e2aa1f47e516f`; `gate-summary.json` has `02506ddc0356dbddfe90de84a76e2df2fffd922fc5c0ef4f9e56462f69b433fe` and records coverage and harness as failed. They are stale for this source and must not be represented as current-source passes.

Task 8 may use the two receipt references above for its gate summary only while current `missionInputs` remains `b80c9452c4ab6011fc6dc66af609d822b31aadf2875bb08784a22e78593ba590` and all raw artifacts remain present and hash-valid. The final validator requires the gate-summary command references to equal these measurement receipts. Recollect after any relevant source change. If handoff occurs into another workspace, explicitly preserve and verify the entire referenced raw artifact tree or collect fresh local receipts; ignored `target` files do not transfer automatically. The fixture repair changed the measured source, so prior source-bound gate and QA receipts cannot be relabeled to the new hash.

The assigned change staged only `crates/arc-cli/src/exec/transport/historical_integrity.rs`, `crates/arc-cli/tests/fixtures/historical_transport_tests.rs`, `.arc-verify/traffic-bun/mac-acceptance.json` and this report. Other Actor work was preserved.

---

# Historical earlier task 5 run

The following section records an earlier source identity and is not the current acceptance result.

# Task 5: integrated Cargo–Bun measurement

Status: the assigned measurement and measurement-specific validation passed. The mission-wide evidence suite still needs the final gate and independent QA artifacts. Skill used: `oma-backend`.

## Source and host

The real workspace was `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/59981caa-53a9-40cb-b3a1-3c3dfbb22b09/workspace`. HEAD was the accepted `16772fe2aa350413225f3fb2bb9227d14ae5a6c4`, tree `1834e0cccb50a12fea299618f9a360f4ca10e03f`, with the delivered task 2–4 working-tree changes integrated. The measurement source identity, including the guide and all 314 source inputs, was `20649c7739bb73be67c3855fbe0265c3a5cd2674fcdd84c333dcf8f2a9e806b8`; all of those paths were in `git ls-files` before capture. Task-2 repair and task-3 validator and measurement files matched their handoff hashes. `target/traffic-bun/task5/source-inputs.json` records each input's hash and mode; SHA-256 `7617ab1246dce3544aecc690a69151779c6dba3dc4d6a0a57fe565b292b41ffa`.

The host was Darwin 27.2.0 arm64, Bun 1.4.4 and Cargo 1.95.0. The capture used four Cargo build jobs, 16 GiB reported memory, 8 CPUs and about 75 GiB available disk. Each maintained capture command allowed 7,200,000 ms. Full Git status reported `av.yaml: Operation not permitted`; scoped status worked. Serena tools were not exposed, so known paths were read directly. No GUI or browser was used.

## Measured results

| Arrival order | Bypass peak | Scheduled peak | Visible queue | Queue wait | Invocations | Exit codes |
| --- | ---: | ---: | --- | ---: | --- | --- |
| Cargo first | 2 | 1 | yes | 93 ms | `[1, 1]` | `[0, 0]` |
| Bun first | 2 | 1 | yes | 49 ms | `[1, 1]` | `[0, 0]` |

The invocation and exit columns apply to every corresponding bypass and scheduled case. Both completion outcomes were `EXITED:0` in every case. Each case used separate canonical Cargo and Bun directories under `target/traffic-bun/commands/task5-native-5a28f9f7-8776-49a6-9426-c27b2a4ad7ed/observations/<case>/<tool>` in the workspace above. The raw observations bind each directory, actual executable and test-process identity, launch and body marker, notice, completion, and source file by path and hash.

| Case | Cargo tool PID / test PID | Bun tool and test PID | Committed release timestamp (Unix ms) | Second tool kernel start (Unix µs) |
| --- | --- | ---: | ---: | ---: |
| Cargo first, bypass | 54606 / 54640 | 54644 | — | — |
| Cargo first, scheduled | 54539 / 54573 | 54580 | 1794370160539 | 1794370160575833 |
| Bun first, bypass | 54821 / 54855 | 54817 | — | — |
| Bun first, scheduled | 54677 / 54712 | 54671 | 1794370165292 | 1794370165327081 |

The second kernel start followed the end of the release millisecond by 35,833 µs and 34,081 µs, respectively. The operator accepted the stored timestamp in the committed release row as the ordering proof. That timestamp is sampled before the transaction commits; it is not an independent measurement of the instant the commit returned. The retained validator's early-start and ambiguous-time negative fixture passed.

## Checks and raw evidence

All paths below are relative to the actual workspace above. The ignored `target` files are local raw evidence; their presence is not implied in another checkout.

| Command or check | Outcome | Evidence and SHA-256 |
| --- | --- | --- |
| `CARGO_BUILD_JOBS=4 bun scripts/coverage/traffic-bun-evidence.ts capture measurement-engineer task5-binary cargo test -p arc-cli --tests --message-format=json` | Passed; 66 tests, 0 failed, 0 ignored. | `target/traffic-bun/commands/task5-binary-617f5901-a2e3-4b2d-bedd-e99fc243413a/receipt.json` `3dfa6d1683429622fd4d3049aee9cc0de6a821854e593b836dda6fb595be5e5a`; stdout `5570e675fff01c8a9c5ef3e35187eae89e7bb30edb78a326dbc503752e12e7a8`; stderr `bcba6cb1116f28e04df7c9c77318e406082b36283df650ad70664a18d4cc99fe` |
| `CARGO_BUILD_JOBS=4 bun scripts/coverage/traffic-bun-evidence.ts capture measurement-engineer task5-native --binary-preparation <binary receipt path> <binary receipt SHA-256> cargo test -p arc-daemon --test traffic_cli --message-format=json -- --nocapture` | Passed; 63 tests, 0 failed, 0 ignored, no test filter. | `target/traffic-bun/commands/task5-native-5a28f9f7-8776-49a6-9426-c27b2a4ad7ed/receipt.json` `31a9e252fae1bb056cbc9c71b0614512218454063511f170eef1d6a5bc7ad514`; stdout `ee4bf39ace69c7244e79a1574f8dc74138b092a73aacdb7cf21b2b2fa6fb4f6e`; stderr `5545b60897b1f689b7a7e31ba5a4789fbf0190bddc91d1084a690041b0d44b5a` |
| `bun scripts/coverage/traffic-bun-evidence.ts measurement <native receipt path> <native receipt SHA-256> <binary receipt path> <binary receipt SHA-256>` | Passed; derived the four cases from the native output and validated its source and process bindings. | `target/traffic-bun/commands/task5-native-5a28f9f7-8776-49a6-9426-c27b2a4ad7ed/observations.json` `bba208955352f2b5c1ae5681538738955d5cef187c5ecc17809fbb5d2d303edf`; [retained earlier task-5 receipt](../../target/traffic-bun/task5/measurement.json) `83dd8990cb4195f2bddfb1734220d93945057e9721f93b2d1ea99ff0577a6df2` |
| `validateMeasurement` on the staged `mac-acceptance.json` using `scripts/coverage/traffic-bun-evidence.ts` | Passed and recomputed the result from the raw receipts and observations against current source. | `target/traffic-bun/task5/final-measurement-validation.log` `081ab2efa6f90c9c9a17f5f197ff670a04ec10524d8afdc9ff6b43edaad6e478`; process summary `target/traffic-bun/task5/process-summary.jsonl` `afa859ffdc2d66caa24b513af2a9e140172a618bbbb5626c05ba45a7ecbedfe0` |
| `bun test scripts/coverage/traffic-bun-evidence.test.ts` | 13 passed, 1 failed. The live final-evidence test reported `mandatory final evidence is not indexed`; the gate summary and QA files remain downstream. This is not a full evidence-suite pass. | `target/traffic-bun/task5/evidence-test.log` `1dd78c6ad5e8f3641b857c3010ff1039eaab343f0c72066b54f91a07a21208cb` |
| `git diff --exit-code 16772fe2aa350413225f3fb2bb9227d14ae5a6c4 -- crates/arc-storage/migrations`; scoped cached and working `git diff --check` | Passed. | Command exit 0 |

The binary receipt identifies `arcctl` SHA-256 `e3dcd697fc4341965241a4ed58a57a18ce51d98f13c5e098a6398c41085294f6`; the native receipt identifies `arc-daemon` SHA-256 `f02c10d46550e5a0a481257b8882a0824f91715ed8598dc6d3105fbb0327faee`. Cargo's executable SHA-256 was `c512bff73c86143b557463f021d0c3d5b0490d97d65040ba59ea2b3427784758`, and Bun's was `0a2237cdcfa0d16ef501d9acca9f0026309ef348bea7393a64e8d1e3609565cc`. The individual Cargo test binaries and all process start tokens are in the hashed observations.

## Handoff

- [x] Four cases satisfy the requested peaks, visible scheduled queue, release/start ordering, exactly one launch and body execution per job, and successful exits.
- [x] `mac-acceptance.json` uses the required `orders.cargo_first` and `orders.bun_first` interface and passes the measurement validator against the integrated source.
- [x] Delivered new source and validator paths, including the restoration script, were staged so the later harness inventory can see them. Existing Actor changes and already staged documentation were preserved.
- [ ] Independent `oma-qa`, gate summary, workspace gates, coverage and full harness remain with tasks 7 and 8. Their final evidence must be indexed before rerunning the complete Bun evidence suite.

Task 5 wrote and staged only `mac-acceptance.json` and this report as tracked evidence. It also staged the following previously untracked dependency deliverables without editing their bytes:

```text
.arc-verify/traffic-bun/restore-predecessor.py
crates/arc-cli/src/exec/lineage.rs
crates/arc-cli/src/exec/transport/continuity.rs
crates/arc-cli/src/exec/transport/endpoint.rs
crates/arc-cli/tests/fixtures/frozen_traffic_v1.rs
crates/arc-cli/tests/fixtures/hold_unattached.rs
crates/arc-daemon/tests/support/bun_fault_lifecycle.rs
crates/arc-daemon/tests/support/bun_measurement.rs
crates/arc-daemon/tests/traffic_cli/bun_measurement.rs
crates/arc-daemon/tests/traffic_cli/diagnostics/frozen.rs
crates/arc-daemon/tests/traffic_cli/diagnostics/legacy.rs
crates/arc-daemon/tests/traffic_cli/diagnostics/legacy_recovery.rs
crates/arc-daemon/tests/traffic_cli/diagnostics/two_store.rs
crates/arc-daemon/tests/traffic_cli/semantics/environment.rs
crates/arc-daemon/tests/traffic_cli/semantics/lineage_recovery.rs
scripts/coverage/traffic-bun-evidence.test.ts
scripts/coverage/traffic-bun-evidence.ts
scripts/coverage/traffic-bun-measurement.ts
```

Task 8 can reuse the two command receipts in its gate summary only if the current measured source identity remains `20649c7739bb73be67c3855fbe0265c3a5cd2674fcdd84c333dcf8f2a9e806b8` and the actual referenced raw files remain available and hash-valid. A relevant source change requires fresh CLI preparation, native capture and measurement. If the final owner runs in another workspace, it must collect fresh local receipts or explicitly preserve and verify all referenced raw artifacts; ignored `target` files do not transfer automatically. The task-9 restoration script and full harness remain that owner's responsibility.
