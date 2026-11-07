# Implementation requirement-to-test map

This is task-4 implementation evidence, not the independent task-5 review. The exact required workspace gate remains failing until the environment permits the required loopback TCP monitor. No test is ignored or conditionally disabled. The earlier focused `--skip` run was diagnostic only and cannot satisfy acceptance. Final command receipts and source hashes are listed in summary.json.

## Typed classification and literal syntax

| Requirement | Deterministic checks |
| --- | --- |
| cargo build / cargo test / cargo clippy / cargo fmt remain distinct | contract.json case-1 through case-4; arc-detector delivered_contract_matrix; arc-ipc classification_contract_dispatch_never_calls_doctor; actual_daemon_and_cli_delivered_contract_matrix runs all 45 cases in IPC plus human and JSON CLI subprocesses |
| gh workflow run / gh run watch / gh run view -> REMOTE_CI | case-5 through case-7, same three layers; no GitHub SDK or endpoint API |
| SSH remote command -> REMOTE_EXEC; forwarding and ambiguous modes remain UNKNOWN | case-8 and case-18 through case-20; syntax_quotes_aggregation_and_ssh_negative_matrix also covers -N/-R/-D/-W/-p, --, empty remote words, user@host and loopbacks; numeric IPv6 matrix rejects malformed and loopback representations without a resolver |
| Resolved local/remote compounds -> MIXED; unknown stages never disappear | case-9 through case-13, case-40/41; unit matrix also checks heterogeneous local grouping followed by remote syntax, wrapper flattening and remote-CI/remote-exec ambiguity |
| Quoted command-like arguments create no invocation | case-14/15/16, case-34/35; literal_payloads_and_options_are_preserved_in_one_request; escaped operators, quotes, dollar, backticks, CRLF, line continuations, empty nested shells and quoted keywords in negative units |
| Unresolved scripts, wrappers, variables, active substitutions and Docker endpoints -> UNKNOWN/OBSERVE | case-21 through case-28, case-31/32/33, case-37/38; private PATH traps and substitution markers; active outer substitutions invalidate analysis, single quotes remain literal, nested literal shell data is rescanned without starting a shell |
| Unsupported shells and malformed syntax do not become operational failures | case-29/30/36/39/42 through case-45; empty input and whitespace are completed observations under task-1 contract |
| Repeatability and malformed Unicode/syntax robustness | all fixtures classified twice; 4096 fixed-seed generated Unicode/control/operator fragment strings checked for deterministic consistent results, capped counters and no panic |

## Input and work bounds

Production caps are 16384 UTF-8 command bytes, 1024 emitted word/operator tokens, 128 simple invocations including shell wrappers, active nesting depth 8, and 65536 charged work units. There is no mutable production limit or new profile.

`input_unicode_tokens_invocations_and_depth_boundaries` isolates inclusive and exceeded byte, Unicode, token, invocation, group, shell-wrapper and combined active-path depth limits. `work_counter_refuses_before_operations_and_never_wraps` consumes exactly 65536 units, refuses the next before a sentinel operation, and checks usize overflow rejection. `production_work_exhaustion_trace` uses two literal wrappers around a 12000-byte argument; counters remain shared and exhaustion occurs below every other cap. The exact trace is captured in the source-audit detector log; the generator is in source. `detector_budgets_are_preserved_through_daemon_and_cli` repeats 11 generated limit cases through actual daemon IPC and both CLI modes. Wire output exposes only fixed result fields, not counters, offsets or source text.

## Compatibility and operational behavior

