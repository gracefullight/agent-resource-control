# Supported product claims: Cargo and Bun tests

## Developer benefit and conditions

A developer can run `arcctl exec -- cargo test` and `arcctl exec -- bun test` so that eligible local, one-shot test jobs use the same Traffic TEST window and global admission budget. With the current default one-slot TEST window, one eligible test can wait while the other holds the slot. A confirmed wait normally produces a `QUEUED` notice, followed by `STARTED` after a confirmed grant and successful spawn; diagnostic output is best effort. Job status can show the recorded queue wait and outcome. This provides one way to coordinate these test launches across separate directories; it does not establish that coordination improves a team's workflow.

This behavior requires direct invocations using the recognized `cargo` or `bun` program token and a healthy daemon supporting the relevant Traffic capability. Recognized one-shot Bun options and test paths are eligible. Bun watch, hot, unknown or ambiguous forms, `bun run` scripts, explicit executable paths, unrecognized wrapper names and remote classifications bypass Traffic admission. A PATH shim called `bun` or `cargo` can be eligible. An older daemon without Bun admission uses legacy unresolved/lifecycle handling without a Bun Traffic reservation. Outage fallback can also execute without a confirmed reservation. `arcctl classify` alone does not prove that a particular execution was admitted. See the [direct execution guide](../../docs/managed-exec.md), [native argument detector](../../crates/arc-detector/src/argv.rs), [Bun recognition rules](../../crates/arc-detector/src/rules.rs) and [CLI admission path](../../crates/arc-cli/src/exec/traffic.rs).

The [policy](../../crates/arc-traffic/src/lib.rs) uses a one-slot TEST window, a 1-core and 1-GiB admission reservation per test, and a shared 2-core and 4-GiB budget by default. Reservations are scheduling estimates, not operating-system resource caps. The product does not promise a start time or reserve resources for bypassed work.

## Controlled observations

The task-14 [measurement receipt](mac-acceptance.json) and [handoff](task5-measurement.md) report four macOS arm64 cases using two independent directories and real Cargo and Bun test processes. The [fixture](../../crates/arc-daemon/tests/traffic_cli/bun_measurement.rs) used test-body barriers and PATH shims to make order and overlap observable. Each row represents one bypassed and one scheduled fixture case, not a distribution of production runs.

| Arrival order | Bypass peak test processes | Scheduled peak test processes | Scheduled queue wait |
| --- | ---: | ---: | ---: |
| Cargo first | 2 | 1 | 97 ms |
| Bun first | 2 | 1 | 52 ms |

In both scheduled cases the second job visibly queued; the second tool's kernel start identity followed the daemon's release timestamp stored in the committed row. Its pre-exec witness also observed that row. The timestamp is sampled before commit, so this comparison does not measure when commit returned. In all four cases each tool launched once, each test body ran once, and both exited with code 0. Queue wait is the recorded grant time minus queue time for that case; it is not a promised or typical wait.

The [measurement validator](../../scripts/coverage/traffic-bun-evidence.ts) reproduced the receipt from captured raw files. Its source identity is `cac648d93c9222589275832eaa9138d8f4fbe7241842cc65d570de8bc754d3ec`, covering 319 input paths and their hashes and modes, at accepted HEAD `16772fe2aa350413225f3fb2bb9227d14ae5a6c4` and tree `1834e0cccb50a12fea299618f9a360f4ca10e03f` with the task-13 changes in the working-tree overlay.

| Raw artifact in the execution workspace | SHA-256 |
| --- | --- |
| `target/traffic-bun/commands/task14-native-e2755f61-17da-4ce2-a553-1b4d8a97d022/receipt.json` | `6dea42852afc5af6cd9242fcb2a11b850b55997b68d9419ac3450758105689a3` |
| `target/traffic-bun/commands/task14-native-e2755f61-17da-4ce2-a553-1b4d8a97d022/observations.json` | `28fff71e71dab0c30368c740530df08e76aee1077d31d44cb4896f8273c7c75d` |
| `target/traffic-bun/commands/task14-binary-fd71dd00-00ec-4f21-af9f-7a2b2ca7963d/receipt.json` | `8dc88026db0d7336a0b304297d5f4afa95708a065b632f01b97cad7df06cb95b` |
| `target/traffic-bun/task14/source-inputs.json` | `7f5df153940fecd9f49741efd1b8678f20b9b3d6c3508f7af7de5a115a13372b` |
| `target/traffic-bun/task14/measurement-validation.log` | `f5c74c2771ef8403da5b0f37010ce62db8144b3bc9e705dbda0086e79b01defb` |

The current `mac-acceptance.json` SHA-256 is `1bf95078039c7cab81c0bbb778f595701109844f03c4eb048c48628d42e97778`. The ignored `target` files are present in the execution workspace; no installation or transfer elsewhere is claimed. Task 8 must verify them in its workspace or recollect, and a source change requires refreshed receipts and independent review.

### Historical observations

Task 11 at source identity `f7f8da187251f11c9a06169fd3f0743a40a5892b603202bd6c80b283683d0724` recorded the same bypass/scheduled peaks of 2/1 and waits of 97 ms Cargo first and 47 ms Bun first. Its receipt is retained at `target/traffic-bun/task14/historical-task11-mac-acceptance.json`, SHA-256 `23f029bbeeb37dc7495dafa26dfd6a8b53873a646c49799d99373caedadd8e6c`. Earlier task-5 captures recorded waits of 95/55 ms and 93/49 ms; those are separate historical observations, described in the [handoff](task5-measurement.md).

## Evidence limits

These controlled cases support the observed queue and test-process concurrency results only. They do not measure real-team throughput, pair makespan, peak memory, swap, CPU use, reliability across workloads, cost savings or ROI. They do not establish a customer, user-test outcome, adoption, retention, willingness to pay, price, sale or revenue. Any claim that the feature saves developer time or money, increases adoption or can support a paid offer remains a hypothesis requiring separate evidence.

The task-14 CLI preparation, native measurement and measurement validation passed. Its unfiltered workspace attempt failed in a daemon worker test; the [handoff](task5-measurement.md) identifies the failure and raw receipt. Independent review of the replacement measurement, a passing workspace run, coverage, full harness and current-source QA and final evidence binding remain with their assigned owners. This update used `oma-backend`; it did not include user research, pricing changes or outreach. The earlier task-6 update used `oma-docs` and reported a scoped documentation scan in an isolated copy.
