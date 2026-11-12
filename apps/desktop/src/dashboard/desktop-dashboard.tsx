import {
  ArrowsClockwiseIcon,
  CircleIcon,
  WarningCircleIcon,
} from "@phosphor-icons/react";
import { RecordedTime, type TimeFormat } from "@/components/recorded-time";
import { Button } from "@/components/ui/button";
import { DesktopMeter } from "@/dashboard/desktop-meter";
import type { DashboardSnapshot } from "@/dashboard/desktop-refresh-controller";
import { RecentJobsList } from "@/dashboard/recent-jobs-list";
import { SelectedJobPanel } from "@/dashboard/selected-job-panel";
import {
  desktopErrorMessages,
  desktopMessages as m,
  message,
} from "@/desktop-messages";

interface DesktopDashboardProps extends TimeFormat {
  snapshot: DashboardSnapshot;
  onRefresh(): void;
  onSelect(jobId: string): void;
  visible: boolean;
}
export function DesktopDashboard({
  snapshot,
  onRefresh,
  onSelect,
  visible,
  ...format
}: DesktopDashboardProps) {
  const { connection, health } = snapshot;
  const status =
    connection === "loading"
      ? m.checking
      : connection === "connected"
        ? m.connected
        : connection === "disconnected"
          ? m.disconnected
          : m.unavailable;
  const StatusIcon =
    connection === "connected" ? CircleIcon : WarningCircleIcon;
  return (
    <div className="space-y-5">
      <section
        aria-label={m.dashboard}
        className="rounded-xl border border-border bg-background p-5"
      >
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div className="min-w-0 space-y-2">
            <h2 className="text-xl font-semibold">{m.description}</h2>
            <p
              role="status"
              aria-live="polite"
              className="flex items-center gap-2 font-medium"
            >
              <StatusIcon aria-hidden="true" className="size-4 shrink-0" />
              {status}
            </p>
            {health.error && (
              <p className="text-sm text-destructive">
                {desktopErrorMessages[health.error.code]}
              </p>
            )}
            {health.stale && <p className="text-sm font-medium">{m.stale}</p>}
          </div>
          <Button
            variant="outline"
            className="h-auto min-h-9 max-w-full whitespace-normal"
            onClick={onRefresh}
            disabled={snapshot.refreshing || !snapshot.active}
          >
            <ArrowsClockwiseIcon aria-hidden="true" />
            {snapshot.refreshing
              ? m.refreshing
              : connection === "disconnected" || connection === "unavailable"
                ? m.retry
                : m.refresh}
          </Button>
        </div>
        <dl className="mt-4 grid gap-3 text-sm sm:grid-cols-2">
          <div>
            <dt className="text-muted-foreground">{m.lastCheck}</dt>
            <dd>
              {health.value ? (
                <RecordedTime
                  label={m.lastCheck}
                  milliseconds={health.value.received_at_ms}
                  {...format}
                />
              ) : (
                m.noCheck
              )}
            </dd>
          </div>
          <div>
            <dt className="text-muted-foreground">{m.daemonSample}</dt>
            <dd>{m.unknown}</dd>
          </div>
        </dl>
        {health.value && (
          <p className="mt-3 text-xs text-muted-foreground">
            {message(m.protocol, {
              version: health.value.data.protocol_version,
              sweeper: health.value.data.sweeper,
            })}
          </p>
        )}
        <p className="mt-2 text-xs text-muted-foreground">{m.healthScope}</p>
        {!snapshot.active && <p className="mt-2 text-sm">{m.paused}</p>}
      </section>
      <DesktopMeter section={snapshot.meter} {...format} />
      <p className="text-sm text-muted-foreground">{m.inspectionHelp}</p>
      <div className="grid min-w-0 gap-5 lg:grid-cols-2">
        <RecentJobsList
          snapshot={snapshot}
          onSelect={onSelect}
          visible={visible}
          {...format}
        />
        <SelectedJobPanel snapshot={snapshot} {...format} />
      </div>
    </div>
  );
}
