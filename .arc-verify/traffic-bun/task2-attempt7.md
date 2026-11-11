# Task 2, attempt 7: backend handoff

Status: the operator-note lineage repair and focused checks completed in this Actor worktree. Integrated measurement, independent `oma-qa`, and the final mission gates remain with their assigned owners. Skill used: `oma-backend`. The compatibility limit below must be conveyed to the documentation and QA owners.

## Source and ownership

- Accepted committed baseline: `16772fe2aa350413225f3fb2bb9227d14ae5a6c4`, tree `1834e0cccb50a12fea299618f9a360f4ca10e03f`.
- Actor-local starting and current HEAD: snapshot commit `cb20d1f2f90c8007940d02ba5a92c0b631354e51`, tree `d90fc31a234dd9c72399e2c60a43e3465139de37`, with the accepted commit as its parent. This snapshot contains the retained backend implementation and other Actors' input overlays; it is not the accepted baseline. The five source/test changes listed below are uncommitted working-tree edits relative to that snapshot. The supplied snapshot ref did not resolve in this checkout's ref namespace; do not infer the collection-time worker ref from it.
- No shared manifest, lockfile, migration, measurement-owned file, mission instruction, graph, or Actor assignment was edited. The queued Bun restart fixture already present in this snapshot waits for both the durable `QUEUED` row and the actual `QUEUED` notice before asserting that the second child has not started.
- `git status --short` again reported `av.yaml: Operation not permitted`; scoped status and diff excluding that file are usable but do not establish whole-tree cleanliness. Serena tools were not exposed, so known source paths were read directly. A `ps` probe was denied with `operation not permitted`; no workaround was attempted. Host: Darwin arm64, Bun 1.4.4, Cargo/Rust 1.95.0. Available disk at check: 76 GiB; physical RAM 16 GiB and eight CPUs. Focused checks ran serially.

## Behavior and interface

A scheduled native child now receives the granted `ARC_TRAFFIC_LINEAGE` only when the caller did not supply that environment variable. A caller-supplied value, including an empty value, remains unchanged. The existing private sidecar remains in place: a new nested CLI prefers a live ancestor's sidecar and falls back to the environment when the wrapper no longer belongs to its live ancestry. No wire format, status projection, admission policy, migration, Cargo routing, witness cleanup rule or Bun operation was changed in this repair.

The new claim-lookup test uses the current daemon and executes both the current CLI and the separately executed bounded protocol-1 old-client fixture, under a real Cargo test. For both live-wrapper and killed-wrapper cases, it checks the nested request's stored inherited claim, active root and generation, attached live native child, one active permit, root and nested witnesses, one successful parent and child test body, nested completion and eventual permit release. It additionally calls `arc_storage::jobs::traffic::lineage_snapshots` with the actual nested claim and requires a candidate carrying the live native child and root permit. The `lineage_snapshots` function is byte-identical to the accepted commit's function. The accepted daemon subsequently checks native ancestry; the current daemon supplied the live ancestry proof in the executed test. This is an emulation of the accepted lookup, not an execution of the accepted daemon binary.

The frozen client is a bounded independently executed fixture, not an accepted `arcctl` binary. Its provenance is documented in `crates/arc-cli/tests/fixtures/frozen_traffic_v1.rs`: accepted `exec/traffic.rs` SHA-256 `905bbfcaedc4a51d2b1acaa823e9c3600544e30f80fbdad397e64d604225ac2b`; `exec/witness.rs` `720f1ba33a6a53c1e95e401f8b37c6e05eefb3bad5063f0737306c9f1a090b9a`; `arc-ipc/src/traffic.rs` `ceb3f22ee284b65a834de2a2df4d9e67f3e93cdbf8ddb1deb8772ba00cfa3530`. The accepted daemon's `db_worker/traffic.rs` SHA-256 is `254fb9dd188b02cd55baa65c3f156fc1f1d756f0c4075ba347e0db51bb09f4ba`; its storage admission file is `371096a4194197d3f87d302789e91efa2b90ae83145732afda8ef848eda95f24`.

