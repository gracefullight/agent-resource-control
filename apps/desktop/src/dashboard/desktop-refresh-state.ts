import type {
  DaemonHealth,
  DesktopError,
  JobDetail,
  JobId,
  Received,
  RecentJobs,
} from "@/bridge/types";
import type { GovernorSection } from "@/dashboard/governor-switch-state";
import type { JobMeterSection } from "@/dashboard/job-meter-read-state";
import type { MeterSection } from "@/dashboard/meter-read-state";
import type { PolicySection } from "@/dashboard/policy-read-state";

export interface RecordedSection<T> {
  value: Received<T> | null;
  loading: boolean;
  error: DesktopError | null;
  stale: boolean;
}
export interface SelectedDetail extends RecordedSection<JobDetail> {
  state: "idle" | "loading" | "ready" | "not_found" | "error";
  lookupReceivedAt: number | null;
}
export interface DashboardSnapshot {
  connection: "loading" | "connected" | "disconnected" | "unavailable";
  active: boolean;
  refreshing: boolean;
  health: RecordedSection<DaemonHealth>;
  meter: MeterSection;
  policy: PolicySection;
  governor: GovernorSection;
  jobMeter: JobMeterSection;
  recent: RecordedSection<RecentJobs>;
  selectedId: JobId | null;
  detail: SelectedDetail;
}
export const emptySection = <T>(): RecordedSection<T> => ({
  value: null,
  loading: false,
  error: null,
  stale: false,
});
export const emptyDetail = (): SelectedDetail => ({
  ...emptySection<JobDetail>(),
  state: "idle",
  lookupReceivedAt: null,
});

export const TRANSIENT_CODES = new Set(["unavailable", "uncertain", "busy"]);

export const invalidReply = (): DesktopError => ({
  code: "invalid_reply",
  retryable: false,
});

export function initialDashboardSnapshot(
  sections: Pick<
    DashboardSnapshot,
    "meter" | "jobMeter" | "policy" | "governor"
  >,
): DashboardSnapshot {
  return {
    connection: "loading",
    active: false,
    refreshing: false,
    health: emptySection<DaemonHealth>(),
    recent: emptySection<RecentJobs>(),
    selectedId: null,
    detail: emptyDetail(),
    ...sections,
  };
}
