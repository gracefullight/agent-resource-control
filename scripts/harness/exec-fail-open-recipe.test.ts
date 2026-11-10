import { expect, test } from "bun:test";
import { fixtureFirstRun } from "./exec-fail-open-fixture";
import { validateGateData } from "./exec-fail-open-gates";
import { requiredTests } from "./exec-fail-open-recipe";

const nativeRegressions = [
  "fallback_preserves_inherited_ignored_sigpipe",
  "fallback_preserves_inherited_default_sigpipe",
  "full_stderr_unavailable_does_not_block_supervision",
  "full_stderr_uncertain_does_not_block_supervision",
  "full_stderr_managed_completion_failure_does_not_block_exit",
  "full_stderr_directed_signal_preserves_native_status",
];

test("workspace gate requires each signal and full-stderr regression", () => {
  for (const name of nativeRegressions)
    expect(requiredTests.filter(actual => actual === name)).toHaveLength(1);
  fixtureFirstRun((dir, _refs, fixture) => {
    for (const name of nativeRegressions) {
      const recipe = { ...fixture.recipe, requiredTests: [name] };
      expect(() => validateGateData(dir, fixture.ready.gates, undefined, recipe))
        .toThrow(`missing live execution: ${name}`);
    }
  });
}, 3600000);

test("workspace gate requires peer and endpoint replacement regressions", () => {
  const regressions = [
    "committed_register_ack_loss_endpoint_replacement_runs_once",
    "committed_claim_ack_loss_endpoint_replacement_runs_once",
    "confirmed_register_endpoint_replacement_before_claim_runs_once",
    "sent_request_then_invalid_endpoint_keeps_uncertainty",
    "confirmed_register_then_invalid_endpoint_is_uncertain_but_local_failure_stops",
    "local_setup_failure_after_send_still_stops",
    "same_user_os_peer_credentials_are_accepted",
    "untrusted_peer_receives_no_request_and_cannot_reject",
    "cancellation_overrides_invalid_endpoint_after_send",
    "malformed_addresses_are_invalid_endpoints_in_preparation",
    "overlong_socket_configuration_does_not_launch",
  ];
  for (const name of regressions)
    expect(requiredTests.filter(actual => actual === name)).toHaveLength(1);
  fixtureFirstRun((dir, _refs, fixture) => {
    for (const name of regressions) {
      const recipe = { ...fixture.recipe, requiredTests: [name] };
      expect(() => validateGateData(dir, fixture.ready.gates, undefined, recipe))
        .toThrow(`missing live execution: ${name}`);
    }
  });
}, 3600000);

test("workspace gate requires fixed historical and separate live transport checks", () => {
  const regressions = [
    "fixed_historical_transport_and_pinned_tests_match_baseline_hashes",
    "live_unavailable_uncertain_and_idempotent_retry_keep_distinct_decisions",
    "live_db_busy_exhaustion_is_uncertain_but_explicit_errors_are_nonlaunching",
    "live_complete_invalid_frames_and_envelopes_are_not_outages",
    "live_absolute_deadline_is_shared_and_partial_reads_cannot_renew_it",
    "live_cancellation_is_observed_while_waiting_and_at_expired_admission",
  ];
  for (const name of regressions)
    expect(requiredTests.filter(actual => actual === name)).toHaveLength(1);
  fixtureFirstRun((dir, _refs, fixture) => {
    for (const name of regressions) {
      const recipe = { ...fixture.recipe, requiredTests: [name] };
      expect(() => validateGateData(dir, fixture.ready.gates, undefined, recipe))
        .toThrow(`missing live execution: ${name}`);
    }
  });
}, 3600000);
