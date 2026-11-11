# Task 2 backend handoff

Status: implementation and focused checks completed in this Actor worktree. Integrated verification and independent `oma-qa` remain with the assigned downstream Actors. Skill used: `oma-backend`.

## Source scope

- Accepted committed baseline: `16772fe2aa350413225f3fb2bb9227d14ae5a6c4`, tree `1834e0cccb50a12fea299618f9a360f4ca10e03f`.
- This Actor's initial and current checked-out HEAD is snapshot commit `a6c553d83679577923ac512018644e19bb649ad8`, tree `0746d087d0a597b35493f501c6c8f45081606f5b`. The supplied snapshot ref resolves to these values in the mission source workspace; it is not exposed in this Actor worktree's ref namespace. The snapshot contains retained implementation and measurement changes beyond the committed baseline. The changes listed below are uncommitted additions to that snapshot; they are not claimed to be in the accepted baseline or in the worker ref.
- The supplied worker ref was not present in the mission source workspace when checked before collection; it must be resolved at collection rather than assumed to contain the uncommitted files. Host: Darwin arm64, Bun 1.4.4, Cargo/Rust 1.95.0. Serena navigation tools were not exposed. Named source files were inspected directly. `av.yaml` still reports `Operation not permitted`; whole-tree cleanliness is unknown. No migration, shared manifest, lockfile, task instruction or measurement-owned file was changed.

## Change and compatibility

The daemon obtains a bounded live parent chain for the authenticated requester and looks for matching active permit members. Both enqueue and poll can reuse a permit after the anchor is independently verified. The transaction rechecks the current anchor identity and job revision, root ID, generation, active permit and native ancestry. An attached live native child remains an anchor after its wrapper dies, including after daemon restart. Sibling, stale or forged identities do not establish ancestry. A lost-proof race retries without reusing a released permit.

The request's original lineage claim remains unchanged in `inherited_context`; it continues to key its own witness and duplicate requests. The native child's environment is not changed by the new wrapper. The wire vocabulary, protocol-1 status projection and Bun capability negotiation are unchanged. The frozen old client fixture is a separately executed, bounded implementation of the accepted Cargo protocol-1 path, not an accepted old `arcctl` binary. Its source lists exact accepted-revision hashes and models a freshly generated claim, witness, dropped enqueue acknowledgement, polling, attach and completion.

The queued Bun restart fixture waits for both the durable queue row and the visible `QUEUED` notice before asserting that the second test has not started. Two status fixtures now use a separately authenticated permit holder so their queued jobs are siblings rather than descendants of the holder. The existing signal test now waits for the parent's signal handler after the child acknowledges its own delivery; it retains the original assertion and duplicate-delivery check.

## Acceptance evidence

- [x] Focused classification and policy checks cover eligible Bun, the shared TEST window, separate CPU and memory decisions, and observe/bypass cases.
- [x] Focused lifecycle, native command, protocol-1, migration, fail-open and redaction checks passed, with the bounded frozen peer identified above.
- [x] Both Cargo and Bun parents and both nested tools were checked after wrapper death, with and without restart; frozen old-client nesting, witness handling and negative ancestry/race cases were checked.
- [x] The restart fixture requires both the durable queue state and visible notice before checking that the second job has not started.
- [ ] Integrated measurement, independent review and full mission gates remain downstream.

