# Reconciled requirement-to-test map

Task-4 attempt 4 implementation evidence. The input specifications are the actual amended
contract.md, contract.json and synchronized cli-requirements.md. Their bytes and SHA-256
values are in repair-4-source-inventory.json. Earlier maps and receipts remain historical.
No specification, reviewer artifact, assignment or verification command was edited.

The shared literal-fixture loader at crates/arc-detector/tests/support/contract.rs feeds
the same 68 successful observations to detector units, typed IPC dispatch and the real
daemon plus both arcctl output modes: 45 original cases, all seven disposition cases,
two explicit byte/shell/empty precedence variants, and 14 exact CLI matrix literals
(including flag-shaped data after the option delimiter). Generators use fixed Rust string
construction, never expression evaluation. Ten malformed payload fixtures remain intact.
Final-source command receipts identify executed tests and full logs; this map is not an
independent review or a passing workspace gate.

## Command families and conservative results

| Requirement | Source and deterministic checks |
| --- | --- |
| cargo build / cargo test / cargo clippy / cargo fmt | Shared original cases 1–4 compare all four fields at unit, IPC, daemon and CLI layers; clippy/fmt are LINT with distinct reasons |
| gh workflow run / gh run watch / gh run view | Shared cases 5–7 require REMOTE_CI/REMOTE/NONE and three distinct bounded reasons |
| SSH -> REMOTE_EXEC, forwarding and ambiguity -> UNKNOWN | Original remote/forwarding/loopback cases; retained numeric IPv6 and SSH option-value unit/IPC/daemon/CLI repair matrices |
| Safely resolved local/remote compounds -> MIXED | Original compound cases, source-order flattening, heterogeneous groups and literal shell-wrapper units; unresolved third stage invalidates the whole result |
| Quoted command-like arguments | Original cases 14–16 and 34–35; escaped operator/dollar/backtick/CRLF and quoted keyword units; fake-peer byte-preservation checks |
| Scripts, expansions, Docker endpoints -> UNKNOWN/OBSERVE | Original script, active/quoted/nested substitution and Docker cases; no file, environment, resolver or endpoint discovery |
| Byte, token, invocation, depth and work limits | Unit inclusive/exceeded/overflow/refused-operation checks; retained 11-case real-daemon and human/JSON CLI boundary matrix; new ASCII/multibyte/whitespace oversized disposition cases |
| Amended successful observations | All seven disposition fixtures plus empty+unsupported-shell and oversized+unsupported-shell ordering variants; pre-scan INPUT_LIMIT asserts default counters |

## Synchronized CLI matrix C01–C26

