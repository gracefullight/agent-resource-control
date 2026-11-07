# Task-4 repair 3 handoff to Chief Director

Status: **partial**. The SSH option-value defect is repaired, with a reproduced failure and passing detector, IPC, actual daemon and actual CLI regressions. Required formatting and clippy gates pass. The workspace-test gate fails at the unchanged TCP monitor because the host denies loopback bind. Contradictory delivered CLI requirements still require Chief disposition and input-owner synchronization. Neither blocker is marked resolved.

Skill used: **oma-backend**, with its execution protocol/checklist, shared execution policy/result contract, backend rule and language rule. The established Rust 2021, Serde, Tokio, Clap and SQLite stack is retained. Serena discovery exposed no tools; only known-path code reads were used. Plain evidence documents were searched for a Chief disposition; none was supplied or found. Memory lookup found no relevant ARC entry. No subagents, AV operations, mission-state changes, GUI, browser, installations, downloads, standalone build or external delivery were performed.

## Changed paths and produced artifacts

Only four source/test inputs differ from the preceding repair inventory:

- `crates/arc-detector/src/rules.rs`: recognize forwarding flags at SSH option positions; treat known attached/separate option values as opaque; stop conservatively at unknown option arity.
- `crates/arc-detector/src/tests.rs`: add deterministic negative and forwarding-preservation cases to the existing matrix.
- `crates/arc-ipc/tests/classification.rs`: add a typed option-position regression matrix without invoking the doctor/database callback.
- `crates/arc-daemon/tests/classification.rs`: run the same distinctions through the real daemon and real arcctl in both human and JSON modes; verify no database work.

All earlier detector, IPC, daemon, CLI, manifest, lockfile and saturation changes are retained. `repair-3-source-inventory.json` identifies all 76 current source/manifest/contract inputs, including untracked files. HEAD remains `05f0812d0841485bbe07b3f1f8f6fde3861a1d55`. All 63 Rust source/test files are within 500 lines; maximum 474. The migration and harness match baseline bytes. Both dependency artifacts, CLI requirements and contract digest are unchanged.

New artifacts are confined to `.arc-verify/command-classification/implementation/repair-3-*`: preparation, this report, static inspection and supplemental requirement map, source inventory, summary, partial claim, recorder/inventory/finalizer scripts, actual-command JSON receipts and complete text logs. `repair-3-observations/` contains fixture outcomes/accounting without raw command/secret canaries. Prior reports and receipts were preserved. This report is an implementation handoff, not the independent reviewer’s report or final mission acceptance.

## Repair and acceptance evidence

- [x] The added detector regression first fails with SshForwarding versus SshAmbiguous for the attached username. The identical focused command passes after the product repair. See repair-3-ssh-red and repair-3-ssh-green receipts/logs and TDD_EVIDENCE in repair-3-static-inspection.md.
- [x] Attached username and -o setting values no longer create forwarding flags. Separate values, including a value that looks like a flag, remain opaque. Genuine attached/combined forwarding and a genuine forwarding flag after an option value remain recognized. Unknown option arity stays SSH_AMBIGUOUS. Existing remote execution, loopback, numeric IPv6, quotes and command-family behavior remains covered.
- [x] Actual daemon/CLI observations check complete four-field results, successful UNKNOWN exit, empty stderr and complete human output. The shared wire schema, protocol 1, public dispatch signature, detector API and CLI command syntax are unchanged. No dependencies, manifests or lockfile changed in this repair.
- [x] Complete run: **204 passed, 1 failed, 0 ignored**. All detector and IPC tests execute under --no-fail-fast despite the daemon integration target’s prerequisite failure. Existing protocol, status/doctor, native same-user policy, admission/deadline, 100000-attempt bounded-load, health and shutdown regressions pass.
- [x] Under sustained maximum-input/work-exhaustion classification load, malformed-request recovery succeeds on the same connection and load continues. The latest complete-run receipt records 16 classifications before recovery, 31 at recovery and 96 total; malformed recovery 10998 microseconds, maximum observed health latency 7598 microseconds, SIGTERM-to-exit 14438 microseconds. Request high-water 15 is within 16; connection high-water 28 is within 64. All handlers are reaped, worker joined, gauges zero, socket removed and database high-water zero. These are local fixture observations, not production performance claims.
- [x] The retained twelve-case Unix privacy test detects zero analyzed tool starts, substitution markers, endpoint contacts or persisted canaries; it checks protocol/client output, daemon logs, runtime reports, SQLite and live WAL/SHM. The static pure-detector call path remains unchanged except for bounded in-memory SSH option inspection. This does not substitute for the missing TCP monitor evidence.
- [ ] `crates/arc-daemon/tests/classification/privacy.rs:95` still cannot bind `127.0.0.1:0`: errno 1, PermissionDenied, Operation not permitted. It fails in focused integration, the exact required workspace gate and the complete no-fail-fast run. The test and expectation were not skipped, weakened, ignored or changed. No permitted escalation or alternate noninteractive environment is supplied; the Chief must arrange permitted execution of the retained test and exact workspace gate.
- [ ] The CLI input-artifact conflict below remains unresolved. The backend cannot claim both contradictory requirements are satisfied.
- [ ] Independent oma-qa remains the separately assigned reviewer’s responsibility. No reviewer-owned evidence was written.