- `semantics-final.log`: 9 focused tests passed. Real Cargo and Bun parents each ran real Cargo and Bun nested tests after killing the wrapper, with and without daemon restart. Each case checked the native child was attached and live before the kill, retained one permit, matched root ID and generation for the nested job, succeeded once, and released the permit after a status-triggered recovery. The two live-wrapper cases used the frozen old client under Cargo and Bun parents and checked its fresh claim, witness bytes, dropped enqueue reply, actual reuse and successful single execution. Other cases check direct versus managed Bun environments for absent, arbitrary and valid-looking lineage values, streams and signals, fail-open behavior, remote and unresolved bypass and redaction.
- `recovery-final.log`: 6 tests passed, including the queue notice restart check, cancellation, forged lineage, spawn failure, nested permit reuse and recovery. The queued recheck test models a temporarily unavailable durable anchor and then requires a poll to verify the actual ancestry; the real wrapper-death cases are separately exercised above.
- `status-compat-final.log`: 4 tests passed, including old-client Bun queued/running/completed status shapes and old-daemon fallback. `diagnostics.log`: 11 tests passed, including independent CPU and memory budget decisions, both replacement directions, blocked/broken diagnostics and cancellation. The status suite waited on the shared native-fixture lock used by another worktree and completed in 86.50 seconds.
- `daemon-traffic.log`: 7 tests passed, including live sibling and stale-identity rejection, attachment and release races, lost replies and witness recovery. `bun-lifecycle-fault.log`: the real daemon lost-grant/old-peer replacement recovery test passed. `storage-traffic.log`: 38 tests passed, including different-claim witness recovery, idempotency, active permit rechecks and recovery. `detector-policy-ipc.log`: all tests for those three crates passed, including option boundaries, TEST sharing and independent CPU and memory limits. `cli-final.log`: all CLI package tests passed. `cargo fmt --all -- --check` and `cargo clippy -p arc-cli -p arc-storage -p arc-daemon --all-targets -- -D warnings` passed.
- The three migration files match the accepted baseline byte for byte: SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`, `464a1c88270f01a56ccab6610f87eff7648e0640f276ebce53b8a97026c50814`, and `1f4522e3f27c124aea7127d92434c7bd22be36168fef0a4bf665f8cf16debe90`. Scoped `git diff --check` passed with `av.yaml` excluded.

Initial failures were retained: `recovery-initial.log` exposed an old fixture that expected a true descendant to queue; the revised test still exercises a queued poll and actual ancestry. `status-compat.log` exposed two holders incorrectly modeled as parents; separate peer fixtures repaired them. `cli.log` exposed the signal acknowledgment race; the exact test passed on rerun, and the full CLI package passed after the bounded synchronization fix. These failures are not counted as passing checks.

The passing commands are listed below in a reproducible order. The CLI binary test supplies the executable fixtures used by the daemon tests when starting from an empty `target` directory.

```text
cargo test -p arc-cli --test binary
cargo test -p arc-daemon --test traffic_cli semantics:: -- --nocapture
cargo test -p arc-daemon --test traffic_cli recovery:: -- --nocapture
cargo test -p arc-daemon --test traffic_cli status_compat:: -- --nocapture
cargo test -p arc-daemon --test traffic_cli diagnostics:: -- --nocapture
cargo test -p arc-daemon --bin arc-daemon db_worker::traffic:: -- --nocapture
cargo test -p arc-daemon --bin arc-daemon lifecycle_tests::committed_bun_grant_lost_before_old_peer_replacement_recovers_original_store -- --nocapture
cargo test -p arc-storage jobs::traffic::tests -- --nocapture
cargo test -p arc-detector -p arc-traffic -p arc-ipc
cargo test -p arc-cli
cargo fmt --all -- --check
cargo clippy -p arc-cli -p arc-storage -p arc-daemon --all-targets -- -D warnings
```

## Actor-local raw logs

All paths in this section are relative to this Actor worktree under `target/traffic-bun/backend-attempt5/`; hashes are SHA-256. They are ignored raw logs and are not part of the tracked delivery. The integrated worktree must rerun its checks and bind its own source and logs. Most focused logs precede the final test-only signal synchronization change; `cli-final.log`, `fmt-final.log` and `clippy-final.log` follow it.

| Log | SHA-256 |
| --- | --- |
| `semantics-final.log` | `ad72922640b9e00dbc310e54e600cb7fc012b4059c2fce62b8234eb3466bea0c` |
| `recovery-final.log` | `83edb1a1a71c4bfb87041a2f8bd2824344b347f0a755a81978487ab88b28856d` |
| `status-compat-final.log` | `d005bb4f185f8fd6bef0691380572e098cb7a7309c82cdcf3b8e87d153873a24` |
| `diagnostics.log` | `a418a04d83c3799b1e04a4b75884fdd524ba928badbf47cd695147daedd6ddeb` |
| `daemon-traffic.log` | `a4d78ef85f70e3663f4192b1fad2d18b75f6a7302cb2c81e81b7d03d0212eeef` |
| `bun-lifecycle-fault.log` | `b38b4c06b141440a15e28e35c1178ffc6253276e7a60253ecf9838dcae593a4a` |
| `storage-traffic.log` | `a782414d5966d58f334b2cf1f192987a5916c8340fd968c96a21bb06f76d092d` |
| `detector-policy-ipc.log` | `b2870c7791b5da01fbee0a09bd4d69e5c6ca3c4b897afb24942e7a30eece9cd1` |
| `cli-binary-final.log` | `0edb9a6867a8961e4674384c0dbb5c93676bc3048016a4a00c18e979269c924c` |
| `cli-final.log` | `20fe041267e4da0528070067e3a00b96e5aaae047bf4c23e6bdbce736d82eb66` |
| `fmt-final.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `clippy-final.log` | `e389a398809185d060e6c52beb1a0e5eaafaa953e96b5a6fde3c51ac0ebb3c81` |
| `recovery-initial.log` | `cb2519bc89d4b38b6d746700de961fd92a3e8330d9daccb9115bab475c924b50` |
| `status-compat.log` | `0b3399a3bbc3502c7b676e2737ef7f33096d4441d026cebe83f68ff27b7c6560` |
| `cli.log` | `a0784d09feb84801e3a70efaa528fe3c2a07353f86b787bd9f93c40504eed1fe` |

