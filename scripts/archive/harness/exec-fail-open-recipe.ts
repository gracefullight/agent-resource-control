//! Leaf observations vary in an isolated fixture; assembly and acceptance flow do not.
//! CLI entrypoints always use productionRecipe. No environment selects a fixture.
import { context, validateCommon, validateIndex } from "./exec-fail-open-inputs";
import { validateCommand, prerequisiteReceipts } from "./exec-fail-open-receipts";
import { validateTiming, validateHistorical, scenarios } from "./exec-fail-open-native";
import { checkFailOpenPreservation } from "./exec-fail-open-preservation";
import { nativeCollection } from "./exec-fail-open-coverage";
export const requiredTests = [
  "fallback_preserves_inherited_ignored_sigpipe",
  "fallback_preserves_inherited_default_sigpipe",
  "full_stderr_unavailable_does_not_block_supervision",
  "full_stderr_uncertain_does_not_block_supervision",
  "full_stderr_managed_completion_failure_does_not_block_exit",
  "full_stderr_directed_signal_preserves_native_status",
  "broken_stderr_unavailable_preserves_child_exit",
  "broken_stderr_uncertain_preserves_child_exit",
  "outages_preserve_native_bytes_exit17_one_invocation_budget_and_no_replay",
  "rejection_invalid_reply_arguments_and_local_config_never_launch",
  "cancellation_during_register_claim_and_near_deadline_never_launches",
  "directed_signals_preserve_native_status_during_fallback",
  "historical_arcctl_fails_live_fallback_assertions",
  "fallback_foreground_pipeline_input_signals_stop_continue_and_exit17",
  "fallback_preserves_closed_standard_and_extra_inherited_descriptors",
  "cancellation_at_spawn_admission_cannot_invoke_spawn",
  "absolute_deadline_is_shared_and_partial_reads_cannot_renew_it",
  "fixed_historical_transport_and_pinned_tests_match_baseline_hashes",
  "live_unavailable_uncertain_and_idempotent_retry_keep_distinct_decisions",
  "live_db_busy_exhaustion_is_uncertain_but_explicit_errors_are_nonlaunching",
  "live_complete_invalid_frames_and_envelopes_are_not_outages",
  "live_absolute_deadline_is_shared_and_partial_reads_cannot_renew_it",
  "live_cancellation_is_observed_while_waiting_and_at_expired_admission",
  "every_outage_preserves_the_complete_native_environment_byte_for_byte",
  "shared_job_terminal_cancellation_before_attachment_blocks_spawn_admission",
  "shared_job_terminal_signals_between_spawn_and_attach_arrive_once",
  "shared_job_terminal_signals_after_admission_before_fork_cancel_execution",
  // libtest includes this annotation in the identity parsed by the maintained helper.
  "isolated_rejects_an_empty_exact_selection - should panic",
  "fallback_preserves_inherited_ignored_and_blocked_sigchld",
  "connect_access_denied_before_register_runs_once",
  "connect_access_denied_after_register_ack_loss_stays_uncertain",
  "connect_access_denied_before_claim_stays_uncertain",
  "connect_access_denied_after_claim_ack_loss_stays_uncertain",
  "invalid_socket_parent_not_directory_does_not_launch",
  "invalid_socket_endpoint_not_socket_does_not_launch",
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
export function productionRecipe(root: string) {
  return {
    root, context: () => context(root), common: validateCommon,
    command: (r: any, ctx: any, actor = "backend") => validateCommand(r, ctx, root, actor),
    prerequisites: (gates: any, ctx: any) => prerequisiteReceipts(root, gates, ctx),
    timing: (t: any, ctx: any, commands: any[]) => validateTiming(t, ctx, root, commands),
    historical: (h: any, ctx: any, commands: any[]) => validateHistorical(h, ctx, root, commands),
    preservation: () => checkFailOpenPreservation(root),
    collection: (runId: string) => nativeCollection(root, runId),
    index: (required: string[], count: number) => validateIndex(root, required, count),
    scenarios, requiredTests, workspaceArgv: ["cargo", "test", "--workspace"],
    terminalPrefix: "fallback_terminal::", terminalCount: 4,
    preservationArgument: "scripts/harness/exec-fail-open-preservation.test.ts",
  };
}
export type EvidenceRecipe = Omit<ReturnType<typeof productionRecipe>, "preservation"> & { preservation: () => { passed: boolean } };
