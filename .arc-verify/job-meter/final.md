# Local managed-job CPU accounting: Chief Director reconciliation

Status: completed for the assigned local macOS slice of MTR-002, MTR-003,
MTR-005 and AT-MTR-01. The retained implementation, corrected execution evidence
and independent final QA satisfy the unchanged verification contract. No required
product blocker or prerequisite remains in this scope. This judgment concerns
the local, uncommitted mission worktree; it is not a release or deployment claim.

Accepted and currently checked HEAD is
`18091eeb1ac0232dfdda4a937113fa3bd149b220`, tree
`c1ed103170701e587991486e318eea47d42d709b`. The implementation is the preserved
staged/unstaged/untracked overlay, not a new committed HEAD. Task-12 owns and
changes only `.arc-verify/job-meter/final.md`. It uses `oma-pm` and
`oma-coordination` to reconcile the existing assignment and handoffs, without
generating a replacement plan, dispatching another Actor, changing mission state,
compiling, rerunning gates, collecting coverage or replaying completed effects.
The six other canonical files and preceding Actors' work are preserved.

The full 509-file final input inventory agrees with current bytes and with the
backend receipts, final input freeze and independent QA:

| Binding | Current value |
| --- | --- |
| Native input digest, SHA-256 of sorted `path + NUL + file SHA-256 + LF` | `c6fea59617be640e5d9a47ce294039badace43ab971707fc46d3ff13e2dfb301` |
| Coverage input fingerprint, SHA-256 of the canonical compact file-map JSON | `e78178c3798d4d52ff3891e52c16bb77f4db5e0ddff2df775b58ab279c01ecc7` |
| Freeze inventory | `target/job-meter-task-13/final-input-freeze.json`, SHA-256 `e6fcbe06ff427e819cab40906172ee71076cd1c8fecea6e925ffd7053405f7ae` |
| Independent final QA | `qa.json`, profile `qa-reviewer`, independent and product-read-only, verdict PASS, zero unresolved product blockers |

The original goal, goalBrief, success criteria, verificationContract version 1,
Actor roster and selected Chief route (Codex, native default) remain unchanged.
The supplied empty `verifyCommand` is unchanged; no substitute command was
invented. The original 172800-second mission budget, 400-run limit, three-retry
limit and maxParallel 3 are retained, including spent attempts. Required backend
check wrappers allowed 7200000 ms each; the contract's 120000 ms planning values
were not used to terminate them. Task-12 adds no native execution or collection.

The Technical Director's decision in `contract.md` selects positive-PID terminal
`wait4` through the existing CLI-owned child and sole waiter. Periodic PID/group
sampling cannot recover descendants reaped between samples and risks overlap or
unrelated pipeline attribution. Live tree/event collection would require a wider
ownership and completeness contract and remains deferred. Existing Rust 1.95,
libc, Tokio, arc-platform, arc-meter, authenticated protocol-1 IPC, rusqlite/SQLite,
React 19, Tauri 2 and Bun 1.4.4 are retained. The sole new workspace dependency
edge is `arc-cli -> arc-platform`; no external dependency/version or Bun lock
change is required. This is an architectural reuse decision, not a measured
overhead or cost-saving result.

The Design Director's `experience.md` chooses inline selected-job measurements
over collapsed Resources or another Meter destination. The delivered detail
shows CPU used, Run elapsed and Queue wait alongside outcome, quality and scope;
technical precision and identity use Accounting details. Unknown, Partial,
non-additive nesting and retrieval freshness remain distinct. This removes an
extra navigation step structurally; no user study establishes faster diagnosis.
The Marketing Director's `value.md` keeps a later opt-in paid diagnostic pilot as
an unrun hypothesis. These decisions preserve the original technical scope and
introduce no promotion, research or retention experiment as an acceptance gate.

Backend-engineer Actors own the product changes, tests, repairs and native gates:
backend-native used `oma-backend` with `oma-db`/`oma-frontend` for shared native
contracts, CLI, integrated acceptance and recovery; backend-store used
`oma-backend`/`oma-db` for persistence and authenticated writer operations;
backend-desktop used `oma-backend`/`oma-frontend` for the bridge, UI and tests.
Independent task-9 and final task-11 review used `oma-qa`. The supplied reviewed
reconciliations of tasks 13, 14 and 10 are corroborating dispositions; current
bytes and receipts, rather than earlier completion prose, support this judgment.
No ownership or interface conflict remains.

