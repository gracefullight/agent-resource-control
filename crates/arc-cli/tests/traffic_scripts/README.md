# Task-6 resolver and CLI handoff

Status: completed for task-6; current-source workspace verification passed. Owner: backend-execution, task-6 attempt 1. Skill used: oma-backend, including its execution protocol and checklist. This component note accompanies the material source and tests; the assigned integration writer retains ownership of canonical mission evidence.

## Source scope

Accepted source baseline: `5578e69fc63ada7a73190f5681e7960da17969f7`, product tree `888ddd6cbe9d02611b87ed98c8529882d8a0778b`.
Actor input snapshot and inspected local HEAD: `818b3fc41d8be63c1729eae1fbbaa45676f827c7`, input tree `2c103ad31e2fb40ae14a9cf2c916ad4ba9cb893a`. Snapshot ref: `refs/agent-valley/task-evidence/69a59d52a042fc13-98c4ca9fa7207111-1/snapshot`. Worker ref is supplied by AV separately. The task-5 interfaces are present at this Actor snapshot; this note does not place them, or the following task edits, in the accepted committed baseline. The hashes below bind the Actor-local task-6 source/test diff. Integration must inspect the delivered files in its own checkout, using its delivery receipt.

Initial scoped source/index diffs were empty. Unscoped Git status reported `av.yaml: Operation not permitted`; that path was excluded from subsequent status inspection and was never read through another route. No Serena tools were callable. Inspection used the already identified files and exact owned paths. No mission state, saved assignment, acceptance criteria or frozen contract was edited.

## Delivered behavior and compatible interfaces

- The exported `arc_detector::analyze_script_argv` consumes the task-5 `PackageMetadata` port. Its signature, result contracts, finite limits, existing classifiers and shared wire types are unchanged. The maintained parser supplies literal invocations for context traversal, while legacy syntax-only classification retains its original behavior.
- The CLI alone opens package.json files. Fresh invocation-scoped snapshots use SHA-256 content identity, bounded ancestor discovery, regular nonblocking/no-follow reads with one overflow sentinel, aggregate metadata and retained-script budgets, and fresh pre-enqueue revalidation. Revalidation also detects newly created nearer manifests. No persistent metadata cache, executable lookup, shell evaluation or package manager probe enters classification.
- Command contents determine BUILD (Bun/Vite), TEST (Bun), LINT (Biome check), and TYPECHECK (tsc with noEmit). Nested script and lifecycle edges share parser byte/token/invocation/depth/work budgets and the frozen metadata/edge caps. Cycle identity uses canonical package plus script name, independent of changing forwarded args. Same-class compounds collapse to one classification; remote-only graphs remain reservation-free. Dynamic, cyclic, mixed, unknown, unsupported, watch/hot and workspace fan-out forms bypass as observations.
- Repository desktop:build and desktop:typecheck resolve through `bun run --cwd apps/desktop`; both tsc project checks are inspected. Forwarded arguments append, shell-escaped, only at the end of the main body; pre/post hooks receive none and do not recursively acquire their own hooks. Bun removes one delimiter immediately after the script name. A separated --cwd value before run is conservatively unsupported for the installed Bun 1.4.4 behavior; `bun run --cwd ...` and `bun --cwd=... run ...` are supported.
- Original native argv, caller cwd, environment and streams still pass to the existing sole `Command::spawn` owner. Resolved leaves are never substituted or separately spawned. Existing signals, native launch fences, Governor pause/pressure behavior, lineage and Meter ownership are reused.
- Enqueue negotiates only `traffic.bun-scripts.v1`, using the frozen Governor envelope on `traffic.bun-scripts.job`. It retains the original job ID, observer, witness and Continuity after a possible commit. A definite unsupported response before submission projects to the legacy observation vocabulary. A lost ACK, later unsupported response, or ambiguous peer retains uncertainty. No shared manifest, dependency version, IPC codec or capability response was widened.
- The owned negotiated exchange reuses `arc_ipc::client::Session` for authenticated connection/continuity, then uses a duplicate nonblocking/CLOEXEC socket and bounded cancellable framed exchange. This is necessary because Session's release-oriented exchange classifies all frame failures as invalid and the older general client does not recognize new operation negatives. Admission retries closed/truncated ACKs up to three times with the identical Request and a shared absolute deadline (each attempt capped at 600 ms), retaining possible-commit state and peer continuity. Malformed JSON, wrong request IDs and future protocols stop execution. Responsive DB_BUSY retains admission with bounded backoff; it never becomes outage permission. Launch/ACK remain on the unchanged release client. Unreserved attach/complete and resume-after-unreserved can use the existing Governor envelope, which carries no new classification at those boundaries.
- New optional headless read: `arcctl job diagnostics JOB_ID [--json]`. It independently negotiates `traffic.diagnostics.v1`, validates the frozen ID-bound DTO, and displays actual recorded classification/admission, historical queue cause, retained/released permit facts and outcomes. Unsupported/unavailable reads never become absent-record claims; `job status` behavior is preserved. No SQLite connection or extra scheduler is added to the CLI.

