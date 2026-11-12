import { expect, test } from "bun:test";
import type { JobDetail } from "@/bridge/types";
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
  page,
  settle,
  within,
} from "./desktop-test-harness";
import { memoryJob, memoryPolicy, policyReply } from "./memory-policy-fixture";

function policyJobRow(view: ReturnType<typeof mountDesktop>, index: number) {
  const row = view.getAllByRole("option")[index];
  if (!row) throw new Error("Expected synthetic job row");
  return row;
}

function queuedFixture() {
  const native = new NativeFixture(),
    job = clone(golden.exit_zero.data.job) as JobDetail;
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
  return native;
}
export function registerPolicyInteractionCases() {
  test("open policy disclosure qualifies retained readings at 3000/3001 ms and disconnect without changing focus or history", async () => {
    const native = queuedFixture(),
      v = mountDesktop(native);
    await settle();
    const panel = within(v.getByRole("region", { name: "Memory admission" }));
    const summary = panel.getByText("Policy and observation details");
    await v.user.click(summary);
    summary.focus();
    expect(document.activeElement).toBe(summary);
    await advance(v.clock, 3000);
    expect(panel.getByText("Effective mode: memory_hold")).toBeDefined();
    await advance(v.clock, 1);
    expect(panel.getByText("Last received mode: memory_hold")).toBeDefined();
    expect(
      panel.getByText(/Last received reason: memory_critical.*unconfirmed/),
    ).toBeDefined();
    expect(panel.getByText("Retained observation availability")).toBeDefined();
    expect(panel.getByText("Retained pressure")).toBeDefined();
    expect(panel.queryByText(/Fresh memory reading: Critical/)).toBeNull();
    expect(panel.queryByText(/Starts automatically/)).toBeNull();
    expect(document.activeElement).toBe(summary);
    expect(summary.parentElement?.hasAttribute("open")).toBe(true);
    native.healthFailure = { code: "unavailable", retryable: true };
    await v.user.click(v.getByRole("button", { name: "Refresh status" }));
    await settle();
    expect(panel.getByText("Last received mode: memory_hold")).toBeDefined();
    summary.focus();
    expect(document.activeElement).toBe(summary);
    native.healthFailure = null;
    native.enqueue("policy_job", () =>
      Promise.reject({ code: "unsupported", retryable: false }),
    );
    await v.user.click(v.getByRole("button", { name: "Retry connection" }));
    await settle();
    expect(panel.getByText("Last received mode: memory_hold")).toBeDefined();
    expect(panel.queryByText(/Fresh memory reading: Critical/)).toBeNull();
    summary.focus();
    expect(document.activeElement).toBe(summary);
    expect(policyJobRow(v, 0).getAttribute("aria-selected")).toBe("true");
    expect(native.policies.get(A)).toBeDefined();
  });

  test("an initial policy failure is unknown; retained policy is reported only after an accepted read", async () => {
    const native = new NativeFixture();
    native.policies.set(A, policyReply(memoryJob(A, memoryPolicy("normal"))));
    const unavailable = () =>
      Promise.reject({ code: "unavailable", retryable: true });
    native.enqueue("policy_job", unavailable);
    const view = mountDesktop(native);
    await settle();
    const policy = within(
      view.getByRole("region", { name: "Memory admission" }),
    );
    expect(policy.getByText(/Memory admission is unknown/)).toBeDefined();
    expect(policy.queryByText(/Last known policy retained/)).toBeNull();
    expect(view.getAllByText("Succeeded · Exit code 0").length).toBeGreaterThan(
      0,
    );
    await view.user.click(view.getByRole("button", { name: "Refresh status" }));
    await settle();
    expect(policy.getByText(/Initial policy active/)).toBeDefined();
    native.enqueue("policy_job", unavailable);
    await view.user.click(view.getByRole("button", { name: "Refresh status" }));
    await settle();
    expect(policy.getByText(/Last known policy retained/)).toBeDefined();
    expect(policy.queryByText(/Memory admission is unknown/)).toBeNull();
    expect(view.getAllByText("Succeeded · Exit code 0").length).toBeGreaterThan(
      0,
    );
    expect(native.maxInFlight).toBe(1);
  });
  test("authoritative pressure waits recover automatically without resubmit and preserve keyboard focus/outcome", async () => {
    const native = queuedFixture(),
      v = mountDesktop(native);
    await settle();
    const panel = within(v.getByRole("region", { name: "Memory admission" }));
    expect(panel.getByText("Waiting for memory")).toBeDefined();
    expect(
      panel.getByText(
        "Starts automatically when memory permits; other queue rules still apply.",
      ),
    ).toBeDefined();
    const row = policyJobRow(v, 0);
    await v.user.click(row);
    expect(document.activeElement).toBe(row);
    const details = panel.getByText("Policy and observation details");
    await v.user.click(details);
    details.focus();
    expect(details.parentElement?.hasAttribute("open")).toBe(true);
    const hold = memoryJob(A),
      recovered = memoryJob(A, memoryPolicy("normal", 2));
    native.policies.set(A, policyReply(recovered));
    await advance(v.clock, 5000);
    expect(panel.queryByText("Waiting for memory")).toBeNull();
    expect(
      panel.getByText(/Last recorded decision: waiting for memory/),
    ).toBeDefined();
    expect(panel.getByText(/Initial policy active/)).toBeDefined();
    expect(details.parentElement?.hasAttribute("open")).toBe(true);
    expect(document.activeElement).toBe(details);
    expect(policyJobRow(v, 0).getAttribute("aria-selected")).toBe("true");
    expect(hold.last_decision?.observation.age_ms).toBe(0);
    const finished = clone(golden.exit_zero);
    finished.data.job.revision = 4;
    native.jobs.set(A, finished);
    native.policies.set(A, policyReply(recovered));
    await advance(v.clock, 5000);
    expect(v.getAllByText("Succeeded · Exit code 0").length).toBeGreaterThan(0);
    expect(native.maxInFlight).toBe(1);
  });
  test("unchanged policy polls change no live announcement and unrelated Meter cannot refresh pressure", async () => {
    const native = queuedFixture(),
      v = mountDesktop(native);
    await settle();
    const selected = within(v.getByRole("region", { name: "Selected job" })),
      status = selected.getByRole("status");
    const text = status.textContent,
      mutations: MutationRecord[] = [];
    const observer = new MutationObserver((records) =>
      mutations.push(...records),
    );
    observer.observe(status, {
      subtree: true,
      childList: true,
      characterData: true,
    });
    await v.user.click(v.getByRole("button", { name: "Refresh status" }));
    await settle();
    expect(status.textContent).toBe(text);
    expect(mutations).toHaveLength(0);
    await advance(v.clock, 3000);
    expect(status.textContent).toBe(text);
    await advance(v.clock, 1);
    expect(status.textContent).toContain("Stale policy");
    // The same measurement with a new receipt remains stale after a refresh.
    await v.user.click(v.getByRole("button", { name: "Refresh status" }));
    await settle();
    expect(status.textContent).toContain("Stale policy");
    expect(native.count("meter_snapshot")).toBeGreaterThan(1);
    observer.disconnect();
  });
  test("stale Critical selects explicit L0 and disconnected/unsupported retain independent recorded outcome", async () => {
    const native = new NativeFixture(),
      stale = memoryPolicy();
    stale.mode = "l0";
    stale.reason = "observation_stale";
    stale.observation.availability = "stale";
    stale.observation.age_ms = 3001;
    native.policies.set(A, policyReply(memoryJob(A, stale)));
    const v = mountDesktop(native);
    await settle();
    const selected = within(v.getByRole("region", { name: "Selected job" }));
    expect(selected.getByText("Succeeded · Exit code 0")).toBeDefined();
    const policy = within(v.getByRole("region", { name: "Memory admission" }));
    expect(policy.getByText(/Initial policy active/)).toBeDefined();
    expect(policy.queryByText("Waiting for memory")).toBeNull();
    expect(policy.getAllByText(/Memory reading stale/).length).toBeGreaterThan(
      0,
    );
    native.healthFailure = { code: "unavailable", retryable: true };
    await v.user.click(v.getByRole("button", { name: "Refresh status" }));
    await settle();
    expect(policy.getByText(/Last known policy retained/)).toBeDefined();
    expect(selected.getByText("Succeeded · Exit code 0")).toBeDefined();
    native.healthFailure = null;
    native.enqueue("policy_job", () =>
      Promise.reject({ code: "unsupported", retryable: false }),
    );
    await v.user.click(v.getByRole("button", { name: "Retry connection" }));
    await settle();
    expect(policy.getByText(/not supported by this daemon/)).toBeDefined();
    expect(selected.getByText("Succeeded · Exit code 0")).toBeDefined();
  });
}

