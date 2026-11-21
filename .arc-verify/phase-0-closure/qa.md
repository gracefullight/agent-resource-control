# Phase 0 closure: independent lane-snapshot QA

**Product review: FAIL; verification: partial.** Task-5 owns findings; report acceptance is independent. `oma-qa` was used. Task-5 changes are outside this review.

## Scope and provenance

Accepted baseline commit `566db287c54320a49692ac238220b180f21787fe`, tree `b7622ba1c876b8cc4680358acbd4857ac304cce8` (verified). Reviewed snapshot HEAD `808eac9aad9f0fad3a4f851bffc7c5107a792ad6`, tree `35658d5ec45e8c584baeecb101b231bad1b5e4fd`. Its 46-file lane overlay is not in the baseline. Snapshot and worker refs did not resolve at inspection. Before this report, scoped status was empty; global status printed `av.yaml: Operation not permitted`.

Sources: SRS `docs/requirements/ARC_SRS_2026-11-06.md` §§8.2, 8.5, 8.7, 17.3, 18, 21.2 and AT-SWP-20; `docs/requirements/closing.md`; `.arc-verify/phase-0-closure/contract.md` (6,841 bytes; four supplied commands once each).

Paths: `T/` = `crates/arc-daemon/tests/managed_exec/capture_integration/`; `C/` = `crates/arc-cli/`; `D/` = `crates/arc-detector/src/`. Task-3 review reports its hook, deadline, outage and Bash tests passed on delivery tree `48aacadbb1ddfbe0ebf1351d9b216b23f3f800be`; task-4 reports 11 shell tests and a detector test passed on its integrated input. Receipts are not QA reruns.

## HIGH — closure gaps

1. **Zsh execution.** `closing.md:144-157` requires Bash/zsh inspection and execution. `T/closing_shell.rs:122-153,193-229` inspects both but executes general cases only via `sh -c`. The gh hook test (`T/closing_exec.rs:68-81`) does not cover general direct, pipeline and compound workloads. Native expansion at `D/locality.rs:380-388` and `C/src/exec/metadata.rs:104-113` explicitly handles `sh -c`. **Fix:** test the required cases through zsh managed execution and assert classification, child and admission; wire the safe path without borrowing startup-sensitive facts.

2. **Positive local CI.** SRS §8.5 lines 724-731 expects local `act -j build` and a same-PC self-hosted runner, and LOCAL_VM for an actual WSL workload. `closing.md:58-65` and `T/closing_locality.rs:240-246,284-286,303-314` only test unproved stubs as UNKNOWN. **Fix:** enumerate applicable positive cases and test proven placement and admission through both production routes; use EXTERNAL only for genuinely unavailable environments.

3. **Nested sh.** `T/closing_shell.rs:322-333` inspects `sh -c 'cargo test'` but executes `cargo test` in the outer sh. `T/closing_exec.rs:430-454` tests direct argv, not the extra nested shell. **Fix:** execute the inspected nested source and assert daemon classification and child outcome.

4. **Mixed commands.** `closing.md:54-69,181-186` requires MIXED on both routes. `T/closing_shell.rs:398-420` asserts bun inspection but only prints execution classification; `:424-445` expects UNKNOWN for Make. `D/locality/shell_closure_tests.rs:83-117` injects a Make fact. `T/closing_locality.rs:137-165` requires MIXED for pnpm; task-2 reports UNKNOWN/UNRESOLVED_SCRIPT on both routes, failing in suite and alone. `C/src/exec/capture.rs:6-19` captures pnpm facts but `C/src/exec/metadata.rs:59-67` limits script analysis to Bun. **Fix:** capture/revalidate Makefile and resolve literal pnpm; assert MIXED for Make, bun and pnpm on both routes without weakening dynamic controls.

5. **Kubernetes split.** `closing.md:8-10,48-50,85-87,113-119` still claims no EXTERNAL item and treats proven placement as HOST. `C/src/exec/endpoint_context/kube.rs:40-111,157-193` captures selection, not pod placement. The registered test `T/closing_locality.rs:53-56` invokes `T/closing_locality/kube_controls.rs:3-77`: five operations are UNKNOWN/OBSERVE with one child and no local admission. **Fix:** split DET-005 item 6 and SAFE-008/NFR-007 references into HOST without placement proof and EXTERNAL with proven remote placement, per the operator. Rerun on the final tree; synthetic pod facts in `crates/arc-daemon/tests/managed_exec/remote_endpoints.rs:44-48` are not live proof.

