# Task-5 collection correction

Status: collection repair completed in the assigned checkout; ready for normal
collection and independent technical-director review. This is attempt 7 of the
existing task, not a new mission or a fresh implementation/test run. Used
oma-backend and the installed backend-engineer definition.

## Restored deliverable and collection

Restored only the missing generated report:
`.agents/results/result-test-engineer-task-5-uninjected-attempt-3-b137e253-def6-46ef-a75d-25ab414784ea.md`.
Read the exact retained Actor source under
`.agent-valley/parallel/workspaces/d6c6b45f4a6c3769-e7c08de777760acd-2/`.
Its original 8,417 bytes have SHA-256
`43c6657a2ddb223d78e1fa4299b719499a8527d90d741e843d66a6878fce991d`
and equal `resume-attempt-3/delivery.md` byte for byte. Prose, historical status,
identity and timestamps within the report are unchanged.

The report directory is ignored by `.gitignore:5`. Executed exactly:

```
git add -f -- .agents/results/result-test-engineer-task-5-uninjected-attempt-3-b137e253-def6-46ef-a75d-25ab414784ea.md
```

Exit 0. Only this report is staged. The index blob equals the restored file.
New correction evidence is confined to this `collection-repair/` directory and
is nonignored, so normal Git-based Actor collection can include it. No commit,
ref change, ignore-rule change, product edit or mission-state write occurred.
`collection.json` records the method; `changeSet.json` lists the actual delta
against the pre-repair inventory. This is local collection readiness, not proof
that the receiving review checkout has collected the report.

## Checks and evidence

- `python3 -B .../resume-attempt-3/verify.py` before restoration: exit 1 for the
  indexed missing report. Raw failure is retained in `original-verifier-before.txt`.
- The same unchanged verifier after restoration: exit 1 at
  `docs/bounded-ipc-recovery.md`. Raw failure is retained in
  `original-verifier-after.txt`. No original assertion was disabled or rewritten.
- `python3 -B .arc-verify/bounded-ipc/implementation/collection-repair/check.py`:
  exit 0. `check.json` contains current-checkout results. All 107 indexed entries
  exist with their unchanged indexed sizes and hashes. The restored path itself
  is checked, not replaced by the retained delivery path.
- All 1,986 readable pre-repair inputs retain their bytes/link targets and modes.
  `av.yaml` remains unreadable; its absence from content verification is explicit.
- The original fingerprint algorithm and an independent reproduction agree on
  all 50 product files:
  `b3610549adfebffdd626a760741a200c848ec7ee26a88482bcf7e38e563383c4`.
- The historical 120-file boundary has the two original task-5 test differences
  already bound to final-source.json, plus exactly two task-6 document differences:
  `docs/bounded-ipc-recovery.md` and `docs/runtime-identity-usage.md`. Their current
  sizes/hashes match task-6/attempt-3/delivery.json. Any other difference fails.
- Historical fmt, Clippy, workspace, focused and harness receipts retain exact
  command, source and output bindings. Logs still reconcile 173 workspace and 51
  focused passing invocations. Raw counters, signal termination/integrity records,
  native RSS observations and P95 samples pass the retained verifier's checks.
  These are historical execution receipts, not rerun commands.
- Migration and harness bytes were directly compared with accepted HEAD
  `42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0`; both are byte-identical.

The checker is read-only: Git reads, Python standard-library reads and inspected
retained helper imports only. It does not execute Cargo, the harness, a benchmark,
the collector or a mission command, and uses no browser or GUI. No installation,
download or build occurred. No OMA runId/claim path was injected; none was invented.

## Failure history

The prior collection omitted this ignored supplemental report. Reviews
`00405e10-593c-4244-95b7-10af107f74c5`,
`cd421f47-3a7d-4713-ae47-59e1e3aaa185` and
`17bf2b0f-9307-42c6-99aa-aeffef564009` remain rejected; 106 present indexed
artifacts did not satisfy the 107-entry index. Those records and all prior
manifests/scripts/logs/receipts are unchanged.

This repair's first in-memory inventory followed existing skill symlinks to
directories and rejected the resulting read errors before writing files. A
premature staging attempt then failed because restoration had not occurred.
The corrected inventory uses lstat/readlink without following symlink targets.
Both failed attempts and the subsequent successful restoration/staging are retained.
Shell startup also emitted an existing mise config-tracking permission warning;
it did not prevent the recorded noninteractive checks.

## Original task-5 acceptance, unchanged

1. Deterministic unit, subprocess and actual CLI checks cover every mission invariant and the 100,000-attempt accounting.
2. Both signal cases enforce five seconds, prove actual handler/worker termination and validate reopen/integrity.
3. Native measurements are real, reproducible and separated from structural proofs; unavailable SRS comparisons remain unmet.
4. Tests are discoverable and run by the required workspace suite; focused results and raw measurement artifacts are retained.

Historical receipts supporting these criteria are preserved and checked for byte
and source bindings. This correction adds no implementation or runtime evidence.
The historical report's host-specific SRS comparison claim remains historical;
current authoritative SRS applicability and any unavailable comparisons remain
the downstream assessment's responsibility.

## Required receiving-checkout review and remaining work

After normal collection, the independent technical-director must inspect check.py
before running it with `python3 -B` in the actual review checkout. It must observe
all 107 indexed paths, including the restored report, unchanged product/evidence
inputs and exactly the attributed task-6 document differences. If collection still
omits the report, report that exact failure; do not repeat unchanged reassessments.

Then retain task-8's current-tree independent oma-qa and task-9's updated final
assessment. Older fingerprints do not close this tree. Configured-target
applicability remains unmet while av.yaml is inaccessible. The unchanged harness
still has three failures: oversized resident.rs, oversized IPC library and missing
test:coverage. Remaining Phase 0, Linux and Windows runtime/measurement obligations,
long-run CPU and broader recovery qualification remain open. No mission completion,
external delivery or deployed-service health proof is claimed.
