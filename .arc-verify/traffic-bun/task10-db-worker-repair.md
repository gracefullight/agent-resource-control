# Task 10: database-worker cancellation test repair

Status: completed. Skill used: `oma-backend`. The repair changes only `crates/arc-daemon/src/db_worker/tests.rs`; `execution.rs` and production behavior are unchanged. No migration, manifest, lockfile, or other Actor's files were changed by this task.

## Investigation

The failed `cargo test --workspace` receipt at `target/traffic-bun/commands/task8-attempt6-workspace-8624e2d1-c462-4874-b32d-2939c3a9729e/receipt.json` records exit 101 and source hash `b80c9452c4ab6011fc6dc66af609d822b31aadf2875bb08784a22e78593ba590`. Its SHA-256 is `bdca8693f4c3539e2171ccd96c2725f666f1fb0d403bff46d77546e150ebf006`; its stdout and stderr hashes, verified against the receipt, are `1aa4767355c0c8811f61039388f4bd21090b3abdbbc1046558b7c09d17fc6405` and `c861ae01d6a431cc982811de702f2468d8b0434e18c7724c7f405779706e1a10`. The panic was `TrySendError::Disconnected` at the release `try_send().unwrap()` in `cancellation_before_statement_and_early_interrupt_cannot_escape_sticky_checks`.

The gate checks cancellation while it waits for release. If cancellation is observed first, it returns and drops the receiver before the test sends. The old log cannot prove the exact preceding thread sequence. A controlled reproduction retained the reply receiver while dropping the ticket, observed `DbError::Cancelled`, and then reproduced the same disconnected-send panic. This establishes that the reported failure can occur without a production defect.

## Repair and acceptance

- [x] The cancelled-first regression requires the worker's explicit `DbError::Cancelled` reply before accepting precisely `TrySendError::Disconnected(())`. A successful send or a full channel fails that assertion.
- [x] The existing sticky-cancellation test now uses the worker's active mutex to keep its gate receiver alive while testing both send-before-cancel and cancel-before-send. Both sends must succeed; the ticket is dropped, its cancellation flag is checked, and the worker must return `DbError::Cancelled`. Holding the mutex also prevents SQL from passing its check before cancellation.
- [x] Every case retains the early SQLite interrupt, no-progress assertion, subsequent doctor recovery, and `stop` checks for a joined worker, closed connection, bounded and zeroed accounting, worker counters, and a reopened database with intact migration history. The reply is retained only by the test so it can observe the result of the dropped ticket.
- [x] The database-worker group passed 15 tests and the complete daemon unit-test binary passed 71 tests, with zero ignored. Formatting and warning-denying daemon Clippy passed.

## Source identity and checks

The accepted and current HEAD is `16772fe2aa350413225f3fb2bb9227d14ae5a6c4`, tree `1834e0cccb50a12fea299618f9a360f4ca10e03f`. The existing `missionInputs` validator hashes 319 working-tree inputs to `f7f8da187251f11c9a06169fd3f0743a40a5892b603202bd6c80b283683d0724` for all passing checks below; each command receipt records the same hash before and after execution. The test file SHA-256 is `bd585db734794c4745f5a7ca123c1a1af6a5a5025bc483b059ed2bce74821d4a`; unchanged `execution.rs` is `aa2d4c01fbb7b2b1a5ca9220eca4315fefab5a1ea0817a6af6cb2fd79500861b`. The RED experiment used source hash `a1f40d36dcb1aac987873ed4c528ce5e8cc1c0f2b8049794512b865249d12606`. Host: Darwin arm64, Cargo and Rust 1.95.0, Bun 1.4.4. Commands ran serially with `CARGO_BUILD_JOBS=4` and a 7,200-second per-command allowance.

Raw receipts and streams are under `target/traffic-bun/task10-db-worker/`. Receipt hashes are SHA-256; each receipt also contains the command, timing, exit, source identity, and stream hashes.

| Command | Result | Receipt SHA-256 |
| --- | --- | --- |
| `cargo test -p arc-daemon --bin arc-daemon db_worker::tests::cancelled_gate_drops_release_receiver_before_late_send -- --exact --nocapture` before the fix | Exit 101, intended disconnected-send panic | `red.json` `f3ad7e3a5f5b675ac8fcf03ca9b1d5d0f0450ef243e5341b93e4a29f3ccf87e4` |
| Same command after the fix | Exit 0, 1 passed | `green.json` `32dc16918d450e2065f80bc92df26923bd36be03819b66bb4d3aa7b3406895fd` |
| `cargo test -p arc-daemon --bin arc-daemon db_worker::tests:: -- --nocapture` | Exit 0, 15 passed | `db-worker.json` `5bac36a707bd1bdf26baba86605b20bc2fe65ddd59c4d0b60336b4777881c5cf` |
| `cargo test -p arc-daemon --bin arc-daemon` | Exit 0, 71 passed | `daemon-unit.json` `2c3d63d2be76467e2a133b962e1e46c6c243b36ed3f82b27e7ac7a9559380ee8` |
| `cargo fmt --all -- --check` | Exit 0 | `fmt.json` `13c6cbb533e093e871beceb02ea9c4927ae167f939b500a0cea676a574105ae4` |
| `cargo clippy -p arc-daemon --bin arc-daemon --tests -- -D warnings` | Exit 0 | `clippy.json` `9be6fad4c2ceb17bf1be990996cf8fe9dbadf2bc2d16f407e931e2a555e02fee` |
| `git diff --cached --check -- crates/arc-daemon/src/db_worker/tests.rs` | Exit 0 | `diff-check.json` `94d300b3a98f7707e204af9c636ad5b8502a1900afcdfda1c997dd80e87f737f` |

For each row, `<label>.stdout.log` and `<label>.stderr.log` are in the same target directory. Their SHA-256 values are:

| Label | stdout | stderr |
| --- | --- | --- |
| red | `45ec2908bca18984cebc3cf555f6dc928ef0f6227bf0882ee2ca81ee7537df1a` | `7dbe28a271aaea370c39d1c63e0318527f3b0927b85f394ed1f3b1827325a077` |
| green | `2936b1331d2f5381cb434e13205c87beafe31a824dc430a7edc0417cb6853729` | `55efb4c8e2305a3c2fcb105b8016d6bfe5cf5190a9faf3eea13a94a7da0d401e` |
| db-worker | `699c8dc35ed0d67c0cc516502a22d7d16ed4ef2107bd8ad498a78a00d4b248ab` | `5c5f62d869f1fac99ce0c871830209244fd2fd810ea767058098ee496aa57f84` |
| daemon-unit | `3dd38a14717cdd26b5a7dda5bbd208e5b3c84302bfcd17780d99e9aa4ccad41a` | `0f838e1b8d67ffbd88797ebed5b1b8ae7340aef2dd937f870d302ede19b6a018` |
| fmt | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| clippy | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `94449063a490b49f9425596ef93c1aea3f04bd16df210de09e4b92be964e6881` |
| diff-check | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |

## Handoff limits

This is focused repair evidence, not a fresh full-workspace, coverage, harness, or final mission verification. The retained `qa.json`, `gates.json`, `gate-summary.json`, and `mac-acceptance.json` still identify source hash `b80c9452c4ab6011fc6dc66af609d822b31aadf2875bb08784a22e78593ba590`; the Chief must rerun or refresh the source-bound evidence as its validators require. The evidence directory contains 18 files and less than 320,000 bytes. Serena tools were not available; investigation used direct reads of the assigned paths and retained logs. `git status` reported `av.yaml: Operation not permitted`, so no claim is made about that path's working-tree state.
