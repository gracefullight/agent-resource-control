import { isReceived, normalizeDesktopError } from "@/bridge/client";
import type {
  AdmissionDiagnostic,
  DiagnosticResult,
} from "@/bridge/traffic-scripts.generated";
import { isTrafficDiagnosticResult } from "@/bridge/traffic-scripts-contract";
import type { DesktopClient, DesktopError, Received } from "@/bridge/types";

export interface TrafficSection {
  value: Received<DiagnosticResult> | null;
  loading: boolean;
  stale: boolean;
  error: DesktopError | null;
  missing: boolean;
}
export const emptyTraffic = (): TrafficSection => ({
  value: null,
  loading: false,
  stale: false,
  error: null,
  missing: false,
});
const invalidReply = (): DesktopError => ({
  code: "invalid_reply",
  retryable: false,
});
function same(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (
    a === null ||
    b === null ||
    typeof a !== "object" ||
    typeof b !== "object"
  )
    return false;
  const left = a as Record<string, unknown>,
    right = b as Record<string, unknown>;
  return (
    Object.keys(left).length === Object.keys(right).length &&
    Object.keys(left).every(
      (key) => Object.hasOwn(right, key) && left[key] === right[key],
    )
  );
}

function consistent(
  previous: DiagnosticResult,
  next: DiagnosticResult,
  lastPermit: AdmissionDiagnostic | null,
): boolean {
  if (
    previous.classification !== null &&
    !same(previous.classification, next.classification)
  )
    return false;
  const old = previous.admission,
    admission = next.admission;
  if (!old || !admission) return true;
  if (
    old.sequence !== admission.sequence ||
    old.queued_at_ms !== admission.queued_at_ms ||
    (old.outcome !== null && !same(old.outcome, admission.outcome)) ||
    (["COMPLETED", "CANCELLED"].includes(old.state) &&
      old.state !== admission.state) ||
    // Storage permits a late observed outcome after abandoned-child recovery;
    // accepting that outcome must never revive its released reservation.
    (old.state === "ABANDONED" &&
      admission.state !== "ABANDONED" &&
      admission.state !== "COMPLETED") ||
    (old.state === "RUNNING" &&
      ["QUEUED", "RESERVED", "UNRESERVED"].includes(admission.state))
  )
    return false;

  // Governor can retire an unspawned root grant, clear its timing/identity,
  // requeue it and grant a new generation, even between two accepted polls.
  const canReset =
    old.state === "QUEUED" ||
    old.state === "UNRESERVED" ||
    (old.state === "UNCONFIRMED" && old.root_job_id === null) ||
    (old.state === "RESERVED" && old.root_job_id === previous.job_id);
  const reset =
    canReset &&
    (old.permit_generation === null ||
      old.permit_generation !== admission.permit_generation);
  if (
    !reset &&
    ((old.granted_at_ms !== null &&
      old.granted_at_ms !== admission.granted_at_ms) ||
      (old.queue_wait_ms !== null &&
        old.queue_wait_ms !== admission.queue_wait_ms) ||
      (old.root_job_id !== null &&
        (old.root_job_id !== admission.root_job_id ||
          old.permit_generation !== admission.permit_generation)))
  )
    return false;
  if (lastPermit && admission.permit_generation !== null) {
    if (
      admission.root_job_id !== lastPermit.root_job_id ||
      admission.permit_generation < (lastPermit.permit_generation ?? 0)
    )
      return false;
    if (
      admission.permit_generation === lastPermit.permit_generation &&
      (!same(lastPermit.reservation, admission.reservation) ||
        (!lastPermit.permit_active && admission.permit_active) ||
        (lastPermit.granted_at_ms !== null &&
          lastPermit.granted_at_ms !== admission.granted_at_ms) ||
        (lastPermit.queue_wait_ms !== null &&
          lastPermit.queue_wait_ms !== admission.queue_wait_ms))
    )
      return false;
  }
  // Snapshot/decision wall clocks are not revisions. Selection serialization,
  // immutable domain facts and permit generations establish read consistency.
  return true;
}

// The existing controller owns serialization, selection generations and polling.
// This view retains one job only and never reclassifies or retries execution.
export class TrafficReadState {
  section = emptyTraffic();
  private automatic = true;
  private receivedAt: number | null = null;
  // One generation watermark survives reservation-free snapshots and retries.
  private lastPermit: AdmissionDiagnostic | null = null;
  select(): void {
    const unsupported = this.section.error?.code === "unsupported";
    if (!unsupported) this.reprobe();
    this.section = {
      ...emptyTraffic(),
      error: unsupported ? this.section.error : null,
    };
    this.receivedAt = null;
    this.lastPermit = null;
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
    this.section = { ...this.section, loading: true };
    publish();
    try {
      const reply = await client.trafficJobDiagnostics(jobId);
      if (!current()) return;
      if (
        !isReceived(reply, (data): data is DiagnosticResult =>
          isTrafficDiagnosticResult(data, jobId),
        )
      )
        throw invalidReply();
      const previous = this.section.value?.data,
        next = reply.data;
      const missing =
        next.classification === null ||
        (previous?.admission !== null &&
          previous?.admission !== undefined &&
          next.admission === null);
      if (missing && previous) {
        this.section = {
          ...this.section,
          loading: false,
          missing: true,
          stale: true,
          error: null,
        };
      } else {
        if (previous && !consistent(previous, next, this.lastPermit))
          throw invalidReply();
        if (next.admission?.root_job_id) this.lastPermit = next.admission;
        else if (this.lastPermit)
          this.lastPermit = { ...this.lastPermit, permit_active: false };
        this.receivedAt = now();
        this.section = {
          value: reply,
          loading: false,
          stale: !Number.isFinite(this.receivedAt),
          error: null,
          missing,
        };
      }
    } catch (error) {
      if (!current()) return;
      const failure = normalizeDesktopError(error);
      this.automatic = failure.retryable;
      this.section = {
        ...this.section,
        loading: false,
        stale: this.section.value !== null,
        error: failure,
      };
    }
    if (current()) publish();
  }
}
