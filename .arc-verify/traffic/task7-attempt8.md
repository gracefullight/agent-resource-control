# Traffic task 7 — attempt 8 handoff

Status: **completed for the assigned repair**. The independent QA review and task 9 mission receipts remain outstanding. Used **oma-backend**. HEAD remains `4d2f71c1a1482925f6c9918296d7ceb3435b7592`.

## Repair

- Before enqueue, the CLI writes and syncs a private witness containing the job ID, owner PID and start token, and the opaque key. The pre-exec child adds its PID and start token and syncs the record. A held file lock prevents a recovery read during an incomplete fork-to-exec update. Witness creation or update failure prevents the target from executing and reports `SPAWN_FAILED`; actual daemon outages still use the existing bounded fail-open path.
- Cancellation and native spawn failures write their no-spawn or spawn-failure outcome before attempting the corresponding daemon request. Recovery can therefore release an unacknowledged grant without replay and can restore the actual spawn-failure outcome. The reader retains support for the earlier witness formats.
- Startup and each admission or status reconciliation scan active reservations. A verified dead owner and no child, or a verified exited child, releases the applicable grant; a verified live child retains it. Unknown child liveness records a first observation in the existing SQLite audit table, reports an unknown outcome, and retains the reservation until the configurable `ARC_TRAFFIC_RECOVERY_GRACE_MS` expires. The default is 120,000 ms. Expiration and permit release are recorded transactionally, including a `TRAFFIC_GRACE_EXPIRED` audit event. No migration was added or altered.
- CLI status distinguishes a confirmed unsupported legacy Traffic query from an unavailable or expired query. The former uses protocol-1 lifecycle status; the latter reports `unknown`, including in recent listings. A legacy lifecycle record with no Traffic entry continues to use its existing status. Original protocol-1 payloads and original Traffic status replies remain unchanged.
- The native fixture guard stops verified Cargo descendants and their build-script children on drop, including when the wrapper was killed. A test observes the blocked build script and verifies that both it and Cargo stop.

## Paths changed in this attempt

- CLI: `crates/arc-cli/src/exec.rs`, `exec/preparation.rs`, `exec/traffic.rs`, `exec/transport.rs`, `exec/witness.rs`, `src/job.rs`.
- Daemon: `crates/arc-daemon/src/db_worker/traffic.rs`, `traffic_recovery.rs`, `traffic_witness.rs`, `traffic_witness_repair_tests.rs`; `crates/arc-daemon/tests/traffic_cli.rs` and `traffic_cli/{cleanup,status_compat,unattached}.rs`.
- Storage: `crates/arc-storage/src/jobs/traffic.rs`, `traffic/{lifecycle,witness,uncertain}.rs`, `traffic/tests.rs`, `traffic/tests/{recovery,uncertain}.rs`.
- Documentation and handoff: `docs/managed-exec.md` and this file.

All paths above are relative to the mission worktree; directory prefixes in the grouped entries continue from the preceding full prefix. Existing changes from earlier Actors were preserved. No shared manifest, lockfile, mission state, assignment, or migration was edited in this attempt.

## Acceptance evidence

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed. |
| `cargo test --workspace` | 516 passed, 0 failed, 0 ignored. |
| `cargo test -p arc-daemon --test traffic_cli -- --nocapture` | 31 passed. The native macOS run used independent projects and target directories. Both bypassed jobs ran once with peak 2; both scheduled jobs ran once with peak 1. The second reported queued before the first release, started after release, waited 110 ms, and both outcomes were `EXITED:0`. Remote and UNKNOWN commands bypassed while the queue was full. |
| Witness and daemon-fault tests | CLI witness unit tests: 4 passed. Two real-CLI tests killed the owner after a grant before spawn or dropped a spawn-failure completion; each reopened the database, recovered one permit once, and admitted the next job without a target replay. Earlier witness formats and a forged job ID were checked. |
| Recovery and status tests | Dead owner plus exited child, unknown child across the configured grace, witness creation failure, delayed recent and by-ID queries, legacy status fallback, and fixture cleanup passed in the current workspace and Traffic CLI suites. |
| `bun run test:coverage` | Passed all seven production-line floors. |
| `./scripts/harness/validate.sh` | Passed all five checks, zero violations, coverage enabled and no skips. The separately copied intent-to-add index under `target/traffic-task7-attempt8/harness.index` included the previously untracked Traffic files without modifying the shared index. |
| `bun test scripts/coverage/evidence.test.ts` | Passed after the last coverage collection; current source hash `c83e627ffdecb5c2c15ce143321f877b6022bb8eafeacfe98f73cd56366c660d`. |
| `bun test scripts/coverage/traffic-accounting.test.ts` | 6 passed, including seven-crate and below-floor rejection checks. |
| `bun test scripts/harness/traffic-targets.test.ts` | 8 passed. The saved mission has no configured observation policy; the live result was not applicable and does not claim container or service health. |
| `git diff --check -- crates scripts docs` with the separate index | Passed. |