## Commands and outcomes

Cargo checks run with CARGO_NET_OFFLINE=true using the existing toolchain/dependency cache and lockfile. Each recorder command has a 180-second bound; checks run serially. No host resource limits were changed. Existing fixtures retain owned-child cleanup. Mise emits an external tracking-link permission warning; it was not bypassed.

| Command | Exit | Evidence stem |
| --- | ---: | --- |
| `cargo test -p arc-detector syntax_quotes_aggregation_and_ssh_negative_matrix -- --exact tests::syntax_quotes_aggregation_and_ssh_negative_matrix` | 101 then 0 | repair-3-ssh-red / repair-3-ssh-green |
| `cargo fmt --all` | 0 | repair-3-format; required formatting after edits |
| `cargo test -p arc-detector -p arc-ipc -p arc-daemon --test classification` | 101 | repair-3-focused; retained TCP prerequisite failure, six daemon cases pass |
| `cargo fmt --all -- --check` | 0 | repair-3-final-fmt |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | repair-3-final-clippy |
| `cargo test --workspace` | 101 | repair-3-final-workspace; unchanged required argv |
| `cargo test --workspace --no-fail-fast` | 101 | repair-3-all-tests; 204 passed, one TCP prerequisite failure |
| `./scripts/harness/validate.sh` | 1 | repair-3-harness; actual missing test:coverage failure, unresolved |
| `python3 .arc-verify/command-classification/implementation/repair-3-inventory.py` | 0 | repair-3-source-inventory.json; source hashes/lengths, baseline invariants, contract digest |
| `python3 .arc-verify/command-classification/implementation/repair-3-finalize.py` | 0 | repair-3-summary.json; current-input and receipt/log validation |

Final receipts bind argv, cwd, exits, elapsed time, complete log hash and before/after source maps. The finalizer checks those maps against current source hashes and checks that no failure is represented as success. Harness runs with coverage enabled and without modifying its bytes or supplying dummy coverage tooling. The actual harness coverage failure is retained; no historical passing result certifies changed code.

## Interface conflict for Chief coordination

No authoritative disposition or synchronized replacement artifact was supplied. The assignment explicitly forbids this Actor from modifying those input artifacts. There is no connected supervising-Chief message target in this run; this report and returned partial claim deliver the blocker to the supervising Chief.

| Topic | Task-1 contract | Task-2 CLI requirements | Current behavior |
| --- | --- | --- | --- |
| Empty/whitespace | contract.md:115 sends explicit empty input; :117 observations exit 0 | cli-requirements.md:46-47 and C03 reject with nonzero exit | EMPTY_COMMAND observation |
| Oversized | contract.md:117,125 requires INPUT_LIMIT observation | cli-requirements.md:49,94 and C15 describe size-error exit | INPUT_LIMIT observation |
| Human label | contract.md:117 requires Classification: | cli-requirements.md:69 requires Class: | Classification: |
| UNKNOWN explanation | contract.md:117 requires exactly five lines | cli-requirements.md:80/C23 adds an explanation | Five lines |

Chief action: issue the authoritative disposition and have task-1/task-2 owners synchronize the actual artifacts. Backend then updates any affected CLI behavior/tests and refreshes evidence. Existing protocol, types, status/doctor output, same-user authentication and admission limits remain compatible; this repair does not choose a side or change the observed CLI behavior.

## Remaining risks and scope

Task-4 remains partial until permitted TCP execution and the input-interface conflict are resolved. The known SSH defect is fixed without weakening tests or adding execution/contact/persistence. Independent review, final verification and final mission delivery remain assigned elsewhere. Global status still reports an av.yaml access denial, so no globally clean-worktree claim is made.

The unchanged harness coverage failure remains open. This slice does not satisfy the wider detector catalog, scheduling, managed-execution, desktop or native Windows acceptance obligations. Typed results describe only the documented restricted syntax, without runtime executable/topology proof, admission, execution or resource-saving claims.
