# Task-2 attempt-4 evidence delivery

Status: ready for independent review. This repair changes evidence only. It does not claim mission completion, independent review, or future normal collection.

Actor: backend / IPC Engineer. Mission b137e253-def6-46ef-a75d-25ab414784ea; task-2, attempt 4. Codex session 01a1b68c-b5e1-78e0-902b-b47669bb7195. Skill used: oma-backend, with its execution protocol/checklist and shared execution policy/result contract. No OMA run ID or claim path was injected; neither is invented. No new mission run, agent, retry budget, commit, evidence ref or mission-state change was made. The exact mission start/deadline was not injected; saved duration/run/retry/parallel limits are recorded unchanged in execution-context.json.

## Changed paths and preserved inputs

All repair deliverables are under `.arc-verify/bounded-ipc/ipc/attempt-4/`: this delivery, evidence.json, recorder scripts, command logs/receipts, execution contexts, provenance, test summary, probe and collection observations. The original Actor report remains byte-identical at [original-delivery.md](../original-delivery.md), SHA-256 5fea11c0be36072eb0a5dbf7791997a403fca936c0fef0a2fc0a9fc942c020cb. Existing task-2 receipts and delivery/evidence files are untouched and hashed in provenance-before/after.json. Earlier gate claims are retained as historical context, not reused as fresh results.

No product, test, manifest, lockfile, database-owned file or shared registry changed in this repair. The 51-entry source manifest (crates/manifests/lockfile/toolchain plus unchanged harness) has fingerprint `ce490655f54afd9eda3498a1d2c78d02dfde38707b2cc4a4688d300f84a725a4` before and after every command. Original full status emitted `av.yaml: Operation not permitted`; scoped product status is separately retained and empty in the Actor checkout. Serena is unavailable in this runtime; inspection used supplied known-path reads and read-only Git metadata, without replacement native source discovery.

## Provenance and inherited integration

Accepted committed source baseline: 42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0; exact tree c802f71ac4d91ed6b8a602d5c83bf9cf57116b76. Original task-2 snapshot: 3816bafaa684c8099dad78be2d0cb0e474c5f08b; tree e79ff4ed85c94059275bbb4992579a2d55f51eb3. Review-side original snapshot and worker refs resolve to that revision. Their absence in this isolated clone is retained as exit-128 receipts, not replaced by local HEAD.

This invocation's supplied and inspected Actor HEAD/input snapshot is eac1ffc1739f3cc3da254376b8243c7e05012087; tree 4214462f01a3c059d825ca6ae7ed90dc156ed086. It contains inherited later integration. Review HEAD remains the accepted baseline, with those same product bytes in its dirty/untracked overlay; tracking in the Actor snapshot is not tracking in the review checkout or inclusion in accepted baseline. Review-side attempt-4 snapshot ref resolves to eac1ffc; its worker ref is absent before collection. Both scopes, ancestry, trees, ls-tree records and dirty boundaries are in provenance-before.json.

All nine delivered transport files match the current snapshot and observed review overlay. Eight match their original task-2 hashes. `crates/arc-daemon/src/connection.rs` now hashes to 560c0cd1728d50ac507fc42506bcd859941b6d46f706ab3276676add18f66244 instead of original 90c31e454a00b698dd1390f6dbe18edc9211f27a34b3d5848e43602142ed2195. The inherited diff against repair snapshot eee0c01750bcf852d691395774215b18628e89d7 replaces only the temporary local DB_BUSY constant with `pub(crate) use arc_ipc::ERROR_DB_BUSY;`. Do not describe all nine as original-hash-matched. No rollback or product repair was made to satisfy stale recovery wording.

The migration is byte-identical to `git show` of accepted baseline, SHA-256 046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384. Harness bytes also match accepted baseline, SHA-256 93140173df8b8bd28b067aff68301134b0398b643240b0b1e6a13da4487ebe33. This task did not rerun the harness; later verification obligations remain intact.

## Exact serial checks and retained failures

