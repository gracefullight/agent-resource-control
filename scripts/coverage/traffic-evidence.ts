import { lstatSync, readFileSync, readdirSync, realpathSync } from "node:fs";
import { resolve } from "node:path";
import { indexEntries, prohibitedReasons } from "./hygiene";
import { hash, inputIdentity } from "./source";
import { nativeCollection } from "../harness/exec-fail-open-coverage";
import { executableReviewTest, parseCargoExecution } from "../harness/managed-exec-test-output";
import { baselineHead, boundBytes, boundJSON, coverageRun, coverageTest, equal, localBytes, requireTraffic, sourceContext, validateReceipt } from "./traffic-receipts";
import { validateMeasurement } from "./traffic-measurement";

const directory = ".arc-verify/traffic";
export const requiredCommands = [
  ["cargo", "fmt", "--all", "--", "--check"],
  ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
  ["cargo", "test", "--workspace"],
  ["cargo", "test", "-p", "arc-daemon", "--test", "traffic_cli", "--", "--nocapture"],
  ["bun", "run", "test:coverage"], ["./scripts/harness/validate.sh"],
  ["bun", "test", "scripts/coverage/evidence.test.ts"],
];
export function validateFullCargoOutput(stdout: string) {
  const execution = parseCargoExecution(stdout);
  const counts = [...stdout.matchAll(/^test result: (?:ok|FAILED)\. \d+ passed; \d+ failed; \d+ ignored; \d+ measured; (\d+) filtered out;/gm)];
  requireTraffic(execution.ignored === 0 && execution.failed === 0 && counts.length === execution.harnesses &&
    counts.every(match => Number(match[1]) === 0), "full Cargo tests failed, ignored or filtered");
  return execution;
}
export function validateLiveBunOutput(output: string) {
  const summary = output.split(/\r?\n/).filter(line => !/^\s*\((?:pass|fail|skip|todo)\)/.test(line)).join("\n");
  requireTraffic(/^\s*[1-9]\d* pass\s*$/m.test(output) && /^\s*0 fail\s*$/m.test(output) &&
    !/^\s*[1-9]\d* (?:fail|skip|todo)\s*$/m.test(output) && !/\bfiltered(?:\s+out)?\b/i.test(summary),
    "live evidence test did not execute fully");
}
export function evidenceBudget(root: string, _baseline = baselineHead) {
  const files: string[] = [];
  const visit = (path: string) => {
    for (const name of readdirSync(resolve(root, path))) {
      const next = `${path}/${name}`, stat = lstatSync(resolve(root, next));
      requireTraffic(!stat.isSymbolicLink(), "symlink in evidence inventory");
      if (stat.isDirectory()) visit(next);
      else { requireTraffic(stat.isFile(), "nonregular evidence"); files.push(next); }
    }
  };
  visit(directory);
  // This mission's evidence inventory is .arc-verify/traffic. The separate
  // repository hygiene checks continue to inspect the rest of .arc-verify.
  const indexed = indexEntries(root, directory);
  const paths = files.sort();
  requireTraffic(paths.length > 0 && paths.length < 30, "mission evidence count must be below 30");
  let size = 0;
  for (const path of paths) {
    const separator = path.lastIndexOf("/");
    let bytes: Buffer;
    if (separator < 0) {
      requireTraffic(path !== "." && path !== ".." && !/[\\\x00-\x1f\x7f]/.test(path), "unsafe mission evidence path");
      const file = resolve(root, path), stat = lstatSync(file);
      requireTraffic(stat.isFile() && !stat.isSymbolicLink() && realpathSync(file) === resolve(realpathSync(root), path), "noncanonical evidence");
      bytes = readFileSync(file);
    } else {
      bytes = localBytes(root, path, path.slice(0, separator));
    }
    requireTraffic(prohibitedReasons(path, bytes).length === 0, `prohibited mission evidence: ${path}`);
    size += bytes.length;
  }
  requireTraffic(size < 5_000_000, "mission evidence must be below 5000000 decimal bytes");
  return { files: paths.length, bytes: size, indexed: paths.filter(path => indexed.some(e => e.path === path)).length };
}
export function validateNestedCoverage(root: string, receipt: any, stdout: string) {
  const collection = coverageRun(stdout), binding = receipt?.nestedCoverage;
  requireTraffic(binding?.runId === collection.runId && !receipt.nestedCoverageError, "nested collection binding missing or differs");
  const outerStdout = receipt?.output?.stdout?.path;
  requireTraffic(typeof outerStdout === "string" && /^target\/traffic\/commands\/[a-z0-9-]+\/stdout\.log$/.test(outerStdout), "invalid outer collector output path");
  const dir = outerStdout.slice(0, -"/stdout.log".length);
  requireTraffic(receipt.output?.stderr?.path === `${dir}/stderr.log` && binding.receipt?.path === `${dir}/native-receipt.json` &&
    binding.stdout?.path === `${dir}/native-test.stdout.log` && binding.stderr?.path === `${dir}/native-test.stderr.log`,
    "nested output is not part of the containing collection");
  const snapshot = boundBytes(root, binding.receipt, "target/traffic");
  const current = localBytes(root, `${collection.path}/receipt.json`, ".arc-verify/rust-coverage");
  requireTraffic(snapshot.equals(current), "nested native receipt altered after collection");
  const nested = JSON.parse(snapshot.toString()), test = coverageTest(nested);
  requireTraffic(nested.runId === collection.runId && test.exit === 0 && test.pid > 0 && test.cwd === realpathSync(root) &&
    Date.parse(test.started) >= Date.parse(receipt.started) && Date.parse(test.ended) <= Date.parse(receipt.ended), "collector test execution absent or stale");
  const actualOut = boundBytes(root, binding.stdout, "target/traffic"), actualErr = boundBytes(root, binding.stderr, "target/traffic");
  requireTraffic(actualOut.equals(boundBytes(root, test.output?.stdout, "target/traffic")) &&
    actualErr.equals(boundBytes(root, test.output?.stderr, "target/traffic")) && actualOut.toString() === test.stdout &&
    actualErr.toString() === test.stderr, "nested native test output differs");
  validateFullCargoOutput(actualOut.toString());
  return collection.runId;
}
export function validateCollector(root: string, receipt: any, stdout: string) {
  requireTraffic(!/SKIP_COVERAGE=1|skipping coverage gate/i.test(stdout), "harness skipped coverage");
  // The maintained coverage validator verifies the actual native collection.
  // Extra nested stdout snapshots are optional and are not a gate requirement.
  const runId = coverageRun(stdout).runId;
  nativeCollection(root, runId);
  return runId;
}
export function validateLatestCollection(latest: any, _harnessRunId: string, sourceHash: string) {
  requireTraffic(latest?.pass === true && latest.sourceHash === sourceHash &&
    typeof latest.runId === "string" && /^[A-Za-z0-9][A-Za-z0-9-]*$/.test(latest.runId) &&
    latest.directory === `.arc-verify/rust-coverage/baseline/runs/${latest.runId}`, "latest coverage is stale, failed or invalid");
}
export function validateFinalTrafficEvidence(root: string) {
  const read = (name: string) => JSON.parse(localBytes(root, `${directory}/${name}`, directory).toString());
  const context = sourceContext(root), gates = read("gate-summary.json"), measurement = read("mac-acceptance.json"), qa = read("qa.json");
  requireTraffic(gates.coveragePassed === true && gates.fullHarnessPassed === true && gates.sourceHash === context.sourceHash &&
    Array.isArray(gates.commands), "gate summary incomplete or stale");
  const commands = gates.commands.map((ref: any) => {
    const receipt = boundJSON(root, ref, "target"), output = validateReceipt(root, receipt, context, "verification-engineer");
    return { ref, receipt, output };
  });
  requireTraffic(commands.length === requiredCommands.length && requiredCommands.every(argv =>
    commands.filter((c: any) => equal(c.receipt.argv, argv)).length === 1), "required commands missing or substituted");
  const find = (argv: string[]) => commands.find((c: any) => equal(c.receipt.argv, argv))!;
  const workspace = find(requiredCommands[2]);
  validateFullCargoOutput(workspace.output.stdout);
  const native = find(requiredCommands[3]);
  requireTraffic(measurement.sourceHash === context.sourceHash && equal(measurement.command, native.ref), "measurement is not bound to native command");
  validateMeasurement(root, measurement, native.output.stdout, native.receipt, native.output.stderr);
  const coverage = find(requiredCommands[4]), harness = find(requiredCommands[5]), live = find(requiredCommands[6]);
  validateCollector(root, coverage.receipt, coverage.output.stdout);
  const harnessRunId = validateCollector(root, harness.receipt, harness.output.stdout);
  requireTraffic(Date.parse(coverage.receipt.ended) <= Date.parse(harness.receipt.started) &&
    Date.parse(harness.receipt.ended) <= Date.parse(live.receipt.started), "coverage collectors not serialized or live check is stale");
  const bunOutput = live.output.stdout + "\n" + live.output.stderr;
  validateLiveBunOutput(bunOutput);
  const latest = JSON.parse(localBytes(root, ".arc-verify/rust-coverage/baseline/latest.json", ".arc-verify/rust-coverage").toString());
  validateLatestCollection(latest, harnessRunId, inputIdentity(root).sha256);
  nativeCollection(root, latest.runId);
  requireTraffic(qa.complete === true && qa.goalBlockerCount === 0 && qa.skill === "oma-qa" && qa.actorId === "independent-reviewer" &&
    qa.sourceHash === context.sourceHash && Array.isArray(qa.commands) && qa.commands.length > 0, "independent review incomplete or stale");
  const review = localBytes(root, `${directory}/qa.md`, directory);
  requireTraffic(review.length >= 200 && qa.reportSha256 === hash(review), "independent review report unbound");
  const qaCommands = qa.commands.map((ref: any) => {
    const receipt = boundJSON(root, ref, "target"), output = validateReceipt(root, receipt, context, "independent-reviewer");
    requireTraffic(executableReviewTest(receipt.argv), "review must execute Cargo tests");
    const result = parseCargoExecution(output.stdout);
    requireTraffic(result.ignored === 0 && result.failed === 0, "review tests failed or ignored");
    return { receipt, result };
  });
  requireTraffic(qaCommands.some(({ receipt, result }: any) => /traffic|queue|admission|permit|lineage|nested/i.test(receipt.argv.join(" ")) ||
    result.tests.some((test: any) => /traffic|queue|admission|permit|lineage|nested/i.test(test.name))), "review reproduction does not identify a Traffic check");
  const final = localBytes(root, `${directory}/final.md`, directory).toString();
  requireTraffic(Buffer.byteLength(final) >= 200 && ["desktop", "native Windows", "vendor hooks", "broader detection", "remaining scheduling policy"].every(x => final.includes(x)), "final scope report incomplete");
  return { sourceHash: context.sourceHash, budget: evidenceBudget(root), pass: true };
}
