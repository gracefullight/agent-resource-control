# Library coverage handoff — task-3

Status: partial. The additive library tests and focused verification are ready for integration. Every owned crate exceeds the new 80% engineering target in a fresh four-library diagnostic. The required shared workspace measurement remains blocked by the process-owned CLI schema fixture; this report does not claim workspace gate or mission completion.

The accepted committed baseline is `fbe76a5f15101ad415f355ef06f1deeb1f97d568`, tree `8db02719b5bd06d888729ca337f9bf8887991515`. This Actor inspected local snapshot HEAD `6175ac9ba822ca20c0529394cd22e0ac8a4b9d48`, tree `23e3dd57a4929d7c30e17472121d015b3ea95f83`. The new tests are Actor-local working-tree changes, absent from the accepted committed baseline and snapshot commit. No commit, mission graph, task instructions or Actor assignments were changed. Exact source bytes and patch hashes are in [delivery-source.json](delivery-source.json) and [tests.patch](tests.patch); integration must verify those bytes in its own checkout.

The existing accessible scoped diff was empty before implementation. Global Git status/diff diagnostics reported `av.yaml: Operation not permitted`; its contents were not accessed and global cleanliness is unverified. Serena was absent from the available tool inventory. Inspection used task-supplied paths, directly read module declarations and the inherited coverage inventory, without native project code search. [inspection.json](inspection.json) records the initial state, platform and retained mission budgets. No subagents were dispatched.

## Files delivered

- `crates/arc-core/tests/coverage.rs`: isolated subprocess probes for absent/empty HOME, empty/default database overrides, override precedence and non-UTF-8 override bytes. Environment changes occur only in the owned child, avoiding parallel test environment races.
- `crates/arc-detector/src/coverage_tests.rs`: empty nested-shell handling, conservative SSH destination and option boundaries, numeric IPv6 compression rejection and unquoted shell control words. All assertions exercise the existing analysis interface.
- `crates/arc-detector/src/lib.rs`: only an appended `#[cfg(test)] mod coverage_tests;` declaration; production tokens are unchanged.
- `crates/arc-storage/tests/coverage.rs`: invalid database paths, caller-file preservation, memory-URI WAL rejection, a view masquerading as history, invalid UTF-8 history, and typed error display/source chains. The history checks verify unchanged bytes/change counters and autocommit state through public APIs.
- `crates/arc-ipc/tests/coverage.rs`: fragmented/interrupted transport, header/body EOF and IO failures, write/flush failures, exact frame-size boundaries, error display/source chains, failed serialization, typed decode errors, request/response exchange and doctor failure privacy/recovery.
- `.arc-verify/rust-coverage/library/`: reports, raw diagnostic exports, source fingerprints, line deltas, command receipts, supplemental measurement helper and an applyable source patch.

There are 22 added test items, including the core child probe. No previous assertion/helper was edited or removed. No crate manifest, dependency, lockfile, shared gate script, daemon/CLI file or production behavior was changed. Existing Rust/IPC/storage/CLI interfaces remain compatible. The only production-file edit is the test-only module declaration above. No production coverage exclusion or coverage-only cfg was added. Every delivered Rust source file is below 500 lines.

## Measurement and original uncovered-line trace

Task-1's actual accounting contract is [../contract/accounting.md](../contract/accounting.md). The consumed inherited run is `2026-11-07T23-42-57-893Z-acb0fb6d-78cf-4df8-bfdc-c971e85992db`, source/configuration hash `23db85747b5e61c3637d93684e84aba95fd6c8bd5d924135541bc3c875859399`. Its copied summary, inputs, versions, receipt and uncovered report are prefixed `inherited-` here. These are inherited workspace observations, not fresh results on the new tests.

Final diagnostic: [libraries-2026-11-07T23-59-54-881Z/summary.json](libraries-2026-11-07T23-59-54-881Z/summary.json). Its actual source/configuration SHA-256 is `a0df31d544fe0afd0425baf8cf8d20b74a2c0a94c8a39073e41b28bc1f3b8cd5`. All final focused/fmt/clippy/preservation/root-command receipts bind to this same input hash. Before/after hashes were equal throughout each check.

| Crate | Inherited workspace count | Final library diagnostic | Final percentage |
| --- | ---: | ---: | ---: |
| arc-core | 64/64 | 64/64 | 100.00% |
| arc-detector | 572/578 | 577/578 | 99.83% |
| arc-storage | 214/227 | 226/227 | 99.56% |
| arc-ipc | 178/215 | 215/215 | 100.00% |

