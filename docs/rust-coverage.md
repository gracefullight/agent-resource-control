# Rust coverage

From the repository root, run:

```sh
bun run test:coverage
```

For retained historical receipts and amendment verification, use the [evidence index](../.arc-verify/evidence-hygiene/index.md). Current run reports are local and ignored by Git; a historical pass does not verify this checkout.

The gate runs unit, integration and instrumented CLI/daemon tests across all six Rust crates. Each crate must reach **80% production-source line coverage**. This is a new engineering target, not an observed baseline or SRS threshold. A passing coverage run does not complete the separate Cargo, harness or independent QA requirements.

## Set up once

Use these exact tools:

| Tool | Required version |
| --- | --- |
| Bun | 1.4.4 |
| Rust and Cargo | 1.95.0 |
| cargo-llvm-cov | 0.6.21, installed with its locked dependencies |
| llvm-tools-preview | Rust 1.95.0 component; matching LLVM 22.1.2 |

With mise and rustup available, explicitly provision before running coverage:

```sh
mise run coverage:provision
```

The task in [mise.toml](../mise.toml) invokes [provision.sh](../scripts/coverage/provision.sh), which runs these commands in order:

```sh
mise install
rustup component add llvm-tools-preview --toolchain 1.95.0
cargo +1.95.0 install cargo-llvm-cov --version 0.6.21 --locked
cargo +1.95.0 fetch --locked
```

Provisioning requires access to the runtime/tool registries, writable tool homes and mise caches. It stops on the first failure. If mise reports a denied cache write, resolve that prerequisite; its failure does not establish that later setup steps ran. Where Bun 1.4.4 and Rust 1.95.0 are already installed, the remaining three commands above provision LLVM, the pinned coverage driver and workspace dependencies explicitly. The gate checks installed versions and never installs anything. Its Cargo operations use locked, offline dependencies; fetch them during setup.

Run from an environment without Rust/LLVM flags, compiler wrappers or target overrides. The gate rejects nonempty `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`, `LLVM_COV_FLAGS`, `LLVM_PROFDATA_FLAGS`, `RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER`, `CARGO_BUILD_TARGET`, and matching Cargo profile/target Rust flag overrides. This keeps the measured configuration fixed.

## Repeat use

1. Run `bun run test:coverage` from the root.
2. Read the six labeled results, in order: arc-core, arc-detector, arc-storage, arc-ipc, arc-daemon and arc-cli.
3. Open the directory printed after `Coverage reports:`. Read its `uncovered.md` for each production file's covered/total count and uncovered line numbers.
4. Repair the relevant source or tests, then rerun the same command.

Output announces three stages: checking pinned tools; running workspace unit, integration and child-binary tests; merging fresh execution profiles and validating reports. Valid measurements show `PASS` or `FAIL`, the crate name, covered/total production lines, a percentage and the required 80%. Labels convey the result without relying on color or a prompt.

Percentages are displayed to two decimal places. The decision uses exact integer counts: `covered * 100 >= total * 80`. Exactly 80% passes; a lower value that rounds to 80.00% fails. Every crate must pass independently; a workspace average cannot compensate for a failing crate.

When measurement cannot complete, each crate is labeled `UNAVAILABLE` with no valid current measurement. This is neither zero coverage nor a pass. A valid below-threshold measurement retains its real counts and `FAIL` result. Both cases exit nonzero.

## Diagnose a failed run

Failures print `Coverage unavailable or failed:`, a diagnostic and an `Evidence:` directory. Numbered command receipts there include the actual argv, exit, stdout and stderr. Test output is captured in those receipts rather than streamed to the terminal.

| Diagnostic | Meaning | Next action |
| --- | --- | --- |
| `MISSING_TOOL` | A required command, toolchain or LLVM binary is unavailable. | Check the command receipt; complete explicit setup above and rerun. |
| `TOOL_VERSION` | Bun, Rust, cargo-llvm-cov or LLVM differs from the pins. | Activate the pinned versions and matching Rust LLVM component. |
| `TEST_FAILURE` | The instrumented workspace test command failed or timed out. | Read that command's stdout/stderr, repair the reported test failure and rerun. An older passing report cannot satisfy this run. |
| `MISSING_REPORT` | Required fresh execution profiles or a native report are absent. | Inspect the collection/export receipts and repair the missing output; rerun the whole command. |
| `STALE_REPORT` | A report predates its collection step, or source/configuration changed during measurement. | Finish edits, keep inputs stable and rerun. Do not copy a previous report into this run. |
| `BELOW_THRESHOLD` | At least one valid crate measurement is below 80%. | Use the named crate counts and `uncovered.md` to add meaningful tests, then rerun. |
| `MALFORMED_REPORT`, `MISSING_CRATE`, `MISSING_FILE`, `UNMAPPED_FILE` | Accounting cannot validate the reports or inventory. | Inspect the raw reports, inventory and receipts; repair the tooling/source-layout mismatch before trusting a percentage. |
| `CHILD_PROFILE`, `REPORT_FAILURE` | Child execution or profile merging/export lacks required proof. | Inspect child PIDs, profile inventory and report command receipts; repair collection and rerun. |
| `CONFIGURATION`, `PLATFORM`, `METADATA_FAILURE`, `SOURCE` | The environment, workspace layout or platform does not meet the measurement contract. | Resolve the stated configuration or prerequisite; retain the failed receipt. |

