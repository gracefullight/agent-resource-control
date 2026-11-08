# Coverage evidence index

Use the [coverage guide](../../docs/rust-coverage.md) for prerequisites, explicit provisioning, `bun run test:coverage`, six crate verdicts and uncovered lines. Each crate must independently reach 80%; a displayed rounded percentage cannot override the exact covered/total decision.

| Evidence state | What it establishes | Where to read |
| --- | --- | --- |
| Historical receipt | A recorded result for its original source and collecting checkout. | Selected files below; not fresh amendment acceptance. |
| Current local output | The most recent collection attempt in your checkout, including failures. | Directory printed by the root command; local `baseline/latest.json` identifies that attempt. Both are ignored. |
| Fresh amendment verification | Commands actually executed for the amended implementation, within each receipt's stated source and checkout scope. | Amendment receipts below. Preliminary task-2 proof is separate from final integrated verification. |

The amendment's accepted committed source is `aacb3b4359f13c31c0705c0cffc5bf1ac00f387e`, tree `8131af56cf26cbf3d332fbcf1af00858e5a4bc3a`. The original accounting baseline remains `fbe76a5f15101ad415f355ef06f1deeb1f97d568`. Fields named `acceptedBaseline` in the collector and frozen historical files refer to that older accounting baseline, not the amendment's starting commit or proof of current bytes.

## Historical receipts

These files preserve original bytes or explicitly identify compact derivations. Their original completion/pass wording applies to the earlier coverage delivery. It does not establish this amendment's completion.

The selected historical run was collected on `aarch64-apple-darwin`, from `2026-11-08T01:44:40.486Z` to `2026-11-08T01:46:03.208Z`, run ID `2026-11-08T01-44-40-398Z-a4294aac-18e6-499d-b452-44ffb3b612ab`. Its source/configuration SHA-256 is `cf31468e23ac45fee3e5f04704e19d4b89f9b56365ef516b9daa9ebc6c9833d8`. Its original directory is `.arc-verify/rust-coverage/baseline/runs/2026-11-08T01-44-40-398Z-a4294aac-18e6-499d-b452-44ffb3b612ab/`.

The storage commit is `aacb3b4359f13c31c0705c0cffc5bf1ac00f387e`. The receipt does not record execution HEAD; storage commit is not asserted as execution HEAD. The retained summary embeds the measured input map and native-report hashes. Six historical crate passes are recorded, including arc-daemon 1507/1613 and arc-cli 203/213. These are recorded measurements, not rerun results here. The daemon child probe's exit 1 is the expected invalid-path rejection, not successful daemon startup.

| Retained category | Direct links |
| --- | --- |
| Selected run summary and uncovered lines | [Summary](../rust-coverage/historical/accepted-head-summary.json), [uncovered lines](../rust-coverage/historical/accepted-head-uncovered.md), [host and versions](../rust-coverage/historical/accepted-head-versions.json) |
| Compact command receipts | [Selected run commands](../rust-coverage/historical/accepted-head-commands.json), [repair-4 commands](../rust-coverage/historical/repair-4-commands.json) |
| Accounting contracts | [Accounting definition](../rust-coverage/contract/accounting.md), [machine contract](../rust-coverage/contract/accounting.json), [preservation references](../rust-coverage/contract/preservation-references.json), [original source inventory](../rust-coverage/contract/source-inventory.json) |
| First measurement, a different historical run | [Narrative](../rust-coverage/baseline/first-measurement.md), [summary](../rust-coverage/baseline/first-measurement.json) |
| Earlier process gaps and reports | [Uncovered lines](../rust-coverage/process/uncovered.md), [process report](../rust-coverage/process/report.md), [recovery](../rust-coverage/process/recovery.md), [library report](../rust-coverage/library/report.md) |
| Historical integration verdict | [Repair-4 report](../rust-coverage/integration/repair-4/report.md), [acceptance](../rust-coverage/integration/repair-4/acceptance.json) |
| Historical independent QA | [Review](../rust-coverage/qa/review.md), [structured verdict](../rust-coverage/qa/review.json), [driver probes](../rust-coverage/qa/driver-probes.json), [reconciliation](../rust-coverage/qa/reconciliation.json), [omission audit](../rust-coverage/qa/omission-audit.json) |
| Earlier final assessment | [Final report](../rust-coverage/final.md), [command/result summary](../rust-coverage/final-summary.json) |

