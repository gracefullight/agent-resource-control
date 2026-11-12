import { expect, test } from "bun:test";
import {
  A,
  act,
  advance,
  B,
  deferred,
  mountDesktop,
  NativeFixture,
  page,
  settle,
  T,
} from "./desktop-test-harness";
import {
  jobMeterFixture,
  jobMeterReply,
  jobReport,
  unknownQueue,
} from "./job-meter-fixture";
import {
  fullJobMeter,
  jobField,
  jobRegion,
  jobRow,
  refreshJobMeter,
} from "./job-meter-presentation-cases";

const error = { code: "uncertain", retryable: true };

export function registerJobMeterInteractionCases() {
  test("late A summary successes and failures never appear under B or a new A generation", async () => {
    for (const returningToA of [false, true])
      for (const failure of [false, true]) {
        const v = await fullJobMeter(),
          pending = deferred<unknown>();
        const b = jobMeterFixture(B);
        jobReport(b).cpu_core_seconds = { user: 6, system: 1, total: 7 };
        v.native.jobMeters.set(B, jobMeterReply(b));
        v.native.enqueue("meter_summary", () => pending.promise);
        await refreshJobMeter(v);
        await v.user.click(jobRow(v, 1));
        expect(jobField(v, "CPU used").queryByText("2 core·s")).toBeNull();
        if (returningToA) await v.user.click(jobRow(v, 0));
        const bad = jobMeterFixture();
        jobReport(bad).cpu_core_seconds = { user: 99, system: 0, total: 99 };
        await act(async () => {
          if (failure) pending.reject(error);
          else pending.resolve(jobMeterReply(bad));
        });
        await settle();
        expect(
          jobField(v, "CPU used").getByText(
            returningToA ? "2 core·s" : "7 core·s",
          ),
        ).toBeDefined();
        expect(jobRegion(v).queryByText("99 core·s")).toBeNull();
        expect(jobRegion(v).queryByText(/deadline/)).toBeNull();
        expect(v.native.maxInFlight).toBe(1);
        v.unmount();
      }
  });

  test("summary-only transport failure retains values and coverage while healthy detail stays fresh", async () => {
    const s = jobMeterFixture();
    jobReport(s).completeness = "partial";
    jobReport(s).missing_coverage = ["unobserved_descendants"];
    const v = await fullJobMeter(s);
    v.native.enqueue("meter_summary", () => Promise.reject(error));
    await refreshJobMeter(v);
    expect(jobField(v, "CPU used").getByText("2 core·s")).toBeDefined();
    expect(jobField(v, "CPU used").getByText(/Partial —/)).toBeDefined();
    expect(
      jobRegion(v).getByText("Stale · showing last recorded data"),
    ).toBeDefined();
    expect(
      jobRegion(v).getByText(/Job measurements unavailable.*deadline/),
    ).toBeDefined();
    expect(v.getByText("Daemon connected")).toBeDefined();
    expect(v.getAllByText("Succeeded · Exit code 0").length).toBeGreaterThan(0);
    await refreshJobMeter(v);
    expect(
      jobRegion(v).queryByText("Stale · showing last recorded data"),
    ).toBeNull();
    expect(jobField(v, "CPU used").getByText(/Partial —/)).toBeDefined();
    v.unmount();
    const native = new NativeFixture();
    native.enqueue("meter_summary", () => Promise.reject(error));
    const next = mountDesktop(native);
    await settle();
    expect(jobField(next, "CPU used").getByText("Unknown")).toBeDefined();
    expect(
      jobRegion(next).queryByText("Last recorded measurements"),
    ).toBeNull();
  });

  test("unsupported is probed once until manual refresh or reconnect; malformed replies do not stop job reads", async () => {
    const native = new NativeFixture();
    native.enqueue("meter_summary", () =>
      Promise.reject({ code: "unsupported", retryable: false }),
    );
    const v = mountDesktop(native);
    await settle();
    expect(
      jobRegion(v).getByText(
        "Job measurements are not supported by this daemon.",
      ),
    ).toBeDefined();
    await advance(v.clock, 5000);
    expect(native.count("meter_summary")).toBe(1);
    expect(native.count("job_by_id")).toBe(2);
    native.jobMeters.set(A, jobMeterReply());
    await refreshJobMeter(v);
    expect(jobField(v, "CPU used").getByText("2 core·s")).toBeDefined();
    native.jobMeters.set(A, {
      ...jobMeterReply(),
      private: "PRIVATE_SENTINEL",
    });
    await refreshJobMeter(v);
    expect(jobRegion(v).getByText(/Invalid daemon response/)).toBeDefined();
    const calls = native.count("meter_summary"),
      jobs = native.count("job_by_id");
    await advance(v.clock, 5000);
    expect(native.count("meter_summary")).toBe(calls);
    expect(native.count("job_by_id")).toBe(jobs + 1);
    expect(v.container.textContent).not.toContain("PRIVATE_SENTINEL");
    expect(jobField(v, "CPU used").getByText("2 core·s")).toBeDefined();
  });

  test("wrong-ID and conflicting immutable reports are rejected; reordered duplicates refresh only retrieval", async () => {
    const v = await fullJobMeter();
    const changed = jobMeterFixture();
    jobReport(changed).report_id = "a".repeat(32);
    for (const s of [jobMeterFixture(B), changed]) {
      v.native.jobMeters.set(A, jobMeterReply(s));
      await refreshJobMeter(v);
      expect(jobRegion(v).getByText(/Invalid daemon response/)).toBeDefined();
      expect(jobField(v, "CPU used").getByText("2 core·s")).toBeDefined();
    }
    const valid = jobMeterFixture(),
      r = jobReport(valid);
    const reordered = {
      ...valid,
      accounting: {
        state: "recorded",
        report: Object.fromEntries(Object.entries(r).reverse()),
      },
    };
    v.native.jobMeters.set(A, {
      data: Object.fromEntries(Object.entries(reordered).reverse()),
      received_at_ms: T + 1000,
    });
    await refreshJobMeter(v);
    expect(jobRegion(v).queryByText(/Invalid daemon response/)).toBeNull();
    expect(jobField(v, "CPU used").getByText("2 core·s")).toBeDefined();
    expect(
      jobRegion(v)
        .getByLabelText("Exact time for Measurements received by desktop")
        .querySelector("time")
        ?.getAttribute("datetime"),
    ).toBe(new Date(T + 1000).toISOString());
  });

  test("disconnect/restart retains accepted job accounting through a pending new read and host reset", async () => {
    const v = await fullJobMeter();
    v.native.healthFailure = { code: "unavailable", retryable: true };
    await refreshJobMeter(v);
    expect(v.getByText("Disconnected")).toBeDefined();
    expect(jobField(v, "CPU used").getByText("2 core·s")).toBeDefined();
    expect(jobRegion(v).getByText("Complete for recorded scope")).toBeDefined();
    expect(
      jobRegion(v).getByText("Stale · showing last recorded data"),
    ).toBeDefined();
    const pending = deferred<unknown>();
    v.native.healthFailure = null;
    v.native.enqueue("meter_summary", () => pending.promise);
    await advance(v.clock, 1000);
    expect(v.getByText("Daemon connected")).toBeDefined();
    expect(
      jobRegion(v).getByText("Refreshing job measurements…"),
    ).toBeDefined();
    await act(async () =>
      pending.resolve(jobMeterReply(jobMeterFixture(), T + 1000)),
    );
    await settle();
    expect(jobField(v, "CPU used").getByText("2 core·s")).toBeDefined();
    expect(
      jobRegion(v).queryByText("Stale · showing last recorded data"),
    ).toBeNull();
  });

  test("outside Recent and confirmed missing keep last recorded values; only explicit expiry removes them", async () => {
    const v = await fullJobMeter();
    v.native.recent = page([B]);
    await refreshJobMeter(v);
    expect(
      v.getByText("Selected job is outside the recent list."),
    ).toBeDefined();
    expect(jobField(v, "CPU used").getByText("2 core·s")).toBeDefined();
    const missing = jobMeterFixture();
    missing.accounting = { state: "missing" };
    missing.queue_wait = unknownQueue();
    v.native.jobMeters.set(A, jobMeterReply(missing, T + 1000));
    await refreshJobMeter(v);
    expect(
      jobRegion(v).getByText("No matching job measurements."),
    ).toBeDefined();
    expect(jobField(v, "CPU used").getByText("2 core·s")).toBeDefined();
    expect(jobRegion(v).getByText("Last recorded measurements")).toBeDefined();
    expect(jobRegion(v).queryByText(/expired under/)).toBeNull();
    const expired = structuredClone(missing);
    expired.accounting = { state: "expired" };
    v.native.jobMeters.set(A, jobMeterReply(expired));
    await refreshJobMeter(v);
    expect(
      jobRegion(v).getByText(
        "Job measurements expired under the 90-day metadata policy.",
      ),
    ).toBeDefined();
    expect(jobField(v, "CPU used").getByText("Unknown")).toBeDefined();
    expect(jobRegion(v).queryByText("2 core·s")).toBeNull();
    v.unmount();
    const native = new NativeFixture();
    native.jobMeters.set(A, jobMeterReply(missing));
    const next = mountDesktop(native);
    await settle();
    expect(
      jobRegion(next).getByText("No matching job measurements."),
    ).toBeDefined();
    expect(jobField(next, "CPU used").getByText("Unknown")).toBeDefined();
    next.unmount();
    const empty = new NativeFixture();
    empty.recent = page([]);
    const none = mountDesktop(empty);
    await settle();
    expect(empty.count("meter_summary")).toBe(0);
    expect(none.queryByRole("region", { name: "Job measurements" })).toBeNull();
  });

  test("keyboard selection does not poll on arrow-only focus; unchanged disclosure and status stay stable", async () => {
    const v = await fullJobMeter();
    const rowA = jobRow(v, 0),
      rowB = jobRow(v, 1),
      calls = v.native.count("meter_summary");
    await act(async () => rowA.focus());
    await v.user.keyboard("{ArrowDown}");
    await settle();
    expect(document.activeElement).toBe(rowB);
    expect(v.native.count("meter_summary")).toBe(calls);
    await v.user.keyboard("{Enter}");
    await settle();
    expect(
      v.native.calls.filter((c) => c.command === "meter_summary").at(-1)?.args,
    ).toEqual({ job_id: B });
    expect(document.activeElement).toBe(rowB);
    await act(async () => rowA.focus());
    await v.user.keyboard(" ");
    await settle();
    const disclosure = jobRegion(v).getByText("Accounting details");
    expect(disclosure.tagName).toBe("SUMMARY");
    await v.user.click(disclosure);
    disclosure.focus();
    expect(disclosure.parentElement?.hasAttribute("open")).toBe(true);
    const live = v
      .getByRole("region", { name: "Selected job" })
      .querySelector('[role="status"]');
    if (!live) throw new Error("Missing selected-job live region");
    const mutations: MutationRecord[] = [],
      observer = new MutationObserver((records) => mutations.push(...records));
    observer.observe(live, {
      childList: true,
      characterData: true,
      subtree: true,
    });
    await act(async () => {
      v.clock.advance(5000);
    });
    await settle();
    expect(document.activeElement).toBe(disclosure);
    expect(jobRegion(v).getByText("Accounting details")).toBe(disclosure);
    expect(disclosure.parentElement?.hasAttribute("open")).toBe(true);
    expect([...mutations, ...observer.takeRecords()]).toHaveLength(0);
    observer.disconnect();
    expect(v.native.maxInFlight).toBe(1);
  });

  test("hide/reopen retires old-epoch summary replies and coalesces repeated refreshes without hidden polling", async () => {
    const v = await fullJobMeter(),
      pending = deferred<unknown>();
    v.native.enqueue("meter_summary", () => pending.promise);
    await refreshJobMeter(v);
    await act(async () => {
      v.visibility.hide();
    });
    const calls = v.native.count("meter_summary");
    await advance(v.clock, 20_000);
    expect(v.native.count("meter_summary")).toBe(calls);
    await act(async () => {
      v.visibility.show();
    });
    const bad = jobMeterFixture();
    jobReport(bad).cpu_core_seconds = { user: 99, system: 0, total: 99 };
    await act(async () => pending.resolve(jobMeterReply(bad)));
    await settle();
    expect(jobField(v, "CPU used").getByText("2 core·s")).toBeDefined();
    expect(jobRegion(v).queryByText("99 core·s")).toBeNull();
    const count = v.native.count("meter_summary"),
      pendingAgain = deferred<unknown>();
    v.native.enqueue("meter_summary", () => pendingAgain.promise);
    const button = v.getByRole("button", { name: "Refresh status" });
    await v.user.click(button);
    await v.user.click(button);
    await settle();
    expect(v.native.count("meter_summary")).toBe(count + 1);
    v.unmount();
    await act(async () => pendingAgain.reject(error));
    await settle();
    await advance(v.clock, 20_000);
    expect(v.native.count("meter_summary")).toBe(count + 1);
    expect(v.native.maxInFlight).toBe(1);
  });
}
