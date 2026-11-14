import { afterEach, describe, expect, test } from "bun:test";
import type { DiagnosticResult } from "@/bridge/traffic-scripts.generated";
import { isTrafficDiagnosticResult } from "@/bridge/traffic-scripts-contract";
import type { JobDetail } from "@/bridge/types";
import { trafficMessages as m } from "@/dashboard/traffic-messages";
import { TrafficReadState } from "@/dashboard/traffic-read-state";
import {
  A,
  act,
  advance,
  B,
  cleanup,
  clone,
  deferred,
  found,
  golden,
  mountDesktop,
  NativeFixture,
  settle,
  T,
  within,
} from "./fixtures/desktop-test-harness";
import {
  trafficFixture,
  trafficReply,
  trafficWait,
} from "./fixtures/traffic-scripts-fixture";
import wire from "./fixtures/traffic-scripts-wire.json";

afterEach(cleanup);

async function readTraffic(
  owner: TrafficReadState,
  native: NativeFixture,
  result: DiagnosticResult,
  now = 0,
) {
  expect(isTrafficDiagnosticResult(result, A)).toBe(true);
  native.traffic.set(A, trafficReply(result));
  await owner.read(
    native.client,
    A,
    () => true,
    () => now,
    () => {},
  );
}

test("D11 pause/resume can clear an unspawned grant and regrant a higher generation, including missed polls", async () => {
  for (const observeReset of [true, false]) {
    const owner = new TrafficReadState();
    const native = new NativeFixture();
    const reserved = trafficFixture(A, "RESERVED");
    await readTraffic(owner, native, reserved);
    if (observeReset) {
      const paused = trafficFixture(A, "UNRESERVED");
      if (!paused.admission) throw new Error("Missing pause fixture");
      paused.admission.queue_wait_ms = 6;
      await readTraffic(owner, native, paused);
      expect(owner.section.error).toBeNull();
      expect(owner.section.value?.data).toEqual(paused);
      await readTraffic(owner, native, trafficWait(A));
      expect(owner.section.error).toBeNull();
    }
    const regrant = trafficFixture(A, "RESERVED");
    if (!regrant.admission) throw new Error("Missing regrant fixture");
    regrant.admission.permit_generation = 2;
    regrant.admission.granted_at_ms = 18;
    regrant.admission.reservation = { cpu_millis: 1500, memory_bytes: 2048 };
    await readTraffic(owner, native, regrant);
    expect(owner.section.error).toBeNull();
    expect(owner.section.value?.data).toEqual(regrant);
    const running = clone(regrant);
    if (!running.admission) throw new Error("Missing running fixture");
    running.admission.state = "RUNNING";
    running.admission.queue_wait_ms = 8;
    await readTraffic(owner, native, running);
    expect(owner.section.error).toBeNull();
    expect(owner.section.value?.data.admission?.state).toBe("RUNNING");
  }
});

test("D08 a retired generation cannot reactivate across a reset or explicit reprobe; a new grant can recover", async () => {
  const owner = new TrafficReadState();
  const native = new NativeFixture();
  const first = trafficFixture(A, "RESERVED");
  const paused = trafficFixture(A, "UNRESERVED");
  await readTraffic(owner, native, first);
  await readTraffic(owner, native, paused);
  expect(owner.section.error).toBeNull();
  for (let attempt = 0; attempt < 2; attempt++) {
    owner.reprobe();
    await readTraffic(owner, native, first);
    expect(owner.section.error?.code).toBe("invalid_reply");
    expect(owner.section.value?.data).toEqual(paused);
  }
  const second = clone(first);
  if (!second.admission) throw new Error("Missing second grant fixture");
  second.admission.permit_generation = 2;
  second.admission.granted_at_ms = 18;
  owner.reprobe();
  await readTraffic(owner, native, second);
  expect(owner.section.error).toBeNull();
  expect(owner.section.value?.data).toEqual(second);
  await readTraffic(owner, native, first);
  expect(owner.section.error?.code).toBe("invalid_reply");
  expect(owner.section.value?.data).toEqual(second);
});

