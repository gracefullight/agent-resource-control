# Task-5 current-source recovery handoff

Status: scoped recovery work completed; normal receiving collection and independent review are pending. No mission-completion claim. This report is for the supervising Chief Director.

Only new files under `.arc-verify/bounded-ipc/implementation/recovery-current/` were written. The original generated report path already existed on entry and remains byte-identical. No production, test, documentation, manifest, lockfile, migration, harness, configuration, collector, commit, ref, mission state or saved assignment was edited. `before.json`, the current-source manifests and the read-only checker verify this boundary. The original worktree changes were inspected first and retained.

Skill used: oma-backend, with the installed backend-engineer definition, execution policy, result contract and backend checklist. Rust/Tokio/rusqlite and the five-crate Cargo workspace were detected from Cargo.toml. Backend-specific web/JWT/ORM defaults do not replace this established authenticated local IPC design. Serena tools were not exposed, so inspection used supplied known paths, scoped Git metadata and non-code evidence. No agents, recursive AV operations, browser/GUI, Playwright, installations, downloads, standalone builds, external delivery or mission-state writes occurred.

## Collection repair

The retained Actor source in `collection-repair/attempt-8/restoration.json`, the original receiving report and `implementation/resume-attempt-3/delivery.md` are byte-identical:

- Original path: `.agents/results/result-test-engineer-task-5-uninjected-attempt-3-b137e253-def6-46ef-a75d-25ab414784ea.md`.
- Additional normal payload: `.arc-verify/bounded-ipc/implementation/recovery-current/report-payload.md`.
- Size: **8,417 bytes**. SHA-256: **43c6657a2ddb223d78e1fa4299b719499a8527d90d741e843d66a6878fce991d**.
- Binding: `collection-binding.json`, explicitly declared in the Actor output and supplemental artifact index.

The unchanged original 107-entry index was verified independently; every original indexed path exists and matches its exact size/hash, including the original report. The historical tree `6cab57f62ebeb0ae38ee9609c974f15816390abc` omits that original generated path: direct git show still exits 128. This recovery does not replace an index entry, stage the file again or claim the original appears in that tree. The nonignored supplemental payload carries the same report bytes through normal collection.

Receiving verification has not happened yet. After normal collection, the independent technical-director must inspect `check.py` before executing:

```
python3 -B .arc-verify/bounded-ipc/implementation/recovery-current/check.py --receiving-delivery <actual-AV-delivery.json>
```

The supplied file must be the actual receiving AV delivery record with its supplied `changes` and `deliveryTree`, not an Actor-authored substitute. The checker requires both payload and binding in that changeSet, verifies their actual tree blobs/OIDs, compares payload bytes with the original receiving report, and verifies all 107 historical indexed paths. It returns observed tree/path/blob/hash bindings through stdout for supported review output. If payload is omitted or the original receiving report is absent, report that exact collection prerequisite. Do not repeat an unchanged restoration/staging loop or write into another checkout. A local run without receiving delivery deliberately exits **2**, meaning pending collection, with distinct results for integrity, owner attribution, current inputs and current acceptance. It does not claim successful post-collection verification.

## Owner attribution

`attribution.json` enumerates all differences across the historical 50-file product manifest, original 120-file boundary and attempt-8 full snapshot. It contains no broad path exclusions or unexplained differences:

- `crates/arc-daemon/src/lifecycle.rs` changed and `crates/arc-daemon/src/database_failure_tests.rs` was added by accepted task-4 classification repair. All 303 integration artifacts and its 51 current source hashes/modes verify against integration/evidence.json and the source-bound repair-2026-11-07-attempt-7 receipts.
- The two original task-5 test edits, resident.rs and support/mod.rs, match the unchanged historical final-source.json. All their prior assertions remain in the current source; attempt-9 made no test edits.
- `docs/bounded-ipc-recovery.md` is 17,203 bytes, SHA-256 d690a0432c86a418557a0dac6bc772e6cc68cded79245678f5ed167f616317b5. `docs/runtime-identity-usage.md` is 15,863 bytes, SHA-256 5c5a852344a000f2e9aa3b93aa6487a3b09262512b5de40d2473fd804c73773a. Both documents and all 18 task-6 attempt-4 evidence additions are attributed to that independently accepted delivery. All 19 indexed artifacts (index excludes itself) and all 20 delivery changes match actual file bytes, Git blob OIDs and modes.
- The task-6 tree object was not locally installed: cat-file and the retained Actor-path probe exited 128. The retained actual delivery.json and patch are copied as supplemental provenance. The checker independently recomputes the full snapshot Merkle root, verifies every change's before OID/mode, recomputes each after blob from current bytes, and reconstructs every tree to the supplied root **aee07c312f488891b288f6fa1e06973417b414cc**. The exact root matches; no object/ref was imported or invented. The retained patch hash and source-evidence.json also match. This is content verification of that tree, not a claimed successful git show of an unavailable object.
- `docs/bounded-ipc-value-scope.md` is attributed to task-7's actual delivery and unchanged accepted followup. Current bytes match the delivered blob and accepted followup tree. Original delivery root 200679950bc1e49ebcdbf14df1407bcf0458eaf2 is also independently reconstructed.
- `docs/bounded-ipc-final.md` matches task-9's owner receipt report hash and the retained later snapshot. Its 50-file assessment remains historical. It must be refreshed by task-9 after current-source task-8 QA; this recovery does not rewrite its acceptance claim.
- All 15 original attempt-8 delivery paths match their retained change manifest and reconstructed root 6cab57f62ebeb0ae38ee9609c974f15816390abc. The separately restored original report and all task-6 additions account exactly for the added paths after the attempt-8 pre-repair snapshot.

