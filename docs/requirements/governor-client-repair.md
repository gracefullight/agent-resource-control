# Governor client repair — task-7, attempt 2

Status: completed for the assigned client repair and focused checks. **Governor slice only.** This report supplements the preserved [attempt-1 handoff](governor-client-handoff.md), whose completion claim was rejected for the three supplied defects. Task-8 receives these client implementation and acceptance-test changes through this Git-visible delivery. Independent review and mission-wide acceptance remain downstream.

## Source provenance and ownership

- Accepted committed baseline: `08e7e526cce7e03ca9264afc926ed2e878dfde6d`; verified tree `d3cbd610653d0b78adeb2ffc26a9cab1790e877f`.
- Actual inspected Actor-local HEAD and supplied input snapshot: `e72d0b4ab1c8a8a0a580f44a23edcc836c75025a`; verified tree `6ac8ff0938d35e2107a3442fb89f9a99fdfe296a`. It already contains preceding contract, runtime and client work absent from the accepted baseline. This repair is an uncommitted delta over that snapshot, not a change already present in either committed tree.
- Supplied `refs/agent-valley/task-evidence/b29b06c236fcb043-014e4ceaa5a9e511-2/{snapshot,worker}` each returned exit 128 when resolved individually. The actual snapshot commit/tree resolve; neither missing named ref is replaced with another checkout revision. The reviewer must verify integrated bytes against the hashes below; Actor-local tracking does not establish tracking in the review checkout.
- No edits to shared manifests/locks, generated TS/fixtures, daemon/storage/policy implementation, original migrations, canonical Governor reports, prior handoffs, saved instructions, assignments or mission state. The scoped preservation comparison against the snapshot passed; migrations 0001–0005 also match the accepted baseline byte-for-byte.
- Used the installed backend-engineer profile: **oma-backend** primary for CLI lifecycle/control repair; **oma-db** for release authority, committed outcomes and recovery-key reconciliation; **oma-frontend** for registered bridge, structural and deterministic UI verification. No subagents or new AV/OMA run were started. No OMA claim/run identity was injected.
- Configured Serena has no callable tools in this environment. Inspection used scoped reads of named files; no substitute native project-source search. Unscoped Git inspection reported `av.yaml: Operation not permitted`; it remains uninspected. Scoped status/diff and protected-input comparisons succeeded.

## Repairs and compatible interfaces

1. Before native spawn, sync the existing witness with the committed unreserved release nonce doubled to the daemon’s established 64-hex recovery key. Both new/unreserved and reserved-to-unreserved paths use the same owner/job file. Preserve the enqueue key in memory and restore it when an expired, acknowledged-unspawned release returns to scheduled admission after resume. The on-disk witness format and daemon query remain unchanged.
2. On unreserved native spawn failure, sync SPAWN_FAILED evidence before dropping the launch fence. Do not send UNSPAWNED, which clears the release and requeues the job. Drop the socket/fence, then complete with the still-valid release ID; discard evidence only after confirmed terminal recording. Cancellation retains its existing cleanup. A missing acknowledgment never permits another spawn.
3. A replayed committed control receipt triggers an identity-aware current-state read without rebasing/resending the write. Text labels the historical receipt separately and displays the current state/revision. JSON uses the existing GovernorSwitchReadResult shape: current `switch` plus historical `mutation_result`.
4. Extend the existing opt-in, delay-only acceptance checkpoint with a `spawned` stage and comma-separated stage selection. It cannot authorize execution; an error after spawn continues existing child supervision rather than claiming no child. The single native spawn site, protocol 1, supported-command classification, witness ownership, Meter reporting and legacy command semantics remain covered.

## Final-input focused evidence

All commands below ran serially from this Actor worktree:
`/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/6dd4dd59-bec1-4e67-a1ba-65f73e0660ce/workspace/.agent-valley/parallel/workspaces/b29b06c236fcb043-014e4ceaa5a9e511-2`

