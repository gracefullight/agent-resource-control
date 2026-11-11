//! Explicit isolated fixture leaves. Production entrypoints never select this recipe.
import { resolve } from "node:path";
import { root as productRoot, load, canonicalBytes, boundJSON, equal, hash, requireValue, directory, validateIndex } from "./exec-fail-open-inputs";
import { type EvidenceRecipe } from "./exec-fail-open-recipe";
import { validateInvocationNumbers } from "./exec-fail-open-native";
export const fixtureCatalog = load(productRoot, `${directory}/contract.json`).receiptDependencyGraph.nodes;
export function fixtureRecipe(root: string): EvidenceRecipe {
  requireValue(resolve(root) !== productRoot && load(root, "fixture-recipe.json").fixtureOnly === true, "fixture requires a separate explicit root");
  const manifest = load(root, "fixture-recipe.json");
  const catalog = () => load(root, `${directory}/contract.json`).commandPhases.prerequisiteCommands;
  const ctx = () => ({ ...manifest.context, sourceHash: hash(canonicalBytes(root, "inputs.json")) });
  const common = (value: any, expected: any, kind: string, actor?: string) => {
    requireValue(value.fixtureOnly === true && expected.fixtureOnly === true && value.kind === kind && value.sourceHash === expected.sourceHash && equal(value.currentInputs, expected.currentInputs), "stale or invalid fixture observation");
    requireValue(!actor || value.actorId === actor, "fixture actor mismatch");
  };
  const processResult = (receipt: any) => {
    const observed = boundJSON(root, receipt.processObservation);
    requireValue(observed.fixtureOnly === true && observed.exit === 0 && equal(observed, {
      fixtureOnly: true, pid: receipt.pid, exit: receipt.exit,
      stdoutSha256: receipt.stdoutSha256, stderrSha256: receipt.stderrSha256,
    }), "failed fixture subprocess or relabeled result");
    for (const stream of ["stdout", "stderr"]) requireValue(hash(canonicalBytes(root, receipt.output[`${stream}Path`])) === receipt[`${stream}Sha256`], "altered fixture subprocess output");
    return receipt;
  };
  const command = (receipt: any, expected: any, actor = "backend") => {
    common(receipt, expected, "command", actor); processResult(receipt);
    requireValue(receipt.skill === (actor === "reviewer" ? "oma-qa" : "oma-backend"), "fixture skill mismatch");
    requireValue(catalog().some((argv: string[]) => equal(receipt.argv, argv)) && receipt.sourceHashAfter === expected.sourceHash, "fixture command/input substitution");
    const output = JSON.parse(canonicalBytes(root, receipt.output.stdoutPath).toString());
    requireValue(output.fixtureOnly === true && output.inputSha256 === expected.sourceHash && output.passed === true &&
      (manifest.compact ? receipt.execution === undefined : equal(receipt.execution, { passed: 1, failed: 0, ignored: 0, tests: [{ name: output.test, status: "ok" }] })), "fixture command did not execute");
    return manifest.compact && equal(receipt.argv, manifest.workspaceArgv) ? { ...receipt, execution: { passed: 1, failed: 0, ignored: 0, tests: [{ name: "fixture::prerequisite", status: "ok" }] } } : receipt;
  };
  const preservation = () => {
    const r = manifest.compact ? boundJSON(root, manifest.prerequisite) : load(root, `${directory}/implementation/commands/prerequisite.json`);
    command(r, ctx());
    return { passed: true, fixtureOnly: true };
  };
  const recipe: EvidenceRecipe = {
    root, context: ctx as any, common: common as any, command,
    prerequisites: (gates, expected) => {
      const catalog = load(root, `${directory}/contract.json`).commandPhases.prerequisiteCommands;
      const receipts = gates.commands.map((ref: any) => command(boundJSON(root, ref), expected));
      requireValue(receipts.length === catalog.length && catalog.every((argv: string[]) => receipts.some((r: any) => equal(r.argv, argv))), "fixture prerequisite partition differs");
      return receipts;
    },
    timing: (timing, expected, commands) => {
      common(timing, expected, "native-timing", "backend");
      requireValue(timing.passed === true && timing.cases.length === (manifest.scenarios?.length ?? 1), "fixture timing incomplete");
      for (const c of timing.cases) {
        const raw = boundJSON(root, c.observation); validateInvocationNumbers(c);
        requireValue(raw.fixtureOnly === true && equal(c.samples, raw.samples) && c.elapsedToInvocationMs === raw.parentSpawnToInvocationMs && commands.some(r => equal(manifest.compact ? { ...r, execution: undefined } : r, boundJSON(root, c.commandReceipt))), "fixture timing binding differs");
      }
      return timing;
    },
    historical: (h, expected) => {
      common(h, expected, "historical-fixture", "backend"); preservation();
      requireValue(h.passed === true && h.preparation.fixtureOnly === true && boundJSON(root, h.negativeControl).fixtureOnly === true, "fixture history incomplete");
      return h;
    },
    preservation,
    collection: runId => {
      const report = load(root, `.arc-verify/rust-coverage/baseline/runs/${runId}/summary.json`);
      requireValue(report.fixtureOnly === true && report.sourceHash === ctx().sourceHash && report.pass === true, "stale fixture coverage");
      preservation();
      return { report, profiles: { profiles: [], childProof: [] } };
    },
    index: (required, count) => validateIndex(root, required, count),
    scenarios: manifest.scenarios ?? ["absent"], requiredTests: ["prerequisite"],
    workspaceArgv: manifest.workspaceArgv, terminalPrefix: "fixture::", terminalCount: 1,
    preservationArgument: manifest.workspaceArgv[1],
  };
  // Supplementary fixture process validation is exposed only by its named export.
  return Object.assign(recipe, { processResult });
}
export function validateFixtureProcess(root: string, receipt: any) {
  return (fixtureRecipe(root) as EvidenceRecipe & { processResult: (r: any) => any }).processResult(receipt);
}
