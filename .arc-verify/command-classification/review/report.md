# Review Result: PASS

Independent QA Reviewer, task-5, attempt 4. Skill used: oma-qa and its execution
protocol, shared execution policy/result contract and i18n rule. Verification
completeness: completed for the assigned repaired classification slice.
No current CRITICAL, HIGH or MEDIUM implementation finding was reproduced.
The prior HIGH SSH-POST-DESTINATION finding is resolved by inspected source and
fresh IPC and CLI execution. The unchanged harness still fails coverage; this
verdict does not approve full ARC acceptance.

## Source, ownership and preservation

HEAD is 05f0812d0841485bbe07b3f1f8f6fde3861a1d55. I inspected existing changes
first, including task-4's repair-7-report.md, actual detector modules, permanent
assertions, transport integration, CLI code, manifests and lockfile. Historical
Actor verdicts were inspection inputs, not substitutes for current execution.
Only .arc-verify/command-classification/review/ and verification/ were written.
No product code, test, specification, manifest, lockfile, migration, harness,
implementation evidence, assignment or mission state was changed. Exact changed
paths are in claim.json. No interface or ownership conflict was found.

Before refreshing current reports, I archived all 50 preceding review/verification
files in attempt-3-evidence.tar.gz and recorded their hashes in
attempt-3-preservation.json. The finalization helper validates archived bytes.
Earlier archives and the original failing ssh-post-destination.json/.txt remain.
Independent framework reviews and Chief-owned final artifacts were not changed.

## Resolved HIGH: SSH option boundary

crates/arc-detector/src/rules.rs:69 now advances a borrowed slice over initial
empty words with charges before token/length operations. At :79, Budget.eq checks
explicit --. At :82, a charged leading-byte check returns
UNKNOWN/UNKNOWN/SSH_AMBIGUOUS/OBSERVE for unresolved options. At :90, charged
nonempty checks preserve uncertainty when termination has no command. A real
command's remaining arguments stay opaque. Explicit terminators follow the
accepted contract and retained controls; no SSH tool was executed.

The independent verification/ssh-repair.py retains all 17 original probe inputs
and adds 15 controls. All 32 exact four-field expectations pass protocol-1 IPC,
real human arcctl and real JSON arcctl: 96 actual classifications, zero mismatches.
It checks protocol, request correlation, successful observation exit, empty stderr,
exact five human lines, exactly four JSON fields and one JSON newline. Only an
owned daemon and arcctl run; analyzed strings remain data. Daemon SIGTERM exits 0
and removes its socket. Actual argv/output, context, times and binary/source hashes
are in ../verification/ssh-repair.json, with complete ssh-repair.txt output.

The 11 former mismatches now return SSH_AMBIGUOUS/OBSERVE, including quoted flags
and compounds. Recognized remote execution and quoted local arguments retain
exact results. Added controls cover -- without a command before/after destination,
quoted terminators/flags, opaque separated/quoted arguments, empty words hiding
flags and resolved/uncertain compounds. The retained old probe remains historical.

I inspected all 99 permanent literal cases in
crates/arc-detector/tests/support/ssh.rs and their assertions in src/tests.rs,
crates/arc-ipc/tests/classification.rs and crates/arc-daemon/tests/classification.rs.
The workspace run executes tests::ssh_post_destination_matrix,
tests::ssh_post_destination_work_checks_refuse_before_overrun,
ssh_post_destination_typed_dispatch and
ssh_post_destination_daemon_human_and_json_cli_matrix. All 99 cases run through
detector, typed dispatch, actual daemon and both actual CLI modes. Six recognition
budget controls accept their measured exact budget and refuse a one-short budget.

## Security, work accounting and behavior

I read detector lib.rs, budget.rs, lexer.rs, parser.rs, rules.rs and types.rs.
Production arc-detector depends only on existing Serde and has no process, shell,
filesystem, environment, network, resolver, logging, database or cache operation.
Fixed result enums cannot echo commands. Cargo.lock adds no external package.

budget.rs:23 checks addition and refuses before committing an overrun. The lexer
charges inspections, lookahead, decoding copies and UTF-8 validation. Parser token
visits, nodes, recognition and nested analysis share one budget; nested input
borrows decoded storage. Rules precharge keyword/option/suffix/destination checks;
aggregation charges each leaf. The repair's slice advancement/checks are bounded
and charged without command copying or evaluation. Caps remain 16384 bytes,
1024 tokens, 128 invocations, depth 8 and 65536 work units.

Fresh boundary tests cover inclusive/over-limit values, Unicode, combined nesting,
checked overflow, determinism and real production exhaustion. The supplemental
existing trace passes with 12033 source bytes, 60203 committed work units, six
tokens, two invocations and depth one before a larger precharged operation fails.
Refusal below 65536 is correct: the next operation must fit in full. Primitive
exact-limit and 4096 fixed-seed Unicode/syntax tests also pass.

