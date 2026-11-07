# Attempt 6 implementation-owner inspection

Skill: oma-backend. This is the implementation owner's reconciliation, not independent
oma-qa. Direct reads confirm no concrete discrepancy requiring product edits. All 77
product/specification inputs match attempt 5, including the amended contract pair and
synchronized CLI requirements. Prior failed receipts remain unchanged.

## Contract and interfaces

The Rust 2021 workspace retains Serde, Tokio, Clap and SQLite. arc-detector has only
Serde as a runtime dependency and no process, filesystem, environment, resolver,
network, logging, storage or transport calls. A bounded quote-aware scanner and small
parser implement the documented grammar; no shell or external parser runs. Budget
counters are shared through wrappers/groups and charged before operations. Checked
addition refuses overflow and 65536-work overruns. Original UTF-8 byte precheck occurs
before shell resolution, then parsing/empty recognition.

The closed class/locality/reason/uncertainty enums and strict typed payload/result
remain in arc-detector and are re-exported through arc-ipc. dispatch(request, doctor)
is source-compatible and version-first. job.classify never calls the doctor callback.
connection.rs analyzes under existing admission and clears the payload before await;
classification does not enter DbWorker. Existing limits.rs and platform.rs match the
baseline byte-for-byte. Status and doctor output and command_classification=not_checked
are preserved by workspace regressions.

The CLI sends one literal UTF-8 argument with optional --shell and optional --json
through the existing local owner-only IPC. The caller's shell has already processed
its own quoting; tests pass substitution data with Command::arg. The CLI never starts
the analyzed tool. Successful observations have exactly five human lines or four
JSON fields and empty stderr. UNKNOWN/OBSERVE, empty, oversized and unsupported-shell
observations succeed; malformed payload, usage and transport failures remain failures.
Fixed errors and generated request IDs do not derive from command data. Incoming
result correlation, protocol, schema and enum combinations are validated.

## Concrete deterministic coverage

Read source: arc-detector src/lib.rs, types.rs, budget.rs, lexer.rs, parser.rs,
rules.rs, tests.rs and tests/support/contract.rs; arc-ipc tests/classification.rs;
arc-cli src/main.rs, classify.rs and tests/classification.rs; arc-daemon
connection.rs and classification.rs, classification/boundaries.rs, privacy.rs,
saturation.rs and shared runtime receipt helper. Existing scoped diffs were inspected.
The requirement map names the assertions and all 26 synchronized CLI rows.

The fresh full workspace run passed all 205 tests, including 8 detector tests,
3 IPC classification tests, 4 CLI classification tests and all 7 actual-daemon
classification integration tests. Shared fixtures compare all four fields for
68 successful observations and ten separate malformed payloads. Boundaries isolate
input/token/invocation/nesting/work limits without lowering production caps. Tests
also cover 4096 fixed-seed Unicode/syntax inputs and the retained SSH option repair.

The unchanged privacy tests exercise twelve canary-bearing data families against
private executable traps, active/literal/nested/backtick substitution markers, Unix
and TCP monitored endpoints, responses/errors, CLI/daemon output, runtime reports,
SQLite and live WAL/SHM. Both pass in the fresh workspace run; TCP also passes as an
unchanged exact focused test after a successful bind/listen prerequisite probe.
The checks combine pure call-path inspection with negative runtime assertions; they
do not claim exhaustive shell or runtime-topology coverage.

Classification saturation checks continuing maximum/exhausting traffic, malformed
same-connection recovery, health successes under existing deadlines, zero database
work and SIGTERM within 3500 ms. Workspace compatibility tests exercise existing
protocol, authentication policy, admission, health and lifecycle behavior.

## Environment and evidence limits

The operator note changes the execution prerequisite, not test expectations. A new
normal noninteractive TCP bind/listen succeeds. Historical PermissionDenied evidence
is retained but no longer treated as current. No sandbox change, runner workaround,
GUI, dependency installation, browser suite, standalone build, AV dispatch or mission
state mutation was used. All checks run serially with a 120-second command bound
and CARGO_NET_OFFLINE=true.

The recorder set ARC_RUNTIME_EVIDENCE_DIR, which the existing fixture helper does not
consume (it uses ARC_TEST_EVIDENCE_DIR). No new fixture metric JSON export is claimed.
Complete Cargo logs and passing assertions are the fresh behavior evidence. Existing
runtime observations remain historical; no timing values are inferred from them.

Serena tools were unavailable; known-path reads and the existing input inventory
were used without native code search. The memory lookup yielded no ARC facts.
Global Git status reports av.yaml access denial and shell startup reports a mise
tracking-link warning. Neither was bypassed or repaired; required checks ran.

The unchanged harness result is retained separately, with coverage unresolved.
Independent oma-qa, final mission verification, remaining detector coverage,
scheduling, managed-execution, desktop and native Windows acceptance remain open.
