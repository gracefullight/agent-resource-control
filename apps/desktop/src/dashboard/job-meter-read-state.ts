import { isReceived, normalizeDesktopError } from "@/bridge/client";
import type { JobMeterSummary } from "@/bridge/job-meter.types";
import { isJobMeterSummary } from "@/bridge/job-meter-contract";
import type { DesktopClient, DesktopError, Received } from "@/bridge/types";

export interface JobMeterSection {
  value: Received<JobMeterSummary> | null;
  loading: boolean;
  stale: boolean;
  error: DesktopError | null;
  state: "idle" | "loading" | "ready" | "missing" | "expired" | "error";
  lookupReceivedAt: number | null;
}
export const emptyJobMeter = (): JobMeterSection => ({
  value: null,
  loading: false,
  stale: false,
  error: null,
  state: "idle",
  lookupReceivedAt: null,
});

function canonical(v: unknown): string {
  if (Array.isArray(v)) return `[${v.map(canonical).join(",")}]`;
  if (v !== null && typeof v === "object") {
    const o = v as Record<string, unknown>;
    return `{${Object.keys(o)
      .sort()
      .map((k) => `${JSON.stringify(k)}:${canonical(o[k])}`)
      .join(",")}}`;
  }
  return JSON.stringify(v);
}
const invalidReply = (): DesktopError => ({
  code: "invalid_reply",
  retryable: false,
});

// One selected job only; no socket owner, polling clock or persistent cache.
// The controller supplies its selection/generation/connection-epoch guard.
export class JobMeterReadState {
  section = emptyJobMeter();
  private receivedAt: number | null = null;
  private automatic = true;

  select(loading = false): void {
    const unsupported = this.section.error?.code === "unsupported";
    if (!unsupported) this.reprobe();
    this.section = {
      ...emptyJobMeter(),
      loading: loading && !unsupported,
      state: loading && !unsupported ? "loading" : "idle",
      error: unsupported ? this.section.error : null,
    };
    this.receivedAt = null;
  }
  reprobe(): void {
    this.automatic = true;
  }
  retire(error: DesktopError | null = null): void {
    this.reprobe();
    this.section = {
      ...this.section,
      loading: false,
      stale: this.section.value !== null,
      ...(error ? { error, state: "error" as const } : {}),
    };
  }
  expire(now: number, staleAfter: number): number | null {
    if (this.receivedAt === null || !this.section.value || this.section.stale)
      return null;
    const age = now - this.receivedAt;
    if (!Number.isFinite(age) || age < 0 || age >= staleAfter) {
      this.section = { ...this.section, stale: true };
      return null;
    }
    return staleAfter - age;
  }
  async read(
    client: DesktopClient,
    jobId: string,
    current: () => boolean,
    now: () => number,
    publish: () => void,
  ): Promise<void> {
    if (!this.automatic || !current()) return;
    this.section = { ...this.section, loading: true, state: "loading" };
    publish();
    try {
      const reply = await client.meterSummary(jobId);
      if (!current()) return;
      if (
        !isReceived(reply, (data): data is JobMeterSummary =>
          isJobMeterSummary(data, jobId),
        )
      )
        throw invalidReply();
      const next = reply.data.accounting;
      const previous = this.section.value?.data.accounting;
      if (
        previous?.state === "recorded" &&
        (next.state === "unknown" ||
          (next.state === "recorded" &&
            canonical(next.report) !== canonical(previous.report)))
      )
        throw invalidReply();
      if (next.state === "missing" && this.section.value) {
        this.section = {
          ...this.section,
          state: "missing",
          loading: false,
          stale: true,
          error: null,
          lookupReceivedAt: reply.received_at_ms,
        };
      } else {
        this.receivedAt = now();
        this.section = {
          value: reply,
          loading: false,
          stale: !Number.isFinite(this.receivedAt),
          error: null,
          state:
            next.state === "missing" || next.state === "expired"
              ? next.state
              : "ready",
          lookupReceivedAt: reply.received_at_ms,
        };
      }
    } catch (error) {
      if (!current()) return;
      const failure = normalizeDesktopError(error);
      // Unsupported/invalid peers need an explicit refresh or reconnection,
      // while other sections keep their ordinary automatic read schedule.
      this.automatic = failure.retryable;
      this.section = {
        ...this.section,
        error: failure,
        loading: false,
        state: "error",
        stale: this.section.value !== null,
      };
    }
    if (current()) publish();
  }
}
