import type { MeterMeasurement, MeterSnapshot } from "@/bridge/meter.types";
import { RecordedTime, type TimeFormat } from "@/components/recorded-time";
import {
  type MeterSection,
  meterFieldStale,
  meterFields,
} from "@/dashboard/meter-read-state";
import {
  desktopErrorMessages,
  desktopMessages as m,
  message,
  meterMessages as mm,
} from "@/desktop-messages";

type DisplayUnit = "GiB" | "MiB" | "percent" | "core_seconds" | "level";
export function formatMeterValue(
  value: number | string,
  unit: DisplayUnit,
): string {
  if (typeof value === "string")
    return mm[value as "normal" | "warning" | "critical"];
  const n =
    unit === "GiB" ? value / 2 ** 30 : unit === "MiB" ? value / 2 ** 20 : value;
  const digits = unit === "percent" ? 1 : 2;
  const suffix =
    unit === "percent" ? "%" : unit === "core_seconds" ? " core·s" : ` ${unit}`;
  return `${n > 0 && n < 10 ** -digits ? `<${(10 ** -digits).toFixed(digits)}` : n.toFixed(digits)}${suffix}`;
}
const availability = (field?: MeterMeasurement<unknown>) => {
  switch (field?.reading.availability) {
    case "available":
      return mm.available;
    case "first_interval":
      return mm.waiting;
    case "unsupported":
      return mm.unsupported;
    case "denied":
      return mm.denied;
    case "failed":
      return mm.failed;
    default:
      return mm.loading;
  }
};

interface MeterRowProps extends TimeFormat {
  label: string;
  field?: MeterMeasurement<number | string> | undefined;
  section: MeterSection;
  unit: DisplayUnit;
  observedLabel?: string;
}
function MeterRow({
  label,
  field,
  section,
  unit,
  observedLabel,
  ...format
}: MeterRowProps) {
  const r = field?.reading;
  const known = r?.availability === "available";
  const stale = known && field && meterFieldStale(section, field);
  return (
    <div className="min-w-0 space-y-1">
      <dt className="text-sm text-muted-foreground">{label}</dt>
      <dd className="space-y-1">
        <p className="font-medium">
          {known ? formatMeterValue(r.value, unit) : m.unknown}
        </p>
        <p className="text-xs">
          {field
            ? availability(field)
            : section.error
              ? section.error.code === "unsupported"
                ? mm.unsupported
                : mm.unavailable
              : mm.loading}
          {stale ? ` · ${m.stale}` : ""}
        </p>
        {field && (
          <>
            <p className="text-xs text-muted-foreground">
              {field.provenance === "native" ? mm.native : mm.derived} ·{" "}
              {field.unit}
            </p>
            {known && (
              <details className="text-xs">
                <summary className="w-fit cursor-pointer rounded-sm underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
                  {message(mm.exactValue, { field: label })}
                </summary>
                <span className="break-words font-mono">
                  {r.value} {field.unit}
                </span>
              </details>
            )}
            <RecordedTime
              label={observedLabel ?? message(mm.observed, { field: label })}
              milliseconds={field.observed_at_ms}
              {...format}
            />
          </>
        )}
      </dd>
    </div>
  );
}

