import { expect, test } from "bun:test";
import type { JobDetail } from "@/bridge/types";
import { formatJobMeterValue } from "@/dashboard/job-meter-panel";
import {
  A,
  act,
  advance,
  B,
  clone,
  deferred,
  found,
  golden,
  mountDesktop,
  NativeFixture,
  settle,
  within,
} from "./desktop-test-harness";
import {
  jobMeterFixture,
  jobMeterReply,
  jobReport,
  unknownJobMeter,
} from "./job-meter-fixture";

type View = ReturnType<typeof mountDesktop>;
export function jobRow(v: View, index: number): HTMLElement {
  const row = v.getAllByRole("option")[index];
  if (!row) throw new Error("Missing synthetic job row");
  return row;
}
const region = (v: View) =>
  within(v.getByRole("region", { name: "Job measurements" }));
const field = (v: View, label: string) => {
  const element = region(v).getByText(label).closest("div");
  if (!element) throw new Error("Missing job metric");
  return within(element);
};
async function full(s = jobMeterFixture()) {
  const native = new NativeFixture();
  native.jobMeters.set(s.job_id, jobMeterReply(s));
  const v = mountDesktop(native);
  if (s.job_id === B) {
    await settle();
    await v.user.click(jobRow(v, 1));
  }
  await settle();
  return v;
}
async function refresh(v: View) {
  await v.user.click(v.getByRole("button", { name: "Refresh status" }));
  await settle();
}

