# Managed execution: product claims and value scope

Status: completed documentation assignment, task-3, attempt 3, 2026-11-09.
Owner: Marketing Director. Changed path: this report only. Skill: `oma-docs`.

## Audience and value

The mission requirements imply three audiences: a macOS developer running an
interactive local tool, a script author depending on exact arguments, streams
and exit status, and a tool maintainer inspecting work after daemon loss. These
are requirement-derived audiences, not interviewed users, prospects or buyers.

Successful lifecycle tests support a technical claim: the tested invocation
executes one child, records its lifecycle and reconciles process identity after
daemon restart. The possible customer value is less uncertainty about whether
work started or remains running. Investigation time, user-triggered duplicate
launches, support effort, adoption and willingness to pay have not been measured.
A fixture pass does not establish those outcomes or financial returns.

The earlier [bounded IPC value scope](../../docs/bounded-ipc-value-scope.md),
[runtime identity value scope](../../docs/runtime-identity-value-scope.md) and
[classification value scope](../command-classification/value-scope.md) make the
same distinction between technical results, inferred user benefits and unknown
commercial value. Their command inventories and outstanding-work statements
retain their original scope; they do not describe this later snapshot. The
supplied marketing advice proposes assessments, integration and distribution.
The saved assignment defers those activities; they add no completion gate.

## Inspected evidence

This report inspects Actor-local snapshot `bb9f46b314bc49a2a79b6503a3cac27c0ae48946`,
not a new runtime execution. The [CLI declaration](../../crates/arc-cli/src/main.rs)
contains `exec` and `job status`. The [execution path](../../crates/arc-cli/src/exec.rs)
passes native arguments to `Command::new` and `args`, with one spawn outside
metadata retries. The [detector](../../crates/arc-detector/src/lib.rs) exports
argv classification alongside literal-text classification. These observations
establish source structure, not successful execution by themselves.

The retained [Chief report](final.md) and [final summary](final-summary.json)
record native macOS lifecycle acceptance. The retained [independent QA](qa.md)
and [QA receipt](qa.json) record PASS, no open findings and renewed execution.
This attempt recomputed all 177 content/mode bindings in each JSON summary:
all match this snapshot. Their common managed-input hash is
`e3df693429e2f7dd43f7a1b5fb1835265c9931a31cbe4686073ac6309a3f9d29`.
This binding check did not rerun Cargo, coverage, the harness or runtime probes,
and does not independently verify every retained output or local coverage file.

The previous version of this report described a foreground-pipeline failure.
Later evidence in this snapshot reports its repair and passing native pipeline
checks; renewed QA also records closure of the signal/core-display finding.
The previous failure remains historical evidence, not the latest recorded
verdict. Final acceptance belongs to the Chief in the integrated worktree.

## Claims and their limits

Each row names an existing acceptance test in the inspected snapshot. The
[contract](contract.json), retained final report and independent QA describe the
broader required gates. Use these claims only with matching final-source results.

