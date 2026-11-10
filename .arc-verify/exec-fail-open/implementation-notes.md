# Execution repair handoff — task 3, attempt 3

Status: assigned signal-handoff repair completed with passing focused checks.
Final mission collection and independent oma-qa remain downstream. Skill used:
oma-backend. Accepted HEAD: `00773c69a8e5e7fd417fd5cc78d03e222364f609`.

This report supersedes attempt 2's handoff. All earlier command receipts remain
unchanged as development history. Only the six `repair8-*` command receipts and
`implementation/repair8-native-timings.json` are the current focused handoff set.
Their before/after source fingerprint is
`b4dd903e5cd70b327392b67a9a364f4107f8e3c435d81c2ec35c606f6d536567`.

## Repair

The reported defect was reproduced before the implementation change. A separate
native sender waits for an admission pipe barrier, delivers INT or QUIT to the
wrapper's process group, and acknowledges the delivery before any target fork.
The wrapper waits for its handler's atomic receipt because libtest has multiple
threads. The production pre-exec admission is then exercised by a real
`Command::spawn`; a subsequent fixture hook must never run. The original code
allowed execution. The repaired code returns Interrupted, and the fixture pipe
proves zero invocation for both signals. The isolated helper verifies the exact
registered test body ran; a zero-test libtest success cannot satisfy it.

The shared-job quiet handler now retains terminal signals throughout SPAWNING.
At fork the child inherits any cancellation handled before its creation. Its
pre-exec hook checks that copy and rejects execution with EINTR. The CLI reports
that case as cancellation, with the existing static NOT_STARTED diagnostic.
The child blocks signals while checking and restoring saved native dispositions,
then restores its inherited mask. This closes the inherited quiet-handler window
before exec. Ignored dispositions and native masks remain preserved.

Parent sightings of group terminal signals after fork remain queued until
attachment but are discarded there for a shared job: the existing child receives
those signals from the OS. The separate executed-child ready-pipe test still
verifies one INT and one QUIT before attachment and no extra delivery afterward.
Nonterminal and non-shared terminal forwarding retain the existing behavior.
The sole product `Command::spawn` remains in exec.rs. No additional execution
owner, dependency, runtime, daemon startup, reservation, cleanup or replay exists.

The production gate's required test identities now include the new pre-fork
barrier test, in addition to the existing spawn-to-attachment test. Final workspace
receipts that omit it cannot pass the maintained validator.

## Changed paths and interfaces

This attempt changes these owned source inputs:

- `crates/arc-cli/src/exec.rs` — distinguish pre-exec cancellation from spawn failure.
- `crates/arc-cli/src/exec/signals.rs` — retain cancellation through fork, restore
  child dispositions under its signal mask, and avoid shared terminal redelivery.
- `crates/arc-cli/src/exec/signals/tests.rs` — register the focused test module and
  assert the retained parent observation in the existing post-spawn regression.
- `crates/arc-cli/src/exec/signals/handoff.rs` — new native admission/delivery barriers.
- `scripts/harness/exec-fail-open-recipe.ts` — require the new executed test identity.

It also updates this report and adds command receipts `repair6-red.json`,
`repair6-green.json`, `repair7-*.json`, `repair8-*.json`, and focused timing reports
`implementation/repair7-native-timings.json` and `implementation/repair8-native-timings.json`.

All preceding task-3 implementation, CLI fixtures/native acceptance tests,
help/documentation and executable validators remain in the delivery. Task-1's
contract pair and task-2's design contract were read and preserved byte-for-byte.
Public IPC, authentication, lifecycle/storage contracts and validator call forms
are unchanged. No ownership conflict or external file handoff was needed. No
manifest, lockfile, migration, historical receipt, accounting protection, mission
state, assignment, acceptance criterion or .agents definition was changed.
The mission index was not reset or staged by this Actor.

The five changed source files have 226, 271, 467, 73 and 41 lines respectively.
The unchanged preservation guard also checks the full source bounds. The initial
uncommitted work was inspected before edits and retained. Serena was unavailable
in the tool inventory, so inspection used scoped known-path reads and Git
metadata, without substitute native project-code search. No browser, GUI,
Playwright, installation or download was used.

## Focused verification

All six final receipts have exit zero, the same source fingerprint before/after,
and an actual process/argv/output binding. The maintained `validateCommand`
revalidated all six against current inputs, native host/tools, executable hashes
and raw-output hashes. Each check allows 3,600,000 ms. Checks ran serially, without
a coverage collector. The observed host is macOS arm64, 8 CPUs and 16 GiB RAM.
Raw logs remain under ignored `target/exec-fail-open/commands/`.

| Actual command | Result | Elapsed (s) | Receipt under implementation/commands/ |
| --- | --- | ---: | --- |
| `cargo fmt --all -- --check` | passed | 0.580 | `repair8-fmt.json` |
| `cargo test -p arc-cli` | 35 passed; 0 failed/ignored | 8.896 | `repair8-cli.json` |
| `cargo test -p arc-daemon --test managed_exec` | 35 passed; 0 failed/ignored | 71.235 | `repair8-managed.json` |
| `cargo clippy -p arc-cli -p arc-daemon --all-targets -- -D warnings` | passed | 1.372 | `repair8-clippy.json` |
| `bun test scripts/harness/exec-fail-open-docs.test.ts scripts/harness/exec-fail-open-preservation.test.ts` | 4 passed; 0 failed | 22.824 | `repair8-docs-preservation.json` |
| `bun test scripts/harness/exec-fail-open-gates.test.ts scripts/harness/exec-fail-open-evidence.test.ts -t 'fixture\|arithmetic\|predicates\|stale or incomplete'` | 6 passed; 0 failed; 2 production cases unselected | 5.379 | `repair8-validator-regressions.json` |

The filtered validator run exercises fixture and rejection regressions. It is
neither mandatory unfiltered production terminal command. The two production
cases require actual gates, independent QA and a final decision downstream;
none was fabricated. No ignored Cargo test or skip environment was used.
`cargo fmt --all` was used to format changes. Scoped
`git diff --check -- . ':!av.yaml'` passed; the denied, unowned av.yaml was not
read or bypassed. The index remains unchanged.

## Native observations

`implementation/repair8-native-timings.json` binds the actual managed-execution
receipt and each raw observation under `target/exec-fail-open/native/53949`.
All thirteen cases observed one invocation, exit 17 and one total invocation
after daemon recovery, with zero permits and no ARC cleanup. Candidate binary
hashes, observation timestamps and the executed test identity were checked.

| Outage | Parent-spawn to executed-child handshake (ms) |
| --- | ---: |
| absent | 9.546 |
| busy-claim | 15.677 |
| busy-register | 13.714 |
| committed-claim | 17.078 |
| committed-register | 16.821 |
| disconnect-before-read | 12.247 |
| disconnect-claim | 17.200 |
| disconnect-register | 12.295 |
| partial-register | 1814.959 |
| refused | 8.248 |
| shared-budget | 1845.304 |
| stall-claim | 1819.448 |
| stall-register | 1812.127 |

Maximum: **1845.304 ms**, below the 2,000 ms engineering target. The
boundary includes wrapper startup, spawn handoff and observer overhead. Internal
metadata/startup/handoff subdivisions remain unobserved; none were subtracted.
The shared absolute metadata budget remains 1,800 ms. These are controlled native
macOS measurements and focused tests, not final workspace or coverage acceptance.
The same managed receipt records native environment bytes, argv/cwd/descriptors,
streams, directed signals, terminal/pipeline behavior, rejection/cancellation,
idempotency, daemon-loss survival and fixed-baseline historical assertions.

## Failures retained

- `repair6-red.json`: `cargo test -p arc-cli
  shared_job_terminal_signals_after_admission_before_fork_cancel_execution`
  exited 101, reporting “pre-child terminal cancellation admitted execution”.
  This is the expected regression failure before the signal repair.
- `repair6-green.json`: the signal subset passed after the implementation change.
  It predates the final handler-acknowledgement barrier/import changes and is not
  current acceptance evidence.
- `repair7-docs-preservation.json`: exited 1. The unchanged guard rejected the new
  test's wildcard import as a possible assertion binding change. Replaced that
  import with explicit names. No guard or accounting rule was relaxed. The six
  `repair8-*` checks then passed on the final fingerprint.
- Other `repair7-*` successes and timing measurements describe the intermediate
  source. All earlier attempts' receipts remain intact, without relabeling them.

TDD_EVIDENCE: the exact new barrier selector failed in `repair6-red.json` before
repair; the same body passed in `repair6-green.json`, and both signals execute
again in the final unfiltered `repair8-cli.json` (35 passed, none failed/ignored).

## Preservation and downstream handoff

Migration worktree/index/accepted bytes retain SHA-256
`046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`.
The accepted historical assertion bodies and two fixed-baseline routes still
execute; the preservation tamper cases pass. Coverage accounting and every crate's
80 percent floor remain unchanged. Existing contracts retain hashes:

- contract.json: `474fbf2004483dfd56d3858c689dd77674c3dab008d34f8fa28b121dee524ff3`
- contract.md: `bb25c8603508ef555e786a4c0acc72b43f28e260d24f5b7c0400309d5375d6ed`
- design-contract.md: `1eb777d89b34d450628d5c707e424ea10be241fb675235676f3e327c803689db`

There are 63 files present under this evidence directory; none was staged by this
Actor. This leaves space below 150 for final collection. Final tracked/index-equal
sealing remains the assigned downstream owner's work.

Freeze/integrate these source inputs before task 4. Execute every one of the eleven
exact `contract.json.commandPhases.prerequisiteCommands`, serializing collectors
and running live coverage verification after the final collector. Capture actual
commands using `bun scripts/harness/exec-fail-open-receipts.ts NAME PROGRAM ARG...`,
then assemble using `bun scripts/harness/exec-fail-open-assemble.ts <11 names>`.
These focused receipts do not substitute for mandatory workspace commands.
The assembler needs no successor files. Run the exact gates validator before
independent oma-qa. The reviewer must perform the real unfiltered workspace
reproduction and author its own review. The final owner writes the actual verdict
and seals reachable evidence before the full evidence validator. Only afterward
record the actual terminal-command results in terminal.json; do not add a receipt
back-edge or claim a terminal result without executing it.

Task-3 repair acceptance: the signal defect is reproduced and repaired, the new
native barrier and preserved single-delivery test pass, native outage tests remain
passing, docs/protection/validator regressions pass, interfaces and ownership are
preserved, and current focused receipts/timings are delivered. No task-3 blocker
remains. Final workspace formatting/Clippy/tests, all coverage collections and
per-crate floors, the full no-skip harness, unfiltered production terminal
validators, independent oma-qa and the final mission decision remain required
upcoming work. They are not claimed passed here.
Desktop, native Windows, vendor hooks and scheduling remain outstanding.
User, adoption and business outcomes remain unmeasured.

## Task 8, attempt 2 — instrumentation boundary correction, 2026-11-09

Status: fixture correction and C1 clarification implemented. Ordinary and pinned
instrumented focused tests pass. The unchanged fail-open preservation validator
rejects the authorized fixture edit because it pins that fixture's old hash.
This requires a separate validator ownership handoff from the Chief. Final
acceptance is not claimed. Skill used: oma-backend. No subagents were dispatched.

Owned changes in this corrective delivery:

- `crates/arc-daemon/tests/managed_exec/fallback_environment.rs`: define equivalent
  complete synthetic inputs for direct and wrapped execution; retain raw output
  byte equality and direct stderr checks; assert supplied values in both outputs;
  add eight mismatch controls and real daemon recovery checks for every outage.
- `crates/arc-daemon/tests/managed_exec/runtime_environment_probe.rs`: standalone
  native attribution fixture, compiled with pinned rustc with and without LLVM
  instrumentation. It reads native environment bytes at its first main statement
  and contains no ARC execution or environment mutation code.
- `crates/arc-cli/src/main.rs:44`: add the C1 qualification in `long_about`, outside
  the accepted exact help amendment. `docs/managed-exec.md:11`: qualify fallback
  as outage behavior and identify invalid protocol replies and local preparation
  failure as non-launching cases. The locked amendment, accepted contracts,
  diagnostics, claims review and documentation checker are unchanged.
- This additive note and `implementation/task8-attempt2-correction.json` retain
  the corrective evidence. Earlier report text and all earlier receipts remain.

No execution production defect was established. No `exec.rs` or `exec/` code,
module registry, manifest, dependency, lockfile, migration, collector, accounting
rule, validator, historical test body or fixed-baseline route changed in this
corrective delivery. Public execution and IPC interfaces remain compatible.

### RED and initializer provenance

The exact pinned collection path reproduced the original equality failure before
any fixture correction. In a locked, offline Rust 1.95.0 / cargo-llvm-cov 0.6.21
workspace invocation, the original test failed in the absent-daemon case. The
sole complete-environment difference was:

```text
key:     __LLVM_PROFILE_RT_INIT_ONCE
 direct: absent
wrapped: __LLVM_PROFILE_RT_INIT_ONCE
```

`target/exec-fail-open/task8-correction/red-instrumented-executed.json` binds the
actual command, source fingerprint, native binaries and raw output hashes. The
RED source fingerprint is
`3d51ee23c34f602cf6bc5bb4ca6faf7d626aa7f5bcfc7347210e338dd3a0df17`.
The original instrumented candidate is preserved locally as
`target/exec-fail-open/task8-correction/red-instrumented-arcctl`.
The compact corrective receipt also retains the complete decoded RED environment
bytes and original candidate hash, rather than relying on the old unavailable
raw command from task 4.

The standalone probe provides four controlled observations: ordinary and
instrumented binaries, each with the runtime key absent and explicitly supplied.
Inputs include empty, multiline, Unicode and native non-UTF-8 entries. The
ordinary probe exactly preserves its defined inputs. The instrumented probe adds
this literal key/value before application code when absent, and preserves it
when explicitly supplied. No ARC code is linked to the probe. Unique profile
paths prevent stale-profile reuse: ordinary probes produce zero profiles;
instrumented probes produce one profile each. Pinned `llvm-profdata show
--all-functions --counts` confirms the seeded probe's main function executed
once with nonzero counters. Explicit input does not disable profiling.

The repaired test recognizes only this named initializer marker and verifies its
observed value equals the known literal. When the profiling runtime is active,
that literal is an explicit input to BOTH direct and wrapped commands and an
expected value in BOTH outputs. No other host environment value is copied.
Ordinary native execution retains the original unseeded synthetic environment;
its actual receipt has `runtimeMarkerInput: false`. Instrumented execution has
`runtimeMarkerInput: true`. Neither comparison filters keys, normalizes values,
substitutes binaries, skips a case, changes production environment handling, or
uses a coverage-only production cfg. Whole stdout byte equality still compares
the complete native environment representations, including Python startup
entries. Eight expected-failure controls call the same comparison and detect
additions, removals and changes involving non-UTF-8, empty and multiline values.

### Exact focused commands and outcomes

