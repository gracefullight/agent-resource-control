import { describe, expect, test } from "bun:test";
import {
  createDesktopClient,
  type DesktopCommand,
  type DesktopInvoke,
  isDaemonHealth,
  isDesktopMetadata,
  isJobLookup,
  isReceived,
  isRecentJobs,
} from "@/bridge/client";
import type { CreditLink, JobDetail, JobLookup } from "@/bridge/types";
import fixtures from "./fixtures/bridge.json";

const A = fixtures.exit_zero.data.job.job_id;
const B = fixtures.exit_seventeen.data.job.job_id;
const clone = <T>(value: T): T => structuredClone(value);
const invokeReply =
  (reply: unknown): DesktopInvoke =>
  async () =>
    reply;
const invalidReply = { code: "invalid_reply", retryable: false };

describe("delivered production protocol-1 bridge contract", () => {
  test("shared synthetic golden cases validate the exact Rust/TypeScript wire shapes", () => {
    expect(isReceived(fixtures.health, isDaemonHealth)).toBe(true);
    expect(isReceived(fixtures.recent, isRecentJobs)).toBe(true);
    for (const lookup of [
      fixtures.exit_zero,
      fixtures.exit_seventeen,
      fixtures.unknown,
      fixtures.not_found,
    ]) {
      expect(
        isReceived(lookup, (value): value is JobLookup => isJobLookup(value)),
      ).toBe(true);
    }
    expect(isDesktopMetadata(fixtures.metadata)).toBe(true);
    expect((fixtures.exit_zero.data.job.outcome as { code: number }).code).toBe(
      0,
    );
    expect(fixtures.exit_seventeen.data.job.outcome.code).toBe(17);
    expect(fixtures.unknown.data.job.outcome).toBeNull();
  });

  test("the three data commands and snake_case selected-ID argument stay fixed", async () => {
    const calls: {
      command: DesktopCommand;
      args: Parameters<DesktopInvoke>[1];
    }[] = [];
    const client = createDesktopClient(async (command, args) => {
      calls.push({ command, args });
      return command === "daemon_health"
        ? fixtures.health
        : command === "recent_jobs"
          ? fixtures.recent
          : fixtures.exit_zero;
    });
    expect<unknown>(await client.daemonHealth()).toEqual(fixtures.health);
    expect<unknown>(await client.recentJobs()).toEqual(fixtures.recent);
    expect<unknown>(await client.jobById(A)).toEqual(fixtures.exit_zero);
    expect(calls).toEqual([
      { command: "daemon_health", args: undefined },
      { command: "recent_jobs", args: undefined },
      { command: "job_by_id", args: { job_id: A } },
    ]);
    expect("execute" in client).toBe(false);
    expect("stopDaemon" in client).toBe(false);
  });

  test("health requires protocol 1 and supplies no substitute metric fields", async () => {
    for (const data of [
      { ...fixtures.health.data, protocol_version: 2 },
      { ...fixtures.health.data, daemon: "stopped" },
      { ...fixtures.health.data, sweeper: "on" },
      { ...fixtures.health.data, cpu: 0 },
      [],
      null,
    ]) {
      const client = createDesktopClient(
        invokeReply({ ...fixtures.health, data }),
      );
      await expect(client.daemonHealth()).rejects.toEqual(invalidReply);
    }
  });

  test("recent page bounds, uniqueness, timestamps and last-known liveness are enforced", async () => {
    const summary = fixtures.recent.data.jobs[0];
    if (!summary) throw new Error("Missing golden summary");
    for (const data of [
      {
        ...fixtures.recent.data,
        jobs: Array.from({ length: 51 }, (_, i) => ({
          ...summary,
          job_id: i.toString(16).padStart(32, "0"),
        })),
      },
      { ...fixtures.recent.data, jobs: [summary, summary] },
      { ...fixtures.recent.data, limit: 500 },
      { ...fixtures.recent.data, liveness_fresh: true },
      { ...fixtures.recent.data, observed_at_ms: -1 },
      {
        ...fixtures.recent.data,
        jobs: [{ ...summary, updated_at_ms: summary.registered_at_ms - 1 }],
      },
    ])
      await expect(
        createDesktopClient(
          invokeReply({ ...fixtures.recent, data }),
        ).recentJobs(),
      ).rejects.toEqual(invalidReply);
  });

  test("invalid job IDs are rejected before native I/O and a mismatched found ID is invalid", async () => {
    let calls = 0;
    const client = createDesktopClient(async () => {
      calls++;
      return fixtures.exit_seventeen;
    });
    for (const id of [
      "",
      "../socket",
      "A".repeat(32),
      "a".repeat(31),
      "a".repeat(33),
    ]) {
      await expect(client.jobById(id)).rejects.toEqual({
        code: "invalid_request",
        retryable: false,
      });
    }
    expect(calls).toBe(0);
    await expect(client.jobById(A)).rejects.toEqual(invalidReply);
    expect(calls).toBe(1);
    expect<unknown>(await client.jobById(B)).toEqual(fixtures.exit_seventeen);
  });

  test("safe-integer receipts, explicit nullable fields and terminal phase/outcome consistency are required", async () => {
    for (const received_at_ms of [
      Number.NaN,
      Number.POSITIVE_INFINITY,
      Number.MAX_SAFE_INTEGER + 1,
      1.5,
      -1,
      null,
    ]) {
      await expect(
        createDesktopClient(
          invokeReply({ ...fixtures.health, received_at_ms }),
        ).daemonHealth(),
      ).rejects.toEqual(invalidReply);
    }
    const job = clone(fixtures.exit_zero.data.job) as JobDetail;
    const missingOutcome: Partial<JobDetail> = { ...job };
    delete missingOutcome.outcome;
    const missingLaunch: Partial<JobDetail> = { ...job };
    delete missingLaunch.launch_claimed_at_ms;
    const cases: unknown[] = [
      missingOutcome,
      missingLaunch,
      { ...job, outcome: null },
      { ...job, completed_at_ms: null },
      { ...job, phase: "SUCCESS" },
      { ...job, revision: 0 },
      { ...job, outcome: { kind: "EXITED", code: 256 } },
      { ...job, outcome: { kind: "EXITED", code: 0.5 } },
      { ...job, observation: { ...job.observation, reason: "NONE" } },
      { ...job, registered_at_ms: job.updated_at_ms + 1 },
    ];
    for (const value of cases) {
      const reply = {
        ...fixtures.exit_zero,
        data: { kind: "found", job: value },
      };
      await expect(
        createDesktopClient(invokeReply(reply)).jobById(A),
      ).rejects.toEqual(invalidReply);
    }
  });

  test("unsupported result kinds and metadata with invented/missing fields are rejected", async () => {
    await expect(
      createDesktopClient(invokeReply(fixtures.recent)).jobById(A),
    ).rejects.toEqual(invalidReply);
    for (const metadata of [
      { ...fixtures.metadata, build_commit: "a9e7a315" },
      { ...fixtures.metadata, product_name: "Agent Valley" },
      { ...fixtures.metadata, version: "" },
      { ...fixtures.metadata, architecture: "" },
      { ...fixtures.metadata, license: undefined },
    ])
      await expect(
        createDesktopClient(invokeReply(metadata)).desktopMetadata(),
      ).rejects.toEqual(invalidReply);
  });

  test("opener and lifecycle expose exactly the three enums and local unit-returning commands", async () => {
    const calls: {
      command: DesktopCommand;
      args: Parameters<DesktopInvoke>[1];
    }[] = [];
    const client = createDesktopClient(async (command, args) => {
      calls.push({ command, args });
      return null;
    });
    for (const link of ["github", "linkedin", "oma"] as const)
      await client.openCreditLink(link);
    await client.openDashboard();
    await client.hideDashboard();
    await client.quitDesktop();
    expect(calls).toEqual([
      { command: "open_credit_link", args: { link: "github" } },
      { command: "open_credit_link", args: { link: "linkedin" } },
      { command: "open_credit_link", args: { link: "oma" } },
      { command: "open_dashboard", args: undefined },
      { command: "hide_dashboard", args: undefined },
      { command: "quit_desktop", args: undefined },
    ]);
    await expect(
      client.openCreditLink("https://other.example" as CreditLink),
    ).rejects.toEqual({ code: "invalid_request", retryable: false });
    expect(calls).toHaveLength(6);
    await expect(
      createDesktopClient(invokeReply({ status: "ok" })).quitDesktop(),
    ).rejects.toEqual(invalidReply);
  });

  test("rejected native values retain safe error codes and drop raw diagnostics", async () => {
    const known = createDesktopClient(async () => {
      throw fixtures.error;
    });
    await expect(known.daemonHealth()).rejects.toEqual(fixtures.error);
    const raw = createDesktopClient(async () => {
      throw { code: "unavailable", retryable: true, message: "SECRET" };
    });
    await expect(raw.daemonHealth()).rejects.toEqual({
      code: "local",
      retryable: false,
    });
  });
});
