# Independent command-classification verification matrix

Reviewer: Independent QA Reviewer, task-5. Skill: oma-qa. Accepted baseline and
current HEAD: 05f0812d0841485bbe07b3f1f8f6fde3861a1d55. The implementation is an
uncommitted slice over that baseline. This matrix records executed tests, not a
proposed plan. Attempt 4 verifies the repaired source and permanent SSH regressions with fresh execution. Product source, tests, contracts and implementation evidence were not edited.

`W` means the exact `cargo test --workspace` run retained in workspace-tests.json
and workspace-tests.txt. Every named test below ran successfully in W. Names are
qualified by crate/target where different executables contain the same name.
`P` means the additional exact command:

```text
cargo test -p arc-daemon --test classification privacy::required_loopback_endpoint_nonexecution_and_privacy -- --exact
```

Its receipt and complete output are privacy.json and privacy.txt. Input hashes,
post-check hashes, cwd, effective user/context, timestamps and exits accompany each
command. tcp-prerequisite.json records a fresh permitted bind/listen after the
operator's environment note; historical bind denials are not current evidence.

## Required behavior

| Obligation | Exact executed test names | Assertions inspected |
| --- | --- | --- |
| cargo build; cargo test; cargo clippy; cargo fmt | arc-detector/lib `tests::delivered_contract_matrix`; arc-ipc/classification `classification_contract_dispatch_never_calls_doctor`; arc-daemon/classification `actual_daemon_and_cli_delivered_contract_matrix` | Four-field expected values across detector, dispatch, actual daemon and human/JSON arcctl. BUILD, TEST, LINT, and distinct CARGO_BUILD/CARGO_TEST/CARGO_CLIPPY/CARGO_FMT reasons. |
| gh workflow run; gh run watch; gh run view | Same three matrix tests | REMOTE_CI, REMOTE, corresponding GH reason, NONE. |
| SSH remote execution and forwarding ambiguity | Same three matrix tests; arc-detector/lib `tests::syntax_quotes_aggregation_and_ssh_negative_matrix`, `tests::ipv6_destination_numeric_validation_and_loopback_conservatism`; arc-ipc/classification `ssh_option_values_are_not_forwarding_flags`; arc-daemon/classification `ssh_option_values_preserve_typed_daemon_and_cli_observations` | REMOTE_EXEC for supported remote-command syntax; The covered pre-destination forwarding/unsupported options, loopback and missing-command cases yield UNKNOWN/OBSERVE. Option values containing N/D/L are not forwarding switches. The new SSH matrix and independent probe cover post-destination flags, terminators, empty words, opaque arguments and compound uncertainty. No host lookup. |
| Safely resolved local/remote compounds | Three matrix tests; arc-detector/lib `tests::syntax_quotes_aggregation_and_ssh_negative_matrix` | MIXED for local+remote leaves, including pipes, groups and literal shell wrappers. Unknown third stages invalidate certainty; heterogeneous local-only stages are UNRESOLVED_COMPOUND. |
| Quoted command-like text is data | Three matrix tests; arc-detector/lib `tests::syntax_quotes_aggregation_and_ssh_negative_matrix`; arc-cli/classification `literal_payloads_and_options_are_preserved_in_one_request` | Quoted gh/SSH/cargo text and escaped operators never create invocations. UTF-8, tabs, whitespace and exact argument bytes are retained. |
| Unresolved scripts, expansions and Docker endpoints | Three matrix tests; both privacy tests below | UNKNOWN locality and OBSERVE uncertainty, with fixed UNRESOLVED_SCRIPT, UNRESOLVED_EXPANSION or DOCKER_ENDPOINT_UNKNOWN. No script reads or endpoint resolution. |
| Empty, oversized and unsupported-shell observations | Three matrix tests; arc-detector/lib `tests::malformed_payloads_are_not_analysis_requests` | Valid observations succeed, including explicit empty input; byte check precedes shell resolution, which precedes empty analysis. Missing command, null/wrong types and extra fields fail typed request decoding. |
| Inclusive byte/token/invocation/depth limits | arc-detector/lib `tests::input_unicode_tokens_invocations_and_depth_boundaries`; arc-daemon/classification `boundaries::detector_budgets_are_preserved_through_daemon_and_cli` | 16384/16385 UTF-8 bytes, 1024/1025 tokens, 128/129 invocations, depth 8/9 for groups and wrappers. Unit checks also combine group/wrapper depth. Production caps are unchanged. |
| Shared work accounting and overflow | arc-detector/lib `tests::work_counter_refuses_before_operations_and_never_wraps`, `tests::production_work_exhaustion_trace`; daemon boundary test above | 65536 accepted units then refused operation; checked overflow; actual nested re-tokenization exhausts work below the other caps; repeated results/counters agree. Supplemental work-trace.json/.txt retains the existing production test's printed counters. |
| Determinism and malformed syntax | arc-detector/lib `tests::bounded_deterministic_arbitrary_unicode_inputs_never_panic`, `tests::syntax_quotes_aggregation_and_ssh_negative_matrix`, `tests::delivered_contract_matrix` | 4096 fixed-seed Unicode/syntax inputs; repeated result and counter equality; consistent wire combinations; bounded counters; no panic. |
| Never execute tools or substitutions, contact endpoints, or persist commands/secrets | arc-daemon/classification `privacy::required_loopback_endpoint_nonexecution_and_privacy` (W and P), `privacy::unix_endpoint_traps_substitutions_and_live_wal_never_receive_input` (W) | Twelve data families, private executable traps, active/literal/nested/backtick substitutions, marker absence, TCP and Unix monitors, response/output privacy and recursive before/during/after scans including SQLite main/live WAL/SHM, captured daemon logs and runtime reports. Zero DB work. Supported by inspection of every detector production module and dependencies; probes alone are not universal proof. |

