import { expect, test } from "bun:test";
import { createDesktopClient } from "@/bridge/client";
import type { JobMeterSummary } from "@/bridge/job-meter.types";
import { isJobMeterSummary } from "@/bridge/job-meter-contract";
import {
  jobMeterFixture,
  jobReport,
  sharedJobReport,
  unknownJobMeter,
  unknownQueue,
} from "./job-meter-fixture";

const invalidReply = { code: "invalid_reply", retryable: false };
export function registerJobMeterContractCases() {
  test("job summary consumes the exact Rust report golden and all explicit unavailable states", () => {
    const r = sharedJobReport();
    expect(
      isJobMeterSummary({
        schema_version: 1,
        job_id: r.job_id,
        accounting: { state: "recorded", report: r },
        queue_wait: unknownQueue(),
      }),
    ).toBe(true);
    const s = jobMeterFixture(),
      report = jobReport(s);
    expect(isJobMeterSummary(s, s.job_id)).toBe(true);
    expect(isJobMeterSummary(s, "b".repeat(32))).toBe(false);
    for (const reason of [
      "legacy",
      "interrupted",
      "unavailable",
      "not_terminated",
      "unsupported",
    ] as const)
      expect(isJobMeterSummary(unknownJobMeter(s.job_id, reason))).toBe(true);
    for (const state of ["missing", "expired"])
      expect(
        isJobMeterSummary({
          ...s,
          accounting: { state },
          queue_wait: unknownQueue(),
        }),
      ).toBe(true);
    for (const seconds of [0, 0.0002, Number.MAX_SAFE_INTEGER]) {
      const v = structuredClone(s),
        r = jobReport(v);
      r.cpu_core_seconds = { user: seconds, system: 0, total: seconds };
      r.run_elapsed.seconds = seconds;
      expect(isJobMeterSummary(v)).toBe(true);
    }
    report.completeness = "partial";
    report.missing_coverage = ["unobserved_descendants"];
    report.accounting_relation = {
      kind: "included_in_root",
      root_job_id: "b".repeat(32),
    };
    expect(isJobMeterSummary(s)).toBe(true);
    report.accounting_relation = { kind: "unverified" };
    report.missing_coverage = ["lineage_unverified"];
    expect(isJobMeterSummary(s)).toBe(true);
    report.completeness = "unknown";
    report.cpu_core_seconds = { user: null, system: null, total: null };
    report.missing_coverage.push("native_usage_unavailable");
    expect(isJobMeterSummary(s)).toBe(true);
    report.cpu_core_seconds.user = 0;
    expect(isJobMeterSummary(s)).toBe(false);
  });

  test("job summary rejects malformed object trees, fields, units, totals, lineage, scope and private content", async () => {
    const s = jobMeterFixture(),
      r = jobReport(s);
    const withReport = (report: unknown) => ({
      ...s,
      accounting: { state: "recorded", report },
    });
    const bad: unknown[] = [
      null,
      [],
      {},
      r,
      { ...s, schema_version: 2 },
      { ...s, job_id: "A".repeat(32) },
      { ...s, private: "PRIVATE_SENTINEL" },
      { ...s, accounting: ["recorded", r] },
      { ...s, queue_wait: Object.values(s.queue_wait) },
      {
        ...s,
        queue_wait: { seconds: 0, clock: "unknown", basis: "unavailable" },
      },
      {
        ...s,
        queue_wait: {
          seconds: 3,
          clock: "monotonic",
          basis: "queued_to_reserved",
        },
      },
      {
        ...s,
        queue_wait: {
          seconds: 3,
          clock: "not_applicable",
          basis: "lifecycle_only",
        },
      },
      {
        ...s,
        queue_wait: {
          seconds: null,
          clock: "daemon_wall_clock",
          basis: "queued_to_reserved",
        },
      },
      {
        ...s,
        queue_wait: {
          seconds: null,
          clock: "unknown",
          basis: "unavailable",
          ttl: 90,
        },
      },
      { ...s, accounting: { state: "unknown", reason: "guessed" } },
      { ...s, accounting: { state: "missing" } },
      { ...s, accounting: { state: "expired" } },
      withReport(Object.values(r)),
      ...[
        { ...r, job_id: "b".repeat(32) },
        { ...r, schema_version: 2 },
        { ...r, report_id: "no" },
        { ...r, observer: Object.values(r.observer) },
        { ...r, child: r.observer },
        { ...r, child: { ...r.child, pid: 0 } },
        { ...r, child: { ...r.child, pid: 2_147_483_648 } },
        { ...r, child: { ...r.child, boot_id: "f".repeat(36) } },
        {
          ...r,
          child: {
            ...r.child,
            boot_id: "00000000-0000-0000-0000-000000000002",
          },
        },
        { ...r, child: { ...r.child, start_token: "01" } },
        { ...r, child: { ...r.child, start_token: "18446744073709551616" } },
        { ...r, child: { ...r.child, private: "PRIVATE_SENTINEL" } },
        { ...r, cpu_core_seconds: { user: 1, system: 1, total: 3 } },
        { ...r, cpu_core_seconds: { user: 1, system: null, total: 1 } },
        { ...r, cpu_core_seconds: { user: 1, system: 1, total: null } },
        { ...r, cpu_core_seconds: [1.25, 0.75, 2] },
        { ...r, cpu_core_seconds: { user: 1.25, system: 0.75 } },
        { ...r, cpu_core_seconds: { user: null, system: null, total: null } },
        { ...r, run_elapsed: { ...r.run_elapsed, clock: "wall" } },
        { ...r, run_elapsed: { ...r.run_elapsed, includes_stopped: false } },
        { ...r, run_elapsed: { ...r.run_elapsed, seconds: null } },
        { ...r, attribution_basis: "sampled" },
        { ...r, scope: "complete_process_tree" },
        { ...r, terminated_at_ms: undefined },
        { ...r, terminated_at_ms: -1 },
        { ...r, outcome: { kind: "EXITED", code: 256 } },
        { ...r, outcome: { kind: "SIGNALED", signal: 0, core_dumped: false } },
        { ...r, outcome: { kind: "SIGNALED", signal: 65, core_dumped: false } },
        { ...r, outcome: { kind: "SIGNALED", signal: 15 } },
        {
          ...r,
          outcome: { kind: "SPAWN_FAILED", reason: "NOT_FOUND", os_code: 2 },
        },
        { ...r, missing_coverage: ["arbitrary reason"] },
        { ...r, missing_coverage: ["unobserved_descendants"] },
        { ...r, completeness: "partial", missing_coverage: [] },
        { ...r, completeness: "unknown", missing_coverage: ["interrupted"] },
        {
          ...r,
          completeness: "partial",
          missing_coverage: [
            "unobserved_descendants",
            "unobserved_descendants",
          ],
        },
        {
          ...r,
          completeness: "partial",
          missing_coverage: ["native_usage_unavailable"],
        },
        {
          ...r,
          completeness: "partial",
          missing_coverage: ["run_duration_unavailable"],
        },
        { ...r, accounting_relation: { kind: "unverified" } },
        {
          ...r,
          accounting_relation: {
            kind: "canonical_root",
            root_job_id: "b".repeat(32),
          },
        },
        {
          ...r,
          accounting_relation: {
            kind: "included_in_root",
            root_job_id: s.job_id,
          },
        },
        {
          ...r,
          accounting_relation: { kind: "included_in_root", root_job_id: "bad" },
        },
        {
          ...r,
          completeness: "partial",
          missing_coverage: ["lineage_unverified"],
        },
        { ...r, completeness: "guessed" },
      ].map(withReport),
    ];
    for (const value of [
      NaN,
      Infinity,
      -1,
      Number.MAX_SAFE_INTEGER + 1,
      undefined,
    ]) {
      bad.push(
        withReport({
          ...r,
          cpu_core_seconds: { user: value, system: 0, total: value },
        }),
      );
      bad.push(
        withReport({ ...r, run_elapsed: { ...r.run_elapsed, seconds: value } }),
      );
    }
    for (const data of bad) {
      expect(isJobMeterSummary(data)).toBe(false);
      await expect(
        createDesktopClient(async () => ({
          data,
          received_at_ms: 123,
        })).meterSummary(s.job_id),
      ).rejects.toEqual(invalidReply);
    }
  });

  test("nullable metrics preserve only what was actually recorded with explicit unavailability reasons", () => {
    const s = jobMeterFixture(),
      r = jobReport(s);
    r.completeness = "partial";
    r.missing_coverage = ["native_usage_unavailable"];
    r.cpu_core_seconds = { user: 1, system: null, total: null };
    expect(isJobMeterSummary(s)).toBe(true);
    r.run_elapsed.seconds = null;
    r.missing_coverage.push("run_duration_unavailable");
    expect(isJobMeterSummary(s)).toBe(true);
    const v: JobMeterSummary = {
      ...s,
      accounting: { state: "unknown", reason: "not_terminated" },
    };
    expect(isJobMeterSummary(v)).toBe(true);
    // A captured user+system value can exceed run time with parallel workers.
    const parallel = jobMeterFixture();
    jobReport(parallel).run_elapsed.seconds = 0.5;
    expect(isJobMeterSummary(parallel)).toBe(true);
  });
}
