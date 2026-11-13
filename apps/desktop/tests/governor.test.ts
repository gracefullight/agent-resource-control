import { afterEach, describe, expect, test } from "bun:test";
import type {
  GovernorMutationResult,
  GovernorSwitchState,
  SetGovernorSwitchRequest,
} from "@/bridge/governor.generated";
import type { JobDetail } from "@/bridge/types";
import { governorMessages as m } from "@/dashboard/governor-messages";
import { GovernorSwitchStateOwner } from "@/dashboard/governor-switch-state";
import {
  A,
  act,
  advance,
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
import { memoryJob, policyReply } from "./fixtures/memory-policy-fixture";
import { meterFixture, meterReply } from "./fixtures/meter-fixture";

// Deterministic DOM/socket DTO fixtures. These are not production-daemon child proof.
afterEach(cleanup);
const state = (enabled = true, revision = 4): GovernorSwitchState => ({
  key: "governor.enabled",
  enabled,
  revision,
  updated_at_ms: T,
});
const reply = (s = state(), result?: GovernorMutationResult) => ({
  data: { switch: s, ...(result ? { mutation_result: result } : {}) },
  received_at_ms: T,
});
const committed = (
  request: SetGovernorSwitchRequest,
  enabled = request.enabled,
  revision = request.expected_revision + 1,
): GovernorMutationResult => ({
  status: "COMMITTED",
  mutation_id: request.mutation_id,
  switch: state(enabled, revision),
  changed: true,
  replayed: false,
});
const region = (view: ReturnType<typeof mountDesktop>) =>
  within(view.getByRole("region", { name: m.label }));
const toggle = (view: ReturnType<typeof mountDesktop>) =>
  region(view).getByRole("switch", { name: m.label });
function nativeOn() {
  const native = new NativeFixture();
  native.governor = reply();
  return native;
}
function intent(native: NativeFixture): SetGovernorSwitchRequest {
  const args = [...native.calls]
    .reverse()
    .find((call) => call.command === "governor_set_switch")?.args;
  if (!args || !("request" in args))
    throw new Error("Expected typed Governor intent");
  return args.request;
}

describe("Governor master switch", () => {
  test("G01 initial loading has no unchecked actionable switch; saved OFF is authoritative", async () => {
    const native = nativeOn();
    const pending = deferred<unknown>();
    native.enqueue("governor_get_switch", () => pending.promise);
    const view = mountDesktop(native);
    expect(region(view).getByText(m.checking)).toBeDefined();
    expect(region(view).queryByRole("switch")).toBeNull();
    expect(region(view).queryByText(m.active)).toBeNull();
    await act(async () => pending.resolve(reply(state(false, 7))));
    await settle();
    expect(toggle(view).getAttribute("aria-checked")).toBe("false");
    expect(region(view).getByText(m.paused)).toBeDefined();
    expect(native.count("governor_set_switch")).toBe(0);
  });

  test("G02/G03/G04 keyboard, focus, one applying mutation and commit confirmation", async () => {
    const native = nativeOn();
    const write = deferred<unknown>();
    native.enqueue("governor_set_switch", () => write.promise);
    const view = mountDesktop(native);
    await settle();
    const control = toggle(view);
    control.focus();
    expect(control.className).toContain("focus-visible:ring-2");
    expect(control.getAttribute("aria-describedby")).toBeTruthy();
    await view.user.keyboard(" ");
    await settle();
    expect(region(view).getByText(m.applyingPause)).toBeDefined();
    expect(control.getAttribute("aria-checked")).toBe("true");
    expect(control.getAttribute("aria-disabled")).toBe("true");
    expect(document.activeElement).toBe(control);
    await view.user.keyboard("{Enter} ");
    await settle();
    expect(native.count("governor_set_switch")).toBe(1);
    const request = intent(native);
    expect(request).toMatchObject({
      key: "governor.enabled",
      enabled: false,
      expected_revision: 4,
    });
    expect(request.mutation_id).toMatch(/^4-[0-9a-f]{32}$/);
    native.governor = reply(state(false, 5));
    await act(async () =>
      write.resolve({ data: committed(request), received_at_ms: T }),
    );
    await settle();
    expect(control.getAttribute("aria-checked")).toBe("false");
    expect(control.getAttribute("aria-disabled")).toBe("false");
    expect(document.activeElement).toBe(control);
    expect(region(view).getByText(m.paused)).toBeDefined();
    expect(region(view).getByText(/without resubmitting/)).toBeDefined();
    expect(native.maxInFlight).toBe(1);
    expect(region(view).getAllByRole("status")).toHaveLength(1);
  });

  test("G05 confirmed transport and typed rejection retain prior confirmed setting", async () => {
    for (const typed of [false, true]) {
      const native = nativeOn();
      native.enqueue("governor_set_switch", () => {
        if (!typed) throw { code: "rejected", retryable: false };
        return {
          data: {
            status: "REJECTED",
            mutation_id: intent(native).mutation_id,
            switch: state(),
            reason: "RECEIPT_CAPACITY",
          },
          received_at_ms: T,
        };
      });
      const view = mountDesktop(native);
      await settle();
      await view.user.click(toggle(view));
      await settle();
      expect(toggle(view).getAttribute("aria-checked")).toBe("true");
      expect(region(view).getByText(m.pauseRejected)).toBeDefined();
      expect(region(view).queryByText(m.uncertain)).toBeNull();
      expect(toggle(view).getAttribute("aria-disabled")).toBe("false");
      cleanup();
    }
  });

  test("G06/G07/G12 DB_BUSY is uncertain; missing receipt at base cannot settle; reconnect reads identity", async () => {
    const native = nativeOn();
    native.enqueue("governor_set_switch", () => {
      throw { code: "busy", retryable: true };
    });
    const view = mountDesktop(native);
    await settle();
    await view.user.click(toggle(view));
    await settle();
    const request = intent(native);
    expect(region(view).getByText(m.uncertain)).toBeDefined();
    expect(toggle(view).getAttribute("aria-checked")).toBe("true");
    expect(toggle(view).getAttribute("aria-disabled")).toBe("true");
    expect(
      [...native.calls]
        .reverse()
        .find((call) => call.command === "governor_get_switch")?.args,
    ).toEqual({
      query: { key: "governor.enabled", mutation_id: request.mutation_id },
    });
    native.enqueue("governor_get_switch", () => {
      throw { code: "unavailable", retryable: true };
    });
    await advance(view.clock, 5_000);
    expect(region(view).getByText(m.uncertain)).toBeDefined();
    const receipt = {
      ...committed(request),
      replayed: true,
    } as GovernorMutationResult;
    native.governor = reply(state(false, 5), receipt);
    await advance(view.clock, 5_000);
    expect(region(view).queryByText(m.uncertain)).toBeNull();
    expect(toggle(view).getAttribute("aria-checked")).toBe("false");
    expect(native.count("governor_set_switch")).toBe(1);
  });

  test("G08 cross-client conflict displays current evidence without automatic rebase", async () => {
    const native = nativeOn();
    native.enqueue("governor_set_switch", () => ({
      data: {
        status: "CONFLICT",
        mutation_id: intent(native).mutation_id,
        switch: state(false, 5),
        reason: "REVISION_CONFLICT",
      },
      received_at_ms: T,
    }));
    const view = mountDesktop(native);
    await settle();
    native.governor = reply(state(false, 5));
    await view.user.click(toggle(view));
    await settle();
    expect(region(view).getByText(m.conflict)).toBeDefined();
    expect(toggle(view).getAttribute("aria-checked")).toBe("false");
    expect(native.count("governor_set_switch")).toBe(1);
  });

  test("G09 older revisions cannot regress state; equal conflicting revisions become unavailable", async () => {
    const native = nativeOn();
    const view = mountDesktop(native);
    await settle();
    native.governor = reply(state(false, 5));
    await advance(view.clock, 5_000);
    native.governor = reply(state(true, 6));
    await advance(view.clock, 5_000);
    native.governor = reply(state(false, 5));
    await advance(view.clock, 5_000);
    expect(toggle(view).getAttribute("aria-checked")).toBe("true");
    native.governor = reply(state(false, 6));
    await advance(view.clock, 5_000);
    expect(region(view).getByText(m.unavailable)).toBeDefined();
    expect(toggle(view).getAttribute("aria-checked")).toBe("true");
    expect(toggle(view).getAttribute("aria-disabled")).toBe("true");
    native.governor = reply(state(false, 7));
    await view.user.click(view.getByRole("button", { name: "Refresh status" }));
    await settle();
    expect(toggle(view).getAttribute("aria-checked")).toBe("false");
  });

  test("G10 Governor reads remain independent of daemon health and refresh activity", async () => {
    const native = nativeOn();
    native.governor = reply(state(false, 5));
    native.healthFailure = { code: "unavailable", retryable: true };
    const view = mountDesktop(native);
    await settle();
    expect(region(view).getByText(m.paused)).toBeDefined();
    expect(view.getByText("Disconnected")).toBeDefined();
    await view.user.click(view.getByRole("button", { name: "Hide Dashboard" }));
    await settle();
    expect(region(view).getByText(m.unavailable)).toBeDefined();
    expect(toggle(view).getAttribute("aria-checked")).toBe("false");
    expect(native.count("governor_set_switch")).toBe(0);
  });

  test("G11/G13 pending mutation survives About, hide, cancelled refresh and reopen", async () => {
    const native = nativeOn();
    const pending = deferred<unknown>();
    native.enqueue("governor_set_switch", () => pending.promise);
    const view = mountDesktop(native);
    await settle();
    await view.user.click(toggle(view));
    await settle();
    const request = intent(native);
    await view.user.click(
      view.getByRole("button", { name: "About / Credits" }),
    );
    await view.user.click(view.getByRole("button", { name: "Hide Dashboard" }));
    await settle();
    native.governor = reply(state(false, 5));
    await act(async () =>
      pending.resolve({ data: committed(request), received_at_ms: T }),
    );
    await settle();
    await view.user.click(view.getByRole("button", { name: "Dashboard" }));
    await settle();
    expect(toggle(view).getAttribute("aria-checked")).toBe("false");
    expect(region(view).queryByText(m.applyingPause)).toBeNull();
    expect(native.count("governor_set_switch")).toBe(1);
    expect(native.maxInFlight).toBe(1);
  });

  test("malformed post-dispatch reply remains uncertain; current evidence can supersede an evicted receipt", async () => {
    const native = nativeOn();
    native.enqueue("governor_set_switch", {
      data: { status: "COMMITTED" },
      received_at_ms: T,
    });
    const view = mountDesktop(native);
    await settle();
    await view.user.click(toggle(view));
    await settle();
    expect(region(view).getByText(m.uncertain)).toBeDefined();
    native.governor = reply(state(true, 6));
    await advance(view.clock, 5_000);
    expect(region(view).getByText(m.unknownOutcome)).toBeDefined();
    expect(toggle(view).getAttribute("aria-disabled")).toBe("false");
    expect(native.count("governor_set_switch")).toBe(1);
  });

  test("paused policy explanation keeps pressure readings and records historical holds", async () => {
    const native = nativeOn();
    native.governor = reply(state(false, 5));
    const job = clone(golden.exit_zero.data.job) as JobDetail;
    Object.assign(job, {
      phase: "REGISTERED",
      reason: "REGISTERED_NOT_CLAIMED",
      outcome: null,
      launch_claimed_at_ms: null,
      completed_at_ms: null,
    });
    Object.assign(job.observation, {
      target: "OBSERVER",
      status: "VERIFIED_MATCH",
      reason: "REGISTERED_NOT_CLAIMED",
    });
    native.jobs.set(A, found(job));
    native.policies.set(A, policyReply(memoryJob(A)));
    const meter = meterFixture();
    meter.host.memory_pressure.reading = {
      availability: "available",
      value: "critical",
    };
    native.meter = meterReply(meter);
    const view = mountDesktop(native);
    await settle();
    expect(
      region(view).getByText(/Running jobs and usage measurement continue/),
    ).toBeDefined();
    expect(
      view.getByRole("region", { name: "Resource availability" }),
    ).toBeDefined();
    expect(
      view.queryByText(
        "Starts automatically when memory permits; other queue rules still apply.",
      ),
    ).toBeNull();
    const policy = within(
      view.getByRole("region", { name: "Memory admission" }),
    );
    expect(policy.queryByText("Waiting for memory")).toBeNull();
    expect(
      policy.getByText(/Last recorded decision: waiting for memory/),
    ).toBeDefined();
    const metrics = within(
      view.getByRole("region", { name: "Resource availability" }),
    );
    expect(metrics.getByRole("status").textContent).toContain(
      "Memory pressure: Critical",
    );
    expect(metrics.getByText("Critical")).toBeDefined();
  });
});

test("a receipt for a different intent never settles an uncertain write", async () => {
  const native = nativeOn();
  const owner = new GovernorSwitchStateOwner(
    () => {},
    () => 0,
    () => "a".repeat(32),
  );
  await owner.read(native.client);
  owner.request(false);
  native.enqueue("governor_set_switch", () => {
    throw { code: "uncertain", retryable: true };
  });
  native.governor = reply(state(true, 4));
  await owner.work(native.client);
  const request = owner.section.pending;
  expect(request).not.toBeNull();
  if (!request) throw new Error("Missing intent");
  native.governor = reply(state(true, 4), {
    status: "COMMITTED",
    mutation_id: request.mutation_id,
    switch: state(true, 4),
    changed: false,
    replayed: true,
  });
  await owner.read(native.client);
  expect(owner.section.pending).toBe(request);
  expect(owner.section.error?.code).toBe("invalid_reply");
});

test("saved OFF resumes with Enter and stays Paused until a committed acknowledgment", async () => {
  const native = nativeOn();
  native.governor = reply(state(false, 5));
  const write = deferred<unknown>();
  native.enqueue("governor_set_switch", () => write.promise);
  const view = mountDesktop(native);
  await settle();
  const control = toggle(view);
  control.focus();
  await view.user.keyboard("{Enter}");
  await settle();
  expect(region(view).getByText(m.applyingResume)).toBeDefined();
  expect(control.getAttribute("aria-checked")).toBe("false");
  const request = intent(native);
  expect(request.enabled).toBe(true);
  native.governor = reply(state(true, 6));
  await act(async () =>
    write.resolve({ data: committed(request), received_at_ms: T }),
  );
  await settle();
  expect(control.getAttribute("aria-checked")).toBe("true");
  expect(document.activeElement).toBe(control);
});

test("initial unavailable or unsupported Governor never guesses ON from healthy daemon status", async () => {
  for (const code of ["unavailable", "unsupported"]) {
    const native = nativeOn();
    native.enqueue("governor_get_switch", () => {
      throw { code, retryable: code === "unavailable" };
    });
    const view = mountDesktop(native);
    await settle();
    expect(region(view).getByText(m.unavailable)).toBeDefined();
    expect(region(view).queryByRole("switch")).toBeNull();
    expect(view.getByText("Daemon connected")).toBeDefined();
    expect(native.count("governor_set_switch")).toBe(0);
    cleanup();
  }
});

test("replayed mutation receipts cannot be presented as the current switch before readback", async () => {
  const native = nativeOn();
  const current = deferred<unknown>();
  native.enqueue("governor_set_switch", () => ({
    data: { ...committed(intent(native)), replayed: true },
    received_at_ms: T,
  }));
  const view = mountDesktop(native);
  await settle();
  native.enqueue("governor_get_switch", () => current.promise);
  await view.user.click(toggle(view));
  await settle();
  expect(region(view).getByText(m.unavailable)).toBeDefined();
  expect(toggle(view).getAttribute("aria-checked")).toBe("true");
  expect(region(view).queryByText(m.paused)).toBeNull();
  await act(async () => current.resolve(reply(state(true, 6))));
  await settle();
  expect(region(view).getByText(m.active)).toBeDefined();
  expect(toggle(view).getAttribute("aria-disabled")).toBe("false");
  expect(native.count("governor_set_switch")).toBe(1);
});
