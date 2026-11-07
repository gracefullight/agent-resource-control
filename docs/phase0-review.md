# Phase 0 cargo gate review

Status: this run observed cargo fmt exit 0, cargo clippy exit 0, and cargo test exit 0.

Task: `task-review`, attempt 1. Workspace HEAD `3dee4ff`. The commands ran in this mission worktree on 2026-11-06 from `2026-11-06T03:54:57Z` through `2026-11-06T03:54:58Z`.

Evidence log: `target/tmp/review-gates-unsplit.log` (63 lines). SHA-256 `6f60b487af6e758eea98ed5e99e20c8eec91aa7e4a2a2feb123527e2f3e059a3`. This file did not exist before the run. Earlier logs, including `target/tmp/daemon-gates-unsplit.log` and `target/tmp/gate-unsplit.log`, are not the exit evidence for this review.

## Environment

```sh
export MISE_AUTO_INSTALL=false
export MISE_EXEC_AUTO_INSTALL=false
export CARGO_HOME="$PWD/target/cargo-home"
export TMPDIR="$PWD/target/tmp"
```

`rust@1.95.0` was written with no space. Bare `cargo` was not called. `mise use -g` was not run. No toolchain was downloaded. Rust sources, manifests, `rust-toolchain.toml`, `av.yaml`, and `.agent-valley` were not edited.

## Commands and exits

| Argv | Exit recorded in this run |
| --- | --- |
| `mise exec rust@1.95.0 -- cargo fmt --all -- --check` | cargo fmt exit 0 (`EXIT_FMT=0`). No formatter diff was printed. |
| `mise exec rust@1.95.0 -- cargo clippy --workspace --all-targets -- -D warnings` | cargo clippy exit 0 (`EXIT_CLIPPY=0`). Clippy finished the dev profile in 0.21s. |
| `mise exec rust@1.95.0 -- cargo test --workspace` | cargo test exit 0 (`EXIT_TEST=0`). Cargo finished the test profile in 0.11s. |

`cargo test --workspace` reported these results:

- `tests/startup.rs`: `migration_failure_exits_nonzero_with_one_stderr_sentence` ok; `startup_and_restart_exit_without_cleanup` ok. 2 passed.
- `tests/nfr010.rs`: `migration_failure_rolls_back_all_new_tables` ok; `foreign_keys_and_unique_resource_identity_are_enforced` ok; `nfr010_wal_migration_survives_close_and_reopen_without_cleanup` ok; `mismatched_checksum_is_rejected_without_reapplying_migration` ok. 4 passed.
- Unit tests and doc tests for `arc-core`, `arc-daemon`, and `arc-storage`: 0 failed.

## `wal_checkpoint` under `crates/`

A search of `crates/` for `wal_checkpoint` found one line:

`crates/arc-storage/tests/nfr010.rs:126` is the comment `close without wal_checkpoint: use SQLite's normal connection close path.` The next statement is `connection.close()`.

No file under `crates/` contains `PRAGMA wal_checkpoint` or a `wal_checkpoint(` call. The checkpoint operation is absent. The identifier text is present only in that comment.

## Scope left unchanged

This review did not add crates, did not copy the SRS, and did not start Sweeper. Pre-existing modifications to `.gitignore` and `.qwen/settings.json` were left as found.
