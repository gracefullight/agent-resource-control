import { useEffect, useRef, useState } from "react";
import type { JobId } from "@/bridge/types";
import {
  RecordedTime,
  readableTime,
  type TimeFormat,
} from "@/components/recorded-time";
import type { DashboardSnapshot } from "@/dashboard/desktop-refresh-controller";
import { outcomeLabel } from "@/dashboard/recorded-outcome";
import {
  desktopErrorMessages,
  desktopMessages as m,
  message,
  phaseMessages,
} from "@/desktop-messages";
import { cn } from "@/lib/merge-classes";

interface RecentJobsListProps extends TimeFormat {
  snapshot: DashboardSnapshot;
  onSelect(jobId: JobId): void;
  visible: boolean;
}
export function RecentJobsList({
  snapshot,
  onSelect,
  visible,
  ...format
}: RecentJobsListProps) {
  const { recent, selectedId, detail } = snapshot;
  const jobs = recent.value?.data.jobs ?? [];
  const [focusedId, setFocusedId] = useState<JobId | null>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const previouslyVisible = useRef(visible);
  const lastFocusedOption = useRef<HTMLElement | null>(null);
  const tabStop =
    jobs.find((job) => job.job_id === focusedId)?.job_id ??
    jobs.find((job) => job.job_id === selectedId)?.job_id ??
    jobs[0]?.job_id;

  // Restore the previous selection on navigation return, never on each poll.
  useEffect(() => {
    if (visible && !previouslyVisible.current && selectedId) {
      const target = listRef.current?.querySelector<HTMLElement>(
        `[data-job-id="${selectedId}"]`,
      );
      if (target) target.focus();
      else document.getElementById("selected-heading")?.focus();
    }
    previouslyVisible.current = visible;
  }, [visible, selectedId]);

  useEffect(() => {
    const previous = lastFocusedOption.current;
    if (
      visible &&
      previous &&
      !previous.isConnected &&
      !jobs.some((job) => job.job_id === previous.dataset.jobId)
    ) {
      if (document.activeElement === document.body)
        document.getElementById("selected-heading")?.focus();
      lastFocusedOption.current = null;
    }
  }, [visible, jobs]);

  function moveFocus(index: number) {
    const job = jobs[index];
    if (!job) return;
    setFocusedId(job.job_id);
    listRef.current
      ?.querySelector<HTMLElement>(`[data-job-id="${job.job_id}"]`)
      ?.focus();
  }

  return (
    <section
      aria-labelledby="recent-heading"
      aria-busy={recent.loading}
      className="min-w-0 rounded-xl border border-border bg-background p-5"
    >
      <h2 id="recent-heading" className="text-lg font-semibold">
        {m.recent}
      </h2>
      <p
        id="recent-keyboard-help"
        className="mt-1 text-sm text-muted-foreground"
      >
        {m.selectionHelp}
      </p>
      {recent.loading && !recent.value && (
        <p className="mt-4">{m.recentLoading}</p>
      )}
      {!recent.value && !recent.loading && !recent.error && (
        <p className="mt-4">{m.recentLoading}</p>
      )}
      {recent.error && (
        <p className="mt-3 text-sm text-destructive">
          {m.unavailable} · {desktopErrorMessages[recent.error.code]}
        </p>
      )}
      {recent.stale && <p className="mt-3 text-sm font-medium">{m.stale}</p>}
      {recent.value && (
        <>
          <p className="mt-3 text-sm text-muted-foreground">
            {message(m.recentCount, { count: jobs.length })}
          </p>
          {recent.value.data.truncated && (
            <p className="text-sm text-muted-foreground">{m.truncated}</p>
          )}
          <p className="mt-2 text-xs text-muted-foreground">{m.lastKnown}</p>
          <div className="mt-3 space-y-1 text-sm">
            <span className="text-muted-foreground">{m.listObserved}</span>
            <RecordedTime
              label={m.listObserved}
              milliseconds={recent.value.data.observed_at_ms}
              {...format}
            />
            <span className="block pt-1 text-muted-foreground">
              {m.received}
            </span>
            <RecordedTime
              label={m.received}
              milliseconds={recent.value.received_at_ms}
              {...format}
            />
          </div>
          {jobs.length === 0 && (
            <div className="mt-5 space-y-1">
              <p>{m.empty}</p>
              <p className="text-sm text-muted-foreground">{m.emptyHelp}</p>
            </div>
          )}
          {jobs.length > 0 && (
            <div
              ref={listRef}
              role="listbox"
              aria-label={m.recentList}
              aria-describedby="recent-keyboard-help"
              className="mt-4 space-y-1"
            >
              {jobs.map((job, index) => {
                const selected = selectedId === job.job_id;
                const known =
                  detail.value?.data.job_id === job.job_id
                    ? detail.value.data
                    : null;
                const result = outcomeLabel(known);
                return (
                  <div
                    key={job.job_id}
                    role="option"
                    aria-selected={selected}
                    aria-label={message(m.rowName, {
                      id: job.job_id,
                      phase: phaseMessages[job.phase],
                      outcome: result,
                      time: readableTime(job.updated_at_ms, format),
                    })}
                    tabIndex={tabStop === job.job_id ? 0 : -1}
                    data-job-id={job.job_id}
                    onFocus={(event) => {
                      lastFocusedOption.current = event.currentTarget;
                      setFocusedId(job.job_id);
                    }}
                    onClick={() => onSelect(job.job_id)}
                    onKeyDown={(event) => {
                      const destination =
                        event.key === "ArrowDown"
                          ? (index + 1) % jobs.length
                          : event.key === "ArrowUp"
                            ? (index + jobs.length - 1) % jobs.length
                            : event.key === "Home"
                              ? 0
                              : event.key === "End"
                                ? jobs.length - 1
                                : null;
                      if (destination !== null) {
                        event.preventDefault();
                        moveFocus(destination);
                      } else if (event.key === "Enter" || event.key === " ") {
                        event.preventDefault();
                        onSelect(job.job_id);
                      }
                    }}
                    className={cn(
                      "cursor-pointer rounded-lg border border-transparent p-3 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                      selected
                        ? "border-border bg-secondary"
                        : "hover:bg-muted",
                    )}
                  >
                    <span className="block break-all font-mono text-xs">
                      {job.job_id}
                    </span>
                    <span className="mt-1 block font-medium">
                      {phaseMessages[job.phase]}
                    </span>
                    <span className="block">{result}</span>
                    {known &&
                      (detail.stale ||
                        known.updated_at_ms < job.updated_at_ms) && (
                        <span className="block text-xs">{m.stale}</span>
                      )}
                    <span className="mt-1 block text-xs text-muted-foreground">
                      {message(m.labelledTime, {
                        label: m.updated,
                        time: readableTime(job.updated_at_ms, format),
                      })}
                    </span>
                  </div>
                );
              })}
            </div>
          )}
        </>
      )}
    </section>
  );
}