| Original obligation | Delivered implementation and current evidence |
| --- | --- |
| MTR-002: owned-job CPU, identity, attribution and short-lived work | `crates/arc-platform/src/child_wait.rs`, CLI `exec/signals.rs` and `exec/accounting.rs` capture terminal user/system usage for the one owned child. `arc-core/src/job_meter*`, `arc-meter/src/lib.rs` and `arc-ipc/src/meter/job*` share validated additive types. `native.json` calibration and matrix plus task-11 review verify the native route. |
| MTR-003: one durable, idempotent summary and retention | Additive `crates/arc-storage/migrations/0004_job_meter.sql`, `arc-storage/src/job_meter*`, daemon `db_worker/job_meter.rs` and `job_meter_dispatch.rs` use the existing bounded single SQLite writer. Persistence, conflict/restart and retention receipts pass. |
| MTR-005: separately explained queue/run time and usable reads | Shared queue/run fields, `crates/arc-cli/src/meter.rs`, `docs/managed-exec.md`, production Tauri `bridge/job_meter.rs`, fixed `meter_summary` command, typed TS contracts/client, refresh state and `dashboard/job-meter-panel.tsx` expose the selected job. Deterministic desktop and authenticated bridge checks pass. |
| AT-MTR-01: calibrated two-worker execution including reaped descendants | `scripts/job-meter/native-workload.c` and `apps/desktop/src-tauri/tests/job_meter_acceptance/calibration.rs` provide an independent native reference and real daemon/SQLite/CLI/registered-bridge comparison. Fresh task-13 execution passes the specified engineering tolerance. |
| Existing command, protocol, host snapshot and redaction contracts | Additive `meter.report`/`meter.summary` leave lifecycle start/complete messages and `meter.snapshot` shape intact. Exact command outcome, signal/stop handling, one spawn, bounded fail-open, old-daemon capability handling, selection correlation and redaction are covered by the current workspace/Bun receipts and independent QA. |

Terminal CPU is cumulative user-plus-system **core-seconds**, without division
by host CPU count or conversion to job CPU utilization. Attribution basis is
`native_owned_child_wait4`; scope is `native_waited_descendants`: the owned child
and descendant usage propagated through explicit waits. Native stop/continue
events do not produce terminal usage. Conversion/metadata failures preserve the
original child outcome and leave unavailable measurements explicitly unknown.
The production CLI treats an opaque target as Partial with
`unobserved_descendants`; it cannot prove arbitrary detached or unwaited coverage.
Even the calibrated production report retains that conservative Partial label.
No incomplete number is presented as a complete process-tree total.

Process identity combines PID, start token and boot UUID for observer and child.
The authenticated peer and durable observer/attached child/outcome/root binding
are checked; PID or process-group membership alone cannot authorize a report.
Native ancestry and verified durable lineage support scheduled, lifecycle-only
and mixed nesting. `included_in_root` observations retain their own job outcome
and root ID but are non-additive. Unverified lineage stays non-additive and
Partial/Unknown. Root CPU is its own inclusive terminal observation; neither
nested totals nor sampled values are added to it.

Run elapsed uses one CLI monotonic interval around native spawn through terminal
observation, before reporting. It includes stopped time and excludes queueing
and post-termination I/O. Queue wait is a separate scheduler wall-clock interval,
QUEUED to RESERVED, labeled `daemon_wall_clock` / `queued_to_reserved`. A new,
affirmatively measurement-aware lifecycle-only job has established zero with
`not_applicable` / `lifecycle_only`; missing, reversed or legacy timing is null
with an unknown basis. Undetectable scheduler wall-clock jumps remain a stated
limitation. No mixed-epoch subtraction or displayed wall-time reconstruction is
used for monotonic running duration.

`meter.report` is bounded additive metadata, with one immutable report identity
per durable job. Identical retries return unchanged; conflicting reports cannot
replace accepted values. Completion remains a separate legacy-shaped operation.
The existing pre-spawn metadata deadline stays at most two seconds. At terminal
observation, the CLI fixes accounting delivery's deadline at `observed_at + 500 ms`.
Lifecycle completion runs first with its separate existing two-second deadline.
Time awaiting completion consumes the accounting window; if it exhausts that
window, accounting delivery is skipped and accounting remains Unknown unless a
valid accepted report already exists. Accounting delivery and lost-acknowledgement
reconciliation must finish within that original 500 ms window. Independent summary
reads have their separate bounded deadline. Recovery reads the same job/report or
retries identical metadata within the accounting window; it never grants another
execution, relaunches the command or starts an uploader.

