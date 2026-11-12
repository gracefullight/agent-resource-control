import type { JobMeterSummary } from "@/bridge/job-meter.types";
import type { MeterSnapshot } from "@/bridge/meter.types";
import type { EffectivePolicy, JobPolicyResult } from "@/bridge/policy.types";

// Job projection omits private command data. Meter carries only its bounded
// daemon identity and observed local filesystem identity from the core schema.
export type Ms = number;
export type Revision = number;
export type JobId = string;
export type Phase =
  | "REGISTERED"
  | "LAUNCHING"
  | "RUNNING"
  | "COMPLETED"
  | "SIGNALED"
  | "SPAWN_FAILED"
  | "OUTCOME_UNKNOWN";
export type LifecycleReason =
  | "NONE"
  | "REGISTERED_NOT_CLAIMED"
  | "CLAIMED_NOT_ATTACHED"
  | "OBSERVER_MISSING_NOT_CLAIMED"
  | "OBSERVER_MISSING"
  | "PROCESS_MISSING"
  | "IDENTITY_MISMATCH"
  | "IDENTITY_UNAVAILABLE"
  | "CHILD_VERIFIED"
  | "OBSERVED_EXIT"
  | "OBSERVED_SIGNAL"
  | "OBSERVED_SPAWN_FAILURE";
export type ObservationTarget = "OBSERVER" | "CHILD" | "NONE";
export type ObservationStatus =
  | "VERIFIED_MATCH"
  | "MISSING"
  | "MISMATCH"
  | "INDETERMINATE"
  | "NOT_ATTACHED";
export type SpawnReason =
  | "NOT_FOUND"
  | "PERMISSION_DENIED"
  | "INVALID_INPUT"
  | "RESOURCE_UNAVAILABLE"
  | "OTHER";
export type Outcome =
  | { kind: "EXITED"; code: number }
  | { kind: "SIGNALED"; signal: number; core_dumped: boolean }
  | { kind: "SPAWN_FAILED"; reason: SpawnReason; os_code: number | null };
export interface IdentityObservation {
  target: ObservationTarget;
  status: ObservationStatus;
  observed_at_ms: Ms;
  reason: LifecycleReason;
}
export interface Received<T> {
  data: T;
  received_at_ms: Ms;
}
export interface DaemonHealth {
  daemon: "running";
  protocol_version: 1;
  sweeper: "off";
}
export interface JobSummary {
  job_id: JobId;
  phase: Phase;
  registered_at_ms: Ms;
  updated_at_ms: Ms;
  reason: LifecycleReason;
}
export interface RecentJobs {
  jobs: JobSummary[];
  truncated: boolean;
  observed_at_ms: Ms;
  liveness_fresh: false;
  limit: 50;
}
export interface JobDetail {
  job_id: JobId;
  phase: Phase;
  reason: LifecycleReason;
  observation: IdentityObservation;
  outcome: Outcome | null;
  registered_at_ms: Ms;
  launch_claimed_at_ms: Ms | null;
  completed_at_ms: Ms | null;
  updated_at_ms: Ms;
  revision: Revision;
}
export type JobLookup =
  | { kind: "found"; job: JobDetail }
  | { kind: "not_found" };
export type CreditLink = "github" | "linkedin" | "oma";
export interface DesktopMetadata {
  product_name: "Agent Resource Control";
  version: string;
  build_commit: string | null;
  build_dirty: boolean | null;
  os: string;
  architecture: string;
  license: string | null;
  open_source_notices: string;
}
export type ErrorCode =
  | "unavailable"
  | "uncertain"
  | "busy"
  | "rejected"
  | "protocol"
  | "invalid_reply"
  | "invalid_request"
  | "invalid_endpoint"
  | "local"
  | "cancelled"
  | "unsupported"
  | "opener_failed"
  | "window_unavailable";
export interface DesktopError {
  code: ErrorCode;
  retryable: boolean;
}
export interface DesktopClient {
  policyCurrent(): Promise<Received<EffectivePolicy>>;
  policyJob(jobId: JobId): Promise<Received<JobPolicyResult>>;
  daemonHealth(): Promise<Received<DaemonHealth>>;
  meterSnapshot(): Promise<Received<MeterSnapshot>>;
  meterSummary(jobId: JobId): Promise<Received<JobMeterSummary>>;
  recentJobs(): Promise<Received<RecentJobs>>;
  jobById(jobId: JobId): Promise<Received<JobLookup>>;
  desktopMetadata(): Promise<DesktopMetadata>;
  openCreditLink(link: CreditLink): Promise<void>;
  openDashboard(): Promise<void>;
  hideDashboard(): Promise<void>;
  quitDesktop(): Promise<void>;
}
