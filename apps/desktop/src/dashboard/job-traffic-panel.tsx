import { RecordedTime, type TimeFormat } from "@/components/recorded-time";
import type { SelectedDetail } from "@/dashboard/desktop-refresh-state";
import { governorMessages } from "@/dashboard/governor-messages";
import type { GovernorSection } from "@/dashboard/governor-switch-state";
import { decisionReasonMessages } from "@/dashboard/policy-messages";
import {
  trafficMessages as m,
  trafficClassMessages,
  trafficClassWaitMessages,
  trafficStateMessages,
  trafficWaitMessages,
} from "@/dashboard/traffic-messages";
import type { TrafficSection } from "@/dashboard/traffic-read-state";
import {
  desktopErrorMessages,
  desktopMessages,
  message,
} from "@/desktop-messages";

function trafficStatus(
  section: TrafficSection,
  detail?: SelectedDetail,
  governor?: GovernorSection,
): string {
  const result = section.value?.data,
    admission = result?.admission;
  const job = detail?.value?.data;
  if (!admission) {
    if (result?.classification?.locality === "REMOTE") return m.remote;
    if (
      result?.classification?.uncertainty === "OBSERVE" ||
      result?.classification?.locality === "MIXED"
    )
      return m.bypass;
  }
  // A known native outcome wins over historical waits. Recovery facts cannot
  // be revived by a RUNNING lifecycle snapshot retained from an earlier read.
  if (job?.outcome || admission?.outcome) return m.outcome;
  if (
    job?.phase === "RUNNING" &&
    (!admission ||
      !["ABANDONED", "CANCELLED", "UNCONFIRMED"].includes(admission.state))
  )
    return trafficStateMessages.RUNNING;
  if (admission?.state === "QUEUED") {
    if (
      governor?.value?.data.enabled === false &&
      !governor.stale &&
      !governor.error
    )
      return governorMessages.pausedHelp;
    const reason = admission.wait_reason;
    return reason === "class_window"
      ? (result?.classification &&
          trafficClassWaitMessages[result.classification.classification]) ||
          trafficWaitMessages.class_window ||
          m.queuedUnknown
      : (reason && trafficWaitMessages[reason]) || m.queuedUnknown;
  }
  if (admission) {
    if (
      admission.state === "CANCELLED" &&
      admission.granted_at_ms === null &&
      admission.queue_wait_ms === null &&
      admission.root_job_id === null &&
      !executionUnconfirmed(detail) &&
      job?.phase === "REGISTERED" &&
      job.launch_claimed_at_ms === null
    )
      return m.cancelledBeforeStart;
    return trafficStateMessages[admission.state];
  }
  return m.unavailable;
}

function executionUnconfirmed(detail?: SelectedDetail): boolean {
  return Boolean(
    detail?.value &&
      (detail.stale || detail.error || detail.state === "not_found"),
  );
}

// Excludes timestamps and polling state: identical polls keep the same text in
// the existing polite live region. Only semantic changes are announced.
export function trafficAnnouncement(
  section: TrafficSection,
  detail?: SelectedDetail,
  governor?: GovernorSection,
): string {
  const result = section.value?.data;
  if (!result)
    return section.error?.code === "unsupported"
      ? m.unsupported
      : section.loading
        ? ""
        : m.unavailable;
  const classLabel = result.classification
    ? trafficClassMessages[result.classification.classification]
    : m.classUnknown;
  return `${classLabel}. ${message(m.recorded, { status: trafficStatus(section, detail, governor) })}${section.stale || section.error || section.missing ? ` ${m.stale}` : ""}${executionUnconfirmed(detail) ? ` ${m.executionStale}` : ""}`;
}

