# Evidence-hygiene claims audit

Status: completed for task-4, attempt 2. The claims audit identifies an Actor-snapshot integration discrepancy; it does not certify mission completion.

Question: Which delivery and benefit claims follow from the measured repository state and the supplied Director evidence?

Owner: Marketing Director, applying the installed research-explorer definition. Only this report changes. Implementation, documentation, the Git index and mission state remain with their assigned owners.

## Source scope

The accepted committed baseline is `aacb3b4359f13c31c0705c0cffc5bf1ac00f387e`, tree `8131af56cf26cbf3d332fbcf1af00858e5a4bc3a`. The inspected Actor-local HEAD and supplied snapshot commit are `1850339111b1b0e35d1117edd2fa2361e8637f7c`, tree `1f0bc2e5e2c1df171e310655583d531611106390`. Git resolved both commit/tree pairs. They are different scopes: the Actor snapshot includes implementation and report overlays absent from the accepted baseline.

Neither supplied ref under `refs/agent-valley/task-evidence/8753207314f4ee46-8f9ba36d8e59dd86-2/` resolved locally: `snapshot` and `worker` each returned exit 128, `Needed a single revision`. The actual inspected commit above is not substituted for either missing ref. No receiving checkout or delivery receipt was inspected. The scoped index diff was empty before editing; this report is an unstaged task edit until managed integration.

The preceding claims report is available at this snapshot's `HEAD:.arc-verify/evidence-hygiene/claims.md`, SHA-256 `e472845c01204ed50af24ebd173dc6469284dcf46c0bff69063eb83c4bc74d51`. Its earlier Actor revision and documentation findings are historical. This audit refreshes those statements against the current input snapshot.

## Measurements and recorded results

Evidence class: local Git measurements and repository documents. Confidence is high for directly reproduced counts and hash equality. Execution and preservation results from other workspaces are attributed receipts, not fresh execution here.

| Scope | Coverage files | Coverage blob bytes | All evidence files | All evidence blob bytes | Prohibited files |
| --- | ---: | ---: | ---: | ---: | ---: |
| Accepted baseline, reproduced | 927 | 260,548,113 | 2,628 | 278,635,154 | 456 |
| Task-2 originating index, recorded | 24 | 475,720 | 1,732 | 18,659,837 | 0 |
| Later staged checkpoint, recorded | 24 | 475,720 | 1,740 | 18,836,277 | 0 |
| Prior complete-delivery overlay, recorded | 24 | 475,720 | 1,744 | 18,987,445 | 0 |
| Current Actor snapshot HEAD/index, reproduced before this edit | 932 | 260,660,104 | 2,652 | 279,171,829 | 456 |

Sources: [baseline.json](baseline.json), `before` and `prohibited.union`; [retention-receipt.json](retention-receipt.json), `afterScope`, `after` and `finalLocalRecheck`; [before-after.json](before-after.json), `classification` and `after`; [final.json](final.json), `accounting`; direct Git checks below. Count each blob path once and sum uncompressed Git blob sizes, including repeated object IDs at separate paths. These checkpoints must not be substituted for one another.

The recorded coverage reduction is **903 paths and 260,072,393 logical blob bytes**. This Actor index does not reproduce that reduction. Its 456 prohibited paths total 252,272,711 bytes. The audit inspected run/profile directories, coverage extensions, known raw-export names and JSON export type. The existing hygiene suite also tests renamed native exports. Overlapping classifications are a union, not additive totals.

[retention.json](retention.json) selects 24 destinations spanning accounting contracts, summaries, uncovered lines, compact receipts and reviews. The selected-evidence provenance test passes here. The contract's `task-1-handoff-not-applied` label is its original proposal state; the operation receipt records later application in the originating index. The receipt records 908 index removals with original files preserved locally, including 456 raw files, and history preserved. This Actor lacks `target/evidence-hygiene/baseline-local-manifest.json`; original-workspace preservation was not independently reproduced here.

