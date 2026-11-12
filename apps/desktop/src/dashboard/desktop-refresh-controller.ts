import { isJobId, normalizeDesktopError } from "@/bridge/client";
import type {
  DaemonHealth,
  DesktopClient,
  DesktopError,
  JobDetail,
  JobId,
  Received,
  RecentJobs,
} from "@/bridge/types";

import { consistentDetail } from "@/dashboard/job-detail-consistency";
import {
  MeterReadState,
  type MeterSection,
} from "@/dashboard/meter-read-state";

type CancelTimer = () => void;
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
const TRANSIENT_CODES = new Set(["unavailable", "uncertain", "busy"]);

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
  recent: RecordedSection<RecentJobs>;
  selectedId: JobId | null;
  detail: SelectedDetail;
}

const emptySection = <T>(): RecordedSection<T> => ({
  value: null,
  loading: false,
  error: null,
  stale: false,
});
const emptyDetail = (): SelectedDetail => ({
  ...emptySection<JobDetail>(),
  state: "idle",
  lookupReceivedAt: null,
});
const invalidReply = (): DesktopError => ({
  code: "invalid_reply",
  retryable: false,
});

// A single owner serializes native calls (the Rust bridge has a bounded permit).
// At most the latest recent page and the selected detail are retained. Native
// calls finish under their own deadline; retiring a generation discards replies
// without queueing or pretending that an in-flight socket read was cancelled.
export class DesktopRefreshController {
  private readonly meter = new MeterReadState();
  private snapshot: DashboardSnapshot = {
    connection: "loading",
    active: false,
    refreshing: false,
    health: emptySection<DaemonHealth>(),
    meter: this.meter.section,
    recent: emptySection<RecentJobs>(),
    selectedId: null,
    detail: emptyDetail(),
  };
  private listeners = new Set<() => void>();
  private generation = 0;
  private connectionEpoch = 0;
  private selectionGeneration = 0;
  private running = false;
  private cycleActive = false;
  private refreshRequested = false;
  private detailRequested = false;
  private automatic = true;
  private reconnectAttempt = 0;
  private cancelRefresh: CancelTimer | null = null;
  private cancelStale: CancelTimer | null = null;
  private healthAt: number | null = null;
  private recentAt: number | null = null;
  private detailAt: number | null = null;

  constructor(
    private readonly client: DesktopClient,
    private readonly clock: RefreshClock = refreshClock,
  ) {}

  getSnapshot = (): DashboardSnapshot => this.snapshot;
  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  resume = (): void => {
    if (this.snapshot.active) return;
    this.generation++;
    this.automatic = true;
    this.meter.reprobe();
    this.refreshRequested = true;
    this.publish({ active: true });
    this.expire();
    void this.pump();
  };

  pause = (): void => {
    this.generation++;
    this.connectionEpoch++;
    this.selectionGeneration++;
    this.refreshRequested = false;
    this.detailRequested = false;
    this.cancelTimers();
    this.meter.retire();
    this.publish({
      active: false,
      refreshing: false,
      meter: this.meter.section,
      health: { ...this.snapshot.health, loading: false },
      recent: { ...this.snapshot.recent, loading: false },
      detail: { ...this.snapshot.detail, loading: false },
    });
  };

  refresh = (): void => {
    this.meter.reprobe();
    this.requestRefresh();
  };

  private requestRefresh = (): void => {
    if (!this.snapshot.active || this.refreshRequested || this.cycleActive)
      return;
    this.cancelRefresh?.();
    this.cancelRefresh = null;
    this.generation++;
    this.selectionGeneration++;
    this.automatic = true; // Manual retry/reopen may retry incompatible replies.
    this.refreshRequested = true;
    void this.pump();
  };

  select = (jobId: JobId): void => {
    if (!isJobId(jobId) || jobId === this.snapshot.selectedId) return;
    this.selectionGeneration++;
    this.detailAt = null;
    this.publish({ selectedId: jobId, detail: emptyDetail() });
    if (this.snapshot.active && this.snapshot.connection === "connected") {
      this.detailRequested = true;
      this.publish({
        detail: { ...this.snapshot.detail, state: "loading", loading: true },
      });
      this.cancelRefresh?.();
      this.cancelRefresh = null;
      void this.pump();
    }
  };

  private publish(patch: Partial<DashboardSnapshot>): void {
    this.snapshot = { ...this.snapshot, ...patch };
    for (const listener of this.listeners) listener();
  }

