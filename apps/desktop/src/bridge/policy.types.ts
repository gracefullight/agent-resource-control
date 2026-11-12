import type { MeterLevel, MeterSnapshot } from "@/bridge/meter.types";

export const POLICY_VERSION = "macos-memory-pressure-v1";
export const L0_PROFILE = "macos-memory-l0-v1";
export type ObservationAvailability =
  | "available"
  | "missing"
  | "stale"
  | "failed"
  | "unsupported"
  | "denied"
  | "invalid";
export interface PolicyObservation {
  availability: ObservationAvailability;
  pressure: MeterLevel | null;
  age_ms: number | null;
  stale_after_ms: number | null;
  sampler: {
    daemon_instance: MeterSnapshot["daemon_instance"];
    generation: number;
    monotonic_epoch: number;
  } | null;
  sequence: number | null;
  observed_at_ms: number | null;
  observed_monotonic_ms: number | null;
}
export type PolicyReason =
  | "pressure_normal"
  | "pressure_warning"
  | "memory_critical"
  | "observation_missing"
  | "observation_stale"
  | "observation_failed"
  | "observation_unsupported"
  | "observation_denied"
  | "observation_invalid"
  | "sampler_mismatch"
  | "clock_invalid";
export interface Resources {
  cpu_millis: number;
  memory_bytes: number;
}
export interface ClassLimits {
  window: number;
  reservation: Resources;
}
export interface EffectivePolicy {
  schema_version: 1;
  policy_version: typeof POLICY_VERSION;
  profile: typeof L0_PROFILE;
  mode: "l0" | "memory_hold";
  reason: PolicyReason;
  observation: PolicyObservation;
  limits: {
    build: ClassLimits;
    test: ClassLimits;
    lint: ClassLimits;
    budget: Resources;
  };
}
export type DecisionKind =
  | "admit"
  | "wait"
  | "reuse"
  | "preserve"
  | "bypass"
  | "cancel";
export type DecisionReason =
  | "memory_pressure"
  | "fifo"
  | "class_window"
  | "cpu_budget"
  | "memory_budget"
  | "admitted"
  | "existing_reservation"
  | "running"
  | "remote"
  | "unknown"
  | "unsupported"
  | "cancelled";
export interface RecordedDecision {
  policy_version: typeof POLICY_VERSION;
  kind: DecisionKind;
  reason: DecisionReason;
  decided_at_ms: number;
  permit_generation: number | null;
  observation: PolicyObservation;
}
export interface JobPolicyResult {
  schema_version: 1;
  job_id: string;
  effective: EffectivePolicy;
  decision_availability: "available" | "not_recorded" | "expired" | "not_found";
  last_decision: RecordedDecision | null;
}