All commands have hash-bound local receipts and raw stdout/stderr under
`target/exec-fail-open/task8-correction/`; the compact corrective receipt lists
actual argv, PIDs, elapsed times, binary identities and before/after fingerprints.
Verification wrappers allowed 3,600,000 ms. Native test/instrumentation runs were
serialized; inspection of a completed immutable probe profile overlapped the
non-collecting preservation tests. No final coverage collector was run.

```sh
# RED; same pinned setup as scripts/coverage/run.ts, in a dedicated local target:
CARGO_LLVM_COV_TARGET_DIR="$PWD/target/exec-fail-open/task8-correction/instrumented" \
LLVM_PROFILE_FILE_NAME='arc-%p-%m.profraw' CARGO_LLVM_COV_DENY_WARNINGS=1 \
RUSTUP_TOOLCHAIN=1.95.0 CARGO_NET_OFFLINE=true CARGO_TERM_COLOR=never \
rustup run 1.95.0 cargo llvm-cov --workspace --all-targets --locked --offline \
  --no-report --no-cfg-coverage --no-cfg-coverage-nightly \
  -- every_outage_preserves_the_complete_native_environment_byte_for_byte
# GREEN boundary: same setup/flags, with -- fallback_environment::
# GREEN entire native suite: same setup, replace --all-targets with
# --test managed_exec, and omit the trailing name filter.
```

The actual commands additionally supply `LLVM_COV` and `LLVM_PROFDATA` from the
matching installed Rust toolchain, as recorded in each raw receipt. The original
RED test body can be recovered from accepted task-3 input; its pre-correction
bytes hash to `2ff7df390775837e6d4d5094956b880bc128b1b03e8be6b4cc77c4613edd3e50`.
On the delivered source, these commands are GREEN reproductions.

- Initial `cargo llvm-cov` selector without `--`: exited 1 before execution
  (`unrecognized subcommand`). The corrected invocation above is the actual RED.
- Standalone probe compile commands: `rustup run 1.95.0 rustc --edition=2021
  crates/arc-daemon/tests/managed_exec/runtime_environment_probe.rs -o <local>`;
  repeat with `-C instrument-coverage --cfg=trybuild_no_target`. Both passed.
  Explicit byte-valued inputs and all four observed outputs are hash-bound in
  `runtime-probe-clean-observations.json`; `probe.py` reproduces them locally.
- Original instrumented environment test: exited 1, one actual assertion failure.
- Corrected instrumented environment selection: two passed, none failed/ignored;
  other workspace tests were explicitly filtered, not claimed as a workspace gate.
- `cargo test -p arc-daemon --test managed_exec --locked --offline`: final ordinary
  run, 36 passed, zero failed/ignored/filtered. The instrumented native suite also
  passed all 36 with zero failed/ignored/filtered. Each includes 13 environment
  outage cases, eight mismatch controls, exit 17, one invocation, no recovery
  replay, descriptors, streams, signals, pipelines and historical routes.
- `cargo test -p arc-cli --tests --locked --offline`: 35 passed on the intermediate
  fixture source. `cargo test -p arc-cli --bin arcctl --locked --offline`: 19 unit
  tests passed on the final fingerprint, including deadline and signal handoff.
- `rustfmt --edition 2021 --check` on the three owned Rust paths: passed.
- `cargo clippy -p arc-cli -p arc-daemon --all-targets --locked --offline --
  -D warnings`: passed.
- `bun test scripts/harness/exec-fail-open-docs.test.ts`: two passed. C1 is resolved
  against the unchanged exact-copy checker and the actual non-launching paths in
  `preparation.rs:34-36,54,67-70`.
- `bun test scripts/harness/managed-exec.test.ts`: twelve passed. Migration bytes,
  index/mode checks, assertion protections, coverage denominator, six-crate floor
  definitions, profile/PID merge protections and 500-line bounds remain protected.
- `bun test scripts/harness/exec-fail-open-preservation.test.ts`: exited 1; one
  passed and one failed at the explicit fixture hash pin. This is not a pass.

### Ownership conflict and stable task-4 handoff

`scripts/harness/exec-fail-open-preservation.ts:77` pins the owned environment
fixture to SHA-256
`2ff7df390775837e6d4d5094956b880bc128b1b03e8be6b4cc77c4613edd3e50`.
Its line 79 rejects the reviewed corrective bytes, now
`f778416771edac2fcc692f570b92480cb5ea908e25af88b69a21e749e131fd72`,
with `EXEC_FAIL_OPEN: historical helper changed:
crates/arc-daemon/tests/managed_exec/fallback_environment.rs`.
The assignment explicitly forbids editing validators. Chief coordination is
required for a separately owned amendment that continues protecting the reviewed
fixture. No guard was removed or weakened here. Re-run its complete positive and
tamper tests after that amendment; the tamper test passing while this hash mismatch
exists does not establish that each deliberate mutation independently caused its
rejection.

Final corrective source fingerprint:
`62238c2ce55acf00c179e38527c0de31e1d7dd78bd2ee6fadec390a9c9c00e57`.
Both final native suites and focused final unit/style/docs/protection checks bind
this fingerprint. Earlier intermediate/RED fingerprints remain distinct. A later
validator amendment changes the fingerprint and requires fresh downstream gates.
The compact receipt binds expanded input inventory, raw logs, runtime probe
observations, candidate binaries and every fresh native timing case. It is focused
corrective evidence, not a replacement for task-4's required workspace receipt.

Acceptance checklist: RED attribution, equivalent complete native comparisons,
negative controls, ordinary/instrumented execution and C1 clarification are
verified. Existing production execution and interfaces are unchanged. The old
migration, accounting and historical assertions remain unchanged. The three owned
Rust files are 205, 17 and 232 lines. All six 80% coverage floors remain required;
no new full coverage measurement or floor pass is claimed. The fixture pin needs
Chief coordination before the final schedule. Independently authored QA, final
workspace formatting/Clippy/tests, all required coverage/live checks, full harness
and final/terminal sealing remain downstream work. Keep desktop, native Windows,
vendor hooks and scheduling outstanding. User and commercial outcomes remain
unmeasured.

There were 111 evidence files before this attempt; one compact corrective receipt
brings the present set to 112. No staging or mission-state change was performed.
Large logs, binaries, profiles and expanded observations stay in ignored target
artifacts with hashes. The complete earlier implementation note is retained as
an unchanged prefix, and every earlier evidence file except this additive note
retains its original bytes. No retained receipt or acceptance contract was edited.

Fresh final native maxima: ordinary **1813.487 ms**; instrumented **1816.647 ms**.
Each thirteen-case matrix records one invocation, exit 17 and one total invocation
after recovery. The measured boundary includes wrapper startup, metadata, native
handoff, scheduling and observer overhead. Internal subdivisions were not
observed or subtracted; the unchanged metadata deadline is 1,800 ms.


## Task-8 attempt 3: preservation-owner handoff (2026-11-08 UTC)

Status: **blocked on validator ownership**. Skill: oma-backend. This attempt
inspected the integrated checkout before work and made no product-source changes.
The corrective source and all five retained binary identities still match
`implementation/task8-attempt2-correction.json`. The current 213-input source
fingerprint remains
`62238c2ce55acf00c179e38527c0de31e1d7dd78bd2ee6fadec390a9c9c00e57`.
The earlier RED/GREEN and initializer-provenance evidence remains unchanged; no
new ordinary/instrumented suite, coverage collector or final gate is claimed.

Fresh commands, each allowing 3,600,000 ms:

- `bun test scripts/harness/exec-fail-open-preservation.test.ts`: exit 1,
  **1 pass, 1 fail**, 23,830.527 ms. The positive test fails with
  `EXEC_FAIL_OPEN: historical helper changed:
  crates/arc-daemon/tests/managed_exec/fallback_environment.rs`. The passing
  tamper test is inconclusive while that unconditional mismatch exists.
- `bun test scripts/harness/exec-fail-open-docs.test.ts`: exit 0,
  **2 pass, 0 fail**, 91.388 ms. C1 clarification remains compatible with the
  unchanged documentation checks.

### Exact handoff to the Chief

Task-8 says: “Do not edit coverage accounting, collectors, validators, historical
assertion bodies, fixed-baseline routing, migration 0001, .agents definitions or
retained receipts.” The supplied review requires a **Chief-assigned validator
owner**. No such ownership handoff accompanies this attempt. Backend therefore
has no authority to change the remaining failing file.

Assign `scripts/harness/exec-fail-open-preservation.ts:77` to a validator owner
for the single reviewed fixture-pin amendment:

```text
fixture: crates/arc-daemon/tests/managed_exec/fallback_environment.rs
old: 2ff7df390775837e6d4d5094956b880bc128b1b03e8be6b4cc77c4613edd3e50
new: f778416771edac2fcc692f570b92480cb5ea908e25af88b69a21e749e131fd72
```

Keep the protection assertions, all other pins and historical routing intact.
Then run the complete positive and tamper checks above, establishing a passing
clean fixture before treating deliberate mutations as evidence. This is a
concrete source/ownership conflict, not an implementation-method question for the
operator. No validator edit, workaround fixture, production change, assertion
weakening, staging, mission-state update or recursive AV invocation was made.
Public execution and IPC interfaces are unchanged.

`implementation/task8-attempt3-handoff.json` binds the fresh failure, exact argv,
PID, time, local output hashes, current source inventory and proposed handoff.
Raw outputs remain in ignored `target/exec-fail-open/task8-attempt3/`. Before
appending this note, every one of the 112 existing evidence files was checked
byte-for-byte unchanged; this receipt brings the file inventory to 113. The
prior note is retained as an unchanged prefix. No failed receipt was rewritten.

Acceptance: the prior corrective boundary, native equivalence, mismatch controls
and C1 work are preserved, with no new code changes. The mandatory preservation
positive check still fails; task-8 is **not complete**. After the separately owned
pin amendment is independently accepted, task-4 must collect final gates against
the changed source fingerprint. Coverage floors, full harness, independent QA and
final/terminal artifacts remain pending. Desktop, native Windows, vendor hooks
and scheduling remain outstanding. No downstream pass is claimed.


## Task-8 attempt 4: reviewed fixture pin and mutation controls (2026-11-08 UTC)

Status: scoped corrective repair complete; independent acceptance and final gates
remain downstream. Skill: oma-backend. The Chief's explicit ownership amendment
authorizes only the preservation validator, its tests, additive notes and one
compact corrective receipt in this wave. No AV operation or mission-state change
was made. Serena was unavailable; inspection used supplied paths, direct reads
and Git observations. The unrelated av.yaml access denial was not bypassed.

Before editing, the retained attempt-2 receipt matched SHA-256
`3ffa6df0d3c810a5b38f1d0a64e7b147f17147a0192bbd45f8a1cea403ab02f6`
and attempt-3 matched
`3927fe54b4b346a0c6a2c02366df6f1bfebb002d3eda776a692b073f4792a8df`.
All 91 referenced attempt-2 artifacts and five candidate/probe binaries match.
The reviewed Rust fixture remains
`f778416771edac2fcc692f570b92480cb5ea908e25af88b69a21e749e131fd72`.
Its runtime probe, all product execution code and the C1 help/docs clarification
retain their pre-task bytes. No production behavior or public interface changed.

The production validator changes exactly one SHA-256 literal: the old environment
fixture pin `2ff7df390775837e6d4d5094956b880bc128b1b03e8be6b4cc77c4613edd3e50`
becomes the independently reviewed digest above. The strict comparison, every
other pin, fixed historical routing, assertion protections, migration identity,
coverage accounting and 500-line check are unchanged.

The existing mutation suite now requires its temporary checkout to pass before
mutation and after every restored case. All eleven original mutation recipes and
assertions remain. A separate case replaces the complete-environment equality
with a self-comparison, requires the exact rejection
`EXEC_FAIL_OPEN: historical helper changed: crates/arc-daemon/tests/managed_exec/fallback_environment.rs`,
restores the original Buffer byte for byte, and passes preservation afterward.
Both temporary suites check the clean state on entry and exit. The integrated
checkout also passes its existing positive check.

Fresh command evidence (cwd and native host, timestamps, PIDs, exact argv, exits,
elapsed measurements and output hashes are in the receipt):

- Before the edit: `bun test scripts/harness/exec-fail-open-preservation.test.ts`
  exited 1, 1 pass/1 fail, 22,764.376 ms. The stale fixture pin is the observed
  failure; the old tamper result alone is not accepted as mutation proof.
- After the edit: the same command exited 0, 3 pass/0 fail, 41 expectations,
  62,714.119 ms. Both clean fixtures, eleven existing mutations and the new
  fixture-specific mutation/restoration completed without skips.
- `bun test scripts/harness/exec-fail-open-docs.test.ts` exited 0, 2 pass/0 fail,
  82.417 ms. C1 remains clarified at main.rs:44 and managed-exec.md:11.
- Scoped `git diff --check` exited 0. Additional no-index checks of the exact
  reconstructed before/after source bytes produced no whitespace diagnostics;
  Git returned 1 because these untracked files differ. That status is retained
  and is not represented as an exit-zero check.

Every command wrapper permits 3,600,000 ms. No execution/retry budget was reset.
The mission retains its 86,400-second limit; no absolute deadline was injected
into this process. No full collector, build or final-gate replay was performed.

Task-4 input fingerprint (213 inputs):
`691e213bc85144f5079f80f39e91100a146013855db66ddb80d9f4dc85aeaaf3`.
Only the two authorized validator files differ from the retained corrective
input map. The receipt binds their exact before/after hashes and local patches,
the expanded input inventory and actual Git observations. Raw observations and
logs stay in ignored target/exec-fail-open/task8-attempt4/. Reproduction: run the
two Bun commands above in this integrated checkout.

Retained native evidence is reused only for its original source scope:
ordinary and pinned LLVM-instrumented runs each passed 36 native tests. Their
thirteen complete-environment observations were rechecked for byte equality,
all defined input bytes, one invocation, exit 17 and no recovery replay; each
retains eight negative controls for added/removed/changed values, including
non-UTF-8, empty and multiline inputs. Native maximum times remain 1813.487 ms
and 1816.647 ms, respectively. The earlier RED case and standalone initializer
probe remain hash-bound in attempt-2; no runtime attribution or native outcome
was invented or recollected for this validator amendment.

All 113 prior evidence files were checked unchanged before this additive note;
the original note remains an exact prefix. One compact receipt brings the
tracked-intended inventory to 114, leaving room below 150 for QA/final/terminal
artifacts. No file was staged. Required next steps: independent review of this
narrow correction, task-4 fresh source-bound gates with all six 80% coverage
floors and the full no-skip harness, then live verification after the final
collector, task-6 independent oma-qa and task-7 final assessment. This correction
does not establish any downstream pass. Desktop, native Windows, vendor hooks
and scheduling remain outstanding; commercial outcomes remain unmeasured.


## Task-10 attempt 1: endpoint access and request uncertainty (2026-11-09 UTC)

**Status:** scoped backend repair and focused verification complete. Independent task review, task-11 validator integration, task-4 final collection, independent oma-qa and mission acceptance remain pending. Skill used: `oma-backend`.

