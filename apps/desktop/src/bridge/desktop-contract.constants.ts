export const PHASES = [
  "REGISTERED",
  "LAUNCHING",
  "RUNNING",
  "COMPLETED",
  "SIGNALED",
  "SPAWN_FAILED",
  "OUTCOME_UNKNOWN",
] as const;
export const REASONS = [
  "NONE",
  "REGISTERED_NOT_CLAIMED",
  "CLAIMED_NOT_ATTACHED",
  "OBSERVER_MISSING_NOT_CLAIMED",
  "OBSERVER_MISSING",
  "PROCESS_MISSING",
  "IDENTITY_MISMATCH",
  "IDENTITY_UNAVAILABLE",
  "CHILD_VERIFIED",
  "OBSERVED_EXIT",
  "OBSERVED_SIGNAL",
  "OBSERVED_SPAWN_FAILURE",
] as const;
export const ERROR_CODES = [
  "unavailable",
  "uncertain",
  "busy",
  "rejected",
  "protocol",
  "invalid_reply",
  "invalid_request",
  "invalid_endpoint",
  "local",
  "cancelled",
  "unsupported",
  "opener_failed",
  "window_unavailable",
] as const;
export const CREDIT_LINKS = ["github", "linkedin", "oma"] as const;
