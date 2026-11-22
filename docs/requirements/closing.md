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
   `closing_shell` on both production routes. This item is classification
   evidence; the stage-by-stage control of the same compound is
   [AT-TRF-12](#at-trf-12--mixed-command) and
   [DET-016](#det-016--stage-by-stage-permits-for-mixed-commands).
9. **HOST** — `docker build .` and `docker buildx build .` with proven local
   engine/builder endpoints are BUILD controls placed in a Local Container:
   `arcctl classify` and the negotiated `job.status.container.v1` reader name
   the compute location `LOCAL_CONTAINER`; every reader that did not negotiate
   it (`*.locality.v1`, Traffic diagnostics, the desktop) receives the
   HOST_LOCAL projection, and a protocol-1 reader keeps UNKNOWN. Each control
   takes one queue entry and one reservation. For `act -j build`,
   `wsl.exe -- cargo build`, and the fixture `./self-hosted-runner` with no
   proof of their execution endpoint, the expected result is UNKNOWN/OBSERVE.
   The SRS labels
   an actual WSL workload LOCAL_VM and a runner on this machine HOST_LOCAL;
   these unproved negative controls do not complete those broader
   local-detection cases. Evidence: individual `closing_locality` controls;
   the local-endpoint controls are
   `closing_locality::docker_local_listener_controls_use_both_production_routes_and_admission`,
   which is macOS-only.
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
    fixture listener proven to be a local engine is BUILD with compute
    location `LOCAL_CONTAINER` (HOST_LOCAL for a reader that did not negotiate
    it) and receives local admission: one queue entry and one reservation.
    Evidence: the case "act on local engine" of the same macOS-only test.
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
   as a blanket remote bypass; evidence: `closing_shell`. The stage-by-stage
   control of the two named compounds is separate evidence, assessed under
   [AT-TRF-12](#at-trf-12--mixed-command); no live dispatch is claimed.
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
   On `arcctl exec -- bash -c` the MIXED record is the request record and its
   two stages are recorded separately. Classification alone does not establish
   AT-TRF-12; its local-Test control is the separate evidence in
   [AT-TRF-12](#at-trf-12--mixed-command), and no live dispatch is claimed.
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
independent. Classification of either mixed example is not AT-TRF-12's
stage-by-stage acceptance; that acceptance was assessed later, with its own
named tests, in [Phase 0 locality and desktop](#phase-0-locality-and-desktop).

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
`AT-TRF-12` and `AT-SWP-20` kept their existing statuses in that cycle;
AT-TRF-12 was assessed later in
[Phase 0 locality and desktop](#phase-0-locality-and-desktop). The Kubernetes
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
AT-GUI-01, AT-GUI-04, DEC-003, DEC-004, DEC-012, DEC-022, G-02, ADR-002 and
ADR-005 were assessed in the locality and desktop cycle; their named tests are
in [Phase 0 locality and desktop](#phase-0-locality-and-desktop).
NFR-001 to NFR-005, R-004, DET-002 and NFR-022 were assessed in the Phase 1
entry cycle; the measured values, the conditions of each test and the two
rows that hold external items only are in
[Phase 1 entry: assessment](#phase-1-entry-assessment).

### Requirements

- **DET-002** — 1. **EXTERNAL** PowerShell direct command and quoting on Windows; 2. **EXTERNAL** Windows cmd direct command and quoting. Blocked; items 1 and 2 remain, both need a Windows host, and neither was run or claimed.
- **DET-006** — 1. **HOST** the selected remote `gh workflow run` does not occupy a local slot. Complete in the existing row; this does not itself close AT-TRF-02's hook journey.
- **DET-007** — 1. **HOST** an unverified command remains UNKNOWN/OBSERVE and executes without forced admission. Complete in the existing row.
- **DET-008** — 1. **HOST** analysis of a fixture build and poisoned metadata does not execute the workload. Complete in the existing row.
- **SAFE-001** — 1. **HOST** owned process identity is tracked; 2. **HOST** a process of unknown ownership is never automatically killed by the Reaper. Complete.
- **SAFE-007** — 1. **HOST** current-user job and socket ownership checks; 2. **EXTERNAL** another authenticated account's files and processes remain inaccessible. Blocked; only item 2 remains.
- **SAFE-011** — 1. **HOST** genuine daemon failure does not indefinitely prevent an ordinary development command. Complete in the existing row.
- **NFR-001** — 1. **HOST** measure long-run idle daemon CPU against the stated 0.5% target. Complete; `phase0_targets::nfr_001_idle_daemon_cpu_below_half_percent_of_one_core`.
- **NFR-002** — 1. **HOST** measure settled idle daemon RSS against the stated 50 MB target; 2. **EXTERNAL** establish the result on each required platform. Blocked; only item 2 remains. Item 1: `bounded::native_probe_then_full_100000_attempt_workload`.
- **NFR-003** — 1. **HOST** measure bypass-hook P95 against the stated 50 ms target. Complete; `phase0_targets::nfr_003_installed_hook_bypass_p95_overhead_within_50_ms`.
- **NFR-004** — 1. **HOST** measure small local IPC round trips against the stated 20 ms P95 target. Complete; `bounded::native_probe_then_full_100000_attempt_workload`.
- **NFR-005** — 1. **HOST** measure cached ordinary classification against the stated 50 ms P95 target. Complete; `phase0_targets::nfr_005_cached_ordinary_classification_p95_within_50_ms`.
- **NFR-010** — 1. **HOST** forced termination and restart preserve database integrity; 2. **HOST** run the integrity check on the recovered database. Complete.
- **NFR-014** — 1. **HOST** a new non-destructive job executes once when the daemon is unavailable. Complete in the existing row.
- **NFR-022** — 1. **EXTERNAL** after real login, the installed daemon passes its health check; 2. **EXTERNAL** the installed tray passes its health check. Blocked; items 1 and 2 remain, both need an installed login service and tray, and neither was run or claimed.

### Acceptance tests

- **AT-TRF-11** — 1. **HOST** UNKNOWN command passes without a local permit; 2. **HOST** its actual execution and native result are retained. Complete in the existing row.
- **AT-GUI-01** — 1. **HOST** closing the fixture window hides it and leaves the daemon running; 2. **EXTERNAL** installed macOS menu-bar and Windows tray remain available after window close. Blocked; only item 2 remains.
- **AT-GUI-04** — 1. **HOST** About shows the specified icon-only links and Try OMA; 2. **EXTERNAL** each link opens its official destination in the real default browser. Blocked; only item 2 remains.

### Product decisions, goals, architecture and risks

- **DEC-001** — 1. **HOST** the product and switch display the specified Agent Resource Control / ARC identity. Complete.
- **DEC-003** — 1. **HOST** GUI and CLI use the shared Rust daemon; 2. **EXTERNAL** per-user installation and lifecycle on the required platforms. Blocked; only item 2 remains.
- **DEC-004** — 1. **HOST** fixture window close hides without daemon exit; 2. **EXTERNAL** installed macOS menu-bar and Windows tray behavior. Blocked; only item 2 remains.
- **DEC-005** — 1. **HOST** daemon uses Rust/Tokio and SQLite without a persistent Node or LLM service. Complete.
- **DEC-006** — 1. **HOST** desktop uses the declared React 19 and TypeScript 7 stack. Complete.
- **DEC-007** — 1. **HOST** Bun is the declared JS/TS package manager. Complete.
- **DEC-008** — 1. **HOST** desktop uses the declared Tailwind, basecn and Phosphor stack. Complete.
- **DEC-009** — 1. **HOST** Biome is the declared JS/TS formatting and lint tool. Complete.
- **DEC-012** — 1. **HOST** the named local and remote endpoint controls in DET-005; 2. **EXTERNAL** proven remote pod placement and Windows WSL placement. Blocked; only item 2 remains. These cases do not close other endpoint families.
- **DEC-022** — 1. **HOST** specified About icons and CTA are displayed; 2. **EXTERNAL** actual browser opening of their official destinations. Blocked; only item 2 remains.
- **G-02** — 1. **HOST** the named finite remote triggers bypass local admission; 2. **EXTERNAL** remaining placement-dependent scenarios; 3. **HOST** [AT-TRF-03](#at-trf-03--local-docker-build): a local Docker build is placed as Local Container and takes a Build reservation; 4. **HOST** [AT-TRF-12](#at-trf-12--mixed-command): only the local Test stage of the mixed command is admitted and the remote CI trigger uses no local slot. Blocked; only item 2 remains. The SRS names AT-TRF-02, 03, 04 and 12 for this goal (§1.2 and §24); AT-TRF-02 and AT-TRF-04 are already Complete, so items 3 and 4 are the whole remainder of the former open item.
- **ADR-002** — 1. **HOST** separated Rust daemon and React/Tauri GUI; 2. **EXTERNAL** installed cross-platform lifecycle. Blocked; only item 2 remains.
- **ADR-005** — 1. **HOST** named endpoint-based local, remote and UNKNOWN decisions; 2. **EXTERNAL** live pod and WSL placement; 3. **HOST** a local Docker engine is placed as Local Container ([DET-011](#det-011--local-dockerwsl-detection) item 1); 4. **HOST** a local Buildx builder is placed as Local Container (DET-011 item 2); 5. **HOST** stage-by-stage permits for the two named mixed compounds ([DET-016](#det-016--stage-by-stage-permits-for-mixed-commands) items 1 and 2). Blocked; only item 2 remains. Items 3 to 5 are the cases its related IDs DET-011 and DET-016 and the §8.5 table give, and replace the former open item; WSL placement stays in item 2.
- **ADR-011** — 1. **HOST** the declared Bun, TS7, React19, Tailwind4, basecn, Phosphor and Biome stack is maintained. Complete.
- **R-004** — 1. **HOST** macOS admission fallback; 2. **EXTERNAL** Windows and Linux capability/hard-limit behavior. Blocked; only item 2 remains. Item 1: `traffic_cli::measurement::two_real_builds_queue_fifo_and_bypass_remote_and_unknown_while_full`.
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

At the end of this cycle Phase 0 was 18 Complete, 10 Partial, 8 Pending and
4 Blocked of 40; see
[traceability](traceability.md#phase-0-reaper-safety-integration-evidence).
The current counts are under
[Phase 1 entry: assessment](#phase-1-entry-assessment).

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

## Phase 0 locality and desktop shell: finite Phase 2 lists

The replacements for G-02 item 3 and ADR-005 item 3 above name cases that
belong to four Phase 2 rows. Their finite lists are fixed here before any lane
starts; the interfaces and file ownership are in the
[cycle contract](../../.arc-verify/phase-0-locality-desktop/contract.md).
Sources: [locality §8.5](ARC_SRS_2026-11-06.md#85-locality-판정네트워크-요청과-로컬-컴퓨팅의-분리),
[detector §8.7](ARC_SRS_2026-11-06.md#87-detector-요구사항) and
[traffic acceptance §21.2](ARC_SRS_2026-11-06.md#212-gherkin-기반-acceptance-tests--traffic).

This section fixed the obligations before the lanes started. Integration then
assessed every item against a named test; the files, the conditions and the
row outcomes are in
[Phase 0 locality and desktop](#phase-0-locality-and-desktop), and the status
line under each list is that assessment. An item
counts only when the path a user's command takes (the installed hook,
`hook-exec`, `arcctl exec` or `arcctl classify`, then a real daemon) produces
the behaviour with offline stubs; an injected fact proves the classifier only.
No live Docker engine, GitHub dispatch or vendor runtime is run or claimed.

"Proven local endpoint" below means an isolated Docker context and Buildx
builder whose Unix endpoint has a fixture listener the existing socket
inventory proves to be a local engine. Classification never contacts that
socket. The compute location is the negotiated value `LOCAL_CONTAINER`; a
protocol-1 peer or a reader that did not negotiate it receives a valid earlier
record instead.

### AT-TRF-03 — local Docker build

1. **HOST** — `docker buildx build .` with a proven local endpoint is recorded
   BUILD with compute location Local Container on the `arcctl exec` route.
   Test: `local_container_buildx_via_exec`.
2. **HOST** — the same command on the `hook-exec` route. Test:
   `local_container_buildx_via_hook_exec`.
3. **HOST** — the global resource budget is checked: on both routes the build
   takes one BUILD queue entry and reservation; while a separate Build permit
   is held the offline docker stub has zero starts, then runs exactly once and
   releases its permit. Asserted by the two tests above.
4. **HOST** — classification executes nothing and contacts no socket, and
   `arcctl classify` names the location. Test:
   `local_container_classify_contacts_no_socket`.

Status: Complete. Every item has a passing test; all three tests are
macOS-only.

### AT-TRF-12 — mixed command

1. **HOST** — `bun test && gh workflow run ci.yml` with a proven remote CI
   endpoint through the installed hook: the local Test stage is recorded
   TEST/HOST_LOCAL and admitted; while a separate TEST permit is held it has
   zero starts, then runs exactly once and releases its permit; the gh stage
   is recorded REMOTE_CI with no queue entry or reservation and is not delayed
   by a held Build permit; both stage records are linked to the one request.
   Test: `mixed_bun_stages_via_hook`.
2. **HOST** — the same compound through `arcctl exec -- bash -c`. Test:
   `mixed_bun_stages_via_exec`.
3. **HOST** — shell semantics are preserved: a failing test stage prevents the
   gh stage, and arguments, working directory, environment, streams and exit
   status are unchanged. Test: `mixed_failing_test_stage_skips_gh_stage`, with
   the exact comparisons in the two tests above.
4. **HOST** — the command graph is analysed without executing anything. Test:
   `mixed_classify_executes_nothing`.

Status: Complete. Every item has a passing test.

### DET-011 — local Docker/WSL detection

1. **HOST** — `docker build .` on a proven local engine is BUILD with compute
   location Local Container on both production routes and takes a BUILD
   reservation. Tests: `local_container_docker_build_via_exec`,
   `local_container_docker_build_via_hook_exec`.
2. **HOST** — `docker buildx build .` with a proven local context and builder,
   as AT-TRF-03 items 1 to 3. Tests: `local_container_buildx_via_exec`,
   `local_container_buildx_via_hook_exec`.
3. **HOST** — remote, mixed-node, missing or conflicting endpoints keep their
   existing REMOTE_EXEC or UNKNOWN/OBSERVE result and take no local
   reservation (the AT-TRF-04 control). Test:
   `local_container_other_endpoints_unchanged`.
4. **HOST** — detection contacts no socket and runs nothing. Test:
   `local_container_classify_contacts_no_socket`.
5. **HOST** — `act -j build` on a proven local engine shares the reason
   `LOCAL_ENDPOINT_BUILD` and the §8.5 expectation, so it moves with the Docker
   cases. Test: `closing_locality::docker_local_listener_controls_use_both_production_routes_and_admission`,
   which is macOS-only (`#[cfg(target_os = "macos")]`, it injects the socket
   inventory through `DYLD_INSERT_LIBRARIES`); on another host this item has no
   executed evidence.
6. **HOST** — the location is additive vocabulary: a protocol-1 peer keeps
   protocol-1 vocabulary and every reader that did not negotiate receives the
   HOST_LOCAL projection. Test: `local_container_projection_for_earlier_peers`
   (daemon replies: protocol-1 status, locality status, the three Traffic
   diagnostics operations and an earlier-peer enqueue). For the desktop reader
   two further tests continue from the same record:
   `registered_traffic_diagnostics_return_the_local_container_projection`
   (`apps/desktop/src-tauri/tests/registered_commands_support/traffic_diagnostics.rs`)
   and "local container build diagnostics are read as the HOST_LOCAL
   projection and LOCAL_CONTAINER is rejected"
   (`apps/desktop/tests/traffic-scripts-contract.test.ts`). These are three
   tests joined by one identical record (daemon reply, registered command,
   TypeScript reader), not one end-to-end test.
7. **EXTERNAL** — `wsl.exe -- cargo build` with actual LOCAL_VM placement
   needs a Windows host with WSL (DET-005 item 15). Not selected or claimed.

Status: Blocked; only item 7 remains. Items 1, 2, 4, 5 and 6 have executed
evidence on macOS only.

The Sweeper's Docker switch condition (SRS §12.15, `sweeper.docker`) cites
DET-011 but belongs to the Sweeper rows and is not an item here.

### DET-016 — stage-by-stage permits for mixed commands

1. **HOST** — `bun test && gh workflow run ci.yml`: the stage-by-stage permits
   of AT-TRF-12 items 1 and 2 on both routes. Tests:
   `mixed_bun_stages_via_hook`, `mixed_bun_stages_via_exec`.
2. **HOST** — `make test && gh workflow run ci.yml` with the literal Makefile
   recipe `cargo test` and a proven remote CI endpoint: the same on both
   routes. Tests: `mixed_make_stages_via_hook`, `mixed_make_stages_via_exec`.
3. **HOST** — a failing test stage prevents the gh stage and the compound's
   exit status is the shell's. Test: `mixed_failing_test_stage_skips_gh_stage`.
4. **HOST** — a compound that cannot be separated safely (command
   substitution, redirection, a dynamic target, a startup-altering nested
   shell) keeps today's MIXED or UNKNOWN/OBSERVE pass-through with no stage
   record. Test: `mixed_unseparable_compounds_pass_through`.
5. **HOST** — classification executes nothing. Test:
   `mixed_classify_executes_nothing`.

Status: Complete. Every item has a passing test.

Exactly these two compounds are separated. Every other source, including
`sh -c` and `zsh -c` on the `arcctl exec` route, keeps today's result.

## Phase 0 locality and desktop

This cycle assessed thirteen rows against their numbered cases: nine Phase 0
rows from the [remaining inventories](#remaining-phase-0-inventories)
(DEC-003, DEC-004, ADR-002, AT-GUI-01, AT-GUI-04, DEC-022, DEC-012, G-02 and
ADR-005) and the four Phase 2 rows whose
[finite lists](#phase-0-locality-and-desktop-shell-finite-phase-2-lists)
replaced the two former open items (AT-TRF-03, AT-TRF-12, DET-011 and
DET-016). The interfaces are in the
[cycle contract](../../.arc-verify/phase-0-locality-desktop/contract.md).
Sources: [product decisions §0](ARC_SRS_2026-11-06.md#0-최종적으로-확정한-제품-결정),
[product goals §1.2](ARC_SRS_2026-11-06.md#12-제품-목표),
[architecture decisions §4.4](ARC_SRS_2026-11-06.md#44-핵심-architecture-decision-records),
[locality §8.5](ARC_SRS_2026-11-06.md#85-locality-판정네트워크-요청과-로컬-컴퓨팅의-분리)
and the linked acceptance tests.

### Where the tests are and what limits them

- **Desktop shell**, under `apps/desktop/src-tauri/tests`: `lifecycle.rs`
  (target `lifecycle`), and `registered_commands_support/input_validation.rs`
  and `registered_commands_support/traffic_diagnostics.rs` (target
  `registered_commands`). Each builds the application through
  `configure_with_ports` with the production command handlers on Tauri's mock
  runtime. `apps/desktop/tests/credits.test.ts` and
  `apps/desktop/tests/traffic-scripts-contract.test.ts` are Bun tests of the
  React About view and of the TypeScript reader.
- **Mixed stages**, under
  `crates/arc-daemon/tests/managed_exec/capture_integration` (target
  `managed_exec`): `closing_exec/stages.rs` for the installed-hook route and
  `closing_shell/stages.rs` for `arcctl exec -- bash -c`. Both run
  `closing_shell/stages/admission.rs` with the helpers in
  `closing_shell/stages/harness.rs`.
- **Local container**, same target: `closing_locality/local_container.rs` with
  `local_container/fixture.rs` and `local_container/readers.rs`, and
  `closing_locality/docker_controls.rs`.

A test that starts a binary runs the built `arcctl` and the real `arc-daemon`
with a cleared environment, a temporary HOME and explicit socket and database
paths, against offline stubs. The conditions below limit what the evidence
shows:

- **macOS-only local proof.** The six local-proof container tests (every
  `local_container_*` test except `local_container_other_endpoints_unchanged`)
  and the Docker controls test are compiled only on macOS: the listener
  inventory that proves a local engine is injected through
  `DYLD_INSERT_LIBRARIES`. They ran on this macOS host; on another host those
  items have no executed evidence.
- **Hook route for the container literals.** The two `*_via_hook_exec` tests
  first assert that the installed Claude hook returns exactly
  `'<arcctl>' hook-exec -- <literal>`, and then run that `hook-exec` command
  directly. The vendor shell is not in between, because macOS removes
  `DYLD_INSERT_LIBRARIES` for system shells and with it the injected inventory.
- **Four-core Traffic budget.** The four `mixed_*_stages_via_*` tests start
  the daemon with `ARC_TRAFFIC_CPU_BUDGET_MILLIS=4000`. With the default
  two-core budget one held Build reservation is the whole budget, so the Test
  stage would also wait for the build; with four cores only the TEST window
  delays it. On the hook route the rewrite returned by the installed Claude
  hook is run by `/bin/sh` (Bun compound) and `/bin/zsh` (Make compound). The
  Cursor adapter shares the rewrite and has no installed-hook stage test.
- **Injected close request.** The desktop test calls
  `lifecycle::close_requested`, the adapter that `on_window_event` installs,
  with a counting prevention port, because Tauri's `CloseRequestApi` has no
  public constructor. No native close event is delivered.
- **Self-reported daemon identity is macOS-only.** In the same test, the step
  in which the daemon states its own PID and start time (the registered
  `meter_snapshot` and `arcctl meter snapshot --json`) is compiled on macOS
  only. On another Unix host the test shows the unchanged process identity,
  `daemon_health` and `arcctl status`.

### Desktop shell rows

**DEC-003** — public shared local daemon.

1. **HOST** — GUI and CLI use the shared Rust daemon.
   `close_request_hides_main_and_same_daemon_serves_bridge_and_cli`
   (`lifecycle.rs`): one private real daemon answers the registered
   `daemon_health` command and the built `arcctl status` from the same socket,
   and (macOS) names the same PID and start time to both; a job started with
   `arcctl exec` is read through the registered `job_by_id` as RUNNING, is
   first in `recent_jobs`, and reads COMPLETED with exit 17 after exactly one
   launch. `registered_app_reads_private_real_daemon` (same file) is the seam
   test for the registered application reading that daemon.
2. **EXTERNAL** — per-user installation and lifecycle on the required
   platforms. Not selected or claimed.

**DEC-004** — window close hides, daemon keeps running.

1. **HOST** — fixture window close hides without daemon exit. The same test:
   after `open_dashboard`, the close request on `main` is prevented exactly
   once; the shell records `show, unminimize, focus, hide`; the same `main`
   window object remains the only window, with no exit and no bridge shutdown;
   the daemon's process identity is unchanged before and after, and at the end
   of the test. `close_request_hides_existing_window` (same file) is the
   earlier accepted test of prevent-and-hide without a daemon.
2. **EXTERNAL** — installed macOS menu-bar and Windows tray behavior. Not
   selected or claimed.

**ADR-002** — Rust per-user daemon with a separate React/Tauri GUI.

1. **HOST** — separated Rust daemon and React/Tauri GUI. The same test: the
   daemon is its own `arc-daemon` process, reached by the desktop only through
   the registered bridge commands; hiding the GUI window changes neither the
   daemon's identity nor its answers to the CLI, and the CLI's job is visible
   to the GUI only through that daemon.
2. **EXTERNAL** — installed cross-platform lifecycle. Not selected or claimed.

**AT-GUI-01** — menu bar / tray.

1. **HOST** — closing the fixture window hides it and leaves the daemon
   running. `close_request_hides_main_and_same_daemon_serves_bridge_and_cli`,
   as under DEC-004 item 1 and DEC-003 item 1.
2. **EXTERNAL** — installed macOS menu-bar and Windows tray remain available
   after window close. Not selected or claimed.

**AT-GUI-04** — credits links.

1. **HOST** — About shows the specified icon-only links and Try OMA.
   `apps/desktop/tests/credits.test.ts`, test "exact SRS identity and three
   destinations use only the fixed native opener": GitHub and LinkedIn are
   links with an accessible name and one `aria-hidden` SVG each, equal to the
   glyph Phosphor's `GithubLogoIcon` and `LinkedinLogoIcon` render, with no
   visible text; the Try OMA call to action is present; the three clicks reach
   the opener with the three fixed destinations.
   `registered_opener_accepts_exactly_the_three_enum_destinations`
   (`registered_commands_support/input_validation.rs`): the registered
   `open_credit_link` command accepts exactly `github`, `linkedin` and `oma`
   with the three fixed official URLs, rejects five other values with
   `invalid_request`, and the opener plugin's own command is refused.
2. **EXTERNAL** — each link opens its official destination in the real default
   browser. Not selected or claimed; the opener port is a recording fake.

**DEC-022** — Credit/About.

1. **HOST** — specified About icons and CTA are displayed. The same
   `credits.test.ts` test.
2. **EXTERNAL** — actual browser opening of their official destinations. Not
   selected or claimed.

### Locality rows

Test names below are in `capture_integration::closing_locality`,
`closing_shell` or `closing_exec` as named in
[DET-005](#det-005--remote-ci-and-ssh-exclusion) and in the
[finite Phase 2 lists](#phase-0-locality-and-desktop-shell-finite-phase-2-lists).

**DEC-012** — the place where the computation runs decides, not the command.

1. **HOST** — the named local and remote endpoint controls in DET-005, set
   from the existing tests of that list:
   - remote, items 1 to 5:
     `closing_exec::installed_claude_hook_executes_same_remote_child_with_held_build_permit`,
     `closing_locality::proven_gh_remote_is_the_same_inspected_and_executed_child`,
     `closing_locality::proven_remote_cases_never_classify_local_on_either_route`,
     `closing_locality::gitlab_explicit_public_endpoint_and_unresolved_controls_on_both_routes`
     and
     `closing_locality::remote_docker_and_ssh_execute_while_local_build_permit_remains_held`;
   - unresolved, items 6, 7 and 11:
     `closing_locality::kubectl_without_placement_proof_stays_observed_without_starting_a_probe`,
     `closing_locality::ambiguous_and_local_controls_are_counted_separately`
     and
     `closing_locality::explicit_prod_context_takes_precedence_over_docker_host_on_both_routes`;
   - local, items 9, 13 and 14:
     `closing_locality::docker_local_listener_controls_use_both_production_routes_and_admission`
     (macOS-only; restated above for `LOCAL_CONTAINER`) and
     `closing_locality::integration::same_pc_runner_starts_a_recorded_locally_admitted_build_child`;
   - mixed, items 8 and 10:
     `closing_shell::actual::bash_and_zsh_execute_the_same_direct_pipeline_compound_and_mixed_commands`,
     `closing_shell::actual::makefile_selection_and_post_capture_changes_fail_closed_on_both_routes`
     and
     `closing_locality::pnpm_deploy_literal_build_and_remote_ci_is_mixed_on_both_routes`.
2. **EXTERNAL** — proven remote pod placement and Windows WSL placement
   (DET-005 items 12 and 15). Not selected or claimed.

**G-02** — only local execution is scheduled.

1. **HOST** — the named finite remote triggers bypass local admission: the
   five remote tests listed under DEC-012 item 1. They are the evidence
   already accepted for [SAFE-008](#safe-008--remote-work-is-not-forcibly-scheduled-locally)
   items 1 to 3 (no local queue entry or reservation), and two of them run
   while a separate local Build permit stays held (AT-TRF-02 and AT-TRF-04).
2. **EXTERNAL** — remaining placement-dependent scenarios. Not selected or
   claimed.
3. **HOST** — AT-TRF-03: `local_container_buildx_via_exec` and
   `local_container_buildx_via_hook_exec` (`local_container.rs`).
4. **HOST** — AT-TRF-12: `mixed_bun_stages_via_hook`
   (`closing_exec/stages.rs`) and `mixed_bun_stages_via_exec`
   (`closing_shell/stages.rs`).

This is the finite set the SRS names for the goal (AT-TRF-02, 03, 04 and 12).
It is not a measurement of every official remote trigger, and no live GitHub
dispatch ran.

**ADR-005** — the actual execution location decides the Traffic target.

1. **HOST** — named endpoint-based local, remote and UNKNOWN decisions: the
   remote, unresolved and local tests listed under DEC-012 item 1.
2. **EXTERNAL** — live pod and WSL placement. Not selected or claimed.
3. **HOST** — a local Docker engine is placed as Local Container:
   `local_container_docker_build_via_exec` and
   `local_container_docker_build_via_hook_exec`.
4. **HOST** — a local Buildx builder is placed as Local Container:
   `local_container_buildx_via_exec` and
   `local_container_buildx_via_hook_exec`.
5. **HOST** — stage-by-stage permits for the two named mixed compounds:
   `mixed_bun_stages_via_hook`, `mixed_make_stages_via_hook`,
   `mixed_bun_stages_via_exec` and `mixed_make_stages_via_exec`.

### What the Phase 2 tests show

The four container build tests each run one build on one route with an
isolated `DOCKER_CONFIG` and `BUILDX_CONFIG`: `arcctl classify` names
`LOCAL_CONTAINER`; while a separate `arcctl exec -- cargo build`, whose
`cargo` is an offline stub, holds the Build permit, the docker stub has zero
starts and the build has a QUEUED entry with no reservation; after the release the stub runs exactly once, the stored
location is `LOCAL_CONTAINER`, the one reservation has the same CPU and memory
amounts as the held build and is released, and the Docker socket has zero
connections. `local_container_other_endpoints_unchanged` keeps REMOTE_EXEC or
UNKNOWN on both routes with no admission for a remote context, a remote
builder, a mixed-node builder, a missing endpoint, and an engine and builder
that conflict. Selectors that conflict on the command line are UNKNOWN on
`arcctl exec`; the installed hook leaves that command alone and `hook-exec`
rejects it without starting a child.

The four stage tests each run one compound on one route: the Test stage is
recorded TEST/HOST_LOCAL and QUEUED with zero stub starts while separate TEST
and Build permits are held; after the TEST permit is released it starts
exactly once with an ACTIVE permit and ends COMPLETED/RELEASED; the gh stage
is recorded REMOTE_CI/REMOTE with no queue entry, reservation or schedule
while the Build permit is still ACTIVE; both stage records are read back in
order from the one request. The exit status, stdout, child stderr, arguments,
working directory, stdin and environment equal a native run.
`mixed_stages_run_once_without_a_daemon` (`closing_shell/stages.rs`) adds
that with no daemon each stage still runs exactly once.

**Queue notice.** A waiting stage or build writes one notice to its own
stderr, which on the hook route is the vendor shell's:
`QUEUED`, `Waiting to start.`, `Reference: <job id>`. The four stage tests
require the reference to be the waiting TEST stage's job id and exclude the
notice from the child-stream comparison; the four container build tests
require it to be the waiting build's id. The gh stage writes none. The
reference line was added to this admission route in this integration
(`crates/arc-cli/src/exec/governor/admission.rs`); the same route also serves
directly executed Cargo, Bun, Python and native-tool waits, whose notices now
carry it too.

**Desktop.** The desktop does not distinguish container builds. It accepts
the HOST_LOCAL projection of a local container build, a record that is
BUILD/HOST_LOCAL like any other local build and is shown as one, and it never
accepts `LOCAL_CONTAINER`: the TypeScript reader rejects a record that carries
it. The evidence is the chain of three tests named under
[DET-011](#det-011--local-dockerwsl-detection) item 6, joined by one
identical record; it is not one end-to-end test, and no test renders a
container build in the dashboard.

### Row outcomes

| Row | Status | Remaining item |
| --- | --- | --- |
| DEC-003 | Blocked | External per-user installation and lifecycle (item 2). |
| DEC-004 | Blocked | External installed menu-bar and tray behavior (item 2). |
| ADR-002 | Blocked | External installed cross-platform lifecycle (item 2). |
| AT-GUI-01 | Blocked | External installed menu-bar and tray (item 2). |
| AT-GUI-04 | Blocked | External real default-browser opening (item 2). |
| DEC-022 | Blocked | External real browser opening (item 2). |
| DEC-012 | Blocked | External remote pod and Windows WSL placement (item 2). |
| G-02 | Blocked | External placement-dependent scenarios (item 2). |
| ADR-005 | Blocked | External live pod and WSL placement (item 2). |
| AT-TRF-03 | Complete | None in the numbered HOST list. |
| AT-TRF-12 | Complete | None in the numbered HOST list. |
| DET-016 | Complete | None in the numbered HOST list. |
| DET-011 | Blocked | External Windows WSL placement (item 7). |

At the end of this cycle Phase 0 was 18 Complete, 1 Partial, 8 Pending and
13 Blocked of 40; R-018 was the remaining Partial row and was not selected.
Phase 2 was 4 Complete, 8 Partial, 30 Pending and 1 Blocked of 43. Phases 1,
3, 4 and 5 were unchanged: 13/46/12/0 of 71, 0/5/103/0 of 108, 0/15/29/0 of
44 and 0/1/9/0 of 10 in the same order. See
[traceability](traceability.md#phase-0-locality-and-desktop-integration-evidence).
The current counts are under
[Phase 1 entry: assessment](#phase-1-entry-assessment). These are row counts; no phase
is declared finished.

Not claimed by any row: a live Docker engine, a live GitHub dispatch, a live
vendor runtime, a native tray click, an installed menu bar, a real browser or
a screen check. No customer or revenue effect is claimed. No EXTERNAL item
was selected, run or claimed.

### Integration checks and boundary

This integration inspected Actor snapshot
`5faacb7876621e232ca75077c7bd3400111b2fdd`, tree
`0cf6af957ffe732899b86e067caa82f07ae142a0`, which contains the shared seam,
the three lane deliveries and the desktop reader vocabulary; the accepted
committed baseline is `aabf278d0d29f07fa9514d3ff8df4cb52742a7dc`, tree
`a2f228e88a90465c619430f7f27134547e358cad`. The integration edits are
uncommitted on that snapshot: the queue-notice reference and the test helpers
that read it, `docs/managed-exec.md`, the cycle contract, this document and
the traceability matrix. These are focused local checks using `oma-backend`,
not the runner's final verification. The independent QA report
(`.arc-verify/phase-0-locality-desktop/qa.md`) is written separately for its
own snapshot and is not cited here.

- Passed on the final source: `cargo fmt --all -- --check`; `cargo clippy
  --workspace --all-targets -- -D warnings`; `cargo test -p arc-daemon --test
  managed_exec`, 181 of 181 with none ignored; `cargo test -p arc-cli`, all 13
  targets (202 unit tests, `binary` 66, `governor` 34, `classification` 7);
  `cargo test -p arc-detector -p arc-ipc -p arc-storage`, every target; the
  `arc-daemon` unit tests (143, and 128 for each of the three fixture
  daemons) with `bounded`, `classification`, `db_worker_unit`, `governor`,
  `memory_pressure`, `resident`, `startup`, `traffic_bun_scripts`,
  `traffic_cli`, `traffic_python_tests`, `transport_unit`, `worker_runtime`
  and `legacy_preparation`; and the `arc-desktop` unit tests with
  `lifecycle` (10), `registered_commands` (31), `daemon_bridge` (14),
  `governor`, `job_meter_acceptance` (35), `memory_pressure`,
  `traffic_bun_scripts` and `traffic_python_tests`. No target reported a
  failed or ignored test.
- Passed: `bun test` on `apps/desktop/tests/credits.test.ts`,
  `apps/desktop/tests/traffic-scripts-contract.test.ts`,
  `scripts/requirements/traceability-parser.test.ts` and
  `scripts/requirements/traceability.test.ts`, 44 pass and 0 fail;
  `bun scripts/requirements/traceability.ts` reports the matrix valid with
  the counts above.
- No test failed in a combined run and passed alone in these checks, so there
  is no such result to record.
- Not run here, by assignment: the workspace test command, coverage, the
  architecture harness, any browser or screen check, and the SwiftPM-backed
  `traffic_native_tools` targets of `arc-daemon` and `arc-desktop`. The
  daemon target reads the queue notice only by checking that stderr contains
  `QUEUED` and `Waiting to start.`
  (`crates/arc-daemon/tests/traffic_native_tools/witnesses.rs`), which the
  added reference line still satisfies; that is a reading of the source, not
  an execution.
- **The 60-second rule is not met for `managed_exec`.** The binary took 112 s
  for its 163 tests before this mission, 134.79 s in the runner's check after
  the lanes and 137.02 s for 181 tests here. The mission's new test modules
  were not moved to their own target: they use the fixtures and helpers of
  their mod parents (`capture_integration.rs`, `closing_exec.rs`,
  `closing_shell.rs`, `closing_locality.rs`), so a second target would need
  those parents compiled into it, running their existing tests twice, or a
  refactor of the shared helpers. That is more than a manifest entry and
  mod-parent lines. No test was ignored or removed. No other binary run here
  exceeded 60 s; the longest were `traffic_cli` at 47.01 s and the desktop
  `daemon_bridge` at 40.91 s.

Gaps that the row statuses do not cover:

- No installed-hook stage test uses the Cursor adapter; it shares the rewrite
  with the Claude adapter.
- The hook rewrites the two compounds without checking the endpoint or the
  Makefile recipe; each stage is classified on its own when it runs. The
  hook-route case with an unproven remote endpoint is not exercised.
- With no daemon, the `arcctl exec` route writes one outage notice per stage
  and one for the request; the test asserts their presence, not their count.
- A shell alias or function named `bun`, `make`, `gh` or `docker` is bypassed
  by a stage or container rewrite, as documented in
  [managed execution](../managed-exec.md#bounded-hook-behavior).
- Job rows stored before this cycle keep HOST_LOCAL; there is no backfill, so
  the negotiated reader returns HOST_LOCAL for them.
- The desktop's Rust bridge forwards whatever the diagnostics decoder accepts.
  The daemon projects every diagnostics reply, so `LOCAL_CONTAINER` does not
  reach it today; if it did, only the TypeScript reader would reject it.
- The non-macOS configuration of the container test modules was not compiled
  on this host.

Unmet prerequisite: the working `av.yaml` returns `Operation not permitted`
from this workspace, so the runner configuration in effect, and whether any
container targets are configured and healthy, is not established here. This
document does not claim mission completion; the runner's four commands and
the final review decide that.

## Phase 1 entry: Traffic journey lists

These are the first Phase 1 closing lists. They fix the finite cases of three
acceptance rows before any lane starts; the interfaces, the lane test names
and the file ownership are in the
[cycle contract](../../.arc-verify/phase-1-entry/contract.md). Sources:
[fairness §9.6](ARC_SRS_2026-11-06.md#96-공정성-정책),
[nested jobs §9.7](ARC_SRS_2026-11-06.md#97-중첩-작업과-이중-permit-방지),
[hook timeout §9.8](ARC_SRS_2026-11-06.md#98-hook-timeout과-비동기-큐-문제) and
[traffic acceptance §21.2](ARC_SRS_2026-11-06.md#212-gherkin-기반-acceptance-tests--traffic),
SRS lines 2657-2665, 2667-2675 and 2687-2695.

The rules of the lists above apply. An item counts only when the path a
user's command takes (the installed hook, `hook-exec` or `arcctl exec`, then a
real daemon) produces the behaviour with offline stubs; an injected fact
proves the classifier only. As in [AT-TRF-02](#at-trf-02--agent-triggered-github-actions-bypass),
the installed hook fed a Claude Code PreToolUse payload derived from the
versioned vendor contract stands for the vendor, and the command the hook
returns is the one executed. The hook manages exactly the whole commands
`cargo test` and `bun run test` in addition to today's literals; a Test
command with an argument, such as `cargo test -p x`, is outside these lists
and keeps today's pass-through.

Every case below is HOST: the SRS text of these three tests forces no
EXTERNAL case. This section records obligations only. Each item is assessed
against its named passing test in
[Phase 1 entry: assessment](#phase-1-entry-assessment).

### AT-TRF-05 — nested execution without a second permit

Source: [AT-TRF-05](ARC_SRS_2026-11-06.md#at-trf-05-중첩-실행-이중-permit-방지),
§9.7 and §9.8. Root literal `bun run test`, whose package script is `vitest`,
an offline stub; nested literal `cargo test`. Lane test:
`at_trf_05_nested_hook_test_shares_the_root_test_permit`.

1. **HOST** — with a real daemon and TEST window 1, the agent command
   `bun run test` passes through the installed hook, the returned command is
   the one executed, and it holds exactly one TEST permit.
2. **HOST** — its `vitest` child calls the ARC hook again: it executes the
   command the installed hook returns for the nested literal `cargo test`.
3. **HOST** — that nested execution is recorded on the existing job lineage:
   the daemon attributes it to the root's permit from the job identity and
   process ancestry it verifies, not from an environment value alone (§9.7).
4. **HOST** — it requests no new permit and never waits on itself: TEST
   permit high-water is 1, no QUEUED notice is written, and the nested
   command runs exactly once.
5. **HOST** — the one permit is the root's: it is released only after the
   root exits (§9.7, the same permit).
6. **HOST** — for `bun run test` and for the nested `cargo test`, the hook
   only replaces the command and returns without waiting in a queue (§9.8):
   every other field of the tool input is unchanged, no approval decision is
   emitted, and the nested command's arguments, working directory,
   environment, streams and exit status are unchanged.

### AT-TRF-06 — fail-open

Source: [AT-TRF-06](ARC_SRS_2026-11-06.md#at-trf-06-fail-open) and §9.8.
Literal `cargo test`. Lane test:
`at_trf_06_installed_hook_cargo_test_fails_open_exactly_once`.

1. **HOST** — with no daemon reachable, a Claude Code PreToolUse payload for
   the literal `cargo test` passes through the installed hook, and the
   returned command is the one executed with an offline cargo stub.
2. **HOST** — the internal error is recorded: exactly one ARC notice,
   `TRACKING_UNAVAILABLE`, is written to the stderr of the executed command
   and nothing is added to its stdout (§9.8, the diagnostic channel).
3. **HOST** — the original command is not blocked: the stub runs exactly once
   with unchanged arguments, working directory, environment, streams and exit
   status.
4. **HOST** — no destructive GC runs: no Reaper or GC action is started,
   recorded or signalled.
5. **HOST** — a failure of the hook itself blocks nothing: when the hook
   cannot produce its rewrite it emits no decision and the vendor runs the
   literal `cargo test` unchanged.

### AT-TRF-08 — fairness between projects

Source: [AT-TRF-08](ARC_SRS_2026-11-06.md#at-trf-08-공정성) and §9.6. Lane
test: `at_trf_08_project_b_is_granted_after_at_most_one_project_a_job`.

1. **HOST** — one coordinator holds both projects' requests in one queue:
   with built `arcctl exec`, a real daemon, offline stubs and Build window 1,
   while a separate Build permit is held, project A queues 100 Build requests
   and then project B queues 1, each enqueue confirmed from daemon state. The
   project of each job is recorded without user input, as an opaque key
   derived locally from the working directory without running a tool.
2. **HOST** — project B is not starved: as the slot frees one job at a time
   after the release, at most one project A job is granted between B's
   enqueue and B's grant. Every job runs exactly once or is cancelled with
   zero starts, and all permits are released.
3. **HOST** — the decision reason is recorded: project B's record carries the
   fairness reason, readable through SQLite and through `arcctl job
   diagnostics`.

The other §9.6 statements (interactive priority, fairness between vendors,
short-job preference with aging, bounded backfilling) are not cases of
AT-TRF-08; they belong to TRF-013 and its related rows, which this cycle did
not select. Two conditions hold for the items above and are checked by
existing tests rather than counted as cases: a request without a project key
is ordered exactly as today, and order within one project stays first in,
first out.

Not a case of any list above and not claimed: a live vendor session, and a
queued wait measured against a vendor's tool timeout (§9.8). In AT-TRF-05 the
nested command does not wait, and in AT-TRF-06 there is no daemon to queue on.

## Phase 1 entry: assessment

This cycle assessed eight Phase 0 rows of the
[inventories above](#remaining-phase-0-inventories) (NFR-001 to NFR-005,
R-004, DET-002 and NFR-022) and the three acceptance rows of the
[Traffic journey lists](#phase-1-entry-traffic-journey-lists) (AT-TRF-05,
AT-TRF-06 and AT-TRF-08), and wrote a closing list for every other Phase 1
row under [Remaining Phase 1 inventories](#remaining-phase-1-inventories).
The decisions, the fixed test names and the file ownership are in the
[cycle contract](../../.arc-verify/phase-1-entry/contract.md).

The tests named in the next two subsections are `arc-daemon` test targets.
Each starts built binaries (the real `arc-daemon`, built `arcctl`, the hook as
`arcctl hooks install` writes it) with a cleared environment, a temporary HOME
and explicit socket and database paths, and uses offline stubs for Cargo, Bun
and Vitest. The one exception is the R-004 test, which predates this cycle and
inherits the runner's environment with a real Cargo.

### Performance targets (NFR-001 to NFR-005)

Each target is asserted at the SRS number by one named test. The values below
were printed by one run of this integration: `cargo test -p arc-daemon --test
phase0_targets -- --nocapture` once, and the named `bounded` test once. They
are from one macOS host, fixture workload (Apple M1 Pro, 8 cores, Darwin
27.2.0 arm64, debug build). They are measurements of this run, not benchmarks
or field results.

| Row | Bound the test asserts | Measured in this run | Samples and window |
| --- | --- | --- | --- |
| NFR-001 | idle daemon CPU below 0.5% of one core | 0.021% (6.29 ms of CPU) | one 30.0 s window after a 3 s settle, holding 2 idle Meter ticks (15 s cadence) |
| NFR-002 | settled idle RSS below 50 MB | 8.04 MB maximum; lifetime peak 9.85 MB | 100 samples in a 10 s window after a 5 s settle |
| NFR-003 | P95(hook) minus P95(reference) at most 50 ms | hook P95 10.34 ms absolute, reference P95 7.96 ms, difference 2.39 ms | 300 hook, 300 reference and 300 bare-spawn runs, interleaved |
| NFR-004 | small IPC round trip P95 at most 20 ms | P95 0.080 ms (P50 0.066 ms, maximum 0.246 ms) | 10,000 exchanges after 200 warm-up |
| NFR-005 | classification P95 at most 50 ms | P95 0.094 ms (`job.classify`) and 0.124 ms (`job.classify.locality.v1`) | 1,000 round trips per request form after 200 warm-up, on one connection |

Tests: NFR-001 `phase0_targets::nfr_001_idle_daemon_cpu_below_half_percent_of_one_core`;
NFR-003 `phase0_targets::nfr_003_installed_hook_bypass_p95_overhead_within_50_ms`;
NFR-005 `phase0_targets::nfr_005_cached_ordinary_classification_p95_within_50_ms`;
NFR-002 and NFR-004 `bounded::native_probe_then_full_100000_attempt_workload`,
an existing test named as evidence, not duplicated.

- **NFR-001.** The numerator is the daemon's user plus system CPU time from
  the OS (`proc_pidinfo`, converted with the Mach timebase), so host load
  cannot inflate it. The test first checks the reader against a child that
  burns CPU: 293.19 ms from the counters against 293.20 ms from `wait4`, and
  it requires agreement within 10%. It also requires at least one Meter tick
  inside the window with the Meter in idle mode. The window is 30 s. The SRS
  says long-run average; no longer window was measured.
- **NFR-003.** The payload is the recorded Claude Code 2.1.327 PreToolUse
  event carrying `git status`, which the hook passes through. The reference is
  a no-op (`/usr/bin/true`) started through the same `/bin/sh -c` as the
  installed hook command, so the difference is what the hook adds to starting
  a process that way. The absolute hook P95 in this run, 10.34 ms, is below
  50 ms. Against a bare no-op spawn (P95 1.89 ms) the difference is 8.45 ms;
  that figure is printed, not asserted. The reference is fixed in contract
  D11; if the bare spawn were the intended reference, the asserted bound
  would lack the margin described below and the row would be Partial.
- **NFR-005.** The timed requests are classification round trips for the
  ordinary local command `cargo build` on one connection to the real daemon.
  Three untimed runs of built `arcctl classify --json "cargo build"` against
  the same daemon come first and prove the path.
- **Margins.** An elapsed-time bound is kept only at five times the worst
  value of five consecutive runs made while another test binary ran. Those
  runs are lane A's and were not repeated here: worst difference 5.4 ms for
  NFR-003 (9.3 times), worst P95 0.115 ms for NFR-004 (174 times) and
  0.304 ms for NFR-005 (164 times). In the same loaded runs lane A saw an
  absolute hook P95 of 34.3 ms and a bare-spawn difference of 27.2 ms: both
  below 50 ms, neither with a fivefold margin, and neither asserted. NFR-001
  and NFR-002 are not elapsed-time bounds.

**R-004.** Item 1, the macOS admission fallback, is
`traffic_cli::measurement::two_real_builds_queue_fifo_and_bypass_remote_and_unknown_while_full`
(macOS-only): macOS gives no hard cap to enforce, and admission holds the
second real `cargo build` at zero launches, with a QUEUED entry, until the
first releases. Item 2, Windows and Linux capability and hard-limit
behaviour, is external and was not run.

**DET-002 and NFR-022.** Both rows hold external items only: PowerShell and
Windows cmd on a Windows host, and the installed daemon and tray after a real
login. They are Blocked on those items. Nothing was run for them and nothing
is claimed.

### Agent Test journey and project fairness

The installed hook, fed a Claude Code PreToolUse payload derived from the
recorded 2.1.327 event, stands for the vendor, and the command it returns is
the one executed. No live vendor session ran.

**AT-TRF-05**, items 1 to 6:
`agent_test_journey::at_trf_05_nested_hook_test_shares_the_root_test_permit`.
With a real daemon and the default TEST window of 1, `bun run test` goes
through the installed hook and is recorded TEST/`BUN_SCRIPT_TEST`, RUNNING,
with one ACTIVE permit (item 1). Its `vitest` stub passes a `cargo test` event
through the installed hook and executes what it returns (item 2). The nested
entry is TEST/`CARGO_TEST` with the root's job ID and no permit row, the
diagnostics read `reuse`/`existing_reservation`, and the nested tool's
environment carries no lineage value, so ancestry attributed it (item 3).
Held permits peak at 1, stderr equals the run without a hook (no QUEUED
notice) and the nested stub starts once (item 4). The permit is ACTIVE while
the root lives after the nested exit and RELEASED after the root exits
(item 5). Each hook reply changes only the command and carries no permission
decision; arguments, working directory, standard input, streams and exit
status equal the direct run (item 6).

**AT-TRF-06**, items 1 to 5:
`agent_test_journey::at_trf_06_installed_hook_cargo_test_fails_open_exactly_once`.
With the daemon stopped, the returned command for `cargo test` is executed
(item 1). Stderr is the three-line `TRACKING_UNAVAILABLE` notice followed by
the stub's bytes, and stdout is the stub's (item 2). The stub runs once with
the same arguments, working directory, standard input, environment and exit
status 17 as a direct run (item 3). No socket appears, the database files
and `runtime.json` are byte-identical afterwards, ARC leaves nothing in its
runtime directory and a bystander process of the same user stays alive
(item 4, shown by absence). A truncated event, an oversized event and a
removed hook executable each produce no decision, and the literal then runs
unchanged (item 5).

**AT-TRF-08**, items 1 to 3:
`project_fairness::at_trf_08_project_b_is_granted_after_at_most_one_project_a_job`.
While a separate Build permit is held, project A queues 100 builds from two
of its directories and project B then queues one; each enqueue is confirmed
QUEUED from SQLite, and the stored keys are three 64-hex digests, one for all
of A (item 1). After the release at most one A build is granted before B; 97
queued A builds are cancelled with zero starts, the rest run once in queue
order, and all permits are released (item 2). B's record reads
`project_fair_turn` in SQLite, in `arcctl job diagnostics --json` as
`last_recorded_reason` beside the unchanged `admit`/`admitted` object, and in
the rendered `Last recorded decision:` line; the passed-over A build reads
`class_window` (item 3). The daemon reports zero connection, request and
database rejections.

Also passing and named, outside the item counts: `arc-cli`
`agent_hooks_test_literals::built_hook_rewrites_exactly_cargo_test_and_bun_run_test_for_both_vendors`,
`built_hook_keeps_the_existing_rewrites_beside_the_test_literals` and
`hook_exec_accepts_exactly_the_two_test_argv_forms`; `arc-storage`
`jobs::traffic::tests::project_key::null_project_keys_keep_sequence_order`
and `fair_head::a_keyless_entry_passes_nothing_and_is_passed_by_nothing` (a
request without a project key is ordered as before); `arc-ipc`
`traffic::fairness::tests::v1_projection_serializes_only_existing_vocabulary`.

### Row outcomes

| Row | Status | Remaining item |
| --- | --- | --- |
| NFR-001 | Complete | None in the numbered HOST list. |
| NFR-002 | Blocked | External result on each other required platform (item 2). |
| NFR-003 | Complete | None in the numbered HOST list, with the reference of contract D11. |
| NFR-004 | Complete | None in the numbered HOST list. |
| NFR-005 | Complete | None in the numbered HOST list. |
| R-004 | Blocked | External Windows and Linux capability and hard-limit behaviour (item 2). |
| DET-002 | Blocked | External PowerShell and Windows cmd cases (items 1 and 2); no HOST item. |
| NFR-022 | Blocked | External installed daemon and tray health after login (items 1 and 2); no HOST item. |
| AT-TRF-05 | Complete | None in the numbered HOST list. |
| AT-TRF-06 | Complete | None in the numbered HOST list. |
| AT-TRF-08 | Complete | None in the numbered HOST list. |

Phase 0 is now 22 Complete, 1 Partial, 0 Pending and 17 Blocked of 40; R-018 is the remaining Partial row and was not selected.
Phase 1 was then 19 Complete, 39 Partial, 12 Pending and 1 Blocked of 71. It was
13, 46, 12 and 0: the three acceptance rows above, TRF-001, TRF-006 and
AT-MTR-01 moved Partial → Complete and R-003 moved Partial → Blocked, each
for the reason given under
[Remaining Phase 1 inventories](#remaining-phase-1-inventories). Phases 2, 3,
4 and 5 are unchanged: 4/8/30/1 of 43, 0/5/103/0 of 108, 0/15/29/0 of 44 and
0/1/9/0 of 10 in the same order. See
[traceability](traceability.md#phase-1-entry-integration-evidence). These are
row counts; no phase is declared finished.

Not claimed by any row: a live vendor session, a queued wait measured against
a vendor's tool timeout, any EXTERNAL item, a real Bun, Vitest or Cargo
toolchain in the two journeys, or a long-run idle average. No customer or
revenue effect is claimed.

### Integration checks and boundary

This integration inspected Actor snapshot
`f23f16a46d40edb1a42e09738004e847c2439087`, tree
`03b3c4de923eab9bb86e8855c35d5980a6181c63`, which holds the contract seam and
the three lane deliveries; the accepted committed baseline is
`393f2a3f5fc9eec4a33c6ed1c6867dffa0a86a54`, tree
`f425c30b91ae9700a108b04b62dd5a1b7ed86c6b`. The integration edits are
uncommitted on that snapshot. These are focused local checks using
`oma-backend`, not the runner's final verification. The independent QA report
(`.arc-verify/phase-1-entry/qa.md`) is written separately for the snapshot it
reviewed and is not cited here.

- Passed on the final source: `cargo fmt --all -- --check`; `cargo clippy -p`
  with `--all-targets -- -D warnings` for each of the ten crates under
  `crates/`, and the same for the whole workspace; `cargo test -p` for
  `arc-ipc`, `arc-policy`, `arc-traffic`, `arc-storage`, `arc-detector`,
  `arc-cli` and `arc-daemon`, every target except the SwiftPM-backed
  `traffic_native_tools`: 78 test binaries, 2,294 tests, none failed and none
  ignored.
- One repair, in shared test support. Cause, reproduced: a failed test keeps
  its fixture directory `b<pid>-<n>` in the per-user runtime root (580 were
  present), and a later test process that reuses the pid fails with
  `AlreadyExists` when it creates the same name; this is the failure
  `registered_python_lifecycle_and_compatibility` showed once in the runner's
  earlier full run and not when run alone. Fix:
  `crates/arc-daemon/tests/support/native_root.rs` takes the next counter
  value when the name exists. Regression test:
  `traffic_python_tests::native_fixture_root_skips_a_leftover_directory_under_a_reused_pid`,
  which failed with that error before the fix. `startup.rs` builds its
  `d<pid>-<n>` directory the same way and was left unchanged: it removes the
  directory on failure too.
- No test failed in a crate run and passed alone in these checks.
- Wall time of the new test binaries in the `arc-daemon` crate run:
  `agent_test_journey` 4.21 s, `phase0_targets` 34.85 s (45.62 s in the
  separate `--nocapture` run) and `project_fairness` 9.71 s; `arc-cli`
  `agent_hooks_test_literals` 0.54 s. None exceeds 60 s. `managed_exec` took
  130.46 s for 181 tests, as before this cycle. The test binaries of the seven
  crates summed to 375 s, compilation excluded. The workspace run was not made
  here, so against the 900 s budget of
  `.arc-verify/test-performance/contract.md` this establishes only that 525 s
  remain for what was not measured: the `arc-core`, `arc-meter`,
  `arc-platform` and `arc-desktop` targets and the SwiftPM-backed targets.
- Desktop: every file under `apps/desktop` equals the base tree except one
  Rust test, `src-tauri/tests/memory_pressure/native_reserved.rs`, where the
  contract task added the new optional request field. No TypeScript, generated
  binding or wire fixture differs, and no Bun test file was added.
- `docs/managed-exec.md` said local commands enter a FIFO queue. It now states
  the order lane C delivered (first in, first out within a project, fair
  across projects within a class, arrival order without a key), the project
  key, the two files beside the socket and the poll cadence.
- Not run here, by assignment: the workspace test command, coverage, the
  architecture harness, any browser or screen check, the SwiftPM-backed
  targets and the `arc-desktop` tests. Tests of `arc-desktop` named in the
  Phase 1 lists were read, not run.

Gaps that the row statuses do not cover:

- `Sandbox::shell` runs commands through `/bin/sh`, which drops the injected
  interface inventory; the two journeys were not shown to read no network
  interface on that path.
- A script body of bare `vitest`, started from an interactive terminal,
  watches and holds its TEST permit until it exits. Under an agent's shell
  tool it runs once. Only the stub was exercised.
- A waiter behind the queue head learns that it became the head at its next
  poll, up to 2.56 s later. Under a memory hold every waiter keeps the 40 ms
  cadence, so the saturation repaired for the class-window wait can recur at
  depth; that case has no test.
- Fair turns count a project by its earliest queued entry of any class, and a
  directory outside any repository is its own project. Both are recorded as
  open questions in contract D1 and D5.
- The measurement record of the `bounded` test no longer carries the `rustc`,
  `cargo` and `local_head` host keys. Readers of those keys could not be
  searched for, because the code-search server was unavailable; the two
  evidence readers that were read do not use them.
- `managed_exec` still exceeds the 60-second rule, as recorded in the
  previous cycle.

Unmet prerequisite: the working `av.yaml` returns `Operation not permitted`
from this workspace, so the runner configuration in effect is not established
here. This document does not claim mission completion; the runner's four
commands and the final review decide that.

## Phase 1 resilience: assessment

This cycle closed ten rows of the
[Phase 1 inventories](#remaining-phase-1-inventories): TRF-009, AT-TRF-10 and
NFR-009 (Traffic resilience), MET-001, MET-006, MET-008, MET-024 and MET-025
(Meter readings), and AT-TRF-09 and ADR-014 (hook event identity). The
decisions, fixed test names and file ownership are in the
[cycle contract](../../.arc-verify/phase-1-resilience/contract.md). Each named
test is an `arc-daemon` target that starts built `arcctl` (or the hook as
`arcctl hooks install` writes it) and the real daemon with a cleared
environment, a temporary HOME and explicit paths, with offline Cargo stubs.

### Row outcomes

| Row | Before | Now | Test for the open item |
| --- | --- | --- | --- |
| TRF-009 | Partial | Complete | Item 4: `traffic_resilience::restart::trf_009_schema_migrating_restart_keeps_the_active_permit_and_running_job` |
| AT-TRF-10 | Partial | Complete | Item 3: `traffic_resilience::restart::at_trf_10_forced_daemon_kill_keeps_one_active_permit_and_one_start` |
| NFR-009 | Partial | Complete | Item 2: `traffic_resilience::faults::nfr_009_seeded_fault_interleavings_never_duplicate_or_lose_a_permit` |
| AT-TRF-09 | Partial | Complete | Item 1: `hook_event_identity::at_trf_09_redelivered_claude_event_creates_one_job_and_one_permit`, `hook_event_identity::at_trf_09_redelivered_cursor_event_creates_one_job_and_one_permit` |
| ADR-014 | Partial | Complete | Item 2: the AT-TRF-09 pair |
| MET-001 | Pending | Complete | Item 1: `meter_readings::met_host_cpu_memory_and_swap_values_agree_with_os_readings` |
| MET-006 | Pending | Complete | Item 1: the same test |
| MET-008 | Pending | Complete | Item 1: the same test |
| MET-024 | Partial | Complete | Item 2: `meter_readings::met_daemon_cpu_seconds_and_peak_memory_agree_with_os_readings` |
| MET-025 | Partial | Complete | Item 2: the same test |
| MTR-001 | Partial | Partial | Item 2 now names the MET-001 test. Remaining: 3. |

Phase 1 is now 29 Complete, 32 Partial, 9 Pending and 1 Blocked of 71; it was
19, 39, 12 and 1. Phase 0 is unchanged at 22 Complete, 1 Partial, 0 Pending
and 17 Blocked of 40, and Phases 2 to 5 are unchanged. See
[traceability](traceability.md#phase-1-resilience-integration-evidence).
These are row counts; no phase is declared finished, and no customer,
retention or revenue effect is claimed.

### Meter tolerances and deviations

Actor-measured, 5 runs, one macOS arm64 host, debug build (8 logical CPUs,
16 GiB), while the `arc-storage` test binaries ran in a loop as load; the
worst deviation is over two such series. A tolerance is kept only at five
times the worst deviation or more. Every run prints each figure on stderr as
a `meter-readings-<value>` line. These are measurements of one host and a
fixture workload, not benchmarks.

| Value | Tolerance | Worst deviation | Observed range |
| --- | --- | --- | --- |
| `host.cpu.utilization` | 8 ticks per logical CPU | 0 ticks | 59.3% to 97.8% |
| `host.memory.available_bytes` | 64 MiB | 1,982,464 bytes | 2.95 to 3.24 GB |
| `host.swap.used_bytes` | 16 MiB | 0 bytes | 11.163 to 11.172 GB |
| `arc.daemon.cpu.core_seconds` | 10,000 ns | 0 ns | 0.054 to 0.070 core·s |
| `arc.daemon.memory.peak_bytes` | none: rss <= peak <= OS lifetime peak, non-decreasing | 0 bytes | 8.44 to 12.34 MB |

The CPU and core-seconds tolerances are rounding bounds, not empirical ones.
In the Actor's runs swap did not change inside a window, the reported peak
equalled RSS in every snapshot and the daemon sampled at 1 s under host
memory pressure. This integration's single run of the target (17.49 s, every
deviation 0) printed: CPU 99.91% over a 2,012 ms interval (window 99.68%),
available memory 3.32 and 3.41 GB inside OS readings of 3.16 to 3.50 GB, swap
10.44 to 10.45 GB with an 8 MiB step inside one window, daemon core-seconds
0.082 and 0.099 inside `proc_pidinfo` brackets, and a peak of 12,369,920
bytes, 2,195,456 above RSS in three snapshots and 32,768 below the OS
lifetime peak. One run; the tolerances rest on the Actor's five.

### Decisions and follow-ups

- **Follow-up 1, unrecorded attach: not a defect.** Rule: it is a defect if,
  with a healthy daemon and a child that exited normally, stderr shows
  `TRACKING_UNCERTAIN` or `arcctl job status <ID>` shows `outcome unknown`.
  `traffic_resilience::attach::short_lived_child_attach_is_reported_truthfully`
  runs eight normally exiting children (two held until the process table shows
  them gone, exit 0 and 17; four unheld; two queued behind a held permit): no
  `TRACKING_UNCERTAIN`, and status shows succeeded or failed with the right
  exit code. `child_pid` was recorded in all eight (printed, not asserted);
  the NULL seen in cycle 33 was not reproduced. `project_fairness.rs` now
  derives start order from `traffic_entries` grant times.
- **Follow-up 2, turn-lock bound.** A waiter waits at most 64 slices (2.56 s)
  for `.traffic-wait-turn`, then polls without it, so it polls at the latest
  about 5.1 s after its previous reply:
  arc-cli `exec::traffic::cadence::tests::held_turn_lock_cannot_stop_a_waiter`,
  with the injected sleep. No stderr text was added.
- **Named follow-up:** an `arcctl job diagnostics` line for duplicate
  deliveries (for example `Duplicate deliveries: 1 (ran without a permit)`).
  Today the count is in SQLite only (`HOOK_EVENT_DUPLICATE` audit rows).
- Other follow-ups: TRF-009 steps back one migration (8 to 7) with an active
  permit; the step to 6 runs only in the seam test with a finished job.
  Including `managed_exec/fault.rs` runs its three unit tests twice in a
  workspace run. `project_fairness.rs` is at 495 of 500 lines. The
  `meter_readings` OS readers use Mach calls and compile on macOS only. Copies
  of the hook compiled by path into other crates' tests use a stand-in digest
  that returns no key, so only `arcctl` and its own tests cover keyed
  rewrites. The NFR-009 monitor accepts a released permit granted again at a
  higher generation, as the product's permit upsert does; the per-job
  single-start check is the backstop.

### Untested EXTERNAL observations

- Whether the keyed command text, which differs per tool call, changes a
  vendor's approval prompts (for example an exact-match allow rule). The
  recorded Claude Code payload ran with `bypassPermissions`; no prompt was
  observed either way. Smallest later check: one live Claude Code session in
  default permission mode, `cargo build` twice with the hook at `f4dfeb2` and
  twice with the keyed hook, counting approval prompts per command.
- A live vendor redelivery of one PreToolUse event. The tests feed the
  recorded payloads to the installed hook twice.

### Integration checks

This integration inspected Actor snapshot
`a179faf55da63e5c831e98f2c3cdfc5790cf0ce1`, tree
`14194e44766c9c867f28a59e0631c3fabefade50`, which holds the contract seam and
the three lane deliveries; the accepted committed baseline is
`f4dfeb2e919077188fbddba6a7bc398c7b4f1876`, tree
`a53e9006690617c8b99cbd133cc3ef8261f6db15`. The integration edits are
documentation only and uncommitted on that snapshot. These are focused local
checks using `oma-backend`, not the runner's final verification; the
independent QA report is written separately and is not cited here.

- Passed on the merged tree: `cargo fmt --all -- --check`; `cargo clippy
  --workspace --all-targets -- -D warnings` (src-tauri included); `cargo test
  -p arc-cli -p arc-ipc -p arc-storage -p arc-meter -p arc-platform`, 40 test
  binaries, after the `arc-cli` `governor` target was re-run (below); and
  `cargo test -p arc-daemon` for its binaries and every test target except
  the SwiftPM-backed `traffic_native_tools`, 24 binaries and 1,427 tests. No
  test failed, none is ignored.
- `arc-cli --test governor` failed 9 tests in the first crate run with
  "missing `target/debug/arc-pressure-daemon-fixture`": that run started
  before any `arc-daemon` binary was built. After the `arc-daemon` run built
  it, the target passed 34 of 34. This is a build-order prerequisite of the
  separate per-crate run, not a product defect; a workspace run builds every
  binary first. No file was changed for it.
- Wall time of the new test binaries in the `arc-daemon` run:
  `traffic_resilience` 15.21 s, `meter_readings` 17.49 s and
  `hook_event_identity` 5.05 s; `project_fairness` 10.81 s. None exceeds
  60 s. `managed_exec` took 132.03 s for 181 tests, as before this cycle.
- The two traceability Bun files and `scripts/harness/traffic-targets.test.ts`
  pass (37 tests).
- No integration repair was needed. `docs/managed-exec.md` now states the
  redelivered-event behaviour, the duplicate's CPU, copied managed commands,
  survival of a daemon upgrade and forced kill, the turn-lock bound and the
  first-sample CPU reading.
- Not run here, by assignment: the workspace test command, coverage, the
  architecture harness, the SwiftPM-backed targets and any browser or screen
  check. The `av.yaml` of this workspace returns `Operation not permitted`,
  so the runner configuration in effect is not established here.

## Phase 1 meter and policy: assessment

This cycle closed 22 rows of the
[Phase 1 inventories](#remaining-phase-1-inventories) and moved MTR-003 to
Blocked, in three lanes: job process metering (MTR-002, MTR-003, MTR-009,
DET-012, MET-002, MET-003, MET-004, MET-005, MET-009 and MET-010), usage
summaries (MTR-004, MTR-008, MTR-018, MET-013, MET-015, MET-017 and MET-026)
and Traffic policy and retention (DEC-013, POL-001, TRF-015, POL-004, MTR-014
and NFR-023). The decisions, fixed test names and file ownership are in the
[cycle contract](../../.arc-verify/phase-1-meter-policy/contract.md); the
behaviour is described in [managed execution](../managed-exec.md). Each named
test is an `arc-daemon` target that starts built `arcctl` (or the hook as
`arcctl hooks install` writes it) and a real daemon with a cleared
environment, a temporary HOME and explicit paths, with offline stubs and, for
lane A, a compiled process-tree fixture. Correctness comes from daemon state,
never from elapsed time.

### Row outcomes

| Row | Before | Now | Test for the open item |
| --- | --- | --- | --- |
| MTR-002 | Partial | Complete | Item 2: `job_process_meter::mtr_002_unwaited_grandchild_cpu_memory_and_io_are_attributed_to_the_job` |
| MTR-003 | Partial | Blocked | Item 2: the same test; only EXTERNAL item 3 remains |
| MTR-009 | Pending | Complete | Items 1 and 2: the same test |
| DET-012 | Partial | Complete | Item 2: the same test |
| MET-002 | Partial | Complete | Item 2: the same test |
| MET-003 | Partial | Complete | Item 2: the same test |
| MET-004 | Partial | Complete | Item 2: the same test |
| MET-005 | Pending | Complete | Item 1: the same test |
| MET-009 | Pending | Complete | Item 1: the same test |
| MET-010 | Pending | Complete | Item 1: the same test |
| MTR-004 | Partial | Complete | Items 1 and 2: `usage_summary::mtr_004_vendor_project_and_period_summaries_match_recorded_jobs` |
| MTR-008 | Partial | Complete | Item 2: the same test |
| MTR-018 | Partial | Complete | Item 3: the same test |
| MET-013 | Partial | Complete | Item 2: the same test |
| MET-015 | Partial | Complete | Item 2: the same test |
| MET-026 | Pending | Complete | Item 1: the same test |
| MET-017 | Partial | Complete | Item 2: `usage_summary::met_017_snapshot_reports_traffic_queue_depth_while_build_permit_is_held` |
| DEC-013 | Partial | Complete | Item 4: `traffic_policy::dec_013_start_rate_bucket_queues_then_grants_without_lost_or_duplicated_permit` |
| POL-001 | Partial | Complete | Item 2: the same test |
| TRF-015 | Partial | Complete | Item 3: `traffic_policy::trf_015_job_diagnostics_reads_every_decision_in_order_and_schedule`, with the DEC-013 test for the start-rate schedule |
| POL-004 | Partial | Complete | Item 3: `traffic_policy::pol_004_every_policy_change_writes_before_and_after_audit` |
| MTR-014 | Pending | Complete | Items 1 and 2: `traffic_policy::mtr_014_raw_samples_roll_up_then_delete_and_rows_stay_bounded` |
| NFR-023 | Pending | Complete | Item 1: the same test |

Completion counts per phase after this cycle: Phase 1 is 51 Complete, 16
Partial, 2 Pending and 2 Blocked of 71; it was 29, 32, 9 and 1. Phase 0 is
unchanged at 22 Complete, 1 Partial, 0 Pending and 17 Blocked of 40. Phase 2
is 4, 8, 30 and 1 of 43; Phase 3 0, 5, 103 and 0 of 108; Phase 4 0, 15, 29
and 0 of 44; Phase 5 0, 1, 9 and 0 of 10, all unchanged. See
[traceability](traceability.md#phase-1-meter-and-policy-integration-evidence).
These are row counts; no phase is declared finished, and no customer,
retention or revenue effect is claimed.

### Bases and boundaries

- Process-tree values are Estimated (attribution C) and are reported beside
  the waited rusage, never in place of it. A descendant that starts and exits
  between two Meter passes is not seen, so sampled reports stay `partial` with
  `unobserved_descendants`. The tree is macOS only (MTR-003 item 3).
- The recycled-identity control writes `job_lifecycle.child_start_token`
  directly while its job runs; that is the fixture's injection point for a
  reused PID. The outside control is a real process in no job's tree.
- `job.duration_ms` and CPU in usage summaries come from the job Meter report,
  which the wrapper sends best effort within 500 ms. The MTR-004 test compares
  every recorded report and requires at least one; a missing one reads `null`
  with `no_recorded_report`. One early lane run lost one report; the cause was
  not identified.
- `arc.hook.overhead_ms` is the hook process's own time, not the NFR-003
  end-to-end figure. No live vendor session ran; the hook received recorded
  Claude Code and Cursor payloads.
- The start-rate default (120 per minute, burst 120) is conservative relative
  to the class window of one: it never adds concurrency. No learned value
  exists at this snapshot. Class windows, per-class estimates and the memory
  budget have no setting; the CPU budget and start-rate settings change only
  at daemon startup, the switches at run time, and every one of those changes
  is audited.
- Rollup is exercised with seeded batches across daemon restarts, not a
  long-run soak. An hour or day bucket that spans two rollup passes counts a
  job once per pass.

### Integration checks

The accepted committed baseline is
`9e293033803e9bf2763dd17fec337315c8bedc5b`, tree
`3b79428fbad10bcb6c1da4113e3b0f3989d32c82`. This integration first inspected
snapshot `08584cd0f1fc00925dd45ce3c36ab8bdf20bda40`, tree
`091e7058a3f61ded6350ccc04e48b6ffe72c7807`, which holds the contract seam and
the three lane deliveries, and was updated on snapshot
`4e2cacfc726cb290bdbfe1ec1eaf980d432aa590`, tree
`cc901c2f098bc7387edfa30b825b9917f17a4eff`, and last on snapshot
`fd3f049e642bca2089ea7c502230cf0e4eb497b0`, tree
`7a7e3e009e9f2df8b8c60e29423cb10b06ecd59b`; its edits are uncommitted on that
snapshot. These
are focused local checks using `oma-backend`, `oma-debug` and `oma-docs`, not
the runner's final verification; the independent QA report is written
separately and is not cited here.

- Passed on the merged tree: `cargo fmt --all -- --check`; `cargo clippy
  --workspace --all-targets -- -D warnings`; `cargo test -p arc-cli -p arc-ipc
  -p arc-meter -p arc-platform -p arc-policy -p arc-storage -p arc-traffic`;
  `cargo test -p arc-daemon` for its binaries and every target except the
  SwiftPM-backed `traffic_native_tools`; and `cargo test -p arc-desktop` for
  every target except its `traffic_native_tools`. No test is ignored.
- Repair: `hook_event_identity::at_trf_09_redelivered_claude_event_creates_one_job_and_one_permit`
  failed once in the `arc-daemon` crate run because it compared the two
  returned commands byte for byte and their measured `--arc-hook-overhead-ms`
  differed (0 and 1 ms). It now compares the event keys and the commands
  without the overhead value; the target then passed 7 of 7 in six
  consecutive runs.
- QA repair (lane A test): the process-tree fixture's grandchild now reads
  its 4 MiB output back uncached and touches its 48 MiB on every pass, and
  `mtr_002_unwaited_grandchild_cpu_memory_and_io_are_attributed_to_the_job`
  asserts a grandchild RSS reading and the peak of at least the 48 MiB held
  and read bytes of at least the 4 MiB read back, so a reader stuck at zero
  fails. That test passed in the runner's full `cargo test --workspace
  --no-fail-fast` run of 2026-11-22T11:43Z, but that integration check failed
  its final gates (duration, traceability, coverage, harness): the full run
  failed two fixture tests that then passed alone, and both coverage
  collections stopped at a failing target, so every crate's coverage was
  unavailable. The repairs follow; a later runner check decides the gates.
- Repair (fixture cleanup, `arc-cli --test governor`):
  `owned_tree_initial_capture_failure_cleans_a_separately_grouped_descendant`
  and `owned_tree_unwind_during_initial_capture_still_cleans_the_descendant`
  failed with "fixture identity unavailable" (full run; alone, attempt 1;
  coverage collection 2). Cause: cleanup stopped the fixture's forked child
  while it exec'd the registration helper; macOS accepts that stop for the
  old PID generation and drops it once the exec completes (82 of 100
  reproductions), so the freeze loop spun until the helper's own 30 s
  timeout. The helper's exit then showed Darwin's live-to-zombie window,
  which `process_identity::probe` reports as indeterminate. The fixture now
  re-stops the same kernel process at its new generation and re-observes an
  indeterminate probe while the PID still exists. No product code changed.
  A new deterministic regression test,
  `owned_tree_restops_a_process_whose_stop_was_lost_to_an_exec`, was added.
  The two tests passed 150 of 150 consecutive loops beside six CPU-bound
  processes (before: 1 failure in 40 to 70 loops), and the target passed 35
  of 35.
- Repair (test reads): `arc-platform` `process_tree::tests::ppid_descendants_are_read_with_their_identity_and_usage`
  (full run fail, alone pass) read the tree while its child had just exec'd,
  when the kernel truthfully reports 0 resident bytes (8% of early reads); it
  now waits for both processes to be resident in the read it asserts on.
  It then passed 100 of 100 runs under the same load. `traffic_resilience`
  `attach::short_lived_child_attach_is_reported_truthfully` (scoped check
  11:27Z, failed in the full run and passed alone) saw `Indeterminate` in the
  same exit window; it now waits for the probe to settle before asserting
  `VerifiedMatch`. The target then passed 8 of 8 three times.
- Repair (fail-fast witness): `arc-daemon --test traffic_native_tools`
  failed in two consecutive checks and passed alone each time: coverage
  collection 1 (12:04Z, test output removed with the run directory) and
  `cmake_ordered_pairs` in the full run of 12:59Z, panicking at
  `traffic_native_tools/witnesses.rs:138`. In the kept fixture
  (`ctest_then_cargo-test`) the first project's ctest held its Test permit,
  and the second project's `cargo test` wrapper printed `TRACKING_UNCERTAIN`
  and launched instead of queueing. Its enqueue was not confirmed within
  the wrapper's 1800 ms preparation budget, so it started untracked as
  designed (fail-open), and the witness waited its full 60 s for a QUEUED
  state that could no longer arrive. This mission's daemon work is not the
  cause. With temporary timing instrumentation, under eight CPU-bound
  processes and a write-and-sync loop, the largest lane A sample
  transaction took 3.8 ms and the largest writer operation of any kind took
  15 ms. The wrapper still had at least 1,476 ms of its budget left when
  admission returned, and 10 loaded runs reproduced no fail-open. The two
  earlier full runs with every lane's code took 137 s and 149 s for this
  binary. In the 12:59Z run every binary was slower (1,373.8 s inside test
  binaries, against 899.5 s at 11:43Z), and the failing case's own
  admission observation recorded memory pressure `warning` with about
  12 GB of swap in use. That fits a host stall of more than about 1.5 s.
  The fix is test-only: the queued witness now fails as soon as the second
  wrapper prints `TRACKING_UNCERTAIN` or records a launch, naming the case,
  so one stall costs seconds rather than the 60 s watchdog. The QUEUED
  assertion is unchanged, and no product code changed. The same run's
  `traffic_resilience` `attach::short_lived_child_attach_is_reported_truthfully`
  failure ("process identity unavailable" at `attach.rs:94`) was the capture
  itself landing in the same Darwin exit window; the test now retries the
  capture of the unreaped child, whose PID cannot be reused. Under the same
  load, after the fix, the attach test passed 3 of 3 and
  `cmake_ordered_pairs` 3 of 3, and `traffic_resilience` passed 8 of 8 alone.
- Repair (pipe-free children): `traffic_native_tools` failed a third
  consecutive check at 14:11Z, again passing alone. There
  `suite::go::owned::tests::go_owned_child_counts_are_isolated_between_parallel_cases`
  waited its full 60 s watchdog ("first case never reaped its child") and
  pushed the workspace run to 1,205.8 s against the 1,200 s limit. Cause: its
  two cases each started `/bin/cat` children that exit only at EOF on piped
  stdin, and case 1 is released only after case 0 is reaped. On macOS std
  creates a pipe with `pipe()` and sets close-on-exec afterwards, so a fork in
  that window (the cases spawn concurrently, through `pre_exec`) can leave
  case 0's stdin write end open in case 1's `cat`, which then waits for its
  own release: a deadlock that only the watchdog breaks. The children are now
  `/usr/bin/true` with null stdin, stdout and stderr, so no case's reaping
  depends on a pipe another process may hold. The started and cleaned count
  assertions, both release steps and the 60 s watchdogs are unchanged; no
  product code changed. Audit of the target's other piped-stdin waits
  (`direct`, the Go parity runs, the root `sh` fixtures that read one line):
  each is closed by its own writer and none is gated on another case's
  release, so a leaked descriptor can delay them only until that holder
  exits. This repair compiles and is clippy-clean; it was not run here, and
  the runner's check decides it.
- Added check: `db_worker::resource_samples::tests::written_samples_roll_up_to_their_growth_then_are_deleted`
  rolls rows written by lane A's sample writer up through lane C's rollup and
  checks the CPU, write and peak sums, confirming both lanes use the same
  cumulative encoding.
- `arc-cli --test governor` failed 9 tests in the first crate run with
  "missing `target/debug/arc-pressure-daemon-fixture`", a build-order
  prerequisite of the separate per-crate run; once the `arc-daemon` run built
  it, the target passed 34 of 34. No file changed for it.
- Wall time of the new test binaries: `job_process_meter` 5.80 s,
  `usage_summary` 3.23 s and `traffic_policy` 21.70 s; `hook_event_identity`
  5.93 s. None exceeds 60 s.
- The traceability checker reports the matrix valid with the counts above.
- Not run here, by assignment: the workspace test command, coverage, the
  architecture harness, the SwiftPM-backed targets, the desktop Bun checks and
  any browser or screen check.

## Remaining Phase 1 inventories

These lists give every other Phase 1 row its finite cases, in the style of the
[Phase 0 inventories](#remaining-phase-0-inventories). The three acceptance
rows of this cycle have their lists
[above](#phase-1-entry-traffic-journey-lists). `HOST` and `EXTERNAL` mean what
they mean there, and a missing HOST test is work remaining, not Blocked.

A case counts only with the named passing test beside it. Unless a crate is
named, a test is an `arc-daemon` target that runs built binaries against a
real daemon; `arc-cli` targets run built `arcctl`, mostly against scripted peers, and
`arc-storage`, `arc-traffic` and `arc-detector` tests are named only as
support beside a production-path test. Every `arc-daemon`, `arc-cli`,
`arc-storage`, `arc-traffic` and `arc-detector` test named here passed in this
integration's crate runs. The `arc-desktop` tests named here were read in the
source and are run by the runner's workspace check, not here.

The statuses are lower bounds. The sources could not be searched in this
integration, so "no test identified here" means that no test was found by
reading the test targets and their passing names; such a case counts as
remaining. Four rows changed, each because every HOST case now has a named
passing test:

- **TRF-001** Partial → Complete: the project case is AT-TRF-08 and the vendor
  case is the pair of recorded Claude Code and Cursor events through the built
  hook, which no longer passes commands through.
- **TRF-006** Partial → Complete: its cases are the finite remote triggers
  already closed under DET-005, AT-TRF-02, AT-TRF-04 and AT-TRF-12.
- **AT-MTR-01** Partial → Complete: the earlier assessment named no missing
  case, only final gates that were pending then; the calibration test is run
  by the runner's workspace check.
- **R-003** Partial → Blocked: the three HOST cases of the response the SRS
  assigns to ARC are tested, and what remains is an observation against a
  live vendor's tool timeout.

Every other row keeps its status. A row that stays Partial or Pending lists
its open cases after "Remaining:".

The [Phase 1 resilience cycle](#phase-1-resilience-assessment) then closed
TRF-009, NFR-009, AT-TRF-09, AT-TRF-10, ADR-014, MET-001, MET-006, MET-008,
MET-024 and MET-025, and named a test for MTR-001 item 2. The
[Phase 1 meter and policy cycle](#phase-1-meter-and-policy-assessment)
then closed 22 rows and moved MTR-003 to Blocked; its row outcomes and
completion counts per phase are listed there.

### Requirements

- **DET-004** — 1. **HOST** the leaf commands of a script, not its name, decide BUILD, TEST, LINT or TYPECHECK: arc-detector `script_resolution::leaf_commands_not_script_names_determine_all_four_classes_and_same_class_chains`; 2. **HOST** on the execution route each class is admitted as itself: BUILD `traffic_cli::measurement::two_real_builds_queue_fifo_and_bypass_remote_and_unknown_while_full`, TEST and LINT `traffic_cli::semantics::scheduled_test_and_lint_preserve_native_streams_and_admission`, TYPECHECK arc-desktop `traffic_bun_scripts::registered_bridge_two_typechecks_one_typecheck_slot`. Complete.
- **DET-009** — 1. **HOST** nested scripts, lifecycle scripts, forwarded arguments and explicit `--cwd` delegations resolve to their leaf commands: arc-detector `script_resolution::root_desktop_delegations_explicit_cwd_and_parent_package_execution_context`, arc-detector `script_resolution::lifecycle_is_once_in_order_without_recursive_hooks_or_forwarded_flags`, executed in `traffic_bun_scripts::real_installed_tools_and_original_bun_scripts`; 2. **HOST** workspace analysis: a workspace filter or fan-out resolves the member packages' scripts. Not implemented; such graphs bypass admission. Partial. Remaining: 2.
- **DET-012** — 1. **HOST** a managed child that re-enters ARC is linked to the root job by verified ancestry: `agent_test_journey::at_trf_05_nested_hook_test_shares_the_root_test_permit`, `traffic_cli::semantics::cargo_and_bun_nested_jobs_reuse_the_parent_permit_in_both_directions`, `traffic_cli::recovery::queued_nested_cli_rechecks_ancestry_and_reuses_active_lineage`; 2. **HOST** child runtime processes that never re-enter ARC (test workers, compilers) are linked to the managed job: `job_process_meter::mtr_002_unwaited_grandchild_cpu_memory_and_io_are_attributed_to_the_job` (a compiled grandchild that never runs `arcctl` is sampled under the job from its process group, each sample revalidated by PID, start token and boot identity, confidence C). Complete.
- **DET-014** — 1. **HOST** the analysis of a project is cached and reused by the next classification in that project; 2. **HOST** a changed manifest or script invalidates it. No named test establishes a cache; the NFR-005 test times repeated classification and makes no assertion about a cache. Pending. Remaining: 1, 2.
- **TRF-001** — 1. **HOST** jobs of different projects pass one coordinator queue: `project_fairness::at_trf_08_project_b_is_granted_after_at_most_one_project_a_job`, `traffic_python_tests::python_then_cargo_one_test_slot`; 2. **HOST** jobs of two vendors pass the same queue: recorded Claude Code and Cursor events through the built hook, one Build slot, both arrival orders, `traffic_bun_scripts::agent_hooks_claude_first_one_build_slot` and `traffic_bun_scripts::agent_hooks_cursor_first_one_build_slot`. The queue carries no vendor identity. A Codex adapter is AT-TRF-01 item 2, and fairness between vendors is TRF-013 item 2; neither is claimed here. Complete.
- **TRF-002** — 1. **HOST** each class has its own window: arc-traffic `policy::direct_classes_have_independent_windows_and_fixed_reservations`, arc-traffic `script_policy::all_script_classes_use_their_existing_or_distinct_windows`, and against a real daemon `traffic_cli::protocol::protocol_admission_enforces_windows_budget_fifo_and_bypass`; 2. **HOST** of two real commands of one class one runs and the other waits: BUILD `traffic_cli::measurement::two_real_builds_queue_fifo_and_bypass_remote_and_unknown_while_full`, TEST `traffic_python_tests::python_then_cargo_one_test_slot`, TYPECHECK arc-desktop `traffic_bun_scripts::registered_bridge_two_typechecks_one_typecheck_slot`. Complete.
- **TRF-003** — 1. **HOST** classes with free windows still wait on the shared CPU budget; 2. **HOST** and on the shared memory budget. Both: `traffic_cli::diagnostics::persisted_mixed_tests_wait_for_independent_cpu_and_memory_budgets`; policy arithmetic in arc-traffic `script_policy::typecheck_window_is_independent_but_cpu_and_memory_are_global`. Complete.
- **TRF-004** — 1. **HOST** a grant and its queue, permit and audit rows commit or roll back together (arc-storage `memory_pressure::admission::audit_failure_rolls_back_permit_queue_and_policy_state_together`), and a forced daemon death before a commit leaves no partial state (`managed_exec::crash_recovery::forced_daemon_death_before_commit_rolls_back_and_integrity_ok`); 2. **HOST** a repeated or retried request for one job never reserves twice: `traffic_cli::protocol::lost_grant_and_completion_acknowledgements_are_idempotent`, `managed_exec::protocol::retries_response_loss_concurrent_claim_and_completion_are_idempotent`, `traffic_cli::semantics::bun_lost_grant_attach_and_completion_replies_reuse_one_permit`. Complete.
- **TRF-005** — 1. **HOST** stdout and stderr bytes of a managed job are passed through unchanged; 2. **HOST** its exit code is the wrapper's. Both: `managed_exec::cli::exact_native_argv_environment_cwd_streams_and_exit17_match_direct_execution`, `traffic_cli::semantics::scheduled_test_and_lint_preserve_native_streams_and_admission`, `traffic_python_tests::native_python_execution_semantics`. Complete.
- **TRF-006** — 1. **HOST** `gh workflow run build.yml` through the installed hook is REMOTE_CI and takes no queue entry or reservation while a Build permit is held: `managed_exec::capture_integration::closing_exec::installed_claude_hook_executes_same_remote_child_with_held_build_permit`; 2. **HOST** the other named remote triggers (git push, GitLab dispatch, gh queries) take none: `managed_exec::capture_integration::closing_locality::proven_remote_cases_never_classify_local_on_either_route`, `managed_exec::capture_integration::closing_locality::gitlab_explicit_public_endpoint_and_unresolved_controls_on_both_routes`; 3. **HOST** in a mixed command only the local stage is admitted: `managed_exec::capture_integration::closing_exec::stages::mixed_bun_stages_via_hook`, `managed_exec::capture_integration::closing_shell::stages::mixed_bun_stages_via_exec`; 4. **HOST** a build on a remote Docker builder takes no local permit: `managed_exec::capture_endpoints::held::selected_remote_buildx_bypasses_held_build_permit_and_runs_once`. This is the finite set of DET-005, AT-TRF-02, AT-TRF-04 and AT-TRF-12, with offline stubs; no live dispatch is claimed. Complete.
- **TRF-007** — 1. **HOST** with no daemon reachable a new command runs once, unchanged: `agent_test_journey::at_trf_06_installed_hook_cargo_test_fails_open_exactly_once`, `managed_exec::fail_open::outages_preserve_native_bytes_exit17_one_invocation_budget_and_no_replay`; 2. **HOST** a daemon stop or kill leaves a running managed child running, and the restart reconciles it without replay: `managed_exec::cli::daemon_shutdown_and_crash_preserve_child_and_restart_reconciles_without_replay`. Complete.
- **TRF-008** — 1. **HOST** a wrapper started inside a managed job takes no second reservation: `traffic_cli::semantics::cargo_and_bun_nested_jobs_reuse_the_parent_permit_in_both_directions`, `agent_test_journey::at_trf_05_nested_hook_test_shares_the_root_test_permit`, arc-cli `job_accounting::lineage::nested_wrappers_resolve_one_canonical_root_across_scheduled_and_lifecycle_modes`. Complete.
- **TRF-009** — 1. **HOST** a running job is re-identified after a restart and not run again: `managed_exec::cli::daemon_shutdown_and_crash_preserve_child_and_restart_reconciles_without_replay`, `traffic_cli::recovery::nested_real_cargo_reuses_lineage_and_restart_keeps_the_active_child`; 2. **HOST** a queued job survives a restart and starts once: `traffic_cli::recovery::bun_restart::real_bun_queued_and_running_jobs_survive_daemon_restart`; 3. **HOST** the grant of a gone owner or child is reconciled: `traffic_cli::recovery::restart_recovers_gone_owner_and_child_without_replaying_work`, `traffic_cli::unattached::restart_retains_unknown_child_until_configured_grace_then_logs_release`; 4. **HOST** a restart into a newer daemon that migrates the schema keeps an active permit and its running job (§9.9, daemon upgrade): `traffic_resilience::restart::trf_009_schema_migrating_restart_keeps_the_active_permit_and_running_job`. The daemon stops with `arcctl exec -- cargo build` RUNNING and holding the one Build permit; the database is stepped back to schema version 7 (migration 0008 undone, read as `UpgradeRequired`); the current daemon binary migrates it to 8 at start. The same permit stays ACTIVE, the job is RUNNING with the same PID and start time and `arcctl job status` shows `running`, the stub started once, and the permit is released once when the child exits. Complete.
- **TRF-013** — 1. **HOST** one project's 100 queued jobs do not starve another project's one: `project_fairness::at_trf_08_project_b_is_granted_after_at_most_one_project_a_job`; 2. **HOST** the same bound between vendors (§9.6). Not implemented: job records carry no vendor identity; 3. **HOST** a developer's interactive command generally precedes agent background work; 4. **HOST** bounded short-job preference with aging; 5. **HOST** bounded backfilling behind an oversized request. Items 3 to 5 are not implemented. Not selected in this cycle. Partial. Remaining: 2, 3, 4, 5.
- **TRF-015** — 1. **HOST** each job's queue, grant and terminal facts are recorded and read through SQLite and the CLI: `traffic_cli::terminal_outcomes::scheduled_exit_17_persists_failed_across_restart`, `traffic_cli::status::status_identifies_a_retained_live_child_as_running`; 2. **HOST** the reason of the last decision is recorded and readable: budget waits `traffic_cli::diagnostics::persisted_mixed_tests_wait_for_independent_cpu_and_memory_budgets`, reuse `agent_test_journey::at_trf_05_nested_hook_test_shares_the_root_test_permit`, fair turn `project_fairness::at_trf_08_project_b_is_granted_after_at_most_one_project_a_job`; 3. **HOST** the sequence of a job's decisions, not only the last one, and its expected schedule are readable: `traffic_policy::trf_015_job_diagnostics_reads_every_decision_in_order_and_schedule` (every recorded decision in order, `Queue position: N of M` while queued, `Expected schedule: unknown` when no time is derivable), `traffic_policy::dec_013_start_rate_bucket_queues_then_grants_without_lost_or_duplicated_permit` (a start-rate hold reads `start_rate` with `Expected schedule: start-rate token at <ms>`, and the grant follows that time in daemon state). Complete.
- **TRF-018** — 1. **HOST** cancelling a queued job leaves zero starts and no permit: `project_fairness::at_trf_08_project_b_is_granted_after_at_most_one_project_a_job` (97 cancelled), `traffic_cli::robustness::killed_queued_wrapper_does_not_leave_notice_pipe_open`; 2. **HOST** a signal to the wrapper reaches the child and the native signal status is returned: `traffic_cli::semantics::scheduled_child_signal_is_returned_as_native_signal_status`, `managed_exec::cli::wrapper_directed_signal_preserves_native_signal_status_and_no_second_child`; 3. **HOST** terminal stop, continue and foreground signals match a direct child: `managed_exec::pipeline::native_foreground_pipeline_input_signals_stop_resume_and_exit17`, `managed_exec::terminal::native_pty_foreground_input_modes_and_terminal_signals_match_direct_child`. Complete.
- **MTR-001** — 1. **HOST** the running daemon samples memory pressure and the free space of its filesystem, read through built `arcctl meter snapshot`: `worker_runtime::meter_partial::partial_native_failures_preserve_available_fields_cli_health_and_jobs`; 2. **HOST** host CPU utilization and available memory are read as values from the real daemon through the CLI: `meter_readings::met_host_cpu_memory_and_swap_values_agree_with_os_readings`; 3. **HOST** disk pressure is judged under a pressure workload. No test. Partial. Remaining: 3.
- **MTR-002** — 1. **HOST** a managed job's child is bound by PID, start token and boot identity: `managed_exec::protocol::identity_mismatch_missing_and_restart_never_invent_an_exit`, `managed_exec::current_user_ownership::built_cli_current_user_socket_and_job_ownership`; 2. **HOST** the job's process group and descendants are attributed to it: `job_process_meter::mtr_002_unwaited_grandchild_cpu_memory_and_io_are_attributed_to_the_job` (an unwaited grandchild in the job's process group is in `resource_samples` under the job across at least two samples; a process outside the tree and a recycled identity are never attributed). Complete.
- **MTR-003** — 1. **HOST** a job's CPU core-seconds, run duration and exit status are recorded and agree across SQLite, CLI and the registered bridge: arc-desktop `job_meter_acceptance::calibration::at_mtr_01_two_workers_ten_seconds_reaped_descendants_persist_on_all_surfaces`, arc-cli `job_summary::summary_json_correlates_the_job_and_preserves_queue_zero_and_unknown_states`; 2. **HOST** CPU time of descendants the wrapper does not wait for is included: `job_process_meter::mtr_002_unwaited_grandchild_cpu_memory_and_io_are_attributed_to_the_job` (`process_tree.cpu_core_seconds` is the waited rusage plus the unwaited grandchild's sampled CPU and exceeds the waited total); 3. **EXTERNAL** the same on other platforms. Blocked; only item 3 remains.
- **MTR-004** — 1. **HOST** usage is aggregated by vendor from recorded jobs; 2. **HOST** usage is aggregated by project: `usage_summary::mtr_004_vendor_project_and_period_summaries_match_recorded_jobs` (recorded Claude Code and Cursor events through the installed hook, a direct `arcctl exec` job and a gh REMOTE_CI job in two projects; `arcctl meter summary --by vendor` and `--by project` equal an independent SQLite recompute). Complete.
- **MTR-005** — 1. **HOST** queue wait and run time are separate recorded values, and waiting does not lengthen the run interval: arc-desktop `job_meter_acceptance::lifecycle::real_queue_wait_is_separate_from_monotonic_running_duration`, arc-cli `job_accounting::queued_wait_does_not_inflate_the_native_run_interval`, arc-cli `job_summary::summary_json_correlates_the_job_and_preserves_queue_zero_and_unknown_states`. Complete.
- **MTR-006** — 1. **HOST** the daemon's own CPU time is measured: `phase0_targets::nfr_001_idle_daemon_cpu_below_half_percent_of_one_core`; 2. **HOST** its own RSS is measured, externally in `bounded::native_probe_then_full_100000_attempt_workload` and by the daemon in its Meter snapshot in `worker_runtime::meter_partial::partial_native_failures_preserve_available_fields_cli_health_and_jobs`. Complete.
- **MTR-007** — 1. **HOST** CPU used by a process whose agent ownership cannot be established is counted as Unattributed, not under a vendor. No production-path test; the row is Partial on desktop presentation fixtures. Partial. Remaining: 1.
- **MTR-008** — 1. **HOST** a remote job takes no local reservation and only its local CLI is metered: `managed_exec::capture_integration::held::gh_and_buildx_same_execution_bypass_held_permit_and_buildx_cli_is_metered`; 2. **HOST** remote runner usage is excluded from local usage totals: `usage_summary::mtr_004_vendor_project_and_period_summaries_match_recorded_jobs` (the REMOTE_CI job is counted only under `remote`, with CPU `null` and reason `not_metered_locally`, and the local totals equal the three local jobs). Complete.
- **MTR-009** — 1. **HOST** read and write I/O bytes of a job's processes are recorded; 2. **HOST** cumulative disk bytes written are recorded: `job_process_meter::mtr_002_unwaited_grandchild_cpu_memory_and_io_are_attributed_to_the_job` (per-process `disk_read_bytes` and `disk_write_bytes` rows, and the report's sums of each process's last cumulative reading; written bytes are at least the 4 MiB the grandchild wrote and read bytes at least the 4 MiB it read back uncached). Complete.
- **MTR-014** — 1. **HOST** raw samples are rolled up to minute, hour and day aggregates and then deleted; 2. **HOST** stored Meter records expire on the daemon's own schedule: `traffic_policy::mtr_014_raw_samples_roll_up_then_delete_and_rows_stay_bounded` (raw samples seeded older and newer than 72 h before the daemon starts; its own schedule writes the exact minute, hour and day sums, deletes the old raw rows in the same transaction and expires a stored Meter record), with arc-storage `job_meter_native_retention::real_native_measurement_obeys_ninety_day_boundary_without_resurrection` as support. Complete.
- **MTR-015** — 1. **HOST** job reports carry completeness and missing-coverage flags: arc-desktop `job_meter_acceptance::calibration::at_mtr_01_two_workers_ten_seconds_reaped_descendants_persist_on_all_surfaces`, arc-cli `job_accounting::lineage::unverified_lineage_is_non_additive_and_caller_environment_does_not_create_ownership`; 2. **HOST** host readings carry availability, never a silent zero: `worker_runtime::meter_partial::partial_native_failures_preserve_available_fields_cli_health_and_jobs`; 3. **HOST** quality flags on the metrics not yet recorded (I/O, disk written, per-vendor totals). Partial. Remaining: 3.
- **MTR-018** — 1. **HOST** `arcctl meter summary --job <ID> --json`: arc-cli `job_summary::summary_json_correlates_the_job_and_preserves_queue_zero_and_unknown_states`; 2. **HOST** `arcctl meter snapshot --json`: arc-cli `meter::successful_json_snapshot_and_old_daemon_errors_are_typed_and_private`, `worker_runtime::meter_partial::partial_native_failures_preserve_available_fields_cli_health_and_jobs`; 3. **HOST** JSON summaries by vendor, project and period: `usage_summary::mtr_004_vendor_project_and_period_summaries_match_recorded_jobs` (`arcctl meter summary --by vendor|project|period --json` in hour and day buckets over an explicit range). Complete.
- **POL-001** — 1. **HOST** the default policy is conservative: one slot per class and fixed CPU and memory budgets, `traffic_cli::measurement::two_real_builds_queue_fifo_and_bypass_remote_and_unknown_while_full`, `traffic_cli::diagnostics::persisted_mixed_tests_wait_for_independent_cpu_and_memory_budgets`; 2. **HOST** the remaining initial policy (start-rate limit, per-class estimates) starts conservative: `traffic_policy::dec_013_start_rate_bucket_queues_then_grants_without_lost_or_duplicated_permit` (an unconfigured daemon reports every class at 120 starts per minute with a burst of 120, which never allows more than the window of one and bounds a start storm to two per second per class, with its default CPU and memory estimates, and the start-rate switch on). Complete.
- **POL-003** — 1. **HOST** a fresh Critical memory reading holds every supported class although slots are free (arc-storage `memory_pressure::admission::every_supported_class_holds_with_free_slots_and_duplicate_polls_do_not_append`), and the CLI tells current pressure from the last recorded decision (arc-cli `memory_pressure::status_reads::cli_status_distinguishes_current_pressure_from_last_recorded_decision_and_l0`); the reading is injected, at storage and CLI level; 2. **HOST** a Critical event raised by the OS itself holds admission. Not exercised; 3. **EXTERNAL** pressure sources of other platforms. Partial. Remaining: 2.
- **POL-004** — 1. **HOST** memory-admission decisions are stored with the grant in one transaction and read back as the last recorded decision: arc-storage `memory_pressure::admission::audit_failure_rolls_back_permit_queue_and_policy_state_together`, arc-cli `memory_pressure::status_reads::cli_status_distinguishes_current_pressure_from_last_recorded_decision_and_l0`; 2. **HOST** window, budget and fairness decisions are stored with their reason: `traffic_cli::diagnostics::persisted_mixed_tests_wait_for_independent_cpu_and_memory_budgets`, `project_fairness::at_trf_08_project_b_is_granted_after_at_most_one_project_a_job`; 3. **HOST** every other policy change (windows, learned values) is audited: `traffic_policy::pol_004_every_policy_change_writes_before_and_after_audit` (`policy.set_switch`, `daemon.pause` and `daemon.resume` each write one `audit_events` row with before and after values for the policy enable state, and a daemon started with a changed configuration writes one row covering class windows, per-class estimates, budgets and start-rate settings). Windows and budgets change only at daemon startup; class windows and the memory budget have no setting at this snapshot and are recorded unchanged. No learned value exists at this snapshot, so none is audited or claimed. Complete.
- **NFR-009** — 1. **HOST** deterministic faults (lost replies, retries, forged lineage, failed spawn) never duplicate or lose a permit: `traffic_cli::protocol::lost_grant_and_completion_acknowledgements_are_idempotent`, `traffic_cli::recovery::replay_forged_lineage_cancellation_and_spawn_failure_preserve_one_permit`, `traffic_cli::terminal_outcomes::failed_completion_retry_releases_one_permit`; 2. **HOST** the randomized fault test the SRS names: `traffic_resilience::faults::nfr_009_seeded_fault_interleavings_never_duplicate_or_lose_a_permit`. Seeds 1, 3, 5 and 7 each drive a pseudo-random interleaving, over up to four wrappers of BUILD, TEST and LINT, of the retried request, lost reply, wrapper killed while queued, wrapper killed while running, failed spawn and daemon kill and restart, through built `arcctl exec`, the existing fault proxy of `managed_exec` and the socket; every fault kind acts on a job in every seed. After every step and at the end SQLite shows no window exceeded, no job with two permits, no released grant changed again and no permit disappeared, no ACTIVE permit without a live owner after reconciliation and no stub started twice, and the final active count is 0. A failure prints its seed. Complete.
- **NFR-012** — 1. **HOST** measured values carry completeness labels: arc-desktop `job_meter_acceptance::calibration::at_mtr_01_two_workers_ten_seconds_reaped_descendants_persist_on_all_surfaces`; 2. **HOST** Proven, Estimated and Unattributed usage of native processes are shown separately. No production-path test. Partial. Remaining: 2.
- **NFR-023** — 1. **HOST** over a long run raw metric sampling does not grow the database without bound: `traffic_policy::mtr_014_raw_samples_roll_up_then_delete_and_rows_stay_bounded` (raw and rollup row counts stay bounded across three seeded batches older than 72 h, each started daemon rolling up and deleting them). Not a long-run soak. Complete.

### Acceptance tests

- **AT-TRF-01** — 1. **HOST** two vendors' hook events (recorded Claude Code and Cursor events for `cargo build` and `bun run build`, through the built hook) pass one coordinator; at most one Build runs and the later one starts after the release: `traffic_bun_scripts::agent_hooks_claude_first_one_build_slot`, `traffic_bun_scripts::agent_hooks_cursor_first_one_build_slot`; 2. **HOST** the Codex hook the SRS names. No Codex adapter exists. Partial. Remaining: 2.
- **AT-TRF-07** — 1. **HOST** a real Test command that exits 17 with stderr keeps its stderr and the wrapper exits 17: `traffic_python_tests::native_python_execution_semantics`, `managed_exec::cli::exact_native_argv_environment_cwd_streams_and_exit17_match_direct_execution`; 2. **HOST** the job is recorded FAILED: `traffic_cli::terminal_outcomes::scheduled_exit_17_persists_failed_across_restart`. Complete.
- **AT-TRF-09** — 1. **HOST** a vendor adapter redelivers a PreToolUse event with the same `tool_call_id` and one job is created: `hook_event_identity::at_trf_09_redelivered_claude_event_creates_one_job_and_one_permit` and `hook_event_identity::at_trf_09_redelivered_cursor_event_creates_one_job_and_one_permit`. For the recorded Claude Code 2.1.327 payload and the recorded Cursor preToolUse payload, the installed hook receives the event twice and returns the same keyed command (`hook-exec --arc-event <key> -- cargo build`) both times; neither raw id appears in the command, SQLite or any stream. Both commands run against a real daemon with Build window 1 and an offline stub: SQLite holds one job, one queue entry and one permit for the key, permit high-water is 1, the redelivery runs to its end while the first holds the permit, with no `QUEUED` notice, no other ARC text and no admission decision of its own, each execution ran its stub once with unchanged arguments, cwd, environment, streams and exit status, the redelivery is recorded on the existing job, and the permit is released once. Controls: `hook_event_identity::at_trf_09_distinct_tool_use_ids_create_two_jobs`, `hook_event_identity::at_trf_09_redelivered_event_fails_open_without_a_daemon` and `hook_event_identity::at_trf_09_a_daemon_that_does_not_advertise_receives_no_key`. A live vendor redelivery is not claimed; 2. **HOST** a repeated request for one job identity reserves once: `managed_exec::protocol::retries_response_loss_concurrent_claim_and_completion_are_idempotent`, `traffic_cli::protocol::lost_grant_and_completion_acknowledgements_are_idempotent`. Complete.
- **AT-TRF-10** — 1. **HOST** a managed job runs through a daemon kill and restart; its PID and start time are checked, it is not run again and it is RUNNING afterwards: `managed_exec::cli::daemon_shutdown_and_crash_preserve_child_and_restart_reconciles_without_replay`; 2. **HOST** a scheduled job holding its permit keeps one ACTIVE permit and one start through a daemon stop and restart: `traffic_cli::recovery::nested_real_cargo_reuses_lineage_and_restart_keeps_the_active_child`; 3. **HOST** item 2 across a forced kill of the daemon: `traffic_resilience::restart::at_trf_10_forced_daemon_kill_keeps_one_active_permit_and_one_start`, the TRF-009 item 4 fixture with `SIGKILL` instead of the schema step: one ACTIVE permit, one start, PID and start time rechecked, no replay. Every earlier scheduled restart test used `SIGTERM`. Complete.
- **AT-MTR-01** — 1. **HOST** a managed job of two workers for about ten seconds is recorded in CPU core-seconds, within tolerance of an independent `getrusage` reference, on SQLite, CLI and the registered bridge; 2. **HOST** core-seconds are a metric of the job, distinct from the host CPU percentage of the separate snapshot read. Both: arc-desktop `job_meter_acceptance::calibration::at_mtr_01_two_workers_ten_seconds_reaped_descendants_persist_on_all_surfaces`. The rendered dashboard is not part of this evidence. Complete.
- **AT-MTR-02** — 1. **HOST** after a build writes and deletes a large file, Disk Written shows the cumulative bytes; 2. **HOST** Current Footprint shows what remains, as a different value. Not implemented. Pending. Remaining: 1, 2.
- **AT-MTR-03** — 1. **HOST** CPU used by a process of unknown agent ownership is not counted under a vendor and is shown as Unattributed. No production-path test; Partial on desktop fixtures. Partial. Remaining: 1.

### Product decisions, goals, architecture, metrics and risks

- **DEC-010** — 1. **HOST** current and cumulative usage are distinct values: arc-desktop `job_meter_acceptance::calibration::at_mtr_01_two_workers_ten_seconds_reaped_descendants_persist_on_all_surfaces`; 2. **HOST** cost per vendor; 3. **HOST** savings estimates labelled as estimates. Items 2 and 3 are not implemented. Partial. Remaining: 2, 3.
- **DEC-013** — 1. **HOST** class windows: `traffic_cli::measurement::two_real_builds_queue_fifo_and_bypass_remote_and_unknown_while_full`; 2. **HOST** resource reservation: `traffic_cli::diagnostics::persisted_mixed_tests_wait_for_independent_cpu_and_memory_budgets`; 3. **HOST** fairness between projects: `project_fairness::at_trf_08_project_b_is_granted_after_at_most_one_project_a_job`; 4. **HOST** start-rate control (§9.4): `traffic_policy::dec_013_start_rate_bucket_queues_then_grants_without_lost_or_duplicated_permit` (a Build bucket of one token per 10 s set through the startup Traffic configuration holds the next build QUEUED with a free window, then its own poll is granted once after the token time with one permit and one start; no elapsed-time assertion). Fairness beyond projects is TRF-013. Complete.
- **G-01** — 1. **HOST** CPU, duration and outcome per job: arc-desktop `job_meter_acceptance::calibration::at_mtr_01_two_workers_ten_seconds_reaped_descendants_persist_on_all_surfaces`; 2. **HOST** uncertain values are marked: arc-desktop `job_meter_acceptance::calibration::at_mtr_01_two_workers_ten_seconds_reaped_descendants_persist_on_all_surfaces` (`partial`, `unobserved_descendants`); 3. **HOST** usage per vendor, session and project; 4. **HOST** RAM and disk per job. Items 3 and 4 are not implemented. Partial. Remaining: 3, 4.
- **ADR-003** — 1. **HOST** SQLite WAL state survives a forced daemon death: `managed_exec::crash_recovery::forced_daemon_death_after_commit_survives_and_integrity_ok`; 2. **HOST** every event class is in an append-only audit; 3. **HOST** periodic aggregation. No test for 2 or 3. Partial. Remaining: 2, 3.
- **ADR-014** — 1. **HOST** a retried request with one job identity is idempotent: `managed_exec::protocol::retries_response_loss_concurrent_claim_and_completion_are_idempotent`; 2. **HOST** a redelivered vendor hook event is deduplicated by its event key (AT-TRF-09 item 1): `hook_event_identity::at_trf_09_redelivered_claude_event_creates_one_job_and_one_permit`, `hook_event_identity::at_trf_09_redelivered_cursor_event_creates_one_job_and_one_permit`. The key is the domain-separated SHA-256 of (adapter, `session_id`, `tool_use_id`, phase), stored in `job_lifecycle.event_key` under a partial UNIQUE index (migration 0008); each duplicate is one `HOOK_EVENT_DUPLICATE` audit row on the existing job. Complete.
- **MET-001** — 1. **HOST** `host.cpu.utilization` is read as a value from the real daemon: `meter_readings::met_host_cpu_memory_and_swap_values_agree_with_os_readings`. Built `arcctl meter snapshot --json` is read from a real daemon while one `arcctl exec -- cargo build` stub is held RUNNING. The value is read at least three samples after the test's first read (from `sequence`, not a sleep), is Available in percent, lies in 0 to 100, and its busy and idle shares of the reported interval do not exceed the busy and idle ticks the test counts over a window that contains that interval. Tolerance 8 ticks per logical CPU; worst deviation 0 ticks; observed 59.3% to 97.8% (Actor-measured, 5 runs, one macOS arm64 host, debug build). Complete.
- **MET-002** — 1. **HOST** a root process's CPU is observed: arc-desktop `job_meter_acceptance::calibration::at_mtr_01_two_workers_ten_seconds_reaped_descendants_persist_on_all_surfaces`; 2. **HOST** `proc.cpu.utilization` in core-percent per process: `job_process_meter::mtr_002_unwaited_grandchild_cpu_memory_and_io_are_attributed_to_the_job` (`resource_samples.cpu_core_percent` equals the CPU-time growth over the wall interval of consecutive samples of one process, 100 = one core, null on a first sample). Complete.
- **MET-003** — 1. **HOST** `job.cpu.core_seconds` of the waited root and descendants: arc-desktop `job_meter_acceptance::calibration::at_mtr_01_two_workers_ten_seconds_reaped_descendants_persist_on_all_surfaces`; 2. **HOST** descendants that are not waited for: `job_process_meter::mtr_002_unwaited_grandchild_cpu_memory_and_io_are_attributed_to_the_job` (`unwaited_cpu_core_seconds` holds the grandchild's sampled CPU and the job total exceeds the waited rusage). Complete.
- **MET-004** — 1. **HOST** the daemon's RSS is read: `worker_runtime::meter_partial::partial_native_failures_preserve_available_fields_cli_health_and_jobs`, `bounded::native_probe_then_full_100000_attempt_workload`; 2. **HOST** `proc.memory.rss_bytes` of a job's processes: `job_process_meter::mtr_002_unwaited_grandchild_cpu_memory_and_io_are_attributed_to_the_job` (per-process `rss_bytes` rows for the root and the memory-holding grandchild; a grandchild row reads at least the 48 MiB it holds). Complete.
- **MET-005** — 1. **HOST** `job.memory.peak_bytes` of a job: `job_process_meter::mtr_002_unwaited_grandchild_cpu_memory_and_io_are_attributed_to_the_job` (`process_tree.memory_peak_bytes` is the highest RSS summed over one sample, at least the grandchild's RSS reading and at least the 48 MiB it holds). Complete.
- **MET-006** — 1. **HOST** `host.memory.available_bytes` as a value from the real daemon: `meter_readings::met_host_cpu_memory_and_swap_values_agree_with_os_readings`. The value is Available in bytes, lies inside the test's own OS readings taken before and after that sample within the tolerance, and never exceeds physical memory. Tolerance 64 MiB; worst deviation 1,982,464 bytes; observed 2.95 to 3.24 GB (Actor-measured, 5 runs, one macOS arm64 host, debug build). Complete.
- **MET-007** — 1. **HOST** `host.memory.pressure` is read from the OS by the daemon: `worker_runtime::meter_partial::partial_native_failures_preserve_available_fields_cli_health_and_jobs`; 2. **HOST** a Warning or Critical level raised by the OS. Not exercised. Partial. Remaining: 2.
- **MET-008** — 1. **HOST** `host.swap.used_bytes` as a value from the real daemon: `meter_readings::met_host_cpu_memory_and_swap_values_agree_with_os_readings`. The value is Available in bytes, lies inside the test's own readings before and after that sample within the tolerance, and never exceeds total swap. Tolerance 16 MiB; worst deviation 0 bytes; observed 11.163 to 11.172 GB (Actor-measured, 5 runs, one macOS arm64 host, debug build). Swap did not change inside any window, so the tolerance was not stressed. Complete.
- **MET-009** — 1. **HOST** `proc.io.read_bytes`: `job_process_meter::mtr_002_unwaited_grandchild_cpu_memory_and_io_are_attributed_to_the_job` (per-process `disk_read_bytes`, summed into `process_tree.disk_read_bytes`; the grandchild's reading and the report are each at least the 4 MiB it read back uncached). Complete.
- **MET-010** — 1. **HOST** `proc.io.write_bytes`: `job_process_meter::mtr_002_unwaited_grandchild_cpu_memory_and_io_are_attributed_to_the_job` (per-process `disk_write_bytes`; the grandchild's reading is at least the bytes it wrote). Complete.
- **MET-013** — 1. **HOST** a job's run time is recorded in seconds on every surface: arc-desktop `job_meter_acceptance::calibration::at_mtr_01_two_workers_ten_seconds_reaped_descendants_persist_on_all_surfaces`; 2. **HOST** the named `job.duration_ms` projection: `usage_summary::mtr_004_vendor_project_and_period_summaries_match_recorded_jobs` (each group's `job.duration_ms` equals the recorded `run_elapsed` of its job's Meter report; a job whose best-effort report was not recorded reads `null` with `no_recorded_report`, and at least one report is required). Complete.
- **MET-014** — 1. **HOST** `job.queue_wait`, QUEUED to RESERVED, is recorded apart from run time: arc-desktop `job_meter_acceptance::lifecycle::real_queue_wait_is_separate_from_monotonic_running_duration`, arc-desktop `job_meter_acceptance::queue::production_queue_accounting_reconciles_all_surfaces`. Complete.
- **MET-015** — 1. **HOST** a job's class and outcome are recorded: `traffic_cli::terminal_outcomes::scheduled_exit_17_persists_failed_across_restart`; 2. **HOST** `job.count` by class and outcome: `usage_summary::mtr_004_vendor_project_and_period_summaries_match_recorded_jobs` (`by_class_outcome` per group, local and remote, equals the SQLite recompute). Complete.
- **MET-016** — 1. **HOST** the number of active permits is recorded and returns to zero: high-water 1 then 0 in `agent_test_journey::at_trf_05_nested_hook_test_shares_the_root_test_permit`, and 1 held, then 0, in `project_fairness::at_trf_08_project_b_is_granted_after_at_most_one_project_a_job`. Complete.
- **MET-017** — 1. **HOST** queued entries are recorded and counted from daemon state: `project_fairness::at_trf_08_project_b_is_granted_after_at_most_one_project_a_job` (101 queued); 2. **HOST** `traffic.queue.depth` as a reported metric: `usage_summary::met_017_snapshot_reports_traffic_queue_depth_while_build_permit_is_held` (`arcctl meter snapshot --json` reports `traffic.queue_depth` 2 while a held Build permit keeps two requests QUEUED, equal to SQLite, and 0 after release; a protocol-1 read stays v1). Complete.
- **MET-024** — 1. **HOST** the daemon's CPU time is measured from the OS: `phase0_targets::nfr_001_idle_daemon_cpu_below_half_percent_of_one_core`; 2. **HOST** `arc.daemon.cpu.core_seconds` as reported by the daemon is checked against it: `meter_readings::met_daemon_cpu_seconds_and_peak_memory_agree_with_os_readings`. Each reported value is Available in core-seconds, never decreases, and lies between the kernel's CPU time for the daemon process (`proc_pidinfo`) read before and after that sample. Tolerance 10,000 ns (microsecond truncation); worst deviation 0 ns; observed 0.054 to 0.070 core·s (Actor-measured, 5 runs, one macOS arm64 host, debug build). Complete.
- **MET-025** — 1. **HOST** the daemon's RSS and lifetime peak are measured from the OS: `bounded::native_probe_then_full_100000_attempt_workload`; 2. **HOST** `arc.daemon.memory.peak_bytes` as reported by the daemon is checked against it: `meter_readings::met_daemon_cpu_seconds_and_peak_memory_agree_with_os_readings`. The reported peak is Available in bytes, is never below the reported RSS, never decreases across snapshots and never exceeds the OS lifetime peak of the daemon process read by `wait4` at stop. The reported peak is a running maximum of the daemon's own RSS samples, so no two-sided tolerance applies; worst deviation 0 bytes; observed 8.44 to 12.34 MB, 114,688 to 131,072 bytes below the OS lifetime peak (Actor-measured, 5 runs, one macOS arm64 host, debug build). In the Actor's runs the peak equalled RSS in every snapshot; in this integration's run it lay 2,195,456 bytes above RSS for three snapshots, so a peak above current RSS was exercised. Complete.
- **MET-026** — 1. **HOST** `arc.hook.overhead_ms` is recorded by the product: `usage_summary::mtr_004_vendor_project_and_period_summaries_match_recorded_jobs` (the installed hook's own measured time is stored on the Claude Code and Cursor jobs as written by the hook, non-negative, and absent for the direct and gh jobs). Its basis is the hook process's own time, not the NFR-003 end-to-end figure that `phase0_targets::nfr_003_installed_hook_bypass_p95_overhead_within_50_ms` measures from outside. Complete.
- **MET-027** — 1. **HOST** `attribution.unattributed_ratio` is computed from native processes. No production-path test; Partial on the desktop completeness fixtures. Partial. Remaining: 1.
- **R-003** — 1. **HOST** the hook answers at once and never waits in a queue, also while the window is full: `phase0_targets::nfr_003_installed_hook_bypass_p95_overhead_within_50_ms`, `agent_test_journey::at_trf_05_nested_hook_test_shares_the_root_test_permit`; 2. **HOST** the wait happens in the managed process the hook returned: `managed_exec::capture_integration::closing_exec::stages::mixed_bun_stages_via_hook`, `managed_exec::capture_integration::closing_locality::local_container::local_container_buildx_via_hook_exec`; 3. **HOST** a failed hook or an unreachable daemon blocks nothing: `agent_test_journey::at_trf_06_installed_hook_cargo_test_fails_open_exactly_once`; 4. **EXTERNAL** a queued wait observed against a live vendor's tool timeout. Blocked; only item 4 remains.
- **R-005** — 1. **HOST** job CPU reports state their basis, scope and completeness: arc-desktop `job_meter_acceptance::calibration::at_mtr_01_two_workers_ten_seconds_reaped_descendants_persist_on_all_surfaces`; 2. **HOST** Unattributed usage is shown; 3. **HOST** every other metric is calibrated. No test for 2 or 3. Partial. Remaining: 2, 3.
- **R-014** — 1. **HOST** one writer stays correct under backlog and a busy database: `traffic_cli::robustness::queued_cli_waits_through_real_sqlite_busy_then_starts_after_release`, `project_fairness::at_trf_08_project_b_is_granted_after_at_most_one_project_a_job` (101 waiters, zero DB rejections); 2. **HOST** event coalescing; 3. **HOST** rollups under sustained load. No test for 2 or 3. Partial. Remaining: 2, 3.
