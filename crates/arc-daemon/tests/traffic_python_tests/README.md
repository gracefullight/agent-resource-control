# Task 3: native Python TEST handoff

Status: completed for task-3, backend-native / Python Execution Engineer, using the installed backend-engineer role and oma-backend. This report describes local macOS fixture execution. Registered desktop execution, final integration gates/coverage and independent QA belong to the downstream assigned tasks.

## Source and ownership

- Accepted committed baseline: `6b94c0bd3b309ae7b5d7ce4c3ebef50b62b5e85c`; verified tree `73ae372dc66a88b5a5a9488b8379e13ca9ac657a`.
- Verified initial snapshot commit / current local HEAD: `ec4b42a7211cf077f7f110a24ca62b716fc58362`; snapshot and current HEAD tree `7c3d113dd0fd53bdeeeddc300f3fe0b90e0a51f9`. This snapshot includes the supplied task-2 overlay; that overlay and these new task-3 working-tree edits are not claimed to exist in the accepted committed baseline.
- Provided snapshot ref `refs/agent-valley/task-evidence/04608e0c24b60700-1724980aa084bbaa-1/snapshot` and worker ref `refs/agent-valley/task-evidence/04608e0c24b60700-1724980aa084bbaa-1/worker` both returned `fatal: Needed a single revision` in this isolated worker. The commit objects/tree above were verified directly; no revision was substituted or ref created. Task-3 edits remain an uncommitted, Git-visible working-tree delta for AV collection/integration.
- Branch: `av-task/04608e0c24b60700-1724980aa084bbaa-1`. Mission `0cf2ede2-1fe4-4ab1-88c3-c050e770f179`, task `task-3`, attempt 1. Native Codex thread `01a1dc5f-6dc6-7270-b157-d0d80e44d807`; no AV run ID/claim path was exposed to this worker, so none is invented here.
- Owned additions: `crates/arc-daemon/tests/traffic_python_tests.rs` and this directory, including self-contained source fixtures and interpreter readiness. The sole existing-file change is `../traffic_bun_scripts/mod.rs`, a shared native test-support adaptation. Its default `Case`/`Run` callers retain source compatibility; focused Clippy compiled the Python, Bun-script and managed-execution targets.
- Production source, generated contracts, manifests/locks, the frozen contract, mission state, predecessor evidence and harness were not changed. A temporary addition to `managed_exec/native_python.rs` was moved wholly into this suite's `runtime.rs`; the original file has no final diff. No ownership handoff for production repairs was needed.

## Shared interface for task-4

`mod.rs` provides the frozen API:

```rust
pub type Read = fn(&std::path::Path, &str, serde_json::Value) -> serde_json::Value;
pub enum Peer { Cargo, Bun }
pub fn semantics(read: Read);
pub fn lifecycle(read: Read);
pub fn one_test_slot(read: Read, peer: Peer, python_first: bool);
pub fn direct_read(socket: &std::path::Path, command: &str, args: serde_json::Value) -> serde_json::Value;
```

`true` means Python first. The callback receives `traffic_job_diagnostics` or `meter_summary`, with `{"job_id": id}`, and returns the existing command's data payload. Task-4 should import this exact module alongside the existing root `cleanup`, `native` and `serialization` modules, then supply its registered Tauri router callback, as the predecessor Bun-script target does. Each scenario already checks the same IDs against read-only SQLite, actual `arcctl job diagnostics/status`, Meter CLI and the supplied callback. This run supplied `direct_read` (negotiated production IPC), not the registered desktop callback.

The common `Case<T = projects::Tools>` adds `RunnerTools::shims()` and `with_tools(...)`; it reuses the existing owned fixture, process identity checks, cleanup, persistence and reconciliation. Existing supported-admission assertions remain intact. OBSERVE snapshots additionally require both traffic state and permit state to be absent when admission is null.

## Runtime and fixture facts

