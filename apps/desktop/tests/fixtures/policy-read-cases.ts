import { expect, test } from "bun:test";
import { createDesktopClient } from "@/bridge/client";
import { policyAnnouncement } from "@/dashboard/job-policy-panel";
import { PolicyReadState } from "@/dashboard/policy-read-state";
import {
  fixtureSampler,
  memoryJob,
  memoryPolicy,
  policyReply,
} from "./memory-policy-fixture";

const A = "a".repeat(32);
export function registerPolicyReadCases() {
  test("delayed first current and job reads age Critical before displaying a hold", async () => {
    for (const jobId of [null, A]) {
      const state = new PolicyReadState(),
        effective = memoryPolicy();
      effective.observation.age_ms = 2500;
      const data = memoryJob(A, effective);
      let now = 100;
      await state.read(
        createDesktopClient(async () => {
          now += 1000;
          return policyReply(jobId ? data : effective);
        }),
        jobId,
        () => true,
        () => now,
        () => {},
      );
      expect(state.section.error).toBeNull();
      expect(state.section.ageMs).toBe(3500);
      expect(state.section.stale).toBe(true);
      expect(policyAnnouncement(state.section)).toContain("Stale policy");
      expect(policyAnnouncement(state.section)).not.toBe("Waiting for memory");
      expect(state.section.value?.data.observation.age_ms).toBe(2500);
      if (jobId)
        expect(state.section.job?.data.last_decision?.observation.age_ms).toBe(
          0,
        );
    }
  });
  test("read delay also ages a changed pressure observation, while fresh recovery and recorded ages remain independent", async () => {
    const state = new PolicyReadState();
    let now = 0,
      delay = 0,
      data = memoryJob(A, memoryPolicy("normal"));
    const read = () =>
      state.read(
        createDesktopClient(async () => {
          now += delay;
          return policyReply(structuredClone(data));
        }),
        A,
        () => true,
        () => now,
        () => {},
      );
    await read();
    now = 100;
    delay = 1000;
    data = memoryJob(A, memoryPolicy("critical", 2));
    data.effective.observation.age_ms = 2500;
    await read();
    expect(state.section.ageMs).toBe(3500);
    expect(state.section.stale).toBe(true);
    expect(policyAnnouncement(state.section)).toContain("Stale policy");
    expect(state.section.job?.data.last_decision?.observation.age_ms).toBe(0);
    delay = 500;
    data = memoryJob(A, memoryPolicy("normal", 3));
    await read();
    expect(state.section.ageMs).toBe(500);
    expect(state.section.stale).toBe(false);
    expect(policyAnnouncement(state.section)).toContain(
      "Fresh memory reading: Normal",
    );
  });
  test("read-time clock regression or invalid clocks cannot confirm a first pressure hold", async () => {
    for (const finish of [999, Number.NaN, Number.POSITIVE_INFINITY]) {
      const state = new PolicyReadState();
      let now = 1000;
      await state.read(
        createDesktopClient(async () => {
          now = finish;
          return policyReply(memoryJob(A));
        }),
        A,
        () => true,
        () => now,
        () => {},
      );
      expect(state.section.stale).toBe(true);
      expect(policyAnnouncement(state.section)).toContain("Stale policy");
    }
  });
  test("pressure freshness is strict, duplicate receipts cannot renew it, and recorded decision ages stay fixed", async () => {
    const state = new PolicyReadState();
    let now = 0,
      reply = policyReply(memoryJob(A));
    const client = createDesktopClient(async () => structuredClone(reply));
    const read = () =>
      state.read(
        client,
        A,
        () => true,
        () => now,
        () => {},
      );
    await read();
    expect(state.expire(3000)).toBe(1);
    expect(state.section.stale).toBe(false);
    now = 3001;
    state.expire(now);
    expect(state.section.stale).toBe(true);
    reply.received_at_ms += 100_000;
    await read();
    expect(state.section.ageMs).toBe(3001);
    expect(state.section.stale).toBe(true);
    expect(state.section.job?.data.last_decision?.observation.age_ms).toBe(0);
    const fallback = memoryPolicy();
    fallback.mode = "l0";
    fallback.reason = "observation_stale";
    fallback.observation.age_ms = 3002;
    fallback.observation.availability = "stale";
    reply = policyReply(memoryJob(A, fallback));
    await read();
    expect(state.section.stale).toBe(false);
    expect(policyAnnouncement(state.section)).toContain(
      "Initial policy active",
    );
    expect(policyAnnouncement(state.section)).toContain("Memory reading stale");
    expect(state.section.job?.data.last_decision?.observation.age_ms).toBe(0);
  });
  test("new host sequence and failed reads do not rejuvenate an unchanged pressure measurement", async () => {
    const state = new PolicyReadState();
    let now = 0,
      data = memoryJob(A);
    const client = createDesktopClient(async () =>
      policyReply(structuredClone(data)),
    );
    const read = () =>
      state.read(
        client,
        A,
        () => true,
        () => now,
        () => {},
      );
    await read();
    now = 4000;
    data.effective.observation.sequence = 2;
    await read();
    expect(state.section.stale).toBe(true);
    expect(state.section.ageMs).toBe(4000);
    data.effective = {
      ...data.effective,
      mode: "l0",
      reason: "observation_failed",
      observation: {
        availability: "failed",
        pressure: null,
        age_ms: null,
        stale_after_ms: null,
        sampler: null,
        sequence: null,
        observed_at_ms: null,
        observed_monotonic_ms: null,
      },
    };
    await read();
    expect(state.section.stale).toBe(false);
    now = 4500;
    data = memoryJob(A);
    data.effective.observation.sequence = 3;
    await read();
    expect(state.section.stale).toBe(true);
    expect(state.section.ageMs).toBe(4500);
  });
  test("sampler restart opens a new observation window but retired generations and changed duplicate publications reject", async () => {
    const state = new PolicyReadState();
    let data = memoryJob(A);
    const client = createDesktopClient(async () =>
      policyReply(structuredClone(data)),
    );
    const read = () =>
      state.read(
        client,
        A,
        () => true,
        () => 0,
        () => {},
      );
    await read();
    data = memoryJob(A, memoryPolicy("normal"));
    fixtureSampler(data.effective).generation = 2;
    await read();
    expect(state.section.error).toBeNull();
    expect(state.section.value?.data.observation.sampler?.generation).toBe(2);
    fixtureSampler(data.effective).generation = 1;
    await read();
    expect(state.section.error?.code).toBe("invalid_reply");
    state.reprobe();
    fixtureSampler(data.effective).generation = 2;
    data.effective = {
      ...data.effective,
      mode: "memory_hold",
      reason: "memory_critical",
      observation: { ...data.effective.observation, pressure: "critical" },
    };
    await read();
    expect(state.section.error?.code).toBe("invalid_reply");
    expect(state.section.value?.data.observation.pressure).toBe("normal");
  });
  test("clock regression and retirement keep retained policy stale, independent of wall receipt time", async () => {
    const state = new PolicyReadState();
    await state.read(
      createDesktopClient(async () => policyReply(memoryJob(A))),
      A,
      () => true,
      () => 1000,
      () => {},
    );
    state.expire(999);
    expect(state.section.stale).toBe(true);
    state.retire({ code: "unavailable", retryable: true });
    expect(policyAnnouncement(state.section)).toContain(
      "Current admission is unconfirmed",
    );
    state.select();
    expect(state.section.job).toBeNull();
  });
  test("failed, missing, denied and unsupported readings report real L0 fallback without inventing Normal", async () => {
    for (const availability of [
      "missing",
      "failed",
      "denied",
      "unsupported",
      "invalid",
    ] as const) {
      const state = new PolicyReadState(),
        p = memoryPolicy();
      p.mode = "l0";
      p.reason = `observation_${availability}`;
      p.observation = {
        availability,
        pressure: null,
        age_ms: null,
        stale_after_ms: null,
        sampler: null,
        sequence: null,
        observed_at_ms: null,
        observed_monotonic_ms: null,
      };
      await state.read(
        createDesktopClient(async () => policyReply(memoryJob(A, p))),
        A,
        () => true,
        () => 0,
        () => {},
      );
      expect(state.section.error).toBeNull();
      expect(state.section.value?.data.mode).toBe("l0");
      expect(policyAnnouncement(state.section)).not.toContain("Normal");
    }
  });
}