  private current(generation: number, epoch: number): boolean {
    return (
      this.snapshot.active &&
      generation === this.generation &&
      epoch === this.connectionEpoch
    );
  }

  private async pump(): Promise<void> {
    if (this.running) return;
    this.running = true;
    try {
      while (
        this.snapshot.active &&
        (this.refreshRequested || this.detailRequested)
      ) {
        if (this.refreshRequested) {
          this.refreshRequested = false;
          this.detailRequested = false;
          this.cycleActive = true;
          await this.cycle(this.generation);
          this.cycleActive = false;
        } else {
          this.detailRequested = false;
          if (this.snapshot.connection === "connected")
            await this.lookup(this.generation, this.connectionEpoch);
        }
      }
    } finally {
      this.cycleActive = false;
      this.running = false;
      if (this.snapshot.active) {
        this.publish({ refreshing: false });
        this.schedule();
      }
    }
  }

  private async cycle(generation: number): Promise<void> {
    const epoch = this.connectionEpoch;
    this.publish({
      refreshing: true,
      health: { ...this.snapshot.health, loading: true },
    });
    try {
      const health = await this.client.daemonHealth();
      if (!this.current(generation, epoch)) return;
      this.healthAt = this.clock.now();
      this.reconnectAttempt = 0;
      this.publish({
        connection: "connected",
        health: { value: health, loading: false, stale: false, error: null },
      });
      this.expire();
    } catch (error) {
      if (!this.current(generation, epoch)) return;
      const failure = normalizeDesktopError(error);
      this.connectionEpoch++;
      this.meter.retire();
      this.selectionGeneration++;
      this.automatic = failure.retryable && TRANSIENT_CODES.has(failure.code);
      this.publish({
        meter: this.meter.section,
        connection:
          failure.code === "unavailable" || failure.code === "uncertain"
            ? "disconnected"
            : "unavailable",
        health: {
          ...this.snapshot.health,
          loading: false,
          error: failure,
          stale: this.snapshot.health.value !== null,
        },
        recent: {
          ...this.snapshot.recent,
          loading: false,
          error: failure,
          stale: this.snapshot.recent.value !== null,
        },
        detail: {
          ...this.snapshot.detail,
          state: this.snapshot.selectedId ? "error" : "idle",
          loading: false,
          error: failure,
          stale: this.snapshot.detail.value !== null,
        },
      });
      return;
    }
    this.publish({ recent: { ...this.snapshot.recent, loading: true } });
    try {
      const recent = await this.client.recentJobs();
      if (!this.current(generation, epoch)) return;
      this.recentAt = this.clock.now();
      this.publish({
        recent: { value: recent, loading: false, stale: false, error: null },
      });
      this.expire();
      const newest = recent.data.jobs[0];
      if (!this.snapshot.selectedId && newest) this.select(newest.job_id);
    } catch (error) {
      if (!this.current(generation, epoch)) return;
      const failure = normalizeDesktopError(error);
      this.automatic = failure.retryable && TRANSIENT_CODES.has(failure.code);
      this.publish({
        recent: {
          ...this.snapshot.recent,
          loading: false,
          error: failure,
          stale: this.snapshot.recent.value !== null,
        },
      });
      if (!this.automatic) {
        this.publish({
          detail: {
            ...this.snapshot.detail,
            loading: false,
            stale: this.snapshot.detail.value !== null,
          },
        });
        return;
      }
    }
    if (!this.current(generation, epoch)) return;
    this.detailRequested = false;
    await this.lookup(generation, epoch);
    if (this.current(generation, epoch))
      await this.readMeter(generation, epoch);
  }

  private async readMeter(generation: number, epoch: number): Promise<void> {
    if (!this.meter.canRead) return;
    const selection = this.selectionGeneration;
    this.meter.begin();
    this.publish({ meter: this.meter.section });
    try {
      const reply = await this.client.meterSnapshot();
      if (!this.current(generation, epoch)) return;
      if (selection !== this.selectionGeneration) {
        this.meter.finishRetiredRead();
      } else this.meter.accept(reply, this.clock.now());
    } catch (error) {
      if (!this.current(generation, epoch)) return;
      if (selection !== this.selectionGeneration)
        this.meter.finishRetiredRead();
      else this.meter.fail(normalizeDesktopError(error));
      // Meter capability/read failures never disable health or job refresh.
    }
    if (this.current(generation, epoch)) {
      this.publish({ meter: this.meter.section });
      this.expire();
    }
  }