The passing `reporting_outages_unsupported_peers_and_lost_acks_preserve_single_execution`
fixture covers this distinction: `PeerMode::SlowCompletion` delays completion
550 ms and asserts that no accounting report is requested. Located reads of
`exec.rs`, `exec/transport.rs`, `exec/traffic.rs` and `exec/accounting.rs` confirm
the separate deadlines, consistent with `contract.md:335-344`. Task-12 attempt 2
corrects this report's timing claim; product behavior and retained gate outcomes
remain unchanged.

The additive migration preserves the three earlier migrations byte-for-byte.
Summary payloads expire under the SRS 90-day metadata policy, anchored to the
first durable terminal fact, never to report arrival, reads or daemon uptime.
Reads enforce expiry and writer maintenance clears at most 128 payloads per
batch, preserving active jobs, leases, permits, Traffic and lifecycle/root
dependencies. Known expiry is retained distinctly from absence; late delivery
and clock rollback cannot resurrect cleared payloads. Legacy, unavailable,
not-terminated and interrupted values remain explicitly Unknown unless a valid
unexpired immutable report exists. The retention test uses a controlled clock
and real native data/private SQLite; it does not claim ninety days elapsed.

The current persisted AT-MTR-01 observation belongs to task-13's workspace test,
not to the older feasibility or task-8 runs:

| Matching-scope observation | User core-seconds | System core-seconds | Total core-seconds |
| --- | ---: | ---: | ---: |
| Independent C `getrusage(SELF) + getrusage(CHILDREN)` after explicit waits | 19.902748 | 0.057452 | 19.960200 |
| Production owned-child terminal `wait4`, persisted summary | 19.902772 | 0.057567 | 19.960339 |

Absolute error is **0.000139 core-seconds**. The unchanged criterion permits
`max(0.1, 0.05 * 19.960200) = 0.998010` core-seconds, so this passes. The Chief
recomputed the result from raw reference timeval fields and persisted CPU
components; task-11 independently reached the same result. Reference raw totals
are 19902748 user microseconds and 57452 system microseconds. The C reference
shares neither the production conversion helper nor the outer reaper; its small
printing/exit tail is included only by terminal wait4. Nominal twenty CPU seconds
is not used as the reference.

Two CPU workers ran for a requested ten seconds, each explicitly reaping four
short-lived CPU descendants; the root reaped both workers. Reference elapsed is
10.000539 s and recorded monotonic run elapsed is 10.034648958 s. Eleven workload
identities are recorded, including root PID 32958/start token 1794462804934466,
workers 32959/32960 and eight reaped descendants. The report's observer is PID
32957/start token 1794462804923580; normalized boot UUID is
`2687879a-f0b8-4b44-a7d6-934eb47cac3c`. Root identity and boot match the independent
reference. Job `2217ee782656ad818ee45cb5fec8430a` has the same values, identity,
relation, scope and Partial reason through SQLite, CLI and production bridge,
including after daemon restart.

Raw calibration artifact:
`target/job-meter-task-13/acceptance-observations/at_mtr_01_two_workers_ten_seconds_reaped_descendants_persist_on_all_surfaces.json`,
SHA-256 `b0e6ec75e56e75e85bbc93ac211fdf2f77b68ea607f58fe73708924b071ee612`.
Earlier task-1 feasibility error 0.000076 and task-8 error 0.000221 core-seconds
are preserved historical observations, not substituted for this calibration.
The historical host-meter daemon-overhead reading does not certify job totals.

The Chief independently resolved all 19 matrix cases and eight compatibility
cases to `workspace-current`, its exact successful test markers and full current
input digest. All are task-13 executions; there are eleven distinct retained
raw observation artifacts. Cases sharing one test do not represent extra
independent executions. The following groups cover every required matrix key:

