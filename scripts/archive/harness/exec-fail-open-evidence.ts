import { reachableEvidence } from "./exec-fail-open-traversal";
import { decodeContainer, referenceIdentity } from "./exec-fail-open-container";
import { predecessorReads } from "./exec-fail-open-reads";
import { productionRecipe, type EvidenceRecipe } from "./exec-fail-open-recipe";
import { hash, requireValue, equal, boundJSON, artifact, canonicalBytes, directory, requireStaged } from "./exec-fail-open-inputs";
import { validateGates } from "./exec-fail-open-gates";
import { predecessorBindings } from "./exec-fail-open-graph";
import { commandOutput } from "./exec-fail-open-receipts";
import { parseCargoExecution } from "./managed-exec-test-output";
const previousSource = "6fd5018b3e56e6e1fbc9190fbe74ff3c990b89bbd8b73e9778d354f97d96d00e";
const previousContainer = "4456ce62e9949a3a8e1b208983ec67318cf0ff73e596c76640f7a9149f6e9366";
const approvedInputs = ["crates/arc-cli/src/main.rs", "docs/managed-exec.md", "scripts/harness/exec-fail-open-collection.ts",
  "scripts/harness/exec-fail-open-compact.test.ts", "scripts/harness/exec-fail-open-compact.ts", "scripts/harness/exec-fail-open-container.ts",
  "scripts/harness/exec-fail-open-docs.ts", "scripts/harness/exec-fail-open-evidence.ts", "scripts/harness/exec-fail-open-gates.ts"];
