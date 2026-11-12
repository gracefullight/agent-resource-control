import type { JobDetail, Outcome } from "@/bridge/types";

function sameOutcome(previous: Outcome | null, next: Outcome | null): boolean {
  if (previous === null || next === null) return previous === next;
  if (previous.kind === "EXITED" && next.kind === "EXITED")
    return previous.code === next.code;
  if (previous.kind === "SIGNALED" && next.kind === "SIGNALED")
    return (
      previous.signal === next.signal &&
      previous.core_dumped === next.core_dumped
    );
  return (
    previous.kind === "SPAWN_FAILED" &&
    next.kind === "SPAWN_FAILED" &&
    previous.reason === next.reason &&
    previous.os_code === next.os_code
  );
}

export function consistentDetail(
  previous: JobDetail,
  next: JobDetail,
): boolean {
  if (
    previous.job_id !== next.job_id ||
    next.revision < previous.revision ||
    next.registered_at_ms !== previous.registered_at_ms ||
    next.updated_at_ms < previous.updated_at_ms ||
    next.observation.observed_at_ms < previous.observation.observed_at_ms
  )
    return false;
  for (const key of ["launch_claimed_at_ms", "completed_at_ms"] as const) {
    const before = previous[key];
    const after = next[key];
    if (before !== null && (after === null || after < before)) return false;
  }
  // A recorded terminal outcome cannot disappear after another successful read.
  if (
    previous.outcome !== null &&
    (next.phase !== previous.phase ||
      !sameOutcome(previous.outcome, next.outcome))
  )
    return false;
  return (
    next.revision !== previous.revision ||
    (next.phase === previous.phase &&
      next.reason === previous.reason &&
      next.updated_at_ms === previous.updated_at_ms &&
      next.launch_claimed_at_ms === previous.launch_claimed_at_ms &&
      next.completed_at_ms === previous.completed_at_ms &&
      next.observation.target === previous.observation.target &&
      next.observation.status === previous.observation.status &&
      next.observation.reason === previous.observation.reason &&
      next.observation.observed_at_ms === previous.observation.observed_at_ms &&
      sameOutcome(previous.outcome, next.outcome))
  );
}
