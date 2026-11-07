# Final assessment of the command-classification slice

Task-6 assessment: completed. The repaired implementation and independent QA
support the assigned Phase 0 slice on baseline
`05f0812d0841485bbe07b3f1f8f6fde3861a1d55`. No implementation blocker remains
in the inspected evidence. Independent final framework review and the immutable
final executable verification still have to run. This assessment does not claim
full ARC acceptance. The unchanged harness exits **1** because `test:coverage`
is missing; coverage remains unresolved.

Only `final-report.md` and `final-summary.json` are owned and changed by this
task. Existing implementation, specifications, evidence and mission state are
preserved. Skills used: **oma-pm** and **oma-coordination**. The saved plan and
completed handoffs were reused; no new plan, Actor dispatch or AV delegation was
needed. The original goal, goalBrief, success criteria, assignments, Chief route,
verification command/contract and spent budgets were not changed.

## Delivered behavior and interface compatibility

The retained `arc-detector` is a pure, bounded Rust classifier. Protocol-1
`job.classify` accepts a required command string and optional `shell_kind`.
It returns exactly typed `classification`, `locality`, `reason` and `uncertainty`
fields. Cargo build/test/clippy/fmt have distinct reason codes; build is BUILD,
test TEST, and clippy/fmt LINT. The three supported gh forms classify REMOTE_CI;
supported SSH remote commands classify REMOTE_EXEC. Recognized local/remote
stages aggregate to MIXED. Quoted command-like arguments remain data. Unresolved
scripts, active expansions, Docker endpoints and unsupported syntax produce
UNKNOWN/OBSERVE, without running or probing anything.

Resolution is limited to the documented syntax subset. NONE does not certify
physical topology, runtime behavior or permission to execute. Unknown results are
successful observations, not admission decisions. Exact limits remain 16384 UTF-8
bytes, 1024 tokens, 128 invocations, nesting depth 8 and 65536 charged work units,
shared across nested analysis and checked before charged operations.

The shortest supported CLI path is one literal input and one call:

```text
arcctl classify [--shell <shell_kind>] [--json] -- <command>
arcctl classify -- 'cargo test && gh workflow run ci.yml'
```

Human output has four labeled enum lines followed by `Command not executed.`;
JSON has exactly four fields and one newline. Successful uncertain observations
exit 0 with empty stderr. The caller must preserve the literal string before it
reaches ARC; unquoted shell substitution happens in the caller's shell. There is
no wizard, argv joining, retry loop, scheduling or execution step.

Protocol 1, `dispatch(request, doctor)`, version-first rejection, exact status and
doctor behavior (including classification `not_checked`) remain. Native same-user
authentication, 64 connections, 16 admitted requests, queue eight, existing
deadlines, health handling and 3500 ms shutdown remain unchanged. Classification
is synchronous inside existing admission, clears raw payload/body storage before
response await, and never enters DbWorker. Review inspected the pure dependency
boundary plus executable traps, substitution sentinels, TCP/Unix monitors and
SQLite/WAL/SHM/log/report canary scans. These support the no-execution,
no-endpoint-contact and no-raw-command/secret-persistence claim within the tested
boundary. No live foreign-account impersonation or native Windows test is claimed.

## SSH-POST-DESTINATION: repaired by the implementation owner

Repeated reviewer attempts retained a real HIGH defect: `ssh build01 -N` and
related post-destination flags were incorrectly treated as certain remote work.
Passing Cargo tests alone had not established correctness. Reviewers were excluded
from product/test edits and could not repair those files. Recovery returned the
problem to the backend implementation owner, then returned the repaired inputs to
independent oma-qa. This preserves ownership rather than turning repeated review
into a repair strategy.

The narrow parser change in `crates/arc-detector/src/rules.rs` checks the boundary
after a validated destination. Charged checks skip initial empty words, recognize
an explicit `--`, and return SSH_AMBIGUOUS/OBSERVE for unresolved leading options
including -N/-G/-L/-R/-D/-W, quoted forms and compounds. A terminator without a
nonempty command stays uncertain. Once a real command starts, later arguments
remain opaque: `ssh build01 make -N` retains remote execution classification.
Pre-destination forwarding, option values, destinations and loopback controls
remain covered. No new process, resolver, filesystem, environment or storage
operation was introduced.

[Implementation repair handoff](implementation/repair-7-report.md) records real
RED assertion failures, GREEN outcomes and 99 permanent cases spanning detector,
typed IPC, daemon and actual human/JSON CLI. Independent
[SSH evidence](verification/ssh-repair.json) reuses all 17 original synthetic
probe inputs and expectations and adds 15 controls. I checked all original fields
are retained, all 32 four-field results match IPC and both CLI modes, and all 64
CLI calls exit 0 with empty stderr and exact output. That is 96 classifications
with zero mismatches. The original failing receipt remains historical evidence.
[Independent QA](review/report.md) explicitly resolves SSH-POST-DESTINATION;
its current findings and implementation-defect lists are empty.

## Source-bound acceptance evidence

The current HEAD matches the required baseline. I rehashed all 80 inventory
inputs and checked their sizes, line counts and modes. The before/after inventories
and every current required receipt match. Implementation and QA inventories agree
on their common path/hash/byte/line fields. All inventoried QA artifact hashes
also match. The [source inventory](verification/source-inventory.json),
[test matrix](verification/test-matrix.md), [verification summary](verification/summary.json)
and [review summary](review/summary.json) are bound by hashes in final-summary.json.
All 65 inventoried Rust source/test files, tracked and untracked, are at most 474
lines, under the 500-line cap. Migration `0001_phase0.sql` and the harness match
HEAD bytes. Retained contract/CLI hashes and the value-scope artifact also match.

