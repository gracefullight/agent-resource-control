import { isReceived, normalizeDesktopError } from "@/bridge/client";
import type {
  GovernorRejectionReason,
  GovernorSwitchReadResult,
  GovernorSwitchState,
  SetGovernorSwitchRequest,
} from "@/bridge/governor.generated";
import {
  isGovernorMutationResult,
  isGovernorReadResult,
  sameGovernorState,
} from "@/bridge/governor-contract";
import type { DesktopClient, DesktopError, Received } from "@/bridge/types";

export interface GovernorSection {
  value: Received<GovernorSwitchState> | null;
  stale: boolean;
  loading: boolean;
  pending: SetGovernorSwitchRequest | null;
  phase: "idle" | "applying" | "uncertain";
  notice: "committed" | "rejected" | "conflict" | "unknown_outcome" | null;
  error: DesktopError | null;
  lastIntent: boolean | null;
  rejectionReason: GovernorRejectionReason | "before_dispatch" | null;
}
const invalid = (): DesktopError => ({
  code: "invalid_reply",
  retryable: false,
});
const nonce = () =>
  Array.from(crypto.getRandomValues(new Uint8Array(16)), (v) =>
    v.toString(16).padStart(2, "0"),
  ).join("");

// Lives with the application controller, independently of read generations,
// navigation, connection epochs and hidden-window polling cancellation.
export class GovernorSwitchStateOwner {
  section: GovernorSection = {
    value: null,
    stale: false,
    loading: true,
    pending: null,
    phase: "idle",
    notice: null,
    error: null,
    lastIntent: null,
    rejectionReason: null,
  };
  private queuedWrite = false;
  private readRequested = false;
  private at: number | null = null;
  constructor(
    private readonly publish: () => void,
    private readonly now: () => number,
    private readonly random: () => string = nonce,
  ) {}
  get needsWork(): boolean {
    return this.queuedWrite || this.readRequested;
  }
  request(enabled: boolean): void {
    const state = this.section.value?.data;
    if (
      !state ||
      this.section.stale ||
      this.section.pending ||
      enabled === state.enabled
    )
      return;
    const request: SetGovernorSwitchRequest = {
      key: "governor.enabled",
      enabled,
      expected_revision: state.revision,
      mutation_id: `${state.revision}-${this.random()}`,
    };
    this.queuedWrite = true;
    this.section = {
      ...this.section,
      pending: request,
      lastIntent: enabled,
      rejectionReason: null,
      phase: "applying",
      error: null,
      notice: null,
    };
    this.publish();
  }
  retire(): void {
    this.section = { ...this.section, stale: this.section.value !== null };
  }
  expire(now: number): number | null {
    if (this.at === null || !this.section.value || this.section.stale)
      return null;
    const age = now - this.at;
    if (!Number.isFinite(age) || age < 0 || age >= 10_000) {
      this.retire();
      return null;
    }
    return 10_000 - age;
  }
  private accept(state: GovernorSwitchState, received: number): void {
    const old = this.section.value?.data;
    if (old && state.revision < old.revision) return;
    if (
      old &&
      state.revision === old.revision &&
      !sameGovernorState(old, state)
    )
      throw invalid();
    this.at = this.now();
    this.section = {
      ...this.section,
      value: { data: state, received_at_ms: received },
      stale: false,
      error: null,
    };
  }
  async work(client: DesktopClient): Promise<void> {
    if (this.queuedWrite) {
      this.queuedWrite = false;
      const request = this.section.pending;
      if (!request) return;
      try {
        const result = await client.governorSetSwitch(request);
        if (
          !isReceived(result, (v): v is typeof result.data =>
            isGovernorMutationResult(v, request),
          )
        )
          throw invalid();
        if (result.data.status === "COMMITTED" && result.data.replayed) {
          // A duplicate receipt confirms the original request, not today's
          // switch. Read current state before making the control actionable.
          this.retire();
        } else this.accept(result.data.switch, result.received_at_ms);
        this.section = {
          ...this.section,
          pending: null,
          phase: "idle",
          rejectionReason:
            result.data.status === "REJECTED" ? result.data.reason : null,
          notice:
            result.data.status === "COMMITTED"
              ? "committed"
              : result.data.status === "CONFLICT"
                ? "conflict"
                : "rejected",
        };
        // Replayed receipts can predate other clients. Always read current state.
        this.readRequested = true;
      } catch (error) {
        const failure = normalizeDesktopError(error);
        if (["rejected", "invalid_request"].includes(failure.code)) {
          this.section = {
            ...this.section,
            pending: null,
            phase: "idle",
            notice: "rejected",
            rejectionReason: "before_dispatch",
            error: null,
          };
        } else {
          this.section = {
            ...this.section,
            phase: "uncertain",
            error: failure,
            stale: this.section.value !== null,
          };
          this.readRequested = true;
        }
      }
      this.publish();
    }
    if (this.readRequested) {
      this.readRequested = false;
      await this.read(client);
    }
  }
  async read(client: DesktopClient): Promise<void> {
    // A write waiting behind another read must run before this next read.
    if (this.queuedWrite) return;
    const pending = this.section.pending;
    const query = {
      key: "governor.enabled" as const,
      ...(pending ? { mutation_id: pending.mutation_id } : {}),
    };
    this.section = { ...this.section, loading: true };
    this.publish();
    try {
      const reply = await client.governorGetSwitch(query);
      if (
        !isReceived(reply, (v): v is GovernorSwitchReadResult =>
          isGovernorReadResult(v, query),
        )
      )
        throw invalid();
      const receipt = reply.data.mutation_result;
      if (pending && receipt && !isGovernorMutationResult(receipt, pending))
        throw invalid();
      this.accept(reply.data.switch, reply.received_at_ms);
      if (pending && this.section.pending === pending) {
        if (receipt)
          this.section = {
            ...this.section,
            pending: null,
            phase: "idle",
            notice: "committed",
          };
        else if (reply.data.switch.revision > pending.expected_revision) {
          // The immutable old CAS can no longer change current state. Its
          // historical outcome is unknown when its bounded receipt is absent.
          this.section = {
            ...this.section,
            pending: null,
            phase: "idle",
            notice: "unknown_outcome",
          };
        }
        // An absent receipt at the CAS base cannot settle a delayed write.
      }
    } catch (error) {
      this.section = {
        ...this.section,
        stale: this.section.value !== null,
        error: normalizeDesktopError(error),
      };
    }
    this.section = { ...this.section, loading: false };
    this.publish();
  }
}