The caller-environment cases use real Bun to compare the direct and scheduled environments: absent, empty, arbitrary and valid-looking caller values. With no caller value, the only added entry equals the actual granted root context. The nested tests check that a live sidecar wins over an arbitrary or valid-looking supplied value and that, after wrapper death, the new client falls back to the supplied value when valid. The current daemon still reuses the permit using verified native ancestry in those cases. All these executions complete once.

**Compatibility limit:** preserving an unrelated caller-supplied value means it is not the fresh root context minted by the accepted daemon. The current-daemon wrapper-death tests confirm the resulting nested claim has no candidate in the accepted-style lookup, while the current daemon succeeds through ancestry-based discovery. From the accepted daemon's claim-only lookup, an old daemon cannot discover the parent from that unrelated claim. An old nested client also cannot read the sidecar. That combined old-daemon/caller-value case was not executed against an accepted old daemon and is not established by the three separate operator-note cases. The operator note accepts those separate cases for this compatibility class; this limitation is reported so documentation and QA do not claim universal old-peer reuse.

The retained tests also cover the Bun-specific operation and legacy status projection; unsupported old-peer fallback; probe/enqueue replacement and ambiguous replies with witnesses retained; queued and running restart; cancellation, lost acknowledgements, nested and forged ancestry, redaction, native streams and signals, failed spawn, blocked/broken diagnostics, remote and unresolved bypass, and independent TEST-window, CPU and memory decisions. No integrated four-case measurement is claimed here.

## Acceptance checklist

- [x] Operator-note checks cover accepted-style claim lookup, caller-value preservation, and wrapper-death fallback; the old-daemon limitation is stated above.
- [x] Focused tests cover Bun admission and bypass, shared TEST and independent resource budgets, lifecycle recovery, protocol-1 status, redaction and native command behavior.
- [x] The retained restart fixture requires both the queued state and visible notice before checking that no second child started.
- [x] Existing migration bytes match the accepted commit; edits are confined to owned paths and this handoff.
- [ ] Integrated measurement, independent review, full workspace gates, coverage and harness are downstream work and are not claimed by this Actor.

## Acceptance checks in this Actor worktree

All listed passing commands exited 0. The first check of the new environment test used an over-specific filter and ran zero tests; it was not counted. The corrected RED command executed one test and failed (exit 101) because the retained code did not set the missing lineage; the GREEN execution passed. The final semantics, formatting and Clippy runs below followed the final witness-only test assertion. The other passing groups ran after the production repair and before that final test-only assertion; the integrated owner must rerun against collected source.

