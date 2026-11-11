import { spawn } from "node:child_process";
import { chmodSync, existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, realpathSync, writeFileSync } from "node:fs";
import { relative, resolve } from "node:path";
import { gitBytes, indexEntries, prohibitedReasons } from "./hygiene";
import { hash, inputIdentity, walk } from "../../coverage/source";
import { allowance, boundBytes, boundJSON, captureCoverage, coverageRun, coverageTest, equal, localBytes, nativeHost, requireTraffic, trafficInputs } from "./traffic-receipts";
import { PINS } from "../../coverage/config";
import { validateCollector, validateFullCargoOutput, validateLatestCollection, validateLiveBunOutput, evidenceBudget as oldEvidenceBudget } from "./traffic-evidence";
import { executableReviewTest, parseCargoExecution } from "../harness/managed-exec-test-output";
import { nativeCollection } from "../harness/exec-fail-open-coverage";
import { casesFromOutput, deriveOrders } from "./traffic-bun-measurement";
import { validateQAIndependentHandoff } from "./traffic-bun-qa";

export const acceptedHead = "16772fe2aa350413225f3fb2bb9227d14ae5a6c4";
export const acceptedTree = "1834e0cccb50a12fea299618f9a360f4ca10e03f";
export const directory = ".arc-verify/traffic-bun";
export const binaryCommand = ["cargo", "test", "-p", "arc-cli", "--tests", "--message-format=json"];
export const nativeCommand = ["cargo", "test", "-p", "arc-daemon", "--test", "traffic_cli", "--message-format=json", "--", "--nocapture"];
export const requiredCommands = [
  ["cargo", "fmt", "--all", "--", "--check"],
  ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
  ["cargo", "test", "--workspace"], binaryCommand, nativeCommand,
  ["bun", "run", "test:coverage"], ["./scripts/harness/validate.sh"],
  ["bun", "test", "scripts/coverage/evidence.test.ts"],
];
const actors = ["measurement-engineer", "verification-engineer", "chief-director", "independent-reviewer"];
const isRef = (v: unknown) => typeof v === "string" && /^[a-f0-9]{40}$/.test(v);
export const finalArtifacts = ["gate-summary.json", "mac-acceptance.json", "qa.json", "qa.md"].map(name => `${directory}/${name}`);

/** Add delivered mission executables without changing the historical Traffic identity. */
export function missionInputs(root: string) {
  root = realpathSync(root);
  const base = trafficInputs(root);
  const delivered = walk(resolve(root, directory)).filter(path => {
    const stat = lstatSync(path);
    return /\.(?:py|sh|bash|zsh|js|mjs|cjs|ts|rs)$/i.test(path) || (stat.mode & 0o111) !== 0;
  }).map(path => relative(root, path));
  const paths = [...new Set([`${directory}/restore-predecessor.py`, ...delivered])].sort();
  const extra = Object.fromEntries(paths.map(path => {
    const file = resolve(root, path), stat = lstatSync(file);
    requireTraffic(stat.isFile() && !stat.isSymbolicLink() && realpathSync(file) === file, `nonregular mission source: ${path}`);
    return [path, {sha256: hash(readFileSync(file)), mode: stat.mode & 0o777}];
  }));
  const all = {...base.files, ...extra};
  const files = Object.fromEntries(Object.keys(all).sort().map(path => [path, all[path]]));
  return {sha256: hash(JSON.stringify(files)), files};
}

export function sourceContext(root: string) {
  root = realpathSync(root);
  const head = gitBytes(root, ["rev-parse", "HEAD"]).toString().trim();
  const tree = gitBytes(root, ["rev-parse", `${head}^{tree}`]).toString().trim();
  requireTraffic(gitBytes(root, ["rev-parse", `${acceptedHead}^{tree}`]).toString().trim() === acceptedTree &&
    Bun.spawnSync(["git", "merge-base", "--is-ancestor", acceptedHead, head], { cwd: root }).exitCode === 0,
    "accepted baseline/tree or ancestry differs");
  return { acceptedHead, acceptedTree, head, tree, sourceHash: missionInputs(root).sha256, host: nativeHost() };
}

