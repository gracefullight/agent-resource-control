# Task-6: bounded CLI recovery documentation, attempt 4

Status: assigned documentation changes completed and current-snapshot workspace tests passed. Independent collection/review and mission completion remain with the Chief Director.

## Changed paths and outputs

Owned documents:

- `docs/runtime-identity-usage.md`: preserve start → readiness → status and optional doctor; add the integrated operational/resource-error mapping, qualify retry guidance and record the actual attempt-4 snapshot.
- `docs/bounded-ipc-recovery.md`: preserve the fixed limits/deadlines, predecode closure, decoded DB_BUSY response, liveness/integrity/saturation distinctions, direct signals and normal restart without deleting locks. Add CORRUPT/NOTADB versus operational failures and current source/test evidence.

Material report and receipts: `.arc-verify/bounded-ipc/task-6/attempt-4/`. `delivery.md` is the report; `workspace-test.json` binds the full test output and 51 product inputs; `source-evidence.json` preserves the supplied provenance without replacing any hash. `artifacts.json` binds delivered documents and evidence. Run `python3 -B .arc-verify/bounded-ipc/task-6/attempt-4/verify.py` in the receiving checkout after collection. It is read-only and reports any missing or changed artifact/source.

The earlier `.agents/results/result-design-director-task-6-attempt-3-local.md` is absent in this snapshot. It was not recreated under its old name or treated as delivered. This replacement report is nonignored and part of this attempt's explicit material output. No injected OMA claim path, run ID or session ID was supplied; none was invented. Do not require collection of an ignored path absent from the delivery index.

## Acceptance evidence

- [x] Guidance matches implemented limits, mappings, deadlines and CLI journey. Source inspection: `limits.rs`, `main.rs`, `admission.rs`, `transport.rs`, `connection.rs`, `lifecycle.rs` and `arc-cli/src/main.rs`, at snapshot `eb45d23b3b3be650f4a5cb8e064ba6715ae78b86`, tree `4746b22753c020491e2570d684e5edd86c02d468`. Default caps are 64 handlers, 16 requests and eight queued/reserved DB jobs plus one executing worker job; test-profile caps are 8/4/2+1. Header/frame/write budgets are 1,000/2,000/500 ms by default and 150/300/100 ms in the fixed test profile. DB response/execution/busy budgets are 1,500/1,000/100 ms and 500/300/50 ms; shutdown shares 3,500 ms. `arcctl` retains five-second per-operation socket timeouts, not an absolute total exchange deadline.
- [x] Liveness, integrity and full transport saturation are distinguished. `status` uses hello, no SQLite queue; doctor checks integrity and one migration. Both need transport/request capacity. Predecode overload closes, mapping to DAEMON_UNAVAILABLE. Decoded DB admission/deadline/operational failures use DB_BUSY → IPC_ERROR if the write succeeds. Typed CORRUPT/NOTADB and invalid integrity/schema reports retain INTEGRITY_FAILED. IOERR/IOERR_READ/NOMEM do not establish corruption. The docs explain that IPC_ERROR does not prove saturation and retry is not guaranteed.
- [x] Material design evidence identifies support and untested assumptions. `docs/bounded-ipc-recovery.md` maps guidance to paused-time transport units, real subprocess/CLI recovery, slow-DB health, direct signal accounting, typed database-error regressions and identity/resident checks. Personas and shorter recovery are assumptions/intended outcomes. Four-second guidance is derived from default budgets, not a measured optimal interval. Persistent arrivals and persistent connections can keep slots occupied. Automatic retries/progress controls remain scoped-out hypotheses. No participant, screen-reader, retention, conversion, churn or user-reaction claim is made. No screen checks were performed or required.

The source/test observations apply to this Actor's isolated input snapshot and hashed product inputs. The accepted committed baseline remains `42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0`, tree `c802f71ac4d91ed6b8a602d5c83bf9cf57116b76`. The integrated repair is not claimed to be present in that baseline. The two documentation edits are uncommitted Actor-local changes. The supplied snapshot and worker refs each fail local resolution with exit 128; their missing values were not substituted with HEAD. Receiving-worktree bytes still require collection verification.

## Commands and outcomes

