import { afterEach, describe, expect, test } from "bun:test";
import { desktopVisibility } from "@/shell/desktop-visibility";
import {
  A,
  act,
  advance,
  cleanup,
  deferred,
  golden,
  mountDesktop,
  NativeFixture,
  settle,
  T,
  within,
} from "./fixtures/desktop-test-harness";
import { meterFixture, meterReply, nextMeter } from "./fixtures/meter-fixture";
import { registerPolicyLifecycleCases } from "./fixtures/policy-interaction-cases";

afterEach(cleanup);

describe("production desktop component lifecycle", () => {
  registerPolicyLifecycleCases();
  test("hidden pending Meter replies retire before one serialized reopen cycle and new accounting window", async () => {
    const native = new NativeFixture(),
      first = meterFixture();
    native.meter = meterReply(first);
    const view = mountDesktop(native);
    await settle();
    const pending = deferred<unknown>();
    native.enqueue("meter_snapshot", () => pending.promise);
    await view.user.click(view.getByRole("button", { name: "Refresh status" }));
    await settle();
    const main = view.getByRole("main"),
      calls = native.calls.length;
    await act(async () => {
      view.visibility.hide();
    });
    await advance(view.clock, 40_000);
    expect(native.calls).toHaveLength(calls);
    expect(view.clock.pending).toBe(0);
    const fresh = nextMeter(first);
    fresh.daemon_instance.start_token = "123457";
    fresh.sequence = 1;
    native.meter = meterReply(fresh);
    await act(async () => {
      view.visibility.show();
      view.visibility.show();
    });
    const metrics = within(
      view.getByRole("region", { name: "Resource availability" }),
    );
    expect(metrics.getByRole("status").textContent).toContain("Stale");
    expect(native.calls).toHaveLength(calls);
    const late = nextMeter(first);
    late.sequence = 999;
    await act(async () => {
      pending.resolve(meterReply(late));
    });
    await settle();
    expect(view.getByRole("main")).toBe(main);
    expect(native.count("meter_snapshot")).toBe(3);
    expect(metrics.getByRole("status").textContent).toContain(
      "Daemon restarted · new accounting window",
    );
    expect(metrics.getByRole("status").textContent).not.toContain("Stale");
    expect(native.maxInFlight).toBe(1);
  });

  test("desktop-only quit with a pending Meter read retires it and visibility never restarts inspection", async () => {
    const native = new NativeFixture(),
      pending = deferred<unknown>();
    native.enqueue("meter_snapshot", () => pending.promise);
    const view = mountDesktop(native);
    await settle();
    await view.user.click(
      view.getByRole("button", { name: "Quit Desktop (Daemon continues)" }),
    );
    const calls = native.calls.length;
    await act(async () => {
      pending.resolve(meterReply());
      view.visibility.show();
    });
    await settle();
    await advance(view.clock, 300_000);
    expect(native.calls).toHaveLength(calls);
    expect(native.count("quit_desktop")).toBe(1);
    expect(view.clock.pending).toBe(0);
    expect(view.container.textContent).not.toContain("25.0%");
  });
  test("Hide Dashboard retires late native replies and Open Dashboard resumes the same React window", async () => {
    const native = new NativeFixture();
    const pending = deferred<unknown>();
    native.enqueue("daemon_health", () => pending.promise);
    const view = mountDesktop(native);
    const main = view.getByRole("main");
    await view.user.click(view.getByRole("button", { name: "Hide Dashboard" }));
    expect(native.count("hide_dashboard")).toBe(1);
    expect(view.clock.pending).toBe(0);
    await act(async () => {
      pending.resolve(golden.health);
    });
    await settle();
    expect(native.count("recent_jobs")).toBe(0);
    expect(view.queryByText("Daemon connected")).toBeNull();
    await advance(view.clock, 60_000);
    expect(native.count("daemon_health")).toBe(1);
    await view.user.click(view.getByRole("button", { name: "Dashboard" }));
    await settle();
    expect(native.count("open_dashboard")).toBe(1);
    expect(view.getByRole("main")).toBe(main);
    expect(view.getByText("Daemon connected")).toBeDefined();
    expect(native.count("daemon_health")).toBe(2);
    expect(native.count("job_by_id")).toBe(1);
  });

  test("reopen arriving before a retired read settles queues one fresh cycle and never overlaps reads", async () => {
    const native = new NativeFixture();
    const pending = deferred<unknown>();
    native.enqueue("daemon_health", () => pending.promise);
    const view = mountDesktop(native);
    await settle(); // Governor read precedes the deliberately pending health read.
    await act(async () => {
      view.visibility.hide();
      view.visibility.show();
      view.visibility.show();
    });
    expect(native.count("daemon_health")).toBe(1);
    await act(async () => {
      pending.resolve(golden.health);
    });
    await settle();
    expect(native.count("daemon_health")).toBe(2);
    expect(native.count("recent_jobs")).toBe(1);
    expect(native.maxInFlight).toBe(1);
    expect(view.getByText("Daemon connected")).toBeDefined();
  });

  test("desktop-only quit requests quit_desktop, stops refreshes and never sends a daemon action", async () => {
    const view = mountDesktop();
    await settle();
    expect(
      view.getByText(/Closing this window hides ARC Desktop/),
    ).toBeDefined();
    const jobs = view.native.count("job_by_id");
    await view.user.click(
      view.getByRole("button", { name: "Quit Desktop (Daemon continues)" }),
    );
    await settle();
    const calls = view.native.calls.length;
    expect(view.native.count("quit_desktop")).toBe(1);
    expect(view.clock.pending).toBe(0);
    await act(async () => {
      view.visibility.show();
      view.visibility.show();
    });
    await advance(view.clock, 300_000);
    expect(view.native.calls).toHaveLength(calls);
    expect(view.native.count("job_by_id")).toBe(jobs);
    expect(
      view.native.calls.every((call) =>
        [
          "governor_get_switch",
          "daemon_health",
          "meter_snapshot",
          "meter_summary",
          "policy_current",
          "policy_job",
          "recent_jobs",
          "job_by_id",
          "desktop_metadata",
          "quit_desktop",
        ].includes(call.command),
      ),
    ).toBe(true);
  });

  test("visibility hide/show retains selection and timestamped results, then refreshes once", async () => {
    const view = mountDesktop();
    await settle();
    const main = view.getByRole("main");
    await act(async () => {
      view.visibility.hide();
    });
    await advance(view.clock, 20_000);
    expect(view.native.count("daemon_health")).toBe(1);
    const pending = deferred<unknown>();
    view.native.enqueue("daemon_health", () => pending.promise);
    await act(async () => {
      view.visibility.show();
      view.visibility.show();
    });
    const selected = within(view.getByRole("region", { name: "Selected job" }));
    expect(selected.getByText(A)).toBeDefined();
    expect(selected.getByText("Succeeded · Exit code 0")).toBeDefined();
    expect(
      selected.getByText("Stale · showing last recorded data", {
        selector: '[aria-labelledby="selected-heading"] > p',
      }),
    ).toBeDefined();
    expect(
      selected
        .getByLabelText("Exact time for Status received by desktop")
        .querySelector("time")
        ?.getAttribute("datetime"),
    ).toBe(new Date(T).toISOString());
    await act(async () => {
      pending.resolve(golden.health);
    });
    await settle();
    expect(view.getByRole("main")).toBe(main);
    expect(view.native.count("daemon_health")).toBe(2);
    expect(
      selected.queryByText("Stale · showing last recorded data"),
    ).toBeNull();
  });

  test("StrictMode leaves one subscription and unmount clears timers and ignores pending detail", async () => {
    const native = new NativeFixture();
    const pending = deferred<unknown>();
    native.enqueue("job_by_id", () => pending.promise);
    const view = mountDesktop(native, undefined, undefined, true);
    await settle();
    expect(view.visibility.count).toBe(1);
    expect(native.maxInFlight).toBe(1);
    view.unmount();
    expect(view.visibility.count).toBe(0);
    expect(view.clock.pending).toBe(0);
    const count = native.calls.length;
    await act(async () => {
      pending.resolve(golden.exit_zero);
    });
    await settle();
    await advance(view.clock, 60_000);
    expect(native.calls).toHaveLength(count);
    expect(view.container.innerHTML).toBe("");
  });

  test("a failed local hide/quit action is visible and resumes inspection without leaking diagnostics", async () => {
    const native = new NativeFixture();
    native.enqueue("hide_dashboard", () =>
      Promise.reject("SECRET action diagnostic"),
    );
    const view = mountDesktop(native);
    await settle();
    await view.user.click(view.getByRole("button", { name: "Hide Dashboard" }));
    await settle();
    expect(
      view.getByText("The desktop action could not be completed. Try again."),
    ).toBeDefined();
    expect(view.container.textContent).not.toContain("SECRET");
    expect(view.getByText("Daemon connected")).toBeDefined();
    expect(native.count("daemon_health")).toBe(2);
  });

  test("the real document visibility adapter subscribes and cleans up without another lifecycle command", () => {
    let shown = 0;
    let hidden = 0;
    const before = Object.getOwnPropertyDescriptor(document, "hidden");
    Object.defineProperty(document, "hidden", {
      configurable: true,
      value: false,
    });
    const dispose = desktopVisibility.subscribe(
      () => shown++,
      () => hidden++,
    );
    Object.defineProperty(document, "hidden", {
      configurable: true,
      value: true,
    });
    document.dispatchEvent(new Event("visibilitychange"));
    expect(hidden).toBe(1);
    Object.defineProperty(document, "hidden", {
      configurable: true,
      value: false,
    });
    document.dispatchEvent(new Event("visibilitychange"));
    expect(shown).toBe(1);
    dispose();
    document.dispatchEvent(new Event("visibilitychange"));
    expect(shown).toBe(1);
    if (before) Object.defineProperty(document, "hidden", before);
    else Reflect.deleteProperty(document, "hidden");
  });
});
