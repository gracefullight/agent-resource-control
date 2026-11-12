import type {
  JobMeterReport,
  JobProcessIdentity,
} from "@/bridge/job-meter.types";
import { RecordedTime, type TimeFormat } from "@/components/recorded-time";
import {
  jobMeterMessages as m,
  missingCoverageMessages,
  unknownAccountingMessages,
} from "@/dashboard/job-meter-messages";
import type { JobMeterSection } from "@/dashboard/job-meter-read-state";
import {
  desktopErrorMessages,
  desktopMessages,
  message,
} from "@/desktop-messages";

export function formatJobMeterValue(
  value: number | null,
  unit: "core·s" | "s",
): string {
  if (value === null) return desktopMessages.unknown;
  if (value > 0 && value < 0.001) return `<0.001 ${unit}`;
  return `${new Intl.NumberFormat("en", { maximumFractionDigits: 3 }).format(value)} ${unit}`;
}
function quality(report: JobMeterReport): string {
  const reason = report.missing_coverage
    .map((r) => missingCoverageMessages[r])
    .join("; ");
  return report.completeness === "complete"
    ? m.complete
    : message(
        report.completeness === "partial" ? m.partial : m.coverageUnknown,
        { reason },
      );
}
export function jobMeterAnnouncement(section: JobMeterSection): string {
  const a = section.value?.data.accounting;
  const state =
    a?.state === "recorded"
      ? quality(a.report)
      : a?.state === "unknown"
        ? unknownAccountingMessages[a.reason]
        : a?.state === "missing"
          ? m.missing
          : a?.state === "expired"
            ? m.expired
            : "";
  return state ? message(m.announcement, { state }) : "";
}
const focusClass =
  "w-fit max-w-full cursor-pointer rounded-sm underline decoration-border underline-offset-4 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring";
const exact = (v: number | null, unit: string) =>
  v === null ? desktopMessages.unknown : `${v} ${unit}`;
const process = (p: JobProcessIdentity) =>
  message(m.process, { pid: p.pid, token: p.start_token, boot: p.boot_id });

