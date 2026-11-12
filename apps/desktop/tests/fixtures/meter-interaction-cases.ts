import { expect, test } from "bun:test";
import { act, deferred, settle } from "./desktop-test-harness";
import { meterFixture, meterReply, nextMeter } from "./meter-fixture";
import { field, full, metrics, refresh } from "./meter-test-harness";

export function registerMeterInteractionCases() {
  test("same-sequence mutation is rejected, while JSON key ordering is immaterial", async () => {
    const v = await full(),
      changed = meterFixture();
    changed.host.cpu_utilization.reading = {
      availability: "available",
      value: 99,
    };
    await refresh(v, changed);
    expect(field(v, "Host CPU").getByText("25.0%")).toBeDefined();
    expect(metrics(v).getByText(/Invalid daemon response/)).toBeDefined();
    const s = meterFixture();
    v.native.meter = {
      ...meterReply(),
      data: Object.fromEntries(Object.entries(s).reverse()),
    };
    await refresh(v);
    expect(metrics(v).queryByText(/Invalid daemon response/)).toBeNull();
  });

  test("selection changes retire pending Meter data and late job replies without a second read owner", async () => {
    const v = await full(),
      pending = deferred<unknown>();
    v.native.enqueue("meter_snapshot", () => pending.promise);
    await v.user.click(v.getByRole("button", { name: "Refresh status" }));
    await settle();
    const row = v.getAllByRole("option")[1];
    if (!row) throw new Error("Expected synthetic job B");
    await v.user.click(row);
    const late = nextMeter(meterFixture());
    late.host.cpu_utilization.reading = {
      availability: "available",
      value: 99,
    };
    await act(async () => {
      pending.resolve(meterReply(late));
    });
    await settle();
    expect(field(v, "Host CPU").getByText("25.0%")).toBeDefined();
    expect(v.getAllByText("Failed · Exit code 17").length).toBeGreaterThan(0);
    expect(v.getAllByRole("option")[1]?.getAttribute("aria-selected")).toBe(
      "true",
    );
    expect(v.native.maxInFlight).toBe(1);
    expect(
      v.native.calls.every((c) => !/execute|stop|shutdown/.test(c.command)),
    ).toBe(true);
  });

  test("meaningful status transitions announce once; numeric samples and time changes are outside live regions", async () => {
    const s = meterFixture(),
      v = await full(s);
    const status = metrics(v).getByRole("status"),
      mutations: MutationRecord[] = [];
    const observer = new MutationObserver((records) =>
      mutations.push(...records),
    );
    observer.observe(status, {
      characterData: true,
      childList: true,
      subtree: true,
    });
    try {
      const numeric = nextMeter(s);
      numeric.host.cpu_utilization.reading = {
        availability: "available",
        value: 26,
      };
      await refresh(v, numeric);
      expect([...mutations, ...observer.takeRecords()]).toHaveLength(0);
      expect(
        field(v, "Host CPU").getByText("26.0%").closest("[aria-live]"),
      ).toBeNull();
      const warning = nextMeter(numeric);
      warning.host.memory_pressure.reading = {
        availability: "available",
        value: "warning",
      };
      await refresh(v, warning);
      expect(status.textContent).toContain("Memory pressure: Warning");
      expect([...mutations, ...observer.takeRecords()].length).toBeGreaterThan(
        0,
      );
      mutations.length = 0;
      await refresh(v, nextMeter(warning));
      expect([...mutations, ...observer.takeRecords()]).toHaveLength(0);
      const exact = field(v, "Host CPU").getByLabelText(
        "Exact time for Host CPU observed",
      );
      exact.focus();
      await v.user.keyboard("{Enter}");
      expect(document.activeElement).toBe(exact);
    } finally {
      observer.disconnect();
    }
  });
}
