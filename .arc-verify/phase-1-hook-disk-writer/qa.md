# Independent QA

Reviewed snapshot: local HEAD `63e554fd39de354b149078cb36d42fd4a22d1051`, tree `6d75d8008cfa68e60c3baec6d848a008ff448e20`. Those match the supplied snapshotCommit and snapshotTree. Baseline is commit `257e4ad1c05956727900296ecae87df693b41842`, tree `e68c481795bb551c20703dfb309ac3274e4aae45`. Lane bytes are in the snapshot commit, not the baseline. `snapshotRef` and `workerRef` do not resolve here. The product worktree matches HEAD. `av.yaml` returns `Operation not permitted`.

This review reads source. It did not rerun the named tests or the fmt, Clippy, workspace, Bun, coverage, architecture, or SwiftPM gates. Earlier scoped passes are prior claims. Skill: oma-qa. Stack audit tools and browser checks were not used.

`contract.md` is 4,660 bytes and names Lane A, Lane B, and Lane C. In this snapshot `closing.md` still leaves AT-TRF-01 item 2, MTR-001 item 3, and R-014 items 2–3 Partial (lines 1977, 1956, 2013). `traceability.md:1211` still says Phase 1 is 61 Complete, 8 Partial, 0 Pending and 2 Blocked of 71. Integration owns those records. ADR-003 stays Partial (`traceability.md:889`). This report does not mark a row Complete.

## AT-TRF-01 item 2

Route: the installed hook returns Codex `permissionDecision: "allow"` plus `updatedInput` (`crates/arc-cli/src/hooks/event.rs:137-144`). Claude still has no permission decision (`130-136`). Both commands run on one real daemon with a Build window of one.

Test: `agent_test_journey::at_trf_01_codex_claude_installed_hooks_share_one_build_slot` (`agent_test_journey.rs:468-470`), body `codex_journey::run`, using `Sandbox::provisioned`. A violation fails one of these:

- Both orders (`codex_journey.rs:35-133`). The second start count stays unchanged until release (`89-91`). Each tool then starts once (`132-133`). Permit high-water is 1 and the last value is 0 (`201-202`).
- Codex has three fields and `allow`; Claude has two and no decision (`helpers.rs:117-123`). Other input fields round-trip (`124-127`). The command hides the raw session and tool ids (`128-130`). Stored keys are distinct 64-hex values (`137-141`). Vendors are `claude-code`, `codex`, `codex`, `claude-code` (`136`).
- v1/v2 summaries omit `codex` (`153-167`); v3 and the current CLI include it (`168-200`). An old advertisement omits `--arc-vendor` (`283-285`); `CODEX_ADVERTISEMENT` adds it (`296-300`).
- Malformed or unsupported input yields an empty decision (`204-218`). With no daemon, stderr is `TRACKING_UNAVAILABLE` plus tool stderr and the stub starts once (`302-317`).
- One duplicate event adds one job and one `HOOK_EVENT_DUPLICATE` row, and the second returned command still starts (`222-257`). That is fixture redelivery, not a live once-only vendor execution. The two distinct children are the once-each case.
- Existing PostToolUse and the other PreToolUse entry survive (`319-421`).

Observation: offline fixture (`codex_journey.rs:1`, `README.md:17`), not a trusted session or an observed approval prompt. README has install, removal, and `permissionDecision`. `docs/managed-exec.md` has no `hooks install codex` (Lane C owns the guide, Lane A owns README). No task time or churn was measured.

## MTR-001 item 3

Route: a real daemon samples `/`. The test writes an owned file, reads `arcctl meter snapshot --json` through `Isolated` (cleared environment, temporary HOME, explicit socket and config, forwarded `LLVM_PROFILE_FILE`), and brackets samples with its own `statfs`.

Test: `meter_readings::mtr_001_disk_pressure_during_real_write_workload`. It fails if identity or capacity disagrees (`disk.rs:38-75`); if derived pressure disagrees with the integer ratio (`82`) of Critical at or below 5 percent, Warning through 10, Normal above (`disk_native.rs:161-169`; boundaries `181-190`); if available bytes miss the native bracket by more than 16 MiB (`disk.rs:88-90`); if writes do not advance (`104-108`); if the file remains (`215`, unwind `disk_native.rs:194-208`); or if no later sample arrives in 60 seconds (`disk.rs:144-146`). A separate APFS volume must share the sampled container (`164-181`).

