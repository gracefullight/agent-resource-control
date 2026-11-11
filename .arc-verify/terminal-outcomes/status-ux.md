# Terminal outcome status UX review

Status: completed for task-2's report and focused verification. The binary build, status label unit test and seven status compatibility tests passed on Darwin arm64. Both exit-17 restart tests passed. The additional ten-test terminal selection returned exit 101: nine tests passed and the permit-retry test timed out acquiring the shared fixture lock before reaching its assertions. That test then passed once in isolation. This report does not mark the original selection, the integrated mission or the full gates as passing.

Only `.arc-verify/terminal-outcomes/status-ux.md` is a task-2 deliverable. Product source, tests, dependencies and mission state were not edited. Task-5 owns installation and integrated verification.

## Handoff findings for the Chief and implementation owner

All findings from this review are listed here. The tested status paths produced the expected outcome labels; no functional status mismatch was reproduced.

| Finding | Exact location and observed evidence | Expected behavior / owner handoff |
| --- | --- | --- |
| D1: guide wording is stale | `docs/managed-exec.md:48` lists normal exits as `completed` and says an unavailable Traffic query yields an unknown state whenever lifecycle status is available. In this snapshot, `crates/arc-cli/src/job.rs:109` retains validated recorded terminal outcomes, and `:252` prints `succeeded` or `failed`. Executed compatibility tests assert this behavior for exits 0 and 17, signals and spawn failures. | Technical Director / documentation owner: describe `succeeded` for a recorded exit 0 and `failed` for a recorded nonzero exit in both status views. Explain that recorded terminal evidence remains usable when scheduling status is unavailable; a record without a known outcome remains unknown. Preserve the existing successful-query and no-replay guidance. This report leaves the documentation file unchanged. |
| D2: CLI help omits query-versus-child exit semantics | Executed `target/debug/arcctl job status --help` prints the command usage and 50-record description, but no explanation of its own exit status. Its definition is `crates/arc-cli/src/main.rs:108`. Executed integration assertions demonstrate a successful status query containing `State: failed` and `Exit code: 17`; the guide already explains the distinction at `docs/managed-exec.md:48`. | Technical Director: a concise help note could explain that query success means the record was retrieved, while `State` and recorded exit details describe the child. Keep such guidance in help/status documentation so it adds no bytes to a child's stderr. This is an observed help omission and a usability recommendation, not measured user confusion or a new execution gate. |
| T1: simultaneous native checks can exhaust fixture admission | `cargo test -p arc-daemon --test traffic_cli terminal_outcomes` failed only `terminal_outcomes::failed_completion_retry_releases_one_permit` at `crates/arc-daemon/tests/support/mod.rs:56`: `daemon fixture coordination timed out: /var/folders/r9/0qvpcx794n385zwgzwfvt55h0000gn/T/arc-fixtures-501.lock`. The shared helper waits up to 90 seconds for a per-user file lock. The unchanged exact test passed in isolation in 0.460 seconds. The lock holder was not identified; `ps` was denied. | Chief / backend verification owner: coordinate native suites that share this lock and rerun the unchanged terminal selection with the integrated gates. If admission still fails, investigate the fixture helper under its assigned ownership. The initial failed run remains evidence; the isolated pass does not erase it. No lock, process, timeout or assertion was changed to obtain a pass. |

These findings are addressed to the supervising Chief and task-1 Technical Director through this delivered artifact. Documentation changes and verification coordination are outside task-2's file ownership.

## Source scope and identity

The accepted committed baseline is `6b5218cf63b174b7976e52d81d56c7e3bf81112d`, with verified product tree `8e68a263c1393aca92a5b1d435b5e37ba1d56ada`. The inspected Actor-local HEAD is the supplied input snapshot commit `101f5ee3eb03d45349b134f7f29bfac9294fa651`, with verified tree `be56e98a2524b2d7fbdfa5d307db9d5f1ed19184`. The observations and test results below apply to that snapshot's source bytes, not to the accepted baseline alone or an already integrated review checkout.