/** A later evidence-only commit is valid if the measured inputs are unchanged. */
export function validateRecordedSource(root: string, recorded: any, context: ReturnType<typeof sourceContext>) {
  requireTraffic(recorded?.acceptedHead === context.acceptedHead && recorded.acceptedTree === context.acceptedTree &&
    isRef(recorded.head) && isRef(recorded.tree) && isRef(context.head) && isRef(context.acceptedHead) &&
    typeof recorded.sourceHash === "string" && /^[a-f0-9]{64}$/.test(recorded.sourceHash) &&
    recorded.sourceHash === context.sourceHash, "recorded source inputs or baseline differ");
  requireTraffic(gitBytes(root, ["rev-parse", `${context.acceptedHead}^{tree}`]).toString().trim() === context.acceptedTree &&
    gitBytes(root, ["rev-parse", `${recorded.head}^{tree}`]).toString().trim() === recorded.tree &&
    gitBytes(root, ["rev-parse", `${context.head}^{tree}`]).toString().trim() === context.tree &&
    Bun.spawnSync(["git", "merge-base", "--is-ancestor", context.acceptedHead, recorded.head], {cwd: root}).exitCode === 0 &&
    Bun.spawnSync(["git", "merge-base", "--is-ancestor", recorded.head, context.head], {cwd: root}).exitCode === 0,
    "recorded revision, tree or ancestry differs");
}
function commandExecutable(root: string, program: string) {
  const path = program.includes("/") ? resolve(root, program) : Bun.which(program);
  requireTraffic(path && existsSync(path), "command executable unavailable");
  const canonical = realpathSync(path), stat = lstatSync(canonical);
  requireTraffic(stat.isFile() && (stat.mode & 0o111) !== 0, "command executable is not executable");
  return { path: canonical, sha256: hash(readFileSync(canonical)) };
}

// The harness consumes this exact stdout from git ls-files. Capture it in the
// same invocation, while passing all other Git commands through unchanged.
const harnessGitWrapper = `#!/bin/sh
if [ "$#" -eq 1 ] && [ "$1" = "ls-files" ] && [ ! -e "$ARC_TRAFFIC_BUN_INVENTORY" ]; then
  (set -C; : > "$ARC_TRAFFIC_BUN_INVENTORY") || exit 125
  "$ARC_TRAFFIC_BUN_REAL_GIT" "$@" > "$ARC_TRAFFIC_BUN_INVENTORY" || exit 125
  /bin/cat "$ARC_TRAFFIC_BUN_INVENTORY" || exit 125
  exit 0
fi
exec "$ARC_TRAFFIC_BUN_REAL_GIT" "$@"
`;

function inventoryRef(root: string, path: string, bytes: Buffer) {
  writeFileSync(resolve(root, path), bytes, {flag: "wx"});
  return {path, sha256: hash(bytes)};
}

export function beginHarnessInventory(root: string, runDir: string) {
  requireTraffic(/^target\/traffic-bun\/commands\/[a-z0-9-]+$/.test(runDir), "invalid harness inventory directory");
  const wrapperDir = `${runDir}/harness-bin`, wrapperPath = `${wrapperDir}/git`;
  mkdirSync(resolve(root, wrapperDir));
  const before = inventoryRef(root, `${runDir}/tracked-before.txt`, gitBytes(root, ["ls-files"]));
  const wrapper = inventoryRef(root, wrapperPath, Buffer.from(harnessGitWrapper));
  chmodSync(resolve(root, wrapperPath), 0o700);
  const gitExecutable = commandExecutable(root, "git");
  return {before, wrapper, gitExecutable, env: {
    PATH: `${resolve(root, wrapperDir)}:${process.env.PATH ?? ""}`,
    ARC_TRAFFIC_BUN_REAL_GIT: gitExecutable.path,
    ARC_TRAFFIC_BUN_INVENTORY: resolve(root, runDir, "tracked-actual.txt"),
  }};
}

export function finishHarnessInventory(root: string, runDir: string, sourceHash: string, prepared: ReturnType<typeof beginHarnessInventory>) {
  const after = inventoryRef(root, `${runDir}/tracked-after.txt`, gitBytes(root, ["ls-files"]));
  const actualPath = `${runDir}/tracked-actual.txt`;
  const actual = existsSync(resolve(root, actualPath)) ? {path: actualPath, sha256: hash(readFileSync(resolve(root, actualPath)))} : undefined;
  return {sourceHash, before: prepared.before, actual, after, wrapper: prepared.wrapper, gitExecutable: prepared.gitExecutable};
}