interface JobTrafficPanelProps extends TimeFormat {
  section: TrafficSection;
  detail: SelectedDetail;
  governor: GovernorSection;
}
export function JobTrafficPanel({
  section,
  detail,
  governor,
  ...format
}: JobTrafficPanelProps) {
  const result = section.value?.data,
    admission = result?.admission;
  const classification = result?.classification;
  const classLabel = classification
    ? trafficClassMessages[classification.classification]
    : m.classUnknown;
  const limits =
    classification?.locality === "HOST_LOCAL" && result
      ? result.limits[
          classification.classification.toLowerCase() as
            | "build"
            | "test"
            | "lint"
            | "typecheck"
        ]
      : null;
  return (
    <section
      aria-label={m.heading}
      aria-busy={section.loading}
      className="mt-4 space-y-2 text-sm"
    >
      <h3 className="font-semibold">{m.heading}</h3>
      {section.loading && !result && <p>{m.loading}</p>}
      {section.error && (
        <p>
          {section.error.code === "unsupported"
            ? m.unsupported
            : desktopErrorMessages[section.error.code]}
        </p>
      )}
      {!result && !section.loading && !section.error && <p>{m.unavailable}</p>}
      {section.missing && <p>{m.missing}</p>}
      {result && (
        <>
          <dl>
            <dt className="text-muted-foreground">{m.classification}</dt>
            <dd>{classLabel}</dd>
          </dl>
          <p>
            {message(m.recorded, {
              status: trafficStatus(section, detail, governor),
            })}
          </p>
          {(section.stale || section.error || section.missing) && (
            <p>
              {m.retained} {m.stale}
            </p>
          )}
          {executionUnconfirmed(detail) && <p>{m.executionStale}</p>}
          <p className="text-xs text-muted-foreground">{m.recordedHelp}</p>
          <details>
            <summary className="cursor-pointer rounded-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
              {m.details}
            </summary>
            <p className="mt-2 text-xs text-muted-foreground">{m.estimates}</p>
            <dl className="mt-2 space-y-2 break-all text-xs">
              {admission?.last_decision && (
                <div>
                  <dt>
                    {message(m.historicalDecision, {
                      reason:
                        decisionReasonMessages[admission.last_decision.reason],
                    })}
                  </dt>
                  <dd>{admission.last_decision.decided_at_ms}</dd>
                </div>
              )}
              {admission && (
                <>
                  <div>
                    <dt>{m.queueWait}</dt>
                    <dd>
                      {admission.queue_wait_ms ?? desktopMessages.unknown}
                    </dd>
                  </div>
                  <div>
                    <dt>{m.reservation}</dt>
                    <dd>
                      {admission.reservation
                        ? admission.permit_active
                          ? m.permitActive
                          : m.permitReleased
                        : m.noReservation}
                    </dd>
                  </div>
                  {admission.reservation && (
                    <>
                      <div>
                        <dt>{m.root}</dt>
                        <dd>{admission.root_job_id}</dd>
                      </div>
                      <div>
                        <dt>{m.generation}</dt>
                        <dd>{admission.permit_generation}</dd>
                      </div>
                      <div>
                        <dt>{m.cpu}</dt>
                        <dd>{admission.reservation.cpu_millis}</dd>
                      </div>
                      <div>
                        <dt>{m.memory}</dt>
                        <dd>{admission.reservation.memory_bytes}</dd>
                      </div>
                    </>
                  )}
                </>
              )}
              {limits && (
                <div>
                  <dt>{message(m.window, { class: classLabel })}</dt>
                  <dd>{limits.window}</dd>
                </div>
              )}
              <div>
                <dt>{m.sharedCpu}</dt>
                <dd>{result.limits.budget.cpu_millis}</dd>
              </div>
              <div>
                <dt>{m.sharedMemory}</dt>
                <dd>{result.limits.budget.memory_bytes}</dd>
              </div>
              <div>
                <dt>{m.recordedAt}</dt>
                <dd>
                  <RecordedTime
                    label={m.recordedAt}
                    milliseconds={result.recorded_at_ms}
                    {...format}
                  />
                </dd>
              </div>
              <div>
                <dt>{m.receivedAt}</dt>
                <dd>
                  <RecordedTime
                    label={m.receivedAt}
                    milliseconds={section.value?.received_at_ms ?? null}
                    {...format}
                  />
                </dd>
              </div>
            </dl>
          </details>
        </>
      )}
    </section>
  );
}
