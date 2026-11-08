import { existsSync, readFileSync, writeFileSync, lstatSync } from "node:fs";
import { resolve } from "node:path";
import { createHash } from "node:crypto";
import { managedInputs, managedGit, nativeHost, requiredCommands, validateCommandReceipt } from "../../../scripts/harness/managed-exec-evidence";
import { validateCargoExecution } from "../../../scripts/harness/managed-exec-test-output";
import { gitBytes } from "../../../scripts/coverage/hygiene";
import { inputIdentity, sourceInventory } from "../../../scripts/coverage/source";
import { account } from "../../../scripts/coverage/accounting";
const root = resolve(import.meta.dir, "../../.."), base = ".arc-verify/managed-exec/implementation/";
const hash = (b: Buffer | string) => createHash("sha256").update(b).digest("hex");
const load = (p: string) => JSON.parse(readFileSync(resolve(root, p), "utf8"));
const ref = (path: string) => ({ path, sha256: hash(readFileSync(resolve(root, path))) });
const check = (ok: unknown, message: string) => { if (!ok) throw Error(message); };
const inputs = managedInputs(root), host = nativeHost(), supplied = load(base + "inputs.json");
check(JSON.stringify(inputs) === JSON.stringify(supplied.inputs), "177 supplied bindings differ");
for (const [p, value] of Object.entries(inputs.files)) {
  check(hash(readFileSync(resolve(root, p))) === value.sha256 && (lstatSync(resolve(root, p)).mode & 0o777) === value.mode, p);
}
const names = ["fmt", "clippy", "workspace", "preservation", "equivalence", "coverage-verification", "clean-checkout-final",
  "coverage-cli", "coverage-root", "harness", "harness-root", "evidence", "privacy", "positive-execution-test", "recent-green"];
// This historical bootstrap command is not a required gate. Keep its bytes,
// record its stale binding and rely on fresh exact workspace execution instead.
const historicalPrerequisite = load(base + "cli-prerequisite.json");
const excludedHistoricalReceipt = { ...ref(base + "cli-prerequisite.json"),
  sourceHash: historicalPrerequisite.sourceHash, argv: historicalPrerequisite.argv,
  reason: "Historical bootstrap only; not counted as current acceptance. Current workspace command exercises arc-cli." };
