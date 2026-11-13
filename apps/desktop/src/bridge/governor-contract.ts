import type {
  GetGovernorSwitchRequest,
  GovernorMutationResult,
  GovernorSwitchReadResult,
  GovernorSwitchState,
  SetGovernorSwitchRequest,
} from "@/bridge/governor.generated";

function object(
  value: unknown,
  required: string[],
  optional: string[] = [],
): value is Record<string, unknown> {
  return (
    value !== null &&
    typeof value === "object" &&
    !Array.isArray(value) &&
    required.every((key) => Object.hasOwn(value, key)) &&
    Object.keys(value).every(
      (key) => required.includes(key) || optional.includes(key),
    )
  );
}
const integer = (value: unknown): value is number =>
  typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
export function governorMutationBase(id: unknown): number | null {
  if (typeof id !== "string" || !/^(0|[1-9][0-9]{0,15})-[0-9a-f]{32}$/.test(id))
    return null;
  const base = Number(id.split("-")[0]);
  return integer(base) ? base : null;
}
export function isGovernorSwitchState(
  value: unknown,
): value is GovernorSwitchState {
  return (
    object(value, ["key", "enabled", "revision"], ["updated_at_ms"]) &&
    value.key === "governor.enabled" &&
    typeof value.enabled === "boolean" &&
    integer(value.revision) &&
    (value.updated_at_ms == null
      ? value.enabled && value.revision === 0
      : integer(value.updated_at_ms))
  );
}
export function sameGovernorState(
  a: GovernorSwitchState,
  b: GovernorSwitchState,
): boolean {
  return (
    a.enabled === b.enabled &&
    a.revision === b.revision &&
    (a.updated_at_ms ?? null) === (b.updated_at_ms ?? null)
  );
}
export function isGovernorQuery(
  value: unknown,
): value is GetGovernorSwitchRequest {
  return (
    object(value, ["key"], ["mutation_id"]) &&
    value.key === "governor.enabled" &&
    (value.mutation_id == null ||
      governorMutationBase(value.mutation_id) !== null)
  );
}
export function isGovernorRequest(
  value: unknown,
): value is SetGovernorSwitchRequest {
  return (
    object(value, ["key", "enabled", "expected_revision", "mutation_id"]) &&
    value.key === "governor.enabled" &&
    typeof value.enabled === "boolean" &&
    integer(value.expected_revision) &&
    governorMutationBase(value.mutation_id) === value.expected_revision
  );
}
export function isGovernorMutationResult(
  value: unknown,
  request: SetGovernorSwitchRequest,
): value is GovernorMutationResult {
  if (
    !isGovernorRequest(request) ||
    !object(
      value,
      ["status", "mutation_id", "switch"],
      ["changed", "replayed", "reason"],
    ) ||
    value.mutation_id !== request.mutation_id ||
    !isGovernorSwitchState(value.switch)
  )
    return false;
  const state = value.switch;
  if (value.status === "COMMITTED")
    return (
      object(value, [
        "status",
        "mutation_id",
        "switch",
        "changed",
        "replayed",
      ]) &&
      typeof value.changed === "boolean" &&
      typeof value.replayed === "boolean" &&
      state.enabled === request.enabled &&
      state.revision === request.expected_revision + Number(value.changed)
    );
  if (!object(value, ["status", "mutation_id", "switch", "reason"]))
    return false;
  if (value.status === "CONFLICT")
    return (
      value.reason === "MUTATION_ID_CONFLICT" ||
      (value.reason === "REVISION_CONFLICT" &&
        state.revision !== request.expected_revision)
    );
  return (
    value.status === "REJECTED" &&
    state.revision === request.expected_revision &&
    (value.reason === "RECEIPT_CAPACITY" ||
      (value.reason === "REVISION_EXHAUSTED" &&
        state.revision === Number.MAX_SAFE_INTEGER &&
        state.enabled !== request.enabled))
  );
}
export function isGovernorReadResult(
  value: unknown,
  query: GetGovernorSwitchRequest,
): value is GovernorSwitchReadResult {
  if (
    !isGovernorQuery(query) ||
    !object(value, ["switch"], ["mutation_result"]) ||
    !isGovernorSwitchState(value.switch)
  )
    return false;
  const result = value.mutation_result;
  if (result == null) return true;
  if (
    !object(result, [
      "status",
      "mutation_id",
      "switch",
      "changed",
      "replayed",
    ]) ||
    !isGovernorSwitchState(result.switch)
  )
    return false;
  const base = governorMutationBase(query.mutation_id);
  return (
    base !== null &&
    result.status === "COMMITTED" &&
    result.replayed === true &&
    isGovernorMutationResult(result, {
      key: "governor.enabled",
      enabled: result.switch.enabled,
      expected_revision: base,
      mutation_id: query.mutation_id as string,
    }) &&
    result.switch.revision <= value.switch.revision &&
    (result.switch.revision !== value.switch.revision ||
      sameGovernorState(result.switch, value.switch))
  );
}
export function boundedGovernorValue(value: unknown): boolean {
  try {
    return new TextEncoder().encode(JSON.stringify(value)).byteLength <= 2048;
  } catch {
    return false;
  }
}
