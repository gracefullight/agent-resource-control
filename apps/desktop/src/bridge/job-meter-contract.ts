import type {
  JobMeterReport,
  JobMeterSummary,
  JobProcessIdentity,
  QueueWait,
} from "@/bridge/job-meter.types";

const MAX = Number.MAX_SAFE_INTEGER;
const id = (v: unknown): v is string =>
  typeof v === "string" && /^[0-9a-f]{32}$/.test(v);
const integer = (v: unknown, min = 0, max = MAX): v is number =>
  typeof v === "number" && Number.isSafeInteger(v) && v >= min && v <= max;
const seconds = (v: unknown): v is number | null =>
  v === null ||
  (typeof v === "number" && Number.isFinite(v) && v >= 0 && v <= MAX);
const object = (
  v: unknown,
  keys: readonly string[],
): v is Record<string, unknown> =>
  v !== null &&
  typeof v === "object" &&
  !Array.isArray(v) &&
  Object.keys(v).length === keys.length &&
  keys.every((k) => Object.hasOwn(v, k));
const reasons = [
  "unobserved_descendants",
  "detached_descendants",
  "native_usage_unavailable",
  "run_duration_unavailable",
  "lineage_unverified",
  "interrupted",
  "unsupported",
] as const;
function identity(v: unknown): v is JobProcessIdentity {
  return (
    object(v, ["pid", "start_token", "boot_id"]) &&
    integer(v.pid, 1, 2_147_483_647) &&
    typeof v.start_token === "string" &&
    /^[1-9][0-9]{0,19}$/.test(v.start_token) &&
    BigInt(v.start_token) <= 18_446_744_073_709_551_615n &&
    typeof v.boot_id === "string" &&
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(
      v.boot_id,
    )
  );
}
function queue(v: unknown): v is QueueWait {
  if (!object(v, ["seconds", "clock", "basis"]) || !seconds(v.seconds))
    return false;
  switch (v.basis) {
    case "queued_to_reserved":
      return v.clock === "daemon_wall_clock" && v.seconds !== null;
    case "lifecycle_only":
      return v.clock === "not_applicable" && v.seconds === 0;
    case "unavailable":
      return v.clock === "unknown" && v.seconds === null;
    default:
      return false;
  }
}
function outcome(v: unknown): boolean {
  return (
    (object(v, ["kind", "code"]) &&
      v.kind === "EXITED" &&
      integer(v.code, 0, 255)) ||
    (object(v, ["kind", "signal", "core_dumped"]) &&
      v.kind === "SIGNALED" &&
      integer(v.signal, 1, 64) &&
      typeof v.core_dumped === "boolean")
  );
}
function cpu(v: unknown): boolean {
  if (
    !object(v, ["user", "system", "total"]) ||
    !seconds(v.user) ||
    !seconds(v.system) ||
    !seconds(v.total)
  )
    return false;
  if (v.user === null || v.system === null) return v.total === null;
  return (
    v.total !== null &&
    Math.abs(v.user + v.system - v.total) <=
      Math.max(1e-9, v.total * Number.EPSILON * 2)
  );
}
function report(v: unknown, jobId: string): v is JobMeterReport {
  if (
    !object(v, [
      "schema_version",
      "report_id",
      "job_id",
      "observer",
      "child",
      "cpu_core_seconds",
      "run_elapsed",
      "attribution_basis",
      "scope",
      "completeness",
      "missing_coverage",
      "accounting_relation",
      "outcome",
      "terminated_at_ms",
    ]) ||
    v.schema_version !== 1 ||
    !id(v.report_id) ||
    v.job_id !== jobId ||
    !identity(v.observer) ||
    !identity(v.child) ||
    v.observer.pid === v.child.pid ||
    v.observer.boot_id !== v.child.boot_id ||
    !cpu(v.cpu_core_seconds) ||
    !object(v.run_elapsed, ["seconds", "clock", "includes_stopped"]) ||
    !seconds(v.run_elapsed.seconds) ||
    v.run_elapsed.clock !== "monotonic" ||
    v.run_elapsed.includes_stopped !== true ||
    v.attribution_basis !== "native_owned_child_wait4" ||
    v.scope !== "native_waited_descendants" ||
    !outcome(v.outcome) ||
    !(v.terminated_at_ms === null || integer(v.terminated_at_ms)) ||
    !Array.isArray(v.missing_coverage) ||
    v.missing_coverage.length > 8 ||
    new Set(v.missing_coverage).size !== v.missing_coverage.length ||
    !v.missing_coverage.every((r) => reasons.some((reason) => reason === r))
  )
    return false;
  // Object and numeric checks above establish the typed components for semantics.
  const r = v as unknown as JobMeterReport;
  const has = (reason: string) =>
    r.missing_coverage.some((item) => item === reason);
  const c = r.cpu_core_seconds,
    run = r.run_elapsed,
    relation = r.accounting_relation;
  if (
    (c.total === null &&
      !["native_usage_unavailable", "interrupted", "unsupported"].some(has)) ||
    (run.seconds === null &&
      !["run_duration_unavailable", "interrupted", "unsupported"].some(has)) ||
    (has("native_usage_unavailable") && c.total !== null) ||
    (has("unsupported") && c.total !== null) ||
    (has("run_duration_unavailable") && run.seconds !== null)
  )
    return false;
  switch (r.completeness) {
    case "complete":
      if (r.missing_coverage.length || c.total === null || run.seconds === null)
        return false;
      break;
    case "partial":
      if (!r.missing_coverage.length) return false;
      break;
    case "unknown":
      if (
        !r.missing_coverage.length ||
        [c.user, c.system, c.total].some((v) => v !== null)
      )
        return false;
      break;
    default:
      return false;
  }
  if (object(relation, ["kind"]) && relation.kind === "unverified")
    return r.completeness !== "complete" && has("lineage_unverified");
  return (
    object(relation, ["kind", "root_job_id"]) &&
    id(relation.root_job_id) &&
    !has("lineage_unverified") &&
    ((relation.kind === "canonical_root" && relation.root_job_id === jobId) ||
      (relation.kind === "included_in_root" && relation.root_job_id !== jobId))
  );
}

// Mirror the shared Rust validator, including required nulls, bounded wire size,
// exact object tags and job correlation. No raw diagnostics enter this schema.
export function isJobMeterSummary(
  v: unknown,
  requestedId?: string,
): v is JobMeterSummary {
  if (
    !object(v, ["schema_version", "job_id", "accounting", "queue_wait"]) ||
    v.schema_version !== 1 ||
    !id(v.job_id) ||
    (requestedId !== undefined && v.job_id !== requestedId) ||
    !queue(v.queue_wait)
  )
    return false;
  const a = v.accounting;
  const valid =
    (object(a, ["state", "report"]) &&
      a.state === "recorded" &&
      report(a.report, v.job_id)) ||
    (object(a, ["state", "reason"]) &&
      a.state === "unknown" &&
      [
        "legacy",
        "interrupted",
        "unavailable",
        "not_terminated",
        "unsupported",
      ].some((r) => r === a.reason)) ||
    (object(a, ["state"]) &&
      (a.state === "missing" || a.state === "expired") &&
      v.queue_wait.seconds === null);
  if (!valid) return false;
  try {
    return new TextEncoder().encode(JSON.stringify(v)).length <= 16_384;
  } catch {
    return false;
  }
}
