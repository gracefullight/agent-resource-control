// Generated from Rust arc-ipc::governor by ts-rs 12.0.1. Do not edit.
// biome-ignore-all format: Preserve reproducible ts-rs declarations.
// Revisions/timestamps are validated integers in 0..=9007199254740991.
// Regenerate: ARC_GOVERNOR_EXPORT=1 cargo test -p arc-ipc --lib governor::typescript::regenerate -- --exact

export type GovernorSwitchKey = "governor.enabled";
export type GovernorSwitchState = { key: GovernorSwitchKey, enabled: boolean, revision: number,
/**
 * Absent only for the default ON/revision-0 state, never a guessed OFF.
 */
updated_at_ms?: number | null, };
export type GetGovernorSwitchRequest = { key: GovernorSwitchKey,
/**
 * Read back the committed receipt after an uncertain acknowledgment.
 * Absence of a receipt is NOT evidence that a write cannot still commit.
 */
mutation_id?: string | null, };
export type SetGovernorSwitchRequest = { key: GovernorSwitchKey, enabled: boolean, expected_revision: number, mutation_id: string, };
export type GovernorConflictReason = "REVISION_CONFLICT" | "MUTATION_ID_CONFLICT";
export type GovernorRejectionReason = "REVISION_EXHAUSTED" | "RECEIPT_CAPACITY";
export type GovernorMutationResult = { "status": "COMMITTED", mutation_id: string, switch: GovernorSwitchState, changed: boolean,
/**
 * Replays contain the original committed state, possibly historical.
 */
replayed: boolean, } | { "status": "CONFLICT", mutation_id: string, switch: GovernorSwitchState, reason: GovernorConflictReason, } | { "status": "REJECTED", mutation_id: string, switch: GovernorSwitchState, reason: GovernorRejectionReason, };
export type GovernorSwitchReadResult = { switch: GovernorSwitchState,
/**
 * If present, the original committed result with replayed=true; the current
 * switch above can be newer. If absent, the request outcome remains unknown.
 */
mutation_result?: GovernorMutationResult | null, };