### Source and delivery scope

The accepted committed baseline is `00773c69a8e5e7fd417fd5cc78d03e222364f609`, with tree `b98f996caa04fe13eeeeab1072c3c528f6d18c65`. This Actor checkout began at actual local HEAD/snapshot `d733b0b2fa34f888d1384c36d54a34755146cff2`, tree `e18147b1527b9c186875484f19a39fad3124806c`. These are distinct scopes: accepted task deliveries were present in the isolated snapshot, and this task’s edits are uncommitted worktree changes. The supplied snapshot and worker refs each failed local `git rev-parse --verify` with exit 128; no other revision was substituted. The initial status was clean for the accessible checkout excluding `av.yaml`, whose access was denied. Serena tools were unavailable; known paths were read directly.

Current maintained input fingerprint is `d2420c306d847be8538e44474d97db7d932e702fb14c37f835a9bc72fa811059` across 223 inputs. The full local input inventory is `target/exec-fail-open/task10/source-inputs-final.json`, SHA-256 `a1bc1338becc1653875939b45f1dfddfb59eaffa6df996b667997391c7d33fb2`. The checks below record this fingerprint before and after execution. Exact snapshot and delivered worktree bytes:

| Path | Snapshot SHA-256 | Worktree SHA-256 |
| --- | --- | --- |
| `crates/arc-cli/src/exec/transport.rs` | `34a636d5a67c58a4bde633a15600095bcfac2d64dafe4f66d1b9714938514451` | `fd812936d4f576c8eb7e31c6243d95582e38adfbf05c1e90b0052c2409125c3a` |
| `crates/arc-cli/src/exec/preparation.rs` | `08febe91f0b976559c669f7e3866976e4073a13d06834610f1daf0de107bb4fd` | `757dcc559ecddb58cf311750033e49c36d5268f3efc24ff26df8e01e9acf0aa1` |
| `crates/arc-cli/src/exec/transport/access_tests.rs` | `absent` | `7caf2ae234de8755689ccaf9ac4c2579fe18901707f8a3226f814d9df5d82656` |
| `crates/arc-daemon/tests/managed_exec.rs` | `a62a98b684eadd577612345eb67fef9c9515c49ccb2f5fb9ce4c389689aede0f` | `0d08d6851a2d8184e486460142899db11b3f07f16e9ae0d8a4e8f1be27048604` |
| `crates/arc-daemon/tests/managed_exec/connect_access.rs` | `absent` | `1730bc259555071b578be0e35db88bcf856ce04f0315b07fa6bd0a6adb50cedf` |

`transport.rs` distinguishes socket endpoint errors from descriptor creation, setup, and `SO_ERROR` retrieval failures. `EACCES` and `EPERM` arising from connect or its completed socket error take the existing retry path; an invalid endpoint address and local setup errors remain local failures. A request that may have been sent keeps the existing uncertainty bit across subsequent endpoint failures. Confirmed REGISTER followed by an unavailable CLAIM remains uncertain. Cancellation is checked before retries, after exchanges, and at final exhaustion; explicit authenticated rejection, malformed replies and protocol errors stay non-launching. The same absolute deadline and the existing single CLI-owned spawn, IPC, signal, terminal and storage paths are retained. `preparation.rs` exposes only an internal injectable preparation helper so deterministic tests can verify both stages and the same deadline.

The new native fixture places the proxy socket in its own test-owned directory. It verifies actual native `EACCES` before proceeding, restores permissions on unwind, and joins its proxy before restoration. For lost acknowledgements it forwards the actual request to the daemon, checks the successful daemon reply and the committed SQLite revision before denying later connections, then withholds that reply. The confirmed-REGISTER case denies the next connection before forwarding its acknowledgement. It asserts the child parent PID equals the CLI PID, exact native fixture stdout/stderr after removing static ARC diagnostics, argument/cwd/environment and stdin equality, exit 17, one marker, correct tracking text, no secret in diagnostic or persisted fixture files, zero permits and garbage-collection operations, preserved sentinel, and one marker after daemon restart and repeated status calls. The fixture uses a bounded blocking handshake read; the complete measured interval starts before spawning the wrapper and ends after the child handshake, without subtracting scheduling or observation overhead.

### Native cases from the full current-source run

The full run produced the following locally retained compact observations under `target/exec-fail-open/native/20868/`. Every case observed native errno 13 (`EACCES`), one CLI-owned child, exit 17, one total invocation after recovery, zero permits and GC operations, and unchanged sentinel. Rows refer to the independently read committed SQLite state before acknowledgement handling. Candidate `arcctl` SHA-256 was `cdf160a1829753c1b693c2d54191691031c719e8b97611718c1fdad5a584c1a6`.

| Stable test suffix | Parent spawn to child handshake (ms) | Commit and acknowledgement | Observation SHA-256 |
| --- | ---: | --- | --- |
| `connect_access_denied_before_register_runs_once` | 9.658792 | No request sent; unavailable | `0e98313e744d45cf66772a27e0cfe2e09cb62aa6cf533c3a48614bbfdcfa871d` |
| `connect_access_denied_after_register_ack_loss_stays_uncertain` | 19.327750 | REGISTER revision 1 committed; ack withheld | `573ae634409a50e863e80946e2b281275c228eb091fef54325ae4a527329170a` |
| `connect_access_denied_before_claim_stays_uncertain` | 12.297291 | REGISTER revision 1 committed and ack forwarded; CLAIM denied | `a0ae9432ca2fec461b010c185ca3b74e14861dcfb1b63338786de8322c8a1a92` |
| `connect_access_denied_after_claim_ack_loss_stays_uncertain` | 16.616334 | REGISTER revision 1 and CLAIM revision 2 committed; CLAIM ack withheld | `3b76c2589d2e1b59330c2eb11403cd6c6664f5119610a4642f7fec33718a2bec` |

### Actual commands and outcomes

All commands ran in this Actor checkout on Darwin 27.2.0 arm64, Rust/Cargo 1.95.0 and Bun 1.4.4. Each command wrapper allowed 3,600,000 ms. No original absolute mission deadline was injected into this process; none was invented or reset. Elapsed values use a monotonic clock. Per-command `record.json` files under `target/exec-fail-open/task10/<record>/` include exact argv, cwd, UTC start/end, PID, exit, source bindings, and output paths/digests. Raw stdout/stderr remain alongside them. Filtered focused runs are identified below rather than described as full-suite runs.

| Record and actual command | Start UTC | Elapsed ms | Exit and result | Record SHA-256 |
| --- | --- | ---: | --- | --- |
| `native-red-observed`: `cargo test -p arc-daemon --test managed_exec connect_access_denied_before_register_runs_once -- --nocapture` | 2026-11-09T01:54:23.439323+00:00 | 4142.248 | 101; 0/1, 39 filtered; native errno 13 followed by NOT_STARTED before child handshake | `fa38e0be7f6d0507a313e7964b2c38ef86b0a0b16febc68c92676136688f538c` |
| `clippy-focused`: `cargo clippy -p arc-cli -p arc-daemon --all-targets -- -D warnings` | 2026-11-09T02:00:08.935589+00:00 | 6022.358 | 101; test fixture used non-octal permission literal; corrected to 0o000 | `ff9a12e6699adac0d9b8b59bba3c6ee45db8dacb2270f28574336d579193a860` |
| `managed-delivery`: `cargo test -p arc-daemon --test managed_exec` | 2026-11-09T02:11:37.212478+00:00 | 72627.701 | 101; 39/40, zero ignored; fixture handshake read returned WouldBlock | `2952073402bdfea48b9ec10ebe32aecb62447b7d25944cec9f9a1d8f58a17202` |
| `managed-race-fix`: `cargo test -p arc-daemon --test managed_exec` | 2026-11-09T02:13:04.097707+00:00 | 73313.175 | 0; 40/40, zero ignored and zero filtered | `87ebac0cf965e9e558888528465b641e116578c869416debe4e7b68fb73545e9` |
| `native-race-check1`: `cargo test -p arc-daemon --test managed_exec connect_access:: -- --nocapture` | 2026-11-09T02:14:24.527611+00:00 | 847.667 | 0; 4/4, zero ignored, 36 filtered | `0cd00f79272eedba7eba892923dbdf56c2fffbd2d71350e711ed448b49c302b4` |
| `native-race-check2`: `cargo test -p arc-daemon --test managed_exec connect_access:: -- --nocapture` | 2026-11-09T02:14:27.418551+00:00 | 769.722 | 0; 4/4, zero ignored, 36 filtered | `3864616ba1e3ac288fd0d10318a1141aa78a47cb2779e579c9e51d162b802e0d` |
| `cli-race-fix`: `cargo test -p arc-cli --tests` | 2026-11-09T02:14:30.354248+00:00 | 5978.345 | 0; 39/39 across harnesses, zero ignored and zero filtered | `824653f42e0a2aebbaea33d022dd924653839c9f7669ca6c805e5f6a6ed30986` |
| `clippy-race-fix`: `cargo clippy -p arc-cli -p arc-daemon --all-targets -- -D warnings` | 2026-11-09T02:14:41.471091+00:00 | 1328.076 | 0; warning-denying focused Clippy passed | `11cda20191801b0b2bcbeda1f10bd816bc93776d3485fd9329de434554c80318` |
| `fmt-race-fix`: `cargo fmt --all -- --check` | 2026-11-09T02:14:46.812037+00:00 | 420.868 | 0; workspace formatting check passed | `3a2e4977c52cb3b9ae4a0498ae42abe031e0a4d309909308d16e865a86674765` |
| `preservation`: `bun test scripts/harness/managed-exec.test.ts scripts/harness/coverage-verification.test.ts scripts/harness/equivalence.test.ts scripts/harness/exec-fail-open-preservation.test.ts` | 2026-11-09T02:05:01.706837+00:00 | 264383.885 | 0; 18/18, no skips; older input fingerprint 621552d6… | `dfa459ae9f5ddab1893274eecc979c704ac755acdb71fc26b15def33ff16a6ca` |
| `preservation-race-fix`: `bun test scripts/harness/exec-fail-open-preservation.test.ts` | 2026-11-09T02:14:50.530904+00:00 | 66480.808 | 0; 3/3, no skips, 41 expectations | `5f4bc547ff71d1df921fe3bb73065253b0f0197dcd6846533cf9f32f5957edff` |
| `docs-race-fix`: `bun test scripts/harness/exec-fail-open-docs.test.ts` | 2026-11-09T02:16:00.881940+00:00 | 80.929 | 0; 2/2, no skips | `122006c1981a8c19ccd466513be54e2182642935f71b2e95e3c7174dd9a354b6` |

The RED run used snapshot transport SHA-256 `34a636d5a67c58a4bde633a15600095bcfac2d64dafe4f66d1b9714938514451`; its stdout and stderr SHA-256 values were respectively `b1e89e1ba2f8dd0df90ad520f0854317fa3e0e260ad03be3f75c1194a3106ab3` and `51a5ab41af37e07be2661df4789a74d2c84e341e6b11ec2e028839483c039ebe`. The failed full run exposed a test-only race: the accepted control connection could retain nonblocking mode and return `WouldBlock` before its frame arrived. The fixture now switches that accepted socket to blocking with a three-second read timeout; the unchanged 2000 ms assertion still includes the entire read. The corrected full suite and two additional focused runs passed.

The earlier 18-test protection command ran against fingerprint `621552d6fb15075bd8e430a6baa74b17e1703f12f8f352aab3196d0e1c57267a`. Since then the sole affected product input was the new native test module: child-parent assertion/evidence and bounded blocking handshake were added. The final current-fingerprint preservation suite and docs suite passed afterward. The broad protection tests temporarily overwrote three pre-existing `.arc-verify/harness-speed/` receipts; their generated bytes and hashes are retained in `target/exec-fail-open/task10/preservation/generated-*.json`, and the three original files were restored byte-for-byte from the initial local snapshot.

### Preservation and downstream handoff