## Acceptance evidence

- [x] All four supported classes, misleading script names, same-class chains, explicit/nested cwd, actual desktop scripts and forwarded argument placement have deterministic assertions in script_resolution.rs and metadata/tests.rs.
- [x] Per-file and aggregate bytes, file/discovery/script-retention counts, input/tokens/invocations/depth/work/graph edges, cycles, lifecycle effects, malformed/nonregular metadata, deletion/replacement, nearer-package creation and same-size preserved-mtime changes have regression assertions. Graph-edge guard is independently exercised; the stricter invocation bound ordinarily wins first in leaf-bearing graphs.
- [x] Poison bodies never run during analysis. Unsupported and remote-only CLI graphs send only observation registration, with no admission/capability request. The original fixture invocation still executes once.
- [x] Four-class negotiated CLI cases synchronize an actual owned native child and assert original argument bytes, cwd/environment, stdin/stdout/stderr, exit 17, SIGTERM identity, one start, launch/ACK, same-attempt lifecycle and native Meter report. Healthy lost enqueue ACK and responsive DB_BUSY both retry before the native launch fence; the child has zero starts until admission and release. Unreserved and queued paths are covered.
- [x] Separate real Bun interpreter fixtures compare raw Bun with arcctl output: lifecycle order is pre/main/main/post, hooks receive no args, only the final main command gets literal/empty/quoted arguments, package cwd and env agree, explicit cwd/delimiters agree, and exit 17 is preserved. These tiny interpreter fixtures do not replace real build/tool/production-daemon acceptance.
- [x] Deterministic compatibility tests cover unsupported probe/enqueue, no classification inferred from Governor capability, queued cancellation with zero starts, lost enqueue ACK, unsupported after durable queue evidence, retained witnesses/identity, bounded outage, malformed and truncated replies, request-ID/protocol mismatches, DB_BUSY and cancellation. The ambiguous-peer unit case deliberately injects unidentifiable continuity; production daemon replacement/restart remains downstream acceptance.
- [x] Diagnostic tests round-trip all delivered Rust wire fixtures, preserve recorded-versus-current distinctions, distinguish class/CPU/memory/FIFO queue causes and reject wrong identity/freshness/capability or unavailable reads.

## Verification commands and observed outcomes

Final source verification passed on the bound 24-path product/test input set; its digest remained unchanged throughout the final workspace run. The wrapper allows 7200 seconds per check and executes the exact cargo test --workspace argv. Resource observation before compilation: macOS, eight CPUs, 16 GiB RAM and about 64 GiB available storage. Only one owned Cargo command ran at a time; no new dependencies, tools or browser binaries were installed.

Initial compile-only check `cargo test -p arc-detector -p arc-cli --no-run` passed, then was superseded by completed tests after later source edits. Focused commands used:

- `cargo test -p arc-detector`: 31 tests passed (15 library, 3 argv, 4 shared contract, 9 script resolution), zero failed/ignored.
- `cargo test -p arc-cli --bin arcctl exec::metadata`: six passed, zero failed/ignored.
- `cargo test -p arc-cli --bin arcctl exec::transport::negotiated`: one test with ten failure/continuity subcases passed.
- `cargo test -p arc-cli --test traffic_scripts`: eleven tests passed on the repaired source, zero failed/ignored, including the reservation-free observation and same-Request ACK-retry assertions.
- `cargo fmt -p arc-detector -p arc-cli` formatted only the owned crates. `cargo fmt --all -- --check` passed on the current product sources.
- `cargo clippy --workspace --all-targets -- -D warnings` passed on the current product sources. Existing ts-rs attribute-parser notices remain nonfatal; no lint allowance was added to suppress production warnings.
- `cargo test --workspace`: passed on repaired inputs, exit 0; 1273 passed, zero failed and zero ignored across 74 result blocks, elapsed 449218 ms. Source digest before/after: 6890e0bd48a337f63639bca2c06d7dcd3bf81ad35da72ab551c05db57fed0770. Ignored diagnostic log: target/task6-workspace.log, SHA-256 32a5ccf6895411ff4370941148b28dd431ba2218a547bacd816a8a01ca3471fa. A preceding run passed 1273 tests, zero failed/ignored in 568925 ms; it was superseded by the ACK-retry repair. Its ignored diagnostic log is target/task6-workspace-before-retry.log (SHA-256 624a2078ca6315b75cfce7d4fdc83af389741aecc4525ed833744dd36a8b51ec). Only the subsequent repaired-input result establishes current workspace readiness.

Earlier attempts exposed the lost-ACK frame distinction, a missing required ready_at_ms in a new test fixture, optional Meter expectations on unconfirmed jobs, and a test output-capture setup error. Those were repaired. An intermediate compile reported a test signal type mismatch; two explicit unit expressions failed Clippy and were removed. An initial format check found the new files unformatted; scoped formatting repaired it. No test or acceptance condition was weakened. Final runs, rather than these superseded attempts, establish delivery readiness.

## Remaining integration obligations

The daemon handler/capability/diagnostic read is intentionally owned by the next backend task. Its absence at this snapshot does not authorize a shared-file edit here; the strict negotiated CLI tests use authenticated deterministic peers. Production daemon contention in both BUILD orders, paired TYPECHECK and mixed resource budgets, registered desktop bridge reconciliation, real Bun/Vite/Biome/tsc tool artifacts, recovery/restart and final native gates remain downstream. No coverage collection or 80% floor claim is made here; the exclusive backend gate owner retains that obligation. No traceability row/count is changed before task-9 acceptance evidence arrives. No final mission acceptance boolean, historical receipt or canonical evidence file was rewritten.

Metadata can change after revalidation, including while queued. Recorded classification describes the inspected snapshot; this implementation neither freezes the package filesystem nor rewrites the native invocation. The frozen public interfaces are compatible, and this task needs no out-of-ownership manifest/type change. No blocker remains within task-6 ownership.

Delivery checks: all 25 changed files (24 implementation/tests plus this note) are non-ignored and explicitly staged. Shared manifests, lockfiles, desktop files, frozen contract and canonical integration report have no diff from the Actor input snapshot. No commit, push, mission-state update or external delivery was performed.

## Changed source and test bindings

24 changed implementation/test paths; digest `6890e0bd48a337f63639bca2c06d7dcd3bf81ad35da72ab551c05db57fed0770`. Digest input is sorted `path + NUL + sha256 + newline`. This note is excluded from its own digest.

