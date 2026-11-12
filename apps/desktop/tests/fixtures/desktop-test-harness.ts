import { Window } from "happy-dom";
import {
  createDesktopClient,
  type DesktopCommand,
  type DesktopInvoke,
} from "@/bridge/client";
import type {
  DesktopError,
  JobDetail,
  Received,
  RecentJobs,
} from "@/bridge/types";
import type { RefreshClock } from "@/dashboard/desktop-refresh-controller";
import type { DesktopVisibility } from "@/shell/desktop-visibility";
import fixtures from "./bridge.json";
import { jobMeterReply, unknownJobMeter } from "./job-meter-fixture";
import { meterReply, sharedMeterSnapshot } from "./meter-fixture";

// One shared DOM is installed before ReactDOM/Base UI load. Tests use queries
// scoped to their actual render root, never a stale global screen singleton.
if (typeof document === "undefined") {
  const window = new Window({ url: "http://localhost:1420" });
  for (const key of [
    "window",
    "document",
    "navigator",
    "Node",
    "Element",
    "HTMLElement",
    "HTMLButtonElement",
    "HTMLAnchorElement",
    "HTMLInputElement",
    "Document",
    "ShadowRoot",
    "Event",
    "CustomEvent",
    "KeyboardEvent",
    "MouseEvent",
    "FocusEvent",
    "PointerEvent",
    "MutationObserver",
    "ResizeObserver",
  ] as const) {
    const value = window[key];
    Object.defineProperty(globalThis, key, {
      configurable: true,
      writable: true,
      value,
    });
  }
  Object.assign(globalThis, {
    getComputedStyle: window.getComputedStyle.bind(window),
    requestAnimationFrame: window.requestAnimationFrame.bind(window),
    cancelAnimationFrame: window.cancelAnimationFrame.bind(window),
  });
  // Supply the browser's hidden-element rule, absent from Happy DOM's UA sheet,
  // so keyboard navigation respects the same native hidden state as the app.
  const hiddenRule = window.document.createElement("style");
  hiddenRule.textContent = "[hidden] { display: none; }";
  window.document.head.append(hiddenRule);
}
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });

export const { act, createElement, StrictMode } = await import("react");
export const { render, cleanup, within, fireEvent } = await import(
  "@testing-library/react"
);
const { default: userEvent } = await import("@testing-library/user-event");
const { DesktopApplication } = await import("@/desktop-application");

export const T = fixtures.clock_ms;
export const A = fixtures.exit_zero.data.job.job_id;
export const B = fixtures.exit_seventeen.data.job.job_id;
export const golden = fixtures;
export const clone = <TValue>(value: TValue): TValue => structuredClone(value);