| Matrix cases | Current observed result and evidence test |
| --- | --- |
| `scheduled`, `lifecycle`, `exit_0`, `exit_17`, `signals` | `real_scheduled_lifecycle_exit_signal_and_partial_summaries`: each execution mode retains exit 0, exit 17 and signal 15, with one execution per command and matching persisted surfaces. Accounting quality is independent of success/failure. |
| `queue_wait` | `real_queue_wait_is_separate_from_monotonic_running_duration`: queue prevented spawn, queue 0.424 s, run 0.125616375 s, one execution; distinct durable queue/reservation timestamps. |
| `stop_continue`, `escaped_descendants` | `real_stop_continue_and_escaped_descendant_keep_explicit_partial_scope`: stopped job stays unknown/not_terminated; continued job retains exit 17 and monotonic includes-stopped semantics. An escaped child demonstrably outlives its parent; summary remains Partial, and owned-test cleanup later observes that child absent. |
| `nested_scheduled`, `nested_lifecycle` | `real_nested_modes_use_one_authoritative_accounting_root`: four scheduled/lifecycle outer/inner combinations preserve canonical root and included-in-root/non-additive nested records; no overlapping sum is used. |
| `unrelated_pipeline` | `real_foreground_pipeline_excludes_unrelated_peer_cpu`: actual OS pipeline shares the foreground group; owned total 1.198666 vs native reference 1.198598 core-seconds, while unrelated peer CPU 0.917869 is excluded. Both pipeline outcomes are 0 and owned processes are observed absent. |
| `short_lived_descendants` | `at_mtr_01_two_workers_ten_seconds_reaped_descendants_persist_on_all_surfaces`: calibrated terminal contribution includes the eight reaped descendants within the stated scope. |
| `interrupted`, `daemon_restart` | `real_daemon_restart_during_execution_and_interrupted_observer_remain_truthful`: changed daemon identity, unavailable offline read, surviving command report after restart, and lost observer explicitly Unknown/interrupted. No restart-derived zero or execution replay. |
| `duplicate_reports`, `conflicting_reports`, `identity_mismatch`, `lost_report_ack`, `lost_completion_ack` | `real_lost_completion_report_acks_duplicate_conflict_and_identity_mismatch`: completion/report ACKs deliberately unread; independent status/summary establishes commit. Exactly one accepted row; duplicate and restart retry return unchanged; conflicts and child/observer mismatches reject; one native execution. |

| Compatibility case | Current evidence and boundary |
| --- | --- |
| `old_protocol_1` | `real_protocol_one_old_daemon_keeps_summary_unsupported_and_execution_once`: accepted pre-meter protocol-1 daemon retains execution once; CLI reports UNSUPPORTED_CAPABILITY and production bridge returns normalized unsupported, without changing lifecycle data. |
| `unavailable_reporting` | Fresh passing native CLI fixture assertion in `reporting_outages_unsupported_peers_and_lost_acks_preserve_single_execution`; unavailable report transport preserves command outcome and single execution. |
| `stalled_reporting` | Same executed fixture verifies bounded stalled reporting; no separate raw timing observation is invented. |
| `single_execution` | Same executed fixture verifies unsupported peers/outages/lost ACKs never relaunch the child. |
| `bounded_fail_open` | Fresh passing native assertion `fallback_after_slow_bypass_uses_the_original_pre_spawn_deadline` preserves the original shared preparation deadline. |
| `snapshot_preservation` | AT-MTR-01 reads/validates the existing production host snapshot independently of the new summary; schema, units and host/daemon meaning stay unchanged. |
| `redaction` | `real_job_summary_and_database_preserve_command_redaction` plus Rust/TS boundary tests check private argv/command/metadata does not leak through summary, persistence or normalized errors. |
| `legacy_unknown` | `real_native_measurement_obeys_ninety_day_boundary_without_resurrection`: production migration reopens a controlled schema-v3 fixture, retaining native/lifecycle facts but reporting legacy CPU/queue as unknown. |

Four compatibility cases above use executed native fixture assertions and exact
pass markers without separate raw observation files; that is their evidence
boundary. The other retained observations contain actual native values. The
independent final review and Chief parsed thirteen raw comparisons of SQLite
`report_json`, CLI JSON and registered production desktop data, including restart
reads. They agree, rather than merely sharing a synthetic fixture. The production
Tauri handler/registration and real owner-authenticated daemon transport are
exercised using a mocked shell runtime; no displayed desktop window is claimed.
The test-owned databases/sockets were removed by their owners before review, so
the Chief reconciled retained raw readbacks, not a new live database session.

Retention remains available at controlled time 7776000199 ms and expires exactly
at 7776000200 ms for terminal anchor 200 ms and interval 7776000000 ms. Payloads
are cleared while one job and one lifecycle row remain. Late delivery rejects,
backward time cannot resurrect the report, and active/bounded-maintenance/FK
protection is also covered by storage tests in the successful workspace receipt.

The binding operator amendment **2026-11-12T05:33:37.221Z** defines the efficiency
rule as one successful collection for the final frozen input set. A collection
whose validation failed because of tooling does not consume that allowance.
It authorizes a separate corrective backend task and one new collection after
repair and refreeze; every other obligation stays unchanged.