/** Check the inventory used by the harness and the current measured inputs. */
export function validateHarnessInventory(root: string, receipt: any, required: string[] = []) {
  const inputs = missionInputs(root);
  const requiredPaths = [...new Set([...Object.keys(inputs.files), ...required])];
  const binding = receipt?.trackedInventory, dir = receipt?.runDir;
  requireTraffic(receipt?.sourceHash === inputs.sha256, "harness source identity differs");
  requireTraffic(binding?.sourceHash === receipt?.sourceHash && typeof dir === "string" &&
    binding.before?.path === `${dir}/tracked-before.txt` && binding.actual?.path === `${dir}/tracked-actual.txt` &&
    binding.after?.path === `${dir}/tracked-after.txt` && binding.wrapper?.path === `${dir}/harness-bin/git` &&
    equal(binding.gitExecutable, commandExecutable(root, "git")), "harness inventory binding is missing or inconsistent");
  const wrapper = boundBytes(root, binding.wrapper, "target/traffic-bun");
  requireTraffic(wrapper.toString() === harnessGitWrapper && (lstatSync(resolve(root, binding.wrapper.path)).mode & 0o111) !== 0,
    "harness Git capture wrapper differs");
  const before = boundBytes(root, binding.before, "target/traffic-bun"), actual = boundBytes(root, binding.actual, "target/traffic-bun"),
    after = boundBytes(root, binding.after, "target/traffic-bun");
  requireTraffic(before.equals(actual) && actual.equals(after), "harness tracked inventory changed or differs from actual output");
  const parse = (bytes: Buffer) => {
    const text = bytes.toString("utf8");
    requireTraffic(text.length > 0 && text.endsWith("\n") && Buffer.from(text).equals(bytes), "invalid tracked inventory");
    const paths = text.slice(0, -1).split("\n");
    requireTraffic(paths.every(path => path.length > 0) && new Set(paths).size === paths.length, "duplicate or empty tracked path");
    return paths;
  };
  const currentInventory = gitBytes(root, ["ls-files"]);
  const paths = parse(actual), live = parse(currentInventory);
  const set = new Set(paths), current = new Set(live);
  requireTraffic(requiredPaths.length > 0 && requiredPaths.every(path => set.has(path) && current.has(path)),
    "measured source or validator was untracked during the harness");
  requireTraffic(actual.equals(currentInventory), "tracked inventory changed after the harness");
}
function artifactName(argv: string[]) {
  return equal(argv, binaryCommand) ? "arcctl" : equal(argv, nativeCommand) ? "arc-daemon" : undefined;
}
const validBinaryTarget = (value: unknown) => typeof value === "string" &&
  /^target\/traffic-bun\/commands\/[a-z0-9-]+\/binaries$/.test(value);
