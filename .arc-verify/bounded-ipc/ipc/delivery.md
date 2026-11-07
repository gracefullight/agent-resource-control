# Task-2 transport evidence delivery, attempt 6

Status: task-2 evidence repair prepared for normal collection and independent review. The three fresh Cargo gates pass. No product input changed. Receiving-checkout validation remains pending; this is not mission completion.

Identity: backend / IPC Engineer; task-2 attempt 6; mission `b137e253-def6-46ef-a75d-25ab414784ea`; Codex session `01a1b6d2-c4fb-7d21-902f-39ea6d5490f3`. The supplied sourceEvidence describes attempt 5. No OMA run ID or claim path was injected; their absence is recorded rather than replaced with an invented identity. No new mission, agent, run, commit or evidence ref was created. Saved budgets and assignments were preserved.

Skill: oma-backend, with its execution protocol/checklist, shared execution/result contracts and code-intelligence policy. The existing workspace is Rust/Tokio/rusqlite. REST/JWT/ORM defaults do not replace its same-user Unix IPC design. Serena tools were not available; inspection used known-path reads and read-only Git metadata, without native source discovery.

## Changed paths and preservation

Changes are confined to `.arc-verify/bounded-ipc/ipc/delivery.md`, `evidence.json` and `attempt-6/`. The original Actor report remains at `original-delivery.md`, SHA-256 `5fea11c0be36072eb0a5dbf7791997a403fca936c0fef0a2fc0a9fc942c020cb`. Its provenance, original implementation, failed attempts, handoff and limitations are retained without rewriting them. The preceding root delivery/index are copied byte-for-byte to `attempt-6/preceding-root-*`. Every other preceding IPC artifact matched its pre-run hash at finalization. The interrupted attempt-5 files were preserved.

The fresh source manifest covers crates, manifests, lockfile, toolchain, harness and contract/targets documents. Its fingerprint is `38cf2940ebc9914d3d3bf74c83f57c3b4e9b093d97f3358713df298721290a91` before/after each gate and at finalization. `provenance-after.json` and each command receipt record the preservation boundary. No database, main/server, IPC, CLI, manifest or lockfile edit was made by this attempt.

Eight of the nine transport files match the original task-2 hashes. The inherited `connection.rs` hash is `560c0cd1728d50ac507fc42506bcd859941b6d46f706ab3276676add18f66244`; original task-2 hash was `90c31e454a00b698dd1390f6dbe18edc9211f27a34b3d5848e43602142ed2195`. Its temporary local busy constant was already replaced by `pub(crate) use arc_ipc::ERROR_DB_BUSY;` in later integration. All nine match this input snapshot and the observed review overlay. They were not rolled back to fit stale recovery wording.

## Provenance scopes

| Inspected scope | Commit | Tree |
| --- | --- | --- |
| Accepted baseline; review HEAD | `42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0` | `c802f71ac4d91ed6b8a602d5c83bf9cf57116b76` |
| Original task-2 snapshot/worker in review repository | `3816bafaa684c8099dad78be2d0cb0e474c5f08b` | `e79ff4ed85c94059275bbb4992579a2d55f51eb3` |
| Supplied attempt-5 input snapshot; current Actor HEAD | `d2004475a01cb395753340977cc9b1b29003f6cd` | `2ee8c1b555aceeaed149c7315e4feb32a8e79e5e` |

`attempt-6/provenance-before.json` retains exact argv/cwd/output/status for tree, ancestry, ls-tree and dirty-state inspections in both repositories. Accepted HEAD is an ancestor of both snapshots. Original task-2 files were delivered as uncommitted Actor edits; they are absent from the original snapshot except for the earlier platform implementation. They are tracked inputs in the current Actor snapshot but remain a dirty/untracked overlay in the review checkout at accepted HEAD. Actor-local tracking does not establish review tracking or inclusion in the committed baseline.

Original `...-1/snapshot` and `...-1/worker` refs resolve to `3816baf...` in the review repository. Supplied `...-5/snapshot` resolves there to `d200447...`; `...-5/worker` is absent before collection. These refs are absent in the isolated clone, with exit-128 receipts retained. No missing revision was substituted with another checkout's HEAD, and no refs were changed.

Current migration bytes equal `git show` of accepted HEAD, SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. The harness is also byte-identical, SHA-256 `93140173df8b8bd28b067aff68301134b0398b643240b0b1e6a13da4487ebe33`. Comparison receipts are retained; this repair did not rerun the harness.

## Exact serial verification

| Exact command | Exit | Elapsed | Receipt and full combined stdout/stderr |
| --- | ---: | ---: | --- |
| `cargo fmt --all -- --check` | 0 | 0.336s | `attempt-6/fmt.{json,txt}` |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 9.014s | `attempt-6/clippy.{json,txt}` |
| `cargo test --workspace` | 0 | 44.729s | `attempt-6/workspace-test.{json,txt}` |

Commands ran serially in the mission Actor worktree with Rust/Cargo 1.95.0, native aarch64-apple-darwin, `CARGO_NET_OFFLINE=true` and the normal worktree-local target directory. `attempt-6/execution-context.json` retains toolchain, cache paths, relevant environment, host limits and inherited mission limits. Each receipt records exact argv/cwd, UTC interval, PID, observed termination, output hash and pre/post source fingerprints. No database Actor receipt was reused. No standalone build, dependency install/download, browser, Playwright, GUI or screen check ran. Test compilation was part of the requested Cargo gates.

