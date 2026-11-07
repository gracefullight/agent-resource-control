# Task-4 backend result, attempt 5

Status: completed task-local resumption and verification; ready for collection and independent review. Mission completion remains open.

Skill used: oma-backend with the installed backend-engineer definition, execution policy, execution protocol, result contract, backend checklist and backend/i18n rules. This is the existing Rust/Tokio/rusqlite workspace; no stack or dependency change was made. No OMA run ID or claim path was injected, and none was invented.

## Completed result

The input snapshot already includes the accept-error recovery and ARC_RUNTIME_REPORT repairs. Source inspection found no additional product repair necessary. The retained supervisor uses bounded tracked handlers, routes only doctor through the single bounded worker, cancels/reaps handlers and closes/joins the actual worker under the same 3,500 ms shutdown deadline. Signals, cleanup and optional final export share that deadline. The instance lock remains held through join and cleanup.

Accept errors use fixed saturating counters and one inline backoff: 5/10/20/40/80/100 ms, capped at 100 ms without a fatal pressure threshold. Signals and reaping remain responsive. Only invalid listener OS errors terminate service through normal accounting. The safe optional file export is authoritative: private create-new/no-follow, retained descriptor, complete serialization before writing, 4,096-byte maximum. The earlier stdout substitution remains rejected and superseded. The full exact consumer schema and policy are in integration/delivery.md; task-5 and final QA consume ARC_RUNTIME_REPORT.

## Files and provenance

Changed: .arc-verify/bounded-ipc/integration/{delivery.md,evidence.json,claim.json,repair-2026-11-07-attempt-5/}; this result report; exact restored attempt-3 and attempt-4 result reports under .agents/results/. All previous top-level reports/indices/claims are preserved as previous-* files. Source files, other Actors’ evidence, migration, harness, manifests, lockfile, mission state, assignments, commits and refs were preserved.

Accepted baseline is 42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0, tree c802f71ac4d91ed6b8a602d5c83bf9cf57116b76. Actor-local snapshot HEAD is 2fe61708aeffcd3638c7e9331a3ef3b4f8394ddf, tree 327fef63bace20053064f9662e5b1a5f7a2ea761. The supplied refs do not resolve in this checkout; actual commits/trees and baseline ancestry do. Current 50-file product fingerprint is b3610549adfebffdd626a760741a200c848ec7ee26a88482bcf7e38e563383c4. Current-source observations are scoped to this Actor snapshot, not the accepted baseline or receiving checkout.

The earlier verifier exits 1 because downstream resident.rs/support fixture-lock changes no longer match its historical manifest. That failure remains recorded. Missing own attempt-3/4 reports were restored from the retained source workspace with exact indexed bytes and hashes. No old receipt was relabeled as fresh evidence.

## Acceptance evidence

- [x] Bounded tracked handlers and one worker remain integrated; hello/health bypass SQLite.
- [x] Actual CLI status/doctor recover after one transient and nine resource-pressure accept errors. No retry task or error-log accumulation; consecutive state resets. Backoff shutdown and fatal-listener cause retention pass.
- [x] Export tests cover disabled mode, complete schema and safe permissions, existing-file and link preservation, unsafe parent refusal, retained descriptor, exact byte bound, deadline refusal and actual EBADF write failure with cleanup/join/reopen.
- [x] Both signals with stalled clients and held worker reap all eight handlers, start/join exactly one worker, leave zero gauges, preserve lock lifetime, remove the socket and reopen SQLite with integrity=ok. SIGINT to observed exit: 7,217 us; SIGTERM: 9,484 us.
- [x] Existing protocol/CLI/authentication/identity/admission/filesystem assertions remain active. No test edit, ignore, removal or weakening occurred.
- [x] Material delivery, receipts, source/artifact hashes and boundary evidence produced.

## Commands and outcomes

The supplied route and noninteractive shell worked without routing-policy changes. Saved deadline 2026-11-08T10:52:35.601Z and existing limits were retained. Commands ran offline, serially for Cargo, under Rust/Cargo 1.95.0 on aarch64-apple-darwin.

- Earlier checker: exit 1, historical resident.rs manifest mismatch, retained.
- Focused test using prepared parent target: exit 101, .cargo-lock Operation not permitted, retained.
- Focused test using new local target: exit 101, 48 passed/four failed because arcctl was absent, retained.
- cargo test -p arc-cli --test binary: exit 0, two passed; provides the real local CLI prerequisite without a standalone build.
- cargo test -p arc-daemon --bin arc-daemon -- --nocapture --test-threads=1: final exit 0, all 52 passed.
- cargo fmt --all -- --check: exit 0.
- cargo clippy --workspace --all-targets -- -D warnings: exit 0.
- cargo test --workspace: exit 0, 173 passed across 17 suites; zero failed/ignored/measured/filtered.
- ./scripts/harness/validate.sh unchanged: exit 1; resident.rs 901 lines, arc-ipc/src/lib.rs 551 lines, missing test:coverage. All three failures retained.
- git diff --check: exit 0; av.yaml permission diagnostic retained.

Receipts include argv/cwd/UTC/observed exit/PID/timeouts, toolchain/environment, source-before/after and complete log hashes. The migration matches accepted baseline byte-for-byte: SHA-256 046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384. Harness bytes also match accepted baseline. The original RED accept regression (attempt 3, exit 101) and its hashes remain retained; fresh GREEN is in focused.txt. No installation, download, standalone build, GUI/browser action or deployed-service operation occurred.

## Remaining work

Chief: collect the indexed artifacts, run the receiving-checkout verifier and retain independent task review before downstream work resumes. Final independent oma-qa remains task-8. No ownership handoff or out-of-scope edit is needed. Mission-level saturation/accounting reports, native RSS/thread/task/P95 and SRS assessment, operator verification and tasks 5-9 remain separate obligations. This task’s passing workspace suite does not replace them.

Retain the three harness failures and configured-target access gap (av.yaml denied). Phase 0 features, Linux qualification and Windows Named Pipe/runtime/tray/measurement obligations remain open. Native kernel/filesystem stalls cannot be forcibly preempted; failed I/O/deadlines remain failures. Local evidence verification does not claim post-collection or independent-review completion.
