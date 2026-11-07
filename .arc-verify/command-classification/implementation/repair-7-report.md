# Task-4 SSH boundary repair handoff

Status: completed implementation-owner repair; independent QA pending task-5.
Skill used: oma-backend, with its execution protocol/checklist, shared execution
policy/result contract and backend rule. This report covers attempt 7 only.
HEAD remains 05f0812d0841485bbe07b3f1f8f6fde3861a1d55. Existing implementation,
preceding attempts, spent budgets and failed evidence were retained; no mission
state, assignments, acceptance criteria or accepted specifications changed.

## Delivered behavior

The SSH classifier now checks the option boundary after a validated destination.
Unresolved leading option tokens return UNKNOWN/UNKNOWN/SSH_AMBIGUOUS/OBSERVE.
This covers -N, -G, -L, -R, -D and -W, quoted tokens and local/remote compounds.
Explicit -- before or after the destination permits a following literal command;
a terminator without a nonempty command remains uncertain. Initial empty words
cannot hide the boundary. Once a real remote command starts, arguments remain
opaque: ssh build01 make -N and ssh build01 'make -N' retain
REMOTE_EXEC/REMOTE/SSH_REMOTE_COMMAND/NONE. Pre-destination forwarding detection,
option values, destination validation and loopback behavior are preserved.

The production repair adds only bounded slice advancement, charged empty/token
checks, Budget.eq for -- and a precharged leading-byte option check. Every added
scan/comparison pays the existing shared work budget before operating. No process,
shell evaluation, resolver, endpoint, filesystem, environment, logging or storage
operation was added. The production detector still depends only on existing Serde.

## Changed paths this attempt

- crates/arc-detector/src/rules.rs — bounded SSH command-boundary repair.
- crates/arc-detector/src/tests.rs — permanent deterministic matrix and exact/short
  recognition-work budget regressions, with synthetic counter traces.
- crates/arc-detector/tests/support/ssh.rs — new shared 99-case literal matrix.
- crates/arc-detector/tests/support/contract.rs — expose the shared SSH fixtures;
  original contract fixtures remain unchanged.
- crates/arc-ipc/tests/classification.rs — typed, correlated protocol-1 regressions.
- crates/arc-daemon/tests/classification.rs — reusable existing matrix driver and
  actual daemon/human-CLI/JSON-CLI SSH regressions.

No manifest, lockfile, classification type, wire schema, dispatch signature,
CLI product source or shared runtime fixture changed in this repair. There is no
ownership or interface conflict to report. Other Actors' files were preserved.

## Required input artifacts and regression mapping

Consumed the accepted contract.md/contract.json pair, synchronized
cli-requirements.md, review/report.md and verification/ssh-post-destination.json.
Their exact hashes are in repair-7-preparation.json and the source inventory.
All 357 preceding command-classification artifacts remain byte-identical.
Reviewer-owned review/ and verification/ were neither changed nor regenerated.

repair-7-regression-mapping.json maps all 17 retained probe IDs and exact synthetic
inputs to permanent expected four-field results. It also retains all 99 cases,
actual detector results, charged work counts and layer/test mapping. All 17
original expectations match. Added cases cover quoted versions and compounds for
each of the six flags, explicit terminators before/after the destination, missing
commands, empty words, trailing terminators, opaque remote arguments, forwarding,
option-value, destination and loopback controls. Every case runs through:

- deterministic typed detector analysis;
- typed protocol-1 dispatch without the doctor/database callback;
- actual daemon IPC;
- actual arcctl human output (exact five lines, newline, no ANSI, empty stderr);
- actual arcctl JSON output (exact four-field result, one line, empty stderr).

The shared cases are synthetic data only. No SSH input, SSH host, nested shell,
Cargo/gh command or analyzed endpoint was executed by these regressions.

## TDD_EVIDENCE

RED was recorded before repairing rules.rs:

- cargo test -p arc-detector ssh_post_destination -- --nocapture — exit 101;
  the new matrix reproduces incorrect remote certainty.
- cargo test -p arc-ipc ssh_post_destination -- --nocapture — exit 101;
  typed post-n result incorrectly reports REMOTE_EXEC.
- cargo test -p arc-daemon ssh_post_destination -- --nocapture — exit 101;
  actual daemon post-n result incorrectly reports REMOTE_EXEC.

Complete logs and before/after hashes are repair-7-red-{detector,ipc,daemon}.txt
and .json. These are behavioral assertion failures, not compiler errors. The
retained independent probe supplies the pre-repair human/JSON CLI failures; the
new daemon RED test stops at its first IPC mismatch before reaching CLI assertions.
The first RED corpus had 94 cases. Five additional empty-word controls were
added with the repair; the final permanent corpus has 99 cases.

GREEN on final sources:

- cargo test -p arc-detector ssh_post_destination -- --nocapture — exit 0;
  two tests pass, including work refusal before overrun.
- cargo test -p arc-ipc ssh_post_destination -- --nocapture — exit 0; one test passes.
- cargo test -p arc-daemon ssh_post_destination -- --nocapture — exit 0;
  one test passes, including all 99 actual human/JSON CLI pairs.