Pinned `transport/tests.rs`, `fail_open.rs`, `fault.rs`, `historical.rs` and `fallback_environment.rs` match the Actor snapshot byte-for-byte; no pinned body, test registry other than the additive module declaration, validator, accounting, collector, manifest, lockfile or migration was edited. Migration `0001_phase0.sql` has SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384` at accepted baseline, local snapshot, index and worktree. Changed Rust files contain 309, 141, 220, 43 and 438 lines respectively in table order, all below 500. Scoped whitespace checks passed. The original implementation-notes prefix remains byte-for-byte intact.

Task-11 should consume the four exact test suffixes above and the `connect_access` module without changing protected historical tests. Task-4 must recollect the full workspace, coverage/live-verification and no-skip harness gates on the integrated delivery, then have independent oma-qa and final review assess the results. The retained task-4 attempt-8 gates and coverage use its older source fingerprint `0d289d9012f2145faff637a5691da0953c39752798338a4499bf44dc00a8899f`; they are not current acceptance for these uncommitted edits. No final coverage or mission pass is claimed. Desktop, native Windows, vendor hooks and scheduling remain outstanding. No new tracked evidence file was added; the existing tracked evidence inventory is 143 files.

## Task-10 attempt 2: invalid socket classification

### Scope and provenance

This corrective delivery used `oma-backend` in the AV-provided isolated checkout on Darwin 27.2.0 arm64 with Rust/Cargo 1.95.0 and Bun 1.4.4. The accepted committed source baseline is `00773c69a8e5e7fd417fd5cc78d03e222364f609`, tree `b98f996caa04fe13eeeeab1072c3c528f6d18c65`. The actual inspected Actor-local snapshot HEAD is `1b2c426cad8e9935d666f0bd7c18042c678be315`, tree `4310f82244e45ecb49e03c894497034b1a452061`, whose parent is the accepted baseline. This snapshot includes prior integrated deliveries and is not the committed baseline. The supplied snapshot and worker ref names were not resolvable within this isolated clone (`git rev-parse --verify` exited 128 for both); the snapshot commit itself and its tree were verified locally. The changes below are uncommitted Actor-worktree edits, including a new untracked test file; their integration must be checked against the delivery, not inferred from HEAD.

`endpoint_failure` now recognizes specific connection outages, including endpoint `EACCES`/`EPERM`, absent, refused, unreachable, interrupted, in-progress, stalled or disconnected endpoints. Other errno values, including `ENOTDIR`, `ENOTSOCK`, invalid protocol/address configuration, descriptor exhaustion and memory/buffer exhaustion, remain local, non-launching failures. This common classifier handles both direct `connect` and deferred `SO_ERROR`; descriptor creation, nonblocking setup, close-on-exec setup and `getsockopt` failure remain local regardless of errno. Cancellation still takes precedence. Existing retry, shared-deadline, successful managed tracking, authenticated rejection, malformed-reply handling and CLI-owned spawn paths were not changed. The retained unit cases exercise unavailable-before-REGISTER, a sent request followed by endpoint denial, confirmed REGISTER followed by CLAIM denial, cancellation and authenticated rejection.

### RED reproduction

Before modifying transport, the unit regression failed on errno 20 (`ENOTDIR`): the classifier returned `Endpoint` instead of `Local`. The two native macOS tests independently probed the real socket errors and then exercised the retained candidate: `/dev/null/arc.sock` returned errno 20 and `/dev/null` returned errno 38 (`ENOTSOCK`); each wrapper incorrectly exited 17, created the invocation marker, emitted child stdout and reported tracking started. Both RED commands were filtered reproductions, not full-suite results. The RED source fingerprint below binds the snapshot transport plus the newly added regression files; a separate RED binary digest was not captured.

### Source binding

The focused GREEN commands used the same input fingerprint before and after every command: `67730edd35c4fa46ed1e9fbf08c03a7f5ec2fb8db4660f620b66bbd71c05a34f` across 224 files, as computed by the existing `currentInputs` helper. The RED fingerprint was `4326278471c88f0f72a80c72830fbda19c6f45134a9d1d0e8d6cf0c7a893b313` across 224 files. Full per-command path/mode/hash inventories and raw output are retained locally under `target/exec-fail-open/task10-attempt2/<record>/`; `record.json` binds argv, working directory, UTC times, elapsed time, exit, source fingerprints, and stdout/stderr hashes. Each wrapper allowed 3,600,000 ms; the commands were run serially. No independent mission deadline was injected into this process. The mission configuration supplied a one-day overall limit, three-way maximum parallelism and three retries; this attempt used serial focused checks. At start the filesystem had approximately 86 GiB free, the descriptor limit was 1,048,576, and no CPU time limit was set.

| Actor-worktree path | SHA-256 | Lines |
| --- | --- | ---: |
| `crates/arc-cli/src/exec/transport.rs` | `fe0bb94493b0813000d9bbe795fa656d6ba11ebf4c9017de576aea0803d1cfdd` | 332 |
| `crates/arc-cli/src/exec/transport/access_tests.rs` | `7f22a37a060787d899aadeaebd6c926204b3dba78bab79c18a9bda13186fbdbf` | 295 |
| `crates/arc-daemon/tests/managed_exec/invalid_socket.rs` | `eaceecac271277e76095a6ad96e0329e8dbbfee9338e1c0711e8ad5650421585` | 52 |
| `crates/arc-daemon/tests/managed_exec.rs` | `56410a2ba90e75f8b1f421bd3e73d40aeb926c1296653f0a7fea26120ff20ad2` | 47 |
| `crates/arc-daemon/tests/managed_exec/connect_access.rs` | `1730bc259555071b578be0e35db88bcf856ce04f0315b07fa6bd0a6adb50cedf` | 438 |

The retained `connect_access.rs` bytes match this Actor snapshot; it was not edited. The candidate `target/debug/arcctl` used by the native GREEN suite had SHA-256 `e032b8a3fbbc5966794b3c1a32028b063c8baf882fe748cbd7480d42cdf89a96`. Its fixture binary had SHA-256 `a51691c0b38c6c1a7cf44436b4cf6004a0469db1adb500757190e110e80a9e94`.

### Actual commands and results

All elapsed times are wall-clock monotonic milliseconds. The full CLI and managed suites had zero ignored and zero filtered tests. The Bun suites had no skipped tests. RED commands intentionally selected only the new cases.

| Record | Actual command | Start UTC | Elapsed ms | Exit and observation |
| --- | --- | --- | ---: | --- |
| `unit-red` | `cargo test -p arc-cli --tests invalid_socket_paths_and_local_resource_errors_do_not_launch -- --nocapture` | 2026-11-09T02:25:57.119012+00:00 | 10658.279 | 101; 0/1, 24 filtered; expected RED |
| `native-red` | `cargo test -p arc-daemon --test managed_exec invalid_socket_ -- --nocapture` | 2026-11-09T02:26:13.567566+00:00 | 9959.587 | 101; 0/2, 40 filtered; expected RED |
| `cli-green` | `cargo test -p arc-cli --tests` | 2026-11-09T02:26:49.216168+00:00 | 8747.124 | 0; 41 passed, 0 ignored, 0 filtered |
| `managed-green` | `cargo test -p arc-daemon --test managed_exec -- --nocapture` | 2026-11-09T02:27:04.745522+00:00 | 88376.875 | 0; 42 passed, 0 ignored, 0 filtered |
| `fmt` | `cargo fmt --all -- --check` | 2026-11-09T02:28:43.345416+00:00 | 524.873 | 0; formatting passed |
| `clippy` | `cargo clippy -p arc-cli -p arc-daemon --all-targets -- -D warnings` | 2026-11-09T02:28:45.848107+00:00 | 5266.023 | 0; focused warning-denying Clippy passed |
| `docs` | `bun test scripts/harness/exec-fail-open-docs.test.ts` | 2026-11-09T02:28:53.298699+00:00 | 42.499 | 0; 2 passed, 0 skipped |
| `preservation` | `bun test scripts/harness/managed-exec.test.ts scripts/harness/coverage-verification.test.ts scripts/harness/equivalence.test.ts scripts/harness/exec-fail-open-preservation.test.ts` | 2026-11-09T02:28:56.844671+00:00 | 218285.760 | 0; 18 passed, 0 skipped, 296 expectations |

Each row below hashes the exact captured bytes; inventories before and after matched for every command.

| Record | stdout SHA-256 | stderr SHA-256 | record.json SHA-256 |
| --- | --- | --- | --- |
| `unit-red` | `777353366c09fd8c1e4e9fa8ad3cc3e41dc5c945c14a949e7cbde7831c0baf8a` | `0a675718f8b1cdf3ac756d7cb274ecb1816f1f860fc66d525d032398c06b3bd7` | `ecc6b2d7a87087319522616805a0545650ec9a6f184706b8161b22cbdc9aae09` |
| `native-red` | `32bb7175e88c5f4f0b400aba09645ca42e7a0d28672eb2e477286f89ec57133c` | `adf812efead5a16ba82f403d885a714358b7f18be02ca6000efb667d36819b4b` | `4d8ea6fda258be2179177f277323243f03f34b5fc4a16c5057df1d55fa4dad9c` |
| `cli-green` | `6f126a6e584f9ec8327a4ea10aa72bbd626ce83649d341aa5eef4e6fc9432517` | `baf456492e089c6efbef891971d07e2c697f9dfc67e3e974817da493e4fd2d90` | `fac2ed81de5ee086d8d2c7067c22e14467b939adfd86fe11b6baa96033a57636` |
| `managed-green` | `0953f8502f56790b510ed63194e14d1e5a182b990cecf103b7e15d41d7aba1aa` | `fd1eebcfd8d18d34a5fefc48caa05a2a5558589a0f352d7c1e343954850d98d3` | `c6a0d8eef577a75cf50506bef25c4a87b15335a9b52b88a970ef2f65249f6b57` |
| `fmt` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `989b68d8007ec7a97cf0b552083306c83b491b9d13f936529b3f04c5e42adcc6` |
| `clippy` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `7bb9a2b9bfe6481b2938c8755175eb595a5950c1d219a89da834ade49f76971b` | `363f9a99ccb7f88081d43ffe58c5e518ef336603a8c67e1d6aa88fb986992bc5` |
| `docs` | `bbe6a94c29cc8d3ff5a1792a5abae1f1122aed0642111b52036ab7e0b2b2ca47` | `825a34ad875ad5bf22aed51e5f4fb82644e18419b65450c976ee6d7650e7f233` | `1412a51ff303368e71f93d62966ff51dcce42e4889340d86f86f648d802214a3` |
| `preservation` | `674f98d91dfbc02484a3456ac46f6ea55a4419d8ba436ae2124053270b8468f3` | `1040612d387e186c9fb1afd7c79cf2fb080de9e51a7c122b05a7485c747cb314` | `dc93bd566251b3ecd824f8db3c5606b593828b4b9ba22879d4d27feb2248cb98` |

### Native observations

All six requested native tests passed in the 42-test run. The invalid-path tests observed actual Darwin errno 20 and 38 respectively, exit 1, no marker, empty stdout, exact local `NOT_STARTED` stderr, no tracking-started diagnostic, and no synthetic command or environment secret in diagnostics. The four permission cases each observed real errno 13 (`EACCES`), exactly one CLI-owned child, exit 17, native fixture output parity and the correct unavailable or uncertain diagnostic. Their assertions also checked recovery without replay, zero permits and garbage-collection operations, a preserved sentinel and no secret in diagnostics or persisted fixture files. The fixtures deny only their dedicated test socket directory, prove denial before proceeding and restore permissions on unwind. For acknowledgement loss, the proxy forwards the real request to the daemon and reads the committed SQLite revision before withholding the successful reply.

The measurements below run from the parent's monotonic instant before wrapper spawn through the child handshake, without subtracting scheduling or observation overhead. These observations were produced by the current native suite in `target/exec-fail-open/native/38848/`.

| Stable test suffix | Time ms | Real committed state before denial | Observation SHA-256 |
| --- | ---: | --- | --- |
| `connect_access_denied_before_register_runs_once` | 10.537042 | No request; unavailable | `a5684a85ff21bd176c293c01cc9a31885f83b334219ab4323a3390d4dfc98136` |
| `connect_access_denied_after_register_ack_loss_stays_uncertain` | 11.898958 | REGISTER revision 1; acknowledgement withheld | `8bab86b8770e58662f615dfaa80ebf9277d48b518e8e731c06744bc8e1f0f64a` |
| `connect_access_denied_before_claim_stays_uncertain` | 15.520250 | REGISTER revision 1; acknowledgement delivered | `1dd422a9fa81da8f467a4a7ac5a58b4ca61db991e6a1028b7da20060d09f3043` |
| `connect_access_denied_after_claim_ack_loss_stays_uncertain` | 15.678584 | REGISTER revision 1 and CLAIM revision 2; CLAIM acknowledgement withheld | `5e28c5c990022ab9374ed699c3307927899d84ca9f1093570a115dacb5dd9d47` |

All 17 parent-to-child timings in that run, including existing stall and shared-budget cases, were below 2000 ms; the maximum was 1809.909750 ms. The suite also passed its native descriptor, complete-environment, signal, foreground-pipeline, managed tracking and daemon-loss cases. These results are scoped to this Actor snapshot plus the listed uncommitted edits.

### Preservation, failures and handoff

The pinned `transport/tests.rs`, `fail_open.rs`, `fault.rs`, `historical.rs`, `fallback_environment.rs` and `scripts/coverage/accounting.ts` match this Actor snapshot byte-for-byte. Migration `0001_phase0.sql` matches the accepted baseline, Actor snapshot, index and worktree at SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. The protection suite passed its historical assertions, accounting, migration, mutation and 500-line checks. No manifest, lockfile, collector, contract or harness script was changed.

The protection command temporarily rewrote `.arc-verify/harness-speed/coverage-verification.json`, `equivalence.json` and `invocations.json`. Their test-generated bytes are retained under `target/exec-fail-open/task10-attempt2/harness-speed-generated/`; the pre-check bytes were restored from `harness-speed-original/` and verified by SHA-256. `git status` of `av.yaml` returned `Operation not permitted`; it was excluded from subsequent status inspection and was not modified. A read-only `ps` probe was denied by the environment; no workaround was used. The two supplied evidence refs also failed to resolve locally as recorded above. The initial scoped rustfmt check reported formatting differences in the new tests; formatting the four owned Rust paths corrected them before GREEN verification.

Task-11 can consume the stable suffixes `invalid_socket_parent_not_directory_does_not_launch` and `invalid_socket_endpoint_not_socket_does_not_launch` from the additive macOS test module, along with the four unchanged `connect_access` suffixes. Independent task review must inspect the delivered diff and reproduce these classifications. Task-12 must check the combined inputs; task-4 must collect fresh full workspace, coverage, live-verification and no-skip harness gates after integration. Full independent native reproduction remains task-6. No final coverage, independent oma-qa, final mission verification or mission pass is claimed here. Desktop, native Windows, vendor hooks and scheduling remain outstanding. This append adds no evidence file; the Actor snapshot had 143 tracked files under `.arc-verify/exec-fail-open/`.


## Task-15 native signal and diagnostic repair

**Backend handoff; final mission gates and independent QA remain outstanding.** Skill used: `oma-backend`. This is a corrective delivery on the integrated worktree, not a replacement of the blocked task-14 assessment. Desktop, native Windows, vendor hooks and scheduling remain outstanding.

### Admission and ownership

The mission state read at 2026-11-09T18:34:44Z was `executing`, task-15 was `running`, 178 of 200 runs had started, and the original absolute mission deadline was 2026-11-09T20:40:08.584Z (7,524 seconds remained). No cancellation was indicated by that current mission snapshot. The supplied workspace was the writable integrated checkout. Its HEAD was `00773c69a8e5e7fd417fd5cc78d03e222364f609`; unrestricted Git status reported `av.yaml: Operation not permitted`, so scoped Git reads were used. `ps` was also denied; process identities below are obtained from the actual test and probe subprocesses. No mission state was changed.

The task-14 source fingerprint was independently recomputed as `33a540623527224ced9ad5569446aeda301c9c9bfc04dad8137332ba33da4a34` over 229 inputs. Before edits, `target/debug/arcctl` matched candidate SHA-256 `a9e516bb99d624d5a27fdf5fd122bffe8862c2baec128283588f2dcefde64d1d`; both retained task-14 observations and both reproducer hashes matched `final.md`. The original probes and observation files were not rerun or overwritten. New observations and command streams are under the ignored `target/exec-fail-open/task15/` directory.

### Implementation and interface

The existing pre-runtime native initializer now snapshots whether the caller inherited ignored SIGPIPE. The child pre-exec hook restores ignored or default SIGPIPE after Rust `Command` resets it, without changing the caller's signal mask. A focused unit test observed the Rust test runtime's ignored SIGPIPE and the default disposition inside `Command`'s pre-exec hook; the native shell tests independently compare inherited default and ignored dispositions and verify an inherited blocked SIGUSR1 remains blocked. The existing one-child spawn, admission, terminal and signal-forwarding paths remain in use.

Post-spawn diagnostic messages go to a single lazily created worker thread. The worker is created while the creating thread's signals are blocked with `pthread_sigmask`; it inherits that mask, and the creating thread's previous mask is restored. No descriptor status flags are changed. The supervisor does not wait for diagnostic writes; after the child outcome it allows at most 50 ms for queued output. A blocked or broken sink is not asserted to have received a message. The existing `preparation::started(tracking, &job_id)` call remains after successful spawn and now returns the worker state. Existing secret-free tracking messages and normal managed execution are retained.

The new native full-pipe tests fill stderr before starting each child, keep the reader open without draining until the wrapper has returned, and compare direct and wrapped argv/cwd/environment/stdin/stdout, descriptor and descriptor-status flags, and a zero-byte stderr write. A child that itself writes data into a full blocking pipe would still block natively; that is not claimed otherwise. Tests check exit 17, one invocation, unchanged full-pipe contents, no secret persistence, no permits or GC-operation rows, an untouched cleanup sentinel, and no replay after recovery. The uncertain case observes a real REGISTER commit while the fault proxy withholds its acknowledgement. Separate cases cover SIGTERM supervision and a managed completion failure with the same full pipe. The production gate inventory requires both SIGPIPE tests and all four new full-stderr tests, with a fixture regression proving a missing named test is rejected.

### TDD_EVIDENCE and native observations

- RED from a fresh copy of the task-14 probe, 2026-11-09T18:22:10Z: ignored SIGPIPE produced direct exit 17 and `survived`, but wrapped signal 13 and no stdout. The 65,536-byte full stderr pipe let the direct child exit 17 while the wrapper remained alive until the reader was drained. `red-native.json` SHA-256 `d9b2c68e38a47a0adc9d18dcff08ba46682f8a363a35e7491bb983c42cb44569`; the copy `probe.py` SHA-256 `a16b70aff975f82784f9a9d16b519cb7c8446dc565bf15fefdc41af870cef3a3`. Its source was the verified task-14 fingerprint and candidate above.
- The focused RED tests `cargo test -p arc-daemon --test managed_exec fallback_sigpipe -- --nocapture` and `cargo test -p arc-daemon --test managed_exec full_stderr -- --nocapture` exited 101. The first had one ignored-SIGPIPE failure and one default pass; the second had both unavailable and committed-register tests fail because the wrapper did not exit before draining. RED records are `red-sigpipe/record.json` SHA-256 `bf551cd59d7f054a5a5018df94f667ad25be2c749451498c170ab3976513b76e` and `red-full-stderr3/record.json` SHA-256 `dd10c4369e88cb7975491d8a36b8f1156cb051c4df537e64e381131e55cdd610`. Their source fingerprints and exact output hashes are inside those records; the initial recorder's elapsed values included the final fingerprint-read overhead.
- GREEN from the new copied probe at 2026-11-09T18:41:12Z: ignored direct/wrapped PIDs 2107/2108 both printed `survived` and exited 17; default PIDs 2111/2112 both terminated with signal 13. Full-pipe direct/wrapped PIDs 2114/2115 both exited 17 before any drain, in 8.390/100.695 ms respectively. `green-native.json` SHA-256 `f462bf148f935edc7135bd32bac5dde64c31ab2bd2046b8e0d0aaeb9c900b29d`; `green-probe.py` SHA-256 `ba9c2c8158c4d66c2fbe505a048c7707cba6a5a0f3728a6e59291a8bd5dc6315`. This probe is the retained script copy with only its output path and source fingerprint updated.
- The final unfiltered managed suite additionally measured unavailable direct/wrapped spawn-to-ready 24.983/33.780 ms and release-to-exit 155.857/215.688 ms; the real committed-register case measured 27.802/38.834 ms and 156.190/215.114 ms. Release-to-exit includes the fixture's 100 one-millisecond sampling sleeps; no time was subtracted. The direct/wrapped full-pipe SIGTERM cases returned in 1.560/58.442 ms from signal delivery. These are observations on this host, not hard real-time guarantees. The test's 100 flag samples and the parent's repeated checks observed unchanged descriptor flags.
- Seventeen existing macOS outage/access cases each recorded one invocation, exit 17 and one total invocation after recovery. The largest unadjusted parent-before-wrapper-spawn to child-control-handshake measurement was **1816.376 ms**, below 2,000 ms. This boundary includes startup and observation overhead; internal subdivisions were not measured and Phase 0 has no pre-spawn scheduler stage. `timing-summary.json` SHA-256 `4b86cfa1e919abd2e8abed59707bc84cf456dddfec72b688025ad8e141e15854` binds the individual observations under `target/exec-fail-open/native/582/` to the candidate.

### Final source and command receipts

Final input fingerprint: `335aa0add6b601a8b4a4ab8c3349786be41f7ad4dd4188d7182d2c7dbfd8c27a` over 233 inputs. The ignored full input inventory `target/exec-fail-open/task15/source-inventory.json` has SHA-256 `f98e2a6543b3adb6295734a6fc4d229f78ca5829a61ef0fc81d08441ed9b0c11`. The tested `target/debug/arcctl` SHA-256 is `c857ff1cc184f534d4a85e49a85ccde5d9eb071bca43dfdb2ee707b4d0272c87`. The host was Darwin arm64 `Eunkwangui-MacBookPro.local`, kernel 27.2.0, UID 501. Every final command below used the mission workspace as cwd, had a 7,200,000 ms outer timeout allowance, and recorded identical input fingerprints before and after. PID is the command process; observer PID is the recorder. Existing internal harness timers retain the operator-approved values.

| Receipt | Exact command | Start UTC | Elapsed ms | PID / observer PID | Exit | record.json SHA-256 |
| --- | --- | --- | ---: | --- | ---: | --- |
| `cli-handoff` | `cargo test -p arc-cli --tests` | 2026-11-09T18:39:24.549515+00:00 | 7765.233 | 358 / 356 | 0 | `777bb72ba70c99ff8d56dc2f53415f14080b9e207e7f944d53e2478eae3bf2b2` |
| `managed-handoff` | `cargo test -p arc-daemon --test managed_exec -- --nocapture` | 2026-11-09T18:39:39.273365+00:00 | 78018.815 | 581 / 576 | 0 | `f20dadef2bcb487789cdfa6bd8515e1d8f7a6c868a333dd23989687161fded13` |
| `preservation-current` | `bun test scripts/harness/managed-exec.test.ts scripts/harness/coverage-verification.test.ts scripts/harness/equivalence.test.ts scripts/harness/exec-fail-open-preservation.test.ts` | 2026-11-09T18:35:23.057690+00:00 | 221340.997 | 74323 / 74321 | 0 | `522a37eff935deb6dc4f41f4aa4613acdc969b6218821f5e52e760515897c979` |
| `fmt-handoff` | `cargo fmt --all -- --check` | 2026-11-09T18:41:05.364001+00:00 | 543.704 | 2037 / 2035 | 0 | `d91f01a873f0f8064e871bdbfa2e0e174177083c08a1226dbf1650fda402af84` |
| `clippy-handoff` | `cargo clippy -p arc-cli -p arc-daemon --all-targets -- -D warnings` | 2026-11-09T18:41:06.131592+00:00 | 1006.716 | 2048 / 2042 | 0 | `1f0996c708aa0ae6a2ccf3c860c1b991d4bc8d633424c6c7b7f760eb01ece003` |
| `docs-handoff` | `bun test scripts/harness/exec-fail-open-docs.test.ts` | 2026-11-09T18:41:07.370252+00:00 | 364.027 | 2062 / 2060 | 0 | `004006e4a81d9dc4f34a1248bd228e352c731a762fd9e5f66e6b544ab0c210cf` |
| `recipe-handoff` | `bun test scripts/harness/exec-fail-open-recipe.test.ts` | 2026-11-09T18:41:07.949782+00:00 | 998.854 | 2067 / 2065 | 0 | `075ec39d34b4080a6face11b355e7ec7b2db9ba5c92c333ca760fb1e6c00f581` |
| `green-probe` | `python3 target/exec-fail-open/task15/green-probe.py` | 2026-11-09T18:41:12.254190+00:00 | 200.421 | 2106 / 2102 | 0 | `ad1bd7065465dde90d0aaffe5727630d70de92a70bca849f7d96fb1e9c67510b` |

Each receipt is at `target/exec-fail-open/task15/<receipt>/record.json` and binds the exact stdout and stderr files to the hashes below. The unfiltered CLI suite passed 46 tests (plus a zero-test fixture binary), with zero failed, ignored or filtered; the unfiltered managed suite passed 52 with zero failed, ignored or filtered. The preservation group passed 19 tests, the docs check passed 2 and the recipe inventory test passed 1, all with zero failures. The fixture-based inventory test rejected each of the six named regressions when absent from a workspace receipt. Formatting and affected warning-denying Clippy passed.

The preservation group checked source and accounting protections before the final CLI test refreshed the candidate binary; its record therefore contains the previous candidate hash and is not binary execution evidence.

| Receipt | stdout SHA-256 | stderr SHA-256 |
| --- | --- | --- |
| `cli-handoff` | `758c255e7e000273066bab677e398ec3874d69f29bafd546f5f83eb1880d1efb` | `7745ef1fadfed801bd381c951581c66f9913f713fb640c045b0d5eb2421a8e04` |
| `managed-handoff` | `27bdcb66c131241291e1c00729e6176b629e87afed622bb66dca10783de1b615` | `c313cbe4743f38cae98ce72ad436c685c97e41bcf5653e04fd359e2faf441f83` |
| `preservation-current` | `674f98d91dfbc02484a3456ac46f6ea55a4419d8ba436ae2124053270b8468f3` | `5a33ee882c91da0870c534c729a097e84d4cae55ea82feaa1756ded277f85c76` |
| `fmt-handoff` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `clippy-handoff` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `8436e6499d9a7d28cb62160e4e3eac065f54a288cad704837bb3e979ad4a745e` |
| `docs-handoff` | `bbe6a94c29cc8d3ff5a1792a5abae1f1122aed0642111b52036ab7e0b2b2ca47` | `65067968d8e6b71de3306232fc1781322b4f1305c58b2e9c6844f9884c947d66` |
| `recipe-handoff` | `bbe6a94c29cc8d3ff5a1792a5abae1f1122aed0642111b52036ab7e0b2b2ca47` | `713857223314aa6b135678dd2891e71bfa4b1ad338ba6b37d26102571ce80734` |
| `green-probe` | `f462bf148f935edc7135bd32bac5dde64c31ab2bd2046b8e0d0aaeb9c900b29d` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |

Owned source/test inputs and their SHA-256 values:

| Path | SHA-256 |
| --- | --- |
| `crates/arc-cli/src/exec.rs` | `37080fc45bbd5c203a20746a756e377eb6936a9f44c3164bb883671c583ee1d6` |
| `crates/arc-cli/src/exec/descriptors.rs` | `7f1d0f83f516000bfaaffab02c02f77b9b73624c75f6f3f2492ce06c3838fa24` |
| `crates/arc-cli/src/exec/preparation.rs` | `69265a5b6512bc162bbbe852aa267f408c94233eb9a713d8e1bf5dbce1797f7d` |
| `crates/arc-cli/src/exec/signals.rs` | `2519cec35da6704269b75215192b567a900c7f27b417d87a538fee04cbd683af` |
| `crates/arc-cli/src/exec/signals/tests.rs` | `a6ff5cfe04692578d3b8bc66119561d65ad899671973218e52bc738aad45db86` |
| `crates/arc-cli/src/exec/signals/diagnostic_tests.rs` | `64330855b9db0b8287d765bceda78c85c6f335a9b989ad82a93cfbc86de5c399` |
| `crates/arc-daemon/tests/managed_exec.rs` | `5d4255ec49f879fd02fae8e5968c4eaf7da0357593559e55a69d69793369fdf8` |
| `crates/arc-daemon/tests/managed_exec/fallback_sigpipe.rs` | `854e0a80dc0e8d42859dcfe9cb616dc23fdb9d0d4511c1c1c3e85e558da261ec` |
| `crates/arc-daemon/tests/managed_exec/full_stderr.rs` | `cc94721012cbe03a354aad3292afeaa9848fa3de0c8c4c53b8fc5938f6619069` |
| `scripts/harness/exec-fail-open-recipe.ts` | `c4d9cbbd35049217dfa24a21af4de786a68683ef72c6d8bd5e71cafcc159bb32` |
| `scripts/harness/exec-fail-open-recipe.test.ts` | `12be133a869c7a32c3f27e8691be77e9c07a35b8f239ef4a4c3f2e864ede3ac1` |

The two historical-route test invocations append to an ignored shared route log; its original bytes were backed up before the suite and restored afterward (before and after SHA-256 `3597495e45a8c7063a02f89b8b419331329794ec5718405083cfa92257c02750`); the observed appended copy remains under task15. The preservation group temporarily wrote three prior harness-speed observations; all three were restored byte-for-byte and the scoped Git status is clean. The migration `0001_phase0.sql` has matching HEAD, index and worktree SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. Pinned `fallback_sigstate.rs` remains `1e90f5157b6d252b73cf0f4bc5d32d33895b95cafb56fb8ffdfc684b672bb7b7`. The tracked exec-fail-open evidence inventory remains 149 files; no new tracked evidence path was added.

### Failed attempts and handoff

The first full-pipe test setup compared flags captured before restoring blocking mode; Darwin added a status bit during that setup, so the test was corrected to snapshot flags after restoration and before spawn. A second fixture attempt retained its own writer and hung while trying to read EOF; it was interrupted and corrected by dropping the `Command`. The first apparent GREEN native run used the stale task-14 binary because the daemon-only test does not rebuild `arcctl`; running the CLI tests refreshed it before native verification. These are retained local attempts, not passing evidence.

The first thread-mask unit test failed because `sigprocmask` did not make the desired mask inherit to the new macOS thread. `thread-mask-investigation2/record.json` (exit 101, SHA-256 `27ea1bd9f624d7840cb0334a3968e7bda369b62580a9d104d6ce71f0d62fa900`) and `thread-mask-investigation3/record.json` (exit 0 after `pthread_sigmask`, SHA-256 `fe28b47dca5106fcca381ff56026ece30ee974e4a2c85c6bce5b310b78a24857`) retain that failure and correction. The first CLI unit run also found `/bin/true` absent on this host; the test now uses `/bin/sh`. The initial docs check failed on the changed call signature; the existing signature was restored and the check passed. An initial preservation-group run found a forbidden wildcard import and was interrupted with exit 130 after the failure; the explicit-import correction passed the unchanged full group. Its partial log remains under `preservation-delivery/` but has no completed recorder metadata.

Independent `oma-qa` must inspect this delivery and reproduce the direct-versus-wrapped SIGPIPE and full-stderr cases on these bytes. The Chief must refresh the source-bound workspace Clippy/tests, coverage collection and verification, full no-skip harness, and terminal/final evidence before claiming mission acceptance. Prior gates and QA concern earlier source fingerprints and are not a pass for this revision. The blocked final decision was not rewritten here.


## Task-22: peer authentication and endpoint replacement

**Backend handoff.** Skill used: `oma-backend`. The two task-21 repair decisions are implemented and the assigned focused checks passed. Independent review, fresh full workspace/coverage/harness gates and final mission acceptance remain downstream. This append creates no new tracked evidence path and does not rewrite the blocked final, terminal, QA or gate artifacts.

### Admission and retained inputs

The supplied writable integrated checkout was `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/025db4e3-dd16-4743-ba66-93110c401133/workspace`; HEAD was `00773c69a8e5e7fd417fd5cc78d03e222364f609`. At 2026-11-09T23:33:01Z the recorded original deadline `2026-11-10T20:40:08.584Z` was approximately 76,028 seconds away. No cancellation notification was received. At 23:37:25Z the same deadline was 75,763.260 seconds away. Run and round budgets are framework-controlled; this Actor did not read or change mission state. Host: Darwin 27.2.0 arm64, `Eunkwangui-MacBookPro.local`, effective UID 501, 16 GiB RAM, eight CPUs, 38 GiB free disk at admission, descriptor limit 1,048,576.

Task-21's blocked report and its independently reviewed findings, task-20 QA, task-19 gate and the accepted contracts were read as preceding evidence. The task-21 audit recorded source fingerprint `ec4e258a4ad8f42f59fd67bd92e7733a907e7afadfc386ac5a3e90bc9bc9a663` over 235 inputs. Before product edits, `transport.rs` and `preparation.rs` matched its recorded hashes `fe0bb94493b0813000d9bbe795fa656d6ba11ebf4c9017de576aea0803d1cfdd` and `69265a5b6512bc162bbbe852aa267f408c94233eb9a713d8e1bf5dbce1797f7d`. The retained probe `target/exec-fail-open/task21-attempt3/lost-ack-probe.json` matched SHA-256 `ed40af9dbaf0c8330e4db68b91b9e8f143cd4c78ab3aa09e63f22206b2b66d33`, its script matched `e2061d503860883f0ce860329ae2c4ce593e7a55ff6a2d1e3ce03bb7604459bd`, and both probe-recorded candidate executables matched their files: CLI `c857ff1cc184f534d4a85e49a85ccde5d9eb071bca43dfdb2ee707b4d0272c87` and daemon `091a3d2d7164e8f57dbd59add01bed0c322856d98af433d925d0bda77338c9ac`. The original probe was not rerun or overwritten.

### Repair and interface

The transport distinguishes a connected endpoint's invalid-path errors from descriptor/setup failures. `ENOTDIR`, `ENOTSOCK`, `ELOOP`, `ENAMETOOLONG` and `EPROTOTYPE` remain non-launching when initially invalid, but after a possible send they retain `Uncertain`; a confirmed REGISTER followed by such an invalid CLAIM endpoint is also uncertain. Genuine local setup failures, argument errors, cancellation, complete invalid replies, protocol incompatibility and authenticated rejection still stop execution. Existing access-denied and unavailable endpoints retain fallback behavior. The original shared 1,800 ms pre-spawn deadline and single CLI-owned spawn are unchanged. The two pre-existing unpinned classification assertions were updated only to recognize the new internal endpoint type; their initial non-launching expectations remain.

Every connected stream is checked before request bytes are written or a response is trusted. macOS uses `getpeereid` and compares the peer UID with `geteuid`; Linux uses `SO_PEERCRED` with a credential-length check. A mismatch, credential-query error or unsupported result is treated as an unavailable endpoint before a possible send, and as uncertain after one or after confirmed registration. The injected credential outcome exists only in a private test boundary; there is no runtime bypass. Matching effective-user credentials implement the existing single-user boundary and do **not** identify the daemon binary. The existing daemon helper and same-user client admission remain unchanged.

The real same-user macOS credential query succeeded for both ends of a Unix socket pair; a bad descriptor caused a real query failure. Different UID, query-error and unsupported outcomes were injected internally, with a byte-level server observation verifying that untrusted connections received no request prefix and could not provide a definitive rejection. Real foreign-user socket behavior was not observed: a subprocess starting as UID 501 attempted `seteuid(65534)` and received `EPERM`; `target/exec-fail-open/task22/foreign-uid-capability.json` records that narrow observation. The OS-level unsupported-platform branch was not run on this macOS host.

### RED/GREEN observations

The fresh native RED command recorded in `target/exec-fail-open/task22/native-red.json` ran the three new real-daemon replacement tests on the pre-repair product sources: all three failed because the wrapper exited before the child invocation (exit 101, 3.915 seconds; receipt SHA-256 `bfff7b019952b60f0103936df48979f558a1146e743ef243240e10fea868f342`). For the authentication RED, the test-only intermediate source invoked the injected credential query but deliberately ignored its failure, reproducing the pre-repair trust decision: `auth-red2.json` records `Rejected` instead of `Unavailable` (exit 101, 1.658 seconds; receipt SHA-256 `e7f71c9fdeb636271cc88fe4c0569513a52449911385a6d95d70e921c631cbe9`). The delivered source enforces the result and the unfiltered CLI suite below passes. The additional endpoint RED in `endpoint-extra-red.json` observed `Local` instead of `Uncertain` after a send followed by `ELOOP` (exit 101, 1.697 seconds; receipt SHA-256 `cf3a9c14f8c91e7d7cd9411f517ac2502fd7da29ee9d81b6153d80ff44fdfd5c`); the final CLI run passes all five injected endpoint-error cases.

Three fixture issues were retained and corrected: `auth-red.json` initially failed to compile because the generic `read_message` arguments were incomplete; `auth-green.json` and `cli-first.json` exposed nonblocking/timeout handling in the mock server; `managed-final.json` exposed a nonblocking accepted child-control socket (`WouldBlock`) in the new native fixture. These were fixture failures, not passing evidence. The mock server now observes every accepted connection and counts even a partial request; the control connection is switched to blocking before its frame is read. Subsequent final-source runs passed.

The real-daemon proxy forwards requests to the daemon and waits for its successful response as a commit barrier. For REGISTER and CLAIM acknowledgement-loss cases it replaces the proxy endpoint before withholding that response; for the confirmed-REGISTER case it replaces the endpoint before forwarding the REGISTER response, making the next CLAIM connection encounter the replacement. Native connect checks confirm the file replacement produces `ENOTSOCK` and the parent replacement produces `ENOTDIR`. Each case compares the native fixture with `arcctl` for argv, cwd, environment, stdin/stdout/stderr and exit status, requires static `TRACKING_UNCERTAIN` without secrets, checks the recorded daemon state and absent child identity, then restarts the daemon and confirms one total invocation, zero permits and GC operations, and an unchanged cleanup sentinel.

The following observations are from `target/exec-fail-open/native/62483/` on the final candidate `target/debug/arcctl` SHA-256 `143b289456b44294714783b4d44e46c6a65cf814ac90c3c94a7962f300e860ae`. Elapsed time is the unadjusted parent-before-wrapper-spawn to child control connection; it includes startup and observation overhead. Internal metadata and startup subdivisions were not measured; Phase 0 has no separate scheduler stage. Each row recorded one invocation, exit 17 and one total invocation after recovery.

| Case | Commit state | Elapsed ms | Observation SHA-256 |
| --- | --- | ---: | --- |
| `register-file` | REGISTERED | 11.146 | `5ff01e3d751f1d997638ba33b293c4473628e18e64a0b26be780bb1d355de01c` |
| `register-parent` | REGISTERED | 10.888 | `a7c5d9d4575658bf5cd2488a23311992de524f64dbf22910992140a8315f24e1` |
| `claim-file` | LAUNCHING | 11.107 | `2326072d9e307ffd2f3127492e163df670e6cd8816c91420bb41f659f3ee9e4a` |
| `claim-parent` | LAUNCHING | 12.588 | `28af5bdbfcfb018e69392ddf1c6a84b84f9871381bc2b215e061719f56e45c4f` |
| `registered-file` | REGISTERED | 10.938 | `dc9e8664b2c9643f506d860a3c44eef3eee182b6ea547ea7eaf81b6ed415f272` |
| `registered-parent` | REGISTERED | 10.945 | `c81f7caa871d752ae2f23440983624b6b2f9cc5e62714f6e107ce531b53e702c` |

The complete managed run recorded 23 native outage/access/replacement parent-to-child timings; the maximum was **1,817.188 ms**, below the 2,000 ms target on this host. All timing observations are unadjusted and are not a hard real-time guarantee. The existing suite also covers native descriptors, signals, foreground pipelines, cancellation, invalid arguments and replies, authenticated rejection, successful managed tracking and idempotent recovery.

### Final source-bound checks

All commands below ran in the workspace above. The wrapper allowed 7,200,000 ms per command and recorded exact argv, cwd, host, executable path/hash, candidate hashes before and after, process and observer IDs, timestamps, elapsed time, exit, all 238 input hashes and raw stdout/stderr hashes. Every listed result had identical source fingerprints before and after: `d9b6f08aa8f3577037346e9bcd980b6fa8cd468862603c3b8e0a77e733a1a49e`. Raw output, records and full inventories are under `target/exec-fail-open/task22/`; each receipt is `<name>.json`. Tool versions and resolved binary hashes are in `environment.json` (SHA-256 `94c5e8f592957f6efb71146700500d69f085d3f4f65c1174a6557f078ebd79a6`): Cargo 1.95.0, rustc 1.95.0 aarch64-apple-darwin, Bun 1.4.4, Python 3.14.10.

| Receipt | Exact command | UTC start | Seconds | PID | Exit | Receipt SHA-256 |
| --- | --- | --- | ---: | ---: | ---: | --- |
| `cli-complete` | `cargo test -p arc-cli --tests` | 2026-11-09T23:47:01.313023+00:00 | 7.185 | 62279 | 0 | `5484b27acbe11082b596b79745a4c75a3e54c240c7b7784b50da4c0910ebf9e0` |
| `managed-complete` | `cargo test -p arc-daemon --test managed_exec -- --nocapture` | 2026-11-09T23:47:10.400105+00:00 | 74.322 | 62482 | 0 | `5c6ace280244d292b8a6200518b0099c9995758c8f63aa256ffa453fbea5cb5d` |
| `fmt-complete` | `cargo fmt --all -- --check` | 2026-11-09T23:48:26.870100+00:00 | 0.489 | 63698 | 0 | `708ffa20729b81c85dd1819bd57b6da45c3c53f9d46bb788f7b336e5feb25f38` |
| `clippy-complete` | `cargo clippy -p arc-cli -p arc-daemon --all-targets -- -D warnings` | 2026-11-09T23:48:29.169579+00:00 | 1.088 | 63705 | 0 | `befeacb2c10499e12e6b64428d8438caa5b32c2b1011dd5f4273c1776a447721` |
| `docs-complete` | `bun test scripts/harness/exec-fail-open-docs.test.ts` | 2026-11-09T23:48:33.984701+00:00 | 0.454 | 63718 | 0 | `915cd5244b1ef16b83d9dbd8ba6a575a53b5559eabb40d389e0033d14d3aadb7` |
| `recipe-complete` | `bun test scripts/harness/exec-fail-open-recipe.test.ts` | 2026-11-09T23:48:36.416307+00:00 | 1.715 | 63723 | 0 | `5772593f0f3598598ef166b703012354aef05b4bee44d9053d294894507a5214` |
| `preservation-complete` | `bun test scripts/harness/managed-exec.test.ts scripts/harness/coverage-verification.test.ts scripts/harness/equivalence.test.ts scripts/harness/exec-fail-open-preservation.test.ts` | 2026-11-09T23:48:41.214816+00:00 | 210.759 | 63817 | 0 | `b93e1cbff500938355d2cc0d60df2d2bbbcb2a917c8b28c453ba573c2c308c00` |

The CLI suite passed 52 tests and the managed suite passed 55, with no failures, ignored or filtered tests. Documentation and recipe inventory each passed two tests. The four preservation/accounting files passed 19 tests. The recipe test verifies all nine added stable regression names are required by the maintained workspace-test inventory.

Final owned source/test hashes:

| Path | SHA-256 |
| --- | --- |
| `crates/arc-cli/src/exec/transport.rs` | `72f3e59c782983fc4450cc5cdd4f1da459390ae998a9a8e8e4d94be44b4f92bc` |
| `crates/arc-cli/src/exec/preparation.rs` | `2c1cc75c91e449c43440afaf963085c62cddad00f546229a26bee4937fa5641a` |
| `crates/arc-cli/src/exec/transport/repair_tests.rs` | `cc5db3ed6ff107f79bff27328da52900388a8332e302393dac75dad9cfe88460` |
| `crates/arc-cli/src/exec/transport/peer.rs` | `634b4c5ca677add323a48633808767b4c25fd93d2a3c2ea644ea706b0c08cd58` |
| `crates/arc-cli/src/exec/transport/access_tests.rs` | `629dd4071692c3806bdad4bb6ac0181e8baa2950440588b32181ce525d76983c` |
| `crates/arc-daemon/tests/managed_exec.rs` | `627c020e21c635cd0c226b1b1548c5a9abdb0bf4ba00b01b3a8f46e2babd71ae` |
| `crates/arc-daemon/tests/managed_exec/replaced_endpoint.rs` | `fe525858e85a41289f1237be465df9d8271c8050483b2a8bbeaabf4489c2994d` |
| `scripts/harness/exec-fail-open-recipe.ts` | `d906b7991edc49594234113ee7c2af0f404b155d4770e6a156ae25cecfa3452c` |
| `scripts/harness/exec-fail-open-recipe.test.ts` | `49dd0ac0d2c92b01b9e70fee970f258c62e6c83189b53ed9d08e5ab00cc16842` |

The final/terminal/QA/gate artifacts retained their incoming SHA-256 values: `final.md` `2e94d13c3bb56ea8dadccab7a01c4a05d3fdc0fcdc3c3e06ca14ef40222887b4`, `final.json` `69fe31d1c267fb1f091cb121dda8d1338695ffe921d283d8cba176fdaabc4d7f`, `terminal.json` `96264e89fe89ae0c9ade101a985d1e1ae0050f6106a2d3552acfa2eaaabc3e75`, QA JSON `74584b6628cb308750d48d202b2414389f7e99ea8d40a211462dd3ffa1eba996`, QA report `74b799f9bd892262d06d32eaff6e31247077a4b0507b02ba7b35883162a6f75b`, and gate `094b5ff84e9d11b19878d7b00a1d4ef8334fc46dfb687d7b77cec5bdad0f7fb7`. Contract JSON/Markdown retained their required hashes. The indexed evidence count remains 149. Migration `0001_phase0.sql` matched HEAD, index and worktree at `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. The required preservation suite passed its historical assertions, pinned helpers, coverage accounting, and source-length protections; owned Rust files are at most 362 lines. No manifests, lockfiles, migrations, coverage collectors, pinned tests or historical routes were changed by this task.

