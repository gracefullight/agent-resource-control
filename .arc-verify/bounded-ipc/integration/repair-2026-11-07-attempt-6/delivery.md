# Task-4 collection repair, attempt 6

The missing attempt-5 result report has been restored in the mission receiving checkout from the exact retained Actor artifact. Destination: `.agents/results/result-backend-task-4-attempt-5-b137e253-def6-46ef-a75d-25ab414784ea.md`; bytes: 6,665; SHA-256: `8388a5252ff58f31a1fab8d6bdf5d4a4ba49b1b9fdbe1bb359e0a1e2369b0165`.

The portable copy `result-backend-task-4-attempt-5.md` in this directory has identical bytes. Collect the destination report explicitly even if an ordinary Git tree collector excludes `.agents/results/`. The same applies to the attempt-6 result report named in `index.json`. Do not edit the historical manifest to conceal a missing generated report.

The existing `integration/evidence.json` and `integration/delivery.md` remain the authoritative product evidence index and consumer interface. This directory supplements them without changing any historical hashes. `ARC_RUNTIME_REPORT` remains the report channel, with its full schema and 4,096-byte cap; no production code or interface changed.

The unchanged attempt-5 checker ran in this receiving checkout: exit 1 before restoration for the exact report path, exit 0 afterward. It verified 50 product source files, 60 current artifacts, 166 historical entries and seven command receipts. Original receipts retain 52 focused and 173 workspace passing invocations; no Cargo or runtime workload was replayed for this artifact-only repair. Migration and harness byte comparisons against accepted HEAD passed. The three recorded harness failures remain unresolved.

`index.json` binds this repair's produced artifacts and both result reports. Run `python3 -B .arc-verify/bounded-ipc/integration/repair-2026-11-07-attempt-6/verify_collection.py` after collection. It validates the supplemental hashes and invokes the original unchanged checker. This establishes receiving-checkout collection, not an independent product review or mission completion. The Chief retains independent review and remaining task coordination.
