import { afterEach, describe, expect, test } from "bun:test";
import { MeterReadState } from "@/dashboard/meter-read-state";
import {
  act,
  advance,
  B,
  cleanup,
  deferred,
  mountDesktop,
  NativeFixture,
  page,
  settle,
  T,
  within,
} from "./fixtures/desktop-test-harness";
import { registerJobMeterPresentationCases } from "./fixtures/job-meter-presentation-cases";
import {
  meterFixture,
  meterReply,
  nextMeter,
  sharedMeterSnapshot,
} from "./fixtures/meter-fixture";
import { registerMeterInteractionCases } from "./fixtures/meter-interaction-cases";
import {
  field,
  full,
  metrics,
  refresh,
  text,
} from "./fixtures/meter-test-harness";
import { registerPolicyReadCases } from "./fixtures/policy-read-cases";

const { formatMeterValue } = await import("@/dashboard/desktop-meter");
afterEach(cleanup);

describe("Dashboard host Meter", () => {
  registerPolicyReadCases();
  test("pending then normalized readings preserve focused failed-job selection and unknown attribution", async () => {
    const native = new NativeFixture(),
      pending = deferred<unknown>();
    native.recent = page([B]);
    native.enqueue("meter_snapshot", () => pending.promise);
    const v = mountDesktop(native);
    await settle();
    expect(field(v, "Host CPU").getByText("Unknown")).toBeDefined();
    expect(v.getAllByText("Failed · Exit code 17").length).toBeGreaterThan(0);
    const row = v.getByRole("option");
    await act(async () => {
      row.focus();
      pending.resolve(meterReply());
    });
    await settle();
    expect(document.activeElement).toBe(row);
    expect(row.getAttribute("aria-selected")).toBe("true");
    expect(field(v, "Host CPU").getByText("25.0%")).toBeDefined();
    expect(
      metrics(v).getByText("Host capacity · 2.000 s interval · 8 logical CPUs"),
    ).toBeDefined();
    expect(field(v, "Cumulative CPU").getByText("12.50 core·s")).toBeDefined();
    expect(field(v, "Current RSS").getByText("40.00 MiB")).toBeDefined();
    expect(field(v, "Available RAM").getByText("8.00 GiB")).toBeDefined();
    for (const label of [
      "Disk writes",
      "Storage reclaimed",
      "Active agents",
      "Global running jobs",
      "Global queued jobs",
      "Usage today",
    ])
      expect(metrics(v).getByText(label).nextElementSibling?.textContent).toBe(
        "Unknown",
      );
    expect(native.maxInFlight).toBe(1);
  });

  test("first interval and per-field unknown reasons remain distinct from a legitimate zero", async () => {
    const v = await full(sharedMeterSnapshot());
    expect(
      field(v, "Host CPU").getByText("Waiting for next sample"),
    ).toBeDefined();
    expect(field(v, "Host CPU").queryByText("0.0%")).toBeNull();
    expect(field(v, "Available RAM").getByText("0.00 GiB")).toBeDefined();
    expect(
      field(v, "Memory pressure (native)").getByText("Not supported"),
    ).toBeDefined();
    expect(field(v, "Swap used").getByText("Permission denied")).toBeDefined();
    expect(
      field(v, "Free space").getByText("Measurement failed"),
    ).toBeDefined();
    expect(metrics(v).queryByText(/Host capacity ·/)).toBeNull();
    expect(v.container.textContent).not.toContain("statfs_failed");
    await refresh(v, meterFixture());
    expect(field(v, "Host CPU").getByText("25.0%")).toBeDefined();
    expect(
      field(v, "Host CPU").queryByText("Waiting for next sample"),
    ).toBeNull();
  });

  test("healthy idle sampling stays fresh past job freshness; cached rereads never rejuvenate it", async () => {
    const v = await full();
    await advance(v.clock, 11_000);
    expect(metrics(v).getByRole("status").textContent).toContain(
      "Current sample",
    );
    expect(text(v, "Host CPU")).not.toContain("Stale");
    await advance(v.clock, 20_000);
    expect(text(v, "Host CPU")).not.toContain("Stale");
    await advance(v.clock, 1);
    expect(text(v, "Host CPU")).toContain("Stale");
    expect(v.getByText("Daemon connected")).toBeDefined();
    expect(v.queryByText("Disconnected")).toBeNull();
    await refresh(v, meterFixture(), T + 100_000);
    expect(text(v, "Host CPU")).toContain("Stale");
    expect(
      field(v, "Host CPU")
        .getByLabelText("Exact time for Host CPU observed")
        .querySelector("time")
        ?.getAttribute("datetime"),
    ).toBe(new Date(T).toISOString());
  });

  test("same sequence already aged by the daemon has a strict 31000/31001 boundary", async () => {
    const s = meterFixture();
    s.age_ms = 30_000;
    const v = await full(s);
    await advance(v.clock, 1000);
    expect(text(v, "Host CPU")).not.toContain("Stale");
    await advance(v.clock, 1);
    expect(text(v, "Host CPU")).toContain("Stale");
  });

  test("active, pressure and adjusted cadences use their own exact thresholds", async () => {
    for (const [mode, interval] of [
      ["active", 2000],
      ["pressure", 1000],
      ["active", 3000],
      ["idle", 10_000],
    ] as const) {
      const s = meterFixture();
      s.cadence = {
        mode,
        interval_ms: interval,
        stale_after_ms: 2 * interval + 1000,
      };
      const v = await full(s);
      await advance(v.clock, s.cadence.stale_after_ms);
      expect(text(v, "Host CPU")).not.toContain("Stale");
      await advance(v.clock, 1);
      expect(text(v, "Host CPU")).toContain("Stale");
      v.unmount();
    }
  });

  test("the existing refresh timer follows active cadence without another polling owner", async () => {
    const s = meterFixture();
    s.cadence = { mode: "active", interval_ms: 2000, stale_after_ms: 5000 };
    const v = await full(s);
    await advance(v.clock, 1999);
    expect(v.native.count("meter_snapshot")).toBe(1);
    await advance(v.clock, 1);
    expect(v.native.count("meter_snapshot")).toBe(2);
    expect(v.native.count("daemon_health")).toBe(2);
    expect(v.native.maxInFlight).toBe(1);
  });

  test("fresh health and jobs do not hide stale Meter; fresh Meter does not erase detail failure", async () => {
    const s = meterFixture();
    s.age_ms = 31_001;
    const v = await full(s);
    expect(text(v, "Host CPU")).toContain("Stale");
    const selected = within(v.getByRole("region", { name: "Selected job" }));
    expect(selected.queryByText(/Stale ·/)).toBeNull();
    v.native.enqueue("job_by_id", () =>
      Promise.reject({ code: "uncertain", retryable: true }),
    );
    await refresh(v, nextMeter(s));
    expect(text(v, "Host CPU")).not.toContain("Stale");
    expect(
      selected.getByText("Stale · showing last recorded data"),
    ).toBeDefined();
    expect(selected.getByText("Succeeded · Exit code 0")).toBeDefined();
    expect(v.getByText("Daemon connected")).toBeDefined();
  });

  test("a failed RSS keeps other fields and an honestly old sampled peak; recovery confirms the same maximum", async () => {
    const s = meterFixture(),
      v = await full(s),
      next = nextMeter(s, 40_000);
    next.daemon.rss_bytes.reading = {
      availability: "failed",
      diagnostic: "private-native-label",
    };
    next.daemon.observed_peak_rss_bytes = structuredClone(
      s.daemon.observed_peak_rss_bytes,
    );
    await refresh(v, next);
    expect(
      field(v, "Current RSS").getByText("Measurement failed"),
    ).toBeDefined();
    expect(field(v, "Observed peak RSS").getByText("44.00 MiB")).toBeDefined();
    expect(text(v, "Observed peak RSS")).toContain("Stale");
    expect(text(v, "Available RAM")).not.toContain("Stale");
    expect(v.container.textContent).not.toContain("private-native-label");
    const recovered = nextMeter(next);
    recovered.daemon.rss_bytes.reading = {
      availability: "available",
      value: 39 * 2 ** 20,
    };
    recovered.daemon.observed_peak_rss_bytes.observed_monotonic_ms =
      recovered.sampled_monotonic_ms;
    recovered.daemon.observed_peak_rss_bytes.observed_at_ms = T + 42_000;
    await refresh(v, recovered);
    expect(text(v, "Observed peak RSS")).not.toContain("Stale");
    expect(field(v, "Observed peak RSS").getByText("44.00 MiB")).toBeDefined();
    expect(
      metrics(v)
        .getByLabelText("Exact time for Peak observation started")
        .querySelector("time")
        ?.getAttribute("datetime"),
    ).toBe(new Date(T - 30_000).toISOString());
    expect(
      field(v, "Observed peak RSS")
        .getByLabelText("Exact time for Peak last observed")
        .querySelector("time")
        ?.getAttribute("datetime"),
    ).toBe(new Date(T + 42_000).toISOString());
  });

  test("native filesystem bytes, full local identity and derived pressure remain separately labeled", async () => {
    const s = meterFixture(),
      v = await full(s);
    for (const [label, value] of [
      ["Capacity", "512.00 GiB"],
      ["Free space", "128.00 GiB"],
      ["Available to ARC user", "120.00 GiB"],
    ])
      expect(field(v, label ?? "").getByText(value ?? "")).toBeDefined();
    const identity = metrics(v).getByText("Local filesystem details");
    await v.user.click(identity);
    expect(identity.parentElement?.textContent).toContain(
      "synthetic-local-volume",
    );
    expect(identity.parentElement?.textContent).toContain(
      "Local filesystem: Yes",
    );
    for (const [ratio, level] of [
      [0.05, "critical"],
      [0.1, "warning"],
      [0.11, "normal"],
    ] as const) {
      const next = nextMeter(s);
      s.sequence = next.sequence;
      next.host.filesystem.available_bytes.reading = {
        availability: "available",
        value: Math.floor(512 * 2 ** 30 * ratio),
      };
      next.host.filesystem.pressure.reading = {
        availability: "available",
        value: level,
      };
      await refresh(v, next);
      expect(
        field(v, "Disk pressure (derived)").getByText(
          level[0]?.toUpperCase() + level.slice(1),
        ),
      ).toBeDefined();
      expect(
        field(v, "Disk pressure (derived)").getByText("Derived · level"),
      ).toBeDefined();
      expect(field(v, "Capacity").getByText("Native · bytes")).toBeDefined();
    }
  });

  test("positive values below precision retain exact bytes; legitimate zero and unknown never merge", async () => {
    expect(formatMeterValue(1, "GiB")).toBe("<0.01 GiB");
    expect(formatMeterValue(1, "MiB")).toBe("<0.01 MiB");
    expect(formatMeterValue(0.001, "core_seconds")).toBe("<0.01 core·s");
    expect(formatMeterValue(0.01, "percent")).toBe("<0.1%");
    const s = meterFixture(),
      v = await full(s);
    expect(field(v, "Swap used").getByText("0.00 GiB")).toBeDefined();
    s.host.swap_used_bytes.reading = { availability: "available", value: 1 };
    s.sequence++;
    await refresh(v, s);
    const exact = field(v, "Swap used").getByText("Exact value for Swap used");
    await v.user.click(exact);
    expect(exact.parentElement?.textContent).toContain("1 bytes");
    for (const availability of ["unsupported", "denied", "failed"] as const) {
      s.sequence++;
      s.host.swap_used_bytes.reading =
        availability === "failed"
          ? { availability, diagnostic: "swap_failed" }
          : { availability };
      await refresh(v, s);
      expect(field(v, "Swap used").getByText("Unknown")).toBeDefined();
      expect(field(v, "Swap used").queryByText("0.00 GiB")).toBeNull();
    }
  });

  test("field, sample, receipt and health times are distinct; null and out-of-Date-range values are safe", async () => {
    const s = meterFixture();
    s.host.swap_used_bytes.observed_at_ms = T - 500;
    const v = await full(s);
    const check = (label: string, ms: number) =>
      expect(
        metrics(v)
          .getByLabelText(`Exact time for ${label}`)
          .querySelector("time")
          ?.getAttribute("datetime"),
      ).toBe(new Date(ms).toISOString());
    check("Meter sampled", T);
    check("Swap used observed", T - 500);
    const next = nextMeter(s);
    const interval = next.host.cpu_interval;
    if (!interval) throw new Error("Expected synthetic CPU interval");
    next.host.cpu_interval = {
      ...interval,
      started_at_ms: null,
      ended_at_ms: null,
    };
    next.host.swap_used_bytes.observed_at_ms = null;
    next.sampled_at_ms = Number.MAX_SAFE_INTEGER;
    await refresh(v, next, T + 3_600_000);
    check("Meter received by desktop", T + 3_600_000);
    expect(field(v, "Swap used").getByText("Unknown")).toBeDefined();
    const sampled = metrics(v).getByLabelText("Exact time for Meter sampled");
    expect(sampled.textContent).toBe(String(Number.MAX_SAFE_INTEGER));
    expect(sampled.querySelector("time")?.hasAttribute("datetime")).toBe(false);
    expect(
      v.getByText("Daemon health observation time").nextElementSibling
        ?.textContent,
    ).toBe("Unknown");
  });

  test("wall-clock receipts can jump while monotonic freshness and CPU interval remain intact", async () => {
    const v = await full();
    await advance(v.clock, 1000);
    await refresh(v, meterFixture(), T - 3_600_000);
    await advance(v.clock, 1000);
    await refresh(v, meterFixture(), T + 3_600_000);
    expect(text(v, "Host CPU")).not.toContain("Stale");
    expect(metrics(v).getByText("Elapsed: 2000 ms (monotonic)")).toBeDefined();
    await advance(v.clock, 29_001);
    expect(text(v, "Host CPU")).toContain("Stale");
    const state = new MeterReadState();
    state.accept(meterReply(), 1000);
    state.expire(999);
    expect(state.section.stale).toBe(true);
    state.accept(meterReply(), 1001);
    expect(state.section.stale).toBe(false);
  });

  test("unsupported old daemons are probed once per epoch; scheduled health/jobs keep working", async () => {
    const native = new NativeFixture();
    native.enqueue("meter_snapshot", () =>
      Promise.reject({ code: "unsupported", retryable: false }),
    );
    const v = mountDesktop(native);
    await settle();
    expect(metrics(v).getByRole("status").textContent).toContain(
      "Meter not supported by this daemon",
    );
    await advance(v.clock, 5000);
    expect(native.count("meter_snapshot")).toBe(1);
    expect(native.count("daemon_health")).toBe(2);
    expect(native.count("job_by_id")).toBe(2);
    await refresh(v, meterFixture());
    expect(native.count("meter_snapshot")).toBe(2);
    expect(field(v, "Host CPU").getByText("25.0%")).toBeDefined();
  });

  test("Meter failures and invalid replies retain stale readings without disabling automatic job inspection", async () => {
    const v = await full();
    for (const failure of [
      { code: "unavailable", retryable: true },
      { code: "uncertain", retryable: true },
    ] as const) {
      v.native.enqueue("meter_snapshot", () => Promise.reject(failure));
      await refresh(v);
      expect(v.getByText("Daemon connected")).toBeDefined();
      expect(metrics(v).getByRole("status").textContent).toContain(
        "Meter unavailable",
      );
      expect(text(v, "Current RSS")).toContain("Stale");
    }
    v.native.meter = {
      data: { ...meterFixture(), private: "SECRET" },
      received_at_ms: T,
    };
    await refresh(v);
    expect(metrics(v).getByText(/Invalid daemon response/)).toBeDefined();
    const count = v.native.count("job_by_id");
    await advance(v.clock, 5000);
    expect(v.native.count("job_by_id")).toBe(count + 1);
    expect(v.container.textContent).not.toContain("SECRET");
  });

  test("disconnect and coalesced retries retain selection/outcome; health recovery alone cannot freshen Meter", async () => {
    const v = await full();
    const selected = v.getAllByRole("option")[1];
    if (!selected) throw new Error("Expected synthetic job B");
    await v.user.click(selected);
    await settle();
    v.native.healthFailure = { code: "unavailable", retryable: true };
    await refresh(v);
    expect(v.getByText("Disconnected")).toBeDefined();
    expect(text(v, "Current RSS")).toContain("Stale");
    await advance(v.clock, 999);
    const pending = deferred<unknown>();
    v.native.healthFailure = null;
    v.native.enqueue("meter_snapshot", () => pending.promise);
    await advance(v.clock, 1);
    expect(v.getByText("Daemon connected")).toBeDefined();
    expect(text(v, "Current RSS")).toContain("Stale");
    expect(v.getAllByText("Failed · Exit code 17").length).toBeGreaterThan(0);
    await act(async () => {
      pending.resolve(meterReply(nextMeter(meterFixture())));
    });
    await settle();
    expect(text(v, "Current RSS")).not.toContain("Stale");
    expect(v.native.maxInFlight).toBe(1);
  });

  test("same PID with a new start token resets daemon accounting and rejects lower or retired samples", async () => {
    const v = await full();
    const restarted = sharedMeterSnapshot();
    restarted.daemon_instance.start_token = "123457";
    await refresh(v, restarted);
    expect(metrics(v).getByRole("status").textContent).toContain(
      "Daemon restarted · new accounting window",
    );
    expect(
      field(v, "Host CPU").getByText("Waiting for next sample"),
    ).toBeDefined();
    expect(field(v, "Observed peak RSS").queryByText("44.00 MiB")).toBeNull();
    expect(field(v, "Cumulative CPU").getByText("0.00 core·s")).toBeDefined();
    const next = nextMeter(restarted);
    await refresh(v, next);
    expect(metrics(v).getByRole("status").textContent).not.toContain(
      "Daemon restarted",
    );
    await refresh(v, restarted);
    expect(metrics(v).getByText(/Invalid daemon response/)).toBeDefined();
    const old = meterFixture();
    old.sequence = 999;
    await refresh(v, old);
    expect(field(v, "Cumulative CPU").queryByText("12.50 core·s")).toBeNull();
  });

  registerMeterInteractionCases();
  registerJobMeterPresentationCases();
});