The harness may append inherited Vitest guidance. This root command measures Rust coverage; use its diagnostic and receipts to identify the actual failure.

## Reports and source identity

The local runtime pointer `.arc-verify/rust-coverage/baseline/latest.json` identifies the most recent attempt and its pass/fail state. It updates on failure too. It is not a promise that the latest run passed. The pointer and run directories are ignored and are absent in a clean checkout until collection. Use the directory printed by your invocation; older passing runs remain historical evidence.

Each attempt has a unique directory under `.arc-verify/rust-coverage/baseline/runs/`. Files appear as their stage completes:

| Artifact inside a run directory | Contents |
| --- | --- |
| `uncovered.md` | Per-file covered/total counts and uncovered line numbers. |
| `summary.json` | Six crate verdicts; per-file mapped, omitted and uncovered lines; source and raw-report hashes; host and versions. |
| `raw.lcov`, `raw.json` | Unmodified native exports from the same profile set. |
| `inputs.json`, `source-inventory.json` | Exact input hashes, production/test classification and omitted test-only spans. |
| `versions.json`, `metadata.json`, `non-workspace-roots.json` | Tool/host versions, locked Cargo metadata and recorded dependency/compiler filter roots. |
| `profiles.json` | Raw-profile paths, sizes and hashes, explicit child PIDs and the exact merge list. |
| Numbered `*-command.json` files | Actual commands, working directories, times, PIDs, exits and captured output. |
| `receipt.json` | Attempt verdict and command receipts, including failed attempts. |

Run directories, profile directories, `.profraw`, `.profdata`, `.lcov` and native raw JSON exports are local evidence, not distributed tracked proof. Future CLI/harness acceptance receipts are also local under `.arc-verify/rust-coverage/tooling/acceptance/runs/`. Selected historical summaries, uncovered-line reports, compact receipts and verdicts remain available through the [evidence index](../.arc-verify/evidence-hygiene/index.md).

Raw `.profraw` files and the merged profile stay under `target/arc-coverage/` in that run's unique target directory. The profile manifest records their exact location. A failed prerequisite may leave only inputs and a failed receipt; absent summaries mean measurement is unavailable.

The original accounting/preservation baseline remains `fbe76a5f15101ad415f355ef06f1deeb1f97d568`; existing report fields named `acceptedBaseline` retain that meaning. The evidence-hygiene amendment starts from accepted committed source `aacb3b4359f13c31c0705c0cffc5bf1ac00f387e`. Neither label binds a measurement to later edits. The gate hashes crate sources, tests, manifests and migrations; Cargo.lock; root tool/package configuration; the machine-readable accounting contract; coverage scripts; and any root Cargo configuration/build script. It checks equal before/after input hashes and fresh report timestamps. Documentation files are outside that implementation-input fingerprint. Consult each run's input manifest for the actual measured bytes.

Native exports retain the collecting checkout's absolute source paths. Matching input hashes alone do not make copied reports replayable in another worktree. Run evidence checks in the collecting checkout, or collect a fresh run in the checkout being verified. Keep raw exports unchanged.

## Verify current local evidence

Collect in the checkout being verified, then run the live evidence check:

```sh
bun run test:coverage
bun test scripts/coverage/evidence.test.ts
```

Run them in order after finishing implementation edits. The second command requires a successful latest run, matching current input hashes, unchanged native LCOV/LLVM JSON exports and actual profile bytes. It recomputes accounting, checks profile hashes and merge-list equality, and verifies CLI/daemon child PIDs. A retained summary or compact receipt cannot replace those inputs.

If the pointer or reports are missing, collect coverage here. If the latest attempt failed, follow its diagnostic and rerun collection successfully. If inputs changed or reports are stale, finish edits and recollect; do not restore a passing pointer or copy historical exports. Native reports contain checkout-specific paths. Serialize coverage and harness collection in one checkout, then run the live evidence check after the last collection.

The [clean-checkout check](../scripts/coverage/clean-checkout.test.ts), invoked with `bun test scripts/coverage/clean-checkout.test.ts`, separately creates an isolated checkout from the current Git index, verifies no prior run artifacts exist, collects coverage and checks native evidence there. Stage the intended implementation and retention changes first. Its local checkout and receipts stay available for inspection; a previous isolated pass does not verify later source changes. Hygiene checks separately enforce the retained set. Historical preservation checks use isolated fixtures at their original revisions; current-checkout preservation uses the feature contract below.

## Preservation during managed exec implementation

Historical whole-Rust checks retain their original baselines: `fbe76a5f15101ad415f355ef06f1deeb1f97d568` for production tokens and `aacb3b4359f13c31c0705c0cffc5bf1ac00f387e` for exact files/bytes/modes. Their positive and rejection cases execute against isolated Git-object fixtures. Historical receipt meanings and accounting baselines stay unchanged.

