import type { DecisionReason } from "@/bridge/policy.types";
import type {
  AdmissionState,
  DiagnosticResult,
} from "@/bridge/traffic-scripts.generated";
import { isTrafficDiagnosticResult } from "@/bridge/traffic-scripts-contract";
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
