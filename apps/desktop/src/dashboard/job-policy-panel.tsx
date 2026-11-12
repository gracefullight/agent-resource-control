import type { PolicyObservation } from "@/bridge/policy.types";
import {
  decisionReasonMessages,
  policyMessages as m,
  policyReasonMessages,
} from "@/dashboard/policy-messages";
import type { PolicySection } from "@/dashboard/policy-read-state";
import { desktopErrorMessages } from "@/desktop-messages";

export function policyAnnouncement(
  section: PolicySection,
  terminal = false,
): string {
  if (section.error?.code === "unsupported") return m.unsupported;
  if (section.error)
    return `${section.value ? m.retained : m.unknown} ${desktopErrorMessages[section.error.code]}`;
  const effective = section.value?.data,
    decision = section.job?.data.last_decision;
  if (!effective) return m.unknown;
  if (section.stale) return m.stale;
  if (
    !terminal &&
    decision?.reason === "memory_pressure" &&
    effective.mode === "memory_hold"
  )
    return m.waiting;
  return `${effective.mode === "memory_hold" ? m.held : m.l0} · ${policyReasonMessages[effective.reason]}`;
}
function ObservationDetails({
  observation,
  age,
  retained = false,
}: {
  observation: PolicyObservation;
  age: number | null;
  retained?: boolean;
}) {
  return (
    <dl className="space-y-1 break-all text-xs">
      <div>
        <dt>
          {retained
            ? "Retained observation availability"
            : "Observation availability"}
        </dt>
        <dd>{observation.availability}</dd>
      </div>
      <div>
        <dt>{retained ? "Retained pressure" : "Pressure"}</dt>
        <dd>{observation.pressure ?? "Unknown"}</dd>
      </div>
      <div>
        <dt>Observation age (ms)</dt>
        <dd>{age ?? "Unknown"}</dd>
      </div>
      <div>
        <dt>Freshness limit (ms)</dt>
        <dd>{observation.stale_after_ms ?? "Unknown"}</dd>
      </div>
      <div>
        <dt>Observed monotonic (ms)</dt>
        <dd>{observation.observed_monotonic_ms ?? "Unknown"}</dd>
      </div>
      <div>
        <dt>Observed Unix (ms)</dt>
        <dd>{observation.observed_at_ms ?? "Unknown"}</dd>
      </div>
      <div>
        <dt>Sampler identity</dt>
        <dd>
          {observation.sampler
            ? JSON.stringify(observation.sampler)
            : "Unknown"}
        </dd>
      </div>
      <div>
        <dt>Sequence</dt>
        <dd>{observation.sequence ?? "Unknown"}</dd>
      </div>
    </dl>
  );
}
export function JobPolicyPanel({
  section,
  terminal = false,
}: {
  section: PolicySection;
  terminal?: boolean;
}) {
  const effective = section.value?.data,
    result = section.job?.data,
    decision = result?.last_decision;
  const confirmedHold =
    !terminal &&
    !section.error &&
    !section.stale &&
    effective?.mode === "memory_hold" &&
    decision?.reason === "memory_pressure";
  const retained = section.stale || !!section.error;
  return (
    <section aria-label={m.heading} className="mt-4 space-y-2 text-sm">
      <h3 className="font-semibold">{m.heading}</h3>
      <p>{policyAnnouncement(section, terminal)}</p>
      {confirmedHold && <p>{m.recovery}</p>}
      {decision?.reason === "memory_pressure" && !confirmedHold && (
        <p>{m.historicalHold}</p>
      )}
      {result && !decision && (
        <p>Job decision: {result.decision_availability.replaceAll("_", " ")}</p>
      )}
      {decision && decision.reason !== "memory_pressure" && (
        <p>
          Last recorded job decision: {decisionReasonMessages[decision.reason]}
        </p>
      )}
      {effective && (
        <>
          <p className="break-all text-xs text-muted-foreground">
            {retained ? "Last received policy" : "Effective policy"}:{" "}
            {effective.policy_version} · {effective.profile}
          </p>
          <details>
            <summary className="cursor-pointer rounded-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
              {m.details}
            </summary>
            <p>
              {retained ? "Last received mode" : "Effective mode"}:{" "}
              {effective.mode}
            </p>
            <p>
              {retained
                ? `Last received reason: ${effective.reason} (current admission unconfirmed)`
                : `Effective reason: ${policyReasonMessages[effective.reason]}`}
            </p>
            <ObservationDetails
              observation={effective.observation}
              age={section.ageMs}
              retained={retained}
            />
            {decision && (
              <div className="mt-3">
                <p>
                  Recorded job decision · {decision.policy_version} · Unix ms{" "}
                  {decision.decided_at_ms}
                </p>
                <ObservationDetails
                  observation={decision.observation}
                  age={decision.observation.age_ms}
                />
              </div>
            )}
          </details>
        </>
      )}
    </section>
  );
}