export function compilerArtifact(root: string, stdout: string, name: string, binaryTarget: string) {
  requireTraffic(validBinaryTarget(binaryTarget), "invalid isolated binary target");
  const messages = stdout.split(/\r?\n/).flatMap(line => {
    try { return [JSON.parse(line)]; } catch { return []; }
  }).filter((item: any) => item.reason === "compiler-artifact" && item.target?.name === name &&
    equal(item.target?.kind, ["bin"]) && item.profile?.test === false && typeof item.executable === "string");
  requireTraffic(messages.length === 1, `Cargo did not identify exactly one ${name} executable`);
  const expected = resolve(root, binaryTarget, "debug", name), item = messages[0];
  requireTraffic(item.fresh === false && item.executable === expected && item.manifest_path === resolve(root, "crates", name === "arcctl" ? "arc-cli" : "arc-daemon", "Cargo.toml") &&
    realpathSync(expected) === expected, "Cargo artifact target or package differs");
  const stat = lstatSync(expected);
  requireTraffic(stat.isFile() && !stat.isSymbolicLink() && (stat.mode & 0o111) !== 0, "Cargo artifact is not an executable file");
  return { path: expected, sha256: hash(readFileSync(expected)) };
}
export function validateReceipt(root: string, receipt: any, context = sourceContext(root), actor?: string) {
  requireTraffic(receipt?.schemaVersion === 1 && receipt.kind === "traffic-bun-command" && receipt.fixtureOnly === false &&
    receipt.sourceHashAfter === context.sourceHash &&
    equal(receipt.host, context.host) && context.host.platform === "darwin", "stale, fixture-only, or wrong source/host receipt");
  validateRecordedSource(root, receipt, context);
  requireTraffic(actors.includes(receipt.actorId) && (!actor || receipt.actorId === actor) && receipt.cwd === "." &&
    Array.isArray(receipt.argv) && receipt.argv.length > 0 && receipt.argv.every((a: unknown) => typeof a === "string") &&
    !receipt.argv.some((a: string) => /^--(?:skip|ignored|no-run|list)(?:=|$)/.test(a)) && receipt.skipFlags === false &&
    receipt.status === "passed" && receipt.exit === 0 && Number.isSafeInteger(receipt.pid) && receipt.pid > 0,
    "command failed, skipped or has an invalid actor");
  requireTraffic(typeof receipt.runDir === "string" && /^target\/traffic-bun\/commands\/[a-z0-9-]+$/.test(receipt.runDir) &&
    receipt.output?.stdout?.path === `${receipt.runDir}/stdout.log` &&
    receipt.output?.stderr?.path === `${receipt.runDir}/stderr.log`, "command output does not belong to its run");
  requireTraffic(Number.isFinite(Date.parse(receipt.started)) && Number.isFinite(Date.parse(receipt.ended)) &&
    Date.parse(receipt.ended) >= Date.parse(receipt.started) && Number.isFinite(receipt.elapsedMs) && receipt.elapsedMs >= 0 &&
    Number.isSafeInteger(receipt.timeoutAllowanceMs) && receipt.timeoutAllowanceMs >= allowance &&
    equal(receipt.executable, commandExecutable(root, receipt.argv[0])) && receipt.environment === undefined &&
    receipt.stdout === undefined && receipt.stderr === undefined, "invalid timing, executable or raw receipt data");
  const output = { stdout: boundBytes(root, receipt.output?.stdout, "target/traffic-bun").toString(),
    stderr: boundBytes(root, receipt.output?.stderr, "target/traffic-bun").toString() };
  if (equal(receipt.argv, ["./scripts/harness/validate.sh"])) validateHarnessInventory(root, receipt);
  else requireTraffic(receipt.trackedInventory === undefined, "unexpected harness inventory");
  const name = artifactName(receipt.argv);
  if (name) requireTraffic(validBinaryTarget(receipt.binaryTarget) &&
    (name !== "arcctl" || receipt.binaryTarget === `${receipt.runDir}/binaries` && receipt.binaryPreparation === undefined) &&
    (name !== "arc-daemon" || receipt.binaryPreparation?.path === `${receipt.binaryTarget.slice(0, -"/binaries".length)}/receipt.json`) &&
    equal(receipt.productBinary, compilerArtifact(root, output.stdout, name, receipt.binaryTarget)), "source-bound Cargo executable differs");
  else requireTraffic(receipt.productBinary === undefined && receipt.binaryTarget === undefined && receipt.binaryPreparation === undefined, "unexpected executable binding");
  return output;
}
function commandReceipt(root: string, ref: any, context: ReturnType<typeof sourceContext>, actor?: string) {
  const receipt = boundJSON(root, ref, "target/traffic-bun");
  const output = validateReceipt(root, receipt, context, actor);
  requireTraffic(ref.path === `${receipt.runDir}/receipt.json`, "command receipt is outside its run");
  return { receipt, output };
}

