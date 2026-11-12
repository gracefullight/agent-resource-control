import { isReceived, normalizeDesktopError } from "@/bridge/client";
import type {
  EffectivePolicy,
  JobPolicyResult,
  PolicyObservation,
} from "@/bridge/policy.types";
import { isEffectivePolicy, isJobPolicyResult } from "@/bridge/policy-contract";
import type { DesktopClient, DesktopError, Received } from "@/bridge/types";

export interface PolicySection {
  value: Received<EffectivePolicy> | null;
  job: Received<JobPolicyResult> | null;
  loading: boolean;
  stale: boolean;
  ageMs: number | null;
  error: DesktopError | null;
}
export const emptyPolicy = (): PolicySection => ({
  value: null,
  job: null,
  loading: false,
  stale: false,
  ageMs: null,
  error: null,
});
const invalidReply = (): DesktopError => ({
  code: "invalid_reply",
  retryable: false,
});
const identity = (o: PolicyObservation) =>
  o.sampler === null
    ? null
    : JSON.stringify([
        o.sampler.daemon_instance.boot_id,
        o.sampler.daemon_instance.pid,
        o.sampler.daemon_instance.start_token,
        o.sampler.generation,
        o.sampler.monotonic_epoch,
      ]);
const publication = (o: PolicyObservation) =>
  JSON.stringify([
    o.pressure,
    o.sequence,
    o.observed_at_ms,
    o.observed_monotonic_ms,
    o.stale_after_ms,
  ]);

// Only one current view and the selected job's committed decision are retained.
// Read/receipt clocks age observations; they never replace measurement timestamps.
export class PolicyReadState {
  section = emptyPolicy();
  private automatic = true;
  private receiptAt: number | null = null;
  private ageAtReceipt: number | null = null;
  private retiredSampler: string | null = null;
  private cursor: {
    observation: PolicyObservation;
    age: number;
    at: number;
  } | null = null;
  select(): void {
    this.section = { ...this.section, job: null };
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
      error,
    };
  }
  expire(now: number): number | null {
    const value = this.section.value?.data;
    if (!value || this.receiptAt === null) return null;
    const elapsed = now - this.receiptAt;
    if (!Number.isFinite(elapsed) || elapsed < 0) {
      this.receiptAt = null;
      this.section = { ...this.section, stale: true };
      return null;
    }
    const age = this.ageAtReceipt === null ? null : this.ageAtReceipt + elapsed;
    const threshold = value.observation.stale_after_ms;
    this.section = {
      ...this.section,
      ageMs: age,
      stale:
        this.section.stale ||
        (value.observation.availability === "available" &&
          age !== null &&
          threshold !== null &&
          age > threshold) ||
        elapsed >= 10_000,
    };
    if (this.section.stale) return null;
    return Math.min(
      10_000 - elapsed,
      value.observation.availability === "available" &&
        age !== null &&
        threshold !== null
        ? Math.max(1, threshold - age + 1)
        : 10_000,
    );
  }
  private accept(
    reply: Received<EffectivePolicy>,
    job: Received<JobPolicyResult> | null,
    readStartedAt: number,
    now: number,
  ): void {
    this.expire(now);
    const next = reply.data.observation,
      old = this.cursor?.observation;
    const readElapsed = now - readStartedAt;
    let clockInvalid = !Number.isFinite(readElapsed) || readElapsed < 0;
    // The daemon may have computed age before this read/transport wait. Add
    // the entire elapsed read conservatively, including for a new publication.
    let age =
      next.age_ms === null
        ? null
        : next.age_ms + (clockInvalid ? 0 : readElapsed);
    const measured =
      next.availability === "available" || next.availability === "stale";
    if (measured && old && identity(old) !== null && identity(next) !== null) {
      if (identity(old) === identity(next)) {
        if (
          old.sequence !== null &&
          next.sequence !== null &&
          next.sequence < old.sequence
        )
          throw invalidReply();
        if (
          old.observed_monotonic_ms !== null &&
          next.observed_monotonic_ms !== null &&
          next.observed_monotonic_ms < old.observed_monotonic_ms
        )
          throw invalidReply();
        if (
          old.sequence !== null &&
          old.sequence === next.sequence &&
          ["available", "stale"].includes(old.availability) &&
          ["available", "stale"].includes(next.availability)
        ) {
          if (publication(old) !== publication(next)) throw invalidReply();
        }
        // A new host publication can still carry the same old pressure field.
        // Keep one pressure cursor across failed/missing fallback reads too.
        if (
          next.observed_monotonic_ms === old.observed_monotonic_ms &&
          this.cursor &&
          age !== null
        ) {
          if (next.pressure !== old.pressure) throw invalidReply();
          const elapsed = now - this.cursor.at;
          const validElapsed = Number.isFinite(elapsed) && elapsed >= 0;
          clockInvalid ||= !validElapsed;
          age = Math.max(age, this.cursor.age + (validElapsed ? elapsed : 0));
        }
      } else {
        if (identity(next) === this.retiredSampler) throw invalidReply();
        this.retiredSampler = identity(old);
      }
    }
    if (measured && age !== null && !clockInvalid)
      this.cursor = { observation: structuredClone(next), age, at: now };
    this.receiptAt = !clockInvalid ? now : null;
    this.ageAtReceipt = age;
    this.section = {
      value: reply,
      job,
      loading: false,
      stale: this.receiptAt === null,
      ageMs: age,
      error: null,
    };
    this.expire(now);
  }
  async read(
    client: DesktopClient,
    jobId: string | null,
    current: () => boolean,
    now: () => number,
    publish: () => void,
  ): Promise<void> {
    if (!this.automatic || !current()) return;
    this.section = { ...this.section, loading: true };
    publish();
    try {
      const readStartedAt = now();
      if (jobId) {
        const reply = await client.policyJob(jobId);
        if (!current()) return;
        if (
          !isReceived(reply, (d): d is JobPolicyResult =>
            isJobPolicyResult(d, jobId),
          )
        )
          throw invalidReply();
        this.accept(
          { data: reply.data.effective, received_at_ms: reply.received_at_ms },
          reply,
          readStartedAt,
          now(),
        );
      } else {
        const reply = await client.policyCurrent();
        if (!current()) return;
        if (!isReceived(reply, isEffectivePolicy)) throw invalidReply();
        this.accept(reply, null, readStartedAt, now());
      }
    } catch (error) {
      if (!current()) return;
      const failure = normalizeDesktopError(error);
      this.automatic = failure.retryable;
      this.section = {
        ...this.section,
        loading: false,
        error: failure,
        stale: this.section.value !== null,
      };
    }
    if (current()) publish();
  }
}
