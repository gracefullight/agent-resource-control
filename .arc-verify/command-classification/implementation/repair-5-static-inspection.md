# Attempt 5 reconciliation inspection

Skill: oma-backend. Status: implementation-owner inspection, not independent QA.
No product code, tests, input specification or reviewer artifact changed this attempt.
The Rust 2021/Serde/Tokio/Clap/SQLite workspace and dependencies are retained.

## Reconciled interfaces

The actual contract pair and synchronized CLI artifact were consumed. All 77 inputs
are byte-identical to attempt 4. Empty/whitespace commands and exhausted budgets
produce successful UNKNOWN/OBSERVE results. Byte precheck precedes shell resolution,
which precedes empty recognition. Optional --shell maps directly to shell_kind with
sh default. Unsupported/empty shell strings are successful observations. Missing or
wrong typed fields, usage errors and transport failures remain failures. Human output
has exactly five lines beginning Classification:; JSON has exactly four result fields.
There is no remaining concrete discrepancy requiring a product rewrite.

Source-bound observations from direct reads:

| Source | Observed boundary |
| --- | --- |
| arc-detector/src/lib.rs | Original UTF-8 byte precheck returns default counters on INPUT_LIMIT; shell comparison precedes parse; one root aggregation over source-order effective leaves |
| arc-detector/src/types.rs | Closed enums, deny_unknown_fields request/result, string shell default, complete result consistency predicate |
| arc-detector/src/budget.rs | Inclusive five caps, checked_add work charge before operation, shared nested counters |
| arc-detector/src/lexer.rs | Quote/escape-aware byte scanner; single quotes literal, active dollar/backtick unresolved; unsupported syntax fails closed; token reserved before word allocation |
| arc-detector/src/parser.rs | Groups/literal shell wrappers share depth/work; no arbitrary-argument reparse; command nodes and children charged |
| arc-detector/src/rules.rs | Exact Cargo/gh names; SSH repair preserves opaque option values; numeric IPv6 without resolution; Docker always unknown |
| arc-detector/Cargo.toml | Serde only normal dependency; serde_json test-only; no execution/network/parser dependency added |
| arc-ipc/src/lib.rs | Existing public dispatch closure unchanged; protocol guard first; strict payload decode; result contains fixed enums only |
| arc-daemon/src/connection.rs | Synchronous classification under existing request admission, no doctor callback or database path; payload cleared before any await |
| arc-cli/src/main.rs and classify.rs | Exactly one literal argument, one local IPC exchange, generated request ID independent of input; sanitized errors; validated response before output |
| arc-daemon/src/limits.rs and platform.rs | Baseline limits/deadlines and native same-user checks unchanged |

The complete detector runtime source was read. It has no subprocess, filesystem,
environment, resolver, network, logging or storage calls. Scanner comparisons/copies,
parser visits, wrapper recursion and SSH scans share bounded counters; fixed-size
IPv6 arithmetic does not introduce an input-sized uncharged loop. CLI/daemon IPC
and fixture-owned processes/listeners are infrastructure, not execution of analyzed
commands. Static inspection supports the negative probes; it cannot replace the
unexecuted mandatory TCP assertions or establish universal runtime behavior.

## Deterministic tests and limits

The shared test-only loader consumes all 45 original and seven amended successful
fixtures, two ordering variants and fourteen CLI literals. It uses three explicit
oversized Rust generators, never evaluation. Unit tests preserve the ten malformed
payloads, all five limits, production work exhaustion/refusal/overflow, nested shared
counters, syntax/quote/SSH negatives and 4096 deterministic generated inputs.
IPC tests require zero doctor callbacks and version-first rejection. The actual
daemon matrix compares all four fields and exact human/JSON output for 68 cases.
Fresh receipts retain commands, complete logs and before/after hashes.

privacy.rs preserves both Unix and TCP monitors. Twelve canary-bearing families
exercise traps for Cargo/gh/SSH/Docker/shells/touch/curl, active/literal/nested/backtick
substitutions and monitored endpoints. Recursive scans cover SQLite including live
WAL/SHM, daemon logs and runtime reports, with response/CLI error and output checks.
The retained Unix receipt passes. The TCP test still requires TcpListener::bind
and is unchanged; none of its assertions is claimed to have run this attempt.

The retained saturation test preserves production admission, health responsiveness,
same-connection malformed recovery during ongoing traffic and SIGTERM accounting.
Its receipt and compatibility receipts are hash-validated, not reported as fresh runs.

## Execution prerequisite and handoff

The prior normal shell prerequisite probe records errno 1 / Operation not permitted
for local TCP bind/listen. The current supported tool context declares networking
disabled and supplies no established effective local TCP-listen permission. No new
bind probe, TCP test or exact workspace gate was attempted. No sandbox changes,
GUI workaround, invented runner, test skip, weakened assertion or dependency install
was used. Chief should stop identical retries until the execution prerequisite changes.

Serena tools were unavailable. Code inspection used direct reads of known paths and
the retained source inventory, not native repository code discovery. The memory quick
pass found no relevant ARC entry and supplied no implementation facts. Global Git
status reports av.yaml access denial despite exit 0; that path was not bypassed.
The existing mise tracking-link warning was not repaired. No subagent, AV dispatch,
mission-state mutation, standalone build, browser suite or external delivery occurred.

The exact workspace Cargo gate, TCP privacy assertions, independent oma-qa and final
mission acceptance remain incomplete. Broader detector, scheduling, managed-execution,
desktop and native Windows obligations remain open. Harness coverage is unresolved.