Workspace: `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/97195cbe-3041-4f04-986d-54cce01c0f14/workspace/.agent-valley/parallel/workspaces/c95158aa9e183526-78686d2704d4d690-1`.

Input artifact: `.arc-verify/terminal-outcomes/fix.md`, SHA-256 `902cc21f91894f68db2c96fd93aed5f336e245e679e77abeb31f6e103bbd5b37`. All 18 entries in its current source-hash table matched the local input files before checks. They include 14 modified paths and four added paths relative to the accepted baseline. In particular, the snapshot tracking the added test modules does not establish that they are already tracked in the mission's integration checkout. No inherited raw-log claim was needed for the fresh checks below.

The supplied snapshot and worker reference names end in `c95158aa9e183526-78686d2704d4d690-1/snapshot` and `/worker` under `refs/agent-valley/task-evidence/`. Neither ref resolves in this isolated checkout (`git rev-parse --verify <single-ref>`, exit 128). The literal snapshot commit and its tree do resolve. No revision is asserted for the unavailable worker ref. An initial combined `--verify` invocation was invalid because it supplied two references; the corrected individual checks and their failures are retained in the identity log.

Key input hashes are below; the complete 18-file comparison and additional input hashes are in `target/terminal-outcomes/status-ux-attempt-2/source-identity.log`.

| Inspected path | SHA-256 |
| --- | --- |
| `crates/arc-cli/src/job.rs` | `c2024ede50b9f4073cb7979f5f6292b04365c2a9cba5ac98721240eed7e93ccb` |
| `crates/arc-cli/src/main.rs` | `58d4ac2441980f05d5e6cedcefffc234b49173988266576e0974e9355ae8bc21` |
| `crates/arc-daemon/tests/traffic_cli/status_compat.rs` | `b8a4f0222b96b233bba8c996de4c67394f0669dbd213f4cf4dc523782dc020d6` |
| `crates/arc-daemon/tests/traffic_cli/status_compat_terminal.rs` | `510c3d874e5da90436431ef3768890a3ac7e46a006e94778ea14bbb569bdf3a6` |
| `crates/arc-daemon/tests/traffic_cli/terminal_outcomes.rs` | `642a9b3f90ab57d5c8942279b974054bb1ed63d21527568ba0958cc429b94656` |
| `crates/arc-daemon/tests/traffic_cli/terminal_outcomes_historical.rs` | `75f80c0b75324ff266a8a16edb23b80d71b8b191bd52b9254c26a027126cad99` |
| `docs/managed-exec.md` | `5e640a29cbd5b2cc1b8c16ffda1984ec62175b2d7f1ced854d9ff4e984099ac9` |

## Observed journey and wording

The preserved journey is one `arcctl exec -- <PROGRAM> [ARG...]` invocation. Investigation uses `arcctl job status` to find a recent record and, when needed, `arcctl job status <JOB_ID>` for its details. A queued invocation can supply a reference; immediately admitted execution normally supplies no reference. The recent view therefore remains a useful discovery step without requiring another execution. These are observations of the existing interface and help, not participant measurements of the shortest journey.