export function validateQaTransition(root: string, qa: any, ctx: any) {
  const reviewPath = `${directory}/qa/review.json`, reportPath = `${directory}/qa/review.md`;
  const reviewBytes = canonicalBytes(root, reviewPath), reportBytes = canonicalBytes(root, reportPath);
  requireStaged(root, reviewPath, reviewBytes); requireStaged(root, reportPath, reportBytes);
  requireValue(hash(reviewBytes) === "c0b7cfb81f1de855436cab07ea32817bb2cc41e919a2c282451e013c9bc17dd4" &&
    hash(reportBytes) === "ded9cd8ef940dc46081ecedf6f3cc2659926d7e7a9391b812db4839d93d4b1a7" &&
    equal(JSON.parse(reviewBytes.toString()), qa), "accepted QA bytes differ");
  requireValue(qa.sourceHash === previousSource && hash(JSON.stringify(qa.currentInputs)) === previousSource &&
    hash(JSON.stringify(ctx.currentInputs)) === ctx.sourceHash && equal(Object.keys(qa.currentInputs), Object.keys(ctx.currentInputs)), "QA transition inventory differs");
  const changed = Object.keys(ctx.currentInputs).filter(path => !equal(qa.currentInputs[path], ctx.currentInputs[path]));
  requireValue(changed.length > 0 && changed.every(path => approvedInputs.includes(path)), "unapproved QA transition input");
  const reverse = (path: string, after: string, before: string) => {
    const text = canonicalBytes(root, path).toString();
    requireValue(text.split(after).length === 2 && hash(text.replace(after, before)) === qa.currentInputs[path].sha256, `unexpected copy change: ${path}`);
  };
  reverse("crates/arc-cli/src/main.rs", "Use arcctl job status <JOB_ID> to observe a record after daemon recovery; this may update the lifecycle record.",
    "Use arcctl job status <JOB_ID> for read-only inspection after daemon recovery.");
  reverse("docs/managed-exec.md", "Tracking recovery is observational. A by-ID status check may update the lifecycle record with a fresh observation, but never replays the command.",
    "Tracking recovery is observational. Status inspection is read-only.");
  const saved = "target/exec-fail-open/task13-preserved", original = canonicalBytes(root, `${saved}/collection-next-original.json`);
  requireValue(original.length === 152867036 && hash(original) === previousContainer, "accepted collection bytes differ");
  const originalEntries = decodeContainer(original).entries;
  const previousPaths = ["gates", "timing", "historical", "preservation"];
  const previousFiles = previousPaths.map(name => {
    const bytes = canonicalBytes(root, `${saved}/prior-canonical/${name}.json`), path = `${directory}/implementation/${name}.json`;
    return { path, sha256: hash(bytes), value: JSON.parse(bytes.toString()) };
  });
  const gate = previousFiles[0].value;
  requireValue(gate.passed === true && gate.sourceHash === previousSource && equal(gate.currentInputs, qa.currentInputs) &&
    equal(gate.observedGit, qa.observedGit), "accepted gate source differs");
  const expected = [...previousFiles.map(({ path, sha256 }) => ({ path, sha256 })), ...gate.commands];
  requireValue(Array.isArray(qa.reviewedReceipts) && qa.reviewedReceipts.length === expected.length &&
    expected.every(ref => qa.reviewedReceipts.some((item: any) => equal(item, ref))), "accepted QA predecessor bindings differ");
  for (const ref of gate.commands) {
    const entry = originalEntries.find(e => e.path === ref.selector?.path);
    requireValue(ref.path === `${directory}/implementation/collection-next.json` && ref.sha256 === previousContainer && entry?.scope === "current" &&
      entry.sha256 === ref.selector.sha256 && JSON.parse(Buffer.from(entry.bytes, "base64").toString()).sourceHash === previousSource, "accepted command selection differs");
  }
  validateReviewOutcome(qa);
  const command = qa.commands?.[0];
  requireValue(qa.commands?.length === 1 && command.actorId === "reviewer" && command.skill === "oma-qa" &&
    equal(command.argv, ["cargo", "test", "--workspace"]) && command.sourceHash === previousSource && command.sourceHashAfter === previousSource &&
    equal(command.currentInputs, qa.currentInputs) && equal(command.observedGit, qa.observedGit) && equal(command.observedGitAfter, qa.observedGit) &&
    command.cwd === "." && command.exit === 0 && command.status === "passed" && command.skipFlags === false && command.pid > 0 && command.timeoutAllowanceMs >= 7200000,
  "accepted QA workspace command differs");
  const output = commandOutput(root, command), execution = parseCargoExecution(output.stdout);
  requireValue(equal(execution, command.execution) && execution.passed > 0 && execution.failed === 0 && execution.ignored === 0 &&
    !/^test result:.*; [1-9]\d* filtered out;/m.test(output.stdout), "accepted QA workspace output differs");
  return { previousSourceHash: previousSource, currentSourceHash: ctx.sourceHash, reviewSha256: hash(reviewBytes),
    previousGateSha256: previousFiles[0].sha256, previousContainerSha256: previousContainer,
    changedInputs: changed.map(path => ({ path, before: qa.currentInputs[path].sha256, after: ctx.currentInputs[path].sha256 })) };
}
export function validateCorrectionReview(review: any, ctx: any, ready: any, transition: any, recipe: EvidenceRecipe) {
  recipe.common(review, ctx, "correction-review", "reviewer");
  requireValue(review.status === "passed" && review.openBlockers === 0 && Array.isArray(review.findings) && review.findings.length === 0 &&
    typeof review.frameworkReviewId === "string" && /^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/i.test(review.frameworkReviewId) &&
    equal(review.gates, ready.reference) && review.transitionSha256 === hash(JSON.stringify(transition)), "independent correction review missing or unbound");
  const required = ["exec-fail-open-docs.test.ts", "exec-fail-open-compact.test.ts", "exec-fail-open-compact-flow.test.ts"].map(path =>
    ["bun", "test", `scripts/harness/${path}`]);
  requireValue(Array.isArray(review.commands) && review.commands.length === required.length &&
    required.every(argv => review.commands.some((r: any) => equal(r.argv, argv))) &&
    new Set(review.commands.map((r: any) => JSON.stringify(r.argv))).size === required.length, "missing independent correction checks");
  const checked = review.commands.map((command: any) => {
    requireValue(command.timeoutAllowanceMs >= 7200000, "independent correction timeout too short");
    return recipe.command(command, ctx, "reviewer");
  });
  requireValue(Number.isFinite(Date.parse(review.reviewedAt)) &&
    Date.parse(review.reviewedAt) >= Math.max(...[...checked, ...ready.commands].map((r: any) => Date.parse(r.ended))), "correction review predates observations");
}
export function validateReviewOutcome(qa: any) {
  requireValue(qa.actorId === "reviewer" && qa.skill === "oma-qa" && qa.status === "passed" && qa.openBlockers === 0 && qa.productFilesEdited === false && qa.implementationTestsEdited === false, "self-review or incomplete independent QA");
  requireValue(Array.isArray(qa.findings) && qa.findings.every((f: any) => ["resolved", "closed"].includes(f.disposition) && typeof f.evidence === "string" && f.path), "unresolved or unsupported finding");
}
export function validateVerdict(final: any) {
  requireValue(final.verdict === "pass" && final.openBlockers === 0, "invalid final verdict or blockers");
}
export function validateEvidence(root: string, recipe: EvidenceRecipe = productionRecipe(root)) {
  const checked = predecessorReads(root, "evidence", () => validatePredecessors(root, recipe));
  // Optional terminal bytes are checked for index hygiene only, never consumed
  // by the semantic acceptance walk above.
  const count = recipe.index(checked.required, checked.evidenceFiles);
  return { passed: true, sourceHash: checked.sourceHash, trackedFiles: count };
}
function validatePredecessors(root: string, recipe: EvidenceRecipe) {
  const ready = validateGates(root, recipe), ctx = ready.context;
  const final = JSON.parse(canonicalBytes(root, `${directory}/final.json`).toString());
  recipe.common(final, ctx, "final-decision");
  requireValue(equal(final.currentInputs, ctx.currentInputs) && final.verdict === "pass" && final.openBlockers === 0 && equal(final.gates, ready.reference), "invalid final decision or gate binding");
  requireValue(equal(final.timings, ready.gates.timings) && equal(final.coverage, ready.gates.coverage), "final measurements differ");
  validateVerdict(final);
  requireValue(final.review.path === `${directory}/qa/review.json` && final.reviewReport.path === `${directory}/qa/review.md`, "substituted independent report path");
  const qa = boundJSON(root, final.review);
  const transitioned = final.reviewTransition !== undefined;
  if (transitioned) {
    requireValue(equal(final.reviewTransition, validateQaTransition(root, qa, ctx)), "QA transition binding differs");
    validateCorrectionReview(final.correctionReview, ctx, ready, final.reviewTransition, recipe);
  }
  else {
    recipe.common(qa, ctx, "independent-review", "reviewer");
    requireValue(equal(qa.currentInputs, ctx.currentInputs) && qa.status === "passed" && qa.openBlockers === 0 && qa.productFilesEdited === false && qa.implementationTestsEdited === false, "self-review or incomplete independent QA");
    requireValue(Array.isArray(qa.reviewedReceipts) && [ready.reference, ready.gates.timings, ready.gates.historical, ...ready.gates.commands].every(ref => qa.reviewedReceipts.some((q: any) => equal(q, ref))), "review does not bind predecessor receipts");
    qa.reviewedReceipts.forEach((ref: any) => artifact(root, ref));
  }
  requireValue(equal(qa.report, final.reviewReport), "review report binding differs");
  requireValue(Array.isArray(qa.findings) && qa.findings.every((f: any) => ["resolved", "closed"].includes(f.disposition) && typeof f.evidence === "string" && f.path), "unresolved or unsupported finding");
  requireValue(Array.isArray(qa.commands) && qa.commands.length > 0, "missing independent execution");
  const reviewCommands = transitioned ? qa.commands : qa.commands.map((command: any) => {
    const checked = recipe.command(command, ctx, "reviewer");
    requireValue(Date.parse(command.started) >= Math.max(...ready.commands.map((r: any) => Date.parse(r.ended))), "review predates completed prerequisites");
    return checked;
  });
  validateReviewOutcome(qa);
  const workspace = reviewCommands.find((r: any) => equal(r.argv, recipe.workspaceArgv));
  requireValue(workspace && workspace.execution.ignored === 0 && workspace.execution.passed > 0, "missing unfiltered independent workspace test");
  const report = artifact(root, final.reviewReport).toString();
  requireValue(report.length >= 200 && report.includes(ctx.baselineHead), "QA report incomplete or wrong baseline");
  const claims = artifact(root, final.claimsReview).toString();
  requireValue(final.claimsReview.path === `${directory}/claims-review.md` && claims.length >= 200 && claims.includes("unmeasured"), "unbound claims review");
  for (const term of ["desktop", "native Windows", "vendor hooks", "scheduling"]) requireValue(Array.isArray(final.outstanding) && final.outstanding.some((s: string) => s.includes(term)) && claims.includes(term), `missing outstanding ${term}`);
  requireValue(typeof final.limitations === "string" && final.limitations.includes("unmeasured"), "unmeasured outcomes omitted");
  const required = ["contract.json", "contract.md", "design-contract.md", "claims-review.md", "implementation/gates.json", "final.json"].map(p => `${directory}/${p}`)
    .concat(ready.gates.commands.map((r: any) => r.path), [final.review.path, final.reviewReport.path, ready.gates.timings.path, ready.gates.historical.path, ready.gates.preservation.path]);
  // The terminal sink may be absent on the first pass. Index/type/count rules
  // include it when present, without requiring or trusting its passed flag.
  // Every reachable new evidence reference must be staged, including native
  // observation leaves. No successor sink is read to establish acceptance.
  required.push(...reachableEvidence(root, transitioned ? { ...final, review: undefined } : final, [referenceIdentity({ path: `${directory}/final.json` })]));
  const contract = JSON.parse(canonicalBytes(root, `${directory}/contract.json`).toString());
  predecessorBindings(contract.receiptDependencyGraph.nodes, "final", {
    inputs: { sha256: ctx.sourceHash }, "prerequisite-observations": { sha256: hash(JSON.stringify([ready.gates.commands, ready.gates.timings, ready.gates.historical, ready.gates.preservation, ready.gates.coverage])) },
    gates: ready.reference, review: final.review,
  });
  return { sourceHash: ctx.sourceHash, required, evidenceFiles: final.evidenceFiles };
}
