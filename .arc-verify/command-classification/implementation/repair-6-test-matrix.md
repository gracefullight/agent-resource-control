# Attempt 6 requirement-to-test map

Implementation-owner reconciliation using oma-backend. The actual amended contract.md,
contract.json and synchronized cli-requirements.md are the inputs, with all 77 source
and specification hashes unchanged since attempt 5. No product code, expectations,
input specifications or reviewer artifacts changed. See repair-6-source-inventory.json.

W means fresh cargo test --workspace, exit 0 (repair-6-workspace.json/.txt).
T means the fresh unchanged required TCP privacy test, exit 0 (repair-6-tcp.json/.txt).
Both receipts contain exact argv, complete output hashes and before/after source hashes.
The operator's environment change was checked with a successful local TCP bind/listen
before either test; repair-6-tcp-prerequisite.json supersedes the historical denial
for this execution context. Previous failing receipts remain unchanged.

## Shared deterministic matrix

The shared fixture loader in crates/arc-detector/tests/support/contract.rs supplies
68 successful observations: 45 original fixtures, seven disposition fixtures, two
precedence variants and fourteen exact CLI literals. Ten malformed payload fixtures
are separate. Detector units, IPC dispatch and actual daemon/human/JSON CLI consume
the same literal data. No fixture input is evaluated as a shell command.

| Required family | Assertions and source | Evidence |
| --- | --- | --- |
| cargo build, cargo test, cargo clippy, cargo fmt | Distinct BUILD/TEST/LINT and CARGO_BUILD/CARGO_TEST/CARGO_CLIPPY/CARGO_FMT | W |
| gh workflow run, gh run watch, gh run view | REMOTE_CI/REMOTE/NONE and distinct reasons | W |
| SSH remote execution | REMOTE_EXEC/REMOTE; forwarding and options uncertain; opaque option values preserved | W |
| Local/remote compounds | MIXED/MIXED; group/wrapper flattening; unknown stages invalidate resolution | W |
| Quoted command-like text | Arguments never become invocations; quoted operators, substitutions, Unicode and escapes | W |
| Scripts, expansions, Docker | UNKNOWN/OBSERVE without script reading, evaluation or endpoint resolution | W, T |
| Five inclusive limits | 16384 bytes, 1024 tokens, 128 invocations, depth 8, 65536 work; charge before operation and checked overflow | W |
| Reconciled dispositions | Successful empty/oversized/unsupported-shell observations; byte then shell then empty precedence | W |

## CLI requirements C01 through C26

Source checks below were compared to the synchronized requirements. W includes
fake-peer CLI tests and all seven actual-daemon classification integration tests.

| ID | Source check / assertion | Evidence |
| --- | --- | --- |
| C01 | arc-cli/tests/classification.rs help_usage_privacy_and_absent_daemon: seven examples, quoting guidance, no IPC for help/usage; binary.rs no-args regression | W |
| C02 | literal_payloads_and_options_are_preserved_in_one_request: exact bytes, sh default, bash/zsh options before/after argument; shared shell observations | W |
| C03 | Same help/usage test: missing/extra/unknown/missing-shell failures are input-free; shared explicit empty/whitespace/empty+unsupported observations | W |
| C04 | Literal payload test and shared cases preserve leading/trailing whitespace and actual tabs | W |
| C05 | Same payload test preserves Korean/Greek UTF-8; shared exact literal and Unicode boundary units | W |
| C06 | Four Cargo fixtures compare all result fields and distinguish clippy/fmt reasons | W |
| C07 | Three gh fixtures require REMOTE_CI with fixed reasons | W |
| C08 | SSH execution, forwarding, missing command and localhost; numeric IPv6 and opaque option-value regressions | W |
| C09 | Shared MIXED, same-locality, grouped heterogeneous and unresolved-stage aggregation cases | W |
| C10 | Quoted gh/SSH text remains Cargo arguments | W |
| C11 | printf-like/quoted executable unresolved; only exact literal bash -lc recurses | W |
| C12 | Script/package runner/expansion/Docker uncertainty | W |
| C13 | Active/double-quoted/nested substitutions unresolved; single-quoted data literal; executable and substitution markers checked separately | W |
| C14 | Malformed quotes/operators/unsupported syntax; 4096 fixed-seed generated inputs never panic | W |
| C15 | Inclusive Unicode byte limit; oversized ASCII/multibyte/whitespace and shell precedence with zero scan counters; existing frame cap units | W |
| C16 | Token/invocation/group/wrapper/combined-depth bounds; shared production work counter, refusal and overflow trace | W |
| C17 | Ten malformed typed payloads vs successful INPUT_LIMIT; fixed fake-peer INVALID_REQUEST | W |
| C18 | Version-first rejection with malformed payload; fake-peer incompatible protocol remains failure | W |
| C19 | Fake-peer UNSUPPORTED_CAPABILITY; no preliminary status/doctor or retry | W |
| C20 | Absent socket, premature EOF, malformed response and bounded timeout | W |
| C21 | Sixteen invalid peer result/error/status/correlation fixtures; static errors never reflect remote secret text | W |
| C22 | Each failed peer request followed by one fresh successful process | W |
| C23 | Complete five-line LF-terminated human bytes for known/UNKNOWN/MIXED/dispositions | W |
| C24 | Four-field JSON and exactly one final LF; no envelope or extra text | W |
| C25 | fixture.assert_cli and resident/lifecycle.rs preserve exact status/doctor strings and protocol 1 | W |
| C26 | privacy.rs executable traps, substitution markers, endpoint monitors and secret scans across responses/errors, CLI/daemon output, reports, SQLite/live WAL/SHM | W, T |

## Preservation and negative behavior

W includes protocol 1, unchanged dispatch(request, doctor), version-first rejection,
status and doctor exact output, same-user authentication policy, existing admission,
frame/deadline bounds, database-independent health and shutdown regressions. The
classification saturation test exercises health and malformed same-connection recovery
under continuing maximum/exhausting traffic, zero database work and SIGTERM within
3500 ms. Native Windows is not exercised by these Unix tests.

W and T execute privacy.rs without modification. Twelve data families exercise private
executable traps, active/literal/nested/backtick substitution markers, monitored Unix
and TCP endpoints, input-free responses/errors, CLI/daemon output, runtime reports,
SQLite and live WAL/SHM scans. No tool starts, marker creation, monitored endpoint
contacts or persisted canaries are allowed by these assertions. Static pure-module
inspection supports this negative evidence; it does not establish all possible inputs.

The exact fmt and clippy gates pass; the unchanged harness exits 1 for missing
test:coverage, without SKIP_COVERAGE. Historical failures remain historical. Independent
oma-qa and final mission acceptance belong to the next Chief-coordinated handoff.
Remaining detector coverage, scheduling, managed-execution, desktop and native Windows
acceptance and harness coverage remain open.
