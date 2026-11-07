# Command classification CLI requirements

Status: synchronized requirements handoff for task-4; implementation and executable verification pending.

Owner: Design Director, task-2. Only this file is owned by this task. Product code, tests, shared contracts, manifests, mission state and other Actors' reports are outside its write scope.

## Evidence and decision scope

Accepted committed baseline: `05f0812d0841485bbe07b3f1f8f6fde3861a1d55`, tree `ab739c266e893550181917a906051ec34a441b51`.

Original inspection (historical): Actor-local HEAD `5661fecb83554b32999ce13b2eb793c5f134cdbc`, tree `ab739c266e893550181917a906051ec34a441b51`. This matched the supplied initial snapshot commit/tree, rather than the baseline commit identity. The four source paths below had no diff between accepted baseline and that inspected HEAD, and scoped status was clean before this report was created. The report was a new, uncommitted Actor-local deliverable, absent from that baseline and snapshot. These original observations and hashes are retained as historical evidence, not current implementation observations.

| Historically inspected path | SHA-256 of inspected bytes | Original observation |
|---|---|---|
| `crates/arc-cli/src/main.rs` | `f016929b258c35cf357cfcd1cf23b12fb541c5824f7876837385d7af5a7786ed` | Clap, `ColorChoice::Never`, status/doctor only; errors on stderr and failure exit |
| `crates/arc-cli/tests/binary.rs` | `b0c93d83c0f61d43f16dca3addb6bcc598a766cea66a2180e73adaf491eb7e2a` | No-argument invocation fails, names status/doctor, rejects ANSI color escapes |
| `crates/arc-ipc/src/lib.rs` | `08e2a76995e10ef45c6229ceda943817107798acc425ff35ff46840c096d40b1` | Protocol 1; hello/health/doctor; version-first rejection; unsupported capability error |
| `crates/arc-daemon/tests/resident/lifecycle.rs` | `36635e569a669173844a00e7287009a7d28f2ddee576eadf2c6b12311bd67b1f` | Exact status/doctor output assertions and daemon-unavailable checks |

The supplied Design Director advice supplies the selected single-string journey. Excerpts inspected from the external ARC SRS, `/Users/gracefullight/Downloads/Agent_Resource_Control_ARC_Requirements_SRS_2026-11-06.md`, §§1.3, 8.3–8.5 and 13.4 support the audience, command/locality distinctions and conservative fallback. That external file is not a baseline repository artifact. The mission's bounded slice takes precedence over the SRS's broader detector catalog.

No users were recruited or observed. No field tests, usability simulations, screen checks, retention studies or commercial measurements were performed. Personas and usability expectations below are assumptions derived from requirements. Source inspection establishes existing behavior in code and assertions; it does not establish passing current tests or terminal accessibility performance.

Skill used: `oma-design`, local contract operation. Its supplier-routing reference permits a local route for documenting an existing design decision. No external visual supplier or frontend skill is needed for this terminal requirements handoff. Design checklist items applied: scope, provenance, preserved behavior, explicit assumptions and implementation handoff. Rendered checks are unrun and outside this task.

## Amendment inputs and scope

This synchronization consumes the actual amended task-1 pair, [contract.md](contract.md) and [contract.json](contract.json), including its Chief disposition, request/result definitions, CLI contract, 45 classification fixtures, 10 malformed-payload fixtures, seven disposition fixtures and six boundary specifications. The earlier report's SHA-256 was `bb5e24df257d28f1e003d5817bab61a8e447b4db79b02f73c332d125de1428bd` (24141 bytes, 179 lines); the original inspection and check record remain below. This amendment updates the requirements themselves, examples and individual matrix rows.

| Consumed prerequisite | SHA-256 of actual input bytes | Bytes / lines |
|---|---|---|
| `contract.md` | `42cafc2b0e7424b50179a7ebc59bdc4f18df29d7657e449b729ad290366a8fe9` | 39897 / 251 |
| `contract.json` | `f7402e685399af7715716969d011c9e9113d6910aa9c4b6c8e9f277449385573` | 41877 / 410 |

