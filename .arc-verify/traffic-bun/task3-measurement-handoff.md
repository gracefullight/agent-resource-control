# Task 3 measurement handoff

Status: the assigned launch-order repair and measurement checks are complete in this actor worktree. Final integrated evidence and the mission gates remain with their assigned owners. Skill used: `oma-backend`.

## Source scope

- Accepted committed baseline: `16772fe2aa350413225f3fb2bb9227d14ae5a6c4`, tree `1834e0cccb50a12fea299618f9a360f4ca10e03f`.
- Actor snapshot and inspected local HEAD: `ce573e0d1f39773c8fe34147aefcab748241d530`, tree `d90fc31a234dd9c72399e2c60a43e3465139de37`. This snapshot includes an input overlay beyond the accepted baseline. It is not the integrated review checkout.
- The native command measured this snapshot plus the uncommitted task-3 changes listed below. Its before/after `trafficInputs` SHA-256 was `68eb6ab08318307d77e0dcd6c35355a228ac332d5842c7cfbfe1ddd0915b4b25`. Do not attribute those changes or results to the accepted commit. The integrated owner must recollect after delivery against the integrated source identity.

Files changed in this attempt, with SHA-256 of the inspected working bytes:

| Path | SHA-256 |
| --- | --- |
| `crates/arc-daemon/tests/traffic_cli/bun_measurement.rs` | `847194980ae073f3cb92ac87fc8053b2b3a4bf95648f009d0da6bed6b909a0fb` |
| `scripts/coverage/traffic-bun-measurement.ts` | `0241d16b4b43e3d6cc83528b0ac11da62232deed2948336902f4f006612444da` |
| `scripts/coverage/traffic-bun-evidence.test.ts` | `f1dafd19ac7ee2b151e39ae1396498318a631a72872827cd5bc4a463716894cb` |
| `.arc-verify/traffic-bun/task3-measurement-handoff.md` | This report |

The following assigned inputs were already tracked in the actor snapshot and were not changed in this attempt: `crates/arc-daemon/tests/traffic_cli.rs` (`0ca60f99cdd5376e67b939555779c30ee12f07e257f841ecfca449a386b2c56c`), `crates/arc-daemon/tests/traffic_cli/measurement.rs` (`e4a139e85876d55be7e9d918b093427b7df9f3f3dd65901e9db9b750294a5136`), `crates/arc-daemon/tests/support/bun_measurement.rs` (`a29e3d70bca5957c051c70ebebc2317201d0c46caa4a97c92a45e09a97d6f994`), and `scripts/coverage/traffic-bun-evidence.ts` (`65f4c2779dc06aece326e8913a137f230ac68fd1eeada6edfce320f3aed447ab`). Actor-snapshot tracking does not establish review-checkout tracking; task 8 must verify the delivered bytes and stage every required source and validator path before collecting the harness inventory.

## Interface and ordering

`deriveCase` now compares the second tool's macOS `proc_pidinfo` start identity, in Unix microseconds, with the persisted release time in milliseconds. It rejects a start in or before the release millisecond, since truncation makes that interval ambiguous. It no longer requires the controller to observe release after the second body; both prompt and deliberately delayed observations are accepted when the underlying evidence validates. The fixture asserts the same boundary. It retains the non-waiting exec shim, the separate body and launch markers, and the committed SQLite state witness. No backend interface, migration, recovery test, historical Traffic validator, or shared manifest was changed.

The operator note accepts the kernel-start comparison against the timestamp in the committed release row as the ordering proof. The stored `released_at` is sampled before the transaction commits; the shim later verifies the committed row, so it is not a separate measurement of the instant the commit returned. The tests reject an early kernel start even when the later shim witness still contains a released row, and reject the precision-ambiguous interval. Both arrival orders are covered.

The retained mission validator checks the actual `git ls-files` output used by the full harness against the before, after, and current inventories, binds it to the receipt's source hash, and rejects missing inventories, changes and untracked required inputs. The regression for an untracked delivered source passed here. The final owner must stage the complete final inventory, including final evidence paths, before harness capture and must preserve that inventory through validation.

## Actor-local native observations

The real macOS command ran one Cargo test and one Bun test in separate canonical directories for each case. Both test bodies and launch markers were observed exactly once, both tools exited zero in every case, and each scheduled second job produced the queued notice.

| Arrival order | Bypass peak | Scheduled peak | Scheduled wait | Second tool kernel start minus end of release millisecond |
| --- | ---: | ---: | ---: | ---: |
| Cargo first | 2 | 1 | 97 ms | 41,166 µs |
| Bun first | 2 | 1 | 52 ms | 32,273 µs |

The controller deliberately observed the committed release after observing the second body. The source-bound receipt, raw observations and derived summary validated again after the targeted Clippy run. The summary is an actor-local result, not the final `.arc-verify/traffic-bun/mac-acceptance.json`.

