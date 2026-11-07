# Task-4 repair handoff to Chief Director

Status: **partial**. Malformed-request recovery is now tested during sustained classification load. The required Cargo fmt and clippy gates pass. The exact workspace-test gate still fails because the required loopback TCP monitor is denied. The delivered CLI requirements still need Chief disposition and synchronization by their owners. Neither blocker is represented as resolved.

Skill used: **oma-backend**, with the local backend execution protocol/checklist, shared execution policy/result contract, backend rule and language rule. Serena discovery found no available tools; inspection used known paths without repository code search. No relevant ARC memory entry was used. No subagent, AV operation, GUI, browser, installation, download, standalone build or external delivery was used.

## Changed paths and artifacts

The only product/test change in this repair is `crates/arc-daemon/tests/classification/saturation.rs` (142 lines). All preceding detector, IPC, CLI, manifest and lockfile edits were retained. `repair-2-source-inventory.json` compares all 76 current inputs with the prior inventory; only the saturation file differs. The original implementation report and Cargo/harness logs are retained as historical evidence; their source maps predate this repair.

New evidence is confined to `.arc-verify/command-classification/implementation/`: `repair-2-report.md`, `repair-2-summary.json`, `repair-2-claim.json`, `repair-2-source-inventory.json`, `repair-2-*.json` command receipts, corresponding complete `.txt` logs, `repair-2-observations/`, and reproducible `repair-2-record.py`, `repair-2-inventory.py`, `repair-2-finalize.py`. Early repair checks used the existing recorder and refreshed its native observations; final complete-run observations are separately retained in `repair-2-observations/` and bound by the new summary. Earlier command receipts/logs were not replaced.

HEAD remains `05f0812d0841485bbe07b3f1f8f6fde3861a1d55`. The migration and harness bytes match that HEAD. All 63 Rust files, including untracked additions, remain within 500 lines; the maximum is 474. Both dependency artifacts and their contract digest remain unchanged.

## Repair and acceptance evidence

- [x] Malformed input under classification load: sixteen workers continue submitting alternating maximum-byte and work-exhausting inputs until shutdown, with a 30-second failure cleanup bound. Admission loss may be retried; a received error must equal the complete fixed `INVALID_REQUEST` response. A subsequent valid classification must succeed on that same connection. Completed classifications are counted before and after recovery, proving traffic continues.
- [x] Existing ten health successes and the 3500 ms shutdown assertion remain. No connection/request/deadline limit was raised. Existing runtime accounting assertions still require all handlers reaped, worker joined, socket removed, gauges zero and database high-water usage zero.
- [x] The repaired scenario passes in both the exact workspace run and complete no-fail-fast run. In the latter observation: 16 classifications completed before recovery, 29 at recovery, 85 total; recovery 9404 microseconds, maximum observed health latency 5455 microseconds, SIGTERM-to-exit 11887 microseconds. Request high-water is 16 against cap 16; connection high-water is 25 against cap 64. These are local deterministic-test observations, not production performance claims.
- [x] Complete suite: **202 passed, 1 failed, 0 ignored**. Detector/IPC/CLI, protocol, status/doctor, same-user authentication, admission, 100000-attempt bounded load, health and shutdown regressions pass. The sole failing test is the retained mandatory TCP privacy monitor.
- [x] The twelve-case Unix privacy fixture again reports zero trap starts, substitution markers, endpoint contacts and persisted canaries, including live SQLite/WAL checks. This supports the unchanged pure-detector call path and does not replace the missing TCP evidence.
- [ ] Required TCP privacy evidence: `crates/arc-daemon/tests/classification/privacy.rs:95` still fails at `TcpListener::bind("127.0.0.1:0")`, errno 1, `PermissionDenied`, `Operation not permitted`. It failed both the focused retained test and both final workspace commands. No ignore, skip, alternative bind, GUI workaround or expectation change was added. This session exposes no permitted escalation route; the Chief must arrange permitted noninteractive execution of this unchanged test and the exact workspace gate.
- [ ] Conflicting CLI requirements: unresolved as detailed below. No input artifact or implementation behavior was changed to choose a side during this repair.
- [ ] Independent oma-qa review remains the separately assigned reviewer's responsibility. This implementation self-check is not that review.

## Commands and outcomes