The preservation tests rewrote three pre-existing `.arc-verify/harness-speed/` outputs during execution. Their generated bytes are retained under `target/exec-fail-open/task22/preservation-generated-complete/`; all three incoming bytes were restored from the pre-run copies and verified by SHA-256. The older failed command observations remain under `target/exec-fail-open/task22/`.

Independent task review must inspect these delivered bytes and reproduce the endpoint and peer decisions. The Chief must then collect fresh source-bound workspace, coverage and full no-skip harness gates, rebind the final and terminal evidence, and obtain independent final QA. Earlier gate and QA receipts bind prior source and are not claimed for this revision. Desktop, native Windows, vendor hooks and scheduling remain outstanding.


## Task-22, attempt 2: malformed endpoint classification

**Backend handoff.** Skill used: `oma-backend`. The repair finding is resolved for the address-validation path. This append does not replace the earlier task-22 evidence or alter the blocked final, terminal, QA, gate or contract artifacts. Independent task review and final workspace, coverage and harness gates remain downstream.

### Admission and retained observations

The supplied integrated workspace was `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/025db4e3-dd16-4743-ba66-93110c401133/workspace` at HEAD `00773c69a8e5e7fd417fd5cc78d03e222364f609`. No cancellation notification was received. At 2026-11-09T23:59:15Z the recorded original deadline of 2026-11-10T20:40:08.584Z was 74,452.817 seconds away. The host is Darwin 27.2.0 arm64, `Eunkwangui-MacBookPro.local`, effective UID 501, 16 GiB RAM, 8 CPUs, a descriptor limit of 1,048,576 and approximately 37 GiB free at admission. Run and round limits are framework-controlled. Serena code-search tools were unavailable, so known assigned paths were read directly; no GUI was used.