Cargo used `CARGO_BUILD_JOBS=2`; each executable-check wrapper allowed 7200000 ms. The mission’s absolute 172800-second budget, maxParallel 3, maxRuns 400 and maxRetries 3 were retained. Host: macOS arm64, 8 CPUs, approximately 85 GB free at preparation. Verified tools: Rust/Cargo 1.95.0, Bun 1.4.4. `bun install --frozen-lockfile` restored the missing 85 local packages (exit 0), without manifest/lock changes. No Playwright/browser package or binary was installed.

Checked consumer/contract input digest: `fd680cec9ee4226da6edac12422913e49deeed9cd1cefcb12c093a3365eaae12` (224 paths; distinct from a final coverage identity). For reproduction, union `git ls-files -- crates/arc-cli apps/desktop crates/arc-ipc Cargo.toml Cargo.lock package.json bun.lock` with nonignored untracked paths under crates/arc-cli and apps/desktop; sort paths, build `{path,sha256}` rows, serialize as compact JSON with sorted keys, and SHA-256 hash UTF-8 bytes. Each command verified those bytes first. Unchanged runtime dependencies remain bound to the inspected snapshot above.

Raw logs/metadata stay local under `target/governor/task7/attempt2/`; the compact outcomes, selectors, source hashes and observations here are the delivered evidence. Raw hashes alone do not imply those ignored logs move to another workspace.

