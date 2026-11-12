import type {
  MissingCoverageReason,
  SummaryAccounting,
} from "@/bridge/job-meter.types";

// Same English source-of-truth pattern as desktop-messages; no peer diagnostics.
export const jobMeterMessages = {
  title: "Job measurements",
  cpu: "CPU used",
  run: "Run elapsed",
  queue: "Queue wait",
  cpuHelp: "Cumulative user + system CPU, in core-seconds.",
  runHelp:
    "Monotonic spawn-to-termination interval; includes stopped time and excludes queue wait.",
  queueHelp: "Recorded separately from running duration.",
  loading: "Loading job measurements…",
  refreshing: "Refreshing job measurements…",
  current: "Latest lookup received",
  unavailable: "Job measurements unavailable",
  unsupported: "Job measurements are not supported by this daemon.",
  none: "No job measurements recorded.",
  missing: "No matching job measurements.",
  missingHelp: "Absence does not establish expiry or whether the command ran.",
  expired: "Job measurements expired under the 90-day metadata policy.",
  retained: "Last recorded measurements",
  received: "Measurements received by desktop",
  lookup: "Latest measurement lookup received by desktop",
  complete: "Complete for recorded scope",
  partial: "Partial — {reason}",
  coverageUnknown: "Unknown coverage — {reason}",
  scope: "Accounting scope",
  scopeHelp:
    "Owned child plus CPU propagated by native waits; detached or unobserved work may be absent.",
  basis: "Attribution basis",
  basisHelp: "Native termination accounting of the CLI-owned child.",
  details: "Accounting details",
  user: "User CPU",
  system: "System CPU",
  total: "Exact CPU used",
  exactRun: "Exact run elapsed",
  exactQueue: "Exact queue wait",
  queueBasis: "Queue basis / clock",
  observer: "CLI observer identity",
  child: "Owned child identity",
  process: "PID {pid} · start token {token} · boot {boot}",
  report: "Report ID",
  terminated: "Native termination recorded",
  included: "Included in accounting root {root}; not additive.",
  root: "Accounting root {root}",
  unverified: "Accounting lineage unverified; not additive.",
  announcement: "Job measurements: {state}",
} as const;

export const missingCoverageMessages: Record<MissingCoverageReason, string> = {
  unobserved_descendants: "Unobserved descendants may be absent",
  detached_descendants: "Detached descendants are not accounted for",
  native_usage_unavailable: "Native CPU accounting unavailable",
  run_duration_unavailable: "Running duration unavailable",
  lineage_unverified: "Accounting lineage unverified",
  interrupted: "Accounting interrupted",
  unsupported: "Native accounting not supported",
};
export const unknownAccountingMessages: Record<
  Extract<SummaryAccounting, { state: "unknown" }>["reason"],
  string
> = {
  legacy: "Legacy measurement unavailable",
  interrupted: "Accounting interrupted; no terminal measurements recorded.",
  unavailable: "No job measurements recorded.",
  not_terminated: "Waiting for termination accounting.",
  unsupported: jobMeterMessages.unsupported,
};
