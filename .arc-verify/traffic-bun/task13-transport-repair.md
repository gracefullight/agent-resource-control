# Task 13: historical transport deadline fixture

Status: completed. Skill: `oma-backend`. The changes are test-only and remain within the assigned transport directory and this handoff. No production transport behavior, migration, manifest, lockfile, or pinned original fixture was changed.

## Failure investigation

The retained failed coverage stdout is `target/traffic/native-coverage/79e41da6-6b62-4483-9176-eaae8e47248a/stdout.log`; its SHA-256 is `50d35a7e4ef42f690bd5352402b9f7c13c74c40c1c7fc3566d5e3a48e82409d3`. It records `Err(Unavailable)` where `Err(Uncertain)` was expected and a peer accept deadline expiry. This establishes that the peer did not accept a connection in that run. It does not measure the duration of first-use verification or prove that verification was the sole cause.

The old test created its 60 ms deadline before `call_until`, which first calls the `OnceLock`-guarded historical integrity verifier. That verifier invokes Git to hash two pinned sources. A slow first-use verification or scheduling delay can therefore consume the deadline before connection; this is a code-supported explanation, not an observed timing from the failed receipt.

## Repair and acceptance

- [x] `Peer` verifies the pinned sources and completes injected setup before a test starts its exchange deadline. It waits for an explicit peer-thread readiness signal; the accept loop uses bounded `poll`, and the peer retains a 60-second overall/read/write bound and hard drop deadline.
- [x] The deadline test uses a 500 ms exchange deadline, below the historical 600 ms per-attempt limit. The peer signals that it read the request, writes an incomplete response and holds the connection. The test requires `Uncertain`, verifies the peer receipt and that the common deadline has expired, then requires `Unavailable` when reusing that deadline.
- [x] A separate controlled-startup test blocks setup on a channel for at least 750 ms, longer than both the former 60 ms deadline and the new exchange deadline, then releases it and verifies a successful request. The deadline is created after setup and peer readiness. The deliberate delay is a lower-bound demonstration; peer and coordination safety limits are 30–60 seconds.
- [x] The accepted baseline original `crates/arc-cli/src/exec/transport/tests.rs` and the retained `crates/arc-cli/tests/fixtures/historical_transport_tests.rs` both have SHA-256 `83757a11e06321cd40be4f877a7c6ba8fba0543612317db4037cff97a73940f3` (Git blob `41902733a5c1b741ac2097dcccb1ad7fd439e9ca`). Historical transport blob verification remains enabled.
- [x] The integrity verifier checks every other original test body byte-for-byte. For the deadline body only, it requires the exact added receipt channel and send, the increased hold and exchange deadlines, the explicit expiry assertion and the explanatory comment, then reverses those exact adaptations and compares all remaining bytes with the pinned original. Negative tests prove that changes to another body and to the deadline outcome are rejected.
- [x] Focused deadline, bounded missing-peer, controlled slow-setup, integrity, and the complete historical group passed. The same instrumented workspace test invocation that failed previously passed once, including the deadline and slow-setup cases.

## Source identity and retained checks

HEAD `16772fe2aa350413225f3fb2bb9227d14ae5a6c4` (tree `1834e0cccb50a12fea299618f9a360f4ca10e03f`). `missionInputs` hashes 319 inputs to `cac648d93c9222589275832eaa9138d8f4fbe7241842cc65d570de8bc754d3ec`; every receipt records the same identity before and after its command. The bounded fixture SHA-256 is `bdfcbd7bda8d358944ed5ff81cfead72204a880c6838757e3f895f8486dbd498`, and the integrity module SHA-256 is `c204b32a9a4018d70aee8b0c5e5f909140534ca622d6e266fe16d1e7bd1668e8`.

Host: Darwin arm64; Cargo/Rust 1.95.0, cargo-llvm-cov 0.6.21. Commands ran sequentially with `CARGO_BUILD_JOBS=4` and a 7,200-second per-command allowance. The instrumented test invocation uses the same arguments as the failed coverage collection step; `--no-report` runs tests only and does not execute the standalone coverage gate.

Raw receipts and stdout/stderr are in `target/traffic-bun/task13-transport/` in this Actor workspace. Each receipt records its exact argv, duration, exit, source identities and log hashes. SHA-256 values follow:

