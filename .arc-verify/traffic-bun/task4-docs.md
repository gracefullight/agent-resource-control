# Task 4: direct Bun test documentation

Status: completed for the assigned documentation scope. Skill used: `oma-docs`.

## Deliverable and acceptance

- [x] `docs/managed-exec.md` shows `arcctl exec -- bun test`, explains the direct one-shot boundary, and identifies the shared Test window and global admission budget.
- [x] The guide describes watch, hot, unresolved and remote bypass, including package scripts and wrappers invoked under unrecognized names. It notes that a PATH shim invoked as `bun` or `cargo` can be eligible, and distinguishes literal-text classification from admission of a particular execution.
- [x] The existing queue notice, status, cancellation and native argument, stream, exit and signal behavior remain documented without promising a start time, admission or a user-test result.
- [x] The guide describes legacy unresolved/lifecycle fallback when an old daemon does not support Bun admission, the uncertain-enqueue witness case, caller-owned lineage preservation and the reported mixed-version lineage limitation.
- [x] The timing section distinguishes the initial deadline from fresh deadlines on the eligible unsupported-enqueue and bypass fallbacks.

The guide was compared against `crates/arc-detector/src/argv.rs` and `rules.rs`, `crates/arc-traffic/src/lib.rs`, `crates/arc-cli/src/exec.rs`, `exec/traffic.rs` and `exec/lineage.rs`, the task-1 contract and the later task-2 handoffs. This is source inspection, not a field test or an independent review of all backend tests. A focused detector test was rerun as recorded below. Task 5 must collect its measurement after this document is integrated; independent QA and the mission-wide gates remain downstream.

## Interface note for the Chief and backend owner

The detector checks literal program and subcommand tokens without resolving the executable (`crates/arc-detector/src/argv.rs` and `rules.rs`). Thus an unrecognized wrapper name such as `bun-wrapper` is unresolved, but a PATH shim invoked as `bun` can be scheduled. The mixed-tool fixture uses such shims in `crates/arc-daemon/tests/support/bun_measurement.rs`. The guide now reflects that boundary; this is a documentation correction, not a backend change.

For the Chief and backend owner: the earlier guide incorrectly implied that every legacy REGISTER and CLAIM shared the initial 1,800 ms deadline and its 200 ms handoff allowance. In `crates/arc-cli/src/exec.rs`, preparation starts the initial deadline; `exec/traffic.rs` passes it through when the capability probe is unsupported, but starts a fresh 1,800 ms deadline after a Bun enqueue returns unsupported without a possibly committed request, or after a Traffic bypass with no possibly committed request. `exec/preparation.rs` shares whichever deadline it receives across REGISTER, CLAIM and retries. The guide now describes those paths and no longer suggests that the fresh fallback fits within the original allowance. This is a documentation correction against the delivered implementation; no backend code was changed or timing test rerun here.

The earlier `.arc-verify/traffic-bun/task2-backend-handoff.md` says the new wrapper does not change the child's environment; `.arc-verify/traffic-bun/task2-attempt7.md` explicitly supersedes that older snapshot. The current `exec.rs` adds `ARC_TRAFFIC_LINEAGE` when absent and preserves a caller-supplied value. The accepted old-daemon-style claim lookup and the separate caller-value and wrapper-death tests do not establish universal mixed-version permit reuse; the guide states the reported limitation. Please use the later handoff when coordinating with the backend owner.

## Checks

`oma docs verify` regenerates `docs/generated/doc-refs.json`. To avoid writing an unowned shared index, the verification commands ran in an isolated snapshot made from `git archive HEAD` plus the accessible tracked and untracked worktree changes at the time, including the guide. The wrapper repair reused that snapshot with the current guide copied in; the subsequently created task report and other later worktree changes were not overlaid. The sync command also created that index in the mission workspace, unexpectedly: `docs/generated` was absent before the command. After checking that the directory contained only this file and had no open handles, the file was moved unchanged to `target/traffic-bun-docs/sync-doc-refs.json` and the empty directory was removed. The snapshot excludes inaccessible `av.yaml` worktree contents and ignored runtime files, so its repository-wide findings are not a complete live-worktree reference audit.

| Command | Result |
| --- | --- |
| `oma docs sync HEAD --json` in the mission workspace | Exit 0; 64 candidate documents. The assigned guide was not auto-matched and was updated from the explicit task and source inspection. Other candidates, including `docs/bounded-ipc-recovery.md`, `docs/resident-slice-review.md` and historical evidence reports, were outside this assignment and left untouched. |
| `oma docs verify docs/managed-exec.md --json --no-urls` in the snapshot, before | Exit 0; one document, zero extracted references and zero broken references. |
| Same scoped verification, final | Exit 0; one document, seven extracted references and zero broken references. |
| `oma docs verify --json --no-urls` in the snapshot, before and final | Exit 1 both times; 756 documents and 471 broken references each time. The broken-reference set was unchanged and none belongs to the assigned guide. These are out-of-scope snapshot findings; examples include references in `docs/prd.md` and `docs/resident-slice-review.md`. |
| Repair: `oma docs verify docs/managed-exec.md --json --no-urls` in the snapshot, before and after the timing edit | Exit 0 both times; one document, seven references, zero broken references. |
| Repair: `oma docs verify --json --no-urls` in the snapshot, before and after the timing edit | Exit 1 both times; 756 documents, 7,050 references, 725 skipped and 471 broken references. The complete broken-reference set was unchanged. |
| Repair: `oma docs verify docs/managed-exec.md --json --no-urls` in the snapshot, before and after the wrapper edit | Exit 0 both times; one document, seven references, zero broken references. |
| Repair: `oma docs verify --json --no-urls` in the snapshot, before and after the wrapper edit | Exit 1 both times; 756 documents, 7,050 references, 725 skipped and 471 broken references. The complete broken-reference set was unchanged. |
| `cargo test --offline -p arc-detector bun_native_argv_is_conservative_and_shell_protocol_is_unchanged` | Exit 0; one test passed. It covers native Bun recognition, explicit paths, package scripts, watch/hot and literal-text classification; it is not a shim execution test. |
| `git diff --check` and `git diff --cached --check` for the guide and this report | Exit 0. |
| Python SHA-256 comparison of the source and raw-log entries below | Exit 0; all listed files matched. |

