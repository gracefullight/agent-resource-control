# Task 3 measurement repair

Status: the task-3 repair and actor-local checks are complete. Final integrated measurement, gate collection and independent QA remain with their assigned owners. Skill used: `oma-backend`.

## Source and interface

- Accepted committed baseline: `16772fe2aa350413225f3fb2bb9227d14ae5a6c4`, tree `1834e0cccb50a12fea299618f9a360f4ca10e03f`.
- Inspected actor snapshot and local HEAD: `d3d4cd57d5aae1ecfad8731ec5b462205b108f08`, tree `6ef5cc2082861cce73c02b5496748162ab5ff6ad`. The snapshot includes source overlays absent from the accepted baseline; it is not the integrated checkout. The supplied snapshot and worker refs did not resolve in this actor checkout. The checks below ran on that local HEAD plus this attempt's working changes.
- The mission-specific measured source SHA-256 was `a8964837d4c4e59e4049392f1ce85e183f7201c8cfdd937fb88aa4a482efa2fc`. The restoration source at this snapshot was `.arc-verify/traffic-bun/restore-predecessor.py`, SHA-256 `7bf1aec7a4c87ef52d7f85d7b10960585c9bf63bdee4263f5754740cf2af29b4`; its local snapshot index entry does not prove it is tracked in the integrated checkout.
- Changed source bytes: `scripts/coverage/traffic-bun-evidence.ts` SHA-256 `31a6ce2f1f916658d1b46bb0cd0b8b6d80a018d88c9d9f522cc9c0ac9cc63b07`; `scripts/coverage/traffic-bun-evidence.test.ts` SHA-256 `325b7fdef32b5f90aff60e17d0db75b340aaadbcd7ee54710a77c7b07e51a55b`. This report is the only new evidence file in this attempt.

`missionInputs` extends the existing `trafficInputs` with the required restoration script and other delivered executable sources under the mission evidence directory. The mission source context and default harness inventory both use that identity. The inventory checks the current source hash and requires every identified input to appear in the captured and current `git ls-files` output; optional extra required paths cannot replace that set. A changed and restaged restoration script therefore invalidates an earlier receipt even when its path inventory is unchanged. The historical Traffic validator and its source-identity helper are unchanged.

The kernel-start repair was already present in this actor snapshot. `deriveCase` compares the second native tool's `proc_pidinfo` Unix-microsecond start identity with the release row's Unix-millisecond timestamp, rejecting an earlier or same-millisecond start. Its fixture retains the non-waiting exec shim and validates the negative early-start and delayed-observer cases in both orders. The release timestamp is sampled before commit; the operator explicitly accepts the timestamp in the committed release row as the ordering proof. This does not independently measure the instant the commit returned.

## Actor-local measurement

Fresh source-bound captures ran the real Cargo and Bun test bodies in independent canonical directories on macOS arm64. Both tools executed once and exited zero in each case. The daemon test command passed 62 tests with none ignored or filtered; the measurement validator recomputed all four cases from the raw observations.

| Arrival order | Bypass peak | Scheduled peak | Visible queue | Wait | Kernel start after end of release millisecond |
| --- | ---: | ---: | --- | ---: | ---: |
| Cargo first | 2 | 1 | yes | 96 ms | 13,886 µs |
| Bun first | 2 | 1 | yes | 53 ms | 51,322 µs |

In both scheduled cases the controller deliberately observed release after it observed the second body; this did not reject the ordered run. These results apply to the actor source identity above, not the accepted baseline or a later integrated checkout.

Raw artifacts are under `target/traffic-bun` in this actor workspace; hashes are SHA-256:

| Artifact | SHA-256 |
| --- | --- |
| `commands/task3-a9-binary-cb822cfa-be45-4cc0-8701-00f5ca2255bd/receipt.json` | `091299ef8c8f4b0b095a2948f8e0b40b4b31ba585e93d97d5d1a028f766aa4ac` |
| `commands/task3-a9-native-1973439a-4278-447a-a291-69bc5b67afe7/receipt.json` | `13aa453b5166ef3b0cc6e7649ca3659eaab30932c30fc88c54b0b94d1ef36f06` |
| `commands/task3-a9-native-1973439a-4278-447a-a291-69bc5b67afe7/stdout.log` | `e9344d89788b214f768aec03854862fcdd8b8eb99155c5610ab4b6af24be1888` |
| `commands/task3-a9-native-1973439a-4278-447a-a291-69bc5b67afe7/stderr.log` | `e1402644947ac7cc7db64b55c13b7aba1faa3f5be79bef6eba677c052a0cf071` |
| `commands/task3-a9-native-1973439a-4278-447a-a291-69bc5b67afe7/observations.json` | `bc44c997cd8bc84b79e88e86742e1b9c3d2a6906728de5b49031fe6f07ff20b0` |
| `task3-attempt9/measurement.json` | `4c6cc89137fb84f41351af26461df2afc547bbe5f27e8f1a887e20b7f2a2ec93` |
| `task3-attempt9/measurement-validation.log` | `8b33dde5a7f9d0330e61bdfa2d548680ad704a227869800e79996d8de8b9d950` |
| `task3-attempt9/native-results.log` | `d7cb4260cd2cfaf062c4f47f845a0a3418a35449777c34a173a296cea264568a` |

## Checks and acceptance

- **Passed:** `bun test scripts/coverage/traffic-bun-evidence.test.ts -t 'kernel start identity rejects'`: one test, 12 assertions, both orders. `task3-attempt9/kernel.log`: `a02b76041203309012dd79bcf0138ac359d7d8b35cac2f72fa270507eb02d5a1`.
- **Passed:** `bun test scripts/coverage/traffic-bun-evidence.test.ts -t 'inventory'`: two tests, 40 assertions. The production-default test rejects an untracked script, a changed and restaged script, a missing script and an untracked additional helper. `task3-attempt9/inventory-green-2.log`: `a8686af48547737805669e992d9c1661fbf4fa187cd33a54f2f8511a3c4ca07c`.
- **RED confirmed:** the production-default inventory regression failed before the fix because the untracked restoration script was accepted. `task3-attempt9/inventory-red.log`: `72253792f9ad8c3c35072c0743554fbd368d8d677ad2e2beedcb5a39bf267d51`. The first post-change run also failed because the correct rejection had a generic diagnostic rather than the test's expected source diagnostic; the message was made specific and the next run passed. `task3-attempt9/inventory-green.log`: `e710986309dfa79d2972914c150a47c7b0cef19145100fe2d9c0f38d0e5fd18a`.
- **Passed:** source-bound `cargo test -p arc-cli --tests --message-format=json`, followed by `cargo test -p arc-daemon --test traffic_cli --message-format=json -- --nocapture`, through the maintained mission capture command. `measurement` and `validateMeasurement` accepted the resulting source-bound receipts and observations. The capture allowance was 7,200,000 ms per command.
- **Passed:** `cargo fmt --all -- --check`; scoped `git diff --check`; and comparison of `scripts/coverage/traffic-evidence.ts`, `scripts/coverage/traffic-receipts.ts` and `crates/arc-storage/migrations` with the accepted baseline. `task3-attempt9/fmt.log`: `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.
- **Not a final gate pass:** `bun test scripts/coverage/traffic-bun-evidence.test.ts` ran all 14 tests: 13 passed, and the live final-evidence test failed because mandatory final evidence is not indexed in this actor workspace. `task3-attempt9/evidence-full.log`: `a5df3aa272fd5631308f625ef70e60f6834f25e39b3a525ceede7fabc92dabff`.

The actor-local evidence budget before adding this report was six indexed files and 114,369 bytes; the final owner must recheck it after integration. Full workspace tests, warning-denying workspace Clippy, coverage, full harness and independent QA were not run as final gates here. Full Git status reported `av.yaml: Operation not permitted`; scoped status worked. Serena tools were not exposed, so identified files were read directly. No GUI or browser tools were used.

## Integration handoff

Task 8 must verify delivered bytes in the actual integrated checkout, stage the restoration script and all source/validator paths there, and stage the final evidence paths before capturing the harness inventory. Recollect measurements, gate receipts and QA against `sourceContext`/`missionInputs` after integration: earlier receipts using the old mission hash are stale. Complete the required serialized coverage and harness runs, final live validator and independent review without skips. This actor did not modify backend-owned files, the historical Traffic validator, shared manifests, lockfiles or `recovery.rs`.