| Coverage attempt | Owner and actual disposition |
| --- | --- |
| Attempt 1, run `2026-11-12T05-05-53-113Z-d1c863a1-8ae1-4b58-b7ea-50250d5ae2b4` | Task-10 native collection occurred, then validation failed with `UNMAPPED_FILE: crates/arc-platform/examples/job_meter_probe.rs`. Its reuse/architecture checks also failed. Raw profiles, exports, receipts and the historical partial result remain retained. This attempt is not represented as a successful gate. |
| Attempt 2, run `2026-11-12T05-56-09-983Z-52a28cae-9ea8-4c6e-a40a-d008111c5c6d` | Separate backend task-13, authorized by the amendment, repaired metadata classification, refroze final inputs, reran affected checks and successfully collected/validated coverage. The harness consumed its generated reuse bundle without recollection. |

Attempt-1 raw receipt remains at
`target/job-meter-final/coverage/runs/2026-11-12T05-05-53-113Z-d1c863a1-8ae1-4b58-b7ea-50250d5ae2b4/receipt.json`,
SHA-256 `046f4c6b274a313e2cb0829c1359f7063a2a5f9982646b6255874a40d733d66d`.
Its failed validation and reuse are historical failures, not the current passing
receipt.

Accordingly `coverage.collectionCount = 1` means **successful final-input
collections**, while `attemptCount = 2`, `nativeCollectionAttemptCount = 2` and
`historicalFailedCollectionCount = 1` retain both attempts. This is not one
attempt. Task-10's completed local native/build/test effects and historical
failure are preserved; neither task-14, independent review nor this Chief task
replayed them. Task-13's new collection is the explicitly authorized recovery.

Task-13 uses Cargo-declared example target metadata and explicit reachable Rust
modules to classify diagnostic examples. `job_meter_probe.rs` is therefore
recognized, along with the declared desktop fixture example. Production source
reused by an example remains production. There is no blanket examples-directory
allowance: an undeclared sibling still raises UNMAPPED_FILE; forged ownership,
escaping paths and unsupported classifications reject. Metadata propagates
through collection, accounting and reuse. The initial regression was red; the
first repair also exposed macOS path canonicalization and incomplete synthetic
target metadata; the corrected focused run passed 91 tests before final gates.
All 95 production inventory entries are unchanged from attempt 1. The repair
does not exclude production code, weaken unknown-file rejection or implement
live process-tree metering.

Task-13 completed that execution and the six gates, but its subsequent evidence
review failed: 27 current case `inputDigest` fields still used an older binding.
Task-14 corrected only those fields from the actual task-13 receipts and updated
current artifact references/provenance. Native observations, times, process
identities, command/test identities, measurements, outcomes and execution owner
remain task-13. Archived original reports preserve their original hashes.
The Chief's structural comparison confirms all non-correction native fields and
gate/coverage/desktop execution sections are unchanged. Task-14 ran only the
existing small evidence check (four passes); it performed no collection or
native execution replay. Subsequent read-only reconciliations of tasks 14, 13
and 10 and independent final task-11 QA verified the corrected bindings.

| Correction/history artifact | SHA-256 |
| --- | --- |
| `target/job-meter-task-13/task-10-retained-gates.json` | `3f2263c8385de70944c2e0a0dd92d6b10d327d24842dc83faf277fc10c9afa5f` |
| `target/job-meter-task-13/task-10-retained-native.json` | `2b4e4c4371086fcdcb2a5b5dcf68af95c6ae52971f6a64728b99ba8983cf5699` |
| `target/job-meter-task-14/task-13-original-native.json` | `8bba0ef73abc9792dc45a05e8e949a2b9f88d9a7003d501203902ab5a3eba870` |
| `target/job-meter-task-14/task-13-original-gates.json` | `0bed1dd8164443d3875ee263ac7b85fe761f645d3d2a4ceab94fe8cd44e00617` |
| `target/job-meter-task-14/task-13-failed-review-disposition.json` | `2a47f62f236a87c88c148870a3aa258c65181930bb3d7bdb95631cfdeb987332` |
| `target/job-meter-task-14/case-binding-provenance.json` | `1bc97e39f3cdb5560f05c00277baf58d593c3e9cb2d35484f92ec92cb4f3a051` |
| `target/job-meter-task-14/historical-resolution.json` | `e4e456bc86a70186aefa693cf6a49656273a413ccece280656f3876b07e1d313` |