Every recorded Cargo invocation uses `CARGO_NET_OFFLINE=true`. Each recorded subprocess has a 180-second deadline, within the supplied 86400-second mission budget. Checks ran serially with no extra agents. Host CPU/memory limits were not changed. Owned test children and load threads retain existing cleanup. Rust/Cargo 1.95.0, rustfmt and clippy were available. Mise still prints an external tracking-link permission warning; it was not bypassed.

| Command | Exit | Evidence stem |
| --- | ---: | --- |
| `cargo test -p arc-daemon --test classification privacy::required_loopback_endpoint_nonexecution_and_privacy -- --exact` | 101 | repair-2-tcp; unchanged mandatory monitor denied |
| `cargo test -p arc-daemon --test classification saturation::maximum_and_exhausting_classification_load_preserves_health_and_shutdown -- --exact` | 0 | repair-2-saturation; initial repair version, before final counter assertion |
| `cargo fmt --all -- --check` | 1 | repair-2-fmt; two formatting differences, preserved |
| `cargo fmt --all` | 0 | repair-2-format-apply; formatting repair only |
| `cargo fmt --all -- --check` | 0 | repair-2-final-fmt; final source |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | repair-2-final-clippy; final source |
| `cargo test --workspace` | 101 | repair-2-final-workspace; exact required command, only TCP privacy failure |
| `cargo test --workspace --no-fail-fast` | 101 | repair-2-all-tests; complete target results, 202/1/0 |
| `./scripts/harness/validate.sh` | 1 | repair-2-harness; unchanged, missing test:coverage, unresolved |
| `python3 .arc-verify/command-classification/implementation/repair-2-inventory.py` | 0 | repair-2-source-inventory.json; hashes, all Rust lengths, baseline invariants and contract digest |
| `python3 .arc-verify/command-classification/implementation/repair-2-finalize.py` | 0 | repair-2-summary.json; receipt/log hashes and current source maps validated |
| `git diff --exit-code 05f0812d0841485bbe07b3f1f8f6fde3861a1d55 -- crates/arc-storage/migrations/0001_phase0.sql scripts/harness/validate.sh` | 0 | direct baseline comparison, also reproduced in inventory |

The unchanged harness retains its actual missing `test:coverage` failure. No dummy coverage task, skip setting or operator-command change was introduced. The new final receipts bind exact argv, exit, duration, complete log digest and before/after source maps. No stale passing gate is used to certify changed source.

## Interface conflict requiring Chief coordination

The supplied repair finding requires Chief disposition, not a routine implementation choice. No disposition or owner-synchronized replacement artifact was provided in this session. There is no connected Chief messaging interface; this report and returned partial claim are the handoff. The backend Actor does not own either input artifact and has not edited them.

| Topic | Task-1 contract | Task-2 CLI requirements | Existing implementation |
| --- | --- | --- | --- |
| Empty/whitespace | `contract.md:115` explicitly sends empty input and returns EMPTY_COMMAND; typed observations exit 0 | `cli-requirements.md:46-47` rejects empty/whitespace with nonzero exit and no result | Contract observation |
| Oversized input | `contract.md:117,125` returns INPUT_LIMIT as completed UNKNOWN/OBSERVE | `cli-requirements.md:49,94` requires a size-error state | Contract observation |
| First human label | `contract.md:117` requires Classification: | `cli-requirements.md:69` requires Class: | Classification: |
| UNKNOWN prose | `contract.md:117` fixes exactly five lines | `cli-requirements.md:80` adds a static explanation before the trailer | Five lines |

Chief action needed: issue the authoritative disposition and have task-1/task-2 owners synchronize their actual artifacts. Backend can then update the dependent CLI behavior and deterministic assertions and refresh affected gates. Reporting one contradictory artifact as satisfied would be inaccurate. The protocol-1 shared schema, dispatch signature, detector API, status/doctor behavior and CLI behavior remain unchanged in this repair.

## Remaining risks and scope

The task remains partial until permitted TCP evidence and the requirements conflict are resolved. The source change closes the saturation recovery coverage gap without weakening acceptance. Independent review and final mission evidence remain with their assigned owners; no reviewer-owned or mission-state files were written.

The unresolved harness coverage task remains open. This slice does not close the broader detector catalog, scheduling, managed-execution, desktop or native Windows acceptance. It establishes bounded non-executing classification within the documented subset; no execution, scheduling, endpoint topology or resource-saving claim follows from these test results.
