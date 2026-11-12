import { invoke } from "@tauri-apps/api/core";
import type { MeterSnapshot } from "@/bridge/meter.types";
import { isMeterSnapshot } from "@/bridge/meter-contract";
import type {
  CreditLink,
  DaemonHealth,
  DesktopClient,
  DesktopError,
  DesktopMetadata,
  IdentityObservation,
  JobDetail,
  JobId,
  JobLookup,
  JobSummary,
  Outcome,
  Received,
  RecentJobs,
} from "@/bridge/types";

const PHASES = [
  "REGISTERED",
  "LAUNCHING",
  "RUNNING",
  "COMPLETED",
  "SIGNALED",
  "SPAWN_FAILED",
  "OUTCOME_UNKNOWN",
] as const;
const REASONS = [
  "NONE",
  "REGISTERED_NOT_CLAIMED",
  "CLAIMED_NOT_ATTACHED",
  "OBSERVER_MISSING_NOT_CLAIMED",
  "OBSERVER_MISSING",
  "PROCESS_MISSING",
  "IDENTITY_MISMATCH",
  "IDENTITY_UNAVAILABLE",
  "CHILD_VERIFIED",
  "OBSERVED_EXIT",
  "OBSERVED_SIGNAL",
  "OBSERVED_SPAWN_FAILURE",
] as const;
const ERROR_CODES = [
  "unavailable",
  "uncertain",
  "busy",
  "rejected",
  "protocol",
  "invalid_reply",
  "invalid_request",
  "invalid_endpoint",
  "local",
  "cancelled",
  "unsupported",
  "opener_failed",
  "window_unavailable",
] as const;
const CREDIT_LINKS = ["github", "linkedin", "oma"] as const;

export type DesktopCommand =
  | "daemon_health"
  | "meter_snapshot"
  | "recent_jobs"
  | "job_by_id"
  | "desktop_metadata"
  | "open_credit_link"
  | "open_dashboard"
  | "hide_dashboard"
  | "quit_desktop";

// Injection seam for deterministic tests; consumers get only the fixed methods.
export type DesktopInvoke = (
  command: DesktopCommand,
  args?: { job_id: JobId } | { link: CreditLink },
) => Promise<unknown>;