Each crate individually exceeds 80% using integer counts. Eighty percent is the mission's new engineering target, not an SRS threshold or historical measured baseline. [line-delta.json](line-delta.json) confirms that every canonical production mapped-line set is identical between these two scopes: no denominator reduction, added test lines or dropped zero-hit paths explain the increase. A total of 54 originally uncovered production lines are now hit, with no newly uncovered library line.

| Original file/lines | Added cases and measured outcome |
| --- | --- |
| `arc-core/src/lib.rs`, no original uncovered lines | New public configuration boundary assertions retain 64/64; no line-coverage improvement is claimed. |
| `arc-detector/src/rules.rs`: 107, 153, 158, 173, 250 | SSH option terminator after a switch; empty/invalid username; empty host; unmatched bracket; eight-group compressed IPv6. All five now hit. |
| `arc-detector/src/parser.rs`: 98 | Empty nested-shell and reserved-word semantics are asserted. This `one_of(...)?` work-budget propagation line remains zero-hit and stays in the denominator. |
| `arc-storage/src/history.rs`: 59, 86 | History-name view and invalid UTF-8 TEXT history. Both now hit; bytes remain unchanged. |
| `arc-storage/src/lib.rs`: 33–37, 41, 47 | Empty/`:memory:` paths, a file blocking parent-directory creation, directory-as-database and memory URI without WAL. All nine now hit. |
| `arc-storage/src/migration.rs`: 48, 49, 66, 116 | SQL/registry error formatting and no-source behavior hit 48, 49, 66. Line 116 remains zero-hit. Additional incompatibility-message tests keep those branches covered without relying on daemon tests. |
| `arc-ipc/src/lib.rs`: 139–146, 150, 152, 156–159, 161, 181–186, 188, 192–197, 199, 215–217, 230, 250, 261, 264, 345 | Error format/source chains; interrupted header, partial header/body IO errors; outbound length rejection; failed doctor callback. Every one of the original 37 gaps now hits. Public exchange tests additionally retain 311–314 in the library-only scope. |

Per-file mapped/omitted/uncovered lines are retained in the final diagnostic summary and [uncovered.md](libraries-2026-11-07T23-59-54-881Z/uncovered.md). Remaining owned zero-hit lines are exactly parser line 98 and migration line 116. The duplicate-ID check at migration line 116 follows ordered-version/parsed-ID validation; that preceding invariant appears to make its rejection branch unreachable. This is a code interpretation, not an exclusion or a reason to change production behavior.

The supplemental [measure.ts](measure.ts) reuses the shared pins, command recorder, source inventory, cfg(test) classification, source fingerprinting and report freshness checks. It selects the four owned packages without skip flags, runs unit/integration tests in a new instrumented target, exports untouched native LCOV and JSON, corroborates every DA count and LF/LH against native segments/summaries, and unions aliases by canonical production path and line. It excludes only dependencies/compiler roots and test-only source per the contract. Its own hash is bound in the result. All 39 resulting profiles match the actual merge input list; hashes are retained in `profiles.json`. Native exports, exact commands, source inventory, versions and fingerprints are in the same run directory.

This diagnostic has a four-library scope and is not the shared six-crate validator or a substitute for the failed root gate. It does not measure daemon/CLI production coverage or establish mission CLI acceptance. The failed workspace runs did execute the existing binary and classification CLI tests successfully before the schema fixture failed. All raw failures are retained rather than being converted into passing coverage.

Measured host: `aarch64-apple-darwin`; Bun 1.4.4; rustc 1.95.0 (`59807616e1fa2540724bfbac14d7976d7e4a3860`); cargo-llvm-cov 0.6.21; matching LLVM 22.1.2 tools. The operator's changed prerequisite environment was checked again by both root attempts and diagnostics: pinned tools and locked offline dependencies were available. This Actor installed/downloaded nothing.

## Commands and outcomes

[command-index.json](command-index.json) indexes every implementation check, with exact argv, exits, times and source hashes. Each `<label>.json` retains full before/after input inventories; `<label>.stdout.txt` and `<label>.stderr.txt` preserve output. Measurement subcommands have separate numbered native receipts.

| Exact command | Labels | Outcome |
| --- | --- | --- |
| `cargo test -p arc-core -p arc-detector -p arc-storage -p arc-ipc` | `focused`, `focused-final` | Both exit 0; final has 82 passed, zero failed/ignored/filtered tests. No doctests are present. |
| `cargo fmt --all -- --check` | `fmt`, `fmt-final` | Both exit 0. |
| `cargo clippy --workspace --all-targets -- -D warnings` | `clippy`, `clippy-final` | Both exit 0. |
| `bun test scripts/coverage/preservation.test.ts` | `preservation`, `preservation-final` | Both exit 0; production tokens and all existing Rust tokens/assertions preserved against accepted HEAD. |
| `bun run test:coverage` | `coverage`, `coverage-final` | Both exit 1 at the same existing CLI schema fixture. Final: 25.43 seconds, source stable. |
| `bun .arc-verify/rust-coverage/library/measure.ts` | `diagnostic`, `diagnostic-final` | Both exit 0, scoped diagnostics only. Final: 17.04 seconds and the four results above. |