export function deferred<TValue>() {
  let resolve!: (value: TValue) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<TValue>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

export class FakeClock implements RefreshClock {
  private time = 0;
  private sequence = 0;
  private timers = new Map<number, { at: number; callback: () => void }>();
  now = () => this.time;
  schedule = (callback: () => void, delayMs: number) => {
    const id = ++this.sequence;
    this.timers.set(id, { at: this.time + delayMs, callback });
    return () => {
      this.timers.delete(id);
    };
  };
  advance(milliseconds: number) {
    const end = this.time + milliseconds;
    while (true) {
      const next = [...this.timers]
        .filter(([, timer]) => timer.at <= end)
        .sort((a, b) => a[1].at - b[1].at || a[0] - b[0])[0];
      if (!next) break;
      this.time = next[1].at;
      this.timers.delete(next[0]);
      next[1].callback();
    }
    this.time = end;
  }
  get pending() {
    return this.timers.size;
  }
}

export class FakeVisibility implements DesktopVisibility {
  private listeners = new Set<{ visible: () => void; hidden: () => void }>();
  subscribe = (visible: () => void, hidden: () => void) => {
    const listener = { visible, hidden };
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };
  show() {
    for (const listener of this.listeners) listener.visible();
  }
  hide() {
    for (const listener of this.listeners) listener.hidden();
  }
  get count() {
    return this.listeners.size;
  }
}

type ScriptedReply = unknown | (() => unknown);
export class NativeFixture {
  calls: { command: DesktopCommand; args: Parameters<DesktopInvoke>[1] }[] = [];
  recent: unknown = clone(golden.recent);
  health: unknown = clone(golden.health);
  metadata: unknown = clone(golden.metadata);
  meter: unknown = meterReply(sharedMeterSnapshot());
  policy: unknown = null;
  policies = new Map<string, unknown>();
  jobMeters = new Map<string, unknown>([
    [A, jobMeterReply(unknownJobMeter(A))],
    [B, jobMeterReply(unknownJobMeter(B))],
  ]);
  jobs = new Map<string, unknown>([
    [A, clone(golden.exit_zero)],
    [B, clone(golden.exit_seventeen)],
  ]);
  healthFailure: DesktopError | null = null;
  private scripts = new Map<DesktopCommand, ScriptedReply[]>();
  private inFlight = 0;
  maxInFlight = 0;
  enqueue(command: DesktopCommand, ...replies: ScriptedReply[]) {
    this.scripts.set(command, [
      ...(this.scripts.get(command) ?? []),
      ...replies,
    ]);
  }
  invoke: DesktopInvoke = async (command, args) => {
    this.calls.push({ command, args });
    const daemonRead =
      command === "daemon_health" ||
      command === "recent_jobs" ||
      command === "meter_snapshot" ||
      command === "meter_summary" ||
      command === "policy_current" ||
      command === "policy_job" ||
      command === "job_by_id";
    if (daemonRead)
      this.maxInFlight = Math.max(this.maxInFlight, ++this.inFlight);
    try {
      const queue = this.scripts.get(command);
      if (queue?.length) {
        const reply = queue.shift();
        return await (typeof reply === "function" ? reply() : reply);
      }
      switch (command) {
        case "policy_current":
          if (this.policy === null)
            throw { code: "unsupported", retryable: false };
          return clone(this.policy);
        case "policy_job": {
          const id = args && "job_id" in args ? args.job_id : "";
          const reply = this.policies.get(id);
          if (!reply) throw { code: "unsupported", retryable: false };
          return clone(reply);
        }
        case "daemon_health":
          if (this.healthFailure) throw this.healthFailure;
          return clone(this.health);
        case "recent_jobs":
          return clone(this.recent);
        case "meter_snapshot":
          return clone(this.meter);
        case "meter_summary": {
          const id = args && "job_id" in args ? args.job_id : "";
          return clone(
            this.jobMeters.get(id) ?? jobMeterReply(unknownJobMeter(id)),
          );
        }
        case "job_by_id":
          return clone(
            this.jobs.get(args && "job_id" in args ? args.job_id : "") ??
              golden.not_found,
          );
        case "desktop_metadata":
          return clone(this.metadata);
        default:
          return null;
      }
    } finally {
      if (daemonRead) this.inFlight--;
    }
  };
  client = createDesktopClient(this.invoke);
  count(command: DesktopCommand) {
    return this.calls.filter((call) => call.command === command).length;
  }
}

export async function settle() {
  // Drain the fixed, finite chain of bridge validation/React microtasks. Polling
  // and retry time are controlled only by FakeClock, with no real sleeps.
  await act(async () => {
    for (let i = 0; i < 40; i++) await Promise.resolve();
  });
}
export async function advance(clock: FakeClock, milliseconds: number) {
  await act(async () => {
    clock.advance(milliseconds);
  });
  await settle();
}

export function mountDesktop(
  native = new NativeFixture(),
  clock = new FakeClock(),
  visibility = new FakeVisibility(),
  strict = false,
) {
  const component = createElement(DesktopApplication, {
    client: native.client,
    clock,
    visibility,
    locale: "en-US",
    timeZone: "UTC",
  });
  return {
    ...render(strict ? createElement(StrictMode, null, component) : component),
    native,
    clock,
    visibility,
    user: userEvent.setup({ document, delay: null }),
  };
}

export function found(
  job: JobDetail,
  receivedAt = T,
): Received<{ kind: "found"; job: JobDetail }> {
  return { data: { kind: "found", job }, received_at_ms: receivedAt };
}
export function page(ids = [A, B]): Received<RecentJobs> {
  const summary = golden.recent.data.jobs[0];
  if (!summary) throw new Error("Missing synthetic summary");
  return {
    ...clone(golden.recent),
    data: {
      ...clone(golden.recent.data),
      jobs: ids.map((id) => ({ ...summary, job_id: id })),
    },
  } as Received<RecentJobs>;
}
