# Fresh task-5 acceptance evidence

Status: completed task-5 checks; independent task-5 review, integrated task-6 QA, task-7 Chief assessment and final managed verification remain downstream. This report does not establish mission acceptance.

Accepted HEAD: `aacb3b4359f13c31c0705c0cffc5bf1ac00f387e`. Historical accounting baseline: `fbe76a5f15101ad415f355ef06f1deeb1f97d568`. Skill used: `oma-dev-workflow` directly through the installed architecture-reviewer profile. Exact assignment commands take precedence over mise preference. No implementation, protected file, threshold, verification command, task definition or mission state changed.

## Historical and Fresh evidence

Retained rust-coverage receipts are Historical and retain their original source identities. Task-2 preliminary collection is separate. Fresh task-5 verification uses integrated tree `6903568319eae73e7518909e54d410379d94dbc1` and input hash `260b6772a9d1fa3baf329befb8d957b3ed80f58116b6be8bb973372367c169fd` in `/var/folders/r9/0qvpcx794n385zwgzwfvt55h0000gn/T/arc-final-coverage-g9ku0kyt`. Its recorded physical precollection inventory found no inherited run, profile, target, native export or runtime latest-pointer artifacts. The nested clean-checkout test independently collected and verified inside its own checkout. Installed toolchains and dependency caches were reused; native exports were never copied between checkouts.

Recovery 6 rechecked the complete input maps in the receiving and retained checkouts, all four accepted task-3/task-4 delivery hashes and index bytes, and the hashes of all prior command streams before admitting work. Only the five task-5 reports differ from the integrated collection tree. Successful same-source receipts remain valid; they were not rerun solely to replace their timestamps. The two failed 120-second harness attempts remain unchanged as historical failed attempts in commands.json and ignored logs.

## Operator timeout clarification and actual results

The operator note recorded at 2026-11-08T03:32:24.730Z confirms that every managed verification-contract command now runs under the operator per-check allowance of 1800 seconds. Saved timeoutMs values of 120000 are planning estimates no longer applied, and no contract change is needed or possible. This supersedes recovery 5's unavailable-maintenance prerequisite. No mission-state or contract file was changed.

Recovery 6 ran the exact harness-wrapper and standalone harness commands serially with 1020 seconds each, capped by the unchanged saved deadline. No skip flag, alternate PATH, reduced scan, trace substitute, shim or cached harness result was used. Both completed successfully. Live evidence.test.ts ran after the final coverage collection and passed. commands.json retains argv, cwd, timestamps, elapsed times, exits, host/tools, input maps, actual command stream hashes and wrapper child binding/command receipt references.

| Exact command | Exit | Elapsed | Receipt origin |
| --- | ---: | ---: | --- |
| `bun run test:coverage` | 0 | 84.974 s | Original same-source batch |
| `bun test scripts/coverage/gate.test.ts scripts/coverage/accounting.test.ts scripts/coverage/preservation.test.ts` | 0 | 1.941 s | Original same-source batch |
| `bun test scripts/coverage/hygiene.test.ts` | 0 | 1.087 s | Original same-source batch |
| `bun test scripts/coverage/hygiene-preservation.test.ts` | 0 | 4.199 s | Original same-source batch |
| `bun test scripts/coverage/clean-checkout.test.ts` | 0 | 87.764 s | Original same-source batch |
| `bun test scripts/coverage/coverage-cli.test.ts` | 0 | 83.404 s | Original same-source batch |
| `bun test scripts/coverage/harness.test.ts` | 0 | 234.146 s | Recovery 6 |
| `cargo fmt --all -- --check` | 0 | 0.311 s | Original same-source batch |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 10.7 s | Original same-source batch |
| `cargo test --workspace` | 0 | 76.109 s | Original same-source batch |
| `./scripts/harness/validate.sh` | 0 | 211.329 s | Recovery 6 |
| `bun test scripts/coverage/evidence.test.ts` | 0 | 2.696 s | Recovery 6 |

