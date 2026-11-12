import { describe, expect, test } from "bun:test";
import {
  createDesktopClient,
  isJobDetail,
  isRecentJobs,
} from "../src/bridge/client";
import type {
  CreditLink,
  DaemonHealth,
  JobDetail,
  Received,
} from "../src/bridge/types";
import { meterReply } from "./fixtures/meter-fixture";

const JOB_A = "0000000000000000000000000000000a";
const JOB_B = "0000000000000000000000000000000b";
const detail: JobDetail = {
  job_id: JOB_A,
  phase: "COMPLETED",
  reason: "OBSERVED_EXIT",
  observation: {
    target: "CHILD",
    status: "MISSING",
    observed_at_ms: 1300,
    reason: "OBSERVED_EXIT",
  },
  outcome: { kind: "EXITED", code: 17 },
  registered_at_ms: 1000,
  launch_claimed_at_ms: 1100,
  completed_at_ms: 1200,
  updated_at_ms: 1300,
  revision: 3,
};

describe("fixed desktop invoke boundary", () => {
  test("Meter is a fixed argument-free read and preserves scoped unsupported/unavailable errors", async () => {
    const calls: unknown[] = [];
    const reply = meterReply();
    const client = createDesktopClient(async (command, args) => {
      calls.push([command, args]);
      return reply;
    });
    expect(await client.meterSnapshot()).toEqual(reply);
    expect(calls).toEqual([["meter_snapshot", undefined]]);
    for (const error of [
      { code: "unsupported", retryable: false },
      { code: "unavailable", retryable: true },
    ] as const)
      await expect(
        createDesktopClient(async () => {
          throw error;
        }).meterSnapshot(),
      ).rejects.toEqual(error);
    await expect(
      createDesktopClient(async () => {
        throw "raw-private-diagnostic";
      }).meterSnapshot(),
    ).rejects.toEqual({ code: "local", retryable: false });
  });
  test("uses the fixed health command and preserves a separate receipt time", async () => {
    const calls: unknown[] = [];
    const reply = {
      data: { daemon: "running", protocol_version: 1, sweeper: "off" },
      received_at_ms: 1500,
    } satisfies Received<DaemonHealth>;
    const client = createDesktopClient(async (command, args) => {
      calls.push([command, args]);
      return reply;
    });
    expect(await client.daemonHealth()).toEqual(reply);
    expect(calls).toEqual([["daemon_health", undefined]]);
  });

  test("selects by exact snake_case ID and retains recorded exit 17", async () => {
    const calls: unknown[] = [];
    const client = createDesktopClient(async (command, args) => {
      calls.push([command, args]);
      return { data: { kind: "found", job: detail }, received_at_ms: 1500 };
    });
    const reply = await client.jobById(JOB_A);
    expect(calls).toEqual([["job_by_id", { job_id: JOB_A }]]);
    expect(reply.data).toEqual({ kind: "found", job: detail });
  });

  test("rejects a different selected ID before the UI can replace its detail", async () => {
    const client = createDesktopClient(async () => ({
      data: { kind: "found", job: { ...detail, job_id: JOB_B } },
      received_at_ms: 1500,
    }));
    await expect(client.jobById(JOB_A)).rejects.toEqual({
      code: "invalid_reply",
      retryable: false,
    });
  });

  test("rejects unsafe receipts, wrong protocol and extra private fields", async () => {
    for (const reply of [
      {
        data: { daemon: "running", protocol_version: 1, sweeper: "off" },
        received_at_ms: Number.MAX_SAFE_INTEGER + 1,
      },
      {
        data: { daemon: "running", protocol_version: 2, sweeper: "off" },
        received_at_ms: 1500,
      },
      {
        data: {
          daemon: "running",
          protocol_version: 1,
          sweeper: "off",
          command: "private-sentinel",
        },
        received_at_ms: 1500,
      },
    ]) {
      await expect(
        createDesktopClient(async () => reply).daemonHealth(),
      ).rejects.toEqual({ code: "invalid_reply", retryable: false });
    }
  });

  test("does not expose raw rejected diagnostics", async () => {
    const client = createDesktopClient(async () => {
      throw new Error("private-command-path-sentinel");
    });
    await expect(client.daemonHealth()).rejects.toEqual({
      code: "local",
      retryable: false,
    });
    const transient = createDesktopClient(async () => {
      throw { code: "unavailable", retryable: true };
    });
    await expect(transient.daemonHealth()).rejects.toEqual({
      code: "unavailable",
      retryable: true,
    });
  });

  test("invalid IDs and URLs cannot cause a native invocation", async () => {
    let calls = 0;
    const client = createDesktopClient(async () => {
      calls++;
      return null;
    });
    await expect(client.jobById("not-a-job")).rejects.toEqual({
      code: "invalid_request",
      retryable: false,
    });
    await expect(
      client.openCreditLink("https://example.com" as CreditLink),
    ).rejects.toEqual({ code: "invalid_request", retryable: false });
    await Promise.resolve();
    expect(calls).toBe(0);
  });

  test("only enum credit links and local lifecycle command names cross the boundary", async () => {
    const calls: unknown[] = [];
    const client = createDesktopClient(async (command, args) => {
      calls.push([command, args]);
      return null;
    });
    await client.openCreditLink("oma");
    await client.openDashboard();
    await client.hideDashboard();
    await client.quitDesktop();
    expect(calls).toEqual([
      ["open_credit_link", { link: "oma" }],
      ["open_dashboard", undefined],
      ["hide_dashboard", undefined],
      ["quit_desktop", undefined],
    ]);
  });
});

