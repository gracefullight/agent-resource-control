import { resolve, relative } from "node:path";
import { existsSync } from "node:fs";
import { gitBytes } from "../coverage/hygiene";
import { context, validateCommon, requireValue, equal, canonicalBytes, boundJSON, artifact, hash, directory, baseline, baselineTree } from "./exec-fail-open-inputs";
import { parseCargoExecution } from "./managed-exec-test-output";
import { validateAdditionalNative } from "./exec-fail-open-native-extra";
export const scenarios = ["absent", "refused", "disconnect-before-read", "disconnect-register", "stall-register", "partial-register", "committed-register", "disconnect-claim", "stall-claim", "committed-claim", "shared-budget", "busy-register", "busy-claim"];
export function validateInvocationNumbers(c: any) {
  requireValue(c.passed === true && c.invocations === 1 && c.observedExit === 17 && c.recoveryInvocations === 1 && Number.isFinite(c.elapsedToInvocationMs) && c.elapsedToInvocationMs >= 0 && c.elapsedToInvocationMs <= 2000, "failed timing/invocation target");
}
export function validateTiming(timing: any, ctx: ReturnType<typeof context>, root: string, commands: any[]) {
  validateCommon(timing, ctx, "native-timing", "backend");
  requireValue(timing.passed === true && timing.metadataBudgetMs === 1800 && timing.invocationTargetMs === 2000 && typeof timing.clock === "string" && /monotonic|Instant/.test(timing.clock), "wrong metadata deadline/clock");
  requireValue(Array.isArray(timing.cases) && timing.cases.length === scenarios.length && scenarios.every(id => timing.cases.some((c: any) => c.id === id)), "incomplete outage matrix");
  for (const c of timing.cases) {
    validateInvocationNumbers(c);
    const command = boundJSON(root, c.commandReceipt);
    requireValue(commands.some(r => equal(r, command)) && equal(command.argv, ["cargo", "test", "--workspace"]), "timing requires actual current workspace test command");
    requireValue(command.execution.tests.some((t: any) => t.name === c.test && t.status === "ok"), "timing test did not execute");
    const raw = boundJSON(root, c.observation);
    requireValue(raw.scenario === c.id && raw.nativeMacos === true && raw.passed === true && raw.invocations === 1 && raw.recoveryInvocations === 1 && raw.observedExit === 17 && raw.permits === 0 && raw.arcCleanup === false, "native observation incomplete");
    requireValue(raw.test === c.test && raw.recordedUnixMs >= Date.parse(command.started) && raw.recordedUnixMs <= Date.parse(command.ended), "timing predates bound execution");
    requireValue(c.candidateBinary?.sha256 === raw.candidateSha256 && c.candidateBinary.path === relative(root, raw.candidateBinaryPath) && hash(canonicalBytes(root, c.candidateBinary.path)) === raw.candidateSha256, "stale candidate binary");
    requireValue(c.elapsedToInvocationMs === raw.parentSpawnToInvocationMs && equal(c.samples, raw.samples) && raw.samples?.startNs === 0 && raw.samples.endNs > 0 && Math.abs(raw.samples.endNs/1e6 - c.elapsedToInvocationMs) < 0.001 && c.boundaries === raw.boundary, "timing clock/boundary mismatch");
    for (const field of ["metadataElapsedMs", "handoffMs", "startupMs"]) requireValue(c[field] === null && typeof c.subdivisionReason === "string" && c.subdivisionReason.length > 10, "unobserved subdivisions must be null with reason");
    requireValue(equal(c.attempts, { counts: raw.counts, events: raw.events, clock: raw.proxyClock }) && typeof c.boundaries === "string" && /parent.*wrapper.*handshake/.test(c.boundaries), "missing attempt/boundary evidence");
    requireValue(Array.isArray(raw.events) && raw.events.every((e: any, i: number) => e.order === i && Number.isSafeInteger(e.relativeNs) && e.relativeNs >= (raw.events[i-1]?.relativeNs ?? 0) && typeof e.operation === "string" && typeof e.event === "string" && ["prefix-peeked", "request-body-read"].includes(e.sendObservation) && e.metadataDeadline === null && e.deadlineReason.length > 10), "proxy event order/send history missing");
    if (c.id.startsWith("committed-")) {
      const action = c.id === "committed-register" ? "REGISTER" : "CLAIM";
      requireValue(raw.events.filter((e: any) => e.operation === action && e.event === "backend-ack-withheld").length === 3 && raw.counts[`${action}-committed`] === 3 && raw.commitRows.length === 1 && raw.commitRows[0].revision >= (action === "CLAIM" ? 2 : 1) && raw.commitRows[0].claimed === (action === "CLAIM") && raw.commitRows[0].childPid === null && typeof raw.commitRows[0].phase === "string" && raw.commitRows[0].phase.length > 0 && raw.commitRows[0].reason === (action === "CLAIM" ? "CLAIMED_NOT_ATTACHED" : "REGISTERED_NOT_CLAIMED") && equal(c.commitProof, raw.commitRows), "real committed lost-ack proof missing");
    }
  }
  validateAdditionalNative(root, timing, commands);
  return timing;
}
export function validateHistorical(h: any, ctx: ReturnType<typeof context>, root: string, commands: any[]) {
  validateCommon(h, ctx, "historical-fixture", "backend");
  requireValue(h.passed === true && h.baselineHead === baseline && h.baselineTree === baselineTree, "wrong historical baseline");
  const prep = JSON.parse(canonicalBytes(root, "target/exec-fail-open/history/preparation.json").toString());
  requireValue(equal(h.preparation, prep) && prep.exit === 0 && prep.baselineHead === baseline && prep.baselineTree === baselineTree && prep.timeoutAllowanceMs >= 3600000 && prep.elapsedMs >= 0, "missing historical preparation");
  const target = resolve(root, "target/exec-fail-open/history/target");
  requireValue(equal(prep.argv, ["cargo", "test", "-p", "arc-cli", "--locked", "--offline", "--target-dir", target]) && prep.cwd === "target/exec-fail-open/history/source", "substitute historical preparation command/cwd");
  const stdout = canonicalBytes(root, "target/exec-fail-open/history/stdout.log"), stderr = canonicalBytes(root, "target/exec-fail-open/history/stderr.log");
  const parsed = parseCargoExecution(stdout.toString());
  requireValue(hash(stdout) === prep.stdoutSha256 && hash(stderr) === prep.stderrSha256 && parsed.ignored === 0 && equal(parsed, prep.execution) && equal(h.fixtureInputs, { files: prep.files, identity: prep.identity, tools: prep.tools }), "historical test/source inventory differs");
  const expectedPaths = gitBytes(root, ["ls-tree", "-r", "--name-only", baseline, "--", "crates", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "mise.toml", ".cargo"]).toString().trim().split("\n").sort();
  requireValue(equal(Object.keys(prep.files).sort(), expectedPaths) && equal(prep.tools, [ctx.tools.cargo.version, ctx.tools.rustc.version]) && prep.identity === hash(JSON.stringify({ baseline, baselineTree, files: prep.files, tools: prep.tools })) && prep.pid > 0 && h.binary.path === "target/exec-fail-open/history/target/debug/arcctl", "historical fixture/toolchain inventory incomplete");
  for (const [path, sha] of Object.entries(prep.files)) requireValue(hash(gitBytes(root, ["show", `${baseline}:${path}`])) === sha && hash(canonicalBytes(root, `target/exec-fail-open/history/source/${path}`)) === sha, "historical fixture source mismatch");
  requireValue(equal(h.binary, prep.binary) && hash(canonicalBytes(root, h.binary.path)) === h.binary.sha256, "historical executable changed");
  requireValue(Array.isArray(h.invocations) && h.invocations.length === 2, "exactly two historical invocations required");
  const expected = ["retries::cli_has_one_child_under_before_after_commit_reply_loss_and_retains_exit17", "cli::spawn_failure_quick_exit_help_and_status_have_static_diagnostics"];
  const routeLines = canonicalBytes(root, "target/exec-fail-open/history/routes.jsonl").toString().trim().split("\n").map(s => JSON.parse(s));
  for (const [i, site] of ["before-register", "after-shutdown"].entries()) {
    const r = h.invocations.find((r: any) => r.site === site);
    requireValue(r && r.test === expected[i] && r.program === resolve(root, h.binary.path) && r.argc === (site === "after-shutdown" ? 0 : 3) && r.programSha256 === h.binary.sha256 && routeLines.some(raw => equal(raw, r)), "historical route not observed");
    requireValue(commands.some(c => equal(c.argv, ["cargo", "test", "--workspace"]) && c.execution.tests.some((t: any) => t.name === r.test && t.status === "ok") && r.recordedUnixMs >= Date.parse(c.started) && r.recordedUnixMs <= Date.parse(c.ended)), "historical assertions not executed in current workspace command");
  }
  const neg = boundJSON(root, h.negativeControl);
  requireValue(neg.expectedFailure === true && neg.historicalExit === 1 && neg.invocations === 0 && neg.passed === true && neg.test === "fail_open::historical_arcctl_fails_live_fallback_assertions", "missing expected historical negative control");
  requireValue(commands.some(c => equal(c.argv, ["cargo", "test", "--workspace"]) && c.execution?.tests.some((t: any) => t.name === neg.test && t.status === "ok") && neg.recordedUnixMs >= Date.parse(c.started) && neg.recordedUnixMs <= Date.parse(c.ended)) && neg.nativeMacos === true, "negative control stale or unexecuted");
  requireValue(h.liveRoutes?.candidateSha256 && h.liveRoutes.allOtherCasesUseCandidate === true && existsSync(resolve(root, h.liveRoutes.candidatePath)) && hash(canonicalBytes(root, h.liveRoutes.candidatePath)) === h.liveRoutes.candidateSha256, "missing live candidate routing");
  return h;
}