  private async lookup(generation: number, epoch: number): Promise<void> {
    const jobId = this.snapshot.selectedId;
    if (!jobId) return;
    const selection = this.selectionGeneration;
    const currentSelection = () =>
      this.current(generation, epoch) &&
      selection === this.selectionGeneration &&
      this.snapshot.selectedId === jobId;
    const previousUpdated = this.snapshot.detail.value?.data.updated_at_ms;
    const summaryUpdated = this.snapshot.recent.value?.data.jobs.find(
      (job) => job.job_id === jobId,
    )?.updated_at_ms;
    this.publish({
      detail: {
        ...this.snapshot.detail,
        state: "loading",
        loading: true,
        stale:
          this.snapshot.detail.stale ||
          (previousUpdated !== undefined &&
            summaryUpdated !== undefined &&
            summaryUpdated > previousUpdated),
      },
    });
    try {
      const reply = await this.client.jobById(jobId);
      if (!currentSelection()) return;
      if (reply.data.kind === "not_found") {
        this.publish({
          detail: {
            ...this.snapshot.detail,
            state: "not_found",
            loading: false,
            lookupReceivedAt: reply.received_at_ms,
            error: null,
            stale: this.snapshot.detail.value !== null,
          },
        });
        return;
      }
      const job = reply.data.job;
      const previous = this.snapshot.detail.value?.data;
      if (
        job.job_id !== jobId ||
        (previous && !consistentDetail(previous, job))
      )
        throw invalidReply();
      // A valid receipt can still lag the latest accepted summary for this ID.
      const latestSummary = this.snapshot.recent.value?.data.jobs.find(
        (summary) => summary.job_id === jobId,
      );
      this.detailAt = this.clock.now();
      this.publish({
        detail: {
          value: { data: job, received_at_ms: reply.received_at_ms },
          state: "ready",
          loading: false,
          lookupReceivedAt: reply.received_at_ms,
          error: null,
          stale:
            latestSummary !== undefined &&
            job.updated_at_ms < latestSummary.updated_at_ms,
        },
      });
      this.expire();
    } catch (error) {
      if (!currentSelection()) return;
      const failure = normalizeDesktopError(error);
      this.automatic = failure.retryable && TRANSIENT_CODES.has(failure.code);
      this.publish({
        detail: {
          ...this.snapshot.detail,
          state: "error",
          loading: false,
          error: failure,
          stale: this.snapshot.detail.value !== null,
        },
      });
    }
  }

  private schedule(): void {
    this.cancelRefresh?.();
    this.cancelRefresh = null;
    if (this.automatic) {
      const reconnecting = this.snapshot.connection !== "connected";
      const delay = reconnecting
        ? (RECONNECT_DELAYS_MS[
            Math.min(this.reconnectAttempt++, RECONNECT_DELAYS_MS.length - 1)
          ] ?? 30_000)
        : Math.min(
            REFRESH_INTERVAL_MS,
            this.snapshot.meter.error
              ? REFRESH_INTERVAL_MS
              : (this.snapshot.meter.value?.data.cadence.interval_ms ??
                  REFRESH_INTERVAL_MS),
          );
      this.cancelRefresh = this.clock.schedule(this.requestRefresh, delay);
    }
    this.expire();
  }

  private expire(): void {
    this.cancelStale?.();
    this.cancelStale = null;
    const now = this.clock.now();
    const patch: Partial<DashboardSnapshot> = {};
    const remaining: number[] = [];
    for (const [key, at] of [
      ["health", this.healthAt],
      ["recent", this.recentAt],
      ["detail", this.detailAt],
    ] as const) {
      const section = this.snapshot[key];
      if (at === null || !section.value || section.stale) continue;
      const age = now - at;
      if (!Number.isFinite(age) || age < 0 || age >= STALE_AFTER_MS) {
        if (key === "detail")
          patch.detail = { ...this.snapshot.detail, stale: true };
        else if (key === "health")
          patch.health = { ...this.snapshot.health, stale: true };
        else patch.recent = { ...this.snapshot.recent, stale: true };
      } else remaining.push(STALE_AFTER_MS - age);
    }
    const meterDelay = this.meter.expire(now);
    patch.meter = this.meter.section;
    if (meterDelay !== null) remaining.push(meterDelay);
    if (Object.keys(patch).length) this.publish(patch);
    if (this.snapshot.active && remaining.length)
      this.cancelStale = this.clock.schedule(
        () => this.expire(),
        Math.min(...remaining),
      );
  }

  private cancelTimers(): void {
    this.cancelRefresh?.();
    this.cancelStale?.();
    this.cancelRefresh = null;
    this.cancelStale = null;
  }
}