The task-21 lost-ack observation remains SHA-256 `ed40af9dbaf0c8330e4db68b91b9e8f143cd4c78ab3aa09e63f22206b2b66d33`, and its script remains `e2061d503860883f0ce860329ae2c4ce593e7a55ff6a2d1e3ce03bb7604459bd`. It was not rerun in place. Its recorded earlier candidate has since been replaced by the subsequent task-22 candidate; the prior task-22 section records its verification before that replacement. The earlier task-22 RED/GREEN observations and its review remain historical evidence, not observations made again by this attempt.

### Change and compatibility

`DeadlineStream::connect_with_auth` now classifies an overlong Unix-socket address or an embedded NUL as `ConnectFailure::InvalidEndpoint`, before descriptor creation. `call_until` and the real `start_until`/`prepare_with` path expose `Failure::InvalidEndpoint` for an initially malformed address, rather than the descriptor or supervision failure `Failure::Local`. Both remain non-launching. Existing endpoint-change uncertainty, cancellation, peer authentication, shared deadline and the single spawn path are untouched.

The new focused regression calls the real address-validation, transport and preparation paths for both malformed forms and verifies that authentication is not attempted. Its initial RED failed at the actual address check before the change; the final GREEN passed. The new native macOS CLI regression checks that an overlong configured socket exits 1, produces `NOT_STARTED`, writes no child output and creates no invocation marker. A NUL cannot be passed through a Unix process environment, so that form is covered at the internal address-validation boundary.