test("D08 clock rollback does not permit running state or same-generation facts to regress", async () => {
  for (const change of [
    "state",
    "generation",
    "grant",
    "root",
    "resources",
  ] as const) {
    const owner = new TrafficReadState();
    const native = new NativeFixture();
    const running = trafficFixture(A, "RUNNING");
    running.recorded_at_ms = 100;
    await readTraffic(owner, native, running);
    const next = clone(running);
    if (!next.admission) throw new Error("Missing running fixture");
    next.recorded_at_ms = 90;
    if (change === "state") next.admission.state = "RESERVED";
    if (change === "generation") next.admission.permit_generation = 2;
    if (change === "grant") {
      next.admission.granted_at_ms = 13;
      next.admission.queue_wait_ms = 3;
    }
    if (change === "root") next.admission.root_job_id = B;
    if (change === "resources")
      next.admission.reservation = { cpu_millis: 1, memory_bytes: 1 };
    await readTraffic(owner, native, next);
    expect(owner.section.error?.code).toBe("invalid_reply");
    expect(owner.section.value?.data).toEqual(running);
  }
});

test("D08 diagnostic wall clocks may roll back while local receipt freshness stays monotonic", async () => {
  const owner = new TrafficReadState();
  const native = new NativeFixture();
  for (const [recorded_at_ms, localAt] of [
    [100, 0],
    [90, 1000],
    [110, 2000],
  ] as const) {
    const result = trafficWait(A);
    result.recorded_at_ms = recorded_at_ms;
    await readTraffic(owner, native, result, localAt);
    expect(owner.section.error).toBeNull();
    expect(owner.section.value?.data.recorded_at_ms).toBe(recorded_at_ms);
    expect(owner.expire(localAt + 9999, 10_000)).toBe(1);
    expect(owner.section.stale).toBe(false);
  }
  expect(native.count("traffic_job_diagnostics")).toBe(3);
  expect(owner.expire(12_000, 10_000)).toBeNull();
  expect(owner.section.stale).toBe(true);
});

test("D10 an ABANDONED job accepts a late authoritative outcome without reviving its permit", async () => {
  const owner = new TrafficReadState();
  const native = new NativeFixture();
  const result = trafficFixture(A, "RUNNING");
  if (!result.admission) throw new Error("Missing recovery fixture");
  result.admission.state = "ABANDONED";
  result.admission.permit_active = false;
  native.traffic.set(A, trafficReply(result));
  await owner.read(
    native.client,
    A,
    () => true,
    () => 0,
    () => {},
  );
  const next = clone(result);
  if (!next.admission) throw new Error("Missing recovered fixture");
  next.recorded_at_ms = 21;
  next.admission.state = "COMPLETED";
  next.admission.outcome = { kind: "EXITED", code: 17 };
  native.traffic.set(A, trafficReply(next));
  await owner.read(
    native.client,
    A,
    () => true,
    () => 1,
    () => {},
  );
  expect(owner.section.error).toBeNull();
  expect(owner.section.value?.data.admission?.outcome).toEqual({
    kind: "EXITED",
    code: 17,
  });
  expect(owner.section.value?.data.admission?.permit_active).toBe(false);
});

test("D08 recorded terminal outcomes and released permits cannot regress on readback", async () => {
  for (const change of ["outcome", "state", "release"] as const) {
    const owner = new TrafficReadState();
    const native = new NativeFixture();
    const completed = trafficFixture(A, "RUNNING");
    if (!completed.admission)
      throw new Error("Missing native contract fixture");
    completed.admission = {
      ...completed.admission,
      state: "COMPLETED",
      outcome: { kind: "EXITED", code: 17 },
      permit_active: false,
    };
    native.traffic.set(A, trafficReply(completed));
    await owner.read(
      native.client,
      A,
      () => true,
      () => 0,
      () => {},
    );
    const next = clone(completed);
    next.recorded_at_ms = 21;
    if (!next.admission) throw new Error("Missing completed fixture");
    if (change === "outcome")
      next.admission.outcome = { kind: "EXITED", code: 0 };
    if (change === "state") {
      next.admission.state = "RUNNING";
      next.admission.outcome = null;
    }
    if (change === "release") next.admission.permit_active = true;
    native.traffic.set(A, trafficReply(next));
    await owner.read(
      native.client,
      A,
      () => true,
      () => 1,
      () => {},
    );
    expect(owner.section.error?.code).toBe("invalid_reply");
    expect(owner.section.value?.data.admission?.outcome).toEqual({
      kind: "EXITED",
      code: 17,
    });
    expect(owner.section.value?.data.admission?.permit_active).toBe(false);
    expect(owner.section.stale).toBe(true);
  }
});
const selected = (view: ReturnType<typeof mountDesktop>) =>
  within(view.getByRole("region", { name: "Selected job" }));