/** Run one exact command; raw outputs and native test artifacts stay under target. */
export async function capture(root: string, name: string, argv: string[], actorId: string, binaryPreparation?: any) {
  root = realpathSync(root);
  requireTraffic(/^[a-z0-9-]+$/.test(name) && Array.isArray(argv) && argv.length > 0 && actors.includes(actorId), "invalid capture request");
  requireTraffic(!Object.entries(process.env).some(([key, value]) => /^(?:SKIP_|ARC_.*SKIP)/.test(key) && value && value !== "0"), "skip environment present");
  requireTraffic(!argv.some(a => /^--(?:skip|ignored|no-run|list)(?:=|$)/.test(a)), "skip option present");
  const before = sourceContext(root), executable = commandExecutable(root, argv[0]);
  const dir = `target/traffic-bun/commands/${name}-${crypto.randomUUID()}`;
  mkdirSync(resolve(root, "target/traffic-bun/commands"), { recursive: true });
  mkdirSync(resolve(root, dir));
  let binaryTarget: string | undefined;
  if (equal(argv, binaryCommand)) {
    requireTraffic(binaryPreparation === undefined, "binary preparation cannot reuse an existing target");
    binaryTarget = `${dir}/binaries`;
  } else if (equal(argv, nativeCommand)) {
    const preparation = commandReceipt(root, binaryPreparation, before);
    requireTraffic(equal(preparation.receipt.argv, binaryCommand), "native test requires a source-bound CLI preparation");
    binaryTarget = preparation.receipt.binaryTarget;
  } else requireTraffic(binaryPreparation === undefined, "unexpected binary preparation");
  const env = { ...process.env };
  delete env.ARC_TRAFFIC_BUN_RUN_DIR;
  if (binaryTarget) env.CARGO_TARGET_DIR = resolve(root, binaryTarget);
  if (equal(argv, nativeCommand)) env.ARC_TRAFFIC_BUN_RUN_DIR = resolve(root, dir, "observations");
  const inventoryPreparation = equal(argv, ["./scripts/harness/validate.sh"]) ? beginHarnessInventory(root, dir) : undefined;
  if (inventoryPreparation) Object.assign(env, inventoryPreparation.env);
  const start = performance.now(), started = new Date().toISOString(), out: Buffer[] = [], err: Buffer[] = [];
  const child = spawn(argv[0], argv.slice(1), { cwd: root, env, stdio: ["ignore", "pipe", "pipe"] });
  child.stdout.on("data", data => out.push(data)); child.stderr.on("data", data => err.push(data));
  const timer = setTimeout(() => child.kill("SIGTERM"), allowance);
  let exit: number;
  try { exit = await new Promise<number>((ok, bad) => { child.once("error", bad); child.once("close", code => ok(code ?? 1)); }); }
  finally { clearTimeout(timer); }
  const ended = new Date().toISOString(), after = sourceContext(root);
  const trackedInventory = inventoryPreparation ? finishHarnessInventory(root, dir, before.sourceHash, inventoryPreparation) : undefined;
  const output = Object.fromEntries([["stdout", Buffer.concat(out)], ["stderr", Buffer.concat(err)]].map(([stream, value]) => {
    const bytes = value as Buffer, path = `${dir}/${stream}.log`;
    writeFileSync(resolve(root, path), bytes, { flag: "wx" });
    return [stream, { path, sha256: hash(bytes) }];
  }));
  const productName = artifactName(argv);
  let productBinary: ReturnType<typeof compilerArtifact> | undefined;
  if (productName && exit === 0) productBinary = compilerArtifact(root, Buffer.concat(out).toString(), productName, binaryTarget!);
  let nestedCoverage: ReturnType<typeof captureCoverage> | undefined, nestedCoverageError: string | undefined;
  if (equal(argv, ["bun", "run", "test:coverage"]) || equal(argv, ["./scripts/harness/validate.sh"])) {
    try { nestedCoverage = captureCoverage(root, dir, Buffer.concat(out).toString()); }
    catch (error) { nestedCoverageError = String(error); }
  }
  const receipt = { schemaVersion: 1, kind: "traffic-bun-command", fixtureOnly: false, ...before, actorId, argv, cwd: ".", runDir: dir,
    started, ended, elapsedMs: performance.now() - start, pid: child.pid, exit, status: exit === 0 ? "passed" : "failed",
    sourceHashAfter: after.sourceHash, timeoutAllowanceMs: allowance, executable, skipFlags: false, output,
    ...(productName ? {binaryTarget, productBinary} : {}), ...(binaryPreparation ? {binaryPreparation} : {}),
    trackedInventory, nestedCoverage, nestedCoverageError };
  const path = `${dir}/receipt.json`, bytes = Buffer.from(JSON.stringify(receipt, null, 2) + "\n");
  writeFileSync(resolve(root, path), bytes, { flag: "wx" });
  requireTraffic(equal(before, after), "source, revision or host changed during command");
  return { path, sha256: hash(bytes), exit };
}