All required commands have actual successful receipts bound to delivered inputs. The original failed harness results were exit -15 after 120.016 and 120.007 seconds; those remain failures and are not counted as passes. New completed runs provide the missing acceptance evidence. The wrapper's actual child `./scripts/harness/validate.sh` receipt and binding both report exit 0 and identical before/after source hashes. Its output asserts zero violations, coverage thresholds met and six PASS crate labels. Detailed streams and receipts remain in ignored runs/ paths in the receiving or collecting checkout.

Pinned Bun 1.4.4, Rust 1.95.0, cargo-llvm-cov 0.6.21 and LLVM 22.1.2 were verified once before the original batch; locked offline dependency preparation passed. Current tool paths match those receipts. Toolchain, dependencies and PATH are unchanged. Environment: SKIP_COVERAGE unset, CARGO_NET_OFFLINE=true, CARGO_TERM_COLOR=never. All nine coverage test files passed, including negative gate/accounting/preservation/ignore checks, renamed-export detection, fresh clean-checkout collection and real CLI/harness children.

| Crate | Covered/total production lines | Verdict |
| --- | ---: | --- |
| arc-core | 64/64 | PASS |
| arc-detector | 577/578 | PASS |
| arc-storage | 226/227 | PASS |
| arc-ipc | 215/215 | PASS |
| arc-daemon | 1507/1613 | PASS |
| arc-cli | 203/213 | PASS |

Every crate independently exceeds 80%. Final latest run: `2026-11-08T03-39-01-852Z-54eb07ec-10c6-4817-8cd9-f9b44bd88848`. Live verification recomputes native LCOV/LLVM JSON accounting, source hashes, profile bytes, merge membership and explicit child-PID profile inclusion. It ran after the last collection in the main checkout; the nested clean-checkout test likewise collected and verified locally. commands.json records complete native-command references, summary/uncovered/native/profile hashes and child proof. Historical incomplete runs and wrapper directories remain listed as historical diagnostics.

## Accounting and preservation

before-after.json uses task-1's Git method: count each tracked blob path and sum logical uncompressed Git blob bytes. Accepted rust-coverage: 927 files / 260548113 bytes; current index: 24 files / 475720 bytes. All 15 new evidence-hygiene files remain below 150. The exhaustive path/content audit covers every indexed .arc-verify blob and finds zero prohibited artifacts, including renamed LLVM JSON exports. Ignore checks retain positive and negative cases. This is current-tree accounting, not clone-size or financial savings.

Recovery 6 freshly rehashed all 84 protected files against accepted HEAD, including 82 Rust files, the migration and unchanged harness, with exact path/byte/mode equality. Receiving, retained and nested checkout preservation checks passed. The original manifest is unchanged; all 456 raw files (252272711 bytes) and all 908 removed files (260184384 bytes) remain locally intact. Both required baseline commit/tree objects remain accessible. preservation.json and local-preservation.json reference ignored inventories; no local raw file or Git history was removed.

## Execution limits and handoff

Saved start: 2026-11-08T02:07:29.971Z; absolute deadline: 2026-11-09T02:07:29.971000+00:00. Concurrency 3; max runs 200; max retries 3; retry delay 1000 ms. Recovery 6 admission observed 37 runs started and 0 retries. Saved budgets and prior attempts were preserved. Collection-dependent checks ran serially. All new Popen roots exited and were waited successfully. Descendant absence remains unverified after the earlier ps denial; no bypass was attempted.

Changed paths: commands.json, fresh.md, before-after.json, preservation.json and local-preservation.json. Ignored artifacts: recovery-6 runner, logs, command receipts, prior report copies, preservation inventories and final index checkpoint. Existing source/runtime interfaces and accepted tasks 1–4 remain unchanged. Return this material deliverable to the existing managed reviewer using oma-qa and the supervisor's actual JSON schema. Independent review is not supplied by this Actor. Task-6 integrated QA, task-7 Chief assessment and final framework verification remain required downstream.