| Recorded evidence / situation | Recent status wording | By-ID wording and details | Verification scope in this review |
| --- | --- | --- | --- |
| Observed exit 0 | `State: succeeded` | `State: succeeded`, `Exit code: 0` | Executed native fixture and restart for scheduled and lifecycle-only paths; SQL checks require `SUCCEEDED`, code 0 and `EXITED`. |
| Observed exit 17 | `State: failed` | `State: failed`, `Exit code: 17` | Both named native restart tests passed. Each runs its fixture once, asserts one marker, exact `fixture-stderr\n` and wrapper exit 17, then independently reads SQL after restart and requires `FAILED`, code 17 and `EXITED`. Status helpers also require query success and empty stderr. |
| Recorded signal | `State: terminated by signal` | Same state, plus `Signal: 15` in the tested case | Executed real CLI/daemon queries against seeded signal records, including transient `DB_BUSY` for recent status and unavailable Traffic for detail. This is not a fresh native signal-delivery test. |
| Recorded spawn failure | `State: spawn failed` | Same state, `Reason: SPAWN_FAILED` | Executed real CLI/daemon queries against seeded spawn-failure records under the same unavailable/busy conditions; label unit assertions also passed. This does not establish a newly executed failing native spawn. |
| Confirmed Traffic cancellation | `State: cancelled` | `State: cancelled` | Cancellation label unit assertion passed; recent and detailed Traffic branches were inspected at `job.rs:231` and `:268`. No end-to-end cancellation was run for this report. If scheduling evidence cannot be retrieved, the CLI does not guess cancellation. |
| Historical `COMPLETED` with code 0 or nonzero | `succeeded` or `failed` according to the known code | Same mapping | Executed historical lifecycle and scheduled cases; unknown historical rows did not hide later known results. |
| Historical completion without an exit code | `State: outcome unknown` | `State: outcome unknown`, no invented exit code | Executed cases cover absent outcome kind and `EXITED` without a code, preserve stored row bytes, and retain a recorded verified-child observation with neutral `Reason: NONE`. Identity evidence does not imply an exit outcome. |
| Outcome cannot be obtained within the recent lookup budget | `State: outcome unknown` for unresolved completed rows; `State: unknown` when scheduling is unavailable and no terminal outcome is recorded | `State: unknown` and `Scheduling: status unavailable` for the latter situation | Executed slow-lookup and delayed scheduling tests. Recorded terminal outcomes remain visible when available. `unknown` and `outcome unknown` are two existing uncertainty wordings; neither is shown as success or failure. |
| Unconfirmed grant | `State: outcome unknown` when retrieved | `State: outcome unknown`, `Reservation: retained` | Label unit test passed; detailed rendering was source-inspected at `job.rs:270`. No new live unconfirmed-grant recovery experiment was added. |

Both query commands return success when they successfully retrieve a failed record; this is explicitly asserted by the executed tests. Their `State` and by-ID `Exit code` fields distinguish that result from child success. Query failure is separate: source at `job.rs:145` uses `STATUS_UNAVAILABLE` and says that the status request launched no work. No shell script should treat a status-query exit 0 as the child's exit code; the documentation already states this, while help has the D2 omission above.

Recent output marks each record `Last known: true`, prints Unix-millisecond timestamps and names truncation at 50 records. Traffic detail also marks last-known state; lifecycle detail gives observation time and describes identity as verified **at observation**. The healthy 50-record label test passed. The slow-lookup test passed with one shared lookup deadline and one delayed lookup, leaving unresolved outcomes unknown. This is deterministic fixture evidence, not a latency distribution or a field performance result.

Executed `arcctl exec --help` directs users to both status commands and explicitly says not to launch another copy to recover tracking. `docs/managed-exec.md:58` and `:66` make the same distinction between locally observed exit status and confirmed persistence. Status source does not print command text. Labels are plain text and do not rely on color. No new prompt, reminder, retention device or stderr diagnostic was introduced. Existing queue notices can still use stderr under the documented queue conditions; exact stderr was verified for the healthy immediately admitted fixture paths, not for every possible queued journey.

## Fresh commands and raw evidence

Checks ran on Darwin arm64 with Cargo `1.95.0 (f2d3ce0bd 2026-03-21)` and rustc `1.95.0 (59807616e 2026-04-14)`. The host reported eight CPU cores, 16 GiB memory and approximately 50 GiB free disk before compilation. Cargo commands used `CARGO_BUILD_JOBS=2` and ran sequentially in this Actor; each subprocess allowed 7,200 seconds, matching the mission's per-check timeout. No shorter wrapper timeout or test assertion was substituted. Check timestamps, durations and return codes are retained in adjacent ignored receipt JSON files.