Both live privacy tests pass with executable traps, active/literal/nested/backtick
substitution text, TCP/Unix monitors and before/during/after scans of SQLite,
live WAL/SHM, captured logs and runtime reports. Tool starts, substitution markers,
endpoint contacts and persisted canaries are zero. Negative checks are paired
with the inspected pure module boundary, not treated as universal proof alone.

## Compatibility and responsiveness

crates/arc-ipc/src/lib.rs:308 preserves dispatch(request, doctor) and checks version
before decoding classification payloads. Classification never calls doctor.
crates/arc-daemon/src/connection.rs:97 classifies synchronously under existing
admission, drops body storage and clears the payload before response await;
raw commands never enter DbWorker. Wire types and CLI behavior match the contract.

Unchanged platform.rs uses native getpeereid on macOS; admission.rs authenticates
before constructing a handler. Actual same-user sockets, non-socket rejection and
foreign-directory UID policy pass. No live foreign-account impersonation is
claimed. Existing exact status/doctor behavior, classification not_checked,
health and protocol tests pass. limits.rs, admission.rs, platform.rs and
lifecycle.rs match baseline bytes, preserving 64 connections, 16 requests,
queue eight, absolute deadlines and 3500 ms shutdown.

Under continuing maximum/exhausting classification traffic, fresh results record
7421 microseconds maximum health latency, 9029 microseconds malformed recovery
and 15096 microseconds SIGTERM-to-exit. Same-connection recovery and continued
traffic are asserted. Joined worker, reaped handlers, zero gauges and socket
removal pass. These are one macOS debug run, not general performance estimates.

## Commands and acceptance evidence

All commands below ran serially in the mission workspace. Exact argv, cwd,
context, timestamps, exits, complete logs and equal before/after input hashes are
in verification/. Cargo used CARGO_NET_OFFLINE=true. record.py retains effective
IDs, host resource limits, one-check parallelism and 120-second command timeouts.

| Command | Actual outcome | Receipt/log stem |
| --- | --- | --- |
| cargo test -p arc-daemon --test classification privacy::required_loopback_endpoint_nonexecution_and_privacy -- --exact | Exit 0; one passed | privacy |
| cargo fmt --all -- --check | Exit 0 | fmt |
| cargo clippy --workspace --all-targets -- -D warnings | Exit 0 | clippy |
| cargo test --workspace | Exit 0; 209 passed, zero failed/ignored | workspace-tests |
| ./scripts/harness/validate.sh | Exit 1; missing test:coverage, one violation, no skip | harness |
| python3 .arc-verify/command-classification/verification/ssh-repair.py | Exit 0; 32 cases, zero mismatches across all three interfaces | ssh-repair |
| cargo test -p arc-detector tests::production_work_exhaustion_trace -- --exact --nocapture | Exit 0; one passed | work-trace |

record.py and work-trace-record.py capture the checks. finalize-repair.py validates
source inventories, logs, original probe identity, named matrix tests, invariant
bytes and current scalar summaries. executed-tests.json and test-matrix.md map
executables and exact test names to every original obligation and SSH repair.

The independent tracked/untracked inventory covers 65 Rust source/test files,
maximum 474 lines, all <=500. All 80 source/config/contract inputs match before
and after required checks, final inventory and task-4 repair-7 input hashes.
0001_phase0.sql and scripts/harness/validate.sh match baseline bytes. QA helpers
also remain below 500 lines.

- [x] Repaired sources and permanent assertions independently inspected with oma-qa.
- [x] Original classifications, quotes, uncertainty, bounds and privacy checks pass.
- [x] All 17 retained SSH cases and controls pass IPC and both actual CLI modes.
- [x] Exact Cargo gates and freshly repeated TCP privacy check pass.
- [x] Protocol, status/doctor, authentication, admission, health and shutdown preserved.
- [x] Migration/harness unchanged; complete resulting-source inventory within cap.
- [x] Actual failed harness coverage result retained without skips or replacement tools.
- [x] Required scalar summaries and material reports refreshed with source hashes.

## Limits, failed attempts and Chief handoff

The harness has not passed: test:coverage remains unresolved as required by the
assignment. The changed TCP permission was freshly checked and passes; stale
errno-1 receipts are historical. Serena discovery exposed no tools, so known-path
reads, supplied inventories and Git diff were used without native code searches.
Global Git status reports av.yaml access denial; no globally clean claim is made.
mise emitted a tracking-link warning while checks continued successfully.
Two early reads of guessed scan.rs/parse.rs paths returned exit 1; lib.rs identified
lexer.rs/parser.rs, which were read successfully. One report patch was rejected
because delete/add targeted the same path; it wrote nothing and was replaced by
a scoped report write. None of these attempts changed product inputs.

No standalone build, installation, browser, GUI, permission bypass or external
delivery occurred. The supported shell subset is conservative syntax analysis;
actual host and Docker locality are not resolved. Broader detector catalogs,
dialects/scripts, scheduling, managed-execution, desktop, native Windows and
coverage acceptance remain open. Chief-owned final mission verification and
independent framework reviews remain downstream. Current findings are empty;
blockersResolved=true applies to this assigned repaired slice, with coverage
explicitly recorded as unresolved.
