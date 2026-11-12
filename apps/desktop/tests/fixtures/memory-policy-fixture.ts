import {
  type EffectivePolicy,
  type JobPolicyResult,
  L0_PROFILE,
  POLICY_VERSION,
  type RecordedDecision,
} from "@/bridge/policy.types";
import type { Received } from "@/bridge/types";

// Synthetic schema-1 policy; no OS measurement or production admission claim.
export function memoryPolicy(
  pressure: "normal" | "warning" | "critical" = "critical",
  sequence = 1,
): EffectivePolicy {
  const resources = (cpu_millis: number, memory_bytes: number) => ({
    cpu_millis,
    memory_bytes,
  });
  return {
    schema_version: 1,
    policy_version: POLICY_VERSION,
    profile: L0_PROFILE,
    mode: pressure === "critical" ? "memory_hold" : "l0",
    reason:
      pressure === "critical" ? "memory_critical" : `pressure_${pressure}`,
    observation: {
      availability: "available",
      pressure,
      age_ms: 0,
      stale_after_ms: 3000,
      sampler: {
        daemon_instance: {
          pid: 123,
          start_token: "456",
          boot_id: "00000000-0000-0000-0000-000000000001",
        },
        generation: 1,
        monotonic_epoch: 1,
      },
      sequence,
      observed_at_ms: 10000,
      observed_monotonic_ms: 10000 + sequence,
    },
    limits: {
      build: { window: 1, reservation: resources(2000, 2147483648) },
      test: { window: 1, reservation: resources(1000, 1073741824) },
      lint: { window: 1, reservation: resources(1000, 1073741824) },
      budget: resources(2000, 4294967296),
    },
  };
}
export function memoryJob(
  job_id: string,
  effective = memoryPolicy(),
): JobPolicyResult {
  const last_decision: RecordedDecision = {
    policy_version: POLICY_VERSION,
    kind: "wait",
    reason: "memory_pressure",
    decided_at_ms: 10000,
    permit_generation: null,
    observation: structuredClone(memoryPolicy().observation),
  };
  return {
    schema_version: 1,
    job_id,
    effective,
    decision_availability: "available",
    last_decision,
  };
}
export const policyReply = <T>(
  data: T,
  received_at_ms = 20000,
): Received<T> => ({ data, received_at_ms });

export function fixtureSampler(policy: EffectivePolicy) {
  const sampler = policy.observation.sampler;
  if (!sampler) throw new Error("Synthetic policy requires a sampler");
  return sampler;
}
export function fixtureDecision(result: JobPolicyResult) {
  const decision = result.last_decision;
  if (!decision) throw new Error("Synthetic job requires a recorded decision");
  return decision;
}
