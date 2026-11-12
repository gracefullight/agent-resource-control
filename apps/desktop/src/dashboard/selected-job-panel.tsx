import { RecordedTime, type TimeFormat } from "@/components/recorded-time";
import type { DashboardSnapshot } from "@/dashboard/desktop-refresh-controller";
import {
  JobMeterPanel,
  jobMeterAnnouncement,
} from "@/dashboard/job-meter-panel";
import {
  JobPolicyPanel,
  policyAnnouncement,
} from "@/dashboard/job-policy-panel";
import { outcomeLabel, RecordedOutcome } from "@/dashboard/recorded-outcome";
import {
  desktopErrorMessages,
  desktopMessages as m,
  message,
  observationMessages,
  phaseMessages,
  reasonMessages,
} from "@/desktop-messages";

interface SelectedJobPanelProps extends TimeFormat {
  snapshot: DashboardSnapshot;
}
export function SelectedJobPanel({
  snapshot,
  ...format
}: SelectedJobPanelProps) {
  const { selectedId, detail, recent } = snapshot;
  const job = detail.value?.data;
  const outsideRecent =
    selectedId &&
    recent.value &&
    !recent.value.data.jobs.some((row) => row.job_id === selectedId);
  return (
    <section
      aria-labelledby="selected-heading"
      aria-busy={detail.loading}
      className="min-w-0 rounded-xl border border-border bg-background p-5"
    >
      <h2
        id="selected-heading"
        tabIndex={-1}
        className="rounded-sm text-lg font-semibold focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        {m.selected}
      </h2>
      {!selectedId && (
        <p className="mt-4 text-muted-foreground">{m.selectHelp}</p>
      )}
      {selectedId && (
        <p className="mt-2 break-all font-mono text-sm">{selectedId}</p>
      )}
      {outsideRecent && (
        <p className="mt-2 text-sm text-muted-foreground">{m.outsideRecent}</p>
      )}
      {detail.loading && <p className="mt-4 text-sm">{m.detailLoading}</p>}
      {detail.error && (
        <p className="mt-3 text-sm text-destructive">
          {m.unavailable} · {desktopErrorMessages[detail.error.code]}
        </p>
      )}
      {detail.state === "not_found" && (
        <div className="mt-4 space-y-1">
          <p className="font-medium">{m.notFound}</p>
          <p className="text-sm text-muted-foreground">{m.notFoundHelp}</p>
        </div>
      )}
      {selectedId &&
        !job &&
        !detail.loading &&
        detail.state !== "not_found" && (
          <p className="mt-4">{m.outcomeUnknown}</p>
        )}
      {detail.stale && <p className="mt-4 text-sm font-medium">{m.stale}</p>}
      <JobPolicyPanel
        section={snapshot.policy}
        terminal={Boolean(
          job && (job.outcome !== null || job.phase === "RUNNING"),
        )}
      />
      {job && detail.value && (
        <div className="mt-4 space-y-5">
          {detail.state === "not_found" && (
            <h3 className="text-sm font-semibold">{m.retainedDetail}</h3>
          )}
          <div>
            <h3 className="mb-2 text-sm text-muted-foreground">{m.outcome}</h3>
            <RecordedOutcome job={job} />
          </div>
          <p className="text-xs text-muted-foreground">{m.lastKnown}</p>
          <dl className="grid min-w-0 gap-4 text-sm sm:grid-cols-2">
            <div>
              <dt className="text-muted-foreground">{m.phase}</dt>
              <dd>{phaseMessages[job.phase]}</dd>
            </div>
            <div>
              <dt className="text-muted-foreground">{m.revision}</dt>
              <dd>{job.revision}</dd>
            </div>
            <div className="sm:col-span-2">
              <dt className="text-muted-foreground">{m.reason}</dt>
              <dd>{reasonMessages[job.reason]}</dd>
            </div>
            <div className="sm:col-span-2">
              <dt className="text-muted-foreground">{m.identity}</dt>
              <dd>
                {message(m.identityValue, {
                  target: observationMessages[job.observation.target],
                  status: observationMessages[job.observation.status],
                })}
              </dd>
            </div>
            {(
              [
                [m.registered, job.registered_at_ms],
                [m.launchClaimed, job.launch_claimed_at_ms],
                [m.completed, job.completed_at_ms],
                [m.updated, job.updated_at_ms],
                [m.observed, job.observation.observed_at_ms],
                [m.received, detail.value.received_at_ms],
              ] as const
            ).map(([label, time]) => (
              <div key={label}>
                <dt className="mb-1 text-muted-foreground">{label}</dt>
                <dd>
                  <RecordedTime label={label} milliseconds={time} {...format} />
                </dd>
              </div>
            ))}
          </dl>
        </div>
      )}
      {detail.state === "not_found" && detail.lookupReceivedAt !== null && (
        <div className="mt-4 text-sm">
          <p>{m.lookupReceived}</p>
          <RecordedTime
            label={m.lookupReceived}
            milliseconds={detail.lookupReceivedAt}
            {...format}
          />
        </div>
      )}
      {selectedId && (
        <JobMeterPanel
          key={selectedId}
          section={snapshot.jobMeter}
          {...format}
        />
      )}
      <p role="status" aria-live="polite" className="sr-only">
        {selectedId && job
          ? message(m.selectedAnnouncement, {
              id: selectedId,
              outcome: outcomeLabel(job),
            })
          : ""}
        {selectedId && snapshot.jobMeter.value
          ? ` ${jobMeterAnnouncement(snapshot.jobMeter)}`
          : ""}
        {job ||
        snapshot.policy.job?.data.job_id === selectedId ||
        (!selectedId && (snapshot.policy.value || snapshot.policy.error))
          ? ` ${policyAnnouncement(snapshot.policy, Boolean(job && (job.outcome !== null || job.phase === "RUNNING")))}`
          : ""}
      </p>
    </section>
  );
}
