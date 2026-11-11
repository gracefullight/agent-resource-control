import { desktopMessages as m, message } from "@/desktop-messages";

export interface TimeFormat {
  locale?: string;
  timeZone?: string;
}
interface RecordedTimeProps extends TimeFormat {
  label: string;
  milliseconds: number | null;
}

export function readableTime(milliseconds: number, format: TimeFormat): string {
  // Protocol integers may exceed the ECMAScript Date range. Preserve the
  // original number without crashing or inventing a formatted timestamp.
  if (!Number.isFinite(new Date(milliseconds).getTime()))
    return String(milliseconds);
  return new Intl.DateTimeFormat(format.locale ?? "en", {
    dateStyle: "medium",
    timeStyle: "long",
    ...(format.timeZone ? { timeZone: format.timeZone } : {}),
  }).format(milliseconds);
}

export function RecordedTime({
  label,
  milliseconds,
  ...format
}: RecordedTimeProps) {
  if (milliseconds === null) return <span>{m.unknown}</span>;
  const date = new Date(milliseconds);
  const iso = Number.isFinite(date.getTime()) ? date.toISOString() : null;
  return (
    <details className="min-w-0 text-sm">
      <summary
        aria-label={message(m.exactTime, { label })}
        className="w-fit max-w-full cursor-pointer rounded-sm text-muted-foreground underline decoration-border underline-offset-4 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        <time dateTime={iso ?? undefined}>
          {readableTime(milliseconds, format)}
        </time>
      </summary>
      <span className="block break-words pt-1 font-mono text-xs text-muted-foreground">
        {message(m.exactInstant, {
          iso: iso ?? m.metadataUnavailable,
          ms: milliseconds,
        })}
      </span>
    </details>
  );
}