Amendment inspection: actual worktree HEAD is `05f0812d0841485bbe07b3f1f8f6fde3861a1d55`, tree `ab739c266e893550181917a906051ec34a441b51`. Existing product edits and untracked implementation/evidence files are present; this is not a clean-worktree claim. Only this CLI requirements file is changed by this task. No current product behavior, successful implementation test or repair is inferred from the contract's historical provenance.

## Users and selected journey

| Requirement-derived persona | Intended job | Design judgment, not observed behavior |
|---|---|---|
| Developer using multiple coding agents/worktrees | Understand whether a command describes local work, remote work or unresolved work | One labeled result should reduce interpretation effort |
| Integration developer | Consume a stable classification without running the command | Typed JSON should avoid prose parsing |
| Operator investigating UNKNOWN | Understand uncertainty and correct an input when possible | A bounded reason and explicit uncertainty should support interpretation; help carries input guidance |

With the daemon already running: submit the literal command once, then read one result. Do not require status, doctor, onboarding, confirmation, a wizard, a spinner, a retry loop or a retention prompt. If the daemon is absent, fail once with an actionable message; do not start it automatically. A subsequent user-initiated invocation is recovery, not an automatic retry.

Alternatives considered: argv mode would lose the original shell structure unless a second interpretation contract were added; a wizard would add steps and ask users to supply the locality the detector must determine. Select the literal-string mode already requested. Do not add stdin, interactive or execution modes to this slice.

## Syntax and input

```text
arcctl classify [--shell <shell_kind>] [--json] -- <command>
arcctl classify --help
```

- `<command>` is exactly one required UTF-8 argument containing command text. The canonical shared syntax above uses `--`; the delimiter may be omitted when the argument is unambiguous, as in the original help examples below. Accept `--json` before or after that positional argument while options remain enabled. After `--`, tokens are positional data; it does not join multiple arguments into one command.
- Preserve command bytes, including quotes, Unicode and internal whitespace, in the typed `job.classify` request. Do not join argv, interpolate, evaluate, normalize away quotes, discover executables, resolve filesystem scripts or probe endpoints. Explicit empty and whitespace-only strings are valid input: within the byte cap and for a supported shell they yield UNKNOWN/UNKNOWN/EMPTY_COMMAND/OBSERVE, exit 0 and empty stderr. Leading/trailing whitespace around a nonempty supported command is accepted.
- A missing argument, extra positional argument, unknown CLI option or missing `--shell` value produces a nonzero usage exit, empty stdout and no classification request. Usage errors must not echo command data, shell values or unrecognized option text. An explicitly supplied empty command is distinct from a missing argument. Malformed shell text in a valid typed request yields UNKNOWN/UNKNOWN/UNSUPPORTED_SYNTAX/OBSERVE; do not start a shell to validate it.
- Optional `--shell <shell_kind>` maps directly to the request's `shell_kind` string. Omission defaults to `sh`; exact case-sensitive `sh`, `bash` and `zsh` use the shared restricted grammar. Other strings, including an explicitly empty value, are valid and yield UNKNOWN/UNKNOWN/UNSUPPORTED_SHELL/OBSERVE with exit 0 and empty stderr. Do not reject them as CLI enum/usage errors or invoke a shell executable. Accept `--shell` before/after the positional argument while options remain enabled; after `--`, it is positional data. Supported names do not claim full dialect or native Windows support.
- Use the existing owner-only IPC connection and protocol 1. Send one `job.classify` request for a valid submission; no preliminary hello/status/doctor exchange is required. Preserve current admission and timeout bounds.
- Apply the shared contract's inclusive analysis limits: 16384 UTF-8 command bytes, 1024 tokens, 128 invocations, nesting depth 8 and 65536 charged work steps. Byte limits count original bytes before trimming, not Unicode characters. A valid typed request fitting the unchanged 1048576-byte transport-frame cap whose command exceeds 16384 bytes succeeds with UNKNOWN/UNKNOWN/INPUT_LIMIT/OBSERVE before scanning, exit 0 and empty stderr. This includes oversized whitespace. Other exhausted budgets also succeed with UNKNOWN/UNKNOWN, the matching TOKEN_LIMIT, INVOCATION_LIMIT, NESTING_LIMIT or WORK_LIMIT reason, and OBSERVE. No stricter CLI analysis policy is allowed.
- Preserve failure ordering: transport/envelope decoding; protocol version guard before typed-payload validation; original-byte precheck; shell-kind resolution; left-to-right analysis. Thus oversized input takes INPUT_LIMIT even with an unsupported shell, and an in-cap empty command with unsupported shell takes UNSUPPORTED_SHELL. Malformed typed requests and transport-frame violations remain failures; they never become analysis observations. Shared counting rules and pre-operation work charges remain owned by task-1.