The first measurement has its own run ID and source fingerprint; do not combine it with the selected run above. The contracts' historical inventory and preservation references retain their original scope. The amendment's exact-byte preservation check is an additional obligation against `aacb3b4359f13c31c0705c0cffc5bf1ac00f387e`.

Frozen reports contain original references to removed raw exports, run directories, inventories and full logs. Those references are historical paths, not promises of distributed files. Follow this index for retained destinations. To inspect an original referenced file in Git history, use its original path at the storage commit, for example:

```sh
git show aacb3b4359f13c31c0705c0cffc5bf1ac00f387e:.arc-verify/rust-coverage/baseline/runs/2026-11-08T01-44-40-398Z-a4294aac-18e6-499d-b452-44ffb3b612ab/receipt.json
git show aacb3b4359f13c31c0705c0cffc5bf1ac00f387e:.arc-verify/rust-coverage/integration/repair-4/commands.json
```

For another frozen link, resolve its original relative path from the containing historical report, then use `git show` at that commit. Compact receipts retain original path, blob/hash identity, argv, timestamps, exits and output hashes; absent metadata is labeled unavailable. Hashes identify original bytes but cannot supply missing native exports or profile bytes. Raw files remain in the original collecting workspace and Git history. Their availability in another checkout is not implied, and absolute source paths prevent treating copied exports as live corroboration there. This task did not recheck original-local raw preservation.

## Current local output

After explicit setup, run these commands serially in the checkout being verified:

```sh
bun run test:coverage
bun test scripts/coverage/evidence.test.ts
```

Read the six `PASS`, `FAIL` or `UNAVAILABLE` labels and covered/total counts, then the printed report directory's `uncovered.md`. Results use text independent of color. Command stdout/stderr is captured in numbered `*-command.json` receipts, not streamed while the subprocess runs. The three stage labels indicate tool checks, test execution, and profile merge/report validation; they are not percentage progress.

The ignored local pointer `.arc-verify/rust-coverage/baseline/latest.json` updates on failed attempts too. A clean checkout has no pointer or previous run artifacts. Missing evidence requires collection; a failed attempt requires diagnosis and successful recollection; stale inputs require finishing edits and recollecting. Do not restore an older passing pointer. After any later coverage/harness collection, run live evidence verification again in that collecting checkout. Keep native LCOV/JSON, actual profiles and the complete merge list available; summaries and receipt hashes cannot replace them.

## Fresh amendment verification

| Amendment artifact | Scope |
| --- | --- |
| [Baseline accounting](baseline.json), [retention contract](retention.json), [technical handoff](technical.md) | Accepted-source inventory and planned retained set. The frozen retention contract's `task-1-handoff-not-applied` status describes its authorship checkpoint; it is not a live implementation status. |
| [Implementation report](implementation.md), [retention operation receipt](retention-receipt.json) | Task-2 implementation and index-only cleanup observations in its original workspace. Final integration must recheck counts and original-local preservation. |
| [Task-2 commands](task-2-checks.json), [isolated-checkout receipt](task-2-clean-checkout.json) | Fresh preliminary collection and native-evidence verification for task-2, not the final integrated suite. |
| [Documentation inspection](documentation.md) | This task's source scope, retained-link checks and evidence-navigation inspection; no coverage or Cargo rerun. |

The task-2 isolated receipt records a clean start, snapshot tree `ea3f9d85304015d3c785d0a3edee13e7556a6c7b`, source hash `260b6772a9d1fa3baf329befb8d957b3ed80f58116b6be8bb973372367c169fd`, host `aarch64-apple-darwin`, and run `2026-11-08T02-41-28-401Z-cb1745f4-2e10-4627-9a70-ffb4514bd9be`. Its collection began at `2026-11-08T02:41:28.363Z`; collection and subsequent evidence verification ended by `2026-11-08T02:42:54.880Z`. Both commands record exit 0, with all six crate floors passing. These are inspected task-2 receipts; this documentation task did not execute those commands. The receipt records the original collecting checkout and native hashes; its raw data is local there.

Final amendment acceptance remains downstream at this documentation checkpoint. Expected final artifacts in this directory are `before-after.json`, `commands.json`, `local-preservation.json`, `qa.md`, `qa.json`, `final.md` and `final.json`; they are not linked as available proof until delivered. Require the specified standalone coverage, coverage verification, Cargo formatting/Clippy/tests and unchanged harness commands without skip flags, fresh source binding, final index accounting, exact preservation and independent QA. Read actual final receipts rather than inferring completion from historical or preliminary passes.