[commands.json](commands.json) records source-bound coverage, clean-checkout, Cargo and harness results. Its two initial harness attempts exited -15 after about 120 seconds; subsequent wrapper and direct harness receipts exited 0 after 234.146 and 211.329 seconds. Those failures remain failures. [qa.json](qa.json) records completed independent oma-qa with PASS; [final.json](final.json) records the prior Chief ACCEPT decision. These are inspected records for their stated source and workspace, not a fresh acceptance verdict for this snapshot or the revised report. The supplied task-2 review likewise covers that task's originating index.

All 118 implementation files listed in `final.json` under `bindings.implementationInputFiles` match their recorded SHA-256 values in this Actor checkout. The recorded source identity is `260b6772a9d1fa3baf329befb8d957b3ed80f58116b6be8bb973372367c169fd`. This verifies input-byte equality; it neither fixes the index discrepancy nor makes native reports with absolute paths replayable here.

## Corrections for the Chief and owning Actors

| Location | Observed issue or claim risk | Correction and owner |
| --- | --- | --- |
| Actor snapshot `1850339…`, `.arc-verify/rust-coverage/` | The snapshot tracks 932 coverage files and 456 prohibited files. Its hygiene suite fails the file-count requirement. The originating receipts describe a different, compacted index. | Chief and implementation owner: preserve the reviewed index-only removals during integration, preserve original local raw files, then rerun receiving-index hygiene and accounting. Do not describe this Actor snapshot as the compacted final tree. This task makes no index edits. |
| [before-after.json](before-after.json), `after`; [final.json](final.json), `accounting` | Checkpoint counts are correct only for their recorded scope. Reusing them as present Actor or post-resumption totals would be misleading. | Final-report owner: recompute the receiving index and complete delivery after all report changes; name revision, checkpoint and method. Preserve earlier receipts as earlier results. |
| [qa.json](qa.json), `reviewedInputs.reviewedFiles`; [final.json](final.json), `bindings.deliveredDependencyArtifacts` and `chiefChecks` | Both bind the preceding claims report's bytes. This authorized task-4 revision changes that artifact. Old report hashes cannot establish review of the new report. | QA and Chief owners: bind and assess the revised delivery in their downstream work. Keep the preceding review valid only for its original inputs. No implementation recollection is implied by this prose-only change. |
| [Historical final report](../rust-coverage/final.md), opening completion/current-HEAD wording and raw-run references | Read alone, the frozen wording could be mistaken for amendment acceptance or a promise that removed raw paths are distributed. | Keep original bytes. The maintained [index](index.md), Historical section, already supplies original provenance and retrieval instructions. Cite the historical report through that context; no further documentation defect was found in this audit. |

The earlier report's proposed guide corrections are now present: the [coverage guide](../../docs/rust-coverage.md), “Reports and source identity” and “Verify current local evidence,” identifies ignored pointers/runs, separates the older accounting baseline from amendment HEAD, and requires fresh collection before native-evidence checks. Treat those prior documentation findings as resolved, not outstanding defects.

The [PRD](../../docs/prd.md) describes broader orchestration. The supplied Technical, Design and Marketing Director inspections describe implemented ARC status, doctor and literal command classification, with syntax analysis rather than command execution. These are attributed Director observations, not a new product-source audit here. The [README](../../README.md) names ARC and links contributor coverage. This amendment supports claims about this evidence workflow; it establishes no broader orchestration, desktop, Windows, commercial or Phase 0 completion capability. The guide states the remaining platform/work boundaries.

## Benefits, costs and scope

Expected maintenance benefits are hypotheses: fewer tracked generated files may reduce review clutter and accidental re-addition; explicit Historical/Fresh labels may reduce mistaken acceptance; a compact index may help readers locate uncovered lines. No contributor session, lookup-time comparison, support-hours measurement, defect-rate study, retention or conversion test was supplied or performed. There is no measured time saving or customer reaction.

**Current-tree Git blob-byte reduction does not measure clone-transfer reduction when history is retained.** Old objects remain accessible in history. Packed-object sizes, fetch choices and actual transfer bytes were not measured. Original raw files remain local, and new collection creates ignored outputs. Index-only removal establishes no disk-space saving.

Financial effects are unknown: revenue, price, willingness to pay, CAC, delivery/support cost, runtime cost and realized margin were not measured. The supplied operations record reports unknown total cost; a known-estimate subtotal of zero does not mean execution was free. Logical blob bytes and command durations do not establish ROI, saved labor or monetary savings.