export function registerJobMeterPresentationCases() {
  test("job summary is inline, cumulative, clocked independently and exact values stay disclosed", async () => {
    const v = await full();
    expect(field(v, "CPU used").getByText("2 core·s")).toBeDefined();
    expect(field(v, "Run elapsed").getByText("10 s")).toBeDefined();
    expect(field(v, "Queue wait").getByText("3 s")).toBeDefined();
    expect(region(v).getByText("Complete for recorded scope")).toBeDefined();
    expect(region(v).getByText("native_waited_descendants")).toBeDefined();
    expect(
      region(v).getByText(/includes stopped time and excludes queue wait/),
    ).toBeDefined();
    const summary = region(v).getByText("Accounting details");
    await v.user.click(summary);
    expect(summary.parentElement?.textContent).toContain("1.25 core·s");
    expect(summary.parentElement?.textContent).toContain("0.75 core·s");
    expect(summary.parentElement?.textContent).toContain(
      "native_owned_child_wait4",
    );
    expect(summary.parentElement?.textContent).toContain(
      "queued_to_reserved / daemon_wall_clock",
    );
    expect(summary.parentElement?.textContent).toContain(
      "PID 101 · start token 1002",
    );
    expect(region(v).queryByText(/%/)).toBeNull();
    expect(v.native.maxInFlight).toBe(1);
  });

  test("measured zero, tiny positive and Unknown stay distinct with exact disclosure", async () => {
    expect(formatJobMeterValue(0, "core·s")).toBe("0 core·s");
    expect(formatJobMeterValue(null, "s")).toBe("Unknown");
    expect(formatJobMeterValue(0.0002, "core·s")).toBe("<0.001 core·s");
    const s = jobMeterFixture(),
      r = jobReport(s);
    r.cpu_core_seconds = { user: 0.0002, system: 0, total: 0.0002 };
    r.run_elapsed.seconds = 0.0002;
    s.queue_wait = {
      seconds: 0,
      clock: "not_applicable",
      basis: "lifecycle_only",
    };
    const v = await full(s);
    expect(field(v, "CPU used").getByText("<0.001 core·s")).toBeDefined();
    expect(field(v, "Run elapsed").getByText("<0.001 s")).toBeDefined();
    expect(field(v, "Queue wait").getByText("0 s")).toBeDefined();
    await v.user.click(region(v).getByText("Accounting details"));
    expect(
      region(v).getByText("Exact CPU used").nextElementSibling?.textContent,
    ).toBe("0.0002 core·s");
    v.unmount();
    const zero = jobMeterFixture();
    jobReport(zero).cpu_core_seconds = { user: 0, system: 0, total: 0 };
    const z = await full(zero);
    expect(field(z, "CPU used").getByText("0 core·s")).toBeDefined();
  });

  test("Partial reasons remain beside CPU; exit 17 and nested inclusion keep independent meanings", async () => {
    const s = jobMeterFixture(B),
      r = jobReport(s);
    r.completeness = "partial";
    r.missing_coverage = ["unobserved_descendants", "detached_descendants"];
    r.accounting_relation = { kind: "included_in_root", root_job_id: A };
    const v = await full(s);
    expect(
      field(v, "CPU used").getByText(
        /Partial — Unobserved descendants may be absent; Detached descendants/,
      ),
    ).toBeDefined();
    expect(
      region(v).getByText(`Included in accounting root ${A}; not additive.`),
    ).toBeDefined();
    expect(v.getAllByText("Failed · Exit code 17").length).toBeGreaterThan(0);
    expect(field(v, "Run elapsed").getByText("10 s")).toBeDefined();
    expect(region(v).queryByText("Complete for recorded scope")).toBeNull();
    expect(region(v).queryByText("4 core·s")).toBeNull();
  });

  test("unavailable CPU preserves measured run/queue while explicit Unknown reasons never become zero", async () => {
    const s = jobMeterFixture(),
      r = jobReport(s);
    r.cpu_core_seconds = { user: null, system: null, total: null };
    r.completeness = "unknown";
    r.missing_coverage = ["native_usage_unavailable", "lineage_unverified"];
    r.accounting_relation = { kind: "unverified" };
    const v = await full(s);
    expect(field(v, "CPU used").getByText("Unknown")).toBeDefined();
    expect(
      field(v, "CPU used").getByText(/Native CPU accounting unavailable/),
    ).toBeDefined();
    expect(field(v, "Run elapsed").getByText("10 s")).toBeDefined();
    expect(field(v, "Queue wait").getByText("3 s")).toBeDefined();
    expect(
      region(v).getByText("Accounting lineage unverified; not additive."),
    ).toBeDefined();
    for (const [reason, text] of [
      ["legacy", "Legacy measurement unavailable"],
      [
        "interrupted",
        "Accounting interrupted; no terminal measurements recorded.",
      ],
      ["not_terminated", "Waiting for termination accounting."],
      ["unavailable", "No job measurements recorded."],
    ] as const) {
      v.unmount();
      const next = await full(unknownJobMeter(A, reason));
      expect(region(next).getByText(text)).toBeDefined();
      for (const label of ["CPU used", "Run elapsed", "Queue wait"]) {
        expect(field(next, label).getByText("Unknown")).toBeDefined();
        expect(field(next, label).queryByText("0 s")).toBeNull();
      }
      await advance(next.clock, 5000);
      expect(field(next, "Run elapsed").getByText("Unknown")).toBeDefined();
      next.unmount();
    }
  });

  test("native signal outcome and accounting quality are displayed independently", async () => {
    const native = new NativeFixture(),
      s = jobMeterFixture();
    const j = clone(golden.exit_zero.data.job) as JobDetail;
    j.phase = "SIGNALED";
    j.reason = "OBSERVED_SIGNAL";
    j.observation.reason = "OBSERVED_SIGNAL";
    j.outcome = { kind: "SIGNALED", signal: 15, core_dumped: false };
    jobReport(s).outcome = j.outcome;
    native.jobs.set(A, found(j));
    native.jobMeters.set(A, jobMeterReply(s));
    const v = mountDesktop(native);
    await settle();
    expect(v.getAllByText("Terminated by signal 15").length).toBeGreaterThan(0);
    expect(field(v, "CPU used").getByText("2 core·s")).toBeDefined();
    expect(region(v).getByText("Complete for recorded scope")).toBeDefined();
  });

  test("initial and subsequent pending reads label only the measurement region busy and retain matching values", async () => {
    const native = new NativeFixture(),
      pending = deferred<unknown>();
    native.enqueue("meter_summary", () => pending.promise);
    const v = mountDesktop(native);
    await settle();
    expect(
      v
        .getByRole("region", { name: "Job measurements" })
        .getAttribute("aria-busy"),
    ).toBe("true");
    expect(
      v.getByRole("region", { name: "Selected job" }).getAttribute("aria-busy"),
    ).toBe("false");
    expect(region(v).getByText("Loading job measurements…")).toBeDefined();
    expect(field(v, "CPU used").getByText("Unknown")).toBeDefined();
    await act(async () => pending.resolve(jobMeterReply()));
    await settle();
    const next = deferred<unknown>();
    native.enqueue("meter_summary", () => next.promise);
    await refresh(v);
    expect(region(v).getByText("Refreshing job measurements…")).toBeDefined();
    expect(field(v, "CPU used").getByText("2 core·s")).toBeDefined();
    await act(async () => next.resolve(jobMeterReply()));
    await settle();
  });
}

export {
  field as jobField,
  full as fullJobMeter,
  refresh as refreshJobMeter,
  region as jobRegion,
};