The bounded receipt wrapper is invoked as `python3 .arc-verify/rust-coverage/library/check.py LABEL COMMAND...`; it executes the exact command argv shown, without rewriting the operator's command. `rustfmt --edition 2021` formatted only the new owned test files. Git metadata/diff and direct baseline-byte comparisons supplied provenance and [preservation-details.json](preservation-details.json); no software build command was separately invoked. Tests performed their normal compilation.

Both protected SHA-256 values equal accepted HEAD byte-for-byte:

- `scripts/harness/validate.sh`: `93140173df8b8bd28b067aff68301134b0398b643240b0b1e6a13da4487ebe33`
- `crates/arc-storage/migrations/0001_phase0.sql`: `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`

The five owned source paths and this library evidence directory are staged for delivery, without committing. The code-only `git diff --cached --check` exits 0. The broader source-and-evidence check exits 2 on blank trailing lines in verbatim test stdout and a blank context line inside `tests.patch`; those captured bytes are retained unchanged. Exact commands and diagnostics are in [whitespace.json](whitespace.json). [delivery.json](delivery.json) binds every report/raw export/receipt by size and SHA-256; `delivery-source.json` separately binds the five source files and patch.

## Blocker and Chief Director handoff

The final unmodified root command produced failed run `2026-11-07T23-59-16-087Z-c03595ea-3a4f-4616-abe8-82f405ed9781`. [failed-final-workspace-receipt.json](failed-final-workspace-receipt.json), [final-workspace-test-failure.stdout.txt](final-workspace-test-failure.stdout.txt) and [coverage-final.json](coverage-final.json) bind the failure to the delivered source hash. The native Cargo test subprocess exits 101, cargo-llvm-cov exits 1 and the root Bun command exits 1. No fresh workspace report exists, and the inherited passing report is not reused as current acceptance.

Reproduced failure: `inconsistent_or_unknown_success_envelopes_cannot_claim_schema_compatibility` panics in its server thread at `crates/arc-cli/tests/schema.rs:57:67` with `Frame(Io(Os { code: 35, kind: WouldBlock, message: "Resource temporarily unavailable" }))`; its parent then panics at line 73 joining that thread. Both attempts reproduce the same failure. This is the process Actor's owned path, named in the operator note and task-1 handoff. No out-of-ownership repair was attempted. Chief should integrate these tests with the process Actor's schema-fixture repair, then rerun the exact root gate and remaining required mission commands on the integrated bytes.

The shared root measurement necessarily generated two new `baseline/runs/` failure directories and updated generated `baseline/latest.json` to failure. These are automatic command outputs, not edited gate/configuration or fabricated receipts; copies sufficient for this handoff are under this owned library directory. The final supplemental measurement does not change that failed shared pointer.

Acceptance checklist:

- [x] Additive deterministic tests exercise measured uncovered paths in all applicable owned libraries; original assertions preserved. Core was already fully line-covered and gains semantic boundary assertions.
- [x] Focused native receipts and source-bound four-library diagnostics independently exceed 80% for each owned crate, with unchanged mapped denominators and explicit remaining lines.
- [x] Cases trace to original uncovered lines; commands, raw reports, source hashes and preservation checks are material files.
- [ ] Required shared workspace measurement succeeds on delivered source: blocked at the process-owned schema fixture. No overall completion claim is made.

Production/API compatibility is unchanged. Native `cargo test --workspace`, the unchanged full harness and independent oma-qa remain Chief/integration obligations; they were not rerun or certified by this library Actor. Historical task-1 harness timeouts are not fresh evidence for this task. Measurement covers only active macOS cfg; inactive Linux/Windows code remains unmeasured, and stable-toolchain doctest coverage is outside the collected coverage scope. No platform path was removed.

- desktop: incomplete
- native Windows: incomplete
- remaining Phase 0: incomplete

Skill used: oma-backend with the installed backend-engineer definition and its execution/checklist references. HTTP/auth/schema-change checklist items do not apply to test-only work. No browser, GUI, Playwright, publication, external messaging, credentials or delivery outside the workspace was used. The result is a scoped implementation handoff, not the assigned independent QA review.