## Changed source files and SHA-256

These hashes describe the final uncommitted contents in this Actor worktree and allow the Chief to check the collected delivery. New files must be included in collection.

```text
c8e04c79ccbf84ac5147b3827b9339780bb2e1f5a6a6d96671198eaa4f818c93  crates/arc-cli/src/exec/signals/tests.rs
10dd8bc00deb32f6764f34ef5d995413b83b016fdd3c17c7503e6ce71fd79dd9  crates/arc-cli/tests/fixtures/child.rs
b35bee33f106038b4abf53ecbbfa4e29cbd9581474aeddd627aabb8549b480e2  crates/arc-cli/tests/fixtures/frozen_traffic_v1.rs
ef64a1be1dda02a238405bc3c273ed29d1d6f7d213f97fe18ff24bbef73b81ab  crates/arc-cli/tests/fixtures/hold_unattached.rs
d34c7977dc42a77522fdcf97514f3311eb12f41f082232f94d7c646fa975ae34  crates/arc-daemon/src/db_worker/traffic.rs
bc5edf88882c01eecfdbed04d2e196a101ac3add49359048d1f273481f2d817a  crates/arc-daemon/src/db_worker/traffic_tests.rs
713498e773b3f90e03152cbc658dc62376344de78556b1a81d920a351c8c5929  crates/arc-daemon/tests/traffic_cli/recovery.rs
58a80a1fe88d97d0acebfbe75aca8b17d027e6dc3995b32301ca3865334b1cb2  crates/arc-daemon/tests/traffic_cli/semantics.rs
21d17f3ead9010127feb058f478dfc68bed371d3213a8ac8b93a81c5601488c0  crates/arc-daemon/tests/traffic_cli/semantics/lineage_recovery.rs
628038507640ad54916665e9a5d91f846d981a39417c021e67285801cd985a37  crates/arc-daemon/tests/traffic_cli/status_compat.rs
d91e1f9322254486e2ca8a514940c162c497b28ec79e79e8e4537d4817ecac58  crates/arc-storage/src/jobs/traffic.rs
bea756928cc519c84ed2dbff74f75318cb25a642ac96e0b53306383b9f1b1dc2  crates/arc-storage/src/jobs/traffic/admission.rs
22acb3b0d8383b283d536196bec93889d2bbcc4729c1ad4b4edfee0193f65141  crates/arc-storage/src/jobs/traffic/tests/lineage.rs
abb0f4214fe4fbdc5fda2d34c3dd0f76f87aecef499df36e51bd26456944303a  crates/arc-storage/src/jobs/traffic/tests/witness.rs
```

## Remaining work

Task 5 must run the unfiltered integrated suite and mixed Cargo/Bun measurement against the collected source. The independent reviewer must review and reproduce checks; the final owner must run the full workspace formatting, Clippy, tests, existing coverage gate and harness with the required per-crate floor. No mixed-tool measurement receipt, independent review, coverage collection or harness result is claimed here. The accepted-contract archive prerequisite and the unreadable `av.yaml` still require the responsible owner to assess them in its own current environment.

The Actor-local `.arc-verify/traffic-bun` inventory has two files and is below both the 30-file and 5,000,000-byte limits; integration must check the final inventory again.
