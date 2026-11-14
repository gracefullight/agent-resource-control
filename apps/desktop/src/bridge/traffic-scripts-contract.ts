import { isJobId, isOutcome } from "@/bridge/client";
import { isRecordedDecision } from "@/bridge/policy-contract";
import type {
  AdmissionDiagnostic,
  DiagnosticResult,
  ScriptPolicyLimits,
  TrafficClassification,
} from "@/bridge/traffic-scripts.generated";

function object(
  value: unknown,
  keys: readonly string[],
): value is Record<string, unknown> {
  return (
    value !== null &&
    typeof value === "object" &&
    !Array.isArray(value) &&
    Object.keys(value).length === keys.length &&
    keys.every((key) => Object.hasOwn(value, key))
  );
}
const integer = (value: unknown, min = 0): value is number =>
  typeof value === "number" && Number.isSafeInteger(value) && value >= min;
const nullableMs = (value: unknown): value is number | null =>
  value === null || integer(value);

// Mirrors Classification::is_consistent. The exhaustive Rust-derived fixtures
// test the complete accepted combinations, rather than accepting enum names
// independently and accidentally combining a local class with a bypass reason.
const reasons: Record<
  TrafficClassification["classification"],
  readonly string[]
> = {
  BUILD: ["CARGO_BUILD", "BUN_SCRIPT_BUILD", "COMPOUND_LOCAL"],
  TEST: ["CARGO_TEST", "BUN_TEST", "BUN_SCRIPT_TEST", "COMPOUND_LOCAL"],
  LINT: ["CARGO_CLIPPY", "CARGO_FMT", "BUN_SCRIPT_LINT", "COMPOUND_LOCAL"],
  TYPECHECK: ["BUN_SCRIPT_TYPECHECK", "COMPOUND_LOCAL"],
  REMOTE_CI: [
    "GH_WORKFLOW_RUN",
    "GH_RUN_WATCH",
    "GH_RUN_VIEW",
    "COMPOUND_REMOTE",
  ],
  REMOTE_EXEC: ["SSH_REMOTE_COMMAND", "COMPOUND_REMOTE"],
  MIXED: ["COMPOUND_MIXED"],
  UNKNOWN: [
    "EMPTY_COMMAND",
    "UNSUPPORTED_SHELL",
    "UNSUPPORTED_SYNTAX",
    "UNRESOLVED_COMMAND",
    "UNRESOLVED_SCRIPT",
    "UNRESOLVED_EXPANSION",
    "UNRESOLVED_COMPOUND",
    "SSH_FORWARDING",
    "SSH_AMBIGUOUS",
    "DOCKER_ENDPOINT_UNKNOWN",
    "INPUT_LIMIT",
    "TOKEN_LIMIT",
    "INVOCATION_LIMIT",
    "NESTING_LIMIT",
    "WORK_LIMIT",
  ],
};
export function isTrafficClassification(
  value: unknown,
): value is TrafficClassification {
  if (
    !object(value, ["classification", "locality", "reason", "uncertainty"]) ||
    typeof value.classification !== "string" ||
    !Object.hasOwn(reasons, value.classification) ||
    typeof value.reason !== "string"
  )
    return false;
  const key = value.classification as TrafficClassification["classification"];
  const unknown = key === "UNKNOWN";
  const locality = unknown
    ? "UNKNOWN"
    : key === "MIXED"
      ? "MIXED"
      : key === "REMOTE_CI" || key === "REMOTE_EXEC"
        ? "REMOTE"
        : "HOST_LOCAL";
  return (
    reasons[key].includes(value.reason) &&
    value.locality === locality &&
    value.uncertainty === (unknown ? "OBSERVE" : "NONE")
  );
}
function resources(value: unknown): value is ScriptPolicyLimits["budget"] {
  return (
    object(value, ["cpu_millis", "memory_bytes"]) &&
    integer(value.cpu_millis, 1) &&
    integer(value.memory_bytes, 1)
  );
}
export function isScriptPolicyLimits(
  value: unknown,
): value is ScriptPolicyLimits {
  if (
    !object(value, ["build", "test", "lint", "typecheck", "budget"]) ||
    !resources(value.budget)
  )
    return false;
  for (const key of ["build", "test", "lint", "typecheck"] as const) {
    const limit = value[key];
    if (
      !object(limit, ["window", "reservation"]) ||
      !integer(limit.window, 1) ||
      limit.window > 4_294_967_295 ||
      !resources(limit.reservation) ||
      limit.reservation.cpu_millis > value.budget.cpu_millis ||
      limit.reservation.memory_bytes > value.budget.memory_bytes
    )
      return false;
  }
  return true;
}
export function isAdmissionDiagnostic(
  value: unknown,
): value is AdmissionDiagnostic {
  if (
    !object(value, [
      "state",
      "sequence",
      "queued_at_ms",
      "granted_at_ms",
      "queue_wait_ms",
      "wait_reason",
      "last_decision",
      "root_job_id",
      "permit_generation",
      "reservation",
      "permit_active",
      "outcome",
    ]) ||
    !integer(value.sequence, 1) ||
    !integer(value.queued_at_ms) ||
    !nullableMs(value.granted_at_ms) ||
    !nullableMs(value.queue_wait_ms) ||
    (value.granted_at_ms !== null &&
      value.granted_at_ms < value.queued_at_ms) ||
    (value.granted_at_ms !== null &&
      value.queue_wait_ms !== null &&
      value.granted_at_ms - value.queued_at_ms !== value.queue_wait_ms) ||
    !(value.root_job_id === null || isJobId(value.root_job_id)) ||
    !(
      value.permit_generation === null || integer(value.permit_generation, 1)
    ) ||
    !(value.reservation === null || resources(value.reservation)) ||
    typeof value.permit_active !== "boolean" ||
    !(value.outcome === null || isOutcome(value.outcome)) ||
    !(value.last_decision === null || isRecordedDecision(value.last_decision))
  )
    return false;
  const permit =
    value.root_job_id !== null &&
    value.permit_generation !== null &&
    value.reservation !== null;
  if (
    ((value.root_job_id !== null ||
      value.permit_generation !== null ||
      value.reservation !== null) &&
      !permit) ||
    (value.permit_active && !permit)
  )
    return false;
  if (
    value.wait_reason !== null &&
    (value.state !== "QUEUED" ||
      ![
        "memory_pressure",
        "fifo",
        "class_window",
        "cpu_budget",
        "memory_budget",
      ].includes(
        typeof value.wait_reason === "string" ? value.wait_reason : "",
      ) ||
      value.last_decision?.kind !== "wait" ||
      value.last_decision.reason !== value.wait_reason)
  )
    return false;
  switch (value.state) {
    case "QUEUED":
      return (
        !value.permit_active &&
        !permit &&
        value.granted_at_ms === null &&
        value.queue_wait_ms === null &&
        value.outcome === null
      );
    case "RESERVED":
      return (
        value.permit_active &&
        value.granted_at_ms !== null &&
        value.outcome === null
      );
    case "RUNNING":
      return value.queue_wait_ms !== null && value.outcome === null;
    case "COMPLETED":
      return value.queue_wait_ms !== null && value.outcome !== null;
    case "CANCELLED":
    case "ABANDONED":
    case "UNCONFIRMED":
      return value.outcome === null;
    case "UNRESERVED":
      return !value.permit_active && value.outcome === null;
    default:
      return false;
  }
}
export function isTrafficDiagnosticResult(
  value: unknown,
  jobId?: string,
): value is DiagnosticResult {
  if (
    !object(value, [
      "schema_version",
      "job_id",
      "recorded_at_ms",
      "liveness_fresh",
      "classification",
      "admission",
      "limits",
    ]) ||
    value.schema_version !== 1 ||
    !isJobId(value.job_id) ||
    (jobId !== undefined && value.job_id !== jobId) ||
    !integer(value.recorded_at_ms) ||
    value.liveness_fresh !== false ||
    !(
      value.classification === null ||
      isTrafficClassification(value.classification)
    ) ||
    !(value.admission === null || isAdmissionDiagnostic(value.admission)) ||
    !isScriptPolicyLimits(value.limits)
  )
    return false;
  const admission = value.admission;
  if (
    admission !== null &&
    (value.classification?.locality !== "HOST_LOCAL" ||
      admission.queued_at_ms > value.recorded_at_ms ||
      (admission.queue_wait_ms !== null &&
        admission.queue_wait_ms >
          value.recorded_at_ms - admission.queued_at_ms) ||
      (admission.granted_at_ms !== null &&
        admission.granted_at_ms > value.recorded_at_ms) ||
      (admission.last_decision !== null &&
        admission.last_decision.decided_at_ms > value.recorded_at_ms))
  )
    return false;
  try {
    return new TextEncoder().encode(JSON.stringify(value)).length <= 8192;
  } catch {
    return false;
  }
}