const receipts = names.map(name => {
  const path = base + name + ".json", value = load(path);
  validateCommandReceipt(value, inputs.sha256, host);
  check(value.exit === 0, `failed receipt ${path}`);
  for (const stream of ["stdout", "stderr"]) check(hash(readFileSync(resolve(root, value.output[stream + "Path"]))) === value[stream + "Sha256"], `output ${path}`);
  if (value.argv[0] === "cargo" && value.argv[1] === "test") validateCargoExecution(value, root);
  if (value.extractedFrom) {
    for (const key of ["command", "binding", "outer"]) check(ref(value.extractedFrom[key].path).sha256 === value.extractedFrom[key].sha256, `nested ${key}`);
    const nested = load(value.extractedFrom.command.path), binding = load(value.extractedFrom.binding.path);
    check(JSON.stringify(nested.argv) === JSON.stringify(value.argv) && nested.exit === 0 && nested.cwd === root && nested.pid === value.pid, "nested exact argv/cwd/PID");
    check(hash(nested.stdout) === value.stdoutSha256 && hash(nested.stderr) === value.stderrSha256, "nested output differs");
    check(binding.sourceHash === inputIdentity(root).sha256 && binding.afterHash === binding.sourceHash && binding.unchanged === true && binding.exit === 0, "nested coverage input binding");
  }
  return { ...ref(path), value };
});
for (const argv of requiredCommands) check(receipts.some(r => JSON.stringify(r.value.argv) === JSON.stringify(argv)), `missing ${argv}`);
const inventoryPath = ".arc-verify/harness-speed/coverage-verification.json", inventory = load(inventoryPath);
check(inventory.inventoryComplete && inventory.status === "passed" && inventory.exit === 0 && inventory.commands.length === 5, "coverage inventory");
check(inventory.discovered.length === 9 && inventory.executed.length === 5 && inventory.separatelyRequired.length === 4, "coverage inventory counts");
for (const entry of inventory.integratedSourceIdentity.entries) {
  check(hash(readFileSync(resolve(root, entry.path))) === entry.sha256 && (lstatSync(resolve(root, entry.path)).mode & 0o777) === entry.mode, `inventory input ${entry.path}`);
}
for (const command of inventory.commands) {
  check(command.exit === 0, "noncollecting coverage command failed");
  for (const s of ["stdout", "stderr"]) check(hash(readFileSync(resolve(command.diagnostics.directory, s + ".txt"))) === command[s + "Sha256"], "noncollecting output digest");
}
const cleanOuter = receipts.find(r => r.path === base + "clean-checkout-final.json")!.value;
const cleanOutput = readFileSync(resolve(root, cleanOuter.output.stdoutPath), "utf8");
const cleanResultPath = cleanOutput.match(/; receipts: (.+)/)![1].trim() + "/result.json";
const clean = JSON.parse(readFileSync(cleanResultPath, "utf8"));
check(clean.pass && clean.sourceHash === inputIdentity(root).sha256 && clean.commands.length === 2 && clean.commands.every((c: any) => c.exit === 0), "clean checkout result");
check(clean.precollection.priorArtifacts.length === 0 && clean.precollection.targetAbsent && clean.precollection.runsAbsent && clean.precollection.runtimeLatestAbsent && clean.precollection.sourceIdentityMatches, "clean isolation preconditions");
check(JSON.stringify(inputIdentity(clean.checkout)) === JSON.stringify(inputIdentity(root)), "isolated checkout source identity");
const cleanSummary = JSON.parse(readFileSync(resolve(clean.checkout, clean.directory, "summary.json"), "utf8"));
check(cleanSummary.pass && cleanSummary.sourceHash === clean.sourceHash && cleanSummary.crates.length === 6 && cleanSummary.crates.every((c: any) => c.covered * 100 >= c.total * 80), "clean native coverage");
const live = receipts.find(r => r.value.argv.join(" ") === "bun test scripts/coverage/evidence.test.ts")!.value;
for (const r of receipts.filter(r => ["clean-checkout-final", "coverage-cli", "coverage-root", "harness", "harness-root"].some(n => r.path === base + n + ".json")))
  check(Date.parse(r.value.ended) <= Date.parse(live.started), "live evidence predates collector");
const protectedPaths = ["crates/arc-storage/migrations/0001_phase0.sql", "scripts/coverage/accounting.ts", "scripts/coverage/source.ts", "scripts/coverage/config.ts", "scripts/coverage/gate.ts", ".arc-verify/rust-coverage/contract/accounting.json"];
const protectedFiles = protectedPaths.map(path => {
  const actual = readFileSync(resolve(root, path));
  check(actual.equals(gitBytes(root, ["show", `19d7a507044cbf83918f58cd41056905c275c8d4:${path}`])), `protected bytes ${path}`);
  return ref(path);
});
const sources = Object.keys(inputs.files).filter(p => /\.(rs|sql|py|ts|sh)$/.test(p)).map(path => ({ path, lines: readFileSync(resolve(root, path), "utf8").split("\n").length - Number(readFileSync(resolve(root, path), "utf8").endsWith("\n")) }));
check(sources.every(s => s.lines <= 500), "source length");
const latest = load(".arc-verify/rust-coverage/baseline/latest.json"), summary = load(latest.directory + "/summary.json"), profiles = load(latest.directory + "/profiles.json"), nativeReceipt = load(latest.directory + "/receipt.json");
check(latest.pass && latest.sourceHash === inputIdentity(root).sha256 && summary.sourceHash === latest.sourceHash && nativeReceipt.pass && nativeReceipt.exit === 0, "coverage source/outcome");
check(hash(readFileSync(resolve(root, latest.directory, "raw.lcov"))) === summary.rawSha256 && hash(readFileSync(resolve(root, latest.directory, "raw.json"))) === summary.llvmSha256, "native raw digest");
check(summary.crates.length === 6 && summary.crates.every((c: any) => c.total > 0 && c.covered * 100 >= c.total * 80), "per-crate floors");
const recomputed = account(readFileSync(resolve(root, latest.directory, "raw.lcov"), "utf8"), sourceInventory(root), root,
  undefined, load(latest.directory + "/raw.json"));