describe("recorded data invariants", () => {
  test("completed detail needs an explicit matching outcome and explicit null fields", () => {
    expect(isJobDetail(detail)).toBe(true);
    expect(
      isJobDetail({ ...detail, outcome: { kind: "EXITED", code: 0 } }),
    ).toBe(true);
    expect(isJobDetail({ ...detail, outcome: null })).toBe(false);
    expect(
      isJobDetail({
        ...detail,
        outcome: { kind: "SIGNALED", signal: 9, core_dumped: false },
      }),
    ).toBe(false);
    const missing = { ...detail } as Partial<JobDetail>;
    delete missing.completed_at_ms;
    expect(isJobDetail(missing)).toBe(false);
    expect(
      isJobDetail({
        ...detail,
        observation: { ...detail.observation, observed_at_ms: 999 },
      }),
    ).toBe(false);
  });

  test("an unresolved record can remain unknown without being a failed exit", () => {
    expect(
      isJobDetail({
        ...detail,
        phase: "OUTCOME_UNKNOWN",
        reason: "PROCESS_MISSING",
        observation: {
          target: "CHILD",
          status: "MISSING",
          observed_at_ms: 1300,
          reason: "PROCESS_MISSING",
        },
        outcome: null,
        completed_at_ms: null,
      }),
    ).toBe(true);
  });

  test("recent history stays bounded, unique and explicitly last-known", () => {
    const summary = {
      job_id: JOB_A,
      phase: "COMPLETED",
      reason: "OBSERVED_EXIT",
      registered_at_ms: 1000,
      updated_at_ms: 1300,
    };
    const recent = {
      jobs: [summary],
      truncated: false,
      observed_at_ms: 1400,
      liveness_fresh: false,
      limit: 50,
    };
    expect(isRecentJobs(recent)).toBe(true);
    expect(isRecentJobs({ ...recent, jobs: [summary, summary] })).toBe(false);
    expect(
      isRecentJobs({
        ...recent,
        jobs: Array.from({ length: 51 }, (_, i) => ({
          ...summary,
          job_id: i.toString(16).padStart(32, "0"),
        })),
      }),
    ).toBe(false);
    expect(isRecentJobs({ ...recent, liveness_fresh: true })).toBe(false);
  });
});