| Check | Outcome | Receipt SHA-256 | stdout SHA-256 | stderr SHA-256 |
| --- | --- | --- | --- | --- |
| `deadline` | 1 passed, exit 0 | `6072748e9e6be3858e44f2d19188a130e2ed151dcadec951ab6ee9aaf5e81631` | `f050ca37d45ce6acb0d40e2d08d515ef5e5550a4bafb3a33293d1d25ff6d8e3a` | `1307fcbca6f0bc044d51f32353c1c804c27b99b67114411bc1822af9b201b298` |
| `missing-peer` | 1 passed, exit 0 | `30fd33aff8aa1e31601a28f07763cd081281322bcf577729d3171ec7228c31a8` | `a0ac22783927bc0658c0aa03b9331d4b94525f6faa249e1f1bc44426aeccf65b` | `86c57d35c211112024fa39fae3591ff3f769b7527fe98864b5f16291aee6690a` |
| `slow-setup` | 1 passed, exit 0 | `be731182a140ab2ae255a64ebdef2b714d0847d1310ce6e525ceb76c813c9d16` | `111f751e033e10f0beaccad45495c3205751c90cf10bf01e7f9c1382c7bff825` | `4eb7169210fe8b82d118aa17a6ba968df4a7c2af807bc268c7be42d00b0de910` |
| `integrity` | 1 passed, exit 0 | `0a3ad023e23a660ac5c088776728ecbd025a09df246683dcf56c7c295ff6e863` | `a7252e19427b45e4220f73ebf9154cd9cbeb4cc9f17e35cb973bae8c522c4b7e` | `525c22ec4a9b3f9c694db27d2af2c28daabeccefee2118b9e67332cae0cb9072` |
| `historical-group` | 9 passed, exit 0 | `3d920f1606adce3141694f011b952f4f73ff241a5bb3aa5597f0243242a738c7` | `e80d837a563a4d29ddadc1c4e13e70b965d813349f11379252df27b3828c74d2` | `525c22ec4a9b3f9c694db27d2af2c28daabeccefee2118b9e67332cae0cb9072` |
| `fmt` | pass, exit 0 | `59d4d5b3cc7b5760811fbaaca69044cecb45ed2416e06c3cb617dd04c7ce9f13` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `clippy` | pass, exit 0 | `16c106342f3569f963d83a954a97f93db48b31baccce0921285b90f2f9234d5b` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `d85f936173749dd3c630874f6d4a60820ce4e04d0344366764106b245e96141b` |
| `instrumented-workspace` | 570 passed; 0 failed/ignored/filtered, exit 0 | `e4fda9d03fed82bb0a26ad090169babc68fda64b7a72a944b785c4e5b32c70fe` | `2ceefaaf96c69cac482be06df7e1e0a1572521f54842872112a4e4798c8786d1` | `f9b05773d4ba62f87c6c37054c4d4a67f4c8a34a3e43dc9675c81eb18533c96f` |

Commands:

- `deadline`: `cargo test -p arc-cli --bin arcctl --locked --offline exec::transport::historical_tests::tests::absolute_deadline_is_shared_and_partial_reads_cannot_renew_it -- --exact`.
- `missing-peer`: `cargo test -p arc-cli --bin arcctl --locked --offline exec::transport::historical_tests::tests::missing_peer_connection_fails_within_the_hard_deadline -- --exact`.
- `slow-setup`: `cargo test -p arc-cli --bin arcctl --locked --offline exec::transport::historical_tests::tests::slow_setup_cannot_consume_the_exchange_deadline -- --exact`.
- `integrity`: `cargo test -p arc-cli --bin arcctl --locked --offline exec::transport::historical_integrity::fixed_historical_transport_and_pinned_tests_match_baseline_hashes -- --exact`.
- `historical-group`: `cargo test -p arc-cli --bin arcctl --locked --offline exec::transport::historical -- --test-threads=1`.
- `fmt`: `cargo fmt --all -- --check`.
- `clippy`: `cargo clippy -p arc-cli --bin arcctl --tests --locked --offline -- -D warnings`.
- `instrumented-workspace`: `rustup run 1.95.0 cargo llvm-cov --workspace --all-targets --locked --offline --no-report --no-cfg-coverage --no-cfg-coverage-nightly`.

## Handoff and limits

The standalone coverage gate, the full harness, final evidence binding and source-bound QA refresh remain with their assigned downstream tasks. The instrumented test pass is not a coverage-percentage measurement. Existing source-bound evidence for the previous source identity must be refreshed by those owners.

The source provider Serena was unavailable; inspection used scoped reads of the assigned files and the retained log. A read of the guessed `scripts/coverage/collect.ts` path failed because it does not exist; the actual `scripts/coverage/run.ts` was then inspected. `av.yaml` remained inaccessible, so no complete worktree-status claim is made. A `ps` process-monitoring attempt returned `operation not permitted`; the instrumented subprocess was monitored through its own logs and exited normally. The test necessarily has a finite exchange deadline, so prolonged process starvation beyond it remains possible; the change removes one-time initialization from that timed interval and synchronizes peer readiness.