| Claim supported by successful evidence | Existing source and named acceptance test | Limit |
| --- | --- | --- |
| Execute argv directly; preserve tested argv bytes, cwd, environment, stdin, stdout, stderr and exit 17. | [CLI tests](../../crates/arc-daemon/tests/managed_exec/cli.rs): `exact_native_argv_environment_cwd_streams_and_exit17_match_direct_execution`. | Caller-shell quoting precedes ARC. Literal shell operators remain arguments unless the caller invokes a shell. |
| Preserve tested terminal input, foreground signals and stop/resume behavior. | [Pipeline tests](../../crates/arc-daemon/tests/managed_exec/pipeline.rs): `native_foreground_pipeline_input_signals_stop_resume_and_exit17`; `native_inherited_ignored_sigchld_preserves_target_state_and_completion`. | Native PTY/pipeline fixtures support only their tested conditions; they are not a human usability study. |
| Launch one child within a managed invocation despite the tested IPC retries and lost replies. | [Retry tests](../../crates/arc-daemon/tests/managed_exec/retries.rs): `cli_has_one_child_under_before_after_commit_reply_loss_and_retains_exit17`. | No exactly-once promise across separate user invocations, another tool's retries or arbitrary failures. |
| Persist typed `job.start`, `job.complete` and `job.status` lifecycle state with process identity and idempotent completion. | [Contract](contract.json), operations/state machine; [storage tests](../../crates/arc-storage/src/jobs/tests/transitions.rs): `register_claim_attach_complete_and_identical_retries_preserve_revision_and_times`; [protocol tests](../../crates/arc-daemon/tests/managed_exec/protocol.rs): `retries_response_loss_concurrent_claim_and_completion_are_idempotent`. | A PID alone is insufficient. Repeating completion does not authorize overwriting a conflicting outcome. |
| Retain defined spawn-failure, signal and observed-child outcomes. | [CLI tests](../../crates/arc-daemon/tests/managed_exec/cli.rs): `spawn_failure_quick_exit_help_and_status_have_static_diagnostics`; `wrapper_directed_signal_preserves_native_signal_status_and_no_second_child`. | Recording failure must not replace the observed child result. Missing outcome evidence is not success. |
| Preserve the tested child through daemon shutdown/crash and reconcile after restart without replay. | [CLI tests](../../crates/arc-daemon/tests/managed_exec/cli.rs): `daemon_shutdown_and_crash_preserve_child_and_restart_reconciles_without_replay`; [protocol tests](../../crates/arc-daemon/tests/managed_exec/protocol.rs): `identity_mismatch_missing_and_restart_never_invent_an_exit`. | Missing or indeterminate identity must not imply a known exit code. Status is an observation, not a perpetual liveness guarantee. |
| Execute remote/UNKNOWN classifications without resource reservations. | [Retry tests](../../crates/arc-daemon/tests/managed_exec/retries.rs): `path_lookup_remote_and_unknown_commands_execute_without_resource_reservations`. | Local executable substitutes test pass-through, not remote-service availability. Classification does not establish scheduling or optimization. |
| Keep raw argv/environment secrets out of tested lifecycle persistence and ARC diagnostics. | [Protocol tests](../../crates/arc-daemon/tests/managed_exec/protocol.rs): `version_first_typed_payload_validation_recent_bound_and_secret_absence`; [independent QA](qa.md), native sentinel audit of rows, database, shm, WAL and ARC diagnostics. | Do not promise censorship of child output or files written by the program. No marketing telemetry is added. |
| Preserve protocol 1, existing diagnostics and bounded admission/shutdown in required regression cases. | [Contract](contract.json), compatibility/bounds; [independent QA](qa.md), requirement matrix and workspace result; [final report](final.md), required gates. | Targeted lifecycle tests alone cannot prove every compatibility case. No unlimited capacity or all-platform reliability claim follows. |

Daemon-loss survival is distinct from terminal-session survival, terminal closure,
logout, CLI death, host sleep/reboot or arbitrary parent loss. Those survival
claims are unsupported here. The slice does not establish orphan discovery or
cleanup, detached-session management, automatic termination policy, scheduling,
CPU/memory optimization, build acceleration or measured resource savings.
Keeping a child alive through daemon loss must not be sold as orphan cleanup.

Native macOS lifecycle acceptance does not establish desktop integration, native
Windows acceptance or Phase 1 scheduling. Those remain outstanding, as recorded
in the retained final report. No distribution readiness claim follows.

## Internal wording for final acceptance

Use after the Chief verifies the final integrated sources and evidence; cite the
actual integrated receipt identity. This is proposed copy, not a fresh acceptance
result from this documentation task:

> Native macOS acceptance passed for the Phase 0 `arcctl exec -- <argv...>`
> lifecycle slice, as recorded in [final integrated receipt identity]. Tested
> cases preserve direct argv execution, command I/O, terminal behavior and child
> outcomes; prove one child through IPC retries; and verify persistent lifecycle
> state and restart reconciliation without replay. The tested child survives
> daemon shutdown and crash. Remote and UNKNOWN commands pass through without
> reservations. Desktop, native Windows and Phase 1 scheduling remain outstanding.
> Customer time savings, adoption and financial returns are unmeasured.

If an integrated check fails or evidence is missing, report that actual state
instead of “acceptance passed.” The fixed Cargo, coverage and full-harness gates
remain unchanged. This report neither replaces nor relaxes them.

## Possible later commercial work

| Proposal requiring separate authority | Hypothesis and missing evidence |
| --- | --- |
| Paid lifecycle assessment | A macOS tool maintainer with a reproducible duplicate-launch or recovery problem might buy a scoped reproduction and compatibility assessment. Buyer access, budget, price, payment and delivery/support effort are unknown. |
| Separately scoped integration | A qualified team might pay to integrate accepted lifecycle contracts. No customer, agreement, support obligation or positive contribution is established. |
| Technical distribution or referrals | A runnable explanation, troubleshooting article, maintainer introduction or disclosed referral might produce a relevant inquiry. Reach, qualified inquiries, paid conversion and channel economics are unmeasured. |

