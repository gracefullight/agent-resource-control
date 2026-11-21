# Phase 0 closure inventory

This inventory fixes the finite cases for the five selected rows from the
[SRS](ARC_SRS_2026-11-06.md). Each numbered item is a required case or a
finite group of the explicitly named variants. `HOST` means an isolated,
offline fixture can exercise the production CLI/daemon path on this host;
`EXTERNAL` would mean the remaining acceptance actually needs an unavailable
external environment. External items below require a live cluster or a Windows/WSL environment.
The agent scenario is simulated, not live vendor or GitHub acceptance. A
missing HOST test is work remaining, not Blocked.

The named test mapping and the current integration assessment are below.
A focused test receipt is not the runner's final verification. The 42 historical simulated remote fixtures remain separate from
the unique named built-CLI/real-daemon cases and are not counted twice for
the two routes. A remote-case report must give each denominator separately,
plus recognized-remote, UNKNOWN and false-local counts on each production
route. UNKNOWN must never be counted as recognized remote.

Sources: [parser §8.2](ARC_SRS_2026-11-06.md#82-파서-아키텍처),
[locality §8.5](ARC_SRS_2026-11-06.md#85-locality-판정네트워크-요청과-로컬-컴퓨팅의-분리),
[detector §8.7](ARC_SRS_2026-11-06.md#87-detector-요구사항),
[safety §17.3](ARC_SRS_2026-11-06.md#173-파일-삭제-안전-규칙),
[NFR §18](ARC_SRS_2026-11-06.md#18-성능운영비기능-요구사항-nfrslo) and
[traffic acceptance §21.2](ARC_SRS_2026-11-06.md#212-gherkin-기반-acceptance-tests--traffic).
Unless otherwise stated, locality evidence is in
`capture_integration::closing_locality`, shell evidence is in
`capture_integration::closing_shell`, and hook/execution evidence is in
`capture_integration::closing_exec`; all must exercise built CLI and daemon,
not only a pure detector or a separately submitted classification.

## DET-005 — remote CI and SSH exclusion

Source: §8.5, §8.7 DET-005; linked AT-TRF-02, AT-TRF-04 and AT-TRF-12.

1. **HOST** — `gh workflow run build.yml` with proven remote host: REMOTE_CI;
   evidence: installed-hook execution in `closing_exec` (AT-TRF-02).
2. **HOST** — `gh run watch` and `gh run view`: REMOTE_CI queries;
   evidence: each named case in `closing_locality`.
3. **HOST** — `git push` with a proven remote CI destination and
   `gitlab pipeline run` with a proven remote host: REMOTE_CI; evidence:
   each named case in `closing_locality`.
4. **HOST** — `ssh build01 'make test'` with a proven remote destination:
   REMOTE_EXEC; evidence: `closing_locality`.
5. **HOST** — `docker --context prod build .` and `docker buildx build .`
   with proven remote endpoints: REMOTE_EXEC; evidence: `closing_locality`
   and the held-permit Buildx execution (AT-TRF-04).
6. **HOST** — `kubectl exec remote-pod -- make` without placement proof is
   UNKNOWN/OBSERVE; see `closing_locality::kubectl_without_placement_proof_stays_observed_without_starting_a_probe`. Proven remote placement is item 12.
7. **HOST** — `ssh -L8080:localhost:80 build01` forwarding, a missing Docker
   context, and a Buildx builder with mixed local/remote nodes:
   UNKNOWN/OBSERVE; evidence: `closing_locality`.
8. **HOST** — `make test && gh workflow run ci.yml` with the literal Makefile
   and proven remote CI endpoint specified in DET-001 item 7: MIXED; evidence:
   `closing_shell` on both production routes. This is classification evidence
   only for AT-TRF-12.
9. **HOST** — `docker build .` and `docker buildx build .` with proven local
   engine/builder endpoints are BUILD/HOST_LOCAL controls. For `act -j build`,
   `wsl.exe -- cargo build`, and the fixture `./self-hosted-runner` with no
   proof of their execution endpoint, the expected result is UNKNOWN/OBSERVE.
   The SRS labels
   an actual WSL workload LOCAL_VM and a runner on this machine HOST_LOCAL;
   these unproved negative controls do not complete those broader
   local-detection cases. Evidence: individual `closing_locality` controls.
10. **HOST** — `pnpm run deploy` with fixture script
    `cargo build && gh workflow run ci.yml` and a proven remote CI endpoint is
    MIXED. Evidence:
    `capture_integration::closing_locality::pnpm_deploy_literal_build_and_remote_ci_is_mixed_on_both_routes`.
11. **HOST** — for `docker --context prod build .`, explicit local versus
    remote endpoint facts take precedence over the context name; conflicting
    or incomplete endpoint evidence is UNKNOWN/OBSERVE. Evidence:
    `closing_locality`.
12. **EXTERNAL** — a pod observed on a genuinely remote cluster has proven
    remote placement. The local capture path does not probe a cluster; the
    simulated pod fact is not a live placement observation.
13. **HOST** — `act -j build` with an explicit Unix Docker endpoint and a
    fixture listener proven to be a local engine is BUILD/HOST_LOCAL and
    receives local admission.
14. **HOST** — a same-PC simulated self-hosted runner starts a locally
    managed `cargo build`; its actual child identity and local admission are
    recorded. An arbitrary runner without that evidence stays UNKNOWN.
15. **EXTERNAL** — `wsl.exe -- cargo build` with actual LOCAL_VM placement
    requires a Windows host with WSL; the unproved macOS control in item 9
    remains UNKNOWN/OBSERVE.

## SAFE-008 — remote work is not forcibly scheduled locally

Source: §17.3 SAFE-008 and §8.5; linked AT-TRF-02, AT-TRF-04, AT-SWP-20.

1. **HOST** — installed-hook `gh workflow run build.yml` executes while a
   separate local Build permit remains held, with no new queue entry or
   reservation; evidence: `closing_exec` (also AT-TRF-02).
2. **HOST** — remote SSH and remote Docker context/Buildx execution are
   recorded against their actual stub child without local queue or reservation;
   evidence: `closing_locality` and held-permit Buildx test (AT-TRF-04).
3. **HOST** — remote Git push, GitLab dispatch and gh queries do not acquire a local permit; kubectl without placement proof remains UNKNOWN/OBSERVE without a local permit. Proven remote pod placement is item 6.
4. **HOST** — unresolved remote destinations and SSH forwarding remain
   UNKNOWN/OBSERVE without forced local scheduling; evidence:
   `closing_locality`.
5. **HOST** — mixed local-test/remote-CI classification is not misrepresented
   as a blanket remote bypass; evidence: `closing_shell`. The linked
   AT-TRF-12 requires separate stage-by-stage control and dispatch evidence.
6. **EXTERNAL** — a genuinely remote pod and its remote execution require a live cluster; see DET-005 item 12.

AT-SWP-20 also requires a remote Docker context to disable the Docker sweep
switch **and** a local context's unconnected named volume to remain a candidate
without deletion when the volume switch is OFF. Neither a remote-build
classification nor this scheduling row completes that sweep acceptance test.

## NFR-007 — zero false-local for the finite remote cases

Source: §18 NFR-007 and the named §8.5 examples; linked AT-TRF-02 and
AT-TRF-04. This is the mission's finite SRS-derived set, not a claim to have
enumerated every possible official trigger.

Preservation evidence, separate from the numbered HOST acceptance cases: retain
CI01–CI20 and EP01–EP22 (42 simulated cases) and report their outcomes per
detector route. The fixture tests are
`remote_ci::ci01_through_ci20_remote_denominator_is_zero_false_local_on_both_routes`
and `remote_endpoints::named_simulated_srs_endpoint_denominator_has_zero_false_local`.
These injected-fact detector tests are not built-CLI/daemon execution evidence.

1. **HOST** — the distinct production-path remote cases in DET-005 items 1–6
   and SAFE-008 items 1–3 have named inputs and outcomes on both CLI
   classification and execution routes; report denominator, recognized remote,
   UNKNOWN, and false-local (must be zero) without double-counting routes or
   overlapping cases. Production-path evidence:
   `capture_integration::closing_locality::proven_remote_cases_never_classify_local_on_either_route`
   and `capture_integration::closing_exec::installed_claude_hook_executes_same_remote_child_with_held_build_permit`.
2. **HOST** — ambiguous/unknown controls and local controls in DET-005 items
   7 and 9 are reported separately from proven-remote cases. Passing
   production-path evidence:
   `capture_integration::closing_locality::ambiguous_and_local_controls_are_counted_separately`.
3. **EXTERNAL** — the genuinely remote pod-placement case in DET-005 item 12 requires a live cluster; its unproved control is HOST and remains UNKNOWN.

## AT-TRF-02 — agent-triggered GitHub Actions bypass

Source: [AT-TRF-02](ARC_SRS_2026-11-06.md#at-trf-02-원격-github-actions-우회).

1. **HOST** — install an existing supported hook in a temporary home and
   submit the representative `gh workflow run build.yml` agent event; the
   returned command is the one executed. Evidence: `closing_exec`.
2. **HOST** — against a real daemon and offline gh stub while another local
   Build permit is held, the same execution is recorded REMOTE_CI; stub runs
   exactly once and creates no local queue entry or reservation. Correlate
   event, execution and child/daemon record. Evidence: `closing_exec`.
3. **HOST** — preserve argv, cwd, environment, stdin/stdout/stderr, exit
   status, existing Cargo/Bun build-hook rewrites and normal vendor permission
   evaluation, without an approval decision. The installed-hook tests in
   `closing_exec` compare the actual execution and native result in sh, Bash
   and zsh, including aliases, functions, the continuing shell and zsh line
   numbers. Non-default inherited shell options, altered special parameters and
   descriptor limits below 256 are outside the operator's stated scope.
4. **HOST** — delayed endpoint capture beyond the former preparation budget
   still registers with a healthy daemon and classifies/executes the same child
   once; genuine daemon outage remains fail-open. Use injected clock and
   observed state. Evidence: named exec-route regression and `closing_exec`.

## DET-001 — Bash/zsh direct, pipeline and compound recognition

Source: §8.2 grammar, §8.7 DET-001, §8.5 mixed example, linked AT-TRF-12.
Run each applicable case as both Bash and zsh inspection and through the
managed execution route; unsafe dynamic forms may be UNKNOWN/OBSERVE rather
than guessed. All evidence targets are `closing_shell` unless specified.

1. **HOST** — direct `cargo build` is BUILD/HOST_LOCAL; `bun test`,
   `cargo test 'two words'`, and `MODE=test cargo test` are TEST/HOST_LOCAL.
   Evidence: `capture_integration::closing_shell::direct_commands_are_classified_on_both_routes`.
2. **HOST** — `cargo test | cargo test`, `cargo test && cargo test`,
   `cargo test || cargo test`, `cargo test; cargo test` and `(cargo test)`
   are TEST/HOST_LOCAL. Evidence:
   `capture_integration::closing_shell::test_pipelines_and_compounds_are_classified_on_both_routes`.
3. **HOST** — `cargo test $(printf x)`: conservative UNKNOWN/OBSERVE without
   evaluating the substitution during classification. Evidence:
   `capture_integration::closing_shell::command_substitution_is_observed_without_evaluation`.
4. **HOST** — with the offline cargo fixture at each indicated path,
   `./cargo build`, `<fixture-root>/bin/cargo build` (a literal absolute
   path), and `./검증/cargo build` are BUILD/HOST_LOCAL. With an existing
   fixture directory, `cd fixtures && cargo build` is BUILD/HOST_LOCAL;
   `cd "$TARGET" && cargo build`, with `TARGET=fixtures`, is UNKNOWN/OBSERVE
   because the directory is dynamically selected. Evidence:
   `capture_integration::closing_shell::literal_paths_and_directory_changes_are_classified_on_both_routes`.
5. **HOST** — `env cargo test`, `timeout 10 cargo build`, `nice cargo test`,
   `sudo cargo build` and `printf x | xargs cargo test`. The first and third
   are TEST/HOST_LOCAL, the second is BUILD/HOST_LOCAL, and the sudo and
   piped-input xargs cases are UNKNOWN/OBSERVE. Use offline wrappers, not a
   real privilege operation. Evidence:
   `capture_integration::closing_shell::literal_and_unproved_wrappers_are_classified_on_both_routes`.
6. **HOST** — `sh -c 'cargo test'` is TEST/HOST_LOCAL on both routes; evidence: `capture_integration::closing_shell::nested_sh_literal_is_local_test_on_both_routes`.
   For `bash -c 'gh workflow run build.yml'` with a fixture `BASH_ENV`, and
   `bash -lc 'gh workflow run build.yml'` with a fixture login profile, the
   startup files set `GH_HOST=localhost`: both are UNKNOWN/OBSERVE on both
   routes, and inspection does not run startup. Passing baseline evidence:
   `capture_integration::closing_shell::nested_bash_startup_cannot_borrow_captured_endpoint_facts_on_either_route`.
7. **HOST** — `make test && gh workflow run ci.yml` with a literal Makefile
   `test` recipe `cargo test`, and `bun test && gh workflow run ci.yml`, with
   a proven remote CI endpoint, are MIXED on both routes. Evidence:
   `capture_integration::closing_shell::actual::bash_and_zsh_execute_the_same_direct_pipeline_compound_and_mixed_commands`.
   These results do not establish AT-TRF-12's separately required local-Test
   control and remote dispatch.
8. **HOST** — the following dynamic wrapper fixtures are UNKNOWN/OBSERVE on
   both routes; inspection does not execute their contents. `./build.sh`
   contains `#!/bin/sh` followed by `exec "$RUNNER"`; `pnpm run deploy` has
   `package.json` script `"deploy": "cargo build && $DEPLOY_STEP"`; `make test`
   has `Makefile` recipe `$(TEST_RUNNER) test`; `just test` has `Justfile`
   recipe `{{env_var("TEST_RUNNER")}} test`; and `task test` has a
   `Taskfile.yml` `test` command `{{.TEST_RUNNER}} test`. The fixture sets
   `RUNNER` and `TEST_RUNNER` to the offline `cargo` stub and `DEPLOY_STEP` to
   `gh workflow run ci.yml`. Production-path evidence:
   `capture_integration::closing_shell::dynamic_wrappers_and_targets_are_observed_on_both_routes`.
   Separately, `npm run verify` has `package.json` scripts
   `"preverify": "$PREVERIFY"` and `"verify": "eslint && vitest"`; with
   `PREVERIFY` set to an offline stub, the dynamic lifecycle makes the whole
   invocation UNKNOWN/OBSERVE on both routes and is not executed by inspection.
   Evidence:
   `capture_integration::closing_shell::npm_verify_with_dynamic_lifecycle_is_observed_on_both_routes`.
   These dynamic controls do not replace DET-005 item 10's distinct literal
   mixed deployment script.

The PowerShell, cmd, Windows Drive/UNC and unrelated language-catalog cases in
§8.2 are not Bash/zsh DET-001 cases. Their own requirement rows remain
independent. AT-TRF-12 is not Complete until its stage-by-stage acceptance is
demonstrated; classification of either mixed example is not that demonstration.

## Task 1 baseline evidence

The first-wave named built-CLI/real-daemon baseline comprises four distinct
offline cases, observed on each production route: the proven gh workflow case
in `closing_locality::proven_gh_remote_is_the_same_inspected_and_executed_child`,
the custom-command control in
`closing_exec::unknown_child_runs_once_with_native_arguments_and_recorded_outcome`,
and Bash `-c` and `-lc` startup controls in
`closing_shell::nested_bash_startup_cannot_borrow_captured_endpoint_facts_on_either_route`.
The production-path baseline denominator is **4**, with **1 recognized remote,
3 UNKNOWN, and false-local: 0** per route; the two routes observe the same four
cases. These controls do not complete the full SRS-derived inventories above.
Protocol-1's existing syntax-only classification remains compatible; the CLI
rejects a confident legacy reply when the guarded contextual analysis does not
agree, as checked by
`classify::tests::repair::nested_bash_rejects_legacy_peer_results_when_startup_can_change_endpoints`.

The two preserved simulated detector fixture tests were rerun independently:
their separate simulated denominator is **42** (CI01–CI20 and EP01–EP22),
with **42 recognized remote, 0 UNKNOWN and false-local: 0** per route. These
injected facts are not live endpoint or vendor acceptance. An ordinary run
with `LLVM_PROFILE_FILE` set also passed the new closure cases and the exact
environment comparison; that checks forwarding, not a coverage measurement.

Task 1 used `oma-backend`. The focused CLI/daemon build passed. Named checks
passed for the detector guard and frozen protocol-1 matrix, the CLI legacy-peer
guard and existing capture path, the three `capture_integration::closing_`
tests, four `shared_execution_contract::synthetic_` tests, both simulated
denominators and the daemon/CLI classification matrices. Scoped Clippy passed
for detector and CLI all targets and daemon all targets; scoped rustfmt and
`git diff --check` passed. The first version of the guard failed the frozen
protocol-1 matrix and empty-shell diagnostic; a contextual-only guard repaired
both. A preliminary daemon/CLI matrix run failed its old CLI expectation for
case 17; its prior fixture did not establish account/interface isolation.
The repaired matrix retained the protocol-1 assertion, required the new
UNKNOWN/OBSERVE CLI result, installed synthetic account/interface interposition
and passed again with an isolated daemon. Runner-owned checks remain pending.

Task 1 repair rechecked the named detector and legacy-peer guards, all three
closure baselines, the daemon/CLI contract matrix, four synthetic shared
execution tests, and both simulated-denominator tests; all passed. The three
closure baselines and the exact-environment test also passed with
`LLVM_PROFILE_FILE` set. `cargo build -p arc-cli -p arc-daemon --bin arcctl
--bin arc-exec-fixture --bin arc-daemon` passed before those checks. The
simulated and production baseline denominators above remain distinct; none of
the planned lane tests above is asserted to have passed in this repair.
The first inventory-structure check failed because its assertion compared an
uppercase word against lowercased text; the corrected check passed for all
five sections, consecutive numbering and the named examples. The contract
check passed for its size, the exact four commands and non-ignored handoffs.

Lane coordination: `crates/arc-cli/src/exec/capture.rs::merge_invocation`
currently rejects UNKNOWN leaves before merging endpoint facts, while
`crates/arc-cli/src/exec/metadata.rs::classify_with_context` delegates
non-Bun native argv to the detector without expanding shell or target
metadata. The positive Make/mixed and native `sh -c` cases therefore need
the shell-parser, locality-capture and exec owners to connect their respective
routes; integration owns final wiring. These are planned lane obligations,
not passing evidence from the task 1 baselines.

The configured Serena tools were unavailable, so inspection used scoped reads
of already identified files. Git reported `av.yaml: Operation not permitted`;
the tracked HEAD blob is readable but does not establish the current runner
configuration or configured container health. Global worktree cleanliness and
those runner prerequisites remain unverified.

## Selected row status

| Row | Status | Remaining item |
| --- | --- | --- |
| DET-001 | Complete | None in the numbered HOST list. |
| DET-005 | Blocked | External remote pod and Windows WSL placement. |
| SAFE-008 | Blocked | External remote pod placement. |
| NFR-007 | Blocked | External remote pod placement. |
| AT-TRF-02 | Complete | None in the numbered HOST list under the operator's shell-state ruling. |

## Integration evidence and outstanding boundary

The current repair inspected Actor snapshot commit
`8795feb3575e0f3f3f175253df6ef696af076bc9`, tree
`350e64fd5d9923bcfc6ecb742d3a5da6d278b202`; its hook, test and documentation
edits are uncommitted on that snapshot. The earlier integration attempt inspected snapshot commit
`282168c41863fab677de50c4d81d95517bfc05a8`, tree
`35658d5ec45e8c584baeecb101b231bad1b5e4fd`, with its integration edits then
uncommitted. The preceding repair inspected snapshot `ffd79b71ca0e07f0e5855b87170378f3e7d6f659`,
tree `76eba86d02af69a0b8929e2cde170200424404de`, which includes those earlier
deliveries. The repair edits are uncommitted on this snapshot. The accepted
committed baseline is `566db287c54320a49692ac238220b180f21787fe`, tree
`b7622ba1c876b8cc4680358acbd4857ac304cce8`. These are focused local checks,
not the runner's final execution or independent QA.

The initial pnpm mixed test returned UNKNOWN on both routes; after bounded
capture and script-proof wiring it passed on both. The first Make mixed run
failed because macOS's case-insensitive filesystem made `makefile` and
`Makefile` aliases; selecting Make's actual precedence fixed it. The first
closed-stderr trace test failed because the helper process reopened a standard
descriptor; the revised shell-side check passed alone and in the 19-test
hook/execution group. These are repaired failures, not flaky-test exclusions.

- DET-005 items 1–5: `closing_locality::proven_gh_remote_is_the_same_inspected_and_executed_child`, `closing_locality::proven_remote_cases_never_classify_local_on_either_route`, `closing_locality::gitlab_explicit_public_endpoint_and_unresolved_controls_on_both_routes`, and `closing_locality::remote_docker_and_ssh_execute_while_local_build_permit_remains_held`.
- DET-005 items 6–7, 9 and 11: `closing_locality::kubectl_without_placement_proof_stays_observed_without_starting_a_probe`, `closing_locality::ambiguous_and_local_controls_are_counted_separately`, `closing_locality::explicit_prod_context_takes_precedence_over_docker_host_on_both_routes`, and `closing_locality::docker_local_listener_controls_use_both_production_routes_and_admission`. The last test includes the proven-local act item 13; item 14 is `closing_locality::integration::same_pc_runner_starts_a_recorded_locally_admitted_build_child`. The local engine listener inventory is simulated and the socket is not contacted by classification.
- DET-005 items 8 and 10: `closing_shell::actual::bash_and_zsh_execute_the_same_direct_pipeline_compound_and_mixed_commands`, `closing_shell::actual::makefile_selection_and_post_capture_changes_fail_closed_on_both_routes`, and `closing_locality::pnpm_deploy_literal_build_and_remote_ci_is_mixed_on_both_routes`; `closing_locality::integration::pnpm_manifest_change_after_capture_is_observed_on_both_routes` checks revalidation.
- SAFE-008 items 1–5 and NFR-007 items 1–2 use the same named cases; `closing_exec::installed_claude_hook_executes_same_remote_child_with_held_build_permit` proves the installed-hook child and held-permit record together.
- DET-001 items 1–8: `closing_shell::direct_commands_are_classified_on_both_routes`, `test_pipelines_and_compounds_are_classified_on_both_routes`, `command_substitution_is_observed_without_evaluation`, `literal_paths_and_directory_changes_are_classified_on_both_routes`, `literal_and_unproved_wrappers_are_classified_on_both_routes`, `nested_sh_literal_is_local_test_on_both_routes`, `nested_bash_startup_cannot_borrow_captured_endpoint_facts_on_either_route`, `dynamic_wrappers_and_targets_are_observed_on_both_routes`, `npm_verify_with_dynamic_lifecycle_is_observed_on_both_routes`, `actual::bash_and_zsh_execute_the_same_direct_pipeline_compound_and_mixed_commands`, and `actual::zsh_startup_that_changes_endpoints_is_observed_on_both_routes`. `actual::managed_bash_and_zsh_records_name_the_real_shell_child` ties the held fixture child to the daemon's shell-child identity. These are under `capture_integration::closing_shell`; the shell tests exercise the same source through real isolated Bash/zsh managed entry points.
- AT-TRF-02 items 1–2 and 4: `closing_exec::installed_claude_hook_executes_same_remote_child_with_held_build_permit`, its Bash and zsh variants, `injected_capture_delay_keeps_daemon_registration_and_child_identity`, and `actual_daemon_outage_still_runs_the_offline_child_once`. Item 3 includes `installed_hook_preserves_sh_functions_and_aliases`, the Bash and zsh function and alias regressions, and `edge_cases::installed_hook_preserves_default_shell_state_after_managed_execution`, `installed_hook_preserves_later_zsh_line_number_in_default_shell`, and `installed_hook_preserves_zsh_environment_when_command_ends_with_semicolon`. `edge_cases::installed_hook_passes_through_zsh_script_context_when_exec_position_is_unknown` checks the conservative script fallback.

In the preceding snapshot's separate isolated-shell comparisons, Bash 5.3 with
`BASH_XTRACEFD=10` directed to stderr produced 225 extra stderr bytes for the
rewritten event, while status and stdout matched; descriptor 3 matched all
three. With zsh tracing enabled, the submitted `gh` command's output matched,
but a subsequent `print` in the same shell was traced at `zsh:23` instead of
`zsh:1` because the returned rewrite contains source newlines. These are
observed stream differences on that snapshot. The current single-line rewrite
fixes the default-shell line-number difference. The operator's later ruling
places inherited xtrace and high trace descriptors outside this mission's
acceptance; the earlier high-descriptor observation is not claimed fixed.

The current focused repair passed `cargo test -p arc-daemon --test
managed_exec closing_exec -- --test-threads=1` (26 tests), including the
installed-hook held-permit, deadline and outage cases. The five new edge cases
also passed with `LLVM_PROFILE_FILE` supplied; that establishes propagation,
not coverage. `cargo test -p arc-cli --bin arcctl hooks -- --test-threads=1`
passed 23 tests; the Claude and Cursor hook targets passed 27 and 44. Scoped
CLI and daemon-test Clippy and changed-file rustfmt checks passed. An initial
unqualified `--exact` filter selected zero tests and is not counted; the
corrected line-number test first reproduced `23` versus `1` and then passed.
The `closing_locality` group passed 14 tests and `closing_shell` passed 15.
The earlier `remote_ci::derived_hooks_and_real_daemon_bypass_a_held_build_permit_with_one_offline_gh_execution` also passed.
Both named simulated-denominator tests passed in the daemon test target; a
prior attempt against the detector's reserved test target selected zero tests
and is not counted. The 42 simulated and 10 production-path cases retain the
separate outcomes documented in [traceability](traceability.md#phase-0-closure-integration-evidence).
An exploratory interrupted-work test with a 10-descriptor limit still failed
on `/bin/sh`; limits below 256 are outside the operator's later ruling and
remain a follow-up. That exploratory test was not part of the tracked snapshot.

The preceding repair's checks used `oma-backend`, offline Cargo and `RUSTC_WRAPPER=`. The
scoped CLI fixture build passed. `cargo test -p arc-cli --bin arcctl hooks`
passed 23 tests, and the Claude and Cursor hook test targets passed 27 and 44.
`cargo test -p arc-daemon --test managed_exec capture_integration::closing_exec`
passed 21, including both new exact-event trace regressions, the held-permit
hook execution, injected capture delay and genuine-outage execution. The
held-permit case also passed with `LLVM_PROFILE_FILE` set; this is forwarding
evidence, not a coverage measurement. The two runner-reported classification
tests, `literal_payloads_and_options_are_preserved_in_one_request` and
`actual_daemon_and_cli_delivered_contract_matrix`, passed after linking the
system Bash and zsh executables into the temporary fixture PATH. Named
Kubernetes no-probe, local Docker listener, pnpm mixed-script, and real
Bash/zsh mixed-command tests passed. Scoped CLI and daemon-test Clippy and
changed-file rustfmt checks passed. The two simulated-denominator tests and
all four named tests in the 10-case production denominator were rerun in
this repair and passed; the counts remain 42/42 remote for the simulated set
and 9 remote plus 1 UNKNOWN for the separate production set, with zero
false-local on each route.

The first trace test attempt lacked the separately built `arcctl` fixture;
after the scoped build, both new trace regressions reproduced the reported
differences. Intermediate hook changes failed existing zsh compatibility and
the hook rewrite shape assertion; those failures were fixed and their groups
passed above. An attempted `arc-cli --lib` check failed because that package
has no library target; the binary-target hook check passed. The high-descriptor
observation belongs to that earlier snapshot. The
runner's workspace, desktop, coverage, traceability and harness gates remain
pending; `av.yaml` still returns `Operation not permitted`, so its actual
configuration and any configured container health are not established here.

The 42 simulated cases and the 10 named production-path remote/uncertain
cases are accounted separately in [traceability](traceability.md#phase-0-closure-integration-evidence).
`AT-TRF-12` and `AT-SWP-20` remain at their existing statuses. The Kubernetes
probe remains removed; an API endpoint alone does not prove pod placement.

## Remaining Phase 0 inventories

These lists record the SRS obligations and their existing traceability status;
they do not promote unrelated rows. `HOST` identifies a locally testable case,
including one that has not yet been implemented or rerun. `EXTERNAL` identifies
a case requiring the stated unavailable platform or installed environment.
Historical evidence for each row is in its corresponding primary row in
[traceability](traceability.md); a historical receipt is not a new check.

### Requirements

- **DET-002** — 1. **EXTERNAL** PowerShell direct command and quoting on Windows; 2. **EXTERNAL** Windows cmd direct command and quoting. Pending.
- **DET-006** — 1. **HOST** the selected remote `gh workflow run` does not occupy a local slot. Complete in the existing row; this does not itself close AT-TRF-02's hook journey.
- **DET-007** — 1. **HOST** an unverified command remains UNKNOWN/OBSERVE and executes without forced admission. Complete in the existing row.
- **DET-008** — 1. **HOST** analysis of a fixture build and poisoned metadata does not execute the workload. Complete in the existing row.
- **SAFE-001** — 1. **HOST** owned process identity is tracked; 2. **HOST** a process of unknown ownership is never automatically killed by the Reaper. Partial; the latter acceptance is outstanding.
- **SAFE-007** — 1. **HOST** current-user job and socket ownership checks; 2. **EXTERNAL** another authenticated account's files and processes remain inaccessible. Partial; full cross-account acceptance is outstanding.
- **SAFE-011** — 1. **HOST** genuine daemon failure does not indefinitely prevent an ordinary development command. Complete in the existing row.
- **NFR-001** — 1. **HOST** measure long-run idle daemon CPU against the stated 0.5% target. Pending/Unverified.
- **NFR-002** — 1. **HOST** measure settled idle daemon RSS against the stated 50 MB target; 2. **EXTERNAL** establish the result on each required platform. Pending/Unverified.
- **NFR-003** — 1. **HOST** measure bypass-hook P95 against the stated 50 ms target. Pending/Unverified.
- **NFR-004** — 1. **HOST** measure small local IPC round trips against the stated 20 ms P95 target. Pending/Unverified.
- **NFR-005** — 1. **HOST** measure cached ordinary classification against the stated 50 ms P95 target. Pending/Unverified.
- **NFR-010** — 1. **HOST** forced termination and restart preserve database integrity; 2. **HOST** run the integrity check on the recovered database. Partial.
- **NFR-014** — 1. **HOST** a new non-destructive job executes once when the daemon is unavailable. Complete in the existing row.
- **NFR-022** — 1. **EXTERNAL** after real login, the installed daemon passes its health check; 2. **EXTERNAL** the installed tray passes its health check. Pending/Unverified.

### Acceptance tests

- **AT-TRF-11** — 1. **HOST** UNKNOWN command passes without a local permit; 2. **HOST** its actual execution and native result are retained. Complete in the existing row.
- **AT-GUI-01** — 1. **HOST** closing the fixture window hides it and leaves the daemon running; 2. **EXTERNAL** installed macOS menu-bar and Windows tray remain available after window close. Partial.
- **AT-GUI-04** — 1. **HOST** About shows the specified icon-only links and Try OMA; 2. **EXTERNAL** each link opens its official destination in the real default browser. Partial.

### Product decisions, goals, architecture and risks

- **DEC-001** — 1. **HOST** the product and switch display the specified Agent Resource Control / ARC identity. Complete.
- **DEC-003** — 1. **HOST** GUI and CLI use the shared Rust daemon; 2. **EXTERNAL** per-user installation and lifecycle on the required platforms. Partial.
- **DEC-004** — 1. **HOST** fixture window close hides without daemon exit; 2. **EXTERNAL** installed macOS menu-bar and Windows tray behavior. Partial.
- **DEC-005** — 1. **HOST** daemon uses Rust/Tokio and SQLite without a persistent Node or LLM service. Complete.
- **DEC-006** — 1. **HOST** desktop uses the declared React 19 and TypeScript 7 stack. Complete.
- **DEC-007** — 1. **HOST** Bun is the declared JS/TS package manager. Complete.
- **DEC-008** — 1. **HOST** desktop uses the declared Tailwind, basecn and Phosphor stack. Complete.
- **DEC-009** — 1. **HOST** Biome is the declared JS/TS formatting and lint tool. Complete.
- **DEC-012** — 1. **HOST** the named local and remote endpoint controls in DET-005; 2. **EXTERNAL** proven remote pod placement and Windows WSL placement. Partial; these cases do not close other endpoint families.
- **DEC-022** — 1. **HOST** specified About icons and CTA are displayed; 2. **EXTERNAL** actual browser opening of their official destinations. Partial.
- **G-02** — 1. **HOST** the named finite remote triggers bypass local admission; 2. **EXTERNAL** remaining placement-dependent scenarios; 3. **HOST** the broader official-trigger goal remains separately unverified. Partial.
- **ADR-002** — 1. **HOST** separated Rust daemon and React/Tauri GUI; 2. **EXTERNAL** installed cross-platform lifecycle. Partial.
- **ADR-005** — 1. **HOST** named endpoint-based local, remote and UNKNOWN decisions; 2. **EXTERNAL** live pod and WSL placement; 3. **HOST** remaining endpoint/tool-family cases. Partial.
- **ADR-011** — 1. **HOST** the declared Bun, TS7, React19, Tailwind4, basecn, Phosphor and Biome stack is maintained. Complete.
- **R-004** — 1. **HOST** macOS admission fallback; 2. **EXTERNAL** Windows and Linux capability/hard-limit behavior. Pending/Unverified.
- **R-007** — 1. **HOST** PID/start-time identity evidence; 2. **HOST** Reaper revalidation before acting on a reused PID. Partial.
- **R-018** — 1. **HOST** pinned dependencies and Rust/TS agreement; 2. **HOST** native no-bundle build smoke; 3. **EXTERNAL** multi-OS smoke. Partial.
