# Reconciliation and static inspection

Task-4 attempt 4; skill: oma-backend. This is implementation-owner evidence, not
independent oma-qa. Source hashes and lengths are in repair-4-source-inventory.json.
The actual input artifacts were read, including the complete amended contract pair,
synchronized CLI requirements, original map and repair-3 handoff. No repository text
was used to change the assignment or its acceptance criteria.

## Contract reconciliation

The selected Chief disposition is now present in both delivered specifications.
Empty/whitespace input, analysis input above 16384 UTF-8 bytes, exhausted analysis
budgets and unsupported/empty shell strings are successful UNKNOWN/OBSERVE results.
The byte check precedes shell resolution; shell resolution precedes empty recognition.
Human output is exactly five LF-terminated lines beginning Classification:. JSON is
exactly four typed fields plus LF. Optional --shell defaults to sh. Typed-request,
usage and transport failures remain failures. The implementation already has these
semantics; none of that working logic was rewritten.

The only product change this attempt adds the three newly required literal help
examples to crates/arc-cli/src/main.rs. The original four examples and caller-shell
quoting explanation remain. A strengthened subprocess assertion reproduced the
missing examples before the change, then passed after it.

The original test loops consumed only contract.json fixtures, omitting the newly
added dispositionFixtures array. The new shared test-only loader constructs the
three oversized inputs from an explicit allowlist of Rust string generators, checks
their byte lengths against the artifact and returns all original plus disposition
cases. It never evaluates expressions. Two precedence and 14 exact CLI matrix
literals supplement those inputs. All 68 cases run at unit, typed-dispatch and real
daemon/CLI layers; the CLI matrix checks both human and JSON modes. INPUT_LIMIT
unit cases require default zero counters. The loader is under tests/support and
adds no normal dependency or runtime path.

## Preserved implementation boundaries

| Path | Inspection result |
| --- | --- |
| crates/arc-detector/Cargo.toml | Normal dependency remains existing Serde only; serde_json is test-only |
| crates/arc-detector/src/lib.rs | Original byte precheck, shell resolution and root aggregation preserved; returns enums and deterministic counters |
| crates/arc-detector/src/lexer.rs | In-memory quote/escape-aware scanning; active expansions fail closed; no shell invocation |
| crates/arc-detector/src/parser.rs | Bounded grouping and exact literal shell-wrapper recursion; one shared budget; arbitrary argument and SSH remote text is opaque |
| crates/arc-detector/src/budget.rs | Same five inclusive caps, checked pre-operation work charge and overflow refusal |
| crates/arc-detector/src/rules.rs | Retained SSH option-value repair and numeric IPv6 parser; no resolver or endpoint probing |
| crates/arc-detector/src/types.rs | Same closed class/locality/reason/uncertainty enums, strict payload fields and complete result-combination validation |
| crates/arc-ipc/src/lib.rs | Protocol 1, public dispatch(request, doctor), version-first rejection, fixed failures and framing unchanged this attempt |
| crates/arc-daemon/src/connection.rs | Classification remains synchronous under existing request admission; payload discarded before await; no DbWorker path |
| crates/arc-cli/src/classify.rs | Same one-request Unix IPC exchange, correlation/status/version/result validation and static errors; no input echo |
| crates/arc-daemon/src/limits.rs and platform.rs | Existing admission, deadlines, native same-user checks and shutdown controls unchanged |

The detector contains no process, filesystem, environment, networking, resolver,
logging or storage calls. Its dependencies and code path support the behavioral
negative probes. CLI and daemon naturally use local IPC; test fixtures start owned
ARC processes and listeners. Those infrastructure effects are distinguished from
executing analyzed command text or contacting an analyzed endpoint. No command or
command-derived hash telemetry was added. Transient disposal is not secure memory
erasure and no such claim is made.

The retained Unix privacy exercise checks twelve secret-bearing input families,
private executable traps, active and literal substitution markers, zero endpoint
connections, typed responses, CLI outputs/errors and recursive fixture artifacts.
It keeps a read-only SQLite transaction open while checking main/WAL/SHM, daemon
logs and runtime reports, then checks again after shutdown. The separate mandatory
TCP exercise is preserved byte-for-byte; its assertions were not executed here.

## Failures and execution limits

A normal exec_command Python AF_INET bind/listen prerequisite probe on 127.0.0.1:0
returned PermissionError, errno 1, Operation not permitted. No supported context
with effective permission was established. No TCP classification test or exact
cargo test --workspace gate was repeated. The earlier failure is prerequisite
evidence, not an observed classifier endpoint contact. No sandbox policy, provider,
GUI, test skip, test assertion or external runner was used as a workaround.

The first daemon matrix compilation found an output assertion placed before the
output binding (E0425). It was moved to the existing post-invocation JSON branch;
the failed receipt is retained and subsequent final-source checks pass. One patch
context failed after formatting the shared loader; it changed no files and was
reapplied after reading the formatted file. No test expectation was weakened.

Serena discovery returned no tools. All code reads used already known paths; no
native repository code search was substituted. The memory quick pass found no ARC
entry and supplied no implementation facts. Global Git status reported av.yaml:
Operation not permitted despite exit zero; that path was not bypassed. Python
startup emitted the existing mise tracking-link warning while commands completed.
No installations, dependency downloads, standalone builds, browser/screen checks,
subagents, AV dispatches, mission-state writes or external delivery occurred.

## TDD_EVIDENCE

- Test command: cargo test -p arc-cli --test classification help_usage_privacy_and_absent_daemon -- --exact
- RED: repair-4-help-red.json/.txt, exit 101, missing help term with original product help.
- GREEN: repair-4-help-green.json/.txt, exit 0 after adding the three examples.
- Final-source confirmation: repair-4-reconciled-unit-ipc-cli.json/.txt runs the complete CLI target plus detector and IPC checks, exit 0.

The additional listener assertion proves help/usage made no IPC connection. Exact
shell bytes/defaults, supported shell flags after the positional input and literal
flag-shaped data after -- are also covered without invoking a shell.