No prospects were researched, contacts made, material published, money spent or
commercial features added. The supplied advice's channels and issue references
are proposals for later research, not revalidated market facts. No external
source was needed for this bounded reconciliation.

Demand, revenue, pricing, acquisition/delivery/support costs, customer time
savings, CAC, LTV and ROI are unknown. Unknown costs are not zero. A later test
would need attributable collected payments and expenses/labor; recurring-value
estimates would also need observed retention. No forecast or savings claim is
made. Commercial uncertainty does not delay this technical delivery.

## Provenance and handoff

Accepted committed baseline: `19d7a507044cbf83918f58cd41056905c275c8d4`.
Recomputed baseline tree: `d2f8ace96256a7e073ab9d1bfb10fdc5cae3bf38`.
Actual inspected HEAD equals supplied snapshot commit
`bb9f46b314bc49a2a79b6503a3cac27c0ae48946`; its tree is
`718ed68af094726b7487daaa2d37d8c00214302f`. The snapshot includes implementation
and reports beyond the committed baseline. `main.rs` differs from that baseline;
`exec.rs` and this report are absent there. Do not attribute snapshot features to
the committed baseline or infer that this report is tracked in another checkout.

The supplied snapshot and worker refs under
`refs/agent-valley/task-evidence/a5b368321dcdfcee-1724980aa084bbaa-3/`
are unavailable here: separate `git rev-parse --verify` calls exited 128. The
snapshot commit itself resolves. No missing worker revision was invented or
replaced with another checkout's HEAD. This updated report is an uncommitted
Actor-local edit requiring verification after integration.

Inspected retained summary SHA-256 values:

| Snapshot path | SHA-256 |
| --- | --- |
| `final-summary.json` | `37ef4eba0ae7ba589f632cb465d88b682d401b015334eae35b9e0826f82d23c2` |
| `qa.json` | `afb3a04ea15a4b0f569ca35080120435014f83f13a0552ca2cb1fc0a10664c8d` |

Coordination required: [receipt-index.json](receipt-index.json), `/artifacts/125`,
binds this report's preceding SHA-256
`0b65100765dc24fb7a214a144ee7319847d1d455a026aeb9d11d0d9a2a323c88`.
This authorized edit makes that entry stale. The Chief owns refreshing dependent
artifact hashes after integration; this task did not modify the index, final
summary or QA records. The current Actor index contains 129 managed-exec files;
the retained final report's count of 127 describes its earlier collection. No
evidence file was added here. Recount in the final review checkout.

## Checks and assignment acceptance

Manual inspection — `oma docs verify` did not run: it regenerates
`docs/generated/doc-refs.json` outside this task's sole owned path. No automated
drift count is claimed. Serena was absent from the available tool inventory;
known paths were read directly. No project code discovery, GUI, browser,
installation, build, runtime suite or external action was performed.

| Command/check executed | Outcome |
| --- | --- |
| `git status --short -- . ':!av.yaml'`; owned-file unstaged/staged diffs on entry | Exit 0; clean accessible worktree. `av.yaml` was excluded and not read. |
| `git rev-parse HEAD HEAD^{tree}`; baseline tree lookup | Recorded snapshot and baseline trees above. A combined lookup also encountered the unresolved worker ref; separate checks confirmed exit 128 for each supplied ref. |
| Direct `cat`/`sed` reads; read-only `python3 -B` inspection | Sources, prior value reports, exact test names and retained acceptance inspected. Both 177-file content/mode binding sets match; baseline comparisons confirm the stated differences. |
| Read-only document validation and `git diff --check -- .arc-verify/managed-exec/value-scope.md` | Local references, named tests, required scope distinctions, recorded hashes, whitespace and sole-file ownership checked after editing. These are document checks, not runtime acceptance. |

Assignment acceptance:

- [x] Provable lifecycle behavior, unmeasured customer value and future commercial hypotheses are separate.
- [x] Proposed claims exclude unsupported survival, cleanup, scheduling, optimization, desktop, native Windows and measured savings.
- [x] Product claims cite existing sources or named acceptance tests.
- [x] No external action or additional commercial completion gate was introduced.

Only this report changed. Product interfaces, sources, manifests, lockfiles,
registries, saved instructions, mission state and other Actors' work are
unchanged. Remaining handoff work is Chief-owned integration and receipt-index
refresh. Customer demand and financial value remain unmeasured.