| Command | Result | Raw log under `target/traffic-bun/backend-attempt7/` and SHA-256 |
| --- | --- | --- |
| `cargo test -p arc-cli --test binary` | 4 passed | `cli-binary-green.log` `3da2fe34e75995325ef55d10d3e875337c988df96d34657c80697b3c950fc167` |
| `cargo test -p arc-daemon --test traffic_cli real_bun_preserves_caller_environment_and_receives_missing_lineage -- --nocapture` before repair | expected RED, 1 failed | `environment-red.log` `6e45afc49562845c9a7dc318d55708ce4204c3ae726f1c451a6922db4eb03bcc` |
| same command after repair | 1 passed | `environment-green.log` `73ab65f909375aee81db039aa21d71618856d95dace2e0b6a8490321424b80de` |
| `cargo test -p arc-daemon --test traffic_cli semantics:: -- --nocapture` | 11 passed | `semantics-final.log` `2715cbced44512f81d9911cea23ae2c7135a8204e04bf867ade139b516ec8d58` |
| same command with `recovery::` | 6 passed | `recovery.log` `518e620483ef44e09403bacdce5c148615bce3e4692854e8c1370a60c554a379` |
| same command with `diagnostics::` | 11 passed | `diagnostics.log` `8290232f46601124000bfbf6ed61b3f183b61a9a4e4256f38c9fefd86b6bdc3f` |
| same command with `status_compat::` | 4 passed | `status_compat.log` `4244486f5cf3a6b6b92e40a6beedb0ecf849ac90dd73c90cd81a73467914f9d2` |
| same command with `protocol::` | 4 passed | `protocol.log` `ef4f14bbcec6c8523f4609493a864580be6ee4c44e734d3a2aa6cef413eb4e50` |
| same command with `robustness::` | 4 passed | `robustness.log` `10dfe59a09deb7aee925c88e0aa86c2b28d64bf13c76935bc11f8471381432d2` |
| same command with `unattached::` | 4 passed | `unattached.log` `62fc42ea89ea410aecfd5f7dab8b649ed01954caf1b99f1a542144489c523540` |
| `cargo test -p arc-daemon --bin arc-daemon db_worker::traffic:: -- --nocapture` | 7 passed | `daemon-traffic.log` `2f542ca83c496021b3095368804c8680fb2b37fcd4e4e274722804f052c72025` |
| `cargo test -p arc-daemon --bin arc-daemon lifecycle_tests::committed_bun_grant_lost_before_old_peer_replacement_recovers_original_store -- --nocapture` | 1 passed | `bun-lifecycle.log` `cc0c5b59a2ab4dacd1b84c97e0c18ab82addb3e1fa34fc29cfd82ba262c10fae` |
| `cargo test -p arc-storage jobs::traffic::tests -- --nocapture` | 38 selected tests passed | `storage.log` `9eb70dcdf87751df9ad514130bc4aef6edfe114c1f78fb690cbf33fa149cec8d` |
| `cargo test -p arc-detector -p arc-traffic -p arc-ipc` | all package tests passed | `detector-policy-ipc.log` `58f3135981a8560b2779b3e842400022212aaca75a898a4b4b2999af225f7516` |
| `cargo test -p arc-cli` | all package tests passed | `cli.log` `be2d54749b454300ec56b288b3883b7f3db439bd9cf2989905ff633b253b1ba7` |
| `cargo fmt --all -- --check` | passed | `fmt-final.log` `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `cargo clippy -p arc-cli -p arc-storage -p arc-daemon --all-targets -- -D warnings` | passed | `clippy-final.log` `5c0fcb53a7faca5901c8a18c5d3d429439de73a6cbcd9922c5217f5fbd298d81` |

The three existing migrations match the accepted commit byte for byte. SHA-256: `0001_phase0.sql` `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`; `0002_managed_exec.sql` `464a1c88270f01a56ccab6610f87eff7648e0640f276ebce53b8a97026c50814`; `0003_traffic.sql` `1f4522e3f27c124aea7127d92434c7bd22be36168fef0a4bf665f8cf16debe90`. Scoped `git diff --check` passed. Raw logs are Actor-local ignored artifacts; collection and integrated runs must bind their own source and receipts.

## Changed files and final Actor-local SHA-256

The following hashes describe the actual uncommitted files atop snapshot `cb20d1f2f90c8007940d02ba5a92c0b631354e51`; they are not hashes of files committed in the accepted baseline. The binary diff for exactly these five paths against this snapshot has SHA-256 `3fc9448ff6f9e8851e88ede16b4170e384a96cada6c5dc04ce966ca6781914dc`.

```text
483e2e25465f1d9c37b2bd19318889e026b19404433f468d0bb6582ca8acb584  crates/arc-cli/src/exec.rs
906224e1b6a03c30190008509a57277ab572915effa8cf014064ab1d4f7af02c  crates/arc-cli/src/exec/lineage.rs
028462bf53ea6ae38b050b12ff84e69227d63e0f6a8743aad2647d5df6744bb8  crates/arc-cli/tests/fixtures/frozen_traffic_v1.rs
a15d583d6cd2c8a1d8021d332fa67c9611206fc662d4e576a542a467e71993bd  crates/arc-daemon/tests/traffic_cli/semantics/environment.rs
8f96267af01fa1735bc1bfa417d5d1874411343e55f4b47fe51e8ecf4a76d122  crates/arc-daemon/tests/traffic_cli/semantics/lineage_recovery.rs
```

The prior `task2-backend-handoff.md` describes its own earlier snapshot and must not be read as the current environment contract. The final gate owner must run the mixed-tool measurements, full workspace test/Clippy gates, existing coverage gate and full harness without skips, and the independent reviewer must inspect integrated source and fresh evidence. The current task did not run those integrated gates or restore predecessor archives.