The current product fingerprint is **9e3335b98f73d9ef964809c8fbb08fb1530661adb1d6e2c1adac492d28543970**, 51 files. The original historical fingerprint remains **b3610549adfebffdd626a760741a200c848ec7ee26a88482bcf7e38e563383c4**, 50 files. Both reproduce using the original sorted path/SHA-256/mode JSON algorithm. A mismatch is not relabeled a historical pass.

## Execution and acceptance evidence

New execution, with UTC intervals, exact argv, process exit, output hashes, source-before/source-after manifests, nanosecond samples and native observations:

| Command | Observed outcome |
| --- | --- |
| `cargo test -p arc-daemon --test bounded --test worker_runtime -- --nocapture` | Exit 0, 22.986 seconds, 56 passed invocations (3 bounded + 53 worker-runtime); zero failed, ignored, measured or filtered. 2026-11-07T17:04:13.729030Z to 17:04:36.715187Z. |
| `python3 -B .arc-verify/bounded-ipc/implementation/resume-attempt-3/verify.py` | Exit 1 at verify.py:52, historical 50-file versus current 51-file mismatch. Original checker and index unchanged. |
| `python3 -B .arc-verify/bounded-ipc/implementation/collection-repair/attempt-8/check.py` | Exit 1; repairBoundary, currentProduct, task6Attribution and historicalBoundary failures retained verbatim. Those historical fixed comparisons were not weakened. |
| New read-only `check.py` without receiving argument | Exit 2, no integrity/attribution/current-source/acceptance failures; actual receiving collection pending. Final output and execution receipt are delivered separately. |
| `git check-ignore` for supplemental payload/binding/checker | Exit 1, empty stdout/stderr: paths are not ignored. This is not a collection receipt. |
| Direct migration/harness comparison to accepted HEAD | Byte-identical; exact hashes in preservation.json. |
| Supported read of av.yaml | PermissionError / Operation not permitted. No bypass. Configured-target applicability remains unverified. |

Task-4's exact gates were reused only after checking each complete current input manifest, mode, argv, exit, UTC interval and output SHA-256 against all 51 current inputs:

- `cargo fmt --all -- --check`: exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0.
- `cargo test --workspace`: exit 0, 183 passing invocations across 17 suites, none ignored or filtered. This includes actual CLI, protocol, identity/TMPDIR, single-daemon, same-user, filesystem and preservation regressions.
- `./scripts/harness/validate.sh`: exit 1. The unchanged harness retains resident.rs at 901 lines, arc-ipc/src/lib.rs at 551 lines, and missing test:coverage. No dummy coverage, skip flag or tooling migration was added.

These are retained source-bound task-4 executions, not newly executed task-5 Cargo gates. Task-8 must still execute the exact gates, unchanged harness and its independent native workload on the current tree. It may not promote its older 50-file QA to current acceptance.

Fresh 100,000-attempt accounting is separated by workload and every admitted attempt has exactly one terminal category:

| Workload | Attempts | Admitted | Rejected | Terminal admitted work |
| --- | --- | --- | --- | --- |
| Production Handlers::try_spawn, authenticated native socketpairs | 100,000 | 8 | 99,992 | 8 shutdown-cancelled; 8 handlers spawned/reaped |
| Production Requests::try_admit | 100,000 | 4 | 99,996 | 4 released without dispatch; no waiting tasks |
| Production DbClient::try_doctor with gated real worker, default queue 8 + executing 1 | 100,000 | 9 | 99,991 | 9 client-cancelled; retained accounting held until worker cleanup |
| Actual default daemon request/response load, four persistent clients | 100,000 | 100,000 | 0 | 100,000 completed; no timeout or shutdown cancellation |

