# Traffic task 7 — attempt 6 handoff

Status: **partial**. The implementation and task checks below pass. One Cargo child from an earlier failed test may still be running; the host denied `ps`, and its PID was not retained, so its state cannot be verified or safely terminated. Task 9 and independent QA are also pending for the overall mission. Used `oma-backend`. HEAD remains `4d2f71c1a1482925f6c9918296d7ceb3435b7592`; preceding changes were preserved. This attempt did not edit the shared manifests, lockfile, saved assignment or mission state.

## Implementation and interfaces

- The CLI uses an owner-only keyed witness. Before target execution, the child attempts to record its PID and native start token. If a daemon outage loses attachment and completion, recovery checks the key and native identities and applies the result in the SQLite writer transaction. A surviving child keeps its reservation; a verified exited child is recorded outcome-unknown and releases it without replay. A witness write failure does not stop native execution.
- If the grant commits but its reply and a subsequent pre-spawn cancellation acknowledgement are lost, the CLI records a keyed no-spawn result. Recovery checks the original job, key, revision, unattached state and dead owner before atomically cancelling and releasing the permit. A real-CLI test drops the committed grant reply, cancels before spawn, reopens SQLite and exercises daemon startup recovery; it observes zero target invocations, one permit and zero active permits afterward. Duplicate recovery is a no-op.
- A fresh random lineage claim is used when no parent context exists. It is not proof of ancestry; existing OS ancestry checks still decide permit reuse. Protocol-1 lifecycle operations and the original Traffic `STATUS` reply shape are preserved. The additive `STATUS_V2` request opts into `UNCONFIRMED`; the new CLI falls back to protocol-1 lifecycle status if an older daemon rejects it. Tests cover both old-client/new-daemon and new-client/old-daemon status behavior.
- By-ID status reconciles dead queued owners. A grant without confirmed child evidence is shown as outcome unknown in the extended query. Cancellation and spawn-failure diagnostics use bounded best-effort output, including for full or broken stderr.
- The 19 storage recovery tests were split into three files under 500 lines, with candidate assertions updated for the new witness scan. The upgraded-database migration test verifies both original permit uniqueness constraints and rejects duplicate permit IDs and duplicate job IDs. The preservation validator has narrowly scoped schema-3 and status adaptations. The fault fixture already used a positive two-second timeout; it now uses a bounded nonblocking fallback for Darwin `EINVAL`.
- `docs/managed-exec.md` describes the fixed per-class windows and budgets, queue messages, bounded exchanges, recovery and compatibility behavior.

Files changed in this attempt:

- `crates/arc-cli/src/exec.rs`, `crates/arc-cli/src/exec/preparation.rs`, `crates/arc-cli/src/exec/traffic.rs`, `crates/arc-cli/src/exec/witness.rs`, `crates/arc-cli/src/job.rs`.
- `crates/arc-daemon/src/db_worker/traffic.rs`, `crates/arc-daemon/src/db_worker/traffic_recovery.rs`, `crates/arc-daemon/src/db_worker/traffic_witness.rs`, `crates/arc-daemon/src/db_worker/traffic_fault_tests.rs`, `crates/arc-daemon/tests/db_worker_unit.rs`, `crates/arc-daemon/tests/managed_exec/fault.rs`, `crates/arc-daemon/tests/traffic_cli.rs`, and `crates/arc-daemon/tests/traffic_cli/{diagnostics,status,status_compat,unattached}.rs`.
- `crates/arc-ipc/src/traffic.rs`.
- `crates/arc-storage/src/jobs/traffic.rs`, `crates/arc-storage/src/jobs/traffic/lifecycle.rs`, `crates/arc-storage/src/jobs/traffic/witness.rs`, `crates/arc-storage/src/jobs/traffic/tests.rs`, `crates/arc-storage/src/jobs/traffic/tests/{recovery,recovery_2,recovery_3,witness}.rs`, `crates/arc-storage/src/migration/tests/acceptance.rs`.
- `scripts/coverage/current-preservation.ts`, `scripts/coverage/schema-adaptations.ts`, `scripts/coverage/traffic-accounting.test.ts`, `scripts/harness/exec-fail-open-preservation.ts`, `docs/managed-exec.md`, and this report.

## Acceptance evidence