The pinned `transport/tests.rs` bytes remain `83757a11e06321cd40be4f877a7c6ba8fba0543612317db4037cff97a73940f3`. Its original pre-split assertion still executes unchanged through `transport/historical_tests.rs`, a test-only compatibility adapter mapping the new invalid-endpoint type to the old local type for that historical test module. Production callers and the new regression use the unmapped type. The adapter uses normal external module declarations, so coverage accounting recognizes both files as test-only. The two new regression identities were added to the existing workspace inventory and its inventory test; no preservation pin, manifest, lockfile or collector was changed.

### RED/GREEN and command observations

All commands ran from the integrated workspace. The recorder allowed 7,200,000 ms per check and retained exact argv, cwd, start/end time, elapsed time, PID, executable identity, before/after source inventories and output hashes in `target/exec-fail-open/task22/<receipt>.json`; raw stdout and stderr are alongside each record. All final GREEN rows below bind the same 240-input fingerprint `63138a023a2dc562e3670c66cbca9565a7727bc883333a644eee25e4eba50bcf` before and after. The source-bound RED fingerprint is `a2c2df01f1038724043e33f9a5a52d78064faa840d4afd2fad839571a2b6be73`, with pre-change `transport.rs` SHA-256 `72f3e59c782983fc4450cc5cdd4f1da459390ae998a9a8e8e4d94be44b4f92bc`. The suite counts were CLI 53 passed, managed execution 56 passed, docs 2 passed, inventory 2 passed, and preservation 19 passed, with no failed, ignored or filtered tests in the unfiltered suites.

| Receipt | Exact command | UTC start | Seconds | PID | Exit | Receipt SHA-256 |
| --- | --- | --- | ---: | ---: | ---: | --- |
| `attempt2-address-red` | `cargo test -p arc-cli --bin arcctl malformed_addresses_are_invalid_endpoints_in_preparation -- --nocapture` | 2026-11-09T23:58:30.100769+00:00 | 2.447 | 92482 | 101 | `1529a0ec63be644def1d691173b654fc28dc8a7cd13406acb005641e7e544b80` |
| `attempt2-address-green2` | `cargo test -p arc-cli --bin arcctl malformed_addresses_are_invalid_endpoints_in_preparation -- --nocapture` | 2026-11-10T00:03:47.713566+00:00 | 1.904 | 16163 | 0 | `f3b0571c52bc6e950385fc6b605db7da6d39b5ef40708d734d40c50046f8c880` |
| `attempt2-cli2` | `cargo test -p arc-cli --tests` | 2026-11-10T00:03:54.271488+00:00 | 6.540 | 16203 | 0 | `221dec4660012940bc475c6b081abe4e9913c2e5009911a28446e222fd142af6` |
| `attempt2-managed2` | `cargo test -p arc-daemon --test managed_exec -- --nocapture` | 2026-11-10T00:04:04.428444+00:00 | 74.083 | 16399 | 0 | `695e7dfe83f67ec58cfc5db5d9c757520ac6486b78eb680ecb7ab9cc7a6379f2` |
| `attempt2-fmt2` | `cargo fmt --all -- --check` | 2026-11-10T00:05:26.584956+00:00 | 0.513 | 18045 | 0 | `60819e3f4f392c0987fcb64cbb402c992f3302e26484eddb398580c8ccccd25a` |
| `attempt2-clippy2` | `cargo clippy -p arc-cli -p arc-daemon --all-targets -- -D warnings` | 2026-11-10T00:05:27.350507+00:00 | 1.046 | 18053 | 0 | `853731e67f820ff00cf424fa48a6b52418651b4fbc2244ef0c76bffcc9f49c7f` |
| `attempt2-docs2` | `bun test scripts/harness/exec-fail-open-docs.test.ts` | 2026-11-10T00:05:28.652719+00:00 | 1.139 | 18066 | 0 | `f58969c445a0469376277df503564c23e425ad2c02d06027bfa7ee791684321d` |
| `attempt2-recipe2` | `bun test scripts/harness/exec-fail-open-recipe.test.ts` | 2026-11-10T00:05:30.105390+00:00 | 1.770 | 18071 | 0 | `b72c7980b5f2eefa02b086d70507dc0aaca4475f6b6434a0989d730460641c8c` |
| `attempt2-preservation` | `bun test scripts/harness/managed-exec.test.ts scripts/harness/coverage-verification.test.ts scripts/harness/equivalence.test.ts scripts/harness/exec-fail-open-preservation.test.ts` | 2026-11-10T00:01:03.239600+00:00 | 137.872 | 94499 | 1 | `049ceef3410cbe4205af8db9cbcfc0e839bd5aa610d9f6bc8212e7651a2a7891` |
| `attempt2-preservation2` | `bun test scripts/harness/managed-exec.test.ts scripts/harness/coverage-verification.test.ts scripts/harness/equivalence.test.ts scripts/harness/exec-fail-open-preservation.test.ts` | 2026-11-10T00:05:34.050072+00:00 | 207.547 | 18148 | 0 | `419908e0e414680c6dedb4a5ee3d858ed49dc6289feca98dab94468c7805e057` |

The first preservation run (`attempt2-preservation`) failed with 13 passes and 6 failures because an initial `include!` adapter left the pinned file unclassified by `scripts/coverage/source.ts`. The correction uses a test-only file with `#[path = "tests.rs"] mod tests;` and preserves the original file; the subsequent unchanged four-file suite passed. Its failed stdout/stderr and receipt remain under `target/`. An earlier GREEN against the intermediate adapter and its other observations also remain there and are not final-source evidence.

The Cargo/rustc launcher resolved to `/Users/gracefullight/.cargo/bin/rustup` (SHA-256 `ec1b9233e7f72990ecd8e62063fa7f6c3dfc2bec8e97f88bff165f9100ac696a`); the actual Cargo 1.95.0 binary SHA-256 was `c512bff73c86143b557463f021d0c3d5b0490d97d65040ba59ea2b3427784758`, and rustc 1.95.0 `aarch64-apple-darwin` was `b829b733131d4e1673eeebd1f34d06ae1e9ff4977b051313cf42e2a9e79ecf1c`. Bun 1.4.4 SHA-256 was `0a2237cdcfa0d16ef501d9acca9f0026309ef348bea7393a64e8d1e3609565cc`; Python 3.14.10 SHA-256 was `26b5194d2beb09e648b4676ddc18a1384f182a7ed84f3db73106096aba6febf4`. Individual receipts identify the actual launched paths and hashes.

### Native observations

The final managed suite used candidate `target/debug/arcctl` SHA-256 `72bd851734eade26fa15be0d6c73fcfbbf0910292e49cae617bac57feefdad77`. The real daemon acknowledged its committed transaction to the proxy, which then withheld or forwarded the acknowledgement and replaced the endpoint as described in the prior task-22 section. For each row below the fixture verified one invocation, exit 17, uncertain diagnostics, no secret leakage, native I/O equivalence, no reservations or cleanup, and one total invocation after daemon recovery. Raw observations are under `target/exec-fail-open/native/16400/`.

| Case | Committed phase | Parent spawn to child handshake, ms | Observation SHA-256 |
| --- | --- | ---: | --- |
| `register-file` | REGISTERED | 11.033 | `7c7224c387cf503d2f36850789598f73cf1aba9932355d69026e8cb386822a8f` |
| `register-parent` | REGISTERED | 12.151 | `3060daf54275b8d0976640bc24ae807ea380ddf0515528e1c596e9cda9cd3cc9` |
| `claim-file` | LAUNCHING | 12.607 | `c9310a1c01c46ae33c56a23be3843a218c132ef21d37329631ce899fb6314461` |
| `claim-parent` | LAUNCHING | 12.061 | `0e4817fcdb1b8614f6b5ff04c7e422dbae9ada381894b5e1b3574036b5b67ae0` |
| `registered-file` | REGISTERED | 10.780 | `e9c06f82c4b41d9854b2b90ba7c23187b73a651651e80cf7d967426b7a47687e` |
| `registered-parent` | REGISTERED | 10.985 | `7c04e616a6322f68a87b8d141e8a24f45cf14634f171d881fab0f274243aae75` |

All 23 recorded outage, access and replacement timings had one invocation, exit 17 and one total invocation after recovery. The maximum was **1822.355 ms** (`partial-register.json`). The elapsed boundary is the unadjusted parent-before-wrapper-spawn to child handshake and includes startup, OS scheduling and observation overhead; metadata and startup subdivisions were not measured. Phase 0 has no separate scheduler stage. The two-second target passed in these observations, not as a hard real-time guarantee.

The final CLI suite also executed the credential injection, real same-user macOS socket-pair credential query, query-failure, cancellation, explicit rejection, invalid-reply and local-setup regressions. Same-user credentials enforce the existing boundary but do not identify a daemon binary. A real foreign-user socket was not observed: the earlier UID 501 to UID 65534 capability probe returned EPERM (`target/exec-fail-open/task22/foreign-uid-capability.json`); the internal injected credential result, not a production bypass, covers that case. Unsupported-platform behavior was injected, not observed on another OS.

### Final source and preservation

| Path | SHA-256 |
| --- | --- |
| `crates/arc-cli/src/exec/transport.rs` | `fd7ad2f25a350043595c5c7134a9395b4714ac184648c8a996f32422cdfe77c0` |
| `crates/arc-cli/src/exec/transport/historical_tests.rs` | `0b408133e7e61962cb98ddc9e58cfc7b20d69c3635fa34ba0382101d0698cf31` |
| `crates/arc-cli/src/exec/transport/tests.rs` | `83757a11e06321cd40be4f877a7c6ba8fba0543612317db4037cff97a73940f3` |
| `crates/arc-cli/src/exec/transport/repair_tests.rs` | `1b32e96cd0fbfe81cbdd6cfde235f7b8ef1415e28ed4eb1cde99f485d702bffd` |
| `crates/arc-cli/src/exec/preparation.rs` | `2c1cc75c91e449c43440afaf963085c62cddad00f546229a26bee4937fa5641a` |
| `crates/arc-cli/src/exec/transport/peer.rs` | `634b4c5ca677add323a48633808767b4c25fd93d2a3c2ea644ea706b0c08cd58` |
| `crates/arc-daemon/tests/managed_exec.rs` | `5e1540de9c1835d54e6e5266e1706fce6b916bc4d108390a3ac48f584fa31e93` |
| `crates/arc-daemon/tests/managed_exec/malformed_endpoint.rs` | `df81e1f8e7fa96c9fceb3b05cf33382fa5a9b5a226feb3dba621656d50b2f174` |
| `crates/arc-daemon/tests/managed_exec/replaced_endpoint.rs` | `fe525858e85a41289f1237be465df9d8271c8050483b2a8bbeaabf4489c2994d` |
| `scripts/harness/exec-fail-open-recipe.ts` | `6cc05f7a5d039884f58a7ed9f5d78eb100b6e0b5de30f9b8c7a3ddae55ee8c40` |
| `scripts/harness/exec-fail-open-recipe.test.ts` | `15cecfaade966edd28b81101032d164b336ba8cecda8a20d388a171e4116695f` |

