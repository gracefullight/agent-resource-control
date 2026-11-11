import { reachableEvidence } from "./exec-fail-open-traversal";
import { predecessorReads } from "./exec-fail-open-reads";
import { productionRecipe, type EvidenceRecipe } from "./exec-fail-open-recipe";
import { requireValue, equal, boundJSON, reference, directory, canonicalBytes, hash, baseline, baselineTree } from "./exec-fail-open-inputs";
import { rejectFutureReferences, predecessorBindings } from "./exec-fail-open-graph";
import { CRATES, HISTORICAL_CRATES } from "../../coverage/config";
import { canonicalArchive, attempt13Path, attempt13Sha256, persistent4Path, persistent4Sha256 } from "./exec-fail-open-compact";
import { commandOutput } from "./exec-fail-open-receipts";
import { readFileSync, realpathSync } from "node:fs";
export function validateFloors(crates: any[], inventory: "current" | "historical-fixture" = "current") {
  requireValue(inventory === "current" || inventory === "historical-fixture", "invalid coverage inventory");
  const expected = inventory === "current" ? CRATES : HISTORICAL_CRATES;
  requireValue(Array.isArray(crates) && equal(crates.map((c: any) => c.crate), [...expected]) && crates.every((c: any) => Number.isSafeInteger(c.total) && Number.isSafeInteger(c.covered) && c.total > 0 && c.covered >= 0 && c.covered <= c.total && BigInt(c.covered)*100n >= BigInt(c.total)*80n), "crate coverage below 80 percent or missing");
}
export function validateFailedAttempt(root: string, recipe: EvidenceRecipe, ctx: any, receipt: any) {
  requireValue(Number.isSafeInteger(receipt.exit) && receipt.exit !== 0 && receipt.status === "failed", "attempt is not a failed command");
  recipe.common(receipt, ctx, "command", "backend");
  requireValue(receipt.skill === "oma-backend" && receipt.sourceHashAfter === ctx.sourceHash && equal(receipt.observedGitAfter, receipt.observedGit) &&
    (!receipt.fixtureOnly || equal(receipt.observedGit, ctx.observedGit)) && equal(receipt.host, ctx.host) && equal(receipt.tools, ctx.tools), "attempt provenance differs");
  requireValue(receipt.cwd === "." && Array.isArray(receipt.argv) && receipt.argv.length > 0 && receipt.argv.every((a: unknown) => typeof a === "string") && receipt.skipFlags === false && !receipt.argv.some((a: string) => /^--(?:skip|ignored|no-run|list)(?:=|$)/.test(a)), "invalid attempt invocation");
  requireValue(Number.isSafeInteger(receipt.pid) && receipt.pid > 0 && Number.isFinite(receipt.elapsedMs) && receipt.elapsedMs >= 0 && Number.isSafeInteger(receipt.timeoutAllowanceMs) && receipt.timeoutAllowanceMs >= 3600000 && Date.parse(receipt.ended) >= Date.parse(receipt.started), "invalid attempt process/timing");
  const executable = Bun.which(receipt.argv[0]);
  requireValue(executable && receipt.executable?.path === realpathSync(executable) && receipt.executable.sha256 === hash(readFileSync(executable)), "attempt executable differs");
  commandOutput(root, receipt);
  if (receipt.fixtureOnly) {
    const process = boundJSON(root, receipt.processObservation);
    requireValue(equal(process, { fixtureOnly: true, pid: receipt.pid, exit: receipt.exit, stdoutSha256: receipt.stdoutSha256, stderrSha256: receipt.stderrSha256 }), "attempt process observation differs");
  }
}
export function validateGates(root: string, recipe: EvidenceRecipe = productionRecipe(root)) {
  const path = `${directory}/implementation/gates.json`;
  const gates = JSON.parse(canonicalBytes(root, path).toString());
  return validateGateData(root, gates, reference(root, path), recipe);
}
export function validatePriorArchive(root: string, gates: any) {
  const ref = gates.priorObservations, path = `${directory}/implementation/collection-next.json`;
  requireValue(ref?.path === path && equal(Object.keys(ref).sort(), ["path", "sha256"]), "required combined archive reference differs");
  requireValue(Array.isArray(gates.commands) && gates.commands.length > 0 && gates.commands.every((command: any) =>
    command.path === ref.path && command.sha256 === ref.sha256 && command.selector), "archive does not bind current command container");
  const archived = canonicalArchive(root, ref);
  for (const [path, sha256] of [[persistent4Path, persistent4Sha256], [attempt13Path, attempt13Sha256]])
    requireValue(archived[path]?.selector?.sha256 === sha256, "required predecessor archive missing or altered");
}
// The assembler evaluates the same predicates before publishing passed=true;
// this function does not require the output file or any successor to exist.
export function validateGateData(root: string, gates: any, boundReference?: { path: string; sha256: string }, recipe: EvidenceRecipe = productionRecipe(root)) {
  return predecessorReads(root, "gates", () => validatePredecessors(root, gates, boundReference, recipe));
}
function validatePredecessors(root: string, gates: any, boundReference: { path: string; sha256: string } | undefined, recipe: EvidenceRecipe) {
  const ctx = recipe.context(), path = `${directory}/implementation/gates.json`;
  recipe.common(gates, ctx, "implementation-gates", "backend");
  requireValue(equal(gates.currentInputs, ctx.currentInputs) && gates.passed === true && Array.isArray(gates.blockers) && gates.blockers.length === 0 && Array.isArray(gates.attempts), "incomplete gate decision");
  // First-run test fixtures deliberately have no accepted production archive.
  // Production context cannot carry fixtureOnly, and validates the full chain.
  if (ctx.fixtureOnly) { if (gates.priorObservations) canonicalArchive(root, gates.priorObservations); }
  else validatePriorArchive(root, gates);
  const dependencies = [gates.commands, gates.timings, gates.historical, gates.preservation, gates.attempts, gates.coverage];
  rejectFutureReferences(dependencies, undefined, root);
  requireValue(gates.timings?.path === `${directory}/implementation/timing.json` && gates.historical?.path === `${directory}/implementation/historical.json` && gates.preservation?.path === `${directory}/implementation/preservation.json`, "substituted gate observations");
  const commands = recipe.prerequisites(gates, ctx);
  reachableEvidence(root, dependencies, [],
    record => rejectFutureReferences(record, "gates attempted successor read or terminal validation", root));
  const attempts = new Set<string>();
  for (const attempt of gates.attempts) {
    const ref = attempt?.receipt, identity = JSON.stringify([ref?.path, ref?.selector?.path ?? null]);
    requireValue(!attempts.has(identity) && attempt?.resolution === "retained; matching successful current command selected in commands", "duplicate or invalid attempt");
    attempts.add(identity);
    const receipt = boundJSON(root, ref);
    validateFailedAttempt(root, recipe, ctx, receipt);
    const index = gates.commands.findIndex((ref: any) => equal(ref, attempt.resolvedBy));
    requireValue(index >= 0 && equal(receipt.argv, commands[index].argv) && equal(receipt.executable, commands[index].executable) && receipt.cwd === commands[index].cwd && Date.parse(receipt.ended) <= Date.parse(commands[index].started), "attempt lacks matching successful command after failure");
  }
  recipe.timing(boundJSON(root, gates.timings), ctx, commands);
  recipe.historical(boundJSON(root, gates.historical), ctx, commands);
  const preservation = boundJSON(root, gates.preservation);
  recipe.common(preservation, ctx, "preservation", "backend");
  requireValue(preservation.passed === true && preservation.positiveTamperRejections >= 7 && recipe.preservation().passed, "preservation/tamper proof incomplete");
  const workspace = commands.find((r: any) => equal(r.argv, recipe.workspaceArgv));
  for (const identity of recipe.requiredTests)
    requireValue(workspace.execution.tests.some((t: any) => t.name.endsWith(`::${identity}`) && t.status === "ok"), `missing live execution: ${identity}`);
  requireValue(workspace.execution.tests.filter((t: any) => t.name.startsWith(recipe.terminalPrefix) && t.status === "ok").length >= recipe.terminalCount, "native terminal matrix incomplete");
  const coverage = gates.coverage;
  const { report, profiles } = recipe.collection(coverage?.runId);
  requireValue(coverage.sourceHash === report.sourceHash && equal(coverage.crates, report.crates) && equal(coverage.childProof, profiles.childProof) && coverage.profilesChecked === profiles.profiles.length && coverage.independentlyRecomputed === true, "coverage accounting/profile mismatch");
  // Only the explicitly isolated first-run fixture represents the old
  // six-crate contract. Real collections are recomputed against live inputs
  // and always require the current seven-crate inventory.
  const inventory = ctx.fixtureOnly === true && ctx.baselineHead === baseline && ctx.baselineTree === baselineTree ? "historical-fixture" : "current";
  validateFloors(report.crates, inventory);
  requireValue(coverage.summary?.path === `.arc-verify/rust-coverage/baseline/runs/${coverage.runId}/summary.json` && hash(canonicalBytes(root, coverage.summary.path)) === coverage.summary.sha256, "coverage summary digest mismatch");
  const latest = JSON.parse(canonicalBytes(root, ".arc-verify/rust-coverage/baseline/latest.json").toString());
  requireValue(latest.pass === true, "live coverage pointer failed"); recipe.collection(latest.runId);
  const contract = JSON.parse(canonicalBytes(root, `${directory}/contract.json`).toString());
  predecessorBindings(contract.receiptDependencyGraph.nodes, "gates", {
    inputs: { sha256: ctx.sourceHash }, "prerequisite-observations": { sha256: hash(JSON.stringify([gates.commands, gates.timings, gates.historical, gates.preservation, gates.coverage])) },
  });
  return { passed: true, sourceHash: ctx.sourceHash, gates, commands, context: ctx, reference: boundReference ?? { path, sha256: hash(JSON.stringify(gates, null, 2)+"\n") } };
}
