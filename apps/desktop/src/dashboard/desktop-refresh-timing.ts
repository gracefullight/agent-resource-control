import type { DashboardSnapshot } from "@/dashboard/desktop-refresh-state";
import type { JobMeterReadState } from "@/dashboard/job-meter-read-state";
import type { MeterReadState } from "@/dashboard/meter-read-state";
import type { PolicyReadState } from "@/dashboard/policy-read-state";

export type CancelTimer = () => void;
export interface RefreshClock {
  now(): number;
  schedule(callback: () => void, delayMs: number): CancelTimer;
}
export const refreshClock: RefreshClock = {
  now: () => performance.now(),
  schedule: (callback, delayMs) => {
    const timer = setTimeout(callback, delayMs);
    return () => clearTimeout(timer);
  },
};

export const REFRESH_INTERVAL_MS = 5_000;
export const STALE_AFTER_MS = 10_000;
const RECONNECT_DELAYS_MS = [1_000, 2_000, 4_000, 8_000, 16_000, 30_000];

export function refreshDelay(
  snapshot: Pick<DashboardSnapshot, "connection" | "meter">,
  reconnectAttempt: number,
): { delay: number; reconnectAttempt: number } {
  if (snapshot.connection !== "connected") {
    return {
      delay:
        RECONNECT_DELAYS_MS[
          Math.min(reconnectAttempt, RECONNECT_DELAYS_MS.length - 1)
        ] ?? 30_000,
      reconnectAttempt: reconnectAttempt + 1,
    };
  }
  return {
    delay: Math.min(
      REFRESH_INTERVAL_MS,
      snapshot.meter.error
        ? REFRESH_INTERVAL_MS
        : (snapshot.meter.value?.data.cadence.interval_ms ??
            REFRESH_INTERVAL_MS),
    ),
    reconnectAttempt,
  };
}

export function expireRecordedSections(
  snapshot: DashboardSnapshot,
  times: {
    health: number | null;
    recent: number | null;
    detail: number | null;
  },
  now: number,
  staleAfterMs: number,
  meter: MeterReadState,
  jobMeter: JobMeterReadState,
  policy: PolicyReadState,
): { patch: Partial<DashboardSnapshot>; remaining: number[] } {
  const patch: Partial<DashboardSnapshot> = {};
  const remaining: number[] = [];
  for (const [key, at] of [
    ["health", times.health],
    ["recent", times.recent],
    ["detail", times.detail],
  ] as const) {
    const section = snapshot[key];
    if (at === null || !section.value || section.stale) continue;
    const age = now - at;
    if (!Number.isFinite(age) || age < 0 || age >= staleAfterMs) {
      if (key === "detail") patch.detail = { ...snapshot.detail, stale: true };
      else if (key === "health")
        patch.health = { ...snapshot.health, stale: true };
      else patch.recent = { ...snapshot.recent, stale: true };
    } else remaining.push(staleAfterMs - age);
  }
  const meterDelay = meter.expire(now);
  patch.meter = meter.section;
  if (meterDelay !== null) remaining.push(meterDelay);
  const jobDelay = jobMeter.expire(now, staleAfterMs);
  patch.jobMeter = jobMeter.section;
  if (jobDelay !== null) remaining.push(jobDelay);
  const policyDelay = policy.expire(now);
  patch.policy = policy.section;
  if (policyDelay !== null) remaining.push(policyDelay);
  return { patch, remaining };
}
