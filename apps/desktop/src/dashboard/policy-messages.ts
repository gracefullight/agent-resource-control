import type { DecisionReason, PolicyReason } from "@/bridge/policy.types";

export const policyMessages = {
  heading: "Memory admission",
  waiting: "Waiting for memory",
  recovery:
    "Starts automatically when memory permits; other queue rules still apply.",
  held: "New memory-intensive jobs are waiting for memory.",
  l0: "Initial policy active (L0)",
  stale: "Stale policy · current admission is unconfirmed",
  unsupported: "Memory admission details are not supported by this daemon.",
  unknown: "Memory admission is unknown.",
  retained: "Last known policy retained. Current admission is unconfirmed.",
  historicalHold:
    "Last recorded decision: waiting for memory. This does not confirm a current hold.",
  details: "Policy and observation details",
};
export const policyReasonMessages: Record<PolicyReason, string> = {
  pressure_normal: "Fresh memory reading: Normal",
  pressure_warning: "Fresh memory reading: Warning",
  memory_critical: "Fresh memory reading: Critical",
  observation_missing: "Memory reading unavailable",
  observation_stale: "Memory reading stale",
  observation_failed: "Memory reading failed",
  observation_unsupported: "Memory reading not supported",
  observation_denied: "Memory reading denied",
  observation_invalid: "Memory reading invalid",
  sampler_mismatch: "Memory sampler identity mismatch",
  clock_invalid: "Memory observation clock invalid",
};
export const decisionReasonMessages: Record<DecisionReason, string> = {
  memory_pressure: "Waiting for memory",
  fifo: "Waiting for an earlier job",
  class_window: "Waiting for a class slot",
  cpu_budget: "Waiting for CPU reservation",
  memory_budget: "Waiting for memory reservation",
  admitted: "Permit granted",
  existing_reservation: "Using existing reservation",
  running: "Running job preserved",
  remote: "Remote workload bypass",
  unknown: "Unknown workload bypass",
  unsupported: "Unsupported workload bypass",
  cancelled: "Cancelled",
};
