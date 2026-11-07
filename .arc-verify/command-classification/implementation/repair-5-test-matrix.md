# Attempt 5 requirement-to-test map

Task-4 implementation-owner reconciliation using oma-backend. Inputs are the actual
amended contract.md, contract.json and synchronized cli-requirements.md. All 77
inputs match attempt 4, including these three artifacts. No specification or test
expectation was changed. Hashes and lengths are in repair-5-source-inventory.json.

Evidence labels used below:

- F: fresh attempt-5 executable receipt on these inputs.
- R: retained attempt-4 receipt, independently hash-compared to these same inputs
  and its complete log; not rerun this attempt.
- X: required execution incomplete. Neither static inspection nor Unix results
  replace TCP assertions or the exact workspace gate.

F-unit = repair-5-unit-ipc-cli.json/.txt: cargo test -p arc-detector -p arc-ipc
-p arc-cli. F-daemon = repair-5-daemon-matrix.json/.txt: cargo test -p arc-daemon
--test classification actual_daemon_and_cli_delivered_contract_matrix -- --exact.
Both compare typed results; the actual daemon invokes the actual arcctl binary.
All fresh checks have unchanged before/after input hashes.

## Shared deterministic matrix

crates/arc-detector/tests/support/contract.rs supplies 68 successful observations:
45 original fixtures, seven amended disposition fixtures, two precedence variants
and fourteen exact CLI literals. Ten malformed payloads are separate. The same
matrix runs in detector units, IPC dispatch and actual daemon plus human/JSON CLI.
Oversized generators are a fixed Rust allowlist, never evaluated expressions.

| Required family | Assertions and source | Evidence |
| --- | --- | --- |
| cargo build, cargo test, cargo clippy, cargo fmt | BUILD/TEST/LINT with distinct CARGO_BUILD/CARGO_TEST/CARGO_CLIPPY/CARGO_FMT reasons | F-unit, F-daemon |
| gh workflow run, gh run watch, gh run view | REMOTE_CI/REMOTE/NONE with operation-specific reason | F-unit, F-daemon |
| SSH remote execution | REMOTE_EXEC; opaque remote data, loopback/option uncertainty, forwarding and retained option-value repair | F-unit; R daemon-ssh |
| Local/remote compounds | MIXED; flattened groups/wrappers; unknown third stage invalidates the whole result | F-unit, F-daemon |
| Quoted command-like text | Cargo arguments create no invocation; quoted executable remains unresolved; escaping and Unicode preserved | F-unit, F-daemon |
| Scripts, expansions, Docker | UNKNOWN/OBSERVE; active/nested expansions differ from single-quoted literal data; endpoints never resolved | F-unit, F-daemon; R daemon-unix-privacy; X TCP |
| Five inclusive limits | 16384 bytes, 1024 tokens, 128 invocations, depth 8, 65536 work; pre-operation rejection and checked overflow | F-unit; R daemon-bounds |
| Amended dispositions | Empty/whitespace, oversized ASCII/Unicode/whitespace and unsupported/empty shell succeed; byte then shell then empty precedence | F-unit, F-daemon |

## CLI requirements C01 through C26

All rows were compared with the synchronized CLI artifact, including its exact
five-line human template and four-field JSON object. Source functions are named
to permit review without treating this map as an independent QA verdict.

