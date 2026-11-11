import { lstatSync, readFileSync, realpathSync } from "node:fs";
import { resolve } from "node:path";
import { hash } from "../../coverage/source";

function requireValue(condition: unknown, message: string): asserts condition {
  if (!condition) throw Error(`MANAGED_EXEC_EVIDENCE: ${message}`);
}
// Standard libtest text with identities is deliberate: quiet/list/compile modes
// cannot provide the execution proof required by independent review.
export function parseCargoExecution(stdout: string) {
  const tests: { harness: number; name: string; status: string }[] = [];
  let block: { count: number; tests: typeof tests } | undefined;
  let harness = 0, passed = 0, ignored = 0;
  for (const line of stdout.split(/\r?\n/)) {
    const running = line.match(/^running (\d+) tests?$/);
    if (running) {
      requireValue(!block, "test harness missing successful summary");
      block = { count: Number(running[1]), tests: [] }; continue;
    }
    const identity = line.match(/^test (.+) \.\.\. (ok|FAILED|ignored)(?:, .*)?$/);
    if (identity) {
      requireValue(block && !block.tests.some(t => t.name === identity[1]), "unmatched/duplicate test identity");
      block.tests.push({ harness, name: identity[1], status: identity[2] }); continue;
    }
    const summary = line.match(/^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out; finished in .+$/);
    if (summary) {
      requireValue(block && summary[1] === "ok" && Number(summary[3]) === 0 && Number(summary[5]) === 0,
        "unsuccessful or unmatched test summary");
      const p = block.tests.filter(t => t.status === "ok").length;
      const i = block.tests.filter(t => t.status === "ignored").length;
      requireValue(p === Number(summary[2]) && i === Number(summary[4]) && block.tests.length === block.count &&
        p + i === block.count, "executed identities/counts differ from test summary");
      tests.push(...block.tests); passed += p; ignored += i; harness++; block = undefined;
    }
  }
  requireValue(!block && harness > 0 && passed > 0, "review must execute and pass positive test count");
  return { executed: passed, passed, failed: 0, ignored, harnesses: harness, tests };
}

export function executableReviewTest(argv: unknown): argv is string[] {
  if (!Array.isArray(argv) || argv.length < 2 || !argv.every(a => typeof a === "string") ||
    argv[0] !== "cargo" || argv[1] !== "test") return false;
  const forbidden = new Set(["--help", "-h", "--version", "-V", "--no-run", "--list", "--skip"]);
  return !argv.slice(2).some(a => forbidden.has(a.split("=", 1)[0]));
}
function selectors(argv: string[]) {
  const cargoValues = new Set(["-p", "--package", "--manifest-path", "--test", "--bin", "--example", "--bench",
    "--features", "--exclude", "--target", "--target-dir", "--profile", "--jobs", "-j", "--color", "--message-format", "--config"]);
  const testValues = new Set(["--test-threads", "--format", "--color", "--logfile"]);
  const result: string[] = [];
  let libtest = false;
  for (let i = 2; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === "--") { libtest = true; continue; }
    if ((libtest ? testValues : cargoValues).has(arg)) { i++; continue; }
    if (!arg.startsWith("-")) result.push(arg);
  }
  return result;
}
function outputBytes(root: string, output: any, stream: "stdout" | "stderr") {
  if (typeof output?.[stream] === "string") {
    requireValue(output[`${stream}Path`] === undefined, "ambiguous execution output binding");
    return Buffer.from(output[stream]);
  }
  const path = output?.[`${stream}Path`];
  requireValue(typeof path === "string" && /^(?:target\/managed-exec\/|\.arc-verify\/managed-exec\/)/.test(path) &&
    !path.split("/").some(p => p === ".." || p === "."), "missing or unsafe execution output artifact");
  const file = resolve(root, path), stat = lstatSync(file);
  requireValue(stat.isFile() && !stat.isSymbolicLink() && realpathSync(file) === resolve(realpathSync(root), path),
    "execution output must be a regular local file");
  return readFileSync(file);
}
export function validateCargoExecution(command: any, root: string) {
  requireValue(executableReviewTest(command.argv), "missing independent deterministic Cargo reproduction");
  const stdout = outputBytes(root, command.output, "stdout"), stderr = outputBytes(root, command.output, "stderr");
  requireValue(hash(stdout) === command.stdoutSha256 && hash(stderr) === command.stderrSha256, "execution output/hash mismatch");
  const actual = parseCargoExecution(stdout.toString());
  requireValue(JSON.stringify(command.execution) === JSON.stringify(actual), "execution counts/identities differ from bound output");
  const exact = command.argv.includes("--exact");
  for (const selector of selectors(command.argv)) requireValue(actual.tests.some(t => t.status === "ok" &&
    (exact ? t.name === selector : t.name.includes(selector))), "unmatched executed test selector");
  return actual;
}