Required help examples, shown as literal documentation only; none was executed for this report:

```text
arcctl classify 'cargo test'
arcctl classify --json 'gh workflow run ci.yml'
arcctl classify 'cargo test && gh workflow run ci.yml'
arcctl classify 'cargo test -- "gh workflow run ci.yml"'
arcctl classify --shell bash -- 'cargo test'
arcctl classify -- ''
arcctl classify --shell powershell -- 'cargo test'
```

Required help explanation: “Pass the entire command as one quoted argument. In POSIX shells, single quotes preserve literal text. Your shell processes quoting and substitutions before ARC receives input. Command text is analyzed, not executed.” These examples target POSIX caller shells; do not claim native Windows quoting support. A command containing a single quote needs caller-shell escaping that still delivers one argument. Do not recommend `eval` or a shell wrapper.

Clarify that active unquoted or double-quoted command substitutions may execute in the caller's shell before ARC is invoked. ARC cannot prevent that caller-side action. Tests must pass substitution text through `std::process::Command::arg`, not `sh -c`; this distinction preserves the literal-input guarantee without running the examples.

## Successful output

Human stdout is exactly five lines in this order, each terminated by one LF, including the last. There is no extra explanation or blank line for UNKNOWN or any other successful observation:

```text
Classification: <typed job class>
Locality: <typed locality>
Reason: <bounded reason code>
Uncertainty: <explicit typed uncertainty>
Command not executed.
```

Use the shared detector/IPC enum values rather than an independent CLI taxonomy. Known Cargo rules must distinguish build, test, clippy and fmt in the typed result/reason. Required semantic outcomes are remote CI for `gh workflow run` and `gh run watch/view`, remote execution for an unambiguous SSH remote command, MIXED for resolved local/remote compounds, and UNKNOWN/OBSERVE for unresolved scripts, expansions and Docker endpoints. Classification describes the analyzed text; it does not certify the actual endpoint or resource consumption.

The CLI uses exactly the shared result fields `classification`, `locality`, `reason` and `uncertainty`, with no lossy remapping or separate confidence model. Class values are BUILD, TEST, LINT, REMOTE_CI, REMOTE_EXEC, MIXED and UNKNOWN; locality values are HOST_LOCAL, REMOTE, MIXED and UNKNOWN; uncertainty is NONE or OBSERVE. Reasons use the closed enum in the consumed contract pair. BUILD/TEST/LINT require HOST_LOCAL/NONE; REMOTE_CI/REMOTE_EXEC require REMOTE/NONE; MIXED requires MIXED/NONE; UNKNOWN requires UNKNOWN/OBSERVE. NONE means syntax-subset resolution, not runtime certainty. Cargo clippy and fmt are both LINT, distinguished by CARGO_CLIPPY and CARGO_FMT.

For UNKNOWN, use the same five-line template with `Classification: UNKNOWN`, `Locality: UNKNOWN`, the actual bounded reason and `Uncertainty: OBSERVE`. Do not append an UNKNOWN explanation, recovery sentence or prompt. All valid observations, including EMPTY_COMMAND, INPUT_LIMIT, other budget exhaustion and UNSUPPORTED_SHELL, exit 0 and leave stderr empty.

Exact human result for `arcctl classify -- ''` with the default shell:

```text
Classification: UNKNOWN
Locality: UNKNOWN
Reason: EMPTY_COMMAND
Uncertainty: OBSERVE
Command not executed.
```

