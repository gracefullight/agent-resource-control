import type {
  DaemonHealth,
  DesktopError,
  JobDetail,
  JobId,
  Received,
  RecentJobs,
} from "@/bridge/types";
import type { JobMeterSection } from "@/dashboard/job-meter-read-state";
import type { MeterSection } from "@/dashboard/meter-read-state";

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