The live [preservation guard](../scripts/coverage/current-preservation.ts) compares existing regression items and helpers with accepted feature HEAD `19d7a507044cbf83918f58cd41056905c275c8d4`. Production edits and new modules are allowed. Existing test/helper items, attributes, module reachability and assertion macros remain protected; add new standalone test items/modules rather than rewriting old cases. The guard checks unchanged `0001_phase0.sql` bytes in the working tree and index, its mode/type, the harness path/mode/type and executable lines, and coverage accounting/collection/evidence protections. Fixed adaptation recipes permit only this task's historical-fixture routing and longer verification timeouts. They do not select a later baseline.

Run the focused protections with:

```sh
bun test scripts/harness/managed-exec.test.ts
bun test scripts/harness/coverage-verification.test.ts
bun test scripts/harness/equivalence.test.ts
```

The nine existing coverage test files remain required. The inventory command runs the same five non-collecting files and identifies the same four separately required collecting/live checks. The clean-checkout check still snapshots the final index and rejects missing/stale inputs; it now applies live regression/migration protections instead of requiring historical production bytes. Coverage/harness command wrappers allow at least the framework's 3,600,000 ms per-check timeout; the clean-checkout test allows both of its sequential commands to finish.

The managed-exec [evidence validator](../scripts/harness/managed-exec-evidence.ts) reads real sources, index entries, compact command receipts, independent review, final reports and current native LCOV/LLVM/profile files. Its [test](../scripts/harness/managed-exec-evidence.test.ts) runs without substitute Cargo commands. Run the full file after implementation, native macOS gates and independent QA exist. Validator unit fixtures are labeled and cannot establish lifecycle acceptance. See [preservation.md](../.arc-verify/managed-exec/preservation.md) for the receipt interface and focused results. Lifecycle acceptance does not certify desktop, native Windows or Phase 1 scheduling.

## Production accounting

The [accounting contract](../.arc-verify/rust-coverage/contract/accounting.md) and [implementation](../scripts/coverage/accounting.ts) define the denominator:

- Inventory every Rust file under all six production `src` roots. Classify modules by reachability, including production modules compiled again by integration tests through path attributes.
- Count unique canonical production file/LLVM `DA` line pairs. Merge repeated compilation instances by line union; a line is covered if any instance has a positive hit. Retain mapped zero-hit lines.
- Omit integration-test sources, files reached only through test modules, and spans whose cfg is provably false with `test` disabled. Record the omitted spans. Unknown platform predicates remain in production scope. Unsupported or ambiguous layouts fail validation.
- Disable broad filename filters. Filter only recorded non-workspace dependency/compiler roots. Do not remove difficult production functions, error paths or platform code to meet the target.
- Require every production file in the report and a positive denominator for every crate. Present declarations-only files may have no mapped lines. LLVM determines instrumentable lines; total physical source lines are not the denominator.
- Corroborate LCOV line counts against a fresh native LLVM JSON export. LLVM 22.1.2 function-summary totals can differ from unique file lines; use the canonical line map for coverage and retain both raw exports unchanged.

The run uses one workspace/all-targets instrumentation pass without coverage-only cfg, plus real binary probes. CLI and daemon child profiles use process/module-specific filenames. The gate checks explicit child profiles and requires the complete fresh profile inventory to equal the native merge input list. A passing subprocess test alone is not proof of measured binary execution.

## Platform boundaries and remaining work

The retained historical measurement receipts record `aarch64-apple-darwin`, using the native host target. The gate admits Apple Darwin and Linux Rust hosts, rejects target overrides, and records the actual host in each summary. Admission of Linux is not evidence of a passing Linux measurement. Native Linux requires its own run; native Windows is not a supported gate host for the current Unix-socket CLI/daemon.

Inactive cfg branches are inventoried production source, but the host compiler does not map them into this run's executable denominator. They are **unmeasured**, not covered. Relevant inactive paths on macOS include:

| Source | Unmeasured branches |
| --- | --- |
| [arc-core/src/lib.rs](../crates/arc-core/src/lib.rs) | Non-macOS temporary runtime and instance-anchor paths, Windows `LOCALAPPDATA`, and XDG/home data paths. |
| [arc-daemon/src/platform.rs](../crates/arc-daemon/src/platform.rs) | Linux `SO_PEERCRED` and `/proc` start-time handling; unsupported-platform fallbacks. |

On Linux, macOS-specific runtime, peer-identity and process-start paths are inactive instead. Host coverage does not certify the other platform. Stable Rust doctest coverage is outside this unit/integration/executable collection. No HTML explorer, interactive setup wizard or retention prompt is required.

Required final checks remain separate: `bun run test:coverage`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `./scripts/harness/validate.sh` with zero violations and no skip flags, followed by independent QA. Read their current receipts before claiming completion.

- desktop: incomplete
- native Windows: incomplete
- remaining Phase 0: incomplete
