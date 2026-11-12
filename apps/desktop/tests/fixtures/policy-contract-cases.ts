import { expect, test } from "bun:test";
import { createDesktopClient } from "@/bridge/client";
import { isEffectivePolicy, isJobPolicyResult } from "@/bridge/policy-contract";
import {
  fixtureDecision,
  fixtureSampler,
  memoryJob,
  memoryPolicy,
  policyReply,
} from "./memory-policy-fixture";

const A = "a".repeat(32),
  B = "b".repeat(32);
export function registerPolicyContractCases() {
  test("inherited object names are invalid decision kinds and normalize to invalid_reply", async () => {
    for (const kind of ["toString", "constructor", "__proto__", "valueOf"]) {
      const value = {
        ...memoryJob(A),
        last_decision: { ...fixtureDecision(memoryJob(A)), kind },
      };
      expect(isJobPolicyResult(value, A)).toBe(false);
      await expect(
        createDesktopClient(async () => policyReply(value)).policyJob(A),
      ).rejects.toEqual({ code: "invalid_reply", retryable: false });
    }
  });
  test("additive policy contract validates exact current and correlated job decisions separately", () => {
    for (const pressure of ["normal", "warning", "critical"] as const) {
      expect(isEffectivePolicy(memoryPolicy(pressure))).toBe(true);
      expect(isJobPolicyResult(memoryJob(A, memoryPolicy(pressure)), A)).toBe(
        true,
      );
    }
    const fallback = memoryPolicy();
    fallback.mode = "l0";
    fallback.reason = "observation_stale";
    fallback.observation.availability = "stale";
    fallback.observation.age_ms = 3001;
    expect(isEffectivePolicy(fallback)).toBe(true);
    expect(isJobPolicyResult(memoryJob(A, fallback), A)).toBe(true);
    expect(isJobPolicyResult(memoryJob(A), B)).toBe(false);
    const arrays = memoryJob(A, memoryPolicy("normal"));
    arrays.last_decision = null;
    arrays.decision_availability = "not_recorded";
    expect(
      isJobPolicyResult({ ...arrays, decision_availability: ["not_recorded"] }),
    ).toBe(false);
    const missing = {
      ...arrays.effective,
      mode: "l0",
      reason: "observation_missing",
      observation: {
        ...arrays.effective.observation,
        availability: ["missing"],
        pressure: null,
        age_ms: null,
        sequence: null,
      },
    };
    expect(isEffectivePolicy(missing)).toBe(false);
    expect(
      isEffectivePolicy({
        ...missing,
        reason: ["observation_invalid"],
        observation: { ...missing.observation, availability: "invalid" },
      }),
    ).toBe(false);
    for (const mutate of [
      (v: ReturnType<typeof memoryJob>) => {
        v.schema_version = 2 as 1;
      },
      (v: ReturnType<typeof memoryJob>) => {
        v.effective.limits.build.window = 2;
      },
      (v: ReturnType<typeof memoryJob>) => {
        v.effective.observation.age_ms = 3001;
      },
      (v: ReturnType<typeof memoryJob>) => {
        v.effective.observation.sequence = Number.MAX_SAFE_INTEGER + 1;
      },
      (v: ReturnType<typeof memoryJob>) => {
        fixtureSampler(v.effective).generation = 0;
      },
      (v: ReturnType<typeof memoryJob>) => {
        v.effective.mode = "l0";
      },
      (v: ReturnType<typeof memoryJob>) => {
        v.decision_availability = "not_recorded";
      },
      (v: ReturnType<typeof memoryJob>) => {
        fixtureDecision(v).kind = "admit";
      },
      (v: ReturnType<typeof memoryJob>) => {
        fixtureSampler(v.effective).daemon_instance.start_token = "0";
      },
    ]) {
      const v = memoryJob(A);
      mutate(v);
      expect(isJobPolicyResult(v, A)).toBe(false);
    }
    const value = memoryJob(A);
    expect(isJobPolicyResult({ ...value, sql: "SELECT" }, A)).toBe(false);
    expect(
      isJobPolicyResult(
        { ...value, effective: Object.values(value.effective) },
        A,
      ),
    ).toBe(false);
    expect(
      isJobPolicyResult(
        {
          ...value,
          last_decision: { ...value.last_decision, secret: "x".repeat(20000) },
        },
        A,
      ),
    ).toBe(false);
  });
  test("policy unavailable/unsupported remain distinct and fixed methods validate before invoking", async () => {
    const calls: unknown[] = [];
    const client = createDesktopClient(async (op, args) => {
      calls.push([op, args]);
      return op === "policy_current"
        ? policyReply(memoryPolicy())
        : policyReply(memoryJob(A));
    });
    await client.policyCurrent();
    await client.policyJob(A);
    expect(calls).toEqual([
      ["policy_current", undefined],
      ["policy_job", { job_id: A }],
    ]);
    await expect(client.policyJob(B)).rejects.toEqual({
      code: "invalid_reply",
      retryable: false,
    });
    await expect(client.policyJob("../private")).rejects.toEqual({
      code: "invalid_request",
      retryable: false,
    });
    expect(calls).toHaveLength(3);
    for (const code of [
      "unsupported",
      "unavailable",
      "invalid_reply",
      "protocol",
      "cancelled",
    ] as const) {
      const error = { code, retryable: code === "unavailable" };
      await expect(
        createDesktopClient(async () => {
          throw error;
        }).policyJob(A),
      ).rejects.toEqual(error);
    }
    await expect(
      createDesktopClient(async () => {
        throw new Error("private/path");
      }).policyCurrent(),
    ).rejects.toEqual({ code: "local", retryable: false });
  });
}