| Receipt | Command | Exit | Elapsed |
| --- | --- | ---: | ---: |
| `fmt.json` | `cargo fmt --all -- --check` | 0 | 0.481s |
| `clippy.json` | `cargo clippy --workspace --all-targets -- -D warnings` | 101 | 0.310s |
| `workspace-test.json` | `cargo test --workspace` | 101 | 0.144s |
| `local-cache/fmt.json` | `cargo fmt --all -- --check` | 0 | 0.315s |
| `local-cache/clippy.json` | `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 8.926s |
| `local-cache/workspace-test.json` | `cargo test --workspace` | 0 | 66.158s |

Every receipt records argv, actual cwd, UTC interval, PID/termination observation, full combined stdout/stderr hash and source fingerprints. Contexts retain Rust/Cargo 1.95.0, aarch64-apple-darwin, relevant environment and cache paths. Commands ran serially, offline. The first Clippy and workspace attempts failed before checking code because the shared review target/debug/.cargo-lock returned Operation not permitted. Those exit-101 receipts remain intact. The subsequent local-cache run uses Cargo's ordinary worktree-local target directory, with no dependency install/download or standalone build. Required command argv were unchanged. Mise tracking-link warnings are retained. An auxiliary noninteractive ps command was denied; no GUI or alternate permission bypass was attempted.

Workspace result: exit 0; 17 reported suites, 173 passed, 0 failed, 0 ignored. transport_unit executed all 18 existing tests: True. Full per-test output is in local-cache/workspace-test.txt and the extracted target section is in test-summary.json. No tests or acceptance criteria were edited.

## Task-2 acceptance evidence

- Admission/accounting: retained_tasks_reap_and_rejections_do_not_call_factory exercises completed-but-unreaped retention, reaping before replacement, full-cap closure, and no rejected factory invocation; request_admission_races_and_unwind_never_exceed_cap exercises concurrent permit accounting and unwind release; cancellation_reaps_unpolled_handlers_and_drops_all_guards accounts for aborted handlers.
- Absolute I/O: headers_and_trickled_bodies_have_absolute_deadlines and header_progress_does_not_renew_the_body_budget use paused time; stalled_header_and_partial_body_release_request_capacity verifies release; nonreading_peer_expires_under_write_backpressure and blocked_handler_write_releases_request_and_response_reservations establish real socket backpressure and deadline expiry.
- Protocol/authentication: malformed_json_and_version_errors_continue_without_database preserves continuation; zero_and_oversized_frames_reject_without_body_allocation validates the cap first; response_serialization_is_capped_before_any_prefix_is_written and oversized_echo_uses_one_small_fallback_then_closes bound response/error work; same_user_authentication_supports_async_sockets_and_rejects_non_sockets preserves native UID checks. Synchronous arc-ipc/CLI APIs were not edited by this repair.
- Material evidence: source hashes, original report, interface boundaries, full failed/passing command receipts and limitations are retained here. Test execution is established by logs, not by this checklist alone.

## Task-4 and task-3 handoff

The original task-2 check scope was a standalone transport_unit target with a test-target dead-code allowance; at that time main.rs did not register the new modules. Those historical passes could not establish production visibility, combined worker compatibility, bounded resident behavior, native performance or mission acceptance. This input snapshot already declares admission/connection/limits/runtime_metrics/transport/db_worker and lifecycle in production main.rs; the imported busy constant is also already present. These are inherited task-4 work, not task-2 changes. Fresh workspace Clippy/tests cover the actual supplied integrated bytes; this evidence repair does not independently accept task-4 or the whole mission.

Task-4 backend retains ownership of production declarations, lifecycle and worker adaptation, public arc-ipc ERROR_DB_BUSY, and its use from connection.rs. Preserve synchronous protocol/CLI APIs. Its acceptance requires an actual integrated CLI DB_BUSY -> exactly IPC_ERROR test, successful CLI output/compatibility assertions and exact Cargo gates with production modules visible and no blanket product dead-code suppression. Existing integrated tests are executed by the workspace run; ownership and independent acceptance remain separate.

Task-3 remains a separately owned prerequisite. The historical recovery finding was nonexistent RuntimeMetrics::default() in db_worker/tests.rs and a divergent isolated-driver metrics double. Current supplied source already uses RuntimeMetrics::new with RuntimeLimits matched to DbConfig in that helper. No database file was changed here, no driver acceptance is inferred, and task-2 receipts do not mark task-3 passed. Database must retain its real limits/metrics driver binding, high-water/final-zero/start/join assertions, refreshed hashes, checks and non-ignored delivery; Chief must retain its independent acceptance before relying on it.

## Review, collection and remaining obligations

One supported Codex noninteractive /bin/pwd probe in the review checkout passed without PTY. It is not evidence that qwen's runner recovered. The independent reviewer must probe its own noninteractive route once. If grantpt denial persists, retain that unmet prerequisite and return it to Chief for routing to an existing configured reviewer with working tools; do not repeat denial, use ComputerUse/GUI or waive review/final oma-qa.

These new artifacts are non-ignored and ready for normal framework collection. collection-check.json records their actual pre-return review-path presence and the already integrated nine-file comparison. Future collection cannot be observed from this Actor turn: Chief/framework must verify delivery.md, evidence.json and all indexed artifact hashes after normal collection. No manual cross-checkout copying, ref creation or commit simulates delivery.

Later obligations remain unchanged: deterministic 100,000-attempt subprocess/CLI cap, overload/recovery and slow-DB health proofs; strictly-under-five-second SIGTERM/SIGINT with all handlers/worker joined, reopen/integrity and cleanup; native RSS/thread/task/P95 measurements compared with authoritative SRS; unchanged harness execution with every unresolved failure retained. This task does not refresh native measurements or claim those later results complete. Remaining Phase 0, Linux qualification and Windows Named Pipe/runtime/tray obligations stay open. Missing task-6 receipts and denied av.yaml/configured-target evidence remain Chief-coordinated final-review issues outside this evidence-only ownership.