## IPC, CLI and preservation

| Obligation | Exact executed test names | Evidence boundary |
| --- | --- | --- |
| Protocol 1, version-first ordering and malformed-request recovery | arc-ipc/classification `malformed_payload_version_priority_and_legacy_envelopes`; arc-daemon/classification `invalid_payloads_version_priority_and_same_connection_recovery`; arc-ipc/lib `tests::dispatch::protocol_version_99_returns_incompatible_and_a_later_hello_succeeds` | Invalid typed payload plus version 99 returns protocol incompatibility first; invalid JSON/payload recovers on same connection. Legacy unknown envelope fields remain accepted. dispatch(request, doctor) signature is unchanged. |
| Classification never enters SQLite | arc-ipc/classification `classification_contract_dispatch_never_calls_doctor`; daemon matrix/recovery/privacy/saturation tests | Callback panics if reached; actual classification-only daemon DB high-water is zero. Only explicit doctor raises DB work in the matrix test. connection.rs drops body then clears payload before response await. |
| status/doctor text, health and diagnostics | arc-daemon/resident `lifecycle::resident_contract_holds_until_sigterm`, `lifecycle::commands_report_daemon_unavailable_when_absent`; arc-ipc/lib `tests::dispatch::hello_and_health_do_not_require_the_database`, `tests::dispatch::doctor_requires_ok_integrity_and_one_migration`; daemon matrix test | Exact existing status and doctor output retained, including classification not_checked and sweeper off; protocol/frame compatibility and clean reopen checked. |
| Same-user authentication | arc-daemon/transport_unit `tests::same_user_authentication_supports_async_sockets_and_rejects_non_sockets`; arc-daemon/main and worker_runtime `server::tests::foreign_uid_and_nonprivate_modes_are_rejected_by_the_live_policy`; resident test above | Actual same-user native socket accepted and non-socket rejected; foreign directory-UID policy checked without impersonation. Unchanged platform.rs checks native peer UID and admission.rs invokes it before handler construction. No cross-account live peer impersonation is claimed. |
| Admission, framing and deadlines | arc-daemon/transport_unit `tests::request_admission_races_and_unwind_never_exceed_cap`, `tests::zero_and_oversized_frames_reject_without_body_allocation`, `tests::saturated_requests_close_before_allocating_even_for_invalid_prefixes`, `tests::headers_and_trickled_bodies_have_absolute_deadlines`; arc-daemon/worker_runtime `admission_load::hundred_thousand_connection_attempts_never_spawn_rejection_tasks`, `admission_load::hundred_thousand_request_attempts_have_no_waiting_admission`, `admission_load::hundred_thousand_database_attempts_bound_retained_cancelled_work` | Existing no-wait admission, frame cap, bounded retained tasks, absolute deadlines, queue and cleanup assertions pass. limits.rs and admission.rs match baseline bytes. |
| Health and shutdown under continuing classification | arc-daemon/classification `saturation::maximum_and_exhausting_classification_load_preserves_health_and_shutdown` | Sixteen traffic workers alternate maximum-size and work-exhausting requests. Same-connection malformed recovery, ten actual health successes, continuing classifications and SIGTERM within 3500 ms. runtime/workspace-tests/classification-saturation.json and classification-accounting.json retain fresh measured values and zeroed gauges. |
| General shutdown and worker responsiveness | arc-daemon/main `lifecycle_tests::sigterm_reaps_stalled_handlers_and_joins_worker`, `lifecycle_tests::sigint_reaps_stalled_handlers_and_joins_worker`; arc-daemon/worker_runtime `health_completes_while_native_sql_preparation_is_deliberately_stalled`, `saturated_sigterm_finishes_within_five_seconds`, `saturated_sigint_finishes_within_five_seconds` | Existing joined-worker/handler accounting, socket cleanup and signal guarantees remain intact; no new classifier tasks or thread pool. |
| CLI help, usage, exact input and typed failure handling | arc-cli/classification `help_usage_privacy_and_absent_daemon`, `literal_payloads_and_options_are_preserved_in_one_request`, `peer_failures_are_typed_correlated_and_never_reflected`, `timeout_is_bounded_with_no_retry_or_partial_result` | Actual arcctl subprocesses and local fake peers cover help/examples, no request on usage errors, input-free errors, no preliminary diagnostic/retry, request correlation, typed enums/combination validation, malformed peer/EOF/timeout, failure recovery, and no ANSI. |
| Successful human/JSON output | arc-daemon/classification `actual_daemon_and_cli_delivered_contract_matrix`, `boundaries::detector_budgets_are_preserved_through_daemon_and_cli`, `ssh_option_values_preserve_typed_daemon_and_cli_observations` | Real daemon plus real CLI, both modes: exact five human LF-terminated lines or exactly four JSON fields plus LF; empty stderr and exit zero for all valid observations. |
| Migration and harness unchanged; source files <=500 lines | Independent source-inventory.json; all command before/after inventories | Byte comparisons with baseline for 0001_phase0.sql and scripts/harness/validate.sh. Complete tracked/untracked Rust source/test inventory records hashes, lengths and modes. New review Python helpers also <=500 lines. |
| Exact required gates and unchanged harness | fmt.json, clippy.json, workspace-tests.json, harness.json and respective complete .txt logs | Three Cargo gates pass on identical sources. The harness's actual missing test:coverage failure is retained as unresolved; it is not a passing coverage result. |