| Command/check | Outcome |
| --- | --- |
| `cargo test --workspace` | Exit 0, 50.12 seconds; 183 passing invocations across 17 suites; zero failed, ignored, measured or filtered. Offline, worktree-local target; no standalone build/download. The count includes repeated module registrations. |
| Source manifests before/after | All 51 product files retain hashes and modes. The run includes the five new adapter regressions, real CLI mapping and recovery, paused-time absolute deadlines, stalled/nonreading subprocesses, full request/connection saturation recovery, controlled slow DB health, SIGTERM/SIGINT accounting and 100,000-attempt fixtures. |
| `git diff --check -- docs/runtime-identity-usage.md docs/bounded-ipc-recovery.md` | Exit 0. |
| Deterministic local Markdown path/heading resolution | Zero broken references before edits; zero broken references after edits. Counts and each target are retained in references-before.json and references-after.json. This is manual inspection — `oma docs verify` did not run. |
| `oma docs verify --help` | Exit 0; confirms shared `docs/generated/doc-refs.json` mutation. No ownership handoff exists, so verify/sync did not run or modify that registry. No external URLs occur in the two documents. |
| Accepted-baseline byte comparison | `0001_phase0.sql` and unchanged `scripts/harness/validate.sh` both match; exact hashes in preservation.json. |
| Boundary check | Only the two owned documents changed among readable tracked inputs; av.yaml is explicitly excluded because access is denied. No product-code, manifest, lockfile, shared registry, mission-state or assignment edits. |
| `git rev-parse` snapshot/worker refs | Exit 128 for each supplied ref; actual local HEAD/tree and baseline tree resolve and match their supplied hashes. |
| Scoped `git check-ignore` | Exit 1, no output: the new report/helper paths are not ignored. Collection is still a separate event. |

The initial unscoped Git status emitted `av.yaml: Operation not permitted` while returning exit 0; no clean whole-tree claim is inferred. It remains unread. OMA/mise emitted tracking-symlink warnings; successful command outcomes above were observed despite those warnings. The first `check.py finish` exited 1 because its two linked output receipts did not yet exist when it checked references; that invocation then wrote both receipts. The failed findings are preserved in first-finish-failure.json. A second pass checks the existing files without removing a link or weakening a check. Initial exploratory reads found `.arc-verify/bounded-ipc/design/`, `.agents/results/`, an optional attempt-3 `run.py` and the pretest local binaries absent; none was treated as a supported tool failure or bypassed. Existing `attempt-3/check.py` was read and adapted into the new attempt's evidence helper; historical files were preserved. The required workspace test command created its own test/binary artifacts normally.

Formatting, Clippy and the unchanged harness were not rerun for these documentation edits. The task-4 dependency delivery retains exact formatting/Clippy/workspace passes and harness exit 1 for resident.rs at 901 lines, arc-ipc/src/lib.rs at 551 lines, and missing test:coverage. Those three failures remain unresolved. This task's passing workspace receipt does not replace final independent verification or native SRS-performance evidence assessment.

## Interfaces, skills and remaining risks

CLI commands, exact successful text, color-independent error lines, exit codes, protocol-1 framing, authentication, identity, lock/filesystem policy, configuration and runtime-report interfaces are unchanged. No product files outside task-6 ownership were modified. Final fingerprints and local link checks are in boundary.json; no post-collection verification event is invented.

Skills: oma-docs, selected verify/sync source-alignment guidance with manual fallback for the shared-registry ownership conflict; oma-design, local contract route for a known interaction decision. No visual system changed and no external design supplier was needed. Serena tools were unavailable; inspection used supplied known paths. No delegation, external delivery, GUI/browser action, installation, standalone build or download occurred.

Chief handoff: collect both documents and the entire attempt-4 evidence directory, then run its verifier against the receiving source inputs. The missing earlier ignored report should not be indexed as a new delivery. Coordinate any shared documentation-registry write separately. Preserve av.yaml's denied-access limitation and unverified configured-target applicability. Required mission QA/performance/final review remains with its assigned owners. Structural caps are not native measurements; fixture executions are not participant studies. Remaining Phase 0/tray/vendor/classification/Sweeper, Linux qualification and Windows runtime/Named Pipe/tray obligations remain open. No overall mission-completion or deployed-target claim is made.
