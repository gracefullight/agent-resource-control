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
import {
  emptyTraffic,
  type TrafficSection,
} from "@/dashboard/traffic-read-state";

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
  traffic: TrafficSection;
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

export function loadingDetail(
  detail: SelectedDetail,
  summaryUpdated: number | undefined,
): SelectedDetail {
  const previousUpdated = detail.value?.data.updated_at_ms;
  return {
    ...detail,
    state: "loading",
    loading: true,
    stale:
      detail.stale ||
      (previousUpdated !== undefined &&
        summaryUpdated !== undefined &&
        summaryUpdated > previousUpdated),
  };
}

export function missingDetail(
  detail: SelectedDetail,
  lookupReceivedAt: number,
): SelectedDetail {
  return {
    ...detail,
    state: "not_found",
    loading: false,
    lookupReceivedAt,
    error: null,
    stale: detail.value !== null,
  };
}

export function readyDetail(
  job: JobDetail,
  receivedAt: number,
  summaryUpdated: number | undefined,
): SelectedDetail {
  return {
    value: { data: job, received_at_ms: receivedAt },
    state: "ready",
    loading: false,
    lookupReceivedAt: receivedAt,
    error: null,
    stale: summaryUpdated !== undefined && job.updated_at_ms < summaryUpdated,
  };
}

export function failedDetail(
  detail: SelectedDetail,
  failure: DesktopError,
): SelectedDetail {
  return {
    ...detail,
    state: "error",
    loading: false,
    error: failure,
    stale: detail.value !== null,
  };
}

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
    traffic: emptyTraffic(),
    ...sections,
  };
}