The reviewed task-13 historical-preservation audit records 20819 task-10 artifact
files/5978269748 bytes unchanged. Task-11 independently verified preservation;
the Chief checked the archived report/audit references against their bytes.
Historical original artifact hashes resolve to their retained original versions,
not to later corrected canonical files. No historical receipt is rebound or
retrospectively narrowed.

All six mandated gates below have backend JSON receipts under
`target/job-meter-task-13/`, with actual argv/cwd, timestamps, exit status, full
before/after input bindings and hashed stdout/stderr. The Chief compared the ten
gate/desktop receipt objects and output artifacts with current `gates.json` and
independent QA. Every receipt binds the same unchanged 509 files and allowed the
full per-check timeout. These are backend execution results, not Chief reruns.

| Mandated gate | Receipt | Outcome |
| --- | --- | --- |
| `cargo fmt --all -- --check` | `format.json` | exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | `clippy.json` | exit 0 |
| `cargo test --workspace` | `workspaceTest.json` | exit 0; 782 passed, zero failed/ignored/filtered, 52 test binaries |
| Explicit active-path Bun invocation below | `activeBun.json` | exit 0; 201 passed, zero failed, sixteen files |
| `bun run test:coverage` | `coverage.json` | exit 0; authorized attempt 2, all ten production floors pass |
| `./scripts/harness/validate.sh` | `architecture.json` | exit 0; zero violations, validated same coverage receipt, no collection |

Exact active Bun argv, preserved from the operator contract:

```sh
bun test apps/desktop/tests/bridge-client.test.ts apps/desktop/tests/contracts.test.ts apps/desktop/tests/credits.test.ts apps/desktop/tests/dashboard.test.ts apps/desktop/tests/lifecycle.test.ts apps/desktop/tests/meter.test.ts scripts/coverage/accounting.test.ts scripts/coverage/desktop-foundation.test.ts scripts/coverage/gate.test.ts scripts/coverage/hygiene.test.ts scripts/coverage/terminal-outcomes.test.ts scripts/coverage/traffic-accounting.test.ts scripts/harness/equivalence.test.ts scripts/harness/traffic-targets.test.ts scripts/harness/coverage-reuse.test.ts scripts/job-meter/evidence.test.ts
```

| Affected desktop check | Backend receipt | Outcome |
| --- | --- | --- |
| `bun run desktop:typecheck` | `desktop-typecheck.json` | exit 0 |
| `bun run desktop:check` | `desktop-check.json` | exit 0 |
| `bun run desktop:build` | `desktop-build.json` | exit 0 |
| `bun run desktop:smoke:tauri` | `desktop-smoke-tauri.json` | exit 0; native build/smoke receipt, no screen launch |

Production coverage has positive denominators and an 80% requirement per crate.
The Chief independently reconstructed every production file's mapped/uncovered
lines from retained raw LCOV plus the validated inventory and compared totals
with summary.json, gates.json and task-11's independent LLVM/LCOV reconstruction:

| Crate | Covered / production executable lines | Coverage | Result |
| --- | ---: | ---: | --- |
| arc-core | 612 / 625 | 97.92% | PASS |
| arc-detector | 706 / 711 | 99.30% | PASS |
| arc-storage | 2211 / 2317 | 95.43% | PASS |
| arc-ipc | 1167 / 1205 | 96.85% | PASS |
| arc-daemon | 2472 / 2716 | 91.02% | PASS |
| arc-cli | 1852 / 2182 | 84.88% | PASS |
| arc-traffic | 111 / 111 | 100.00% | PASS |
| arc-desktop | 479 / 550 | 87.09% | PASS |
| arc-platform | 464 / 502 | 92.43% | PASS |
| arc-meter | 456 / 460 | 99.13% | PASS |

The source inventory has 174 entries and 95 production files. Declaration-only
`crates/arc-platform/src/lib.rs` remains in inventory with zero authored
executable lines; no executable body is excluded by that classification. The
successful collection's 782 native tests passed. The Chief hashed all 2956 raw
profiles (1780814104 bytes), verified exact merge membership and checked the
single native collection command in that successful run. No collector or export
tool was invoked during this reconciliation.

Coverage artifacts remain in
`target/job-meter-task-13-final/coverage/runs/2026-11-12T05-56-09-983Z-52a28cae-9ea8-4c6e-a40a-d008111c5c6d/`.
The following names resolve relative to that exact directory; artifacts are
referenced, not copied into canonical evidence:

| Artifact | SHA-256 |
| --- | --- |
| `receipt.json` | `443e306e841a75b8af3927acdcb8070a78d717aa1e63caee320fa60f6303423f` |
| `summary.json` | `c504fd272b4e1af3b6541203aee6cb22cf4c71c9dd929086831b4879fa3565f0` |
| `reuse.json` | `ce387eb879fd64b0abdee0995d8645184ff5ba3eb63237760b61ab000258d8e8` |
| `source-inventory.json` | `800cbe18e47d77d66e210781ae3c68feccabb6241403fabc63ad267a4a92b25c` |
| `profiles.json` | `c9ada036535cb495ff01e38c0b7bfe6fd4edb58e5b7e89e80defaf785e3a412f` |
| `raw.lcov` | `9f11d1ad3028087f32b17574622bde83eff23f6cf1b14682406d81f17b444a8a` |
| `raw.json` | `8efef153cc145ff96284542b905ffef7b50dbfbba1ac7210f9afe2c540fcab90` |

The generated bundle's existing coverage-contract baseline
`fbe76a5f15101ad415f355ef06f1deeb1f97d568` remains distinct from this mission's
accepted HEAD. The full final source fingerprint binds this new collection.
`ARC_COVERAGE_RECEIPT` in the architecture receipt points to the generated
`reuse.json` above; the harness output explicitly names the same run and says
`no collection`. `target/job-meter-task-13/coverage-before-harness.json` (SHA-256
`1e67166d899682fb7482a80791e54fedf50a35e5569d6d5c03a910abe93409e9`) and
`target/job-meter-task-13/coverage-after-harness.json` (SHA-256
`1e42611e2d31278066343854835f435fac73ae15fb8d13a70ab28e24510682bb`) preserve
that bundle, profile set,
27 coverage artifact digests and run inventories. The source fingerprint remains
unchanged after task-14, QA and this documentation-only handoff.

Container/service observation is **not applicable**, following the accepted
task-4 clarification and reviewed task-13/task-10 provenance. Current
`containerTargets.completionRequirementsSatisfied` is true on that basis;
`configuredTargets` remains null and `checksPerformed` is false. Earlier denied
`av.yaml` readbacks remain history, not a renewed prerequisite or proof of an
empty target list. No container health check, service restart or configuration
access workaround was performed or invented.

The Chief's read-only contract audit reconciles all 39 unchanged checks: 31 exact
JSON assertions, four exact command selectors against successful backend
receipts and four file-size/required-content checks. Coverage and architecture
command receipts are also checked as the other two mandated gates. Task-12 does
not reinterpret a failed command as passing or rerun the contract's commands.
The independent reviewer reports `complete`, `evidenceVerified` and `inputsMatch`
true with zero unresolved product blockers; its product files were read-only,
with zero tests, native gates, exports or coverage collections executed.

Chief commands and outcomes are limited to `git rev-parse HEAD`, scoped Git
status/diff inspection, `git show` for the immutable migration blobs, located
document/source reads, and inline Python standard-library artifact audits.
The completed audits pass: 509 inputs, 27 exact case bindings/markers, eleven raw
observations, thirteen persisted-surface comparisons, ten gate/desktop receipts,
94 unique current artifact references (110176854 bytes), all ten LCOV production
floors, all 2956 raw profiles, calibration arithmetic and original/corrected
history comparisons. Early read-only audit snippets needed corrections for a
stdout string/object assumption, truncated large-output parsing and a Python
parenthesis error; the corrected bounded audits passed without changing product
or evidence inputs. No test, gate or collection was launched by those snippets.

Task-12 attempt 2's inline Python/Git audit again passed the 39 unchanged contract
assertions, receipt selectors and file requirements: 509 input hashes, all 27
case bindings and pass markers, eleven raw observations, thirteen persisted-surface
comparisons, ten gate/desktop receipts, 99 selected artifact references, all ten
recorded production coverage floors, three original migrations and 61 source/test
lengths. Calibration arithmetic and both coverage attempts reconcile; the harness
receipt confirms reuse without recollection. One patch command initially rejected
unmatched context without writing; the corrected scoped patch succeeded.
`git diff --check -- . ':!av.yaml'` passed. The untracked report's no-index
whitespace check emitted no diagnostics and exited 1 for the added-file
difference; the audit wrapper initially treated that exit as a failure. The
corrected interpretation preserves the empty diagnostic result.

Full diff inspection emitted `av.yaml: Operation not permitted`; scoped status
excluding that protected path succeeded. No whole-worktree clean claim is made.
Serena discovery exposed no tools, so the Chief used only already located source
reads and evidence/document inspection, with no replacement native source search.
No skill/config definitions, manifests, lockfiles, assignments, acceptance
criteria, mission graph or Actor state were changed by this task.