The supplied goalBrief defers service offers, promotion and user studies because they do not advance this amendment. Pricing, outreach, spending, commercial experiments, campaign artifacts and retention manipulation add external effects or product behavior without satisfying the engineering criteria. None was performed or is needed for acceptance. Proposed channels and service offers in Director advice remain hypotheses, not demand or revenue evidence.

Delivery wording supported after receiving-index checks: “Reduced tracked Rust coverage evidence from 927 files / 260,548,113 logical blob bytes to 24 / 475,720, preserved local raw files and history, and verified fresh collection and native evidence in a checkout without prior run artifacts.” Attach current receiving-index accounting, preservation proof and source-bound command receipts. Until then, attribute these after-values to their recorded checkpoints.

## Checks, skills and limits

Commands executed in this attempt:

- `git rev-parse HEAD` and `git rev-parse HEAD^{tree}`: exit 0, snapshot identity above. Accepted-commit/tree resolution: exit 0. Supplied snapshot/worker ref resolution: exit 128 each; no substitute ref invented.
- Python aggregation of `git ls-tree -r -l -z <revision> -- .arc-verify`, with `git cat-file blob` inspection: exit 0; reproduced baseline and Actor counts and the prohibited union above. `git diff --cached --name-only -- .arc-verify` was empty before editing. This is local snapshot evidence, not receiving-index verification.
- `bun test scripts/coverage/hygiene.test.ts`: **exit 1; 3 pass, 1 fail, 212 assertions, 1.65 seconds**. Failure: `HYGIENE: rust-coverage count 932 must be <150`. Selected provenance, deterministic ignore probes and renamed-export/count-limit checks passed.
- The operator's environment-change check was rerun unchanged. The reported `pathspec magic ... literal` failure did not recur; deterministic ignore checks passed. The remaining count failure is specific to this Actor snapshot and is reported above rather than conflated with that framework defect.
- Python comparison of all 118 recorded implementation-input hashes: exit 0, all equal. JSON inspection confirms separate failed and successful harness receipts. Original raw preservation and recorded command streams were not replayed here.
- Report/retention validation: exit 0; 17 local Markdown links resolve, all 24 original source hashes match accepted-baseline bytes, 22 retained files match exactly and two derived receipts retain Historical provenance. Scoped diff confirms only this report changed. `git diff --check -- .arc-verify/evidence-hygiene/claims.md`: exit 0.

Applied research-explorer attribution, evidence-class and confidence rules. Consulted `oma-search` and its shared code-intelligence contract; their local-only routing excludes external research here. `oma-market` and `oma-scholar` were not run because no community or academic question is required. Serena discovery returned no tools; inspection used Git metadata and supplied/known evidence/document paths. No native source-code search substituted for Serena.

No coverage collection, Cargo suite, harness, independent final QA, external research, browser, installation, publication, messages, spending or campaign ran in this task. Only this Markdown artifact changes; runtime interfaces, thresholds, implementation and task instructions are unchanged. Remaining coordination items are receiving-index hygiene, updated downstream report bindings and final managed verification. Community and academic planes were outside scope; financial effects remain unknown.

Input anchors, SHA-256: `baseline.json` = `2cf1d7a5b3a89e470dcb0a7a9e883436f7a459c9da3e28ae9c96ecc3742dba42`; `retention.json` = `809b2cd13df460a0ad11afebdb818f60d216e8bff001998fa0b5687fb9053115`; `retention-receipt.json` = `8443c459ce8211c740c40a955268f26f7e361f25a8dc783261bd239908a65d75`; `before-after.json` = `1545468ddee5bfdb0fde81e1da351c1d336f6c6bd2d8166be980ae31e418414a`; `commands.json` = `69972af435fb8955c7c7b8f33621bb56c4b7970370e8a04ece73a0b26fb244ed`; `qa.json` = `b69f6118899cdc7c285520313473028aa276287c5f50c0a26c436bd310651f79`; `final.json` = `c6eb07283cd14b29fa5d7f97d264ffe75265149466466ff48f8da36b4afb9caa`. These bind inspected input bytes, not future integration.