The first edited scoped scan exited 1 because the checker interpreted the example `bun run test` as a missing `test` entry in this repository's `package.json`. The guide now refers to `bun run` package scripts without implying that entry; the final scoped scan passes. URL checks were not run. Serena tools were unavailable, so the identified implementation files were read directly. `git status --short -- av.yaml` reported `av.yaml: Operation not permitted`; no complete worktree-cleanliness claim is made.

## Source and raw-log hashes

SHA-256 of the document and the implementation files inspected:

```text
5e640a29cbd5b2cc1b8c16ffda1984ec62175b2d7f1ced854d9ff4e984099ac9  docs/managed-exec.md
ab2bf3ed6a7ba824b78bb1cf19a4c533613edc99071fd6ddb07787649b8164d6  crates/arc-cli/src/exec.rs
8d3efbbc83a817f34d7874fe54e74baeef3129f7c6dbf5bb50629f0cd1525238  crates/arc-cli/src/exec/traffic.rs
5cb57ed8c9f780e50e8fb2f0206513d276075af54336c110f06bc2a1cc97dd3a  crates/arc-cli/src/exec/preparation.rs
906224e1b6a03c30190008509a57277ab572915effa8cf014064ab1d4f7af02c  crates/arc-cli/src/exec/lineage.rs
148a05909a035df7aab763fa39a20c063ec6ae85bfe9288661208c54488de500  crates/arc-detector/src/argv.rs
a7ac1e9a66fb79524622f07253eab2c9ebe10ecdac54163cdac83779d60988b0  crates/arc-detector/src/rules.rs
f261c5d24e79e9b83eb4c81de342269767cf7acf44ed57edb84da9c81dc676c8  crates/arc-traffic/src/lib.rs
a29e3d70bca5957c051c70ebebc2317201d0c46caa4a97c92a45e09a97d6f994  crates/arc-daemon/tests/support/bun_measurement.rs
```

Raw logs are under `target/traffic-bun-docs/`; the documentation-command stderr files were empty. SHA-256:

```text
d2d3c923a6c6f6ea6a59c828114033f0f89ddaecfb6516ec276ea63bdb177006  sync.stdout
f86bd8c7674a9d834aa63f1bdd2934e1525b80906ca0daaa46c9cfcfa9ec7250  verify-before.stdout
2cbebacfd3060fa34d1cbb6cbf3a4b5557d2b94e0557a5d8564ba2a8d23615de  verify-scope-before.stdout
78e6a2cb6455416762eddd1589fa3a28fd654cfc93bac3ef515b1b41cd8cfd4d  verify-attempt1.stdout
37dfd187402b9997868c406cb8f7cceb4215a648c7c4d38cff3d656006890e6d  verify-scope-attempt1.stdout
08c1728cbdce79d38e2890e1ba1512835e81e0d083af97a686a8d9c45036bdf4  verify-after.stdout
4137e2f090d47ba2b6cecf2f9de91261d22b3a01e391af21d0e84668503fb825  verify-scope-after.stdout
f77fe769ee2a4f2cc15190f03e8469425a80932bef35e0a095d83dee7385faf2  sync-doc-refs.json
4137e2f090d47ba2b6cecf2f9de91261d22b3a01e391af21d0e84668503fb825  repair-scope-before.stdout
4137e2f090d47ba2b6cecf2f9de91261d22b3a01e391af21d0e84668503fb825  repair-scope-after.stdout
08c1728cbdce79d38e2890e1ba1512835e81e0d083af97a686a8d9c45036bdf4  repair-full-before.stdout
08c1728cbdce79d38e2890e1ba1512835e81e0d083af97a686a8d9c45036bdf4  repair-full-after.stdout
4137e2f090d47ba2b6cecf2f9de91261d22b3a01e391af21d0e84668503fb825  wrapper-scope-before.stdout
4137e2f090d47ba2b6cecf2f9de91261d22b3a01e391af21d0e84668503fb825  wrapper-scope-after.stdout
08c1728cbdce79d38e2890e1ba1512835e81e0d083af97a686a8d9c45036bdf4  wrapper-full-before.stdout
08c1728cbdce79d38e2890e1ba1512835e81e0d083af97a686a8d9c45036bdf4  wrapper-full-after.stdout
e04b9405a1f6f28530cbc521b69c5cb909b9815a01be7edd928390589d360c80  wrapper-detector.stdout
843e277a02eecd933cedce1e4ef337f79df22381f181d9a5dee5cd43f49d5c3c  wrapper-detector.stderr
```
