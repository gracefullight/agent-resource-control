//! Authenticate the access-denial and failed-target observations selected by timing.
import { relative } from "node:path";
import { boundJSON, canonicalBytes, directory, equal, hash, requireValue } from "./exec-fail-open-inputs";

export const accessCases = [
  ["connect-access-before-register", "connect_access_denied_before_register_runs_once", [], false],
  ["connect-access-register-ack-loss", "connect_access_denied_after_register_ack_loss_stays_uncertain", ["REGISTER"], true],
  ["connect-access-before-claim", "connect_access_denied_before_claim_stays_uncertain", ["REGISTER"], false],
  ["connect-access-claim-ack-loss", "connect_access_denied_after_claim_ack_loss_stays_uncertain", ["REGISTER", "CLAIM"], true],
] as const;

export function validateAccessRecord(id: string, row: any, selected: any) {
  const spec = accessCases.find(c => c[0] === id);
  requireValue(spec && equal(row, selected) && row.scenario === id && row.test === `connect_access::${spec[1]}`, "access observation differs from selected bytes");
  requireValue(row.nativeMacos === true && row.passed === true && row.invocations === 1 && row.observedExit === 17 && row.recoveryInvocations === 1 && row.permits === 0 && row.arcCleanup === false, "access invocation evidence differs");
  requireValue(Number.isFinite(row.parentSpawnToInvocationMs) && row.parentSpawnToInvocationMs >= 0 && row.parentSpawnToInvocationMs <= 2000 && row.samples?.clock === "std::time::Instant" && row.samples.startNs === 0 && Math.abs(row.samples.endNs / 1e6 - row.parentSpawnToInvocationMs) < 0.001 && /parent.*wrapper.*handshake/.test(row.boundary), "access timing differs");
  requireValue(["metadataElapsedMs", "handoffMs", "startupMs"].every(field => Object.hasOwn(row, field) && row[field] === null) && typeof row.subdivisionReason === "string" && row.subdivisionReason.length > 10 && /not measured/i.test(row.subdivisionReason), "access unmeasured subdivisions lack nulls or reason");
  requireValue(Number.isSafeInteger(row.wrapperPid) && row.wrapperPid > 0 && row.childParentPid === row.wrapperPid && Number.isSafeInteger(row.childPid) && row.childPid > 0, "access child ownership differs");
  requireValue(row.proxy?.nativeErrno === 13 && equal(row.proxy.actions, spec[2]) && row.proxy.commits?.length === spec[2].length && (spec[2].length === 0 || row.proxy.ackWithheld === spec[3]), "access proxy evidence differs");
  for (const [i, action] of spec[2].entries()) {
    const commit = row.proxy.commits[i];
    requireValue(commit && Object.keys(commit).length === 4 && commit.action === action && commit.revision === i + 1 && commit.claimed === (action === "CLAIM") && commit.childAttached === false, "access commit evidence differs");
  }
}

export function validateTerminalRecord(row: any, selected: any) {
  requireValue(equal(row, selected) && row.test === "fallback_terminal::native_pty_spawn_failure_restores_foreground_without_child_output" && row.scenario === "terminal" && row.nativeMacos === true && row.passed === true && row.invocations === 0, "failed target invocation evidence differs");
}

export function validateAdditionalNative(root: string, timing: any, commands: any[]) {
  const workspace = commands.find(c => equal(c.argv, ["cargo", "test", "--workspace"]));
  requireValue(workspace && Array.isArray(timing.accessCases) && timing.accessCases.length === accessCases.length && accessCases.every(([id]) => timing.accessCases.filter((c: any) => c.id === id).length === 1), "access timing matrix incomplete");
  const checkBinding = (item: any, path: string) => {
    const command = boundJSON(root, item.commandReceipt), row = boundJSON(root, item.observation);
    requireValue(equal(command, workspace) && item.observation.selector?.path === `${directory}/implementation/native/${path}.json`, "native selected observation/command differs");
    requireValue(command.execution.tests.some((t: any) => t.name === row.test && t.status === "ok") && row.recordedUnixMs >= Date.parse(command.started) && row.recordedUnixMs <= Date.parse(command.ended), "native observation predates test");
    const binary = relative(root, row.candidateBinaryPath);
    requireValue(hash(canonicalBytes(root, binary)) === row.candidateSha256, "native candidate changed");
    return row;
  };
  for (const c of timing.accessCases) validateAccessRecord(c.id, c.record, checkBinding(c, c.id));
  const terminal = timing.failedTarget;
  requireValue(terminal, "failed target observation missing");
  validateTerminalRecord(terminal.record, checkBinding(terminal, "terminal-2"));
}