The workspace run reports 173 passing invocations, zero failures/ignored tests, including every one of the 18 existing transport_unit tests. These are invocation counts across shared-source targets. `attempt-6/test-summary.json` retains the full transport section and suite summaries. Production modules already present in this snapshot were visible to Clippy and workspace tests. The fresh run also executed existing bounded subprocess/CLI, worker-runtime, resident, startup, storage and protocol targets; their independent task acceptance is not reassigned to this Actor.

## Task-2 acceptance evidence

- [x] Admission and accounting: `retained_tasks_reap_and_rejections_do_not_call_factory` covers completed entries, reaping, cap rejection and no rejected factory call. `request_admission_races_and_unwind_never_exceed_cap` covers concurrent permits and unwinding. `cancellation_reaps_unpolled_handlers_and_drops_all_guards` accounts for cancellation.
- [x] Absolute deadlines: `headers_and_trickled_bodies_have_absolute_deadlines`, `header_progress_does_not_renew_the_body_budget` and `stalled_header_and_partial_body_release_request_capacity` use controlled time/synchronization. `nonreading_peer_expires_under_write_backpressure` and `blocked_handler_write_releases_request_and_response_reservations` exercise blocked socket writes and capacity release.
- [x] Protocol/authentication: malformed/version continuation, same-user async-socket checks, zero/oversized declarations before allocation, capped serialization and one bounded fallback all passed. Hello/health dispatch remains independent of the supplied doctor callback. Task-2 keeps synchronous arc-ipc/CLI APIs intact.
- [x] Material handoff: original report, nine hashes, fresh logs, scope/provenance and remaining integration obligations are delivered here. Normal collection and independent validation are still required.

## Task-4 and task-3 handoff

The original transport_unit target has a test-target dead-code allowance, and original main.rs did not register these modules. Those historical passes could not prove production visibility, bounded resident behavior, combined worker compatibility, performance or mission acceptance. This current snapshot already declares admission/connection/limits/runtime_metrics/transport/db_worker and lifecycle in production main.rs, and already imports the public busy constant. These are inherited task-4 edits. Fresh checks cover the supplied integrated bytes, without attributing their implementation or independent acceptance to this repair.

Task-4 backend retains ownership of production declarations, lifecycle and worker adaptation, public `ERROR_DB_BUSY` and replacement of the temporary constant. Keep synchronous protocol/CLI APIs unchanged. Its acceptance requires an actual integrated CLI test showing DB_BUSY maps to IPC_ERROR, existing successful output/compatibility assertions, and exact Cargo gates with production modules visible and no blanket product dead-code suppression. This repair makes no task-4 edit.

Task-3 remains separately owned and independently accepted. The historical finding was nonexistent RuntimeMetrics::default() in `db_worker/tests.rs` and a divergent isolated-driver metrics double. Current supplied helper already constructs RuntimeMetrics::new using matching RuntimeLimits/DbConfig. No database file changed here and no isolated-driver result is inferred. Database must retain real limits/metrics bindings, high-water cap, final-zero/start/join assertions, refreshed hashes/checks and its non-ignored handoff. Task-2 receipts do not mark task-3 passed. Chief must retain independent acceptance of both implementations before task-4 acceptance.

## Receiving collection and reviewer routing

One supported noninteractive `/bin/pwd` probe in the review checkout passed through this Codex route without a PTY; see `attempt-6/review-environment-probe.json`. That does not establish qwen runner recovery. The independent reviewer must probe its own route once. If grantpt denial persists, return the unmet execution prerequisite to Chief for an existing configured reviewer with functioning noninteractive tools. Do not repeat the denied probe, use GUI/ComputerUse or waive independent review/final oma-qa.

The new evidence paths are non-ignored. `attempt-6/collection-observation.json` records actual pre-return receiving presence and source comparisons. The current Actor can verify its delivery index locally; it cannot observe future normal collection. Chief/framework must run `python3 -B .arc-verify/bounded-ipc/ipc/attempt-6/check_collection.py` from the receiving checkout after collection. The checker verifies every indexed artifact/source hash and frozen baseline bytes read-only. No manual cross-checkout copy, commit or ref mutation simulates delivery.

## Failures and remaining scope

Initial combined ref resolution failed for absent isolated refs; a guessed `attempt-5/repair.py` read returned missing file. These were inspection failures, not Cargo failures. Full Git status reports `av.yaml: Operation not permitted`; its configuration/target applicability cannot be inferred. Mise tracking-symlink warnings were emitted; toolchain checks and all gates still passed. Earlier command failures, including historical shared-target permission failures, remain preserved rather than reclassified as passes.

Later tasks retain deterministic 100,000-attempt subprocess/CLI admission, overload/recovery and slow-DB health proofs; five-second SIGTERM/SIGINT with complete handler/worker accounting, reopen/integrity and cleanup; native RSS/thread/task/P95 measurements against the authoritative SRS; and unchanged harness execution with unresolved failures retained. This repair supplies no new native performance measurements or SRS comparison and does not accept the whole mission. Historical harness failures remain resident.rs and arc-ipc/lib.rs size checks plus missing test:coverage. Phase 0 features, Linux qualification and Windows Named Pipe/runtime/tray obligations remain open. Configured-target access and task-6 collection findings remain Chief-coordinated outside this ownership.
