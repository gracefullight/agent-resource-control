import type { DecisionReason } from "@/bridge/policy.types";
import type {
  AdmissionState,
  TrafficClassification,
} from "@/bridge/traffic-scripts.generated";

export const trafficMessages = {
  heading: "Traffic admission",
  loading: "Loading scheduling details…",
  unsupported: "Scheduling details are not supported by this daemon.",
  unavailable:
    "Scheduling details unavailable. Execution and reservations are unconfirmed.",
  missing:
    "No scheduling record available. Execution and reservations are unconfirmed.",
  retained: "Last known scheduling details retained.",
  stale: "Current admission unconfirmed.",
  executionStale: "Current execution unconfirmed.",
  recorded: "Last recorded: {status}",
  recordedHelp:
    "Recorded scheduling facts; this read does not refresh process liveness.",
  classification: "Recorded class",
  classUnknown: "Classification unavailable",
  queuedUnknown: "Queued · wait cause unavailable",
  outcome: "Execution outcome recorded",
  bypass: "Scheduling bypassed · unresolved or mixed workload",
  remote: "Remote workload · no local reservation",
  details: "Scheduling and reservation details",
  estimates:
    "Reservations are scheduling estimates, separate from measured usage and operating-system limits.",
  reservation: "Recorded reservation",
  noReservation: "No permit recorded in this snapshot",
  permitActive: "Active · last recorded",
  permitReleased: "Released · last recorded",
  permit: "Permit state",
  root: "Reservation root job",
  generation: "Permit generation",
  cpu: "CPU reservation (millicores)",
  memory: "Memory reservation (bytes)",
  queueWait: "Recorded queue wait (ms)",
  window: "Recorded {class} window",
  sharedCpu: "Shared CPU budget (millicores)",
  sharedMemory: "Shared memory budget (bytes)",
  historicalDecision: "Historical decision: {reason}",
  recordedAt: "Scheduling recorded by daemon",
  receivedAt: "Scheduling received by desktop",
  cancelledBeforeStart: "Cancelled · not started",
};
export const trafficClassMessages: Record<
  TrafficClassification["classification"],
  string
> = {
  BUILD: "Build",
  TEST: "Test",
  LINT: "Lint",
  TYPECHECK: "Typecheck",
  REMOTE_CI: "Remote CI",
  REMOTE_EXEC: "Remote execution",
  MIXED: "Mixed workload",
  UNKNOWN: "Unresolved workload",
};
export const trafficStateMessages: Record<AdmissionState, string> = {
  QUEUED: trafficMessages.queuedUnknown,
  RESERVED: "Permit granted · command start unconfirmed",
  RUNNING: "Running",
  COMPLETED: trafficMessages.outcome,
  CANCELLED: "Cancelled · execution outcome unconfirmed",
  ABANDONED: "Tracking abandoned · execution outcome unconfirmed",
  UNCONFIRMED: "Admission unconfirmed",
  UNRESERVED: "No local reservation recorded",
};
export const trafficWaitMessages: Partial<Record<DecisionReason, string>> = {
  memory_pressure: "Waiting for memory",
  fifo: "Waiting for an earlier job",
  class_window: "Waiting for a class slot",
  cpu_budget: "Waiting for shared resources · CPU reservation",
  memory_budget: "Waiting for shared resources · memory reservation",
};
export const trafficClassWaitMessages: Partial<
  Record<TrafficClassification["classification"], string>
> = {
  BUILD: "Waiting for another build",
  TEST: "Waiting for another test",
  LINT: "Waiting for another lint check",
  TYPECHECK: "Waiting for another typecheck",
};