Reasons must identify the supported rule or conservative fallback using a fixed vocabulary; they must never contain a command, argument, path, hostname, expansion, token excerpt or secret. No numeric probability or calibrated confidence claim is supported by the available evidence. No classification implies queued, admitted, started, blocked, bypassed, scheduled or completed work.

With `--json`, stdout is exactly one JSON object serialized from the shared typed result, followed by a newline. It contains the four fields above with the same values as IPC. No IPC envelope, request ID, raw input, arbitrary daemon message, human explanation, “Command not executed.” trailer or timing banner is added. Job class, locality and reason use stable enum/code serialization; uncertainty uses the contract's explicit typed representation. Do not change JSON meaning according to terminal detection, color settings or locale. Test the complete schema and values by decoding JSON, rather than comparing object key order. JSON success also exits 0 with empty stderr, including UNKNOWN/OBSERVE.

## Failures and recovery

On failure, stdout is empty in both output modes. Stderr contains a stable error code and at most one static recovery sentence, plus input-free usage text for CLI syntax errors. Do not emit a success-shaped or partial classification object. `--json` selects result formatting; it does not move errors to stdout. Exact nonzero numbers may follow the existing Clap/runtime convention; tests must not require every error category to share a number.

| State | Exit | Required stderr meaning / recovery |
|---|---|---|
| Explicit help | 0 | Empty stderr; help on stdout; no IPC |
| CLI syntax failure | Nonzero | Input-free usage diagnostics for missing/extra arguments, unknown options or missing shell value. “Pass exactly one command string.” No classification request |
| Malformed typed request / invalid request JSON | Nonzero | `INVALID_REQUEST` after the version guard; fixed diagnostic without payload details. “Use the documented classification request format.” |
| Analysis input above 16384 bytes, valid typed request within transport cap | 0 | Empty stderr; UNKNOWN/UNKNOWN/INPUT_LIMIT/OBSERVE result, not an error |
| Empty/whitespace input, within cap and supported shell | 0 | Empty stderr; UNKNOWN/UNKNOWN/EMPTY_COMMAND/OBSERVE result |
| Unsupported shell string, including empty, within byte cap | 0 | Empty stderr; UNKNOWN/UNKNOWN/UNSUPPORTED_SHELL/OBSERVE result |
| Other analysis budget exhaustion / completed UNKNOWN/OBSERVE | 0 | Empty stderr; bounded typed result with corresponding reason |
| Transport-frame violation, including frame above 1048576 bytes | Nonzero | Fixed transport error code without input; no successful result. “Use a request within the documented transport limit.” |
| Protocol mismatch | Nonzero | `PROTOCOL_INCOMPATIBLE`. “Use matching arcctl and daemon protocol versions.” |
| Protocol-1 daemon lacks classification | Nonzero | `UNSUPPORTED_CAPABILITY`. “Use a daemon that supports job.classify.” |
| Missing daemon/socket | Nonzero | `DAEMON_UNAVAILABLE`. “Start the ARC daemon, then submit again.” |
| Timeout, malformed/truncated response or I/O failure | Nonzero | Distinct transport state in wording; existing `DAEMON_UNAVAILABLE` or `IPC_ERROR` mapping may be retained. “Check the daemon connection, then submit again.” |
| Invalid typed result / unexpected remote error | Nonzero | `IPC_ERROR`, or the applicable shared code; never an unvalidated remote message. “Check the daemon connection, then submit again.” |

Validate protocol, response status, request-ID correlation, result types and shared field-combination invariants before formatting. Reject a result missing any required field, carrying an unknown enum value or violating those invariants; do not display it as a known or uncertain classification. An incompatible protocol must not be disguised as UNKNOWN. Unsupported capability must remain distinguishable from a completed unknown result.

Do not echo command text in success, error, Clap diagnostics, debug output, daemon logs, reports, database records or analytics. Do not persist command hashes or new telemetry as an alternative. A fixed sentinel secret in the subprocess tests must be absent from stdout, stderr and the mission's captured persistence/log surfaces. Passing a command over IPC is transient analysis input, not permission to retain it.

## Preservation and terminal behavior

