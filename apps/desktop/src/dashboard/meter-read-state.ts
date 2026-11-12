import type { MeterMeasurement, MeterSnapshot } from "@/bridge/meter.types";
import type { DesktopError, Received } from "@/bridge/types";

export interface MeterSection {
  value: Received<MeterSnapshot> | null;
  loading: boolean;
  error: DesktopError | null;
  stale: boolean;
  ageMs: number;
  restarted: boolean;
}
export const emptyMeter = (): MeterSection => ({
  value: null,
  loading: false,
  error: null,
  stale: false,
  ageMs: 0,
  restarted: false,
});
export function meterFields(s: MeterSnapshot): MeterMeasurement<unknown>[] {
  return [
    s.host.cpu_utilization,
    s.host.memory_available_bytes,
    s.host.memory_pressure,
    s.host.swap_used_bytes,
    s.host.filesystem.capacity_bytes,
    s.host.filesystem.free_bytes,
    s.host.filesystem.available_bytes,
    s.host.filesystem.pressure,
    s.daemon.cpu_core_seconds,
    s.daemon.rss_bytes,
    s.daemon.observed_peak_rss_bytes,
  ];
}
export function meterFieldStale(
  section: MeterSection,
  field: MeterMeasurement<unknown>,
): boolean {
  const s = section.value?.data;
  return (
    !s ||
    section.stale ||
    section.ageMs + s.sampled_monotonic_ms - field.observed_monotonic_ms >
      s.cadence.stale_after_ms
  );
}
const instance = (s: MeterSnapshot) =>
  `${s.daemon_instance.boot_id}/${s.daemon_instance.pid}/${s.daemon_instance.start_token}`;
function canonical(v: unknown): string {
  if (v !== null && typeof v === "object") {
    const o = v as Record<string, unknown>;
    return `{${Object.keys(o)
      .sort()
      .map((k) => `${JSON.stringify(k)}:${canonical(o[k])}`)
      .join(",")}}`;
  }
  return JSON.stringify(v);
}

// Bounded latest-value bookkeeping only. The existing refresh controller owns
// all reads, generations and timers; this class never schedules or polls.
export class MeterReadState {
  section = emptyMeter();
  private receiptAt: number | null = null;
  private ageAtReceipt = 0;
  private unsupported = false;
  private sampleSignature: string | null = null;
  private retiredInstance: string | null = null;

  get canRead(): boolean {
    return !this.unsupported;
  }
  reprobe(): void {
    this.unsupported = false;
  }
  begin(): void {
    this.section = { ...this.section, loading: true };
  }
  retire(): void {
    this.reprobe();
    this.section = {
      ...this.section,
      loading: false,
      stale: this.section.value !== null,
    };
  }
  finishRetiredRead(): void {
    this.section = { ...this.section, loading: false };
  }
  fail(error: DesktopError): void {
    this.unsupported = error.code === "unsupported";
    this.section = {
      ...this.section,
      loading: false,
      error,
      stale: this.section.value !== null,
    };
  }
  accept(reply: Received<MeterSnapshot>, now: number): void {
    this.expire(now);
    const next = reply.data,
      previous = this.section.value?.data;
    const signature = canonical({ ...next, age_ms: 0 });
    const sameInstance = previous && instance(previous) === instance(next);
    const restarted = Boolean(previous && !sameInstance);
    let age = next.age_ms;
    if (sameInstance) {
      if (
        next.sequence < previous.sequence ||
        next.sampled_monotonic_ms < previous.sampled_monotonic_ms ||
        (next.sequence === previous.sequence &&
          signature !== this.sampleSignature)
      )
        throw {
          code: "invalid_reply",
          retryable: false,
        } satisfies DesktopError;
      if (next.sequence === previous.sequence)
        age = Math.max(age, this.section.ageMs);
    } else if (previous) {
      if (instance(next) === this.retiredInstance)
        throw {
          code: "invalid_reply",
          retryable: false,
        } satisfies DesktopError;
      this.retiredInstance = instance(previous);
    }
    this.receiptAt = Number.isFinite(now) ? now : null;
    this.ageAtReceipt = age;
    this.sampleSignature = signature;
    this.section = {
      value: reply,
      loading: false,
      error: null,
      ageMs: age,
      stale: this.receiptAt === null,
      restarted,
    };
    this.expire(now);
  }
  // Returns the next strict freshness boundary for the controller's one timer.
  expire(now: number): number | null {
    const s = this.section.value?.data;
    if (!s) return null;
    const elapsed = this.receiptAt === null ? NaN : now - this.receiptAt;
    if (!Number.isFinite(elapsed) || elapsed < 0) {
      this.receiptAt = null;
      this.section = { ...this.section, stale: true };
      return null;
    }
    const age = Math.max(this.section.ageMs, this.ageAtReceipt + elapsed);
    this.section = {
      ...this.section,
      ageMs: age,
      stale: this.section.stale || age > s.cadence.stale_after_ms,
    };
    if (this.section.stale) return null;
    const offsets = [
      0,
      ...meterFields(s)
        .filter((f) => f.reading.availability === "available")
        .map((f) => s.sampled_monotonic_ms - f.observed_monotonic_ms),
    ];
    const delays = offsets
      .map((offset) => s.cadence.stale_after_ms - age - offset + 1)
      .filter((delay) => delay > 0);
    return delays.length ? Math.min(...delays) : null;
  }
}