The default PATH `python3` could not import pytest (`ModuleNotFoundError`). Fixture readiness instead reads the genuine installed `/opt/homebrew/bin/pytest` script's shebang, verifies the `pytest:console_main` distribution entry point, matching console/module version, and interpreter identity. Observed Python: 3.13.18, interpreter `/opt/homebrew/opt/python@3.13/bin/python3.13`; pytest: 8.4.1 at `/opt/homebrew/lib/python3.13/site-packages/pytest/__init__.py`. Bun 1.4.4 and Cargo 1.95.0 are checked by the fixtures. No downloads or package installation occurred, and missing prerequisites fail rather than skip. `ARC_TEST_PYTEST`/`ARC_TEST_PYTHON` may select an already-installed matching pair; they cannot bypass readiness checks.

Homebrew Python's native running image differs from its launcher: `/opt/homebrew/Cellar/python@3.13/3.13.18/Frameworks/Python.framework/Versions/3.13/Resources/Python.app/Contents/MacOS/Python`. Readiness records that native image independently; the body separately records `sys.executable`, `sys.orig_argv`, `sys.argv`, prefixes and import path. Classification still uses task-2's pure bounded recognizer and never performs these fixture probes.

Every project is created independently from the Git-visible `sources/` files. Cargo peers run `cargo test --offline -- --nocapture`, Bun peers run actual `bun test`, and the separate global-budget case intentionally runs BUILD against TEST. Virtual environments use `-m venv --without-pip --system-site-packages`. Tests exercise relative/absolute venv Python paths with spaces/Unicode, plus a lexical venv alias to the unchanged genuine pytest console script, preserving that script's original shebang/interpreter choice. Exec observers only log and `exec` the verified real tool with unchanged arguments; pytest is not simulated.

Fixture-only plugin/configuration controls apply equally to direct and ARC baselines. Each project supplies `pytest.ini`; external pytest autoload/options are isolated. Quiet pytest output and a fixture-only unittest duration clock make complete native stdout/stderr comparisons deterministic. Real monotonic release watchdogs, production deadlines, signals and ARC accounting clocks are unchanged. Stdin contains NUL/Unicode bytes. Inherited ARC coverage variables (`LLVM_PROFILE_FILE`, Rust flags and related instrumentation) are retained; coverage itself was reserved for the final gate task.

## Executed check receipts

Cwd for checks: `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/0cf2ede2-1fe4-4ab1-88c3-c050e770f179/workspace/.agent-valley/parallel/workspaces/04608e0c24b60700-1724980aa084bbaa-1`. Cargo checks use `CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2`. The final subprocess wrappers allow `7200000` ms (7200 seconds), matching the required per-check timeout. The host probe observed 8 logical CPUs, 16 GiB RAM and about 75 GiB available disk; compiler parallelism was limited to two. Tests retain the existing native serialization/owned-process support.

| Check | Result |
| --- | --- |
| `cargo build -p arc-cli -p arc-daemon --bins` | Exit 0; actual CLI, daemon and pressure-fixture binaries built offline; 14.82 s |
| `cargo test -p arc-daemon --test traffic_python_tests --no-run` | Exit 0; native suite compiled |
| `cargo fmt --all` | Exit 0 after final source edits |
| `cargo clippy -p arc-daemon --test traffic_python_tests --test traffic_bun_scripts --test managed_exec -- -D warnings` | Exit 0; final run 2026-11-14 23:46:00.483838 UTC to 23:46:01.415655 UTC |
| `cargo test -p arc-daemon --test traffic_python_tests -- --nocapture --test-threads=1` | Exit 0; final run 2026-11-14 23:46:08.105571 UTC to 23:46:47.588901 UTC; **7 passed, 0 failed, 0 ignored**, 38.11 s test time |
| `git diff --check -- crates` | Exit 0 |