| Requirement | Checks |
| --- | --- |
| Protocol 1 and version-before-payload validation | IPC malformed_payload_version_priority_and_legacy_envelopes, daemon invalid_payloads_version_priority_and_same_connection_recovery, unchanged dispatch regressions; unknown envelope keys remain ignored and malformed classification payload keys rejected |
| Exact status / doctor output and semantics | existing resident/lifecycle.rs unchanged; fixture.assert_cli after classifications checks both actual subprocesses; doctor retains classification=not_checked |
| Same-user authentication and filesystem identity | unchanged platform.rs and server/instance/identity tests, including foreign UID and socket path ownership/mode rejection |
| Existing admission/deadline accounting | unchanged 100000-attempt bounded test and paused-time/admission units; classification saturation uses default 64 connections, 16 requests, DB queue 8; report assertions check all high-water bounds and zero terminal gauges |
| Health responsiveness under maximum/work-exhausting classification | maximum_and_exhausting_classification_load_preserves_health_and_shutdown drives 16 owned load threads plus ten actual health exchanges; bounded retry permits admission rejection but requires health success within two seconds |
| SIGTERM shutdown | same saturation test checks signal-to-exit <=3500 ms and retained task/worker/socket accounting; existing SIGINT/SIGTERM and delayed-DB lifecycle suites retained |
| Database independence | IPC doctor closure panics if invoked by classification; daemon privacy/malformed/saturation reports show zero database high-water use; only explicit doctor creates DB work in the main matrix |
| CLI failures and privacy | help_usage_privacy_and_absent_daemon; 15 literal/option-position fake-peer exchanges; 16 malformed/error/result/correlation cases followed by fresh valid invocations; bounded timeout and premature EOF; no preliminary status/doctor exchange or automatic retries |

## No execution, endpoint contact or persistence

The Unix-monitor privacy test starts only owned ARC daemons, arcctl clients and a test listener. Controlled PATH contains private executable traps named cargo, gh, ssh, docker, docker-compose, sh, bash, zsh, touch and curl. Twelve input families carry unique synthetic secrets and command-substitution marker paths. Assertions cover typed responses, both CLI modes, zero tool-trap/marker creation, zero monitored endpoint connections, and recursive byte scans of all regular fixture artifacts before/after each classification and after shutdown. A read-only SQLite transaction keeps relevant WAL state available while main/WAL/SHM, daemon stdout/stderr and runtime output are scanned. Only case counts and outcomes enter retained receipts; generated secret values and full generated commands are never retained there.

The separate required_loopback_endpoint_nonexecution_and_privacy test runs the same exercise with a loopback TCP monitor. It is retained as a mandatory test. It currently fails at TcpListener::bind with errno 1, Operation not permitted. Therefore TCP endpoint-contact acceptance and the exact workspace-test gate remain unmet. Unix endpoint evidence and static inspection do not substitute for that required outcome.

Static support: arc-detector normal dependencies contain only existing workspace Serde. Its modules perform allocation, numeric/string parsing and bounded enum aggregation; they import no process, filesystem, environment, logging, storage, socket or resolver APIs. The custom numeric IPv6 parser avoids resolver libraries. arc-ipc dispatch calls the pure analyzer after its version guard. connection.rs computes the classification response synchronously, clears payload, then reaches response I/O; classification cannot enter the doctor closure or DbWorker. CLI uses existing Unix IPC and writes only validated enum results/static errors. This is implementation inspection; independent QA remains assigned separately.

## Input-artifact conflicts retained for Chief

contract.md's concrete five-line `Classification:` human format conflicts with cli-requirements.md's `Class:` and extra UNKNOWN explanation. Contract empty/whitespace and input-limit observations conflict with task-2 C03/C15 error expectations. Implementation follows the concrete task-1 contract on these points and preserves its typed JSON, enum, payload and counter rules. No input requirements were edited. Chief must reconcile these interface conflicts; this report does not claim every contradictory task-2 row passes.

## Scope

This is one detector slice. Broader detector coverage, scheduling, managed-execution, desktop and native Windows acceptance are not delivered. The unchanged harness coverage failure remains unresolved; no dummy coverage task or skip setting is introduced. Independent review, final mission evidence and final acceptance belong to the supervising Chief and reviewer.