Per-attempt assertions and final reports enforce connections, in-flight requests, tracked tasks including unreaped completion, queued SQL, one executing SQL, outstanding/retained DB work, retained request/response budget and one active rejection. Every final gauge is zero. These structural assertions are separate from performance measurements.

Subprocess evidence covers stalled headers and partial bodies, malformed/oversize/version frames, full request/connection saturation, actual nonreading write backpressure, queue overload and status/doctor recovery in the same process. Saturation recovered in 303.651 ms under the 300 ms frame test budget plus the documented 250 ms scheduling margin; nonreader closure was observed at 114.114 ms with an actual write timeout. Paused-time tests cover absolute/trickled deadlines. Controlled native SQLite preparation was still stalled while actual CLI health completed in 4.612 ms, with transport admission available.

Both signal tests ran with full transport admission, partial body/header clients and outstanding database work. SIGINT-to-reaped-exit was **5,126 microseconds**; SIGTERM was **10,178 microseconds**, each below five seconds. Each reports 11 handlers spawned/reaped, one worker started/joined, no handlers remaining, zero gauges, socket cleanup, successful SQLite reopen/integrity, a new process acquiring admission and successful status/doctor after restart. This is actual accounted termination, not dropping a handle or timing out a caller.

Fresh native default-daemon observations: idle RSS **5,636,096 bytes**, lifetime peak RSS **6,062,080 bytes**, sampled native thread peak **4**, retained-task high water **4**, and 10,000-sample small-message P95 **90,542 ns (0.090542 ms)**. The separate after-load 200-sample P95 is 85,000 ns. performance.md discloses host/profile, native APIs/units, warmup, sample sizes, message sizes, concurrency and overhead. The available authoritative ARC SRS was read and its exact SHA-256 and requirement lines verified. NFR-002 idle RSS <50,000,000 bytes and NFR-004 P95 <=20,000,000 ns both pass on this macOS debug host. No peak-RSS/thread/task performance target or improvement over accepted HEAD is invented.

## Original acceptance retained verbatim

- Deterministic unit, subprocess and actual CLI checks cover every mission invariant and the 100,000-attempt accounting.
- Both signal cases enforce five seconds, prove actual handler/worker termination and validate reopen/integrity.
- Native measurements are real, reproducible and separated from structural proofs; unavailable SRS comparisons remain unmet.
- Tests are discoverable and run by the required workspace suite; focused results and raw measurement artifacts are retained.

Current evidence for these acceptance statements is in acceptance.json, performance.json, the 14 focused-raw artifacts, full focused stdout and unchanged source manifests. Available SRS comparisons were completed rather than repeating the older missing-target claim. Independent task review is still pending normal collection.

## Risks and Chief follow-up

1. Collect this nonignored payload and binding normally. The technical-director reviewer must return actual received tree/path/hash bindings and original-report comparison through supported review output. Absence is an exact collection prerequisite, not another restoration task.
2. Task-8 owns current-tree independent oma-qa gates, unchanged harness and native workload. Task-9 then refreshes its final assessment. No older 50-file acceptance is promoted.
3. av.yaml remains denied; configured/deployed-target applicability and actual target health, availability and recent-error proof remain unmet. Owned test subprocesses do not replace deployed-target observations.
4. Preserve all three harness failures and remaining Phase 0, Linux native qualification, Windows Named Pipe/runtime/tray and native measurements. No long-run idle CPU, every forced-crash mode, full Phase 0, external delivery or commercial result is claimed.

Saved deadline 2026-11-08T10:52:35.601Z, maxParallel=3, maxRuns=200, maxRetries=3 and spent attempts were retained. Before new execution the deadline was checked and no cancellation instruction had been received. Only one Cargo process ran. No cancellation-control tool or injected OMA claim/run ID was exposed; none was fabricated. The supported Actor result is returned with this handoff and explicit artifact paths.

Failed attempts retained: the historical two checkers' nonzero results; initial snapshot symlink-directory read errors (before-initial.json) corrected only in this new evidence snapshot with readlink/lstat; direct absent task-6 tree/Actor-path probes (replaced by read-only Merkle verification, not a Git import); exploratory known-path reads of nonexistent optional receipt names; and mise's nonfatal tracking-link warnings. No failed test or acceptance threshold was weakened to pass.