Final native stdout/stderr log SHA-256: `0fd8edeba2935a84fbbfd4fb892884fdec4abe86cc7bd7e2a5c82f94e173f6e7`. Final focused Clippy log SHA-256: `dafd9fb7aad061bb8248141ebf8942e01a79bd0243eccf263a0dba063254dc08`. Logs/large observations in ignored `target/` are local diagnostics; the required handoff code, facts, IDs and source bindings are retained in this Git-visible report. These hashes are provenance, not a claim that ignored files will be transported.

Executed binaries:

| Binary | SHA-256 |
| --- | --- |
| `arcctl` | `00a91e02f31ffb9d3ccadafad491d63f7d75a9be68bc678cdc7726faaaf30d6d` |
| `arc-daemon` | `2d71a414c67e15778cf26a94dcdb6ff62c7d4a5cd069fa7804219168867f4b7c` |
| `arc-pressure-daemon-fixture` | `c357bb7e289dc458779cc40833073215a1221e13de1cf09c228ff37de0721395` |
| `traffic_python_tests-31fd08f066a19717` | `e5ed0f3dccbc91b939d288b421092087d788bb9def52502ea29769a46f5d778c` |

## Acceptance evidence

- [x] Both required native entry tests run real matching Python/pytest fixtures. `native_python_execution_semantics` compares exact full stdout/stderr, native exit 0/1/17, stdin, cwd, environment, executable, argv, module/import path and venv prefix behavior across 18 admitted cases. A nineteenth real native comparison, `--strict-markers`, stays UNKNOWN/OBSERVE with no traffic entry/reservation, executes once and retains its measured exit.
- [x] Four synchronized Python/Cargo and Python/Bun TEST pairs execute real test bodies in both orders. First-body entry and explicit release hold the first command. Durable queued state and the queue notice precede zero-launch/body/no-child assertions for the second. A pre-exec SQLite witness additionally requires the first permit to be RELEASED; all persisted grant/release intervals are reconstructed. Every TEST permit high-water is one; each command and body executes once; every permit releases. Queued Meter is explicitly `unknown/not_terminated`, never measured zero.
- [x] A distinct BUILD/TEST scenario proves reuse of the global CPU budget with spare TEST class capacity: first reservation 2000 millicores, queued Python wait `cpu_budget`. TEST pairs reserve only 1000 millicores and 1 GiB each with spare global budget, proving their `class_window` wait is genuinely the shared TEST window.
- [x] `native_python_lifecycle_and_compatibility` covers fresh injected Critical pressure, Governor pause releasing queued work unreserved, OFF persistence across restart, resume/new reserved unittest work, queued cancellation before spawn, running/queued recovery after an owned crash, native/forwarded SIGTERM, native/forwarded pytest SIGINT (exit 2 and exact streams), nested verified ancestry/permit reuse and canonical/included Meter lineage.
- [x] Authenticated raw-client loss checks discard committed enqueue, actual Governor launch-start, attachment and completion acknowledgments. They replay metadata with the same job/generation, execute a real Python body once and release the only permit. The start response is observed with non-consuming `MSG_PEEK` then discarded. The raw client has no CLI wait4 report and explicitly reconciles `unknown/unavailable` Meter across CLI/callback. Protocol-1 lifecycle/diagnostic reads project UNKNOWN and null admission while SQLite retains PYTHON_PYTEST.
- [x] Explicit synthetic unsupported-capability and hung protocol-1 peers plus an absent endpoint exercise fail-open without duplicate execution or production DB writes. They are not historical daemon binaries or real-daemon admission evidence. Exact absolute deadline/fallback checks are consumed from task-2's accepted Python-capable `fallback_routes_keep_the_exact_original_deadline_even_when_already_expired` test; native elapsed values below are observations rather than substitute clock proofs.
- [x] All 36 recorded permits across the scenarios released; the 12 final owned daemon fixture teardowns passed the existing native shutdown assertions, including deadline met, gauges zero, worker joined and socket removed. Crash recovery reaped/verified the owned crashed daemon before replacement. Native children are identity-bound and owned cleanup/reaping precedes fixture cleanup. No production permit rows were manually repaired or mutated by this suite.
- [x] Frozen callback API and source files are non-ignored, reusable handoff files. No manifest, lockfile, generated DTO, scheduler, SQLite writer or harness fork was added.