function exactObject(
  value: unknown,
  keys: readonly string[],
): value is Record<string, unknown> {
  return (
    value !== null &&
    typeof value === "object" &&
    !Array.isArray(value) &&
    Object.keys(value).length === keys.length &&
    keys.every((key) => Object.hasOwn(value, key))
  );
}
function member<T extends readonly string[]>(
  value: unknown,
  choices: T,
): value is T[number] {
  return (
    typeof value === "string" && choices.some((choice) => choice === value)
  );
}
function integer(
  value: unknown,
  min = 0,
  max = Number.MAX_SAFE_INTEGER,
): value is number {
  return (
    typeof value === "number" &&
    Number.isSafeInteger(value) &&
    value >= min &&
    value <= max
  );
}
function nullableMs(value: unknown): value is number | null {
  return value === null || integer(value);
}
function nonempty(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}
export function isJobId(value: unknown): value is JobId {
  return typeof value === "string" && /^[0-9a-f]{32}$/.test(value);
}
export function isDesktopError(value: unknown): value is DesktopError {
  return (
    exactObject(value, ["code", "retryable"]) &&
    member(value.code, ERROR_CODES) &&
    typeof value.retryable === "boolean"
  );
}
export function normalizeDesktopError(value: unknown): DesktopError {
  // Never stringify a rejected Tauri/serde/OS error or retain peer diagnostics.
  return isDesktopError(value)
    ? { code: value.code, retryable: value.retryable }
    : { code: "local", retryable: false };
}
export function isDaemonHealth(value: unknown): value is DaemonHealth {
  return (
    exactObject(value, ["daemon", "protocol_version", "sweeper"]) &&
    value.daemon === "running" &&
    value.protocol_version === 1 &&
    value.sweeper === "off"
  );
}
export function isReceived<T>(
  value: unknown,
  validate: (data: unknown) => data is T,
): value is Received<T> {
  return (
    exactObject(value, ["data", "received_at_ms"]) &&
    integer(value.received_at_ms) &&
    validate(value.data)
  );
}
export function isJobSummary(value: unknown): value is JobSummary {
  return (
    exactObject(value, [
      "job_id",
      "phase",
      "registered_at_ms",
      "updated_at_ms",
      "reason",
    ]) &&
    isJobId(value.job_id) &&
    member(value.phase, PHASES) &&
    member(value.reason, REASONS) &&
    integer(value.registered_at_ms) &&
    integer(value.updated_at_ms) &&
    value.registered_at_ms <= value.updated_at_ms
  );
}
export function isRecentJobs(value: unknown): value is RecentJobs {
  if (
    !exactObject(value, [
      "jobs",
      "truncated",
      "observed_at_ms",
      "liveness_fresh",
      "limit",
    ]) ||
    !Array.isArray(value.jobs) ||
    value.jobs.length > 50 ||
    !value.jobs.every(isJobSummary) ||
    typeof value.truncated !== "boolean" ||
    !integer(value.observed_at_ms) ||
    value.liveness_fresh !== false ||
    value.limit !== 50
  )
    return false;
  return (
    new Set(value.jobs.map((job) => job.job_id)).size === value.jobs.length
  );
}
export function isOutcome(value: unknown): value is Outcome {
  if (exactObject(value, ["kind", "code"]) && value.kind === "EXITED") {
    return integer(value.code, 0, 255);
  }
  if (
    exactObject(value, ["kind", "signal", "core_dumped"]) &&
    value.kind === "SIGNALED"
  ) {
    // Native Rust additionally enforces its platform's narrower signal range.
    return (
      integer(value.signal, 1, 127) && typeof value.core_dumped === "boolean"
    );
  }
  return (
    exactObject(value, ["kind", "reason", "os_code"]) &&
    value.kind === "SPAWN_FAILED" &&
    member(value.reason, [
      "NOT_FOUND",
      "PERMISSION_DENIED",
      "INVALID_INPUT",
      "RESOURCE_UNAVAILABLE",
      "OTHER",
    ]) &&
    (value.os_code === null ||
      integer(value.os_code, -2_147_483_648, 2_147_483_647))
  );
}
function isObservation(value: unknown): value is IdentityObservation {
  return (
    exactObject(value, ["target", "status", "observed_at_ms", "reason"]) &&
    member(value.target, ["OBSERVER", "CHILD", "NONE"]) &&
    member(value.status, [
      "VERIFIED_MATCH",
      "MISSING",
      "MISMATCH",
      "INDETERMINATE",
      "NOT_ATTACHED",
    ]) &&
    integer(value.observed_at_ms) &&
    member(value.reason, REASONS) &&
    (value.target !== "NONE" || value.status === "NOT_ATTACHED")
  );
}
export function isJobDetail(value: unknown): value is JobDetail {
  if (
    !exactObject(value, [
      "job_id",
      "phase",
      "reason",
      "observation",
      "outcome",
      "registered_at_ms",
      "launch_claimed_at_ms",
      "completed_at_ms",
      "updated_at_ms",
      "revision",
    ]) ||
    !isJobId(value.job_id) ||
    !member(value.phase, PHASES) ||
    !member(value.reason, REASONS) ||
    !isObservation(value.observation) ||
    !(value.outcome === null || isOutcome(value.outcome)) ||
    !integer(value.registered_at_ms) ||
    !nullableMs(value.launch_claimed_at_ms) ||
    !nullableMs(value.completed_at_ms) ||
    !integer(value.updated_at_ms) ||
    !integer(value.revision, 1)
  )
    return false;
  const times = [
    value.observation.observed_at_ms,
    value.launch_claimed_at_ms,
    value.completed_at_ms,
    value.updated_at_ms,
  ];
  const registeredAt = value.registered_at_ms;
  const updatedAt = value.updated_at_ms;
  if (
    times.some(
      (time) => time !== null && (time < registeredAt || time > updatedAt),
    ) ||
    value.reason !== value.observation.reason ||
    (value.launch_claimed_at_ms !== null &&
      value.completed_at_ms !== null &&
      value.completed_at_ms < value.launch_claimed_at_ms)
  )
    return false;
  if (value.phase === "REGISTERED") {
    if (value.launch_claimed_at_ms !== null) return false;
  } else if (value.launch_claimed_at_ms === null) return false;
  const terminal = member(value.phase, [
    "COMPLETED",
    "SIGNALED",
    "SPAWN_FAILED",
  ]);
  if (
    terminal !== (value.outcome !== null) ||
    terminal !== (value.completed_at_ms !== null)
  )
    return false;
  switch (value.phase) {
    case "COMPLETED":
      return (
        value.outcome?.kind === "EXITED" && value.reason === "OBSERVED_EXIT"
      );
    case "SIGNALED":
      return (
        value.outcome?.kind === "SIGNALED" && value.reason === "OBSERVED_SIGNAL"
      );
    case "SPAWN_FAILED":
      return (
        value.outcome?.kind === "SPAWN_FAILED" &&
        value.reason === "OBSERVED_SPAWN_FAILURE" &&
        value.observation.target !== "CHILD"
      );
    case "RUNNING":
      return (
        value.reason === "CHILD_VERIFIED" &&
        value.observation.target === "CHILD" &&
        value.observation.status === "VERIFIED_MATCH"
      );
    default:
      return !member(value.reason, [
        "OBSERVED_EXIT",
        "OBSERVED_SIGNAL",
        "OBSERVED_SPAWN_FAILURE",
      ]);
  }
}
export function isJobLookup(
  value: unknown,
  requestedId?: JobId,
): value is JobLookup {
  return (
    (exactObject(value, ["kind"]) && value.kind === "not_found") ||
    (exactObject(value, ["kind", "job"]) &&
      value.kind === "found" &&
      isJobDetail(value.job) &&
      (requestedId === undefined || value.job.job_id === requestedId))
  );
}
export function isDesktopMetadata(value: unknown): value is DesktopMetadata {
  return (
    exactObject(value, [
      "product_name",
      "version",
      "build_commit",
      "build_dirty",
      "os",
      "architecture",
      "license",
      "open_source_notices",
    ]) &&
    value.product_name === "Agent Resource Control" &&
    nonempty(value.version) &&
    (value.build_commit === null ||
      (typeof value.build_commit === "string" &&
        /^(?:[0-9a-f]{40}|[0-9a-f]{64})$/.test(value.build_commit))) &&
    (value.build_dirty === null || typeof value.build_dirty === "boolean") &&
    nonempty(value.os) &&
    nonempty(value.architecture) &&
    (value.license === null || nonempty(value.license)) &&
    typeof value.open_source_notices === "string"
  );
}
const invalidRequest = (): DesktopError => ({
  code: "invalid_request",
  retryable: false,
});