These are real writes, not injected readings. This review did not resample the host, so a Normal pass is not a low-capacity result. Disk files are 219, 208, and 307 lines.

## R-014 items 2–3

Item 2. `meter.events.latest` (`connection.rs:331-334`) uses `coalescing::latest`: pending is 0 or 1 (`crates/arc-ipc/src/coalescing.rs:62-69`). `decode_latest` rejects other pairs (`55-59`).

Test: `traffic_policy::r_014_slow_consumer_coalesces_latest_and_retains_audit` in `crates/arc-daemon/tests/traffic_policy/coalescing.rs`. The cursor stays put while publications continue. It fails without three increasing sequences in 90 seconds (`83-107`), if the resumed event is not pending 1 or is older than the last sequence (`110-113`), or if the four `POLICY_SWITCH_CHANGED` rows change (`47-81`, `125-129`). Audit rows stay in SQLite, outside the one replaceable slot.

The stalled party is that cursor consumer. `worker.rs:225` publishes, then `226-228` runs the sample hook before the next sample. `meter_cadence.rs:16-18` does job-tree and attribution work there, so a blocked hook can delay later samples. The named test does not stall that hook, and `crates/arc-meter` is unchanged versus the baseline. Follow-up for the meter owner. SRS §13.4 matches the cursor route: coalesce UI samples, keep audit events.

Item 3. `retention.rs:89-136` admits bounded passes while clients are outstanding. A run is loaded only when `DbOutstanding` never hits zero (`outstanding.rs:21-24`).

Test: `traffic_policy::r_014_scheduled_rollups_progress_during_sustained_load` in `crates/arc-daemon/tests/traffic_policy/retention.rs`. Eight doctor clients loop (`168-237`). All 42,000 seeded rows (two jobs, seven processes, 3,000 sweeps) must roll in one loaded run of at least three passes (`425-434`). It also fails if clients stall or error (`369-371`, `410-415`), if minute, hour, or day sums or distinct jobs differ (`373-407`), or if the two audit rows change (`414`). `MAX_RAW_BATCH` is 20,000 (`rollup.rs:41`, `91`), so a dense minute is split; the expected peaks require those splits to add. An idle-only gate would miss `loaded_raw_rolled_up == 42000`.

## Blocking defects

1. Ownership. `contract.md:9` gives Lane C `db_worker.rs` only. Lines 16–19 limit Lane A to schema expectations in `db_worker/tests.rs`. The snapshot still changes unassigned `crates/arc-daemon/src/db_worker/execution.rs` (race-test `Shared` gains the tracker) and new `crates/arc-daemon/src/db_worker/outstanding.rs`. Integration adds one owner per file and leaves the `tests.rs` assignment unchanged.

2. Persistent DDL, reproduced. `rollup.rs:66-70` creates `resource_rollup_pending_sweeps` and `idx_resource_samples_rollup_next` inside `roll_up`. Neither name is in `crates/arc-storage/migrations`. Temp tables in that batch are not this gap. Integration adds a migration that preserves an existing table and index, then removes that lazy persistent DDL.

No other blocking product defect was reproduced. No full-run failure that passes alone was seen here.

## Follow-ups

- Records stay Partial and 61, 8, 0, 2 of 71 until integration updates them from its own check.
- Codex setup is in README, not in `docs/managed-exec.md`.
- The hook fixture is not a live vendor session. The disk test does not claim low capacity. The host ratio was not remeasured here.
- The sample hook can still delay the next publication.
- Older Meter tests still call `Sandbox::new`. Integration owns that helper. This wave's Codex and disk paths do not use it.
- The runner owns formatting, Clippy, the workspace test, Bun, coverage, traceability, and the architecture harness.
- No field study or revenue figure exists. None is a gate.

Verdict: the three named tests and the assertions above are in this snapshot and can fail. Two integration-owned gaps remain. This is the only QA file and is not rewritten after later deliveries.