| ID | Source/check and evidence scope |
| --- | --- |
| C01 | help_usage_privacy_and_absent_daemon now asserts all seven literal examples and quoting guidance; a bound nonblocking Unix listener proves help/usage sent no request; red/green receipts retained |
| C02 | literal_payloads_and_options_are_preserved_in_one_request checks exact bytes, default sh, zsh before and bash after positional input; real shared cases cover explicit sh/bash/zsh, unsupported and empty shell |
| C03 | Missing/extra/unknown-option/missing-shell usage failures are input-free and make no request; explicit empty/whitespace and empty+unsupported-shell observations run through all shared layers |
| C04 | Leading/trailing spaces and actual tab preserved by fake peer and classified by shared/unit fixtures |
| C05 | UTF-8 Korean/Greek payload preservation; Unicode analyzer boundaries and deterministic generated syntax units |
| C06 | Shared cases 1–4 require distinct Cargo class/reason combinations |
| C07 | Shared cases 5–7 require typed REMOTE_CI without GitHub contact |
| C08 | Shared SSH execution/forwarding/missing/localhost fixtures and retained SSH repair matrices |
| C09 | Shared local/remote, heterogeneous-local, same-class local/remote and unresolved-stage compound fixtures |
| C10 | Quoted gh/ssh arguments remain Cargo arguments, with no false invocation |
| C11 | Quoted executable/printf-like unresolved words remain UNKNOWN; only exact literal shell wrappers recurse |
| C12 | Script, package-runner, expansion and Docker uncertainty in shared fixtures |
| C13 | Shared active/double-quoted, single-quoted and nested substitution fixtures; Unix privacy executable traps and marker assertions |
| C14 | Shared malformed quote/operator/unsupported syntax observations and 4096 deterministic generated inputs |
| C15 | Inclusive byte boundary and 16385-byte ASCII/multibyte/whitespace outcomes through real IPC/CLI; unsupported-shell precedence variant; unchanged IPC framing oversize units and existing daemon bounded-frame tests verify the transport boundary. CLI OS argv limits are not a substitute for transport assertions |
| C16 | Unit exact work/refused-operation/overflow and shared nested counters; actual daemon/CLI end-to-end byte/token/invocation/depth/work exhaustion fixtures |
| C17 | Ten malformed payloads through dispatch and real daemon, fixed fake-peer INVALID_REQUEST failure; valid INPUT_LIMIT succeeds with exact typed result |
| C18 | Version-first rejection against every malformed payload; fake-peer version/error fixtures remain failures |
| C19 | Fake peer UNSUPPORTED_CAPABILITY remains distinct from successful uncertainty; listener prohibits preliminary status/doctor or retry connections |
| C20 | Absent socket, premature EOF, malformed frame and bounded timeout fake-peer checks |
| C21 | Sixteen malformed/error/correlation/status/field-combination fixtures reject untrusted results/messages; typed consistency is checked before display |
| C22 | Each failed fake-peer invocation is followed by one fresh successful invocation |
| C23 | Real shared and budget matrices compare complete five-line human bytes including LF, for known/UNKNOWN/MIXED and all new dispositions |
| C24 | Same matrices compare the exact four-field JSON object; shared real-daemon cases additionally require exactly one LF-terminated line |
| C25 | Existing resident lifecycle assertions and matrix fixture.assert_cli retain protocol-1 status and doctor text, including classification not_checked |
| C26 | Retained Unix executable traps, substitutions, monitored endpoint, unique secret, responses/errors/output/reports/SQLite/live WAL/SHM scans pass. Required TCP version remains unexecuted this attempt because the separate prerequisite probe is denied; this row is incomplete |

The matrix records the supported contract behavior and its deterministic checks; it does
not imply complete acceptance when a required negative-behavior test cannot execute.

## Compatibility and resource controls

Protocol 1 and dispatch(request, doctor) are preserved. Same-user authentication remains
the native platform check. Existing connection/request admission (64/16), queue bounds,
absolute deadlines and retained task/worker shutdown accounting are unchanged. Dedicated
compatibility receipts run daemon unit, resident and bounded tests. Saturation separately
requires health responsiveness, malformed same-connection recovery during continuing load,
database-independent classification and SIGTERM completion within 3500 ms. Its measured
fixture counters are stored in repair-4-observations, not generalized to production.

## Execution prerequisite and remaining scope

repair-4-tcp-prerequisite.json records errno 1 / PermissionError / Operation not permitted
for a normal noninteractive 127.0.0.1:0 bind/listen probe. No supported context with effective
TCP permission was established. The required_loopback_endpoint_nonexecution_and_privacy
test and exact cargo test --workspace gate were therefore not repeated, per assignment.
The prior failing workspace and TCP receipts remain unchanged and historical; the Unix
result and static inspection cannot certify the missing TCP assertions. Chief should stop
identical retries until a permitted noninteractive execution context is supplied.

The unchanged harness is run with coverage enabled and its actual exit retained. Missing
test:coverage remains unresolved. Broader detector coverage, scheduling, managed-execution,
desktop and native Windows acceptance remain open; independent review and final mission
delivery belong to their assigned owners.