check(recomputed.pass && JSON.stringify(recomputed.crates) === JSON.stringify(summary.crates), "recomputed production-line denominators differ");
for (const p of profiles.profiles) {
  const b = readFileSync(resolve(profiles.target, p.path));
  check(b.length === p.size && b.length > 0 && hash(b) === p.sha256, `profile ${p.path}`);
}
check(JSON.stringify([...profiles.merge].sort()) === JSON.stringify(profiles.profiles.map((p: any) => resolve(profiles.target, p.path)).sort()), "profile merge");
check(JSON.stringify(profiles.childProof.map((c: any) => c.binary)) === JSON.stringify(["arcctl", "arc-daemon"]), "missing child profile proof");
for (const child of profiles.childProof) check(child.profiles.length > 0 && child.profiles.every((p: string) =>
  p.startsWith(`arc-${child.pid}-`) && profiles.merge.includes(resolve(profiles.target, p))) && nativeReceipt.receipts.some((r: any) =>
    r.pid === child.pid && r.exit === child.exit && r.argv[0] === resolve(profiles.target, "debug", child.binary)), "child PID receipt/profile differs");
const files = gitBytes(root, ["ls-files", "--cached", "--others", "--exclude-standard", "-z", "--", ".arc-verify/managed-exec"]).toString().split("\0").filter(Boolean);
check(new Set(files).size < 150, "evidence count");
const changed = gitBytes(root, ["diff", "HEAD", "--name-only", "--", "crates", "scripts", "docs", "Cargo.lock"]).toString().trim().split("\n");
const result = { schemaVersion: 1, actorId: "reviewer", skill: "oma-qa", auditedAt: new Date().toISOString(), host, sourceHash: inputs.sha256,
  inputs, git: managedGit(root), reviewedReceipts: receipts.map(({ value, ...r }) => r), excludedHistoricalReceipt, requiredCommandCount: requiredCommands.length,
  protectedFiles, maxSource: sources.sort((a, b) => b.lines - a.lines)[0], evidenceFilesAtAudit: new Set(files).size,
  coverage: { latest: ref(".arc-verify/rust-coverage/baseline/latest.json"), summary: ref(latest.directory + "/summary.json"), runId: latest.runId,
    sourceHash: latest.sourceHash, profilesChecked: profiles.profiles.length, childProof: profiles.childProof, crates: summary.crates, independentlyRecomputed: true },
  coverageInventory: { ...ref(inventoryPath), discovered: inventory.discovered, executed: inventory.executed, separatelyRequired: inventory.separatelyRequired, outputsVerified: true },
  cleanCheckout: { resultPath: cleanResultPath, resultSha256: hash(readFileSync(cleanResultPath)), checkout: clean.checkout, inputIdentityMatches: true, precollection: clean.precollection, crates: cleanSummary.crates },
  finalDiff: changed.map(ref), downstreamArtifactsAbsent: ["final-summary.json", "final.md"].filter(p => !existsSync(resolve(root, ".arc-verify/managed-exec", p))), pass: true };
writeFileSync(resolve(import.meta.dir, "audit.json"), JSON.stringify(result, null, 2) + "\n");
console.log(JSON.stringify({ pass: true, sourceHash: inputs.sha256, inputCount: Object.keys(inputs.files).length, reviewedReceipts: names.length, profilesChecked: profiles.profiles.length,
  coverage: summary.crates, maxSource: result.maxSource, evidenceFiles: result.evidenceFilesAtAudit }));
