import { Switch } from "@base-ui/react/switch";
import { useId } from "react";
import { RecordedTime, type TimeFormat } from "@/components/recorded-time";
import { governorMessages as m } from "@/dashboard/governor-messages";
import type { GovernorSection } from "@/dashboard/governor-switch-state";
import { desktopErrorMessages } from "@/desktop-messages";

export function GovernorMasterSwitch({
  section,
  onChange,
  ...format
}: TimeFormat & {
  section: GovernorSection;
  onChange(enabled: boolean): void;
}) {
  const id = useId();
  const confirmed = section.value?.data;
  const unavailable = !confirmed || section.stale || !!section.error;
  const status =
    !confirmed && section.loading
      ? m.checking
      : unavailable
        ? m.unavailable
        : confirmed.enabled
          ? m.active
          : m.paused;
  const notice =
    section.phase === "applying"
      ? section.pending?.enabled
        ? m.applyingResume
        : m.applyingPause
      : section.phase === "uncertain"
        ? m.uncertain
        : section.notice === "rejected"
          ? section.lastIntent
            ? m.resumeRejected
            : m.pauseRejected
          : section.notice === "conflict"
            ? m.conflict
            : section.notice === "unknown_outcome"
              ? m.unknownOutcome
              : "";
  return (
    <section
      aria-labelledby={`${id}-label`}
      className="mt-5 space-y-3 border-t border-border pt-4"
    >
      <div className="flex items-center justify-between gap-4">
        <label id={`${id}-label`} htmlFor={id} className="font-semibold">
          {m.label}
        </label>
        {confirmed ? (
          <Switch.Root
            id={id}
            checked={confirmed.enabled}
            readOnly={!!section.pending || unavailable}
            aria-disabled={!!section.pending || unavailable}
            aria-busy={!!section.pending}
            aria-describedby={`${id}-help ${id}-notice`}
            onCheckedChange={(enabled) => {
              if (!section.pending && !unavailable) onChange(enabled);
            }}
            className="relative inline-flex h-7 w-12 shrink-0 items-center rounded-full border border-border bg-muted px-1 outline-none data-checked:bg-primary focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 aria-disabled:opacity-60"
          >
            <Switch.Thumb className="block size-5 rounded-full bg-background shadow-sm data-checked:translate-x-5" />
          </Switch.Root>
        ) : (
          <span id={id} aria-hidden="true" className="text-muted-foreground">
            …
          </span>
        )}
      </div>
      <p
        role="status"
        aria-live="polite"
        aria-atomic="true"
        className="text-sm font-medium"
      >
        {notice || status}
      </p>
      {notice && <p className="text-sm">{status}</p>}
      <p id={`${id}-help`} className="text-sm text-muted-foreground">
        {confirmed?.enabled === false && !unavailable
          ? m.pausedHelp
          : m.activeHelp}{" "}
        {m.resumeHelp}
      </p>
      <p id={`${id}-notice`} className="text-sm text-destructive">
        {section.notice === "rejected" && section.rejectionReason
          ? m.rejectionReasons[section.rejectionReason]
          : section.error && section.phase !== "uncertain"
            ? desktopErrorMessages[section.error.code]
            : ""}
      </p>
      {confirmed && (
        <div className="text-xs text-muted-foreground">
          {unavailable && (
            <p>
              {m.lastConfirmed}: {confirmed.enabled ? m.active : m.paused}
            </p>
          )}
          <RecordedTime
            label={m.lastCheck}
            milliseconds={section.value?.received_at_ms ?? null}
            {...format}
          />
          <details>
            <summary className="cursor-pointer rounded-sm focus-visible:ring-2 focus-visible:ring-ring">
              {m.diagnostics}
            </summary>
            <p>
              {m.revision}: {confirmed.revision}
            </p>
          </details>
        </div>
      )}
    </section>
  );
}