### Synchronized job receipts

All rows reconcile classification, outcomes, reservation/generation/root facts and Meter through SQLite/CLI/callback. Each first/second body count is `[1, 1]`, and all final active counts are zero.

| Scenario | First job | Second job | Second starts before release | Command starts | TEST high-water | Released permits | Queue reason |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Pytest-then-Cargo | `4cee665bc7bc8dc1740015384ea6063a` | `ed9e0f730f5cd2ba7ccba5d0453010e3` | 0 | [1, 1] | 1 | 2 | `class_window` |
| Cargo-then-Pytest | `d07a29d743f932f6fdf0a6a7f79e4b66` | `a779c9f13d3b744c6a7bee04ff73ab6d` | 0 | [1, 1] | 1 | 2 | `class_window` |
| Pytest-then-Bun | `8e4641b1688b3b9c931a8124d1b64aa0` | `e37aa918e634539c60502f0dbda3d9fb` | 0 | [1, 1] | 1 | 2 | `class_window` |
| Bun-then-Pytest | `cb58409b749da43a0f788277ef91e145` | `29f7809ede2dff1cbc496e58a30e882d` | 0 | [1, 1] | 1 | 2 | `class_window` |
| global-cpu-budget | `d4224c033c4a875d5f198d68b4574994` | `a01bab7024145040389d0166ea800e13` | 0 | [1, 1] | 1 | 2 | `cpu_budget` |

### Execution-parity job receipts

| Case | Managed job ID | Native/wrapped exit |
| --- | --- | --- |
| pytest-console-discovery | `53da54c99834f84d7281ef771dfbc777` | 0 |
| pytest-module-discovery | `95a0118c3f456ea4b089ed20f8a0634f` | 0 |
| pytest-file-node | `4ea5a8f0586114c14ca3a04bef134cce` | 0 |
| pytest-core-selectors | `bc7e12cbe25268f08199d92655e6ad4d` | 0 |
| unittest-default | `876401aa9cf555f1456c7fbf268f833a` | 0 |
| unittest-discover | `d2de735e6681e20e2711c56e54979142` | 0 |
| unittest-discover-selectors | `f3716dd68bf86b6c805b8598615362f4` | 0 |
| unittest-dotted-selector | `65e5fcf1ec6d5c6c2ba7c6e3c3958493` | 0 |
| unittest-file-selector | `0c894bc84bb41e44387707f90d26f0fb` | 0 |
| Pytest-failure | `0b1bb78967129f19d8021e38024a7d11` | 1 |
| Pytest-stdin | `4f1dd8158601078ecbd317b2d31e023c` | 0 |
| Unittest-failure | `9c82787d8906f0ccdbcde3a0adcdfcfa` | 1 |
| Unittest-stdin | `2b385268dbea9ad7dbae9c17ae068740` | 0 |
| Pytest-exit17 | `1e80f8022bacf0a1cba740c81a1b5191` | 17 |
| Unittest-exit17 | `bbf2c396db9a3613c393e43c161c13dc` | 17 |
| venv-console-alias | `8f83bb02f6366a753b3f44e1243b5407` | 0 |
| venv-pytest | `51f60431755dbd2b8543db24a410b9ce` | 0 |
| venv-unittest | `afab0a3487b4f35744ebd604589723eb` | 0 |
| unresolved native pytest option | `e3aa6aa1027df6f388b7438f1199ff3f` | 0; OBSERVE, no permit |

### Recovery and uncertainty receipts