interface DesktopMeterProps extends TimeFormat {
  section: MeterSection;
}
export function DesktopMeter({ section, ...format }: DesktopMeterProps) {
  const s = section.value?.data;
  const fields = s ? meterFields(s) : [];
  const partial = fields.some((f) => f.reading.availability !== "available");
  const fieldStale = fields.some(
    (f) =>
      f.reading.availability === "available" && meterFieldStale(section, f),
  );
  const pressure = s?.host.memory_pressure;
  const status = section.error
    ? section.error.code === "unsupported"
      ? mm.unsupportedDaemon
      : mm.unavailable
    : !s
      ? mm.loading
      : section.stale
        ? m.stale
        : fieldStale
          ? mm.someStale
          : mm.current;
  const pressureText =
    pressure?.reading.availability === "available" &&
    !meterFieldStale(section, pressure)
      ? message(mm.pressureStatus, { level: mm[pressure.reading.value] })
      : "";
  const announcement = [
    section.restarted ? mm.restart : "",
    status,
    partial ? mm.partial : "",
    pressureText,
  ]
    .filter(Boolean)
    .join(" · ");
  const row = (
    label: string,
    field: MeterMeasurement<number | string> | undefined,
    unit: DisplayUnit,
    observedLabel?: string,
  ) => (
    <MeterRow
      key={label}
      label={label}
      field={field}
      unit={unit}
      section={section}
      {...(observedLabel ? { observedLabel } : {})}
      {...format}
    />
  );
  const fs = s?.host.filesystem;
  const cpuInterval = s?.host.cpu_interval;
  return (
    <section
      aria-labelledby="metrics-heading"
      className="rounded-xl border border-border p-5"
    >
      <h2 id="metrics-heading" className="font-semibold">
        {m.metrics}
      </h2>
      <p className="mt-1 text-xs text-muted-foreground">{mm.scope}</p>
      <p
        role="status"
        aria-label={m.metrics}
        aria-live="polite"
        className="mt-2 text-sm"
      >
        {announcement}
      </p>
      {section.error && section.error.code !== "unsupported" && (
        <p className="text-xs text-destructive">
          {desktopErrorMessages[section.error.code]}
        </p>
      )}
      {section.loading && s && (
        <p className="text-xs text-muted-foreground">{mm.loading}</p>
      )}
      {s && (
        <div className="mt-3 space-y-2 text-xs text-muted-foreground">
          <p>
            {message(mm.cadence, {
              mode:
                s.cadence.mode === "pressure"
                  ? mm.pressureMode
                  : mm[s.cadence.mode],
              seconds: s.cadence.interval_ms / 1000,
            })}
          </p>
          <dl className="grid gap-3 sm:grid-cols-2">
            <div>
              <dt>{mm.sampled}</dt>
              <dd>
                <RecordedTime
                  label={mm.sampled}
                  milliseconds={s.sampled_at_ms}
                  {...format}
                />
              </dd>
            </div>
            <div>
              <dt>{mm.received}</dt>
              <dd>
                <RecordedTime
                  label={mm.received}
                  milliseconds={section.value?.received_at_ms ?? null}
                  {...format}
                />
              </dd>
            </div>
          </dl>
        </div>
      )}
      <section aria-labelledby="host-meter-heading" className="mt-5">
        <h3 id="host-meter-heading" className="font-medium">
          {mm.host}
        </h3>
        <dl className="mt-3 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
          {row(mm.cpu, s?.host.cpu_utilization, "percent")}
          {row(mm.ram, s?.host.memory_available_bytes, "GiB")}
          {row(mm.pressure, pressure, "level")}
          {row(mm.swap, s?.host.swap_used_bytes, "GiB")}
        </dl>
        {cpuInterval && (
          <div className="mt-3 space-y-1 text-xs text-muted-foreground">
            <p>
              {message(mm.interval, {
                seconds: (cpuInterval.elapsed_ms / 1000).toFixed(3),
                cpus: cpuInterval.logical_cpu_count,
              })}
            </p>
            <p>{mm.denominator}</p>
            <p>{message(mm.elapsed, { ms: cpuInterval.elapsed_ms })}</p>
            <RecordedTime
              label={mm.intervalStart}
              milliseconds={cpuInterval.started_at_ms}
              {...format}
            />
            <RecordedTime
              label={mm.intervalEnd}
              milliseconds={cpuInterval.ended_at_ms}
              {...format}
            />
          </div>
        )}
        <p className="mt-3 text-xs text-muted-foreground">{mm.ramHelp}</p>
        <section aria-labelledby="filesystem-meter-heading" className="mt-4">
          <h4 id="filesystem-meter-heading" className="text-sm font-medium">
            {mm.filesystem}
          </h4>
          <p className="mt-1 break-all text-sm">
            {fs?.identity
              ? `${fs.identity.mount_point} · ${fs.identity.filesystem_type}`
              : m.unknown}
          </p>
          {fs?.identity && <FilesystemDetails snapshot={s} />}
          <dl className="mt-3 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
            {row(mm.capacity, fs?.capacity_bytes, "GiB")}
            {row(mm.free, fs?.free_bytes, "GiB")}
            {row(mm.availableSpace, fs?.available_bytes, "GiB")}
            {row(mm.diskPressure, fs?.pressure, "level")}
          </dl>
          <p className="mt-3 text-xs text-muted-foreground">{mm.diskPolicy}</p>
          <p className="mt-1 text-xs text-muted-foreground">
            {mm.filesystemHelp}
          </p>
        </section>
      </section>
      <section aria-labelledby="daemon-meter-heading" className="mt-5">
        <h3 id="daemon-meter-heading" className="font-medium">
          {mm.daemon}
        </h3>
        <p className="mt-1 text-xs text-muted-foreground">
          {mm.cumulativeHelp}
        </p>
        <p className="text-xs text-muted-foreground">{mm.accounting}</p>
        <dl className="mt-3 grid gap-4 sm:grid-cols-3">
          {row(mm.cumulative, s?.daemon.cpu_core_seconds, "core_seconds")}
          {row(mm.rss, s?.daemon.rss_bytes, "MiB")}
          {row(
            mm.peak,
            s?.daemon.observed_peak_rss_bytes,
            "MiB",
            mm.peakObserved,
          )}
        </dl>
        <p className="mt-3 text-xs text-muted-foreground">{mm.rssHelp}</p>
        <p className="mt-1 text-xs text-muted-foreground">{mm.peakHelp}</p>
        <dl className="mt-2 text-xs">
          <dt>{mm.peakStarted}</dt>
          <dd>
            <RecordedTime
              label={mm.peakStarted}
              milliseconds={s?.daemon.peak_observation_started_at_ms ?? null}
              {...format}
            />
          </dd>
        </dl>
      </section>
      <dl className="mt-5 grid grid-cols-2 gap-4 md:grid-cols-3">
        {[
          m.disk,
          m.reclaimed,
          m.agents,
          m.runningTotal,
          m.queuedTotal,
          m.usage,
        ].map((label) => (
          <div key={label}>
            <dt className="text-sm text-muted-foreground">{label}</dt>
            <dd className="font-medium">{m.unknown}</dd>
          </div>
        ))}
      </dl>
      <p className="mt-2 text-xs text-muted-foreground">{m.metricReason}</p>
      <p className="mt-1 text-xs text-muted-foreground">{m.globalJobsReason}</p>
    </section>
  );
}
function FilesystemDetails({
  snapshot,
}: {
  snapshot: MeterSnapshot | undefined;
}) {
  const fs = snapshot?.host.filesystem.identity;
  if (!fs) return null;
  return (
    <details className="mt-2 text-xs">
      <summary className="w-fit cursor-pointer rounded-sm underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
        {mm.filesystemDetails}
      </summary>
      <dl className="mt-1 space-y-1 break-all">
        {[
          [mm.path, fs.path],
          [mm.mount, fs.mount_point],
          [mm.filesystemType, fs.filesystem_type],
          [mm.volume, fs.volume_id],
        ].map(([label, value]) => (
          <div key={label}>
            <dt>{label}</dt>
            <dd>{value}</dd>
          </div>
        ))}
      </dl>
      <p>{mm.local}</p>
    </details>
  );
}
