# Native macOS lifecycle acceptance

Accepted: Phase 0 `arcctl exec -- <argv...>` lifecycle management. Backend used oma-backend, independent QA used oma-qa, and Chief used oma-pm. Scheduling is not implemented. Outstanding: desktop, native Windows and Phase 1 scheduling. Commercial demand, revenue, savings, retention and ROI are unmeasured.

Accepted baseline: `19d7a507044cbf83918f58cd41056905c275c8d4`. Final input identity: `c1728da557260d92269fa306ed5cb79392530d2865917b04244256f4cb76676f` across 179 source, test, configuration, manifest, lockfile, migration and contract inputs. Host: {'platform': 'darwin', 'release': '27.2.0', 'arch': 'arm64', 'hostname': 'Eunkwangui-MacBookPro.local'}. [final-summary.json](final-summary.json) contains exact hashes and measurements; [receipt-index.json](receipt-index.json) binds real command outputs and independent review.

## Lifecycle evidence

A CLI-owned child receives native argv directly through Command, without shell reconstruction. Existing classifier rules, authenticated protocol-1 IPC, bounded SQLite worker and additive migration registry are reused. Durable registration precedes the single spawn. Stable lifecycle identity makes start/attach/complete retries idempotent. Reconciliation checks PID/start-token/boot identity, never replays work and does not invent an exit code for an unobserved outcome.

Native deterministic unit, daemon integration and CLI checks cover one child across lost IPC replies; argv, cwd, environment, binary stdin, stdout, stderr, descriptors, terminal behavior and exit 17; spawn failure and signals; daemon shutdown/crash survival; restart reconciliation; completion conflicts; and secret absence from records and diagnostics. REMOTE_CI, REMOTE_EXEC and UNKNOWN pass through without resource reservations. Existing protocol-1 diagnostics, bounded admission and shutdown checks remain in the workspace suite. The foreground child remains in the caller's job; PTY/pipeline checks cover Ctrl-Z/fg, INT/QUIT, directed signals and stop/continue behavior.

Renewed independent unfiltered `cargo test --workspace`: 342 passed, 0 failed, 0 ignored. [qa.md](qa.md) and [qa.json](qa.json) retain the requirement matrix, focused lifecycle/fault checks and separately authored product verdict. Chief has not authored or edited that verdict.

## Mandatory commands

These are genuine final-input-bound executions, not restamped earlier passes. Nested root coverage and full `./scripts/harness/validate.sh` executed without skips. Collectors ran serially; live evidence ran after collection. Every wrapper allows at least 3600000 ms.

| Exact command | Exit | Evidence |
| --- | ---: | --- |
| `cargo fmt --all -- --check` | 0 | [receipt](implementation/fmt.json) |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | [receipt](implementation/clippy.json) |
| `cargo test --workspace` | 0 | [receipt](implementation/workspace.json) |
| `bun test scripts/harness/managed-exec.test.ts` | 0 | [receipt](implementation/preservation.json) |
| `bun test scripts/harness/equivalence.test.ts` | 0 | [receipt](implementation/equivalence.json) |
| `bun test scripts/harness/coverage-verification.test.ts` | 0 | [receipt](implementation/coverage-verification.json) |
| `bun test scripts/coverage/clean-checkout.test.ts` | 0 | [receipt](implementation/clean-checkout-final.json) |
| `bun test scripts/coverage/coverage-cli.test.ts` | 0 | [receipt](implementation/coverage-cli.json) |
| `bun run test:coverage` | 0 | [receipt](implementation/coverage-root.json) |
| `bun test scripts/coverage/harness.test.ts` | 0 | [receipt](implementation/harness.json) |
| `./scripts/harness/validate.sh` | 0 | [receipt](implementation/harness-root.json) |
| `bun test scripts/coverage/evidence.test.ts` | 0 | [receipt](implementation/evidence.json) |

