# Task-5 attempt-8 collection correction

Status: partial. The missing original report is restored and staged for normal
collection. Strict current-source and historical-boundary checks fail on newer
independently owned changes. No implementation, runtime test or measurement was rerun.
Used oma-backend and the installed backend-engineer definition.

The accepted committed baseline remains
42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0, tree
c802f71ac4d91ed6b8a602d5c83bf9cf57116b76. The inspected Actor snapshot/HEAD is
0192ccb29570654d2a5408162ad262deafdcd63d, input tree
4746b22753c020491e2570d684e5edd86c02d468. Restoration and these new files are
Actor-local task edits, not changes already in the accepted committed baseline.
No commit or ref was written. No cancellation was received; the saved deadline
2026-11-08T10:52:35.601Z and all run/retry/concurrency limits remain unchanged.

## Restoration and collection

Read the exact retained source in the mission workspace at
.agent-valley/parallel/workspaces/d6c6b45f4a6c3769-e7c08de777760acd-2/.agents/results/result-test-engineer-task-5-uninjected-attempt-3-b137e253-def6-46ef-a75d-25ab414784ea.md.
Its 8,417 bytes have SHA-256
43c6657a2ddb223d78e1fa4299b719499a8527d90d741e843d66a6878fce991d and equal
resume-attempt-3/delivery.md byte for byte. Copied those bytes without changing
prose, identity, historical status or timestamps inside the report to:
.agents/results/result-test-engineer-task-5-uninjected-attempt-3-b137e253-def6-46ef-a75d-25ab414784ea.md.

Executed exactly `git add -f --` followed by that report path: exit 0.
Only that report is staged; the staged blob equals the restored bytes. The ignored
report is now visible to Git-based collection. New evidence is nonignored and
confined to collection-repair/attempt-8/. changeSet.json lists the actual local
added paths. No existing manifest, script, log, receipt, product file, test,
document, lockfile, migration, harness, definition, configuration or mission state
was edited. Existing attempt-7 correction evidence is preserved byte for byte.

Normal host collection and the receiving review checkout cannot be observed from
this Actor. Local staging does not establish that the collector included the report.
The Chief must ensure the collected changeSet includes the explicit restored path.
If the collector omits it again, report that exact failure without another unchanged
reassessment loop. No write was made into the integration or review checkout.

## Check evidence and exact conflicts

`python3 -B .../resume-attempt-3/verify.py` before restoration: exit 1 for the
missing indexed report. After restoration: exit 1 at current product fingerprint
equality, before it reaches its historical boundary check. Both raw failures are
retained here. The original verifier is unchanged; no assertion was suppressed.

The new read-only `check.py` evaluates independent checks, reports every failed
requirement, and exits 1 on any failure. It verifies the actual 107 indexed paths,
including the restored report, at their unchanged indexed sizes/hashes. It checks
all readable pre-repair inputs, reproduces the original source fingerprint
algorithm, validates historical command/source/output bindings, compares migration
and harness bytes directly with accepted HEAD, and verifies the two task-6 document
sizes/hashes against task-6/attempt-3/delivery.json. It rejects any additional
historical boundary differences. Its results are retained in check.json and the
execution receipt; they are correction checks, not fresh Cargo results.

Current source fingerprint is
9e3335b98f73d9ef964809c8fbb08fb1530661adb1d6e2c1adac492d28543970 across 51 files.
Retained final-source.json is
b3610549adfebffdd626a760741a200c848ec7ee26a88482bcf7e38e563383c4 across 50 files.
Differences: added crates/arc-daemon/src/database_failure_tests.rs and modified
crates/arc-daemon/src/lifecycle.rs. These paths match the supplied task-4 dependency's
reported ownership, but this correction cannot rewrite or promote historical
receipts onto their newer bytes. check.json records exact expected/actual hashes.

The historical 120-file boundary has the original task-5 resident.rs/support/mod.rs
changes plus the two correctly attributed task-6 documents. It also differs at
crates/arc-daemon/src/lifecycle.rs, docs/bounded-ipc-final.md and
docs/bounded-ipc-value-scope.md. These extra differences are rejected explicitly;
no exception was added for them. Chief coordination and current-tree QA must resolve
the scope mismatch without reverting other Actors or weakening this assignment.

## Observed correction-check outcome

`python3 -B .arc-verify/bounded-ipc/implementation/collection-repair/attempt-8/check.py`
exited 1. Exactly two requirements failed: currentProduct and historicalBoundary.
Collection verified all 107 indexed artifacts; all 2,262 readable pre-repair inputs
were unchanged. Both task-6 document hashes matched. Historical receipt/source/output
bindings passed for their historical fingerprint, with currentSourceBound=false.
Migration and harness were byte-identical to accepted HEAD. These successful sections
do not override the two failures or prove receiving-checkout collection.

## Failed collection history and remaining work

Attempt-7 evidence recorded a local restore/stage and 107 local artifacts; this
attempt-8 snapshot again lacked the report. Prior collection/review rejections
00405e10-593c-4244-95b7-10af107f74c5,
cd421f47-3a7d-4713-ae47-59e1e3aaa185 and
17bf2b0f-9307-42c6-99aa-aeffef564009 remain failures. 106 present artifacts never
satisfied the unchanged 107-entry index. Existing failed-attempt evidence is retained.
The initial broad git status encountered denied av.yaml access; scoped Git reads
and standard-library file reads were used. Existing shell startup emitted mise
config-tracking permission warnings. No GUI workaround, installation or download ran.

After collection, the independent technical-director must inspect this new checker
before executing it with python3 -B in the actual review checkout. Verify all 107
paths there, preserve its nonzero current-source/boundary results, and distinguish
successful collection from current-product acceptance. Current-tree task-8 independent
oma-qa and task-9 final assessment remain required; older fingerprints cannot close
this tree. av.yaml remains unreadable, so configured-target applicability is unmet.
Three harness failures remain: oversized resident.rs, oversized IPC library, and
missing test:coverage. Authoritative SRS comparison requirements and remaining
Phase 0, Linux and Windows runtime/measurement obligations stay open. No mission
completion, external delivery or deployed-service proof is claimed.

## Original task-5 acceptance, unchanged

1. Deterministic unit, subprocess and actual CLI checks cover every mission invariant and the 100,000-attempt accounting.
2. Both signal cases enforce five seconds, prove actual handler/worker termination and validate reopen/integrity.
3. Native measurements are real, reproducible and separated from structural proofs; unavailable SRS comparisons remain unmet.
4. Tests are discoverable and run by the required workspace suite; focused results and raw measurement artifacts are retained.

Historical receipts remain historical execution evidence. This repair changes no
protocol, CLI, worker or test interface and establishes no fresh performance result.