| Path | SHA-256 |
| --- | --- |
| `crates/arc-cli/src/exec.rs` | `251d6b5444b273d6799a17c8aa089496d9f1edffa94d5659abcb460f64cf71f4` |
| `crates/arc-cli/src/exec/governor.rs` | `798f78e198e457bf1225b45f8df9a2ce8e6a4087beca77fb8406c41f130710d0` |
| `crates/arc-cli/src/exec/launch.rs` | `cefd95192369df58bf9796defcbfb2aa1fb552aaf91d07a4400267ff9b8e078d` |
| `crates/arc-cli/src/exec/launch/governor.rs` | `65f689eb7b6f639362810c2f09cf81a162b0afe9f2552305d10131765505a0c3` |
| `crates/arc-cli/src/exec/metadata.rs` | `156eff9717abebcda278303c2d6df10555dd02fa317acb04e6acd97553c97b30` |
| `crates/arc-cli/src/exec/metadata/tests.rs` | `84d2d0f237338b5851cca1f2feacae1327607d505b4d94f3e0c3115d22dc9057` |
| `crates/arc-cli/src/exec/traffic.rs` | `73e743f3cf208e58804394e696b7ad7e3473429d2438fc16ebe12938efecefd3` |
| `crates/arc-cli/src/exec/traffic/lifecycle.rs` | `7e36ee5442a2ad13ad86a5a9330e5b4905317fc9d156f97b7a1f28590fd4ba51` |
| `crates/arc-cli/src/exec/transport.rs` | `b6b8f51b0052667966445cadc62515d2a00630aded167e4cc959233cc2e27d75` |
| `crates/arc-cli/src/exec/transport/negotiated.rs` | `4440f518ef840b0311dc07e861ae41b39a2b021769faac0c48943e1b671260fe` |
| `crates/arc-cli/src/job.rs` | `171a7a94fa177aa8282b654fca0d0d51d308663756189a511c6587753d91d567` |
| `crates/arc-cli/src/job/diagnostics.rs` | `17c1fe745586efefd6611eb42a1691e516aa6e6327ec357c4073ee89bda966cf` |
| `crates/arc-cli/src/main.rs` | `fffa5c710583b851d4428b749ba181cd46b30ef571d303bae04bb6194bd9b6bd` |
| `crates/arc-cli/tests/traffic_scripts.rs` | `ab61498088df16dca4dd1c23bcfc258c7e57442d180682951750cf42b92da29d` |
| `crates/arc-cli/tests/traffic_scripts/bun_semantics.rs` | `3f6afb9002ce1e34247b7959472e71193a38e2f5aebcb754838fec68b3a25221` |
| `crates/arc-cli/tests/traffic_scripts/compatibility.rs` | `c5d3909a45d4b8db701d03ac1f6f7998f4d264a6a3ccbe5c5de76d87b4a27d9d` |
| `crates/arc-cli/tests/traffic_scripts/diagnostics.rs` | `ff36a0ffeae23c34241fe97a86828aa6457154d1bbfb920850fab522296c70a9` |
| `crates/arc-cli/tests/traffic_scripts/execution.rs` | `2ca44daa23585d52a58dee7ee2fe7f0d5c338521e5228fe06fca8b3fc36a6544` |
| `crates/arc-detector/src/lib.rs` | `df21bcb2adbc2033ca1ac0ca6d3f5dcdf95c87c5ad37e156e0ea206f4f40dd2c` |
| `crates/arc-detector/src/parser.rs` | `b617e319a065bb06d3ce4f3813fe2c4db0ccf1ba10ef92f422e42e78d025e0c0` |
| `crates/arc-detector/src/scripts.rs` | `6f3ad93fc08319d38d1bf62e1fa28884e4b6d708e483b570e0b1f261efbf123c` |
| `crates/arc-detector/src/scripts/commands.rs` | `8d6a6299c802fe893490ead09c7329c87d98c1108178744662253ee8735fbb8e` |
| `crates/arc-detector/src/scripts/resolver.rs` | `b37410acf5f46833b1bc35761ba4dd1821a1fbd738cd9ec2da0e67043e4695cb` |
| `crates/arc-detector/tests/script_resolution.rs` | `b300a393a710af3de9084a3aeb4a1c788845d4aaf1c1cbd471a643fe238ce969` |