- `cargo fmt --all -- --check`: passed. `cargo clippy --workspace --all-targets -- -D warnings`: passed. `cargo test --workspace`: 503 passed, 0 failed, 0 ignored.
- `cargo test -p arc-daemon --test traffic_cli -- --nocapture --test-threads=1`: 27 passed. On macOS, two independent Cargo projects and target directories measured peak concurrent builds of 2 bypassed and 1 scheduled, with launch counts `[1, 1]` in both arms. The second queued notice preceded first release; its start followed release. The recorded wait was 373 ms and both outcomes were `EXITED:0`. Remote and UNKNOWN fixtures bypassed while the queue was full without changing reservations.
- The real-CLI fault tests drop replies before and after enqueue/grant and completion commits using a controlled socket server, the production SQLite worker and the authenticated peer PID. Each of those four cases executes once with one permit and one completion. Separate macOS daemon-loss tests block attachment and verify that a surviving child retains its permit and an exited child releases it after restart, without replay. A forged witness key does not release it. Queued cancellation and spawn failure finish with full and broken stderr. Nested lineage, forged and stale contexts, I/O, signals, exit codes, outage fail-open and active-child survival are covered by the workspace suite.
- `cargo test -p arc-daemon lost_grant_ack_then_cancel_recovers_unspawned_reservation_after_reopen -- --nocapture --test-threads=1`: passed in all three targets that include the worker test.
- `bun run test:coverage`: passed. `./scripts/harness/validate.sh`: passed all five checks, with coverage enabled, no skips and 0 violations. The earlier harness also passed before the no-spawn repair; this final rerun covers the repaired source. Its coverage: arc-core 96.48%, arc-detector 99.84%, arc-storage 95.41%, arc-ipc 98.92%, arc-daemon 92.75%, arc-cli 82.47%, arc-traffic 100.00%.
- `bun test scripts/coverage/evidence.test.ts`: passed after the final harness. Live source hash: `8e59fb0c3763ce1aeadf5934ce43cb1f549d48e9cbc39f656feff560a75bd721`. The harness used an isolated intent-to-add index under `target/traffic-task7-attempt6/harness.index` to include 53 previously untracked code paths and 13 Traffic evidence files in its scan; the shared index was not changed. `SKIP_COVERAGE` was unset.
- `bun test scripts/coverage/traffic-accounting.test.ts`: 6 passed. `checkFailOpenPreservation(process.cwd())` and scoped `git diff --check -- crates scripts docs` passed on the integrated source using the isolated index.
- `bun test scripts/harness/traffic-targets.test.ts`: 8 passed; the saved mission has no container or service observation policy, so the live check reported not applicable. No container or service health observation is claimed.
- Migrations `0001_phase0.sql` and `0002_managed_exec.sql` match accepted HEAD byte for byte. Their SHA-256 values are `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384` and `464a1c88270f01a56ccab6610f87eff7648e0640f276ebce53b8a97026c50814` respectively.

## Logs

Raw logs are under `target/traffic-task7-attempt6/`; hashes are SHA-256.

