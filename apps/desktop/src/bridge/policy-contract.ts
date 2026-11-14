import { processIdentity } from "@/bridge/meter-contract";
import {
  type EffectivePolicy,
  type JobPolicyResult,
  L0_PROFILE,
  POLICY_VERSION,
  type PolicyObservation,
  type RecordedDecision,
} from "@/bridge/policy.types";

const integer = (v: unknown, min = 0): v is number =>
  typeof v === "number" && Number.isSafeInteger(v) && v >= min;
const nullable = (v: unknown): v is number | null => v === null || integer(v);
function object(
  v: unknown,
  keys: readonly string[],
): v is Record<string, unknown> {
  return (
    v !== null &&
    typeof v === "object" &&
    !Array.isArray(v) &&
    Object.keys(v).length === keys.length &&
    keys.every((k) => Object.hasOwn(v, k))
  );
}
function bounded(v: unknown): boolean {
  try {
    return new TextEncoder().encode(JSON.stringify(v)).length <= 16_384;
  } catch {
    return false;
  }
}
export function isPolicyObservation(v: unknown): v is PolicyObservation {
  if (
    !object(v, [
      "availability",
      "pressure",
      "age_ms",
      "stale_after_ms",
      "sampler",
      "sequence",
      "observed_at_ms",
      "observed_monotonic_ms",
    ]) ||
    typeof v.availability !== "string" ||
    ![
      "available",
      "missing",
      "stale",
      "failed",
      "unsupported",
      "denied",
      "invalid",
    ].includes(v.availability) ||
    !(
      v.pressure === null ||
      (typeof v.pressure === "string" &&
        ["normal", "warning", "critical"].includes(v.pressure))
    ) ||
    ![
      v.age_ms,
      v.stale_after_ms,
      v.observed_at_ms,
      v.observed_monotonic_ms,
    ].every(nullable) ||
    !(v.sequence === null || integer(v.sequence, 1)) ||
    !(
      v.sampler === null ||
      (object(v.sampler, [
        "daemon_instance",
        "generation",
        "monotonic_epoch",
      ]) &&
        processIdentity(v.sampler.daemon_instance) &&
        integer(v.sampler.generation, 1) &&
        integer(v.sampler.monotonic_epoch, 1))
    )
  )
    return false;
  if (v.availability === "available" || v.availability === "stale")
    return (
      v.pressure !== null &&
      v.sampler !== null &&
      v.sequence !== null &&
      v.observed_monotonic_ms !== null &&
      integer(v.age_ms) &&
      integer(v.stale_after_ms, 1) &&
      v.age_ms > v.stale_after_ms === (v.availability === "stale")
    );
  if (v.availability === "missing")
    return v.pressure === null && v.age_ms === null && v.sequence === null;
  return v.pressure === null;
}
function resources(v: unknown, cpu: number, memory: number): boolean {
  return (
    object(v, ["cpu_millis", "memory_bytes"]) &&
    v.cpu_millis === cpu &&
    v.memory_bytes === memory
  );
}
function limits(v: unknown): boolean {
  if (!object(v, ["build", "test", "lint", "budget"])) return false;
  for (const [key, cpu, bytes] of [
    ["build", 2000, 2147483648],
    ["test", 1000, 1073741824],
    ["lint", 1000, 1073741824],
  ] as const) {
    const c = v[key];
    if (
      !object(c, ["window", "reservation"]) ||
      c.window !== 1 ||
      !resources(c.reservation, cpu, bytes)
    )
      return false;
  }
  return resources(v.budget, 2000, 4294967296);
}
export function isEffectivePolicy(v: unknown): v is EffectivePolicy {
  if (
    !object(v, [
      "schema_version",
      "policy_version",
      "profile",
      "mode",
      "reason",
      "observation",
      "limits",
    ]) ||
    v.schema_version !== 1 ||
    v.policy_version !== POLICY_VERSION ||
    v.profile !== L0_PROFILE ||
    !isPolicyObservation(v.observation) ||
    !limits(v.limits) ||
    !bounded(v)
  )
    return false;
  const o = v.observation;
  if (o.availability === "available") {
    return (
      v.mode === (o.pressure === "critical" ? "memory_hold" : "l0") &&
      v.reason ===
        (o.pressure === "critical"
          ? "memory_critical"
          : `pressure_${o.pressure}`)
    );
  }
  return (
    v.mode === "l0" &&
    (o.availability === "invalid"
      ? ["observation_invalid", "sampler_mismatch", "clock_invalid"].includes(
          typeof v.reason === "string" ? v.reason : "",
        )
      : v.reason === `observation_${o.availability}`)
  );
}
export function isRecordedDecision(v: unknown): v is RecordedDecision {
  if (
    !object(v, [
      "policy_version",
      "kind",
      "reason",
      "decided_at_ms",
      "permit_generation",
      "observation",
    ]) ||
    v.policy_version !== POLICY_VERSION ||
    !integer(v.decided_at_ms) ||
    !(v.permit_generation === null || integer(v.permit_generation, 1)) ||
    !isPolicyObservation(v.observation)
  )
    return false;
  const reasons: Record<string, string[]> = {
    admit: ["admitted"],
    wait: [
      "memory_pressure",
      "fifo",
      "class_window",
      "cpu_budget",
      "memory_budget",
    ],
    reuse: ["existing_reservation"],
    preserve: ["running"],
    bypass: ["remote", "unknown", "unsupported"],
    cancel: ["cancelled"],
  };
  const critical =
    v.observation.availability === "available" &&
    v.observation.pressure === "critical";
  return (
    typeof v.kind === "string" &&
    typeof v.reason === "string" &&
    Object.hasOwn(reasons, v.kind) &&
    (reasons[v.kind]?.includes(v.reason) ?? false) &&
    (v.kind !== "admit" || (v.permit_generation !== null && !critical)) &&
    (v.reason !== "memory_pressure" || critical)
  );
}
export function isJobPolicyResult(
  v: unknown,
  jobId?: string,
): v is JobPolicyResult {
  return (
    object(v, [
      "schema_version",
      "job_id",
      "effective",
      "decision_availability",
      "last_decision",
    ]) &&
    v.schema_version === 1 &&
    typeof v.job_id === "string" &&
    /^[0-9a-f]{32}$/.test(v.job_id) &&
    (jobId === undefined || v.job_id === jobId) &&
    isEffectivePolicy(v.effective) &&
    ["available", "not_recorded", "expired", "not_found"].includes(
      typeof v.decision_availability === "string"
        ? v.decision_availability
        : "",
    ) &&
    (v.decision_availability === "available"
      ? isRecordedDecision(v.last_decision)
      : v.last_decision === null) &&
    bounded(v)
  );
}