| ID | Source check / assertion | Evidence |
| --- | --- | --- |
| C01 | arc-cli/tests/classification.rs help_usage_privacy_and_absent_daemon: seven examples, quoting guidance, no IPC for help/usage; binary.rs no-args regression | F-unit |
| C02 | literal_payloads_and_options_are_preserved_in_one_request: exact bytes, sh default, bash/zsh options before/after argument; shared shell observations | F-unit, F-daemon |
| C03 | Same help/usage test: missing/extra/unknown/missing-shell failures are input-free; shared explicit empty/whitespace/empty+unsupported observations | F-unit, F-daemon |
| C04 | Literal payload test and shared cases preserve leading/trailing whitespace and actual tabs | F-unit, F-daemon |
| C05 | Same payload test preserves Korean/Greek UTF-8; shared exact literal and Unicode boundary units | F-unit, F-daemon |
| C06 | Four Cargo fixtures compare all result fields and distinguish clippy/fmt reasons | F-unit, F-daemon |
| C07 | Three gh fixtures require REMOTE_CI with fixed reasons | F-unit, F-daemon |
| C08 | SSH execution, forwarding, missing command and localhost; numeric IPv6 and opaque option-value regressions | F-unit, F-daemon; R daemon-ssh |
| C09 | Shared MIXED, same-locality, grouped heterogeneous and unresolved-stage aggregation cases | F-unit, F-daemon |
| C10 | Quoted gh/SSH text remains Cargo arguments | F-unit, F-daemon |
| C11 | printf-like/quoted executable unresolved; only exact literal bash -lc recurses | F-unit, F-daemon |
| C12 | Script/package runner/expansion/Docker uncertainty | F-unit, F-daemon |
| C13 | Active/double-quoted/nested substitutions unresolved; single-quoted data literal; executable and substitution markers checked separately | F-unit, F-daemon; R daemon-unix-privacy; X TCP |
| C14 | Malformed quotes/operators/unsupported syntax; 4096 fixed-seed generated inputs never panic | F-unit, F-daemon |
| C15 | Inclusive Unicode byte limit; oversized ASCII/multibyte/whitespace and shell precedence with zero scan counters; existing frame cap units | F-unit, F-daemon; R daemon-bounds and compatibility |
| C16 | Token/invocation/group/wrapper/combined-depth bounds; shared production work counter, refusal and overflow trace | F-unit; R daemon-bounds |
| C17 | Ten malformed typed payloads vs successful INPUT_LIMIT; fixed fake-peer INVALID_REQUEST | F-unit; R daemon-malformed |
| C18 | Version-first rejection with malformed payload; fake-peer incompatible protocol remains failure | F-unit; R daemon-malformed |
| C19 | Fake-peer UNSUPPORTED_CAPABILITY; no preliminary status/doctor or retry | F-unit |
| C20 | Absent socket, premature EOF, malformed response and bounded timeout | F-unit |
| C21 | Sixteen invalid peer result/error/status/correlation fixtures; static errors never reflect remote secret text | F-unit |
| C22 | Each failed peer request followed by one fresh successful process | F-unit |
| C23 | Complete five-line LF-terminated human bytes for known/UNKNOWN/MIXED/dispositions | F-daemon; R daemon-bounds |
| C24 | Four-field JSON and exactly one final LF; no envelope or extra text | F-daemon; R daemon-bounds |
| C25 | fixture.assert_cli and resident/lifecycle.rs preserve exact status/doctor strings and protocol 1 | F-daemon; R compatibility |
| C26 | privacy.rs executable traps, substitution markers, endpoint monitors and secret scans across responses/errors, CLI/daemon output, reports, SQLite/live WAL/SHM | R daemon-unix-privacy; X required TCP monitor |

R receipts use repair-4-reconciled-<name>.json/.txt where name is daemon-ssh,
daemon-bounds, daemon-malformed, daemon-unix-privacy, daemon-saturation or
compatibility. repair-5-source-inventory.json records receipt/log/input validation.
All preceding files, including failures, remain byte-identical.

## Compatibility, bounds and missing evidence

Protocol 1 and dispatch(request, doctor) remain compatible. connection.rs analyzes
synchronously within existing admission and disposes payload before await. Platform
same-user authentication, 64 connections/16 requests, queue 8, deadlines and 3500 ms
shutdown controls match baseline. R compatibility verifies these existing regressions;
R daemon-saturation covers health, malformed same-connection recovery during continuing
maximum/exhausting classification traffic, zero DB work and shutdown accounting.
These checks were not rerun because their source inputs are unchanged.

No effective local TCP-listening context has been established. The historical
repair-4-tcp-prerequisite.json records EPERM at 127.0.0.1:0. The current context
still declares network disabled. Per assignment, neither the known-denied test,
a duplicate bind probe nor cargo test --workspace was replayed. C26 TCP assertions
and the exact workspace gate remain incomplete. Chief should stop identical retries
until a supported noninteractive context permits local TCP listening.

The unchanged harness runs without a coverage skip; its actual result is retained
in repair-5-harness.json/.txt. test:coverage remains unresolved. This is one detector
slice; remaining detector coverage, scheduling, managed-execution, desktop and native
Windows acceptance are open. Independent oma-qa and final delivery remain assigned
to other Actors; this file does not certify them.