The 68 shared successful-observation fixtures comprise 45 original contract cases,
seven disposition cases, two precedence variants and fourteen CLI literals. Ten
malformed payload cases are separate. Shared expectations do not replace independent
inspection: this review read the scanner, parser, rule dispatch, budgets, wire types,
daemon lifetime/admission/authentication paths and assertions themselves.

These executed tests establish the listed assertions on macOS. The repaired slice passes the source-bound behavioral checks below; the prior HIGH SSH finding is resolved for this slice.
Remaining detector catalogs/dialects/scripts/actual locality resolution, scheduling,
managed-execution, desktop, native Windows and coverage acceptance are separate.
No browser, GUI, screen check, standalone build or dependency installation was used.


## Supplemental repaired SSH verification

Command: python3 .arc-verify/command-classification/verification/ssh-repair.py.
The independent probe retains all 17 original inputs and adds 15 focused controls.
It checks every four-field result against an independently stated expectation,
protocol 1, request correlation, successful observation exit, empty stderr, exact
five-line human output, one-line JSON and clean owned-daemon SIGTERM/socket cleanup.
Complete actual results, argv, binary/source hashes, context and timestamps are
in ssh-repair.json; combined stdout/stderr is ssh-repair.txt. These commands are
synthetic data submitted to IPC or arcctl, never evaluated by a shell.

| Exact executed executable/test | Cases and assertions |
| --- | --- |
| arc-detector/lib `tests::ssh_post_destination_matrix` | All 99 permanent cases; exact typed output, consistent combinations and repeated result/counter equality within 65536 work. |
| arc-detector/lib `tests::ssh_post_destination_work_checks_refuse_before_overrun` | Six representative recognition paths consume their measured work exactly; one-short budgets refuse before overrun. |
| arc-ipc/classification `ssh_post_destination_typed_dispatch` | All 99 cases; protocol 1, request correlation, typed fields and a panic callback proving doctor is unused. |
| arc-daemon/classification `ssh_post_destination_daemon_human_and_json_cli_matrix` | All 99 cases through actual daemon and both actual CLI modes; exact fields/output, successful exit and empty stderr. |
| Independent ssh-repair.py | All 17 retained cases plus 15 controls through actual protocol-1 IPC and both actual CLI modes, independently asserting the repaired result. |

| Retained probe IDs | Exact expected result: classification / locality / reason / uncertainty |
| --- | --- |
| post-n, post-g, post-n-with-command, post-g-with-command, post-l, post-r, post-d, post-w, post-quoted-n, compound-n, compound-g | UNKNOWN / UNKNOWN / SSH_AMBIGUOUS / OBSERVE |
| pre-n-control | UNKNOWN / UNKNOWN / SSH_FORWARDING / OBSERVE |
| opaque-word-control, opaque-quoted-command-control, terminated-before-destination-control, terminated-after-destination-control | REMOTE_EXEC / REMOTE / SSH_REMOTE_COMMAND / NONE |
| quoted-local-argument-control | TEST / HOST_LOCAL / CARGO_TEST / NONE |

Additional probe controls cover -- without a command before/after the destination,
quoted -- and quoted flags, empty words hiding flags, leading empty words with
explicit termination, opaque separated/quoted remote arguments, uncertain compounds
and an explicitly terminated local+remote MIXED compound. The permanent 99-case
matrix includes these categories and the 17 exact retained inputs. No product tests
or expectations were changed by QA.

The old ssh-post-destination.json/.txt and script retain the pre-repair failures;
they are historical, not current acceptance receipts. attempt-3-evidence.tar.gz
preserves all preceding review/verification files before this refresh.
