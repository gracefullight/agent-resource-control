import { isJobId, normalizeDesktopError } from "@/bridge/client";
import type { DesktopClient, JobId } from "@/bridge/types";
import {
  type DashboardSnapshot,
  emptyDetail,
  initialDashboardSnapshot,
  invalidReply,
  TRANSIENT_CODES,
} from "@/dashboard/desktop-refresh-state";
import {
  type CancelTimer,
  expireRecordedSections,
  type RefreshClock,
  refreshClock,
  refreshDelay,
  STALE_AFTER_MS,
} from "@/dashboard/desktop-refresh-timing";
import { GovernorSwitchStateOwner } from "@/dashboard/governor-switch-state";

export type { RefreshClock } from "@/dashboard/desktop-refresh-timing";
export {
  REFRESH_INTERVAL_MS,
  refreshClock,
  STALE_AFTER_MS,
} from "@/dashboard/desktop-refresh-timing";

import { consistentDetail } from "@/dashboard/job-detail-consistency";
import { JobMeterReadState } from "@/dashboard/job-meter-read-state";
import { MeterReadState } from "@/dashboard/meter-read-state";
import { PolicyReadState } from "@/dashboard/policy-read-state";

export type {
  DashboardSnapshot,
  RecordedSection,
  SelectedDetail,
} from "@/dashboard/desktop-refresh-state";

// A single owner serializes native calls (the Rust bridge has a bounded permit).
// At most the latest recent page and the selected detail are retained. Native
// calls finish under their own deadline; retiring a generation discards replies
// without queueing or pretending that an in-flight socket read was cancelled.
export class DesktopRefreshController {
  private readonly governor = new GovernorSwitchStateOwner(
    () => this.publish({ governor: this.governor.section }),
    () => this.clock.now(),
  );
  private readonly meter = new MeterReadState();
  private readonly jobMeter = new JobMeterReadState();
  private readonly policy = new PolicyReadState();
  private snapshot: DashboardSnapshot = initialDashboardSnapshot({
    meter: this.meter.section,
    jobMeter: this.jobMeter.section,
    policy: this.policy.section,
    governor: this.governor.section,
  });
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
    this.jobMeter.reprobe();
    this.policy.reprobe();
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
    this.jobMeter.retire();
    this.policy.retire();
    this.governor.retire();
    this.publish({
      governor: this.governor.section,
      active: false,
      refreshing: false,
      meter: this.meter.section,
      jobMeter: this.jobMeter.section,
      policy: this.policy.section,
      health: { ...this.snapshot.health, loading: false },
      recent: { ...this.snapshot.recent, loading: false },
      detail: { ...this.snapshot.detail, loading: false },
    });
  };

  setGovernor = (enabled: boolean): void => {
    this.governor.request(enabled);
    void this.pump();
  };

  refresh = (): void => {
    this.meter.reprobe();
    this.jobMeter.reprobe();
    this.policy.reprobe();
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
    this.policy.select();
    this.jobMeter.select(
      this.snapshot.active && this.snapshot.connection === "connected",
    );
    this.publish({
      selectedId: jobId,
      detail: emptyDetail(),
      jobMeter: this.jobMeter.section,
      policy: this.policy.section,
    });
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
        this.governor.needsWork ||
        (this.snapshot.active &&
          (this.refreshRequested || this.detailRequested))
      ) {
        if (this.governor.needsWork) {
          await this.governor.work(this.client);
        } else if (this.refreshRequested) {
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
    await this.governor.read(this.client);
    if (!this.current(generation, epoch)) return;
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
      this.jobMeter.retire(failure);
      this.policy.retire(failure);
      this.selectionGeneration++;
      this.automatic = failure.retryable && TRANSIENT_CODES.has(failure.code);
      this.publish({
        meter: this.meter.section,
        jobMeter: this.jobMeter.section,
        policy: this.policy.section,
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
    if (this.current(generation, epoch) && !this.snapshot.selectedId)
      await this.readPolicy(generation, epoch);
  }

  private async readPolicy(generation: number, epoch: number): Promise<void> {
    const jobId = this.snapshot.selectedId,
      selection = this.selectionGeneration;
    await this.policy.read(
      this.client,
      jobId,
      () =>
        this.current(generation, epoch) &&
        selection === this.selectionGeneration &&
        jobId === this.snapshot.selectedId,
      () => this.clock.now(),
      () => {
        this.publish({ policy: this.policy.section });
        this.expire();
      },
    );
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
    const jobId = this.snapshot.selectedId,
      selection = this.selectionGeneration;
    if (!jobId) return;
    const current = () =>
      this.current(generation, epoch) &&
      selection === this.selectionGeneration &&
      jobId === this.snapshot.selectedId;
    await this.readDetail(generation, epoch);
    if (current())
      await this.jobMeter.read(
        this.client,
        jobId,
        current,
        () => this.clock.now(),
        () => {
          this.publish({ jobMeter: this.jobMeter.section });
          this.expire();
        },
      );
    if (current()) await this.readPolicy(generation, epoch);
  }

  private async readDetail(generation: number, epoch: number): Promise<void> {
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
      const next = refreshDelay(this.snapshot, this.reconnectAttempt);
      this.reconnectAttempt = next.reconnectAttempt;
      this.cancelRefresh = this.clock.schedule(this.requestRefresh, next.delay);
    }
    this.expire();
  }

  private expire(): void {
    this.cancelStale?.();
    this.cancelStale = null;
    const now = this.clock.now();
    const { patch, remaining } = expireRecordedSections(
      this.snapshot,
      { health: this.healthAt, recent: this.recentAt, detail: this.detailAt },
      now,
      STALE_AFTER_MS,
      this.meter,
      this.jobMeter,
      this.policy,
    );
    const governorDelay = this.governor.expire(now);
    patch.governor = this.governor.section;
    if (governorDelay !== null) remaining.push(governorDelay);
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