Keep these existing outputs byte-for-byte, including punctuation and trailing newline; `<socket>` is the existing resolved path:

```text
running. Socket: <socket>. Protocol: 1. Sweeper off.
ok. IPC owner-only. Tray not connected. Vendors not detected. Command classification not checked. Sweeper off.
```

`status` remains daemon liveness/protocol inspection. `doctor` remains the existing diagnostics operation, including “Command classification not checked.” Adding a classification command does not turn doctor into a detector test. Preserve existing failure mappings for those commands, same-user authentication, IPC permissions, admission bounds, health responsiveness and shutdown behavior. Their operational verification remains with implementation and independent QA.

Preserve no-argument failure/help behavior while adding classify to the command list. Explicit help must work with no daemon. Keep `ColorChoice::Never`; no ANSI escape sequence, color, emoji, column alignment, animation or terminal control state carries meaning. Human output is linear and readable when redirected. Required checks use captured text, not screenshots or browser automation. Screen-reader usability remains untested.

## Deterministic CLI acceptance matrix for task-4

Use subprocess argv with isolated socket/database paths, owned child cleanup and bounded waits. A local fake IPC peer may exercise failure branches; it must validate operation, version and payload, then return fixture responses without contacting real services. Use the existing resident daemon fixture for end-to-end cases. Each successful invocation sends one request and produces one result. Record exact typed enum/reason expectations from task-1 as fixture data. The semantic expectations below must not be weakened to match implementation.

All success rows run in human and JSON modes unless stated otherwise. Assert exit, stdout/stderr, no ANSI escapes and no raw command/secret leakage on every applicable row. Every behavior here is a required future check, not a test result from this task.

