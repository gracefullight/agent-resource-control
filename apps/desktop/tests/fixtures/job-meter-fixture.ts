import type {
  JobMeterReport,
  JobMeterSummary,
  QueueWait,
  SummaryAccounting,
} from "@/bridge/job-meter.types";
import { isJobMeterSummary } from "@/bridge/job-meter-contract";
import type { Received } from "@/bridge/types";
import shared from "../../../../crates/arc-core/tests/fixtures/job-meter-report.json";
import bridge from "./bridge.json";

// Shared fake native identities/numbers, never calibration or persistence proof.
export function sharedJobReport(): JobMeterReport {
  const report = structuredClone(shared);
  const s: unknown = {
    schema_version: 1,
    job_id: report.job_id,
    accounting: { state: "recorded", report },
    queue_wait: unknownQueue(),
  };
  if (!isJobMeterSummary(s) || s.accounting.state !== "recorded")
    throw new Error(
      "Shared Rust job Meter fixture no longer matches TypeScript",
    );
  return s.accounting.report;
}
export const unknownQueue = (): QueueWait => ({
  seconds: null,
  clock: "unknown",
  basis: "unavailable",
});
export function jobMeterFixture(
  jobId = bridge.exit_zero.data.job.job_id,
): JobMeterSummary {
  const report = sharedJobReport();
  report.job_id = jobId;
  report.accounting_relation = { kind: "canonical_root", root_job_id: jobId };
  report.outcome = {
    kind: "EXITED",
    code: jobId === bridge.exit_seventeen.data.job.job_id ? 17 : 0,
  };
  report.run_elapsed.seconds = 10;
  return {
    schema_version: 1,
    job_id: jobId,
    accounting: { state: "recorded", report },
    queue_wait: {
      seconds: 3,
      basis: "queued_to_reserved",
      clock: "daemon_wall_clock",
    },
  };
}
export function jobReport(s: JobMeterSummary): JobMeterReport {
  if (s.accounting.state !== "recorded")
    throw new Error("Expected synthetic recorded summary");
  return s.accounting.report;
}
export function unknownJobMeter(
  jobId: string,
  reason: Extract<SummaryAccounting, { state: "unknown" }>["reason"] = "legacy",
): JobMeterSummary {
  return {
    schema_version: 1,
    job_id: jobId,
    accounting: { state: "unknown", reason },
    queue_wait: unknownQueue(),
  };
}
export function jobMeterReply(
  s = jobMeterFixture(),
  received_at_ms = bridge.clock_ms,
): Received<JobMeterSummary> {
  return { data: s, received_at_ms };
}
