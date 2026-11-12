// Exact schema-1 projection of arc_core::job_meter. CPU is cumulative core·s.
import type { JobId, Outcome } from "@/bridge/types";

export interface JobProcessIdentity {
  pid: number;
  start_token: string;
  boot_id: string;
}
export type MissingCoverageReason =
  | "unobserved_descendants"
  | "detached_descendants"
  | "native_usage_unavailable"
  | "run_duration_unavailable"
  | "lineage_unverified"
  | "interrupted"
  | "unsupported";
export type AccountingRelation =
  | { kind: "canonical_root"; root_job_id: JobId }
  | { kind: "included_in_root"; root_job_id: JobId }
  | { kind: "unverified" };
export interface JobMeterReport {
  schema_version: 1;
  report_id: string;
  job_id: JobId;
  observer: JobProcessIdentity;
  child: JobProcessIdentity;
  cpu_core_seconds: {
    user: number | null;
    system: number | null;
    total: number | null;
  };
  run_elapsed: {
    seconds: number | null;
    clock: "monotonic";
    includes_stopped: true;
  };
  attribution_basis: "native_owned_child_wait4";
  scope: "native_waited_descendants";
  completeness: "complete" | "partial" | "unknown";
  missing_coverage: MissingCoverageReason[];
  accounting_relation: AccountingRelation;
  outcome: Exclude<Outcome, { kind: "SPAWN_FAILED" }>;
  terminated_at_ms: number | null;
}
export type SummaryAccounting =
  | { state: "recorded"; report: JobMeterReport }
  | {
      state: "unknown";
      reason:
        | "legacy"
        | "interrupted"
        | "unavailable"
        | "not_terminated"
        | "unsupported";
    }
  | { state: "missing" }
  | { state: "expired" };
export type QueueWait =
  | { seconds: number; clock: "daemon_wall_clock"; basis: "queued_to_reserved" }
  | { seconds: 0; clock: "not_applicable"; basis: "lifecycle_only" }
  | { seconds: null; clock: "unknown"; basis: "unavailable" };
export interface JobMeterSummary {
  schema_version: 1;
  job_id: JobId;
  accounting: SummaryAccounting;
  queue_wait: QueueWait;
}
