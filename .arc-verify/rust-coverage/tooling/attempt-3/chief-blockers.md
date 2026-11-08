# Task-1 prerequisites and ownership handoff

Task-1 remains incomplete. The LCOV summary validation defect is repaired in the owned coverage scripts. This report requests Chief coordination; it does not transfer file ownership or change the mission contract.

## Coverage prerequisites

Fresh attempt-3 checks found `cargo-llvm-cov` absent (exit 101), no `llvm-tools-preview` in Rust 1.95.0's installed components (component listing exit 0), and locked offline fetch failure for `anstyle-wincon 3.0.11` (exit 101). Rust is 1.95.0 with LLVM 22.1.2 on aarch64-apple-darwin; Bun is 1.4.4. See `tool-availability.json`, `components.json`, `rust-version.json`, `bun-version.json` and `offline-fetch.json`.

Supported provisioning remains `mise run coverage:provision`, with `rustup component add llvm-tools-preview --toolchain 1.95.0`, `cargo +1.95.0 install cargo-llvm-cov --version 0.6.21 --locked` and `cargo +1.95.0 fetch --locked`. The earlier exact attempts are preserved in `../recovery/cargo-llvm-cov.json`, `../recovery/llvm-tools.json` and `../recovery/cargo-fetch.json`. The Cargo installation failed opening `/Users/gracefullight/.cargo/registry/cache/index.crates.io-1949cf8c6b5b557f/cargo-llvm-cov-0.6.21.crate` with Operation not permitted; LLVM component setup rolled back because its download/cache preparation failed. Mise also still reports a denied tracked-config link under `/Users/gracefullight/.local/state/mise/tracked-configs/`.

No operator note reports an environment or permission repair in this assignment. Repeating unchanged denied installations is explicitly prohibited. This attempt rechecked availability and offline readiness, did not repeat those denied installations, did not relocate caches to evade the denial, did not change tool pins and did not use GUI tools. The Chief must resolve supported provisioning access before compatibility or a real first measurement can be accepted. No real raw LCOV, crate percentages, per-file uncovered lines or child-profile merge evidence exists. Keep dependent measurement-driven tasks blocked.

## Process-test ownership conflict

`cargo test -p arc-cli --test schema` freshly failed with six passing tests and one failure. `every_schema_failure_survives_error_envelope_with_actionable_reason` panicked at `crates/arc-cli/tests/schema.rs:57:67`, where `arc_ipc::read_message(&mut stream).unwrap()` received `Frame(Io(Os { code: 35, kind: WouldBlock, ... }))`. The listener is set nonblocking at line 37; the accepted stream gets five-second read/write timeouts at lines 51–56 but no explicit blocking-mode restoration. This is a diagnostic lead, not a verified root cause or repair.

The process-coverage Actor owns that Rust test file. Task-1 did not edit it or any production crate. The Chief should coordinate investigation and a bounded-I/O repair with that owner, preserving all assertions, then obtain passing isolated schema and workspace receipts before real instrumentation. The subsequent exact `cargo test --workspace` passed (exit 0, 50.170 seconds), including its schema target. This does not repair the freshly reproduced isolated failure: the evidence establishes intermittent behavior. See `schema.json` and `workspace-tests.json`. Existing helper tokens are preserved by task-1's passing preservation check; if an owner needs a justified test-helper change, coordinate its preservation implications rather than weakening assertions or the gate.

## Boundaries

Independent oma-qa and final mission acceptance remain for the Chief to coordinate. No missing-prerequisite result is a measured baseline. Eighty percent remains the new per-crate engineering target.

- desktop: incomplete
- native Windows: incomplete
- remaining Phase 0: incomplete

## Fixed harness time budget

The current real harness receipt is `../acceptance/harness-6d5b81a6-77e2-4f73-9bd6-cfc39fb80e11/00-command.json`. It executed exactly `./scripts/harness/validate.sh` with no skip flags, scanned 3,206 tracked files, passed checks 1–3, and was terminated at the retained 115-second child-command deadline during check 4 (file lengths). Its actual exit was 143; the Bun harness acceptance returned 1 after 116.448 seconds. Input hashes before and after were identical. It never reached check 5, so the current timeout must not be described as a completed one-violation coverage failure. Prior harness receipts remain historical. No harness bytes, skip behavior, timeout or verification command were changed. Performance within the fixed 120-second contract remains an additional unresolved prerequisite; this attempt does not establish that an instrumented successful run can fit.