See repair-7-green-detector-final, repair-7-green-ipc and repair-7-green-daemon
receipts/logs. The earlier preliminary GREEN detector receipt is retained but
is historical for the final instrumented test source.

## Commands executed and outcomes

The following required checks ran in the mission workspace, serially in this
order, after the repair. Exact argv, cwd, timestamps, elapsed time, actual exits,
complete combined logs and equal before/after source hashes are in the named
repair-7-*.json/.txt receipts. Cargo used CARGO_NET_OFFLINE=true.

| Command | Outcome | Receipt stem |
| --- | --- | --- |
| cargo test -p arc-daemon --test classification privacy::required_loopback_endpoint_nonexecution_and_privacy -- --exact | Exit 0; one test passed | repair-7-tcp |
| cargo fmt --all -- --check | Exit 0 | repair-7-fmt |
| cargo clippy --workspace --all-targets -- -D warnings | Exit 0 | repair-7-clippy |
| cargo test --workspace | Exit 0; 209 passed, zero failed/ignored | repair-7-workspace |
| ./scripts/harness/validate.sh | Exit 1; missing test:coverage; one violation; no skip | repair-7-harness |

The unchanged TCP privacy test successfully bound/listened on loopback and passed.
The operator's environment change is effective for this run. Earlier
PermissionDenied receipts are historical and are not current blockers.

Supplemental checks: cargo fmt --all applied formatting (exit 0); cargo test -p
arc-detector tests::production_work_exhaustion_trace -- --exact --nocapture passed;
git diff --check -- Cargo.toml Cargo.lock crates scripts/harness passed;
python3 repair-7-finalize.py passed all hash, mapping, count and preservation
assertions. A preliminary work trace briefly overlapped the running harness;
it was repeated after the harness completed for final serial supplemental
verification (repair-7-work-trace-final). No required gate overlapped another.
The work trace retains real production budget exhaustion below every other cap.

Shell startup and Bun emitted a mise tracking-link permission warning. The
commands continued; this warning did not fail the Rust checks. No denied action
was retried through a GUI or another policy context. Serena tools were unavailable
in ALL_TOOLS; inspection used already-known paths and artifact-provided inventory,
without native project code searches. No dependency installation, standalone
build, browser suite, screen check, GUI interaction or external delivery occurred.

## Acceptance evidence

- [x] Conservative post-destination SSH results, compound uncertainty, explicit
  termination and opaque remote arguments match all 17 retained expectations.
- [x] 99 permanent data-driven cases exercise detector, typed IPC, actual daemon
  and actual human/JSON CLI behavior; preceding contract cases remain intact.
- [x] All five original limits remain unchanged: 16384 bytes, 1024 tokens,
  128 invocations, depth 8 and 65536 charged work steps. Original boundary,
  deterministic fuzz and production exhaustion tests pass, plus new SSH work tests.
- [x] Unchanged executable traps, active/literal substitution sentinels, Unix/TCP
  monitors and live SQLite/WAL/log canary checks pass on repaired sources.
  These assert no tool starts, substitution markers, endpoint contacts or persisted
  command/secret canaries. Static dependency/call-path inspection is recorded in
  repair-7-static-inspection.json. Fixed enums prevent raw-input result echoes.
- [x] Protocol 1, dispatch(request, doctor), version-first payload validation,
  same-user native authentication, status/doctor, admission and lifecycle behavior
  remain covered by passing workspace regressions. Maximum/exhausting
  classification load still passes health, malformed recovery and shutdown tests.
  No live foreign-account impersonation or native Windows execution is claimed.
- [x] All 65 Rust source/test files, including untracked files, meet the 500-line
  cap; maximum 474. New evidence helpers also remain below 500 lines.
- [x] Migration 0001_phase0.sql and scripts/harness/validate.sh equal baseline
  bytes; admission, limits, platform/authentication and lifecycle also equal baseline.
- [x] Exact Cargo gates pass on the final sources; unchanged harness result and
  missing coverage failure are retained without skips or dummy coverage tooling.
- [x] Before/after hashes identify the six changed paths; each final receipt
  matches all 80 current source/config/spec/helper inputs. Earlier passing receipts
  are not reused to certify changed inputs.
- [ ] Independent task-5 QA and final mission verification remain pending.

## Artifacts and Chief Director handoff

repair-7-summary.json is the implementation-owner result. Supporting artifacts:
repair-7-preparation.json, repair-7-source-inventory.json,
repair-7-regression-mapping.json, repair-7-static-inspection.json,
repair-7-record.py, repair-7-finalize.py and all repair-7 check receipts/logs.
This report and repair-7-claim.json complete the backend handoff.

Deliver these repaired sources and current evidence to task-5 for independent
oma-qa. Its retained HIGH finding is historical for pre-repair hashes and has not
been independently cleared. Do not mark independentReviewComplete or overall
mission acceptance from this backend report. No reviewer-owned artifact was edited.
The raw request never enters the database worker, and payload disposal before
await is unchanged. Interfaces and accepted specifications require no handoff change.

Remaining obligations: independent QA, final mission verification and unresolved
harness coverage; wider detector coverage, scheduling, managed-execution, desktop
and native Windows acceptance remain open. This slice provides syntax
classification only; it makes no scheduling, execution, locality-probing,
resource-saving or commercial-performance claim.