The final harness coverage was arc-core 96.48%, arc-detector 99.84%, arc-storage 95.68%, arc-ipc 98.92%, arc-daemon 91.18%, arc-cli 83.41%, and arc-traffic 100.00%. Migrations 0001 and 0002 match the accepted HEAD byte for byte: SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384` and `464a1c88270f01a56ccab6610f87eff7648e0640f276ebce53b8a97026c50814`.

## Raw log hashes

All paths below are under `target/traffic-task7-attempt8/`; hashes are SHA-256.

| Log | SHA-256 |
| --- | --- |
| `fmt-verified.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `clippy-verified.log` | `cc606f8a3fdb3dc65aa1a65314fb8628cd3d8c5bdf30dfd5b460efd2b1664bc5` |
| `workspace-verified.log` | `774cdde9efd570bdd50c7f9fd652c8e848284eb0bccf468b3b2c7022f999fdec` |
| `traffic-cli-verified.log` | `a253b384da97332a8bc70ec99ebd366586b11f15005c143d098c7a7b60d0bc77` |
| `mac-measurement.log` | `f1ca9bfa929097b488a2a92a5d2eb4a678c6cd6b5557b3859e31ef111df388b9` |
| `cli-witness-final.log` | `ce0be3714cc4f0698ca3d6260a98ec676293c8514cbc715f95004280a0c45bf8` |
| `witness-recovery-final.log` | `d782c05619e4731a395d2fd11ea3e5244dc8cd0fe5e581f616c8dbe188a710ed` |
| `fixture-cleanup.log` | `bffa26b45c2137ec85c253a5cd4bb5f8e0dbc465de1dee3ef789d53fc70d9859` |
| `coverage.log` | `f7c339ef60592218d1936729c5241f946863062202b8cf18639122216599ad55` |
| `harness.log` | `add53dc94394462ef7bd9bd9cb27cc27b1e15846fb23e5d48925e13aaf3f97d9` |
| `coverage-evidence.log` | `1ec6ead3a49e1175a99605d41aeaea6d831786694b53648ba8be2e6f8fe312e1` |
| `traffic-accounting.log` | `48188dba586a2636026eb0c1d0f88b75201cfa347ee82695ef6a08c27c2c28b0` |
| `traffic-targets.log` | `c67100fcb5959116dca2f202e8bbe053cdea8a0dc5976aa5d3b3e595955f61a5` |

## Failed attempts and limits

The first workspace run failed an existing legacy recent-status assertion because a Traffic lookup tried to reconcile a legacy record with a deliberately future timestamp. The daemon now checks for a Traffic row before that reconciliation; the focused test and final workspace suite passed. The failed log is `workspace.log`, SHA-256 `c2b301c34cfa2463472c37d68fca25fd191af642e1a9571f78a8a03a96940a17`.

An initial focused test command used `--lib` for the binary-only daemon and exited 101; the corrected `--bin arc-daemon` tests passed. While adding the delayed-status fixture, one edit had a misplaced variable, the first socket fixture inherited nonblocking mode, and a subsequent generic argument was incorrect. Those attempts failed and were repaired; the final full Traffic suite passed. Their logs are retained in the same target directory.

Git still reports `av.yaml: Operation not permitted`, so its contents and a complete unscoped status are unverified. The configured Serena tools were not exposed; inspection used known assignment paths. The saved task does not own task 9's `mac-acceptance.json` or `gate-summary.json`, or the independent QA artifacts. Desktop, native Windows, vendor hooks, broader detection and remaining scheduling policy remain outstanding. No Playwright or browser tools were installed or used.

The host was Darwin arm64 with approximately 57 GiB free during verification. Checks had no wrapper timeout shorter than the mission's 7,200,000 ms allowance. Before this report, the existing mission evidence set contained 21 files totaling 226,205 bytes; raw logs remain under `target`.