| File | SHA-256 |
| --- | --- |
| `fmt-unspawned.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `clippy-unspawned.log` | `0915f301b945c0367d0110cc93f2f2b861423d6c46dd1a3bdbd927e5b38a84b4` |
| `workspace-unspawned.log` | `1956495652d9a051fa2f25132243039fcf9d318f7b043063e19c4580b55bae93` |
| `traffic-unspawned.log` | `4c1a4d00bb739774547e027d98ad4dfdb0c2d1cef807664fe706887a8b2d8a03` |
| `unspawned-ack-startup.log` | `0a9d7e07913e27ef2296d1492c1e07b4e7433f5a7cf3281c548dffe6ab2ab68c` |
| `coverage-unspawned.log` | `4a0ff1b872301bfda4a4ff112a4478e704cb36b1dddceb5891f543524e30ae1e` |
| `harness-final.log` | `f28c021e676d4af8d9585fa5a2e60afb923cb2c75b8cd83fd817e310acb76d09` |
| `coverage-evidence-final.log` | `a28a5d46da239f43256ced8c37f367263655dc9e350acc5e2e1ea778e5e3f21a` |
| `targets.log` | `c738485fe527e85fc6316d796d3ddcd22bb18f532fdfba814ca5e96d495e09f3` |
| `preservation-unspawned.log` | `7c6c39a6d807a82fe0a4f630f0ab5a1cc736a509a82088073afefab48f7cd2f4` |
| `diff-final.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `coverage-adaptation-test.log` | `d66ba4814abe00293c4d6f23f69f4414eb4fcec2f8587ff9794dac2c3904406c` |
| `traffic-initial.log` | `ca9cb103cb5a9f64b513aec91bd26927afae2d74102e9980b4c3a71cc9a84853` |
| `traffic.log` | `8ade79a59ced3039a8e2b071afb28f837568ad14f8d9787c04ae7bbc13db29d0` |
| `traffic-ack.log` | `70f2f97944ca90b45ba14f615cd8cc3b5e144d82b78ebb3f566abe0e89da6863` |
| `traffic-ack-final.log` | `aff3440ae097b07cabbc3f3930e5139a70568eaa3d24766a0e845f613d51fbf1` |
| `storage-cli.log` | `38a2921d62cce0b548139475ef0f8a85388cc815f5e87ae1438f92319c2ea0a6` |
| `storage-cli-retry.log` | `f473e3303e03affdb8aa9c291599fd5c222c864996dc4752d675cc3dd48b5506` |
| `current-preservation-final.log` | `579b4c7afdb123c91159d0dac21b093dd2d77cc01d3800947fe5dfbc68b3fd1f` |
| `current-preservation-retry.log` | `0c6f436f0c62fe60941055f6e84b7f906675c18ac8c1dada3f62df60d7a07f05` |
| `fail-open-preservation.log` | `7b58ac714830ee03cc44c0c346a593375446f114bc8da9b7ab773f193ecc9495` |
| `clippy.log` | `33a5a56d6133a0f808dc7ad25c81d839f8d23eca3c09e1dfaf02dd5df370177f` |
| `clippy-retry.log` | `22fc23f34a76e49a8f43b68e67e5324acd9492078ce09a2aafb9337c2d967986` |
| `harness.log` | `f11d5fc3062ddf6aca3c240b0a3413b9da6052477a9c095a2bce422b1398844a` |

## Failed attempts and remaining work

- The first split script stopped on an incorrect group-count assertion before writing any files; the corrected split preserved all 19 tests. The first full Traffic run used a stale CLI binary and failed the full-stderr cases; `cargo test -p arc-cli --tests` produced the current test binary and the rerun passed.
- The first unattached-child tests failed because the old runtime report already existed; the fixture now preserves that report. In that failed run the wrapper had already been killed. Its Cargo child may still be running, but the failure log did not retain its PID and `ps` returned `Operation not permitted`. Its state cannot be verified and no name-based kill was attempted. The corrected fixture records its child identity and releases it on failure.
- Early acknowledgement tests failed first to compile because a test target lacked the platform module, then on an accepted nonblocking socket; those fixtures were repaired. Early storage assertions expected unattached grants to be excluded from recovery; they now assert inclusion and a retained reservation. Early preservation checks exposed a template escape issue, new test-module bindings and a historical status label; exact adaptations and negative checks were added. Early Clippy runs found collapsible match arms, a large enum and a duplicate test module. The final checks above pass; their failure logs are retained in the table.
- A hard crash before any usable witness is written, or a witness write failure during an outage, leaves an unconfirmed grant reserved because the daemon cannot safely prove termination. Observed pre-spawn cancellation with a persisted no-spawn witness now recovers. No outcome or release is inferred from missing evidence.
- `git status` still reports `av.yaml: Operation not permitted`; its contents and the complete worktree status are unverified. Serena was not exposed in the tool inventory, so the repair used the assigned known paths. The isolated-index scan and scoped checks passed without changing that file.
- Task 9 owns `mac-acceptance.json`, `gate-summary.json` and the unfiltered Traffic evidence run. Independent `oma-qa` remains for its assigned reviewer; this handoff does not claim mission-wide acceptance. Desktop, native Windows, vendor hooks, broader detection and remaining scheduling policy remain outstanding.

The host reported Darwin arm64, 8 logical CPUs, 16 GiB RAM and about 70 GiB free at the start. The verification processes were not wrapped in a timeout shorter than the mission's 7,200,000 ms per-check allowance.

Evidence budget: 21 Traffic evidence and handoff files total 226205 bytes, including 13 files under `.arc-verify/traffic`. Raw logs remain under `target`.