- Fresh Critical: sequence `2`, available pressure source explicitly injected into the production Meter worker. Python job `a8d448c5b1bfa330bb97a768b841b3b4` stayed at zero starts until pause, then ran with zero permits; resumed unittest `a1a1a20fc1f16cc6543b14d26d5a3800` used one released permit. OFF persisted across the controlled restart.
- Crash/cancellation: running `8f527573dc4ec17b927af1103bc0e383`, cancelled `e1da388765065ecd2bc5dea789efca0d`, queued survivor `e353820fe244b3bc43f282f20b122c1c`. Cancelled command/body starts are zero. Fresh status after wrapper reaping yields `unknown/interrupted` Meter for cancellation. The survivor keeps the same ID, executes once and releases its permit; because it met two daemon identities before attachment, Meter correctly remains `unknown/unavailable` with TRACKING_UNCERTAIN, while stored native exit 0 is preserved. No accounting success/zero is fabricated.
- Signals: forwarded SIGTERM `c29b06870d1c26550d3f76faef731298`, child-originated SIGTERM `9ac427ff4c35ed7815a09ee1bfd76dec`, pytest SIGINT `9e1a9ad8145a53ba2026a9c63162b5cf`. Native/wrapped SIGTERM is 15; pytest SIGINT is native exit 2, with exact full output parity. All three permits release.
- Nested: root `888c1914f38949145ab8027ece1dc63d`, child `ef210558db876eb1170b89091407ab0a`. One shared permit remains active while the child is complete and the parent lives, then releases with the parent. Child Meter is `included_in_root`; parent is `canonical_root`.
- Lost ACKs: job `1234567890abcdef1234567890abcdef`, one original command start/body, one released permit, RESERVED real Governor launch. Legacy projections change only read vocabulary; stored reason remains PYTHON_PYTEST. Raw-client Meter remains explicitly unknown.
- Synthetic compatibility/outage: unsupported peer starts the body in 280 ms and hung peer in 2045 ms in this run, each once with exit 0. Absent-socket unittest also exits 0 once. Real fixture DB stays empty; these elapsed observations include native overhead and are not asserted as exact production deadlines.

## Failed attempts and repairs

All failures below were repaired in the delivered files; they are not passing gate evidence.

1. Initial full native run: exit 101, 2 passed/5 failed. The fixture assumed Homebrew's launcher path equalled its native image. Readiness now records the image; body facts still check unchanged interpreter/argv/import semantics. The repaired four-pair focus passed 4/4 (9.38 s).
2. Initial lifecycle focus: exit 101 because it expected recorded Meter after a queued client changed daemon identity. The existing uncertainty rule intentionally withholds accounting; the test now explicitly reconciles unknown state while preserving exactly-once execution, stored exit and permit release.
3. A trial transport proxy failed compilation first (wrong `read_message` generic arity), then failed with `INVALID_REQUEST`: proxying changes the authenticated peer PID. The proxy was removed; authentication was not relaxed and request identities were never rewritten. The delivered loss test uses the actual authenticated raw observer and real launch fence, following the maintained reliability seam.
4. First focused Clippy: exit 101 for unused pytest readiness in unrelated managed-execution targets. Readiness was moved to owned `runtime.rs`; the original shared `native_python.rs` is unchanged. Subsequent focused Clippy passed, including the final delivery run.
5. Additional queued/cancelled Meter assertions initially expected `unknown/unavailable` everywhere (full run exit 101, 2 passed/5 failed; focused cancellation exit 101). Live queued Meter is `unknown/not_terminated`; fresh status after cancellation/reaping is `unknown/interrupted`. The final suite asserts these distinct real states and passes all seven tests. These are fixture expectation corrections, not production behavior changes or relaxed acceptance.
6. The supplied snapshot/worker symbolic refs were absent in this isolated checkout (`git rev-parse --verify` exit 128). Actual supplied commit objects and trees were verified directly and reported with their precise scope.

## Source bindings for the passing native inputs

These hashes cover the exact native sources/support at the final passing focused run. This Markdown report was written afterward and is not included in its own hash table. Downstream integration must verify the Git-visible delta/current source hashes and bind its final broader gates after integration.