## MEDIUM — follow-ups

6. **Isolation.** `T/endpoints/kube.rs:26,267` uses `f.spawn`, which inherits the runner environment (`crates/arc-daemon/tests/support/runtime.rs:26-49`; `support/native_fixture.rs:45-59`). Other calls include `T/endpoints.rs:91` and `T/held.rs:18`. **Fix:** use `spawn_isolated` for applicable mission tests and rerun with shared instrumentation forwarding.

7. **Traceability and counts.** `docs/requirements/traceability.md:13,139,143,276,295,327` still says hook pass-through and Partial selected rows. `closing.md:211-231` reports a four-case built-CLI baseline (1 remote, 3 UNKNOWN, false-local 0 per route), separate from 42 simulated cases (42 remote, 0 UNKNOWN, false-local 0 per detector route). Task-2 reports a 10-case subset (9 remote, 1 UNKNOWN, false-local 0 per route); identify its membership. `T/closing_locality.rs:170-226` defines a six-case remote-only test. **Fix:** reconcile Phase 0 lists/counts, name final distinct production cases and passing tests, and report their denominator separately from the preserved 42 without doubling routes or overlaps.

8. **Tracing.** The task-3 review reported extra stderr trace lines with zsh `set -x`, with matching stdout and exit. `C/src/hooks.rs:242-337` still introduces shell branches; QA did not reproduce this observation. **Fix:** compare direct and installed-hook tracing and preserve stderr or pass through if uncertain. Builtin/keyword redefinitions are excluded by the operator; ordinary gh aliases/functions are not (`T/closing_exec/regressions.rs:14-77,79-205,208-347`).

## Evidence boundaries

The installed Claude hook test uses a derived event and asserts no permission decision (`T/closing_exec.rs:124-198`). It asserts a held Build permit, unchanged admission counts, one offline stub PID, the REMOTE_CI record and native argv/cwd/environment/streams/exit (`:142-160,250-306`). Build-rewrite controls remain (`crates/arc-daemon/tests/managed_exec/remote_ci.rs:161-188`). Simulated agent execution; no live vendor/GitHub acceptance.

`C/src/exec/run.rs:58-68` starts the preparation deadline after classification; `C/tests/fixtures/child.rs:75-103` injects three seconds around capture in the same source path. `T/closing_exec.rs:314-357,415-428` asserts child recording and genuine-outage fail-open. Bash BASH_ENV and login-profile cases cover both routes (`T/closing_shell.rs:3-96`, `T/closing_exec.rs:360-412`). These are source assertions and the supplied receipt, not a fresh QA run.

Remote Docker/SSH held-permit assertions are at `T/closing_locality.rs:324-405`; local listener and SSH controls at `T/closing_locality/docker_controls.rs:152-265`. Socket inspection uses metadata and process/FD tables without connecting (`C/src/exec/endpoint_context/docker/socket.rs:8-34,130-150`). AT-TRF-12 still needs stage-by-stage local-Test control and remote dispatch; AT-SWP-20 needs both remote-context switch and named-volume protection. Classification completes neither.

## Checks and gaps

`git rev-parse HEAD HEAD^{tree}` and the baseline tree check passed; `git diff --check 566db287c54320a49692ac238220b180f21787fe HEAD -- . ':!av.yaml'` passed. `git check-ignore -v .arc-verify/phase-0-closure/qa.md` returned 1 with no match. A Python check found four contract commands, each once, with HOST/EXTERNAL. Serena was unavailable; named files were read directly.

QA ran no native build gates, tests, coverage, harness or screen checks. This checkout lacks `target/debug/{arcctl,arc-daemon,arc-exec-fixture}` and a `managed_exec` test binary; a binary in another workspace was not used. The runner must perform final contract, coverage, traceability and architecture checks, including post-ruling Docker/Kubernetes runs. Earlier coverage accounting flagged `C/src/exec/metadata/tests/sh_fallback.rs`; rerun its inventory gate. The probe-timeout test failed a full run and passed alone twice; the operator later removed that probe and its test. No live acceptance is claimed.