| ID | Subprocess input or fixture | Deterministic assertion |
|---|---|---|
| C01 | No args; `--help`; `classify --help`, daemon absent | No args remains nonzero and names status, doctor, classify; explicit help exits 0; classify help shows optional shell syntax, all seven literal examples and quoting guidance; no IPC connection |
| C02 | `classify 'cargo test'`; flags before/after positional; omitted shell; separately `--shell sh`, `--shell bash`, `--shell zsh`; `--shell powershell`; explicitly empty shell value | One unchanged command string and selected/default shell sent to job.classify; supported shells yield TEST/HOST_LOCAL/CARGO_TEST/NONE; unsupported/empty shell yields UNKNOWN/UNKNOWN/UNSUPPORTED_SHELL/OBSERVE, exit 0 and empty stderr. Options after `--` are positional data, never flags |
| C03 | Missing command; extra argv; unknown option; missing shell value; separately explicit empty command and spaces/tabs/newlines only | Usage cases: nonzero, empty stdout, input-free stderr, no classification request. Explicit empty/whitespace with supported/default shell: exit 0, UNKNOWN/UNKNOWN/EMPTY_COMMAND/OBSERVE and empty stderr. Empty command with unsupported shell: UNSUPPORTED_SHELL observation. No execution in any case |
| C04 | `  cargo test  ` and `cargo\ttest` using actual tab argv | Same semantic classification as cargo test; payload preserves exact bytes; no echo |
| C05 | `cargo test -- "검증 λ"` | UTF-8 payload preserved; TEST/HOST_LOCAL/CARGO_TEST/NONE; no Unicode corruption or user text in output |
| C06 | cargo build / cargo test / cargo clippy / cargo fmt | BUILD/CARGO_BUILD, TEST/CARGO_TEST, LINT/CARGO_CLIPPY and LINT/CARGO_FMT respectively, all HOST_LOCAL/NONE; compare all four fields |
| C07 | gh workflow run ci.yml / gh run watch / gh run view | All REMOTE_CI/REMOTE/NONE, with GH_WORKFLOW_RUN, GH_RUN_WATCH and GH_RUN_VIEW respectively; no GitHub call |
| C08 | `ssh build01 'cargo test'`; forwarding `ssh -L8080:localhost:80 build01`; missing remote command `ssh build01`; `ssh localhost 'cargo test'` | Remote command: REMOTE_EXEC/REMOTE/SSH_REMOTE_COMMAND/NONE. Others: UNKNOWN/UNKNOWN/OBSERVE with SSH_FORWARDING or SSH_AMBIGUOUS respectively; no SSH connection or tool start |
| C09 | `cargo test && gh workflow run ci.yml`; grouped heterogeneous local stages followed by remote; same-locality and unresolved-stage fixtures | Resolved local/remote leaves: MIXED/MIXED/COMPOUND_MIXED/NONE. `cargo fmt; cargo clippy`: LINT/HOST_LOCAL/COMPOUND_LOCAL/NONE. `gh run watch; gh run view`: REMOTE_CI/REMOTE/COMPOUND_REMOTE/NONE. `cargo build && cargo test`: UNKNOWN/UNKNOWN/UNRESOLVED_COMPOUND/OBSERVE. Known pair plus `./deploy.sh`: UNKNOWN/UNKNOWN/UNRESOLVED_SCRIPT/OBSERVE; no partial success or scheduling claim |
| C10 | `cargo test -- "gh workflow run ci.yml"`; `cargo test -- "ssh host cargo build"` | TEST/HOST_LOCAL/CARGO_TEST/NONE; quoted arguments create no remote invocation or MIXED result |
| C11 | `printf '%s' 'cargo build && gh workflow run ci.yml'`; executable word containing quoted `cargo test`; literal `bash -lc 'cargo test'` wrapper | First two: UNKNOWN/UNKNOWN/UNRESOLVED_COMMAND/OBSERVE; no argument-text invocations. Exact literal shell wrapper: TEST/HOST_LOCAL/CARGO_TEST/NONE without starting shell |
| C12 | ./verify.sh / npm run verify / `$TOOL test` / docker --context prod build . | All UNKNOWN/UNKNOWN/OBSERVE, reasons UNRESOLVED_SCRIPT, UNRESOLVED_SCRIPT, UNRESOLVED_EXPANSION and DOCKER_ENDPOINT_UNKNOWN respectively; exit 0, empty stderr; no file execution, expansion or Docker probe |
| C13 | Active `$(...)` and backtick syntax supplied as argv, including double quotes; single-quoted literal arguments; nested `sh -c 'cargo test $(touch marker)'` | Active outer syntax and nested-shell expansion: UNKNOWN/UNKNOWN/UNRESOLVED_EXPANSION/OBSERVE. Single-quoted literal arguments to cargo test: TEST/HOST_LOCAL/CARGO_TEST/NONE. No substitution executed |
| C14 | Unterminated quote, dangling conditional or unsupported shell construct as command data | UNKNOWN/UNKNOWN/UNSUPPORTED_SYNTAX/OBSERVE, exit 0 and empty stderr; no shell validation process |
| C15 | 16384-byte boundary; 16385 ASCII/multibyte bytes; oversized whitespace; oversized command with unsupported shell; transport frame above 1048576 bytes | At 16384 bytes, continue shared analysis without INPUT_LIMIT. Valid typed requests within frame cap at 16385 bytes: exit 0, UNKNOWN/UNKNOWN/INPUT_LIMIT/OBSERVE, empty stderr, no scan; applies before whitespace/shell checks. No truncation or character-count substitution. Transport-frame violation remains nonzero with empty stdout, no result |
| C16 | Token 1024/1025, invocation 128/129, depth 8/9 and work 65536 boundary fixtures | Isolated inclusive boundaries proceed; beyond them return exit 0, UNKNOWN/UNKNOWN/OBSERVE with TOKEN_LIMIT, INVOCATION_LIMIT, NESTING_LIMIT or WORK_LIMIT, empty stderr. No partial known class. Detector units verify counters, shared nesting charges, exact work boundary/refused operation and overflow; CLI uses actual source-bound end-to-end exhaustion fixture |
| C17 | Fake peer returns INVALID_REQUEST; valid peer returns INPUT_LIMIT observation; raw IPC malformed-payload fixtures exercised in daemon suite | INVALID_REQUEST: nonzero, empty stdout, fixed code/static correction, no reflected remote text. INPUT_LIMIT is a successful typed result with exit 0/empty stderr. Missing/null/wrong-type command, null/wrong-type shell and unknown keys remain INVALID_REQUEST, distinct from explicit empty strings |
| C18 | Fake peer returns incompatible version or PROTOCOL_INCOMPATIBLE; daemon receives incompatible version with malformed classification payload | Nonzero, no classification; matching-version recovery sentence; never UNKNOWN. Version guard wins over malformed payload, retaining PROTOCOL_INCOMPATIBLE |
| C19 | Protocol-1 fake peer returns UNSUPPORTED_CAPABILITY | Nonzero, capability-specific recovery; status/doctor not invoked as prerequisites |
| C20 | Absent socket; peer closes mid-frame; timeout fixture | Bounded nonzero exit, empty stdout, stable unavailable/transport meaning; one request attempt, no automatic retry/startup |
| C21 | Missing result field, wrong field type, unknown enum, invalid field combination, mismatched request ID, invalid status, arbitrary error message containing sentinel secret | Nonzero, empty stdout; sanitized IPC error without secret/reflected message; no unvalidated or miscorrelated result |
| C22 | Failed request followed by a fresh valid CLI call | Second invocation succeeds without repair wizard, required doctor/status call or automatic replay of failed input |
| C23 | Known, UNKNOWN and MIXED human results, including EMPTY_COMMAND, INPUT_LIMIT, exhausted budgets and UNSUPPORTED_SHELL | Exactly five LF-terminated lines: Classification:, Locality:, Reason:, Uncertainty:, then “Command not executed.” No extra UNKNOWN sentence, blank line or prompt; exact enums, exit 0/empty stderr; no queue, permit or execution claim |
| C24 | JSON versions of known, UNKNOWN and MIXED, including all disposition and budget observations | Exactly one object with classification, locality, reason and uncertainty plus one LF; decode all four fields/invariants; no extra fields, prose, raw input, request IDs or envelope; exit 0/empty stderr |
| C25 | Existing status and doctor, before/after classification and after uncertainty | Exact preserved output and error semantics; protocol 1, no ANSI; existing lifecycle assertions retained |
| C26 | PATH trap executables, substitution sentinel, monitored local endpoint and secret-bearing argv | No trap start or sentinel creation, zero endpoint connections, secret absent in captured CLI/daemon output and database/log/report surfaces; harness itself never executes analyzed text |

