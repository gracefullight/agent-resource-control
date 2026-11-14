import {
  CheckCircleIcon,
  MinusCircleIcon,
  WarningCircleIcon,
  XCircleIcon,
} from "@phosphor-icons/react";
import type { JobDetail } from "@/bridge/types";
import {
  desktopMessages as m,
  message,
  spawnReasonMessages,
} from "@/desktop-messages";

export function outcomeLabel(
  job: Pick<JobDetail, "outcome"> | null | undefined,
): string {
  const outcome = job?.outcome;
  if (!outcome) return m.outcomeUnknown;
  switch (outcome.kind) {
    case "EXITED":
      return outcome.code === 0
        ? m.succeeded
        : message(m.failed, { code: outcome.code });
    case "SIGNALED":
      return message(m.signaled, { signal: outcome.signal });
    case "SPAWN_FAILED":
      return m.spawnFailed;
  }
}

interface RecordedOutcomeProps {
  job: Pick<JobDetail, "outcome">;
}
export function RecordedOutcome({ job }: RecordedOutcomeProps) {
  const outcome = job.outcome;
  const Icon = !outcome
    ? MinusCircleIcon
    : outcome.kind === "EXITED" && outcome.code === 0
      ? CheckCircleIcon
      : outcome.kind === "EXITED"
        ? XCircleIcon
        : WarningCircleIcon;
  return (
    <div className="space-y-2">
      <p className="flex items-center gap-2 text-lg font-semibold">
        <Icon aria-hidden="true" className="size-5 shrink-0" />
        {outcomeLabel(job)}
      </p>
      {!outcome && (
        <p className="text-sm text-muted-foreground">{m.outcomeUnknownHelp}</p>
      )}
      {outcome?.kind === "SIGNALED" && (
        <p className="text-sm">
          {message(m.coreDumped, { value: outcome.core_dumped ? m.yes : m.no })}
        </p>
      )}
      {outcome?.kind === "SPAWN_FAILED" && (
        <dl className="space-y-1 text-sm">
          <div>
            <dt className="text-muted-foreground">{m.spawnReason}</dt>
            <dd>{spawnReasonMessages[outcome.reason]}</dd>
          </div>
          <div>
            <dt className="text-muted-foreground">{m.osCode}</dt>
            <dd>{outcome.os_code ?? m.unknown}</dd>
          </div>
        </dl>
      )}
    </div>
  );
}