function nativeOutput(stdout: string) {
  validateFullCargoOutput(stdout);
  for (const order of ["cargo", "bun"]) for (const mode of ["bypass", "scheduled"]) {
    requireTraffic(stdout.includes(`bun_measurement::mixed_${order}_first_${mode}`), "native test identity missing");
  }
  return casesFromOutput(stdout);
}
function nativeDirectories(root: string, cases: any[], receipt: any) {
  for (const raw of cases) for (const job of raw.jobs ?? []) {
    requireTraffic(job.cwd === resolve(root, receipt.runDir, "observations", raw.id, job.tool),
      "measurement artifacts do not belong to the command run");
  }
}
export function validateBinaryBinding(cases: any[], native: any, preparation: any) {
  requireTraffic(equal(preparation.argv, binaryCommand) && equal(native.argv, nativeCommand) &&
    validBinaryTarget(preparation.binaryTarget) && preparation.binaryTarget === `${preparation.runDir}/binaries` &&
    native.binaryTarget === preparation.binaryTarget && native.binaryPreparation?.path === `${preparation.runDir}/receipt.json` &&
    Date.parse(preparation.ended) <= Date.parse(native.started), "binary preparation is missing, stale or unordered");
  for (const raw of cases) {
    requireTraffic(equal(raw.daemon?.executable, native.productBinary), "observed daemon differs from Cargo artifact");
    if (raw.mode === "scheduled") for (const job of raw.jobs ?? []) {
      requireTraffic(equal(job.process?.wrapperExecutable, preparation.productBinary), "observed CLI differs from Cargo artifact");
    }
  }
}
export function measurementFromReceipt(root: string, ref: any, binaryPreparation: any) {
  const context = sourceContext(root), { receipt, output } = commandReceipt(root, ref, context);
  const preparation = commandReceipt(root, binaryPreparation, context);
  requireTraffic(["measurement-engineer", "verification-engineer", "chief-director"].includes(receipt.actorId) &&
    equal(receipt.argv, nativeCommand), "measurement command or actor differs");
  const cases = nativeOutput(output.stdout);
  validateFullCargoOutput(preparation.output.stdout);
  requireTraffic(equal(receipt.binaryPreparation, binaryPreparation), "native command used a different binary preparation");
  validateBinaryBinding(cases, receipt, preparation.receipt);
  nativeDirectories(root, cases, receipt);
  const orders = deriveOrders(root, cases, receipt);
  const path = `${ref.path.slice(0, -"receipt.json".length)}observations.json`;
  const bytes = Buffer.from(JSON.stringify({ schemaVersion: 1, fixtureOnly: false, cases }, null, 2) + "\n");
  if (existsSync(resolve(root, path))) requireTraffic(readFileSync(resolve(root, path)).equals(bytes), "existing observations differ");
  else writeFileSync(resolve(root, path), bytes, { flag: "wx" });
  return { schemaVersion: 1, platform: "macOS", acceptedHead: receipt.acceptedHead, acceptedTree: receipt.acceptedTree,
    head: receipt.head, tree: receipt.tree, sourceHash: receipt.sourceHash, host: receipt.host, command: ref, binaryPreparation,
    observations: { path, sha256: hash(bytes) }, orders };
}
export function validateMeasurement(root: string, summary: any, context = sourceContext(root)) {
  requireTraffic(summary?.schemaVersion === 1 && summary.platform === "macOS", "measurement source differs");
  validateRecordedSource(root, summary, context);
  const { receipt, output } = commandReceipt(root, summary.command, context);
  requireTraffic(summary.head === receipt.head && summary.tree === receipt.tree && summary.sourceHash === receipt.sourceHash,
    "measurement and command revisions differ");
  const preparation = commandReceipt(root, summary.binaryPreparation, context);
  requireTraffic(equal(receipt.binaryPreparation, summary.binaryPreparation), "native command used a different binary preparation");
  requireTraffic(["measurement-engineer", "verification-engineer", "chief-director"].includes(receipt.actorId) &&
    equal(receipt.argv, nativeCommand), "measurement command or actor differs");
  requireTraffic(summary.observations?.path === `${receipt.runDir}/observations.json`, "observation file is outside its run");
  const data = boundJSON(root, summary.observations, "target/traffic-bun");
  requireTraffic(data?.schemaVersion === 1 && data.fixtureOnly === false && equal(data.cases, nativeOutput(output.stdout)),
    "raw observations differ from executed native output");
  validateFullCargoOutput(preparation.output.stdout);
  validateBinaryBinding(data.cases, receipt, preparation.receipt);
  nativeDirectories(root, data.cases, receipt);
  const orders = deriveOrders(root, data.cases, receipt);
  requireTraffic(equal(summary.orders, orders), "summary differs from raw observations");
  return orders;
}

export function evidenceBudget(root: string, mandatory: string[] = []) {
  const files: string[] = [];
  const visit = (path: string) => {
    for (const name of readdirSync(resolve(root, path))) {
      const next = `${path}/${name}`, stat = lstatSync(resolve(root, next));
      requireTraffic(!stat.isSymbolicLink(), "symlink in mission evidence");
      if (stat.isDirectory()) visit(next);
      else { requireTraffic(stat.isFile(), "nonregular mission evidence"); files.push(next); }
    }
  };
  visit(directory);
  const indexed = indexEntries(root, directory), paths = files.sort();
  requireTraffic(mandatory.every(path => indexed.some(entry => entry.path === path)), "mandatory final evidence is not indexed");
  requireTraffic(paths.length > 0 && paths.length < 30, "mission evidence count must be below 30");
  let size = 0;
  for (const path of paths) {
    const data = localBytes(root, path, path.slice(0, path.lastIndexOf("/")));
    requireTraffic(prohibitedReasons(path, data).length === 0, `prohibited evidence: ${path}`);
    size += data.length;
  }
  requireTraffic(size < 5_000_000, "mission evidence must be below 5000000 decimal bytes");
  requireTraffic(indexed.length < 30, "indexed mission evidence count must be below 30");
  let indexedSize = 0;
  for (const entry of indexed) {
    requireTraffic(["100644", "100755"].includes(entry.mode) && paths.includes(entry.path), `indexed evidence missing or nonregular: ${entry.path}`);
    const data = gitBytes(root, ["cat-file", "blob", entry.oid]);
    indexedSize += data.length;
    const mode = (lstatSync(resolve(root, entry.path)).mode & 0o111) !== 0 ? "100755" : "100644";
    requireTraffic(mode === entry.mode && prohibitedReasons(entry.path, data).length === 0 &&
      data.equals(localBytes(root, entry.path, entry.path.slice(0, entry.path.lastIndexOf("/")))),
      `indexed and working evidence differ: ${entry.path}`);
  }
  requireTraffic(indexedSize < 5_000_000, "indexed mission evidence must be below 5000000 decimal bytes");
  return { files: paths.length, bytes: size, indexed: indexed.length, indexedBytes: indexedSize };
}