C26 supports non-execution evidence together with pure-classifier source/dependency inspection and daemon integration tests. CLI text assertions alone cannot establish absence of execution, contact or persistence. A real remote endpoint, user study, browser suite or screen check is not required or authorized by this matrix. This task does not run these checks or claim any implementation acceptance gate has passed.

## Retention alternatives

| Alternative | Hypothesis | Evidence and selection |
|---|---|---|
| Neutral completed result | Lowest interaction cost for this single-call task | Selected design judgment; no measured retention or usability comparison |
| Contextual discovery nudge in help | Could reveal a useful next command without interrupting success | Unmeasured; literal examples already provide discovery. No extra prompt in this slice |
| Repeated “classify another command” nag after success | Could increase immediate calls while increasing interruption, abandonment or support burden | Unmeasured dark-pattern alternative; reject for this slice because no retention evidence supports intrusion |

More calls do not establish useful repeat work, conversion, reduced churn or profit. There is no observed baseline or simulated result for any alternative. A separately scoped future comparison could measure correct interpretation, task completion, unnecessary steps, complaints and meaningful return among users with another task. No recruitment, experiment, analytics or distribution is added here.

## Acceptance and verification record

- Concrete syntax, labeled human fields, stable shared JSON, exit behavior, literal-input examples and recovery requirements are specified above for task-4.
- The matrix is implementable through deterministic subprocess, fake IPC and resident-daemon fixtures; no browser or visual test requirement is introduced.
- Status/doctor text and semantics, color-independent output and the existing operational guarantees are explicit preservation requirements.
- Personas, expected usability and retention effects are labeled assumptions or judgments; no field-test, simulation or measured-retention claim is made.
- Syntax, input validation, successful output, failure states and every matrix row have been checked against the amended contract pair; C03, C15 and C23 carry the specific corrected outcomes, and C01/C02 cover optional shell handling.