| Exact command | Result |
| --- | --- |
| `cargo build --workspace --bins` | PASS, exit 0; 28.596 seconds. |
| `cargo test -p arc-cli status_labels_require_a_recorded_exit` | PASS, exit 0; one matching unit test, no ignored tests. Other CLI tests were filtered by this focused command. |
| `cargo test -p arc-daemon --test traffic_cli status_compat` | PASS, exit 0; seven tests, no ignored tests, 31.830 seconds including compilation. Includes legacy status fallback, old-client shape fixture, unavailable Traffic, transient busy replies and unavailable unknown rows. |
| `cargo test -p arc-daemon --test traffic_cli terminal_outcomes` | FAIL, exit 101; nine passed, one fixture-lock timeout, no ignored tests; 94.292 seconds. Both exit-17 restart cases, both exit-0 paths, 50-row status, slow lookup and historical cases passed. |
| `cargo test -p arc-daemon --test traffic_cli terminal_outcomes::failed_completion_retry_releases_one_permit -- --exact` | PASS, exit 0; one test, no ignored tests; 0.460 seconds. Verifies repeated/lost-ack completion, one released permit and recorded `FAILED`/17. This is a separate check, not a replacement result for the failed selection. |
| `target/debug/arcctl job status --help` | PASS, exit 0, empty stderr; D2 observed. |
| `target/debug/arcctl exec --help` | PASS, exit 0, empty stderr; status and no-replay guidance present. |

All raw logs stay under `target/terminal-outcomes/status-ux-attempt-2/`. No raw log is included as tracked mission evidence.

| Raw log | SHA-256 |
| --- | --- |
| `build.log` | `342523ba49e5bbc35624a2f3a1d2bce666b9bd3f4bfed36ca8954ba4ad79cfab` |
| `labels.log` | `d7eee981e37b0840a0304a1bc5dab6b6313306862258db641c1ac04aa0901386` |
| `status-compat.log` | `f76a914bc4c192c37e165f4e08527c1ede010936cb2eaf9028fa049b4ad5f74e` |
| `terminal-outcomes.log` | `2b34b21f6257727421a66eb63dc88a162731e73e7411b0ca6375a6946c5c7258` |
| `permit-retry-isolated.log` | `9b941aaec93718f3c8607ef71350706db46be5205f457be45ff2c3613a81bff7` |
| `cli-help.log` | `3c4fc844effe290972cb997a9b4787e6f0ef688192c7b4014efde1833cdf9a4f` |
| `source-identity.log` | `b2b3217523f724b192e6f1f0f3e888909d92d4261fa018a60e72cf77e7e957e8` |

## Acceptance and limits

- [x] Map recent and by-ID wording for known success, failure, signal, spawn failure, cancellation and unavailable outcomes, with executed checks distinguished from source inspection.
- [x] Review the one-execution journey, query/child success distinction, no-replay guidance and stderr behavior.
- [x] Compile the relevant source and execute focused status tests against the verified task-1 input hashes.
- [x] Report exact discrepancies, the failed attempt and out-of-ownership follow-up to the Chief and Technical Director in this file.
- [x] Use `oma-design`: Preserve scope, local OMA semantic-contract review using `resources/external-design-skills.md`, `resources/execution-protocol.md` and the affected delivery checklist. No new visual judgment or external supplier was needed; no design system, web UI or frontend stack was changed.

The configured Serena provider is not exposed in the active tool list. Inspection used direct reads of named paths, not a complete provider-assisted impact search. Global Git status printed `av.yaml: Operation not permitted`; accessible scoped status/diff were used and complete global state remains unverified. Process-list inspection was denied, so the fixture lock holder is unknown; no GUI or other bypass was attempted. The supplied evidence refs are unavailable locally as described above.

Protocol fixture checks establish the exercised protocol-1 response shapes and new-client fallback; they do not prove every historical daemon binary or a same-database downgrade. Task-1 separately reports that the accepted-baseline decoder cannot read new stored terminal state names. Full workspace tests, warning-denying Clippy, formatting, the complete coverage gate, its per-crate 80% production-line floor, the full harness, independent QA, container targets and the integrated evidence budget are not certified by task-2. Its only tracked-deliverable candidate is this report; the Chief must audit all accepted artifacts after task-5 installs them.

No participant, screen-reader, screen-based, browser or commercial study was performed. No retention, churn, conversion, comprehension or task-time effect is measured. The omission's potential effect on script authors is a hypothesis; cancellation and unconfirmed-grant end-to-end behavior were not newly exercised here. Deterministic fixture and seeded-record results are test observations, not user research.
