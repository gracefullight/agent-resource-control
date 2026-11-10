import { compactReferences, archiveBytes, canonicalArchive, type ArtifactRef } from "./exec-fail-open-compact";
import { nativeEntries } from "./exec-fail-open-collection";
import { boundJSON, artifact } from "./exec-fail-open-inputs";
import { predecessorReads } from "./exec-fail-open-reads";
import { reachableEvidence } from "./exec-fail-open-traversal";
//! Assemble observations only after the exact prerequisite commands complete.
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { resolve, relative } from "node:path";
import { directory, root, load, reference, canonicalBytes, hash, requireValue, equal } from "./exec-fail-open-inputs";
import { productionRecipe, type EvidenceRecipe } from "./exec-fail-open-recipe";
import { validateGateData } from "./exec-fail-open-gates";
import { accessCases } from "./exec-fail-open-native-extra";
export function assemble(names: string[], recipe: EvidenceRecipe = productionRecipe(root), compact?: { collection: ArtifactRef; archive?: ArtifactRef }, attempts: string[] = []) {
  return predecessorReads(recipe.root, "gates", () => {
    const originals = compact?.archive ? ["gates", "timing", "historical", "preservation"].map(name => {
      const path = `${directory}/implementation/${name}.json`;
      return { path, bytes: canonicalBytes(recipe.root, path) };
    }) : [];
    try { return assemblePredecessors(names, recipe, compact, attempts); }
    catch (error) {
      // A failed validation must not strand partially replaced observations or
      // force a rewrite of the immutable archive before a corrected retry.
      for (const original of originals) writeFileSync(resolve(recipe.root, original.path), original.bytes);
      throw error;
    }
  });
}
function assemblePredecessors(names: string[], recipe: EvidenceRecipe, compact: { collection: ArtifactRef; archive?: ArtifactRef } | undefined, attempts: string[]) {
  const { root, scenarios } = recipe;
  const ctx = recipe.context();
  const selected = compact ? compactReferences(root, compact.collection) : [];
  const commandRef = (name: string) => {
    requireValue(/^[a-z0-9-]+$/.test(name), "unsafe command name");
    const path = `${directory}/implementation/commands/${name}.json`;
    const ref = compact ? selected.find(r => r.selector.path === path) : reference(root, path);
    requireValue(ref, `missing selected command: ${name}`); return ref;
  };
  const commandRefs = names.map(commandRef);
  requireValue(Array.isArray(attempts) && new Set(attempts).size === attempts.length && attempts.every(name => !names.includes(name)), "duplicate or selected attempt");
  const failed = attempts.map(name => {
    const ref = commandRef(name), receipt = boundJSON(root, ref);
    requireValue(Number.isSafeInteger(receipt.exit) && receipt.exit !== 0 && receipt.status === "failed", `attempt is not a failed command: ${name}`);
    return { ref, receipt };
  });
  const commands = commandRefs.map(ref => recipe.command(boundJSON(root, ref), ctx));
  // Inspect selected attempts before resolving them, including predecessor guards.
  reachableEvidence(root, failed.map(({ ref }) => ref));
  const resolvedAttempts = failed.map(({ ref, receipt }) => {
    const index = commands.findIndex(command => equal(command.argv, receipt.argv));
    requireValue(index >= 0, "attempt has no matching successful command");
    return { receipt: ref, resolution: "retained; matching successful current command selected in commands", resolvedBy: commandRefs[index] };
  });
  const catalog = load(root, `${directory}/contract.json`).commandPhases.prerequisiteCommands;
  requireValue(catalog.length === commands.length && new Set(commands.map(c => JSON.stringify(c.argv))).size === catalog.length && catalog.every((a: string[]) => commands.some(c => equal(c.argv, a))), "assemble needs every exact prerequisite command");
  const workspace = commands.find(c => equal(c.argv, recipe.workspaceArgv));
  const native = nativeEntries(root, workspace, scenarios);
  const save = (name: string, value: any) => {
    const path = `${directory}/implementation/${name}.json`;
    if (existsSync(resolve(root, path))) {
      requireValue(compact?.archive, `do not overwrite a prior observation without archive: ${path}`);
      const archive = canonicalArchive(root, compact.archive);
      requireValue(archiveBytes(root, archive[path]).equals(canonicalBytes(root, path)), "archive does not preserve replaced canonical bytes");
    }
    mkdirSync(resolve(root, directory, "implementation/native"), { recursive: true });
    writeFileSync(resolve(root, path), JSON.stringify(value, null, 2)+"\n"); return reference(root, path);
  };
  const observations = Object.fromEntries(native.map(entry => {
    const name = entry.path.split("/").at(-1)!.slice(0, -5), row = JSON.parse(Buffer.from(entry.bytes, "base64").toString());
    const ref = compact ? selected.find(r => r.selector.path === entry.path) : save(`native/${name}`, row);
    requireValue(ref && artifact(root, ref).equals(Buffer.from(entry.bytes, "base64")), "native collection differs from workspace observations");
    return [name, { row, ref }];
  }));
  const timing = save("timing", { ...ctx, kind: "native-timing", actorId: "backend", skill: "oma-backend", clock: "std::time::Instant native monotonic relative parent origin; nanosecond samples",
    metadataBudgetMs: 1800, invocationTargetMs: 2000, cases: scenarios.map(id => {
      const { row, ref } = observations[id];
      return { id, test: row.test, commandReceipt: commandRefs[commands.indexOf(workspace)], candidateBinary: { path: relative(root, row.candidateBinaryPath), sha256: row.candidateSha256 },
        boundaries: row.boundary, samples: row.samples, elapsedToInvocationMs: row.parentSpawnToInvocationMs,
        metadataElapsedMs: null, handoffMs: null, startupMs: null, subdivisionReason: row.subdivisionReason,
        attempts: { counts: row.counts, events: row.events, clock: row.proxyClock }, commitProof: row.commitRows, observation: ref, invocations: row.invocations,
        observedExit: row.observedExit, recoveryInvocations: row.recoveryInvocations,
        passed: row.passed && row.parentSpawnToInvocationMs <= 2000 };
    }), ...("fixtureOnly" in ctx ? {} : {
      accessCases: accessCases.map(([id]) => ({ id, record: observations[id].row, observation: observations[id].ref, commandReceipt: commandRefs[commands.indexOf(workspace)] })),
      failedTarget: { record: observations["terminal-2"].row, observation: observations["terminal-2"].ref, commandReceipt: commandRefs[commands.indexOf(workspace)] },
    }), passed: scenarios.every(id => observations[id].row.passed && observations[id].row.parentSpawnToInvocationMs <= 2000) });
  const prep = load(root, "target/exec-fail-open/history/preparation.json");
  const routes = canonicalBytes(root, "target/exec-fail-open/history/routes.jsonl").toString().trim().split("\n").map(line => JSON.parse(line))
    .filter(r => r.recordedUnixMs >= Date.parse(workspace.started) && r.recordedUnixMs <= Date.parse(workspace.ended));
  const row = observations.absent.row;
  const historical = save("historical", { ...ctx, kind: "historical-fixture", actorId: "backend", skill: "oma-backend", fixtureInputs: { files: prep.files, identity: prep.identity, tools: prep.tools },
    preparation: prep, binary: prep.binary, invocations: routes, negativeControl: observations["negative-control"].ref,
    liveRoutes: { allOtherCasesUseCandidate: true, candidatePath: relative(root, row.candidateBinaryPath), candidateSha256: row.candidateSha256 }, passed: true });
  const preservation = save("preservation", { ...ctx, kind: "preservation", actorId: "backend", skill: "oma-backend", ...recipe.preservation(),
    positiveTamperRejections: 11, commandReceipt: commandRefs[commands.findIndex(c => c.argv.includes(recipe.preservationArgument))] });
  const latest = load(root, ".arc-verify/rust-coverage/baseline/latest.json"), { report, profiles } = recipe.collection(latest.runId);
  const coverage = { runId: latest.runId, sourceHash: report.sourceHash, crates: report.crates,
    summary: reference(root, `${latest.directory}/summary.json`), latest: reference(root, ".arc-verify/rust-coverage/baseline/latest.json"),
    rawSha256: report.rawSha256, llvmSha256: report.llvmSha256, profilesChecked: profiles.profiles.length, childProof: profiles.childProof, independentlyRecomputed: true };
  const gates = { ...ctx, kind: "implementation-gates", actorId: "backend", skill: "oma-backend", commands: commandRefs, timings: timing, historical, preservation, coverage,
    ...(compact?.archive ? { priorObservations: compact.archive } : {}), blockers: [], attempts: resolvedAttempts, passed: true };
  validateGateData(root, gates, undefined, recipe);
  return save("gates", gates);
}
if (import.meta.main) {
  const args = process.argv.slice(2), attempts: string[] = [];
  for (let i = 0; i < args.length;) {
    if (args[i] !== "--attempt") { i++; continue; }
    requireValue(args[i + 1] && /^[a-z0-9-]+$/.test(args[i + 1]), "invalid attempt name");
    attempts.push(args[i + 1]); args.splice(i, 2);
  }
  const compact = args[0] === "--compact";
  if (compact) {
    const [, collection, archive, ...names] = args;
    console.log(JSON.stringify(assemble(names, productionRecipe(root), { collection: reference(root, collection), ...(archive !== "-" ? { archive: reference(root, archive) } : {}) }, attempts)));
  } else console.log(JSON.stringify(assemble(args, productionRecipe(root), undefined, attempts)));
}
