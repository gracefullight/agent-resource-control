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
SAFE-001, SAFE-007, NFR-010 and R-007 were assessed in the later Reaper
safety cycle; their named tests are in
[Phase 0 Reaper safety, ownership and recovery](#phase-0-reaper-safety-ownership-and-recovery).

### Requirements

- **DET-002** — 1. **EXTERNAL** PowerShell direct command and quoting on Windows; 2. **EXTERNAL** Windows cmd direct command and quoting. Pending.
- **DET-006** — 1. **HOST** the selected remote `gh workflow run` does not occupy a local slot. Complete in the existing row; this does not itself close AT-TRF-02's hook journey.
- **DET-007** — 1. **HOST** an unverified command remains UNKNOWN/OBSERVE and executes without forced admission. Complete in the existing row.
- **DET-008** — 1. **HOST** analysis of a fixture build and poisoned metadata does not execute the workload. Complete in the existing row.
- **SAFE-001** — 1. **HOST** owned process identity is tracked; 2. **HOST** a process of unknown ownership is never automatically killed by the Reaper. Complete.
- **SAFE-007** — 1. **HOST** current-user job and socket ownership checks; 2. **EXTERNAL** another authenticated account's files and processes remain inaccessible. Blocked; only item 2 remains.
- **SAFE-011** — 1. **HOST** genuine daemon failure does not indefinitely prevent an ordinary development command. Complete in the existing row.
- **NFR-001** — 1. **HOST** measure long-run idle daemon CPU against the stated 0.5% target. Pending/Unverified.
- **NFR-002** — 1. **HOST** measure settled idle daemon RSS against the stated 50 MB target; 2. **EXTERNAL** establish the result on each required platform. Pending/Unverified.
- **NFR-003** — 1. **HOST** measure bypass-hook P95 against the stated 50 ms target. Pending/Unverified.
- **NFR-004** — 1. **HOST** measure small local IPC round trips against the stated 20 ms P95 target. Pending/Unverified.
- **NFR-005** — 1. **HOST** measure cached ordinary classification against the stated 50 ms P95 target. Pending/Unverified.
- **NFR-010** — 1. **HOST** forced termination and restart preserve database integrity; 2. **HOST** run the integrity check on the recovered database. Complete.
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
- **R-007** — 1. **HOST** PID/start-time identity evidence; 2. **HOST** Reaper revalidation before acting on a reused PID. Complete.
- **R-018** — 1. **HOST** pinned dependencies and Rust/TS agreement; 2. **HOST** native no-bundle build smoke; 3. **EXTERNAL** multi-OS smoke. Partial.

## Phase 0 Reaper safety, ownership and recovery

This cycle assessed four rows from the inventories above against their
numbered cases: SAFE-001, SAFE-007, NFR-010 and R-007. Sources:
[safety §17.3](ARC_SRS_2026-11-06.md#173-파일-삭제-안전-규칙),
[NFR §18](ARC_SRS_2026-11-06.md#18-성능운영비기능-요구사항-nfrslo) and the
[risk register §25](ARC_SRS_2026-11-06.md#25-위험제약대응-책임). The shared
route, definitions and fixture seams are in the
[cycle contract](../../.arc-verify/phase-0-reaper-safety/contract.md).

Unless stated otherwise the tests below are in the `arc-daemon` target
`managed_exec` and run the built `arcctl`, a daemon and a real fixture child
with a cleared environment, a temporary HOME and explicit socket and database
paths. Two kinds of daemon appear, and each case says which:

- the **real daemon** is the `arc-daemon` binary, started isolated;
- a **fixture daemon** (`arc-reaper-daemon-fixture`,
  `arc-recovery-daemon-fixture`) compiles the production daemon sources and
  runs the same startup, adding only the seams named in the contract: an
  injected Reaper clock, a pass trigger and named faults, or a commit barrier.

### SAFE-001 — unknown ownership is never reaped

1. **HOST** — owned process identity is tracked.
   `reaper_safety::managed_route_tracks_owned_child` (real daemon): the job
   row names the wrapper and the child's PID, start time and boot, and a fresh
   native capture of that PID is the same identity.
   `reaper_safety::owned_inactive_child_reclaimed_and_recorded` (fixture
   daemon): a verified registration is recorded `PROVEN`; after the owner
   session is observed ended and the grace period has elapsed on the injected
   clock, the wrapper signals its own child, and the decision rows and four
   audit rows end in `RECLAIMED` / `TERMINATION_CONFIRMED`.
2. **HOST** — a process of unknown ownership is never automatically killed by
   the Reaper. `reaper_safety::protected_candidates_receive_no_signal`
   (fixture daemon): a real child of a client that never registers is recorded
   `BLOCKED` / `UNKNOWN_OWNERSHIP`; two forged registrations are rejected; a
   live owner session gives `SESSION_ACTIVE`; a restart of the daemon on the
   same database gives `INSUFFICIENT_AUTHORITY`. Both children stay alive and
   then exit by their own code 17.
   `reaper_safety::unconfirmed_actions_are_never_recorded_as_reclaimed`
   shows that an unread reply or a lost capability is never recorded as a
   reclaim. `reaper_escalation::injected_authority_and_stale_request_faults_send_no_signal_and_never_claim_reclaim`
   and `reaper_escalation::indeterminate_identity_sends_nothing_and_ignored_sigterm_escalates_after_grace`
   cover injected authority, stale-request and indeterminate-identity faults.

The linked AT-SWP-01 and AT-SWP-14 keep their own Pending status.

### SAFE-007 — current-user job and socket ownership

1. **HOST** — current-user job and socket ownership checks (real daemon).
   `current_user_ownership::built_cli_current_user_socket_and_job_ownership`:
   the endpoint is a socket owned by the effective user with mode `0600`; the
   daemon, CLI and child report this user's real and effective UID; the job is
   bound to the CLI that registered it and to the child that CLI launched.
   `current_user_ownership::forged_or_mismatched_job_identity_is_rejected`:
   the live daemon named as observer, an unrelated live same-user process or
   the observer itself named as child, and completion or attach as the CLI from
   another same-user peer (with and without a forged proof callback) are all
   rejected and leave the job unchanged; PID 1 never becomes a verified running
   child. A relay that forwards requests unchanged is still accepted.
2. **EXTERNAL** — another authenticated account's files and processes remain
   inaccessible. This needs a real second account on the host. It was not run,
   selected or claimed; every process above runs as the current user, and a
   same-user forged identity is not a second-account test.

### NFR-010 — database integrity after forced termination

1. **HOST** — forced termination and restart preserve database integrity
   (recovery fixture daemon, real SQLite writer).
   `crash_recovery::forced_daemon_death_before_commit_rolls_back_and_integrity_ok`:
   the writer is held inside the commit hook of one lifecycle start; an
   independent connection sees the write lock held, job 700 committed and job
   701 absent; the daemon gets SIGKILL there. After restart 700 is found, 701
   is not, and 701 registers again as `Created`.
   `crash_recovery::forced_daemon_death_after_commit_survives_and_integrity_ok`:
   the writer is held after the commit and before the reply; job 702 is
   visible, and a copy of the main file alone lacks it, so it exists only in
   the WAL; the daemon gets SIGKILL there. After restart 702 and 703 are found
   and the retry is `Unchanged`.
2. **HOST** — run the integrity check on the recovered database. Both tests
   restart the daemon on the same database and WAL, then read `ok` from the
   daemon's doctor report and from `PRAGMA integrity_check` on an independent
   connection, with `journal_mode` still `wal`.

The barriers are socket announcements, not sleeps. The uncommitted change is
lost from SQLite's page cache; a transaction large enough to spill
uncommitted frames into the WAL is not exercised. The linked AT-SWP-10 stays
Pending/Outstanding: its browser-profile GC crash scenario is not demonstrated.

### R-007 — PID reuse

1. **HOST** — PID/start-time identity evidence.
   `reaper_safety::managed_route_tracks_owned_child` (real daemon): a record
   with the live child's PID and a start time one tick earlier probes as
   `Mismatch`, and the exited child probes as `Missing`. The `arc-platform`
   unit test
   `reaper_identity::tests::fresh_direct_child_evidence_rejects_stale_identity_and_changed_fields`
   covers the parent, UID and executable fields of the same evidence.
2. **HOST** — Reaper revalidation before acting on a reused PID (fixture
   daemon). `reaper_safety::stale_recorded_identity_keeps_live_child_alive`: a
   live child is paired with a stale recorded identity, first at the daemon
   (`IDENTITY_MISMATCH`, nothing claimed) and then at the wrapper
   (`WRAPPER_IDENTITY_MISMATCH`, the claim withdrawn); the child stays alive
   and exits by its own code 17.
   `reaper_safety::revalidation_precedes_every_signal`: ten failing
   conditions and one wrapper refusal are each evaluated before SIGTERM and
   again before SIGKILL; none delivers a signal, and the escalation happens
   only after a separate, fully revalidated request. The wrapper-side gate is
   covered by the `arc-cli` unit tests
   `mismatched_identity_is_refused_and_each_accepted_request_sends_one_signal`
   and `no_signal_is_sent_after_reaped_clears_supervision`.

These are stale-identity tests. No test observes the operating system reusing
a PID, and AT-SWP-14 keeps its Pending status. The identity-bound supervision
is the wrapper itself: it is the child's direct parent and only reaper, and it
checks PID, start time, parent, both UIDs and the executable's path, device
and inode while holding the wait gate, so the PID cannot be reaped or reused
between the check and the signal. The daemon never signals.

### Row outcomes

| Row | Status | Remaining item |
| --- | --- | --- |
| SAFE-001 | Complete | None in the numbered HOST list. |
| SAFE-007 | Blocked | External cross-account case (item 2). |
| NFR-010 | Complete | None in the numbered HOST list. |
| R-007 | Complete | None in the numbered HOST list. |

Phase 0 is now 18 Complete, 10 Partial, 8 Pending and 4 Blocked of 40; see
[traceability](traceability.md#phase-0-reaper-safety-integration-evidence).

### Integration checks and boundary

This integration inspected Actor snapshot
`40d2ca1735e865da3ea24fb9ab512f5d912f7463`, tree
`5a58415d96c44c0f4f3c5463a635bb99a76cb528`; the accepted committed baseline is
`86fac84ec77e216f2d86b8c072140ccdf91564b7`, tree
`27635eb500955608140eb71ba557adc5bf5ae371`. One test repair and the
documentation edits are uncommitted on that snapshot. These are focused local
checks using `oma-backend` and offline Cargo, not the runner's final
verification or the independent QA report.

- Passed on the final source: `cargo fmt --all -- --check`; `cargo clippy
  --workspace --all-targets -- -D warnings`; `cargo test -p arc-daemon --test
  managed_exec` twice in succession, 163 of 163 each time; and the 17 Reaper,
  ownership, recovery and forwarder tests again serially on their own.
- Passed before the second forwarder fix below, which changed only that test
  file: `arc-storage` 172 (116 unit tests and its integration targets,
  including the four `nfr010` tests); `arc-platform` 15; `arc-ipc` 128; the
  `arcctl` unit tests 184; and the unit tests of `arc-daemon` 142 and of the
  Reaper and recovery fixture daemons 127 each.
  `bun scripts/requirements/traceability.ts` reports the matrix valid with
  the counts above.
- Repaired, two causes in one test: the runner's integration check of
  2026-11-21T12:12Z failed
  `capture_integration::closing_locality::forwarded_docker_socket_is_not_contacted_or_treated_as_local`
  in the full run (`ConnectionRefused` at the first probe) and passed it
  alone. First cause: the test waited for the forwarder's socket file, which
  exists from `bind` before `listen` accepts. The probe now retries a refused
  connect within the existing 60-second deadline; a refused connect reaches
  nothing, so the barrier is unchanged. With only that fix the test then
  failed once alone, in the first of ten repeated runs, before any daemon had
  started. Second cause: the target listener is nonblocking and on macOS an
  accepted socket inherits that mode, so the sentinel read could return
  `WouldBlock` between the forwarder's connect and its write. A temporary
  300 ms delay before that write reproduced the failure every time; with the
  accepted socket set to blocking the delayed test passed, and the delay was
  removed. On the final source the test passed in 40 repeated runs alone and
  in both full runs. Under the mission rule this test failed in a full run
  and once alone; both failures have an identified cause and a repair.
- Recorded, not reproduced: the recovery lane reported one full-run failure
  of `current_user_ownership::relay_holding_upstream_cannot_change_the_observers_request`
  on the earlier version of that file. The delivered version no longer
  unwraps that read, and the test passed in the runner's check and in every
  run above.
- Not run here, by assignment: the workspace test command, coverage, the
  architecture harness, SwiftPM-backed tests, desktop tests and any browser
  or screen check.

Gaps that the row statuses do not cover: no test runs the resident
`arc-daemon` with `ARC_REAPER=process` on its own interval; foreground
protection is reached only through an injected fault; the lease and protected
conditions are written into the ownership tables by the test because no
production writer exists yet; scheduled (Traffic, Governor) jobs are not
registered with the Reaper.

Unmet prerequisite: the working `av.yaml` and the global `settings.yaml`
return `Operation not permitted`, directly and through `av doctor`. The
tracked `av.yaml` blob declares no container-observation target, but the
configuration in effect cannot be read from this workspace. Whether container
targets are configured, and their health, availability and recent errors, is
therefore not established, and this document does not claim mission
completion.