export function createDesktopClient(
  nativeInvoke: DesktopInvoke = (command, args) =>
    invoke<unknown>(command, args),
): DesktopClient {
  async function read<T>(
    command: DesktopCommand,
    validate: (value: unknown) => value is T,
    args?: { job_id: JobId },
  ): Promise<T> {
    try {
      const reply = await nativeInvoke(command, args);
      if (!validate(reply))
        throw {
          code: "invalid_reply",
          retryable: false,
        } satisfies DesktopError;
      return reply;
    } catch (error) {
      throw normalizeDesktopError(error);
    }
  }
  async function action(
    command: DesktopCommand,
    args?: { link: CreditLink },
  ): Promise<void> {
    try {
      const reply = await nativeInvoke(command, args);
      if (reply !== null && reply !== undefined)
        throw {
          code: "invalid_reply",
          retryable: false,
        } satisfies DesktopError;
    } catch (error) {
      throw normalizeDesktopError(error);
    }
  }
  return {
    meterSnapshot: () =>
      read("meter_snapshot", (value): value is Received<MeterSnapshot> =>
        isReceived(value, isMeterSnapshot),
      ),
    daemonHealth: () =>
      read("daemon_health", (value): value is Received<DaemonHealth> =>
        isReceived(value, isDaemonHealth),
      ),
    recentJobs: () =>
      read("recent_jobs", (value): value is Received<RecentJobs> =>
        isReceived(value, isRecentJobs),
      ),
    jobById: async (jobId) => {
      if (!isJobId(jobId)) throw invalidRequest();
      return read(
        "job_by_id",
        (value): value is Received<JobLookup> =>
          isReceived(value, (data): data is JobLookup =>
            isJobLookup(data, jobId),
          ),
        { job_id: jobId },
      );
    },
    desktopMetadata: () => read("desktop_metadata", isDesktopMetadata),
    openCreditLink: async (link) => {
      if (!member(link, CREDIT_LINKS)) throw invalidRequest();
      await action("open_credit_link", { link });
    },
    openDashboard: () => action("open_dashboard"),
    hideDashboard: () => action("hide_dashboard"),
    quitDesktop: () => action("quit_desktop"),
  };
}

export const desktopClient = createDesktopClient();