| Path | SHA-256 |
| --- | --- |
| `crates/arc-daemon/tests/traffic_bun_scripts/mod.rs` | `12641e8009566fc4e409d6a8a4d02801fb79ed7e6d614ccbfe3568ec1da928be` |
| `crates/arc-daemon/tests/traffic_python_tests.rs` | `86c9851d176d2df186afe7a0deb9b33f738cd235ab939d2e320dba854ace452a` |
| `crates/arc-daemon/tests/traffic_python_tests/contention.rs` | `313d277c39bea8a19ea93b39b638283d1b408350eba9fb3dbe6a70f2bd7a536e` |
| `crates/arc-daemon/tests/traffic_python_tests/execution.rs` | `ddc90af5db47172774ee49ab0128545cc9400a076d8b3b502c261db237b8b79c` |
| `crates/arc-daemon/tests/traffic_python_tests/faults.rs` | `3b8fe1025bb258812efa75a622bb8590bf77767a59fa44338231d353e72f25b9` |
| `crates/arc-daemon/tests/traffic_python_tests/mod.rs` | `e46f9ef498b9ce4aec4eee9680894adace7b4309b6c53e5287ec9538f0202c4a` |
| `crates/arc-daemon/tests/traffic_python_tests/processes.rs` | `c1d1cde574b28b294699a0cf605b770e4796f1547645c8adcb502ee405a8d6bd` |
| `crates/arc-daemon/tests/traffic_python_tests/projects.rs` | `dd985fdbb661f2544f146370dd673bc586ce359c965bfdf2dd195a245f5fbf75` |
| `crates/arc-daemon/tests/traffic_python_tests/recovery.rs` | `e64644e474fd2bf5ec76e9fde4c0d035cc6f481fbd80686f95e3263480c759f1` |
| `crates/arc-daemon/tests/traffic_python_tests/runtime.rs` | `3d8848000550998a0d42530d4986053099622e04fd71c3ba7e9e5af34f27f083` |
| `crates/arc-daemon/tests/traffic_python_tests/sources/build.rs.txt` | `2f6b2f2ada0ef3ae756abad86a674857737a92c57ce541a8d8d66e35b06253bd` |
| `crates/arc-daemon/tests/traffic_python_tests/sources/evidence.test.ts` | `ce34a5531e3c0b54c922bd269487c10cd9471462647d325e4f55f31ce63e6aab` |
| `crates/arc-daemon/tests/traffic_python_tests/sources/lib.rs.txt` | `6b91ace6f1b16d055dbd4a96b7fbcb9ebe19b28b31087b32f387096b5afd0784` |
| `crates/arc-daemon/tests/traffic_python_tests/sources/test_fixture.py` | `164da4e3164bd77f9cd4869624afe6e506bd6b410522aca88deff7da306a7f64` |

## Remaining assigned work and limits

Task-3 has no unresolved implementation or native-test failure. Task-4 must execute this same suite through the registered desktop callback and finish its typed/desktop checks. The exclusive backend gate task owns final documentation/traceability, AV integration receipt consumption, all six unchanged mandated gates, desktop gates and the single final coverage collection/harness reuse. Independent oma-qa follows that gate task. No coverage run, browser/GUI check, publication or external delivery was performed here. Neither these checks nor the supplied predecessor receipts certify the whole mission or broader DET-003/TRF-001 language/vendor scope.

Serena was configured but no applicable Serena tool was available. Inspection used supplied/located paths, scoped file reads, Git metadata and searches confined to ignored diagnostic logs or outside-project memory; no project-wide native source-search substitute was used. The denied `av.yaml` was left untouched. Skills used: oma-backend and its execution/checklist/code-intelligence/result-contract references. The `.agents/results/` human report is an ignored local mirror; this README is the canonical Git-visible handoff so no required evidence depends solely on ignored output.