The three original migration digests still match accepted Git blobs:

| Preserved migration | SHA-256 |
| --- | --- |
| `0001_phase0.sql` | `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384` |
| `0002_managed_exec.sql` | `464a1c88270f01a56ccab6610f87eff7648e0640f276ebce53b8a97026c50814` |
| `0003_traffic.sql` | `1f4522e3f27c124aea7127d92434c7bd22be36168fef0a4bf665f8cf16debe90` |

Fifty-four added source/test files and seven additionally checked coverage files
remain below 500 lines. The maximum retained inventory count is 454 lines for
`crates/arc-cli/tests/job_accounting/support.rs`. There is exactly one small
evidence checker, `scripts/job-meter/evidence.test.ts`, 137 lines; its four tests
passed in the backend receipts. No second checker or source/test file is added.

Final canonical evidence contains exactly these seven regular files, no nested
entries or symlinks, totaling **1798185 bytes** (limit 5000000):

| File under `.arc-verify/job-meter/` | Bytes | SHA-256 |
| --- | ---: | --- |
| `contract.md` | 53906 | `45132566365cec29812106d62d6597900259ba2f648ae5a866de6b7009f8ee63` |
| `experience.md` | 35349 | `c7106eb50dc06048b9b6f3e3dbc0afdb741d75838ca82b28bee1b8cabccb23ac` |
| `value.md` | 9564 | `467eba5181d7b6847a08640ce0e28dda9f1e268aae3974ed57654f1a679f6f39` |
| `native.json` | 1085944 | `968d2ccaa1eeb05088c7c73e9239ec393b013daccfc762970191b1dae3dc1a9d` |
| `gates.json` | 268518 | `82c4f889cfd01592426aa0a7494b9fb4b52589cf1d897d108c5df9ec041cb17b` |
| `qa.json` | 307946 | `adde432df9ca9325b1498791243ce606ff8804911dd8a79efa446be661acaaa1` |
| `final.md` | 36958 | This report; its delivery digest is computed after the final write. |

Coverage, correction and historical artifacts stay at their existing referenced
target paths outside this canonical directory. The six preceding canonical
digests remain unchanged. The initial task-12 exclusion-scoped before/after audit
preserved 3500 preceding regular files' paths, modes and bytes (SHA-256
`d1e196ab56cf2f5314e2a1cd58b16e5e6bb5c7265bcbc77909b531c0d14dfe00`), along with
identical cached and unstaged diff digests outside final.md/protected av.yaml.
Attempt 2 independently preserves all 3925 tracked/nonignored untracked entries
outside those two paths, including modes, regular-file hashes and symlink targets
(inventory SHA-256 `4132c7fa34d1f84fd89730205994c201f5f78cc7bd685113ebe5a77439faf66c`),
with identical cached and unstaged diff digests. Only final.md changes in this
repair; product, configuration, mission state and prior evidence remain intact.

No optional visual inspection was performed. No browser E2E, browser installation,
GUI workaround or new visual acceptance requirement was introduced. Deterministic
React tests cover selection races, retained/stale measurements, unsupported and
invalid replies, keyboard/disclosure behavior and lifecycle retirement. Actual
screen rendering, contrast, enlarged text and assistive-technology behavior
remain unverified; the native smoke build is not screen or user-study evidence.

User comprehension, diagnostic time, retention, conversion, pricing, revenue,
customer savings, CAC, ROI and LTV remain unmeasured. The proposed three-person
design comparison and five-person paid diagnostic pilot are **not run**. No
outreach, publication, promotion, spending or external distribution was performed
in this assignment. CPU core-seconds are neither monetary savings nor vendor
token charges. Marketing and recurring-prompt ideas require later authorization
and evidence; they do not qualify or weaken the completed technical acceptance.

Task-12 acceptance: the report maps the original local slice to current product
paths and independently reviewed receipts, preserves the amendment and both
coverage attempts, distinguishes execution from task-14 correction, reconciles
all required behaviors/gates/coverage floors, and fits the seven-file evidence
budget. Product/IPC/CLI/desktop interfaces remain compatible. No unresolved
required blocker is returned to a backend owner. The supervising Chief may use
this material file for its existing mission completion process; this task does
not mutate that process or claim a commit, push, publication or deployment.

vendor/project aggregation: outstanding

live process-tree metering: outstanding

Windows: outstanding

vendor hooks: outstanding

full Meter acceptance: outstanding