const traffic = (view: ReturnType<typeof mountDesktop>) =>
  within(view.getByRole("region", { name: m.heading }));
const pendingJob = (job_id = A): JobDetail => ({
  ...clone(golden.exit_zero.data.job),
  job_id,
  phase: "REGISTERED",
  reason: "REGISTERED_NOT_CLAIMED",
  observation: {
    target: "NONE",
    status: "NOT_ATTACHED",
    observed_at_ms: T,
    reason: "REGISTERED_NOT_CLAIMED",
  },
  outcome: null,
  launch_claimed_at_ms: null,
  completed_at_ms: null,
  updated_at_ms: T,
  revision: 1,
});
function nativeWith(result = trafficWait(A)): NativeFixture {
  const native = new NativeFixture();
  native.jobs.set(A, found(pendingJob()));
  native.traffic.set(A, trafficReply(result, T));
  return native;
}
const refresh = async (view: ReturnType<typeof mountDesktop>) => {
  await view.user.click(view.getByRole("button", { name: "Refresh status" }));
  await settle();
};

describe("recorded Bun-script classification and scheduling diagnostics", () => {
  test("D08 an in-flight refresh ages retained facts and recovery clears the stale qualifier", async () => {
    const native = nativeWith();
    const view = mountDesktop(native);
    await settle();
    const pending = deferred<unknown>();
    native.enqueue("traffic_job_diagnostics", () => pending.promise);
    await advance(view.clock, 5_000);
    await advance(view.clock, 5_000);
    expect(traffic(view).getByText(`${m.retained} ${m.stale}`)).toBeDefined();
    expect(traffic(view).getByText("Typecheck")).toBeDefined();
    expect(native.count("traffic_job_diagnostics")).toBe(2);
    const next = trafficWait(A);
    next.recorded_at_ms = 21;
    await act(async () => pending.resolve(trafficReply(next)));
    await settle();
    expect(traffic(view).queryByText(`${m.retained} ${m.stale}`)).toBeNull();
    expect(native.maxInFlight).toBe(1);
  });

  test("D01/D05/D07 labels come from the stored four-class contract, with matching class-window causes", async () => {
    for (const [classification, label, wait] of [
      ["BUILD", "Build", "Waiting for another build"],
      ["TEST", "Test", "Waiting for another test"],
      ["LINT", "Lint", "Waiting for another lint check"],
      ["TYPECHECK", "Typecheck", "Waiting for another typecheck"],
    ] as const) {
      const result = trafficWait(A);
      const stored = wire.classifications.find(
        (c) =>
          c.classification === classification &&
          c.reason === `BUN_SCRIPT_${classification}`,
      );
      if (!stored) throw new Error("Missing Rust classification");
      // This selection is checked again by the production runtime validator.
      result.classification = {
        ...stored,
      } as DiagnosticResult["classification"];
      const native = nativeWith(result);
      const view = mountDesktop(native);
      await settle();
      expect(traffic(view).getByText(label)).toBeDefined();
      expect(traffic(view).getByText(`Last recorded: ${wait}`)).toBeDefined();
      expect(traffic(view).getByText(m.recordedHelp)).toBeDefined();
      expect(traffic(view).queryByText(/Command started/)).toBeNull();
      expect(
        native.calls
          .filter((c) => c.command === "traffic_job_diagnostics")
          .map((c) => c.args),
      ).toEqual([{ job_id: A }]);
      expect(native.maxInFlight).toBe(1);
      cleanup();
    }
    const reserved = mountDesktop(nativeWith(trafficFixture(A, "RESERVED")));
    await settle();
    expect(
      traffic(reserved).getByText(
        "Last recorded: Permit granted · command start unconfirmed",
      ),
    ).toBeDefined();
    expect(traffic(reserved).queryByText("Last recorded: Running")).toBeNull();
  });

  test("D06 shared CPU, shared memory, FIFO and measured pressure retain distinct historical causes", async () => {
    for (const [reason, expected] of [
      ["cpu_budget", "Waiting for shared resources · CPU reservation"],
      ["memory_budget", "Waiting for shared resources · memory reservation"],
      ["fifo", "Waiting for an earlier job"],
      ["memory_pressure", "Waiting for memory"],
    ] as const) {
      const view = mountDesktop(nativeWith(trafficWait(A, reason)));
      await settle();
      expect(
        traffic(view).getByText(`Last recorded: ${expected}`),
      ).toBeDefined();
      expect(
        traffic(view).queryByText(/Waiting for another typecheck/),
      ).toBeNull();
      cleanup();
    }
    const view = mountDesktop(nativeWith(trafficWait(A)));
    await settle();
    expect(traffic(view).getByText(m.details).className).toContain(
      "focus-visible:ring-2",
    );
    expect(traffic(view).getByText(m.estimates)).toBeDefined();
    expect(
      traffic(view).getByText("Recorded Typecheck window").nextElementSibling
        ?.textContent,
    ).toBe("1");
    expect(
      traffic(view).getByText(m.sharedCpu).nextElementSibling?.textContent,
    ).toBe("2000");
    expect(
      traffic(view).getByText(m.sharedMemory).nextElementSibling?.textContent,
    ).toBe("4294967296");
  });

  test("D08/D09 known outcomes and running lifecycle facts supersede historical waiting", async () => {
    for (const [outcome, expected] of [
      [{ kind: "EXITED", code: 0 }, "Succeeded · Exit code 0"],
      [{ kind: "EXITED", code: 17 }, "Failed · Exit code 17"],
      [
        { kind: "SIGNALED", signal: 15, core_dumped: false },
        "Terminated by signal 15",
      ],
      [
        { kind: "SPAWN_FAILED", reason: "NOT_FOUND", os_code: 2 },
        "Spawn failed",
      ],
    ] as const) {
      const result = trafficFixture(A, "COMPLETED");
      if (!result.admission) throw new Error("Missing completed fixture");
      result.admission.outcome = outcome;
      const native = nativeWith(result);
      native.jobs.delete(A); // The negotiated outcome works independently of the old projection.
      const view = mountDesktop(native);
      await settle();
      expect(selected(view).getByText(expected)).toBeDefined();
      expect(
        traffic(view).getByText("Last recorded: Execution outcome recorded"),
      ).toBeDefined();
      expect(traffic(view).queryByText(/Last recorded: Waiting/)).toBeNull();
      cleanup();
    }
    const native = nativeWith();
    const exited = clone(golden.exit_zero.data.job) as JobDetail;
    exited.outcome = { kind: "EXITED", code: 17 };
    native.jobs.set(A, found(exited));
    const view = mountDesktop(native);
    await settle();
    expect(selected(view).getByText("Failed · Exit code 17")).toBeDefined();
    expect(
      traffic(view).getByText("Last recorded: Execution outcome recorded"),
    ).toBeDefined();
    expect(
      traffic(view).queryByText("Last recorded: Waiting for another typecheck"),
    ).toBeNull();
  });

  test("D08 a verified running lifecycle snapshot supersedes an older wait without inventing an exit", async () => {
    const native = nativeWith();
    const running = pendingJob();
    running.phase = "RUNNING";
    running.reason = "CHILD_VERIFIED";
    running.launch_claimed_at_ms = T - 1000;
    running.observation = {
      target: "CHILD",
      status: "VERIFIED_MATCH",
      observed_at_ms: T,
      reason: "CHILD_VERIFIED",
    };
    native.jobs.set(A, found(running));
    const view = mountDesktop(native);
    await settle();
    expect(traffic(view).getByText("Last recorded: Running")).toBeDefined();
    expect(
      traffic(view).queryByText("Last recorded: Waiting for another typecheck"),
    ).toBeNull();
    expect(selected(view).getByText("Outcome unknown")).toBeDefined();
    expect(selected(view).queryByText(/Succeeded/)).toBeNull();
  });

  test("D08/D10 newer abandonment and release supersede retained RUNNING lifecycle text and announcements", async () => {
    const native = nativeWith(trafficFixture(A, "RUNNING"));
    const running = pendingJob();
    running.phase = "RUNNING";
    running.reason = "CHILD_VERIFIED";
    running.launch_claimed_at_ms = T - 1000;
    running.observation = {
      target: "CHILD",
      status: "VERIFIED_MATCH",
      observed_at_ms: T,
      reason: "CHILD_VERIFIED",
    };
    native.jobs.set(A, found(running));
    const view = mountDesktop(native);
    await settle();
    expect(traffic(view).getByText("Last recorded: Running")).toBeDefined();
    const status = selected(view)
      .getAllByRole("status")
      .find((element) => element.classList.contains("sr-only"));
    if (!status) throw new Error("Missing selected-job announcement");
    const initial = status.textContent;
    const abandoned = trafficFixture(A, "RUNNING");
    if (!abandoned.admission) throw new Error("Missing abandonment fixture");
    abandoned.recorded_at_ms = 21;
    abandoned.admission.state = "ABANDONED";
    abandoned.admission.permit_active = false;
    native.traffic.set(A, trafficReply(abandoned));
    native.enqueue("job_by_id", () => {
      throw { code: "unavailable", retryable: true };
    });
    await advance(view.clock, 5000);
    expect(
      traffic(view).getByText(
        "Last recorded: Tracking abandoned · execution outcome unconfirmed",
      ),
    ).toBeDefined();
    expect(traffic(view).getByText(m.permitReleased)).toBeDefined();
    expect(traffic(view).queryByText("Last recorded: Running")).toBeNull();
    expect(
      traffic(view).getByText("Current execution unconfirmed."),
    ).toBeDefined();
    expect(status.textContent).not.toBe(initial);
    expect(status.textContent).toContain(
      "Tracking abandoned · execution outcome unconfirmed",
    );
    expect(status.textContent).toContain("Current execution unconfirmed.");
    expect(status.textContent).not.toContain("Last recorded: Running");
    // A successful lifecycle read cannot revive abandoned scheduling either.
    await advance(view.clock, 5000);
    expect(
      traffic(view).getByText(
        "Last recorded: Tracking abandoned · execution outcome unconfirmed",
      ),
    ).toBeDefined();
    expect(
      traffic(view).queryByText("Current execution unconfirmed."),
    ).toBeNull();
    expect(status.textContent).not.toContain("Last recorded: Running");
    expect(native.count("governor_set_switch")).toBe(0);
  });

  test("D08 retained RUNNING lifecycle status carries its own freshness into visible text and announcements", async () => {
    const native = nativeWith();
    const running = pendingJob();
    running.phase = "RUNNING";
    running.reason = "CHILD_VERIFIED";
    running.launch_claimed_at_ms = T - 1000;
    running.observation = {
      target: "CHILD",
      status: "VERIFIED_MATCH",
      observed_at_ms: T,
      reason: "CHILD_VERIFIED",
    };
    native.jobs.set(A, found(running));
    const view = mountDesktop(native);
    await settle();
    native.enqueue("job_by_id", () => {
      throw { code: "unavailable", retryable: true };
    });
    await advance(view.clock, 5000);
    expect(traffic(view).getByText("Last recorded: Running")).toBeDefined();
    expect(
      traffic(view).getByText("Current execution unconfirmed."),
    ).toBeDefined();
    const status = selected(view)
      .getAllByRole("status")
      .find((element) => element.classList.contains("sr-only"));
    expect(status?.textContent).toContain("Current execution unconfirmed.");
    expect(traffic(view).queryByText(`${m.retained} ${m.stale}`)).toBeNull();
  });

  test("D08 clock rollback preserves automatic polling and unchanged semantic announcements", async () => {
    const initial = trafficWait(A);
    initial.recorded_at_ms = 100;
    const native = nativeWith(initial);
    const view = mountDesktop(native);
    await settle();
    const status = selected(view)
      .getAllByRole("status")
      .find((element) => element.classList.contains("sr-only"));
    const announcement = status?.textContent;
    for (const [recorded, received] of [
      [90, T - 100],
      [110, T + 100],
    ] as const) {
      const next = trafficWait(A);
      next.recorded_at_ms = recorded;
      native.traffic.set(A, trafficReply(next, received));
      await advance(view.clock, 5000);
      expect(
        traffic(view).queryByText(
          "Invalid daemon response. The response was not used.",
        ),
      ).toBeNull();
      expect(traffic(view).queryByText(`${m.retained} ${m.stale}`)).toBeNull();
      expect(status?.textContent).toBe(announcement);
    }
    expect(native.count("traffic_job_diagnostics")).toBe(3);
    expect(native.maxInFlight).toBe(1);
  });

  test("D04 unresolved/mixed bypass and remote-only facts do not become execution failures", async () => {
    for (const key of ["UNKNOWN", "MIXED", "REMOTE_CI"] as const) {
      const result = trafficFixture(A);
      result.admission = null;
      const classification = wire.classifications.find(
        (c) => c.classification === key,
      );
      if (!classification)
        throw new Error("Missing Rust bypass classification");
      result.classification = {
        ...classification,
      } as DiagnosticResult["classification"];
      const native = nativeWith(result);
      native.jobs.set(A, golden.exit_zero);
      const view = mountDesktop(native);
      await settle();
      expect(
        traffic(view).getByText(
          `Last recorded: ${key === "REMOTE_CI" ? m.remote : m.bypass}`,
        ),
      ).toBeDefined();
      expect(selected(view).getByText("Succeeded · Exit code 0")).toBeDefined();
      if (key !== "REMOTE_CI")
        expect(traffic(view).queryByText(/no local reservation/i)).toBeNull();
      cleanup();
    }
  });

  test("D08/D14 unsupported daemon details leave lifecycle, Meter and Governor available and retry only on explicit refresh", async () => {
    const native = new NativeFixture();
    const view = mountDesktop(native);
    await settle();
    expect(traffic(view).getByText(m.unsupported)).toBeDefined();
    expect(traffic(view).queryByText(/no local reservation/i)).toBeNull();
    expect(selected(view).getByText("Succeeded · Exit code 0")).toBeDefined();
    expect(
      view.getByRole("switch", { name: "Agent Resource Control" }),
    ).toBeDefined();
    await advance(view.clock, 5_000);
    expect(native.count("traffic_job_diagnostics")).toBe(1);
    await refresh(view);
    expect(native.count("traffic_job_diagnostics")).toBe(2);
    expect(native.count("governor_set_switch")).toBe(0);
    expect(native.maxInFlight).toBe(1);
  });

  test("D08 stale, failed and missing reads retain recorded classification with unconfirmed admission", async () => {
    const native = nativeWith();
    const view = mountDesktop(native);
    await settle();
    native.enqueue("traffic_job_diagnostics", () => {
      throw { code: "unavailable", retryable: true };
    });
    await refresh(view);
    expect(traffic(view).getByText("Typecheck")).toBeDefined();
    expect(traffic(view).getByText(`${m.retained} ${m.stale}`)).toBeDefined();
    expect(
      traffic(view).getByText("Daemon could not be reached."),
    ).toBeDefined();
    const missing = trafficFixture(A);
    missing.classification = null;
    missing.admission = null;
    native.traffic.set(A, trafficReply(missing));
    await refresh(view);
    expect(traffic(view).getByText(m.missing)).toBeDefined();
    expect(traffic(view).getByText("Typecheck")).toBeDefined();
    native.traffic.set(A, trafficReply(trafficWait(A)));
    await refresh(view);
    expect(traffic(view).queryByText(m.missing)).toBeNull();
    expect(traffic(view).queryByText(`${m.retained} ${m.stale}`)).toBeNull();
  });

  test("D03/D08 changed classifications or wrong job IDs cannot rewrite a selected job's historical facts", async () => {
    for (const change of ["classification", "identity"] as const) {
      const native = nativeWith();
      const view = mountDesktop(native);
      await settle();
      const next = trafficWait(A);
      if (change === "classification")
        next.classification = {
          classification: "BUILD",
          locality: "HOST_LOCAL",
          reason: "BUN_SCRIPT_BUILD",
          uncertainty: "NONE",
        };
      if (change === "identity") next.job_id = B;
      native.traffic.set(A, trafficReply(next));
      await refresh(view);
      expect(
        traffic(view).getByText(
          "Invalid daemon response. The response was not used.",
        ),
      ).toBeDefined();
      expect(traffic(view).getByText("Typecheck")).toBeDefined();
      expect(traffic(view).getByText(`${m.retained} ${m.stale}`)).toBeDefined();
      cleanup();
    }
  });

  test("D08 a retired selection reply cannot populate a different job", async () => {
    const native = nativeWith();
    const pending = deferred<unknown>();
    native.enqueue("traffic_job_diagnostics", () => pending.promise);
    const second = trafficWait(B, "cpu_budget");
    second.classification = {
      classification: "LINT",
      locality: "HOST_LOCAL",
      reason: "BUN_SCRIPT_LINT",
      uncertainty: "NONE",
    };
    native.traffic.set(B, trafficReply(second));
    native.jobs.set(B, found(pendingJob(B)));
    const view = mountDesktop(native);
    await settle();
    const options = view.getAllByRole("option");
    if (!options[1]) throw new Error("Missing second row");
    await view.user.click(options[1]);
    expect(traffic(view).queryByText("Typecheck")).toBeNull();
    await act(async () => pending.resolve(trafficReply(trafficWait(A))));
    await settle();
    expect(traffic(view).getByText("Lint")).toBeDefined();
    expect(traffic(view).queryByText("Typecheck")).toBeNull();
    expect(native.maxInFlight).toBe(1);
  });

  test("D11/D12 keyboard selection, focus, controls and meaningful announcements survive polls", async () => {
    const native = nativeWith();
    native.jobs.set(B, found(pendingJob(B)));
    native.traffic.set(B, trafficReply(trafficWait(B, "cpu_budget")));
    const view = mountDesktop(native);
    await settle();
    const [first, second] = view.getAllByRole("option");
    if (!first || !second) throw new Error("Missing rows");
    await act(async () => first.focus());
    await view.user.keyboard("{End}{Home}{ArrowDown}");
    expect(document.activeElement).toBe(second);
    expect(native.count("traffic_job_diagnostics")).toBe(1);
    await view.user.keyboard(" ");
    await settle();
    expect(document.activeElement).toBe(second);
    expect(second.getAttribute("aria-selected")).toBe("true");
    expect(native.count("traffic_job_diagnostics")).toBe(2);
    await view.user.keyboard("{Enter}");
    await settle();
    expect(native.count("traffic_job_diagnostics")).toBe(2);
    const status = selected(view)
      .getAllByRole("status")
      .find((element) => element.classList.contains("sr-only"));
    if (!status) throw new Error("Missing selected-job announcement");
    expect(status.getAttribute("aria-live")).toBe("polite");
    expect(status.textContent).toContain("Waiting for shared resources");
    const initial = status.textContent;
    const next = trafficWait(B, "cpu_budget");
    next.recorded_at_ms = 21;
    native.traffic.set(B, trafficReply(next, T + 100));
    await advance(view.clock, 5_000);
    expect(status.textContent).toBe(initial);
    expect(document.activeElement).toBe(second);
    // Preserve the increasing snapshot clock for the changed semantic value.
    const changed = trafficWait(B, "memory_budget");
    changed.recorded_at_ms = 22;
    native.traffic.set(B, trafficReply(changed));
    await advance(view.clock, 5_000);
    expect(status.textContent).toContain(
      "Waiting for shared resources · memory reservation",
    );
    expect(status.textContent).not.toBe(initial);
    expect(document.activeElement).toBe(second);
    expect(
      view.getByText(
        /Refresh checks tracking; it does not run the command again/,
      ),
    ).toBeDefined();
    expect(native.count("governor_set_switch")).toBe(0);
  });

  test("D09/D11/D13 confirmed pause suppresses historical wait; complete permits and cancellation remain distinct", async () => {
    const native = nativeWith();
    native.governor = {
      data: {
        switch: {
          key: "governor.enabled",
          enabled: false,
          revision: 1,
          updated_at_ms: 20,
        },
      },
      received_at_ms: T,
    };
    const view = mountDesktop(native);
    await settle();
    expect(
      traffic(view).getByText(/Last recorded: Scheduling paused/),
    ).toBeDefined();
    expect(
      traffic(view).queryByText("Last recorded: Waiting for another typecheck"),
    ).toBeNull();
    cleanup();
    const cancelled = mountDesktop(nativeWith(trafficFixture(A, "CANCELLED")));
    await settle();
    expect(
      traffic(cancelled).getByText("Last recorded: Cancelled · not started"),
    ).toBeDefined();
    cleanup();
    for (const permit_active of [true, false]) {
      const result = trafficFixture(A, "RUNNING");
      if (!result.admission) throw new Error("Missing running fixture");
      result.admission = {
        ...result.admission,
        state: "COMPLETED",
        outcome: { kind: "EXITED", code: 17 },
        permit_active,
      };
      const completed = mountDesktop(nativeWith(result));
      await settle();
      expect(
        traffic(completed).getByText(
          permit_active ? m.permitActive : m.permitReleased,
        ),
      ).toBeDefined();
      expect(
        traffic(completed).getByText(m.queueWait).nextElementSibling
          ?.textContent,
      ).toBe("2");
      expect(
        traffic(completed).getByText(m.cpu).nextElementSibling?.textContent,
      ).toBe("1000");
      expect(
        selected(completed).getByText("Failed · Exit code 17"),
      ).toBeDefined();
      cleanup();
    }
  });
});