Coverage verification retains all nine suites, including clean-checkout, coverage-cli, harness and evidence checks. The isolated checkout collects without relying on prior runtime evidence. The Chief's current audit rehashes each reviewed receipt and its stdout/stderr and verifies executable Cargo identities/counts. The unfiltered final evidence suite has passed as a candidate, and completed artifacts receive frozen replay confirmation.

## Recorded coverage and live evidence

| Crate | Covered / production lines | Coverage |
| --- | ---: | ---: |
| arc-core | 247 / 256 | 96.48% |
| arc-detector | 609 / 610 | 99.84% |
| arc-storage | 653 / 674 | 96.88% |
| arc-ipc | 387 / 393 | 98.47% |
| arc-daemon | 1628 / 1746 | 93.24% |
| arc-cli | 674 / 774 | 87.08% |

Each existing crate retains its 80% floor; no crate is added. Exact measurements above belong only to reviewed recorded collection `2026-11-08T19-15-44-425Z-27759437-90c6-4b49-bd86-e6e011e7f13c`, summary digest `19281d0cebf86cdc88f1398650aee55113932f1bfd96aa13bb336822a54318ac`, production-input hash `6f0fbcb95fb05bfc5133d5255ac9cfa1c9afb79a80c47b8cc8060392cdcb05d8`. Native LCOV/LLVM accounting, complete crate/file inventory, profile digests, merge membership and child PID proof are validated within that collection. The current latest collection is independently validated against current sources and all the same floors. Its covered counts may differ. The latest pointer digest is a recorded observation, not an immutable current-pointer claim.

The evidence-identity repair reproduced the prior cross-run-count failure and added deterministic regression/negative checks through the actual validator. Exact equalities stay within each collection. Altered counts, stale sources, changed summary/raw/profile bytes, missing inventory, unsafe paths, inconsistent identity and a below-floor latest run must fail. No threshold, accounting rule, exclusion, collector or latest pointer was weakened or frozen. Final evidence does not overwrite receipts. Original faulty source, failures and prior reviewed bytes remain in hashed historical archives.

Migration 0001_phase0.sql remains byte-for-byte unchanged (SHA-256 046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384). Additive 0002 remains registered; legacy witness semantics stay covered. Historical Rust preservation is scoped to its original contract; current regression assertions, harness equivalence and coverage protections remain enforced. Maximum reviewed source length: 474 lines.

## Replay and ownership

Chief changes final.md, final-summary.json and receipt-index.json. Backend owns the tooling repair and implementation receipts; reviewer owns renewed QA. There are 142 tracked managed-exec evidence files, below 150. Prior evidence is archived rather than discarded. Raw coverage/profile/command exports stay in ignored local paths. Required tests also regenerate the existing harness-speed equivalence, invocations and coverage-verification reports; these generated outputs remain unstaged outside the assigned evidence edits, with their original index entries preserved.

After final staging, the Chief runs the unfiltered final suite, then the fixed serial coverage-verification, clean-checkout, coverage-cli and harness sequence, then live evidence and the unfiltered final suite again. Frozen final reports, QA, implementation receipts, input identity and index are snapshotted and compared after every command. Actual confirmation outcomes, host/times/output digests and before/after pointer observations are retained at `target/managed-exec/chief/task9-attempt6/completion.json`; this avoids circularly hashing a report's own future execution. Final acceptance depends on that confirmation succeeding. Reports are not rewritten to match later counts.

The first Chief candidate suite in this recovery passed thirteen tests and failed the report literal check because Desktop was capitalized. Its receipt and original report bytes are preserved under the local candidate-report-literal-failure archive. The wording was corrected without changing implementation inputs or QA. Earlier failed attempts remain historical failures. The native lifecycle implementation is separate from its evidence repair. A lost CLI may leave an unknown observed outcome; reconciliation does not infer successful completion. No operator goal, saved assignment, mission graph, roster/model, budget, verification command or independent framework review is changed. Serena is unavailable; inspection uses known-path reads and noninteractive tools. No browser, GUI, Playwright, publication, deployment, push, merge, outreach or spending occurs.