`0001_phase0.sql` remains SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384` in HEAD, index and worktree. The indexed `.arc-verify/exec-fail-open/` inventory remains 149 files. Source lengths for the changed Rust/TypeScript files are below 500 lines. The final/terminal/QA/gate hashes remain the values recorded in the preceding task-22 handoff. The historical route log was copied after each managed suite and restored to its incoming SHA-256 `e373197f2725098357cd99f72b35a932aad234bfc73a689fa611e65562f2c472`. The preservation tests' three generated `harness-speed` observations were copied under `target/exec-fail-open/task22/attempt2-preservation-generated*/` and restored to their incoming worktree and index bytes. No new tracked evidence path was added.

The Chief must arrange independent task review on these bytes, then refresh the full source-bound workspace, coverage, no-skip harness and final/terminal evidence. The earlier QA and gate receipts bind earlier source and do not establish a pass for this revision. Desktop, native Windows, vendor hooks and scheduling remain outstanding. No final coverage or mission acceptance is claimed.


## Task 22 attempt 3: fixed historical transport routing
### Scope and implementation
This attempt addresses the task-22 review finding about `historical_tests.rs`: the earlier adapter converted a live `InvalidEndpoint` result into `Local` before the pinned assertion saw it. The adapter has been removed. `historical_tests.rs` now contains the transport implementation from Git snapshot `1b2c426cad8e9935d666f0bd7c18042c678be315` (tree `4310f82244e45ecb49e03c894497034b1a452061`), with only its module location, request-ID path, and an entry integrity check adapted for its test-only location. `historical_integrity.rs` reverses those exact changes, hashes the reconstructed Git blob as `27ac37b19d515804df5cd8dc253023697b60ed0b`, and checks the original pinned test blob `41902733a5c1b741ac2097dcccb1ad7fd439e9ca` before every historical call, once per process. The original call result reaches the unchanged assertion directly; no error is translated. An independent byte comparison against the Git snapshot reconstructed the exact historical source, SHA-256 `fd812936d4f576c8eb7e31c6243d95582e38adfbf05c1e90b0052c2409125c3a`; the pinned test SHA-256 is `83757a11e06321cd40be4f877a7c6ba8fba0543612317db4037cff97a73940f3`. The integrity test also rejects a changed historical `Local` branch.
The five other original transport behaviors have separate live tests in `live_tests.rs`, each with a `live_` prefix and a distinct socket-name prefix. The live malformed-address test remains `malformed_addresses_are_invalid_endpoints_in_preparation`; it observes the production `InvalidEndpoint` result. The maintained recipe now requires the fixed-fixture check and all five distinct live tests in addition to the existing inventory. The recipe test verifies that omitting each new name is rejected. No pinned test or preservation pin was edited. The production execution path, socket classification, signal ownership, peer check and single spawn site were not changed in this attempt.
### Prior lost-ack RED and current GREEN
The retained task-21 lost-ack probe was read, not rerun: `target/exec-fail-open/task21-attempt3/lost-ack-probe.json` SHA-256 `ed40af9dbaf0c8330e4db68b91b9e8f143cd4c78ab3aa09e63f22206b2b66d33`. It records a real committed REGISTER and CLAIM, each with its acknowledgement withheld after endpoint replacement, exit 1 and no invocation. Its recorded candidate SHA-256 `c857ff1cc184f534d4a85e49a85ccde5d9eb071bca43dfdb2ee707b4d0272c87` matches the candidate recorded in the earlier task-22 native RED receipt `target/exec-fail-open/task22/native-red.json`; that receipt and both raw output hashes were checked. The RED transport source hash `fe0bb94493b0813000d9bbe795fa656d6ba11ebf4c9017de576aea0803d1cfdd` matches snapshot `7254e598c0c5830e59998dd27188a9a097a7288a`. The original RED candidate has since been rebuilt, so its current bytes were not independently re-hashed here; the prior receipt records its identity. The prior RED receipt records other source hashes, but this attempt did not reconstruct every one from a saved source tree.
The fresh historical-routing RED (`attempt3-history-red`) exited 101: the adapter reconstructed to Git blob `a47eb74e15c71209bc71264f5621b6a87142dd9f`, not the pinned historical blob. An intermediate GREEN (`attempt3-history-green`) passed. The final-source CLI suite also passed the integrity test, the unchanged historical `invalid_socket_path_is_local_failure` assertion, the five historical behaviors, all five corresponding live tests, and the live invalid-endpoint test. The first combined CLI run (`attempt3-cli`) failed because historical and live fixtures used the same per-process socket names; the live fixture prefix was corrected and `attempt3-cli-fixed` passed. The first format check (`attempt3-fmt-pre`) found formatting differences in the new integrity file; that file was formatted and the final check passed. These failed observations were retained, not overwritten.
### Command receipts
Commands ran in `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/025db4e3-dd16-4743-ba66-93110c401133/workspace` on `Darwin 27.2.0 arm64`. The wrapper is `python3 target/exec-fail-open/task22/run.py <label> <command...>`; it records the actual subprocess, PID, UTC start/end, source-input inventory, candidate hashes, outputs and a 7,200-second timeout allowance in `target/exec-fail-open/task22/<label>.json`. The final source fingerprint before and after every final check is `bd81c863144d1dd2b149122fa47299c60f503064eda68262fa1d464c08212fdb` (242 inputs; input inventory SHA-256 `c8e2dbc121cee00fdd968ac41bece03d9e048c4e91e5ee32819c9567ea514d84`). The launcher for Cargo resolves to rustup SHA-256 `ec1b9233e7f72990ecd8e62063fa7f6c3dfc2bec8e97f88bff165f9100ac696a`; the actual Cargo 1.95.0 and rustc 1.95.0 hashes are respectively `c512bff73c86143b557463f021d0c3d5b0490d97d65040ba59ea2b3427784758` and `b829b733131d4e1673eeebd1f34d06ae1e9ff4977b051313cf42e2a9e79ecf1c`. Bun 1.4.4 SHA-256 is `0a2237cdcfa0d16ef501d9acca9f0026309ef348bea7393a64e8d1e3609565cc`.
| Label | Command | Started UTC | Seconds | Exit | stdout SHA-256 | stderr SHA-256 |
| --- | --- | --- | ---: | ---: | --- | --- |
| `attempt3-history-red` | `cargo test -p arc-cli --bin arcctl fixed_historical_transport_and_pinned_tests_match_baseline_hashes -- --nocapture` | 2026-11-10T00:14:45.976724+00:00 | 2.233 | 101 | `2f9e55dff213329ba1e102cf5e890cafee97077ef90d85e3a31dc9e635db3506` | `62f5efe158d2714f9d27ab580d688547caaf7e8d167519a8961b3c3d73ce9e2b` |
| `attempt3-history-green` | `cargo test -p arc-cli --bin arcctl fixed_historical_transport_and_pinned_tests_match_baseline_hashes -- --nocapture` | 2026-11-10T00:15:06.858765+00:00 | 1.598 | 0 | `ec2b261cd5596124923c38ab12a9e40a5e469094329cbdf7e865a2f329c7d685` | `3a90d3b0dbd53cebd60d5512fcba0a439f0ee1b5d672f4695193e87903369039` |
| `attempt3-fmt-pre` | `cargo fmt --all -- --check` | 2026-11-10T00:15:12.098917+00:00 | 0.393 | 1 | `b3aa61fb528771b93006915f2f1b25ec9cf992209d3ad3e5d7eba25ed43ac5ea` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `attempt3-cli` | `cargo test -p arc-cli --tests` | 2026-11-10T00:15:29.925793+00:00 | 1.148 | 101 | `f44a02c7209cecf35bf00f899ac58b32d53ceefaf0780729bd78381ca0b08014` | `b1db55ab03752cfe7480acd169a4c09463cd192ce64ab48afad23a0f56012681` |
| `attempt3-cli-fixed` | `cargo test -p arc-cli --tests` | 2026-11-10T00:15:37.308227+00:00 | 7.061 | 0 | `bc7e5311a78fe219ddf8ed642d55c6ea36957e60593a9de17003db9e87b4a828` | `7dbe9fcbba08c9ad7aa048b6a0dbea607d848b248ac75f209d549bee018330cb` |
| `attempt3-fmt` | `cargo fmt --all -- --check` | 2026-11-10T00:15:51.276245+00:00 | 0.390 | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `attempt3-clippy` | `cargo clippy -p arc-cli -p arc-daemon --all-targets -- -D warnings` | 2026-11-10T00:15:51.919236+00:00 | 0.827 | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `296abd4e963a65c8ea5dd08d5ea79ebb04505984317afe1081055db2e133b832` |
| `attempt3-docs` | `bun test scripts/harness/exec-fail-open-docs.test.ts` | 2026-11-10T00:15:53.009703+00:00 | 0.865 | 0 | `bbe6a94c29cc8d3ff5a1792a5abae1f1122aed0642111b52036ab7e0b2b2ca47` | `f53666e111581cc720a17c67467695ea9f81090e3400edd34eb9ee7aee5ce785` |
| `attempt3-recipe` | `bun test scripts/harness/exec-fail-open-recipe.test.ts` | 2026-11-10T00:15:54.177057+00:00 | 2.340 | 0 | `bbe6a94c29cc8d3ff5a1792a5abae1f1122aed0642111b52036ab7e0b2b2ca47` | `581a72dbb94352bfff0cc72c839e6ea19271851acae812b95acc586454803051` |
| `attempt3-managed` | `cargo test -p arc-daemon --test managed_exec -- --nocapture` | 2026-11-10T00:16:01.666861+00:00 | 74.914 | 0 | `4d7ec05936c1c30e65e7936ff377d13252174c21d6d3b5788f79d68e0cf0626d` | `cd916d50191b902e3bdafad10f6345befb277d0b7dd738a614a36be5957965f6` |
| `attempt3-preservation` | `bun test scripts/harness/managed-exec.test.ts scripts/harness/coverage-verification.test.ts scripts/harness/equivalence.test.ts scripts/harness/exec-fail-open-preservation.test.ts` | 2026-11-10T00:17:23.377899+00:00 | 208.883 | 0 | `674f98d91dfbc02484a3456ac46f6ea55a4419d8ba436ae2124053270b8468f3` | `ece0b0a9961562ae46af53a851f609d3cdc52a89e4a240df9c6d109fc5c7c225` |

The final CLI command passed 59 tests across its harnesses, with zero failures, ignored or filtered tests. The native managed-execution command passed 56 tests, with zero failures, ignored or filtered tests. The documentation and recipe checks passed 2 and 3 tests. The four preservation/accounting files passed 19 tests. The `cargo fmt` and affected warning-denying Clippy commands passed. The hashes above are of raw outputs, not a claim that the full workspace, live coverage or final harness gates ran in this attempt.
### Native timing observations
The final native suite used `target/debug/arcctl` SHA-256 `80410fb9bf679162e27a410d723dc6aecfd37d063e5fe6340685b9784eaa9f1b`. The six records below are in `target/exec-fail-open/native/46822/`. Each asserts a real daemon commit before the proxy releases or withholds the acknowledgement, one invocation, exit 17, `TRACKING_UNCERTAIN`, native stdout/stderr comparison, zero permits and cleanup, and one total invocation after daemon recovery. The unadjusted measurement runs from before spawning the wrapper to the child control handshake; it includes process startup, OS scheduling and observation overhead. Internal subdivisions were not measured and Phase 0 has no separate scheduling stage.
| Case | Committed state | Handshake ms | Observation SHA-256 |
| --- | --- | ---: | --- |
| `register-file` | REGISTERED | 10.734 | `7cfd54fa4b85105acd6868f6eaf108a2a5ee1e2a362ad093ca399b5e66148aeb` |
| `register-parent` | REGISTERED | 10.329 | `9a6409bb60bdec1817ef5a1e333afca4ac73f32c53d143e95eb6fbbc903c9814` |
| `claim-file` | LAUNCHING | 14.687 | `66266ccf4e0e5b20f029ed86010d3b54e6aa3737fd52521bb68963d38b2c476a` |
| `claim-parent` | LAUNCHING | 11.901 | `cf5fe49c0e7745564c154c05228fb0f6a086198db3613368b1ab3688538e1a61` |
| `registered-file` | REGISTERED | 11.977 | `f43d1325563d93736f5178e1dc2cd21bb4a2409eac8b1ec6c6cecd1251773a86` |
| `registered-parent` | REGISTERED | 11.009 | `8fc896c2ee3c0905ef32488cd63bec77fc987fb039a88968852fcef891c65108` |

Across all 23 recorded outage, access-denial and replacement observations with one invocation and one after recovery, all used the same candidate, reported exit 17 and passed; the maximum unadjusted handshake was **1820.069 ms** (`stall-register.json`). These observations satisfy the two-second target in this run, not a hard real-time guarantee.
### Preservation and handoff
| Path | SHA-256 |
| --- | --- |
| `crates/arc-cli/src/exec/transport.rs` | `af5f348e91e39623d954421cdbd44809091b57b013183b8e36a4bbccd3e84e45` |
| `crates/arc-cli/src/exec/transport/historical_tests.rs` | `4c7ee61bc391b4e941fce2c21c7ca0476414c0ddd0057e6f83285aa721007e8d` |
| `crates/arc-cli/src/exec/transport/historical_integrity.rs` | `5d225ca7a84439d93e7f44fc03c93b071e1c87ab89ed1e40554fc565aa7ef330` |
| `crates/arc-cli/src/exec/transport/live_tests.rs` | `6763b38c220ba3a4b256c235abe505cd1c960b136711368d0a22ef555a2406ef` |
| `crates/arc-cli/src/exec/transport/tests.rs` | `83757a11e06321cd40be4f877a7c6ba8fba0543612317db4037cff97a73940f3` |
| `scripts/harness/exec-fail-open-recipe.ts` | `d39ae3a938c5efdb65e9f2886a1b2edf60e796ac4a88e8ceb95051435e88e15e` |
| `scripts/harness/exec-fail-open-recipe.test.ts` | `834be9e1b667897c30bf465e2fd22c6a0c5f37458d068a5f40c21f82e41a78af` |

The coverage inventory classified the added historical, integrity and live files as test-only. All changed source files are below 500 lines. `0001_phase0.sql` is SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384` in HEAD, index and worktree. The indexed `.arc-verify/exec-fail-open/` count remains 149 files. The native suite appended two historical-route observations; the appended file was retained as `target/exec-fail-open/task22/attempt3-routes-observed.jsonl`, and the previous route log was restored to SHA-256 `e373197f2725098357cd99f72b35a932aad234bfc73a689fa611e65562f2c472`. The preservation tests generated three `.arc-verify/harness-speed/` files; their generated bytes were retained under `target/exec-fail-open/task22/attempt3-preservation-generated/` and the incoming bytes restored. Two temporary staged versions were then replaced with the restored bytes using `git add` on those exact files.
The CLI suite checked real same-user macOS `getpeereid` credentials, injected foreign-UID/query-failure/unsupported results without a production bypass, and verified that untrusted peers receive no request. The earlier real foreign-user capability attempt returned EPERM when changing effective UID from 501 to 65534 (`target/exec-fail-open/task22/foreign-uid-capability.json`); a real foreign-user socket was not observed. Same-user credentials do not identify a daemon binary.
No cancellation request was observed. The runtime exposed no cancellation flag or absolute mission deadline in the environment, so this Actor could not independently calculate remaining mission time; each receipt wrapper allowed 7,200 seconds as required. No external delivery or AV invocation occurred. The final, terminal, QA and existing gate evidence remain historical and must be rebound by their assigned owners after independent task review and final collection. Full workspace, coverage and no-skip harness gates were not run in this attempt. Desktop, native Windows, vendor hooks and scheduling remain outstanding.

The incoming and final SHA-256 values for the preserved evidence match: `final.md` `2e94d13c3bb56ea8dadccab7a01c4a05d3fdc0fcdc3c3e06ca14ef40222887b4`; `final.json` `69fe31d1c267fb1f091cb121dda8d1338695ffe921d283d8cba176fdaabc4d7f`; `terminal.json` `96264e89fe89ae0c9ade101a985d1e1ae0050f6106a2d3552acfa2eaaabc3e75`; `qa/review.md` `74b799f9bd892262d06d32eaff6e31247077a4b0507b02ba7b35883162a6f75b`; `qa/review.json` `74584b6628cb308750d48d202b2414389f7e99ea8d40a211462dd3ffa1eba996`; and `implementation/gates.json` `094b5ff84e9d11b19878d7b00a1d4ef8334fc46dfb687d7b77cec5bdad0f7fb7`. The three restored harness-speed files have no remaining index or worktree changes. Final `currentInputs` remained `bd81c863144d1dd2b149122fa47299c60f503064eda68262fa1d464c08212fdb` over 242 inputs.