Raw evidence, all relative to this actor worktree; hashes are SHA-256:

| Artifact | Hash |
| --- | --- |
| `target/traffic-bun/commands/task3-binary-871c33a0-721d-4682-ab4b-b35cb0560161/receipt.json` | `6631c8dbcb8d689d8cc495f443aaab09251b2ea0b3df5f0fa82d85333d4af32f` |
| `target/traffic-bun/commands/task3-native-be8e02c3-5459-4aa2-a597-b3f9a9b47d69/receipt.json` | `39c07fa60313fbb317597af3289f998f2757fc5932f25f18980961cabe662c18` |
| `target/traffic-bun/commands/task3-native-be8e02c3-5459-4aa2-a597-b3f9a9b47d69/stdout.log` | `9626488213faddbf865a0738169b1b9497627c9033e3153af179584efab03ae5` |
| `target/traffic-bun/commands/task3-native-be8e02c3-5459-4aa2-a597-b3f9a9b47d69/stderr.log` | `0307d468356b0924c648ab31bab9e3454bd8e8404f18901a9d6c18ce94a90260` |
| `target/traffic-bun/commands/task3-native-be8e02c3-5459-4aa2-a597-b3f9a9b47d69/observations.json` | `4a59181691105a290221551e6726f6a8d8dc4258895b5ce995ed5625982ff35a` |
| `target/traffic-bun/task3-attempt7/measurement.json` | `3de66ba871eb9a92deffdb479a87aaf3bbc320fa4c14866ff2495279a6802c95` |
| `target/traffic-bun/task3-attempt7/measurement-validate.log` | `a10c1290d7540361ea3efca6f01c95625a28ff690de87e941a1da4b81056c44c` |
| `target/traffic-bun/task3-attempt7/measurement-after-clippy.log` | `1aed03794d84766348895ab8d9e42592b32946211227ccbf96115eed076d3e3f` |

## Checks

- RED: `bun test scripts/coverage/traffic-bun-evidence.test.ts -t 'kernel start identity rejects'` exited 1 before the fix because the early start was accepted. `target/traffic-bun/task3-attempt7/kernel-red.log`: `550f7768002d4af53e75b066429892b0917195601a05e6164e428099a7c19ddd`.
- GREEN: the same command exited 0 after the fix: 1 pass, 12 filtered, 12 assertions. `target/traffic-bun/task3-attempt7/kernel-green.log`: `ee3d914efec176252b218c4822e373a635e0b4f773b66df4e82de9c95c299ce7`.
- `bun test scripts/coverage/traffic-bun-evidence.test.ts` exited 1: 12 tests passed; the live final-evidence test failed with `mandatory final evidence is not indexed`. Final measurement, gate, and QA artifacts belong to downstream owners and were not present in this actor checkout. `target/traffic-bun/task3-attempt7/evidence-full.log`: `dc96e06173901c89094d3df6496c3a38a08fd79147aa189f04d14e8944232cf8`.
- Source-bound capture of `cargo test -p arc-cli --tests --message-format=json` passed. Capture of `cargo test -p arc-daemon --test traffic_cli --message-format=json -- --nocapture` passed 60 tests, none ignored, in about 63 seconds. The receipt wrapper allowed 7,200,000 ms per command. `measurement` and `validateMeasurement` passed against those receipts and the current actor-local source.
- `cargo fmt --all -- --check` passed. `CARGO_TARGET_DIR=target/traffic-bun/commands/task3-binary-871c33a0-721d-4682-ab4b-b35cb0560161/binaries cargo clippy -p arc-daemon --test traffic_cli -- -D warnings` passed. Clippy log: `target/traffic-bun/task3-attempt7/clippy.log`, `66eae91917da0c7d782cfb4917604c1025726778862cfe03474e0abd0a6ed21c`.
- `git diff --check` passed for the three changed source files. The historical `scripts/coverage/traffic-evidence.ts` and `traffic-receipts.ts` and the three migrations matched the accepted baseline by `git diff --exit-code`.
- `evidenceBudget` passed in this actor worktree with four indexed mission evidence files, below the file-count and byte limits. Repeat the check after integration.

## Integration and remaining checks

Task 8 must validate the delivered files in the actual integrated worktree, run fresh source-bound measurements, stage the files and final evidence before capturing the harness inventory, then run the specified workspace, coverage, full harness and final evidence checks without skips. Independent `oma-qa` remains required. The task-2 owner retains `recovery.rs`; the actor-local CLI suite passed, but that does not replace integration of its restart-notice repair.

The host reported Darwin arm64, Bun 1.4.4, Cargo/Rust 1.95.0. Full Git status reported `av.yaml: Operation not permitted`; the scoped status excluding that path was usable. A `ps` query was also denied; native `proc_pidinfo` identity checks used by the test succeeded. Serena tools were not exposed, so identified files were read directly. No GUI or browser workaround was used.