### Original handoff checks (historical)

Commands executed for the original handoff: direct `cat` of known source/skill paths; targeted Python reads of the assigned report path, contract path and external SRS excerpts; `git status --short --untracked-files=normal`; scoped `git status`; `git rev-parse HEAD HEAD^{tree}`; baseline-tree and supplied-ref resolution; `git diff <baseline> HEAD -- <four source paths>`; `shasum -a 256 <four source paths>`. Scoped status and source comparison passed then. The report and contract did not exist at that initial inspection. Original source hashes are recorded above.

Deliverable checks: a Python content check passed (exit 0), confirming required syntax/state/preservation terms and 26 unique matrix IDs, C01–C26. `git diff --no-index --check /dev/null .arc-verify/command-classification/cli-requirements.md` produced no whitespace diagnostics; its exit 1 reflects the new-file difference. Final scoped `git status --short --untracked-files=all -- .arc-verify/command-classification crates/arc-cli crates/arc-ipc` showed only this new report (exit 0). These are document checks, not executable product acceptance evidence.

Limits/failures: global status printed `av.yaml: Operation not permitted` despite exit 0; scoped status was clean. The supplied snapshot ref could not resolve locally (exit 128), and separate worker-ref verification also failed (exit 128); supplied ref names are not claimed as inspected revisions. Actual HEAD/tree were inspected directly. Python startup printed mise tracking-link permission warnings; reads still completed with exit 0. Serena discovery found no available tools; only known source paths were read. No denied path was accessed through an alternative interface.

No product tests, Cargo gates, harness, builds, user research or rendered checks were run for this documentation task. Those remain implementation/QA obligations under the unchanged mission. Required final checks remain `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and unchanged `./scripts/harness/validate.sh`, retaining its actual coverage failure. This handoff does not close the remaining detector catalog, scheduling, managed-execution, desktop or native Windows obligations.

### Synchronization checks (this amendment)

Skill: `oma-design`, local contract correction. Read the complete owned report and amended contract pair, the shared execution policy, i18n rule, design entrypoint, supplier-routing reference, checklist and execution/result protocols. Serena tool discovery exposed no tools; no repository code discovery or alternate access to denied paths was needed. A quick memory search supplied no relevant ARC facts. No external supplier, installation or delegated Actor was used.

Document checks executed with `python3` inline (exit 0): actual prerequisite SHA-256 values and Markdown digest; 26 unique matrix IDs with individual semantic assertions; exact five-line human template and shared label order; exactly four JSON field names; the 45 original, 10 malformed and seven disposition fixture counts; result-enum invariants; all five limits; exact preserved status/doctor strings; absence of the old contradictory rules. Direct review mapped C01–C05 to CLI syntax/input and shell rules; C06–C14 to typed command/grammar/aggregation fixtures; C15–C17 to analysis/transport bounds and malformed requests; C18–C22 to protocol/response/failure recovery; C23–C26 to output, preservation and negative-evidence obligations. This validates requirements consistency, not implementation behavior.

`git diff --no-index --check /dev/null .arc-verify/command-classification/cli-requirements.md` emitted no whitespace diagnostics (exit 1 because the untracked file differs from `/dev/null`). `git diff --check` passed (exit 0); scoped status confirms the owned file remains untracked. Python before/after SHA-256 comparison of 257 existing files across `crates/`, `Cargo.toml`, `Cargo.lock`, the harness and the remaining classification evidence found no changed path outside the owned report (exit 0). Contract hashes above are unchanged. No claim of a globally clean worktree is made.

Current limits and failed attempts: Git again printed `av.yaml: Operation not permitted`; no bypass or repair was attempted. Python printed a mise tracking-link permission warning while completing its checks successfully. No document-consistency check failed. Product checks, the reported implementation TCP-bind prerequisite problem, independent QA and unresolved harness coverage remain with their assigned owners; this amendment neither resolves nor reruns them. No product source, tests, contracts, manifests, lockfile, migration, harness or mission state was written. The synchronized document is ready for task-4; executable acceptance remains pending.