The independent reviewer ran these exact commands serially after the SSH repair,
on the sources rehashed here. Full argv, cwd, context, timestamps, exits and logs
are under `verification/`; no exit was inferred from an Actor's summary.

| Command | Actual independent result | Receipt |
| --- | --- | --- |
| `cargo test -p arc-daemon --test classification privacy::required_loopback_endpoint_nonexecution_and_privacy -- --exact` | Exit 0; one passed | [privacy.json](verification/privacy.json) |
| `cargo fmt --all -- --check` | Exit 0 | [fmt.json](verification/fmt.json) |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0 | [clippy.json](verification/clippy.json) |
| `cargo test --workspace` | Exit 0; 209 passed, zero failed/ignored | [workspace-tests.json](verification/workspace-tests.json) |
| `./scripts/harness/validate.sh` | Exit 1; missing `test:coverage`, one violation | [harness.json](verification/harness.json) |
| `python3 .arc-verify/command-classification/verification/ssh-repair.py` | Exit 0; 32 cases, zero mismatches across three interfaces | [ssh-repair.json](verification/ssh-repair.json) |
| `cargo test -p arc-detector tests::production_work_exhaustion_trace -- --exact --nocapture` | Exit 0; one passed | [work-trace.json](verification/work-trace.json) |

These checks ran 2026-11-07 at 21:10–21:12 UTC, after the operator's TCP environment
change and on repaired sources. The TCP privacy test successfully listened on
loopback. The old PermissionDenied/errno-1 receipt is superseded and supplies no
current blocker. Independent QA is fresh post-repair execution; earlier passing
Cargo receipts and pre-repair verdicts do not certify this repair. The unchanged
harness's actual failure is retained without skips, dummy coverage tooling or a
changed verification command. Its first four checks passed; its coverage check did
not. Test count and line cap success do not substitute for coverage acceptance.

The behavior matrix maps the required Cargo/gh/SSH/compound/quote/UNKNOWN cases,
all five limits, no-side-effect checks and compatibility guarantees to executed
deterministic tests. Saturated classification tests also cover continuing health,
malformed-request recovery and joined shutdown. Recorded microsecond timings are
one macOS debug run; they do not establish general performance or resource savings.
The supplied framework dependency reviews for tasks 1–5 pass, including the
Technical Director's repaired-source and independent-QA reviews. Those supplied
reviews are separate from the final task-6/mission framework review still pending.

## Decisions, alternatives and recovery lessons

One implementation owner retained the coupled Rust detector/IPC/daemon/CLI work
and shared manifests. The existing Serde, transport and Clap stack were reused;
there is no new external lockfile package or parser platform. A bounded scanner
with an explicit grammar was selected over broad shell interpretation. It supports
the required subset and makes uncertainty explicit; it leaves additional dialects,
scripts and locality resolution for later acceptance. The one-string CLI preserves
quotes and compounds; an argv mode or interactive wizard would require another
contract and is outside this slice.

The important recovery decision was to send an independently reproduced defect
back to its implementation owner. Further unchanged reviewer attempts could only
repeat the finding. Permanent regression matrices plus fresh IPC/CLI probes closed
the gap that earlier passing suites missed. Likewise, an operator-reported
prerequisite change required a fresh TCP check, not repetition of a stale denial.
The failed harness stays visible even when the bounded slice passes Cargo gates.

This task executed scoped Git status/diff/HEAD reads, known-path document/source
reads, `git show HEAD:<migration-or-harness>` comparisons, and inline Python
hash/receipt/probe/output checks. It did not rerun Cargo or the harness: final
inputs are unchanged and final executable verification is a downstream gate.
Two local validation attempts exited 1 before the corrected check passed:
raw inventory-object equality included QA-only mode/tracked fields, and raw probe
expectation equality included newly asserted reason fields absent from the old
probe. Normalizing common inventory fields and checking every original expected
field corrected the validation scripts; no source or expectation was changed.
Repeated final case-sensitive content checks caught capitalized obligation names;
managed-execution and desktop wording was corrected, then the check passed.

Serena tools were unavailable. Inspection used known paths and artifact-provided
inventories, with no native source-content search. A mise tracking-link permission
warning accompanied successful Python commands; it did not prevent checks. No
global-clean worktree claim is made. No build, installation, browser/GUI/screen
check, external delivery, publication, outreach, spending or commercial study was
performed. No additional Actor or recovery budget was created.

## Remaining obligations and next actions

1. The supervising framework must independently review these final artifacts and
   run the unchanged final executable verification contract. Current four scalar
   booleans describe this source-bound assessment, not completion of those gates.
2. Harness coverage remains unresolved: supply a separately scoped, real coverage
   integration and demonstrate its thresholds before claiming that acceptance.
   This task owns no harness or package repair and changes neither.
3. Detector acceptance remains open for wider catalogs, shell dialects, scripts,
   wrappers and actual host/Docker locality resolution. The supported syntax
   subset is not complete shell or topology analysis.
4. Scheduling remains open, including resource policy and job admission decisions
   beyond preserved IPC admission bounds.
5. The managed-execution capability remains open, including permits, starting/stopping jobs,
   cancellation and runtime enforcement.
6. The desktop integration and its acceptance remain open.
7. Native Windows implementation and native Windows acceptance remain open.

No customer study, adoption, profit, commercial validation or measured resource
saving is established. [Value scope](value-scope.md) retains ARC identity and
bounded benefit; commercial hypotheses remain outside this delivery.

Acceptance checklist for task-6: delivered behavior/evidence and actual coverage
failure recorded; current source/specification hashes matched; SSH blocker actually
resolved by its owner and independently verified; preserved interfaces recorded;
all remaining areas, alternatives, lessons and next actions explicit; required
scalar assessment fields truthful; final framework and executable gates pending.