interface JobMeterPanelProps extends TimeFormat {
  section: JobMeterSection;
}
export function JobMeterPanel({ section, ...format }: JobMeterPanelProps) {
  const s = section.value?.data;
  const accounting = s?.accounting;
  const report = accounting?.state === "recorded" ? accounting.report : null;
  const relation = report?.accounting_relation;
  const reason =
    accounting?.state === "unknown"
      ? unknownAccountingMessages[accounting.reason]
      : null;
  const queue = s?.queue_wait;
  return (
    <section
      aria-labelledby="job-meter-heading"
      aria-busy={section.loading}
      className="mt-5 min-w-0 space-y-3 border-t border-border pt-5"
    >
      <h3 id="job-meter-heading" className="text-base font-semibold">
        {m.title}
      </h3>
      {section.loading && (
        <p className="text-sm">{section.value ? m.refreshing : m.loading}</p>
      )}
      {section.error && (
        <p className="text-sm text-destructive">
          {section.error.code === "unsupported"
            ? m.unsupported
            : `${m.unavailable} · ${desktopErrorMessages[section.error.code]}`}
        </p>
      )}
      {section.state === "missing" && (
        <div className="text-sm">
          <p>{m.missing}</p>
          <p className="text-muted-foreground">{m.missingHelp}</p>
        </div>
      )}
      {section.state === "expired" && <p className="text-sm">{m.expired}</p>}
      {section.stale && (
        <div className="text-sm">
          <p className="font-medium">{m.retained}</p>
          <p>{desktopMessages.stale}</p>
        </div>
      )}
      {!section.loading && !section.error && !accounting && (
        <p className="text-sm text-muted-foreground">{m.unavailable}</p>
      )}
      {reason && <p className="text-sm">{reason}</p>}
      <dl className="grid min-w-0 gap-4 text-sm sm:grid-cols-2">
        <div>
          <dt className="text-muted-foreground">{m.cpu}</dt>
          <dd className="break-words">
            <span className="font-medium">
              {formatJobMeterValue(
                report?.cpu_core_seconds.total ?? null,
                "core·s",
              )}
            </span>
            {report && <p className="mt-1 text-sm">{quality(report)}</p>}
            <p className="mt-1 text-xs text-muted-foreground">{m.cpuHelp}</p>
          </dd>
        </div>
        <div>
          <dt className="text-muted-foreground">{m.run}</dt>
          <dd className="break-words">
            <span className="font-medium">
              {formatJobMeterValue(report?.run_elapsed.seconds ?? null, "s")}
            </span>
            <p className="mt-1 text-xs text-muted-foreground">{m.runHelp}</p>
          </dd>
        </div>
        <div>
          <dt className="text-muted-foreground">{m.queue}</dt>
          <dd className="break-words">
            <span className="font-medium">
              {formatJobMeterValue(queue?.seconds ?? null, "s")}
            </span>
            <p className="mt-1 text-xs text-muted-foreground">{m.queueHelp}</p>
          </dd>
        </div>
        {report && (
          <>
            <div className="min-w-0">
              <dt className="text-muted-foreground">{m.basis}</dt>
              <dd className="break-words">{m.basisHelp}</dd>
            </div>
            <div className="min-w-0 sm:col-span-2">
              <dt className="text-muted-foreground">{m.scope}</dt>
              <dd className="break-words">
                <span className="font-mono">{report.scope}</span>
                <p className="mt-1">{m.scopeHelp}</p>
              </dd>
            </div>
          </>
        )}
      </dl>
      {relation && (
        <p className="break-words text-sm">
          {relation.kind === "unverified"
            ? m.unverified
            : message(
                relation.kind === "included_in_root" ? m.included : m.root,
                { root: relation.root_job_id },
              )}
        </p>
      )}
      {s && (
        <details className="min-w-0 text-sm">
          <summary className={focusClass}>{m.details}</summary>
          <dl className="mt-3 grid min-w-0 gap-3 text-xs sm:grid-cols-2">
            {[
              [m.user, exact(report?.cpu_core_seconds.user ?? null, "core·s")],
              [
                m.system,
                exact(report?.cpu_core_seconds.system ?? null, "core·s"),
              ],
              [
                m.total,
                exact(report?.cpu_core_seconds.total ?? null, "core·s"),
              ],
              [m.exactRun, exact(report?.run_elapsed.seconds ?? null, "s")],
              [m.exactQueue, exact(queue?.seconds ?? null, "s")],
              [m.queueBasis, `${s.queue_wait.basis} / ${s.queue_wait.clock}`],
              ...(report
                ? [
                    [m.basis, report.attribution_basis],
                    [m.report, report.report_id],
                    [m.observer, process(report.observer)],
                    [m.child, process(report.child)],
                  ]
                : []),
            ].map(([label, value]) => (
              <div key={label} className="min-w-0">
                <dt className="text-muted-foreground">{label}</dt>
                <dd className="break-all font-mono">{value}</dd>
              </div>
            ))}
            {report && (
              <div className="sm:col-span-2">
                <dt>{m.terminated}</dt>
                <dd>
                  <RecordedTime
                    label={m.terminated}
                    milliseconds={report.terminated_at_ms}
                    {...format}
                  />
                </dd>
              </div>
            )}
          </dl>
        </details>
      )}
      {section.value && (
        <div className="text-xs text-muted-foreground">
          <p>{m.received}</p>
          <RecordedTime
            label={m.received}
            milliseconds={section.value.received_at_ms}
            {...format}
          />
        </div>
      )}
      {section.lookupReceivedAt !== null &&
        section.lookupReceivedAt !== section.value?.received_at_ms && (
          <div className="text-xs text-muted-foreground">
            <p>{m.lookup}</p>
            <RecordedTime
              label={m.lookup}
              milliseconds={section.lookupReceivedAt}
              {...format}
            />
          </div>
        )}
    </section>
  );
}