export function validateGateSummary(gates: any, sourceHash: string) {
  requireTraffic(gates?.coveragePassed === true && gates.fullHarnessPassed === true && gates.sourceHash === sourceHash &&
    Array.isArray(gates.commands), "gate summary is incomplete or stale");
}
export function validateCommandSet(commands: any[]) {
  requireTraffic(commands.length === requiredCommands.length && requiredCommands.every(argv =>
    commands.filter((c: any) => equal(c.receipt.argv, argv)).length === 1), "required commands missing or substituted");
}

/** Bind the collector's actual workspace test and its raw output to this run. */
export function validateNestedCoverage(root: string, receipt: any, stdout: string) {
  const collection = coverageRun(stdout), binding = receipt?.nestedCoverage;
  requireTraffic(binding?.runId === collection.runId && !receipt.nestedCoverageError &&
    typeof receipt.runDir === "string" && binding.receipt?.path === `${receipt.runDir}/native-receipt.json` &&
    binding.stdout?.path === `${receipt.runDir}/native-test.stdout.log` &&
    binding.stderr?.path === `${receipt.runDir}/native-test.stderr.log`, "nested coverage binding missing or outside its run");
  const snapshot = boundBytes(root, binding.receipt, "target/traffic-bun");
  requireTraffic(snapshot.equals(localBytes(root, `${collection.path}/receipt.json`, ".arc-verify/rust-coverage")),
    "nested coverage receipt differs from collection");
  const nested = JSON.parse(snapshot.toString()), test = coverageTest(nested);
  const expected = ["rustup", "run", PINS.rust, "cargo", "llvm-cov", "--workspace", "--all-targets", "--locked", "--offline", "--no-report", "--no-cfg-coverage", "--no-cfg-coverage-nightly"];
  const start = Date.parse(test.started), end = Date.parse(test.ended);
  requireTraffic(nested.runId === collection.runId && nested.pass === true && nested.exit === 0 &&
    equal(test.argv, expected) && test.exit === 0 && Number.isSafeInteger(test.pid) && test.pid > 0 && test.cwd === realpathSync(root) &&
    Number.isFinite(start) && Number.isFinite(end) && start >= Date.parse(receipt.started) && end <= Date.parse(receipt.ended) && start <= end,
    "nested coverage test command, outcome or timing differs");
  const out = boundBytes(root, binding.stdout, "target/traffic-bun"), err = boundBytes(root, binding.stderr, "target/traffic-bun");
  requireTraffic(out.equals(boundBytes(root, test.output?.stdout, "target/traffic")) &&
    err.equals(boundBytes(root, test.output?.stderr, "target/traffic")) && out.toString() === test.stdout && err.toString() === test.stderr,
    "nested coverage test bytes differ");
  validateFullCargoOutput(out.toString());
  return collection.runId;
}