export function registerPolicyLifecycleCases() {
  test("retired policy replies cannot replace the selected job or start another in-flight read", async () => {
    const native = new NativeFixture(),
      pending = deferred<unknown>();
    native.policies.set(
      B,
      policyReply(memoryJob(B, memoryPolicy("normal", 2))),
    );
    native.enqueue("policy_job", () => pending.promise);
    const v = mountDesktop(native);
    await settle();
    const before = native.calls.length;
    await act(async () => {
      v.visibility.hide();
      v.visibility.show();
    });
    await v.user.click(policyJobRow(v, 1));
    expect(native.calls).toHaveLength(before);
    await act(async () => {
      pending.resolve(policyReply(memoryJob(A)));
    });
    await settle();
    const region = within(v.getByRole("region", { name: "Memory admission" }));
    expect(region.queryByText("Waiting for memory")).toBeNull();
    expect(policyJobRow(v, 1).getAttribute("aria-selected")).toBe("true");
    expect(native.maxInFlight).toBe(1);
  });
  test("no-selection current policy still uses the bounded argument-free bridge", async () => {
    const native = new NativeFixture();
    native.recent = page([]);
    native.policy = policyReply(memoryPolicy("normal"));
    const v = mountDesktop(native);
    await settle();
    expect(
      within(v.getByRole("region", { name: "Memory admission" })).getByText(
        /Initial policy active/,
      ),
    ).toBeDefined();
    expect(native.count("policy_current")).toBe(1);
    expect(native.count("policy_job")).toBe(0);
  });
}
