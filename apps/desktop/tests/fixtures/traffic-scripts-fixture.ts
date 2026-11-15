import type { DecisionReason } from "@/bridge/policy.types";
import type {
  AdmissionState,
  DiagnosticResult,
  TrafficClassification,
} from "@/bridge/traffic-scripts.generated";
import {
  isTrafficClassification,
  isTrafficDiagnosticResult,
} from "@/bridge/traffic-scripts-contract";
import { memoryPolicy } from "./memory-policy-fixture";
import wire from "./traffic-scripts-wire.json";

// Rust-derived recorded facts extended with deterministic historical decisions.
// These values model the consumer contract; they are not native observations.
export function trafficFixture(
  jobId = wire.queries[0]?.job_id ?? "",
  state: AdmissionState = "QUEUED",
): DiagnosticResult {
  const result: unknown = structuredClone(
    wire.results.find((r) => r.admission?.state === state),
  );
  if (!isTrafficDiagnosticResult(result))
    throw new Error("Missing valid Rust traffic fixture");
  if (result.admission?.root_job_id) result.admission.root_job_id = jobId;
  result.job_id = jobId;
  return result;
}
export function trafficWait(
  jobId: string,
  reason: DecisionReason = "class_window",
): DiagnosticResult {
  const result = trafficFixture(jobId);
  if (!result.admission) throw new Error("Missing queued fixture");
  result.admission.wait_reason = reason;
  result.admission.last_decision = {
    policy_version: "macos-memory-pressure-v1",
    kind: "wait",
    reason,
    decided_at_ms: 15,
    permit_generation: null,
    observation: memoryPolicy(
      reason === "memory_pressure" ? "critical" : "normal",
    ).observation,
  };
  if (!isTrafficDiagnosticResult(result))
    throw new Error("Invalid synthetic traffic wait");
  return result;
}
export const trafficReply = (data: DiagnosticResult, received_at_ms = 30) => ({
  data,
  received_at_ms,
});

export type PythonReason = Extract<
  TrafficClassification["reason"],
  "PYTHON_PYTEST" | "PYTHON_UNITTEST"
>;
export type ToolReason =
  | PythonReason
  | Extract<
      TrafficClassification["reason"],
      | "GO_BUILD"
      | "GO_TEST"
      | "GO_VET"
      | "SWIFT_BUILD"
      | "SWIFT_TEST"
      | "CMAKE_BUILD"
      | "CTEST"
    >;

// Select the maintained Rust-generated classification; do not regenerate DTOs
// or infer a Python class from a legacy lifecycle projection.
export function pythonTraffic(
  data: DiagnosticResult,
  reason: PythonReason = "PYTHON_PYTEST",
): DiagnosticResult {
  return toolTraffic(data, reason);
}

export function toolTraffic(
  data: DiagnosticResult,
  reason: ToolReason,
): DiagnosticResult {
  const classification = wire.classifications.find((c) => c.reason === reason);
  if (!isTrafficClassification(classification))
    throw new Error("Missing Rust tool classification");
  const result = { ...structuredClone(data), classification };
  if (!isTrafficDiagnosticResult(result))
    throw new Error("Invalid synthetic tool traffic fixture");
  return result;
}