export function validateFinalEvidence(root: string) {
  const read = (name: string) => JSON.parse(localBytes(root, `${directory}/${name}`, directory).toString());
  const budget = evidenceBudget(root, finalArtifacts);
  const context = sourceContext(root), gates = read("gate-summary.json"), measurement = read("mac-acceptance.json"), qa = read("qa.json");
  validateGateSummary(gates, context.sourceHash);
  const commands = gates.commands.map((ref: any) => {
    const { receipt, output } = commandReceipt(root, ref, context);
    return { ref, receipt, output };
  });
  validateCommandSet(commands);
  const find = (argv: string[]) => commands.find((c: any) => equal(c.receipt.argv, argv))!;
  validateFullCargoOutput(find(requiredCommands[2]).output.stdout);
  requireTraffic(equal(find(nativeCommand).ref, measurement.command), "measurement command is not a gate receipt");
  requireTraffic(equal(find(binaryCommand).ref, measurement.binaryPreparation), "binary preparation is not a gate receipt");
  validateMeasurement(root, measurement, context);
  const coverage = find(requiredCommands[5]), harness = find(requiredCommands[6]), live = find(requiredCommands[7]);
  validateCollector(root, coverage.receipt, coverage.output.stdout);
  const harnessRun = validateCollector(root, harness.receipt, harness.output.stdout);
  validateNestedCoverage(root, coverage.receipt, coverage.output.stdout);
  requireTraffic(validateNestedCoverage(root, harness.receipt, harness.output.stdout) === harnessRun, "harness nested run differs");
  requireTraffic(!/SKIP_COVERAGE=1|skipping coverage gate/i.test(`${coverage.output.stderr}\n${harness.output.stderr}`), "collector reported a skip");
  requireTraffic(Date.parse(coverage.receipt.ended) <= Date.parse(harness.receipt.started) &&
    Date.parse(harness.receipt.ended) <= Date.parse(live.receipt.started), "coverage, harness or live check not serialized");
  validateLiveBunOutput(`${live.output.stdout}\n${live.output.stderr}`);
  const latest = JSON.parse(localBytes(root, ".arc-verify/rust-coverage/baseline/latest.json", ".arc-verify/rust-coverage").toString());
  requireTraffic(latest.runId === harnessRun, "latest coverage is not the harness collection");
  validateLatestCollection(latest, harnessRun, inputIdentity(root).sha256);
  nativeCollection(root, latest.runId);
  requireTraffic(qa.complete === true && qa.goalBlockerCount === 0 && qa.skill === "oma-qa" && qa.actorId === "independent-reviewer" &&
    qa.sourceHash === context.sourceHash && Array.isArray(qa.commands) && qa.commands.length > 0, "independent QA incomplete or stale");
  validateQAIndependentHandoff(root, qa);
  const policy = commandReceipt(root, qa.independentRuns.freshSnapshotPolicy.receipt, context, "independent-reviewer");
  requireTraffic(executableReviewTest(policy.receipt.argv), "independent policy receipt did not run Cargo tests");
  const policyResult = parseCargoExecution(policy.output.stdout);
  requireTraffic(policyResult.passed === qa.independentRuns.freshSnapshotPolicy.passed && policyResult.failed === 0 &&
    policyResult.ignored === 0 && (/policy/i.test(policy.receipt.argv.join(" ")) ||
      policyResult.tests.some(test => /policy/i.test(test.name))), "independent policy results differ from receipt");
  const report = localBytes(root, `${directory}/qa.md`, directory);
  requireTraffic(report.length >= 200 && report.toString().includes("oma-qa") && qa.reportSha256 === hash(report), "QA report unbound");
  const review = qa.commands.map((ref: any) => {
    const { receipt, output } = commandReceipt(root, ref, context, "independent-reviewer");
    requireTraffic(executableReviewTest(receipt.argv), "QA did not run Cargo tests");
    const result = parseCargoExecution(output.stdout);
    requireTraffic(result.ignored === 0 && result.failed === 0, "QA tests failed or ignored");
    return { receipt, result };
  });
  requireTraffic(review.some(({receipt, result}: any) => /traffic|bun|queue|permit|lineage/i.test(receipt.argv.join(" ")) ||
    result.tests.some((test: any) => /traffic|bun|queue|permit|lineage/i.test(test.name))), "QA did not reproduce a Traffic check");
  return { sourceHash: context.sourceHash, budget, historicalBudget: oldEvidenceBudget(root), pass: true };
}

if (import.meta.main) {
  const root = resolve(import.meta.dir, "../../.."), [action, ...args] = process.argv.slice(2);
  if (action === "capture") {
    const [actor, name, ...command] = args;
    const binaryPreparation = command[0] === "--binary-preparation" ? {path: command[1], sha256: command[2]} : undefined;
    const argv = binaryPreparation ? command.slice(3) : command;
    const ref = await capture(root, name, argv, actor, binaryPreparation);
    console.log(JSON.stringify({path: ref.path, sha256: ref.sha256})); process.exitCode = ref.exit;
  } else if (action === "measurement") {
    requireTraffic(args.length === 4, "measurement needs native and binary preparation paths and hashes");
    console.log(JSON.stringify(measurementFromReceipt(root, { path: args[0], sha256: args[1] }, { path: args[2], sha256: args[3] }), null, 2));
  } else if (action === "validate") console.log(JSON.stringify(validateFinalEvidence(root)));
  else throw Error("usage: traffic-bun-evidence.ts capture ACTOR NAME [--binary-preparation PATH SHA] COMMAND... | measurement NATIVE_PATH NATIVE_SHA BINARY_PATH BINARY_SHA | validate");
}