| Check / exact argv | Exit / outcome | Log / SHA-256 |
| --- | --- | --- |
| `cargo test -p arc-cli` | 0; 105 passed, 0 failed, 0 ignored | `cli.log` / `c5e5dc2be21df6adade5f84c2d5f02be61d0b0d3dc1214a80cadb60b29a81c06` |
| `cargo test -p arc-desktop --lib --test registered_commands` | 0; 26 passed, 0 failed, 0 ignored | `bridge.log` / `7eea4ce99e36f1278f0fe88dff1febd9d75a88cfa534e3a380dc1652862f5fd3` |
| `cargo test -p arc-ipc --lib governor::typescript::generated_types_are_current_without_writing -- --exact` | 0; 1 passed, 0 failed, 0 ignored | `rust-contract.log` / `97fc70e9091bcda957c86c85846dbc93bed7ec02218e3c2f2aff4e864197404e` |
| `cargo clippy -p arc-cli -p arc-desktop --all-targets -- -D warnings` | 0; passed | `clippy.log` / `7107f1b97d59fdf91f2c76042ec1372f207e8115182825c5b2330ef1c31cb19c` |
| `cargo fmt -p arc-cli -p arc-desktop -- --check` | 0; passed | `fmt.log` / `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `bun test apps/desktop/tests/governor.test.ts apps/desktop/tests/governor-contract.test.ts apps/desktop/tests/bridge-client.test.ts apps/desktop/tests/contracts.test.ts apps/desktop/tests/credits.test.ts apps/desktop/tests/dashboard.test.ts apps/desktop/tests/lifecycle.test.ts apps/desktop/tests/meter.test.ts` | 0; 133 passed, 0 failed | `bun.log` / `815e9d1b0b518b3a5bd3bd8b3a8d3fe5edbfac6b44dd04ba580724b8259e3ee4` |
| `bun run desktop:typecheck` | 0; passed | `typecheck.log` / `7c3cb6045e4508ad4ca264a0bdbb3207275c78410b843909c0ea071a97268c0f` |
| `bun run desktop:check` | 0; passed | `desktop-check.log` / `c35f5ff6172889954db8b7427a96865adc10b3c73eb3e878f1adb7fe4307668e` |
| `cargo test -p arc-cli --test governor native_recovery:: -- --nocapture` | 0; 3 passed, 0 failed, 0 ignored; compact native observations | `native-final.log` / `0443aa15b71b6c403f0f0a4da6c06cc54a6505d53c176a0ec1572520229c9d94` |

The native receipt reruns three tests already included in the 105 CLI total; counts are not additive. The bridge total is 5 library + 21 actual registered-command tests. Its authenticated socket fixtures prove dispatch/DTO/uncertainty behavior, not production SQLite/desktop reconciliation. Bun’s 133 tests cover the eight explicit files, including G01–G13, keyboard/focus/help, applying/rejection/uncertainty, revision ordering, replay, conflict, navigation and reconnect; Rust regeneration agreement and compiler-backed structural/optional/null/tag/numeric checks passed without rewriting generated output.

### Native recovery observations

Production resident startup/routing/authentication/writer/recovery are reused through the maintained daemon fixture, with explicitly injected Normal pressure. This is not a native Critical OS event or a workload-performance measurement. Each row below had a real child, a wrapper killed before ACK/attachment, matching SQLite pid/start/boot recovery, one target marker and zero active permits at the end. “Released” is the final count of retained permit rows; no blanket reservation deletion occurred.

| Route | Daemon restarted | Job ID | Child PID / start token | Starts / released permits |
| --- | --- | --- | --- | --- |
| New unreserved | false | `f9a49be4f9018d6f19f3e2705ecc191f` | 44407 / `1794514950193003` | 1 / 0 |
| New unreserved | true | `d4dcfbe516b0e767b065a0c693627f66` | 44421 / `1794514950437170` | 1 / 0 |
| Reserved → unreserved | false | `75daf879fbefb705dcc1f8eacfb4ac5b` | 44437 / `1794514950652964` | 1 / 1 |
| Reserved → unreserved | true | `687f72d9c01812cbde06615bfd301029` | 44453 / `1794514950856051` | 1 / 1 |
| Expired unreserved → resumed scheduled | false | `61dbe2c46f6cb1db167e11188e3658c0` | 44468 / `1794514953212561` | 1 / 1 |
| Expired unreserved → resumed scheduled | true | `e5e79cfca494a8815ac90de050d3ea76` | 44482 / `1794514955581778` | 1 / 1 |

All six recovered the same native child as RUNNING, then reconciled its disappearance as OUTCOME_UNKNOWN/ABANDONED with no invented exit outcome. The wrapper cannot report an observed exit after it is killed. The restored enqueue key is checked against SQLite after expiry/resume; converted reservations remain exactly one RELEASED row.

Two additional native cases run literal `cargo test` with an isolated missing executable and a non-executable file. Both persist SPAWN_FAILED with the exact NOT_FOUND/ENOENT or PERMISSION_DENIED/EACCES outcome, no child identity, COMPLETED Traffic state, no queued row, no active permit and zero target executions. CLI status confirms SPAWN_FAILED. A production control case pauses at revision 1, resumes at revision 2, replays the original pause in text and JSON, and independently checks SQLite remains enabled/revision 2 while the original OFF/revision-1 receipt stays historical.

### Reproduced failures and repairs

TDD_EVIDENCE: these observed RED runs returned exit 101. Final CLI/native checks above are GREEN on the current input digest. The first three used the original client production bytes from snapshot e72d0b4, with uncommitted regression-test additions. The fourth used an intermediate uncommitted repair before restoring the enqueue key; no complete intermediate input digest was captured, so it is reproduction evidence only.

| RED selector (cargo test -p arc-cli --test governor … -- --exact) | Observed failure | Log / SHA-256 |
| --- | --- | --- |
| `replayed_pause_receipt_displays_current_state_after_another_client_resumes` | Printed Paused/revision 5 after intervening resume. | `replay-red.log` / `61fd37d3dcb66db772b0b59020924f575d0a876f4f59faab375324fe1c17cead` |
| `execution::negotiated_new_queued_and_reserved_jobs_run_unreserved_once_with_native_lifecycle_and_meter` | Witness key differed from release_id concatenated with itself. | `witness-red.log` / `e8bb809f89361cc255b2facda1f82c28d221559fab3c7c80a1a5db7c33bf6cd5` |
| `native_recovery::unreserved_missing_and_denied_executables_persist_spawn_failed_without_requeue_or_execution` | SQLite retained QUEUED with null outcome rather than SPAWN_FAILED. | `spawn-failure-red.log` / `7e478dfd71649748adc78fec8aec10b3d1b0094ff50029e6b6d69891fba7f39f` |
| `native_recovery::wrapper_loss_before_attachment_recovers_the_release_bound_child_with_and_without_restart` | Expired unreserved release retained its nonce after resumed scheduled admission; added original-key restoration. | `restore-witness-red.log` / `6fda657707799a39dd378f2b992ca9cb52211706eb430745a733ee35d05f77f0` |

The restore-witness RED command also supplied `--nocapture`. Authorized native prerequisites `cargo build -p arc-daemon -p arc-cli --bins` passed (exit 0); log `native-prerequisites.log`, SHA-256 `953c55c33cdf54afbde67e84c9a4a48492bb9d7d3f5dcb9523bc48fa8318969c`. Formatting used `cargo fmt -p arc-cli`; final check-mode receipt is above. The existing ts-rs deny_unknown_fields exporter diagnostic remains; structural agreement passed and no suppression or generated rewrite was added.

## Changed paths and acceptance

Nine source/test files changed in this attempt. Owned delta digest: `4b8fd1e0c6c6731de66c637465ea62138a04273c59699c17fb1d6866c31cfe50` using the same sorted-row recipe; excludes this report to avoid circular identity.

| Path | SHA-256 |
| --- | --- |
| `crates/arc-cli/src/exec.rs` | `4a339a23e8ae60d5786c69b8c3387202147097fc156f549ebd7febe70b828640` |
| `crates/arc-cli/src/exec/launch.rs` | `ed8460d13ddf8cb080c42b4d1a39e51eca2a8f22a02b8a32b30ceef4a094866d` |
| `crates/arc-cli/src/exec/launch/checkpoint.rs` | `7a23dcd4ff0c2035248fb80c403b05da17c856a203f3dc6072f875212d2f8332` |
| `crates/arc-cli/src/exec/launch/governor.rs` | `2fce455a848d80621b62c4379a9eef9c7a919d9976914069d8da18767aaad71a` |
| `crates/arc-cli/src/exec/witness.rs` | `68b5b014fd66186f03dc278b98955b85ea1b9b0d5d49af59b351093578c8d5a2` |
| `crates/arc-cli/src/governor.rs` | `b370a64090d706e63c5794eb4850f21172eb4b560157d4446d85903cb99fe840` |
| `crates/arc-cli/tests/governor.rs` | `3ae5556d621a2ed81e0390555768667e439d4f3e26040da9f81b409218ad6a6d` |
| `crates/arc-cli/tests/governor/execution.rs` | `16bec50d456a623a86977c0de1c8b8260c642d4c6b20b177e7ccf9de6b88c8c6` |
| `crates/arc-cli/tests/governor/native_recovery.rs` | `400d8c0071eb2790bdc22b683ffdae327196cda9448a4e7386b7eabcb7c5785e` |

The tenth delivered path is this report, `docs/requirements/governor-client-repair.md`. The preceding 35-file client delivery remains in the snapshot and its report is preserved unchanged.

- [x] Headless controls and registered bridge retain the same authenticated, revisioned Rust DTOs; replay display uses authoritative readback.
- [x] Unreserved native execution retains one spawn, recoverable owner-bound witness and durable failure authority; all six loss/restart variants and both exec failures passed.
- [x] Deterministic accessible switch recovery, rejection, uncertainty, revision, navigation and reconnect tests passed.
- [x] Governor state remains independent of daemon/refresh liveness; paused historical-hold and Critical measurement checks passed.
- [x] Rust-derived structural agreement and compiled CLI/bridge/desktop focused checks passed; generated bytes match the snapshot.
- [x] Shared manifests, runtime ownership, migration bytes and previous reports are preserved. No task-7 ownership/interface conflict or remaining client repair blocker is known.

## Handoff and remaining verification

Task-8 must reconcile these actual integrated bytes with task-6’s runtime delivery before claiming synchronized production-daemon CLI/registered-bridge acceptance. The one-slot A/B sequence, complete race/uncertainty inventory, all six mission gates, eleven 80% production-line floors, desktop build/smoke, final input receipt reuse, dedicated Phase 0–5 traceability/counts and independent read-only QA retain their saved owners and requirements. **Zero coverage collections** and no mission-wide completion, savings, user-study or visual/VoiceOver claim are made here. No browser, GUI workaround, installed-daemon restart, external delivery, commit or push occurred. Only isolated fixture daemons were restarted.

This is the material, nonignored handoff. Final staging verifies all ten paths are Git-visible, staged bytes equal worktree bytes, source/checked digests still match and scoped protected inputs remain unchanged; that final transport result is returned to the Chief without rewriting the evidence file.
