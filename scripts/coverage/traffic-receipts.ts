import { spawn } from "node:child_process";
import { existsSync, lstatSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { arch, hostname, platform, release } from "node:os";
import { relative, resolve } from "node:path";
import { gitBytes } from "./hygiene";
import { hash, walk } from "./source";

export const baselineHead = "4d2f71c1a1482925f6c9918296d7ceb3435b7592";
export const baselineTree = "4455896fdf976244313a70a9fa38b730b344b411";
export const allowance = 7_200_000;
export const equal = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);
export function requireTraffic(ok: unknown, message: string): asserts ok {
  if (!ok) throw Error(`TRAFFIC_EVIDENCE: ${message}`);
}
export const nativeHost = () => ({ platform: platform(), release: release(), arch: arch(), hostname: hostname() });
export const isDigest = (value: unknown) => typeof value === "string" && /^[a-f0-9]{64}$/.test(value);

export function localBytes(root: string, path: string, prefix: string) {
  requireTraffic(typeof path === "string" && path.startsWith(prefix + "/") && !/[\\\x00-\x1f\x7f]/.test(path) &&
    !path.split("/").some(part => !part || part === "." || part === ".."), "unsafe artifact path");
  const file = resolve(root, path), stat = lstatSync(file);
  requireTraffic(stat.isFile() && !stat.isSymbolicLink() && realpathSync(file) === resolve(realpathSync(root), path), "noncanonical artifact");
  return readFileSync(file);
}
export function boundBytes(root: string, ref: any, prefix: string) {
  requireTraffic(ref && isDigest(ref.sha256), "missing artifact hash");
  const bytes = localBytes(root, ref.path, prefix);
  requireTraffic(hash(bytes) === ref.sha256, `altered artifact: ${ref.path}`);
  return bytes;
}
export const boundJSON = (root: string, ref: any, prefix: string) => JSON.parse(boundBytes(root, ref, prefix).toString());
export function trafficInputs(root: string) {
  root = realpathSync(root);
  const required = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "mise.toml", "package.json", ".gitignore",
    ".arc-verify/rust-coverage/contract/accounting.json"];
  const optional = [".cargo/config", ".cargo/config.toml", "build.rs", "docs/managed-exec.md", "docs/rust-coverage.md"];
  const paths = [...required, ...optional.filter(p => existsSync(resolve(root, p))),
    ...["crates", "scripts/coverage", "scripts/harness"].flatMap(p => walk(resolve(root, p)).map(f => relative(root, f)))];
  const files = Object.fromEntries([...new Set(paths)].sort().map(path => {
    const file = resolve(root, path), stat = lstatSync(file);
    requireTraffic(stat.isFile() && !stat.isSymbolicLink() && realpathSync(file) === file, `nonregular source: ${path}`);
    return [path, { sha256: hash(readFileSync(file)), mode: stat.mode & 0o777 }];
  }));
  return { sha256: hash(JSON.stringify(files)), files };
}
export function sourceContext(root: string) {
  const head = gitBytes(root, ["rev-parse", "HEAD"]).toString().trim();
  const tree = gitBytes(root, ["rev-parse", `${head}^{tree}`]).toString().trim();
  requireTraffic(gitBytes(root, ["rev-parse", `${baselineHead}^{tree}`]).toString().trim() === baselineTree, "baseline tree differs");
  requireTraffic(Bun.spawnSync(["git", "merge-base", "--is-ancestor", baselineHead, head], { cwd: root }).exitCode === 0, "HEAD is not descended from baseline");
  return { baselineHead, baselineTree, head, tree, sourceHash: trafficInputs(root).sha256, host: nativeHost() };
}
export function validateReceipt(root: string, receipt: any, context: ReturnType<typeof sourceContext>, actor?: string) {
  requireTraffic(receipt?.schemaVersion === 1 && receipt.kind === "traffic-command" && receipt.fixtureOnly !== true &&
    receipt.baselineHead === baselineHead && receipt.baselineTree === baselineTree && receipt.sourceHash === context.sourceHash &&
    receipt.sourceHashAfter === context.sourceHash && equal(receipt.host, context.host) && context.host.platform === "darwin", "stale source, baseline or host");
  requireTraffic(/^[a-f0-9]{40}$/.test(receipt.head) && /^[a-f0-9]{40}$/.test(receipt.tree) &&
    gitBytes(root, ["rev-parse", `${receipt.head}^{tree}`]).toString().trim() === receipt.tree &&
    Bun.spawnSync(["git", "merge-base", "--is-ancestor", baselineHead, receipt.head], { cwd: root }).exitCode === 0, "unavailable or unrelated receipt revision");
  requireTraffic((!actor || receipt.actorId === actor) && receipt.cwd === "." && Array.isArray(receipt.argv) && receipt.argv.length > 0 &&
    receipt.argv.every((a: unknown) => typeof a === "string") && !receipt.argv.some((a: string) => /^--(?:skip|ignored|no-run|list)(?:=|$)/.test(a)) &&
    receipt.skipFlags === false && receipt.status === "passed" && receipt.exit === 0 && Number.isSafeInteger(receipt.pid) && receipt.pid > 0, "failed or skipped command");
  requireTraffic(Number.isFinite(Date.parse(receipt.started)) && Number.isFinite(Date.parse(receipt.ended)) &&
    Date.parse(receipt.ended) >= Date.parse(receipt.started) && Number.isFinite(receipt.elapsedMs) && receipt.elapsedMs >= 0 &&
    Number.isSafeInteger(receipt.timeoutAllowanceMs) && receipt.timeoutAllowanceMs >= allowance, "invalid command time or allowance");
  const executable = Bun.which(receipt.argv[0]);
  requireTraffic(executable && receipt.executable?.path === realpathSync(executable) && receipt.executable.sha256 === hash(readFileSync(executable)), "command executable differs");
  requireTraffic(receipt.environment === undefined && receipt.stdout === undefined && receipt.stderr === undefined, "raw output or environment in receipt");
  return { stdout: boundBytes(root, receipt.output?.stdout, "target").toString(),
    stderr: boundBytes(root, receipt.output?.stderr, "target").toString() };
}
export function coverageRun(stdout: string) {
  const found = [...stdout.matchAll(/^Coverage reports: (\.arc-verify\/rust-coverage\/baseline\/runs\/([A-Za-z0-9-]+))\r?$/gm)];
  requireTraffic(found.length === 1, "actual coverage collection missing");
  return { path: found[0][1], runId: found[0][2] };
}
export function coverageTest(receipt: any) {
  requireTraffic(Array.isArray(receipt?.receipts), "nested native receipts missing");
  const found = receipt.receipts.filter((r: any) => r.argv?.[0] === "rustup" && r.argv[3] === "cargo" && r.argv[4] === "llvm-cov" &&
    r.argv.includes("--workspace") && r.argv.includes("--all-targets") && r.argv.includes("--no-report"));
  requireTraffic(found.length === 1 && typeof found[0].stdout === "string" && typeof found[0].stderr === "string", "nested native test output missing or ambiguous");
  return found[0];
}
export function captureCoverage(root: string, dir: string, stdout: string) {
  const collection = coverageRun(stdout);
  const raw = localBytes(root, `${collection.path}/receipt.json`, ".arc-verify/rust-coverage");
  const test = coverageTest(JSON.parse(raw.toString()));
  const actualOut = boundBytes(root, test.output?.stdout, "target/traffic");
  const actualErr = boundBytes(root, test.output?.stderr, "target/traffic");
  requireTraffic(actualOut.toString() === test.stdout && actualErr.toString() === test.stderr, "native test text differs from captured bytes");
  const save = (name: string, bytes: Buffer) => {
    const path = `${dir}/${name}`;
    writeFileSync(resolve(root, path), bytes, { flag: "wx" });
    return { path, sha256: hash(bytes) };
  };
  return { runId: collection.runId, receipt: save("native-receipt.json", raw),
    stdout: save("native-test.stdout.log", actualOut), stderr: save("native-test.stderr.log", actualErr) };
}
export async function capture(root: string, name: string, argv: string[], actorId = "verification-engineer") {
  requireTraffic(/^[a-z0-9-]+$/.test(name) && argv.length > 0 && ["verification-engineer", "independent-reviewer"].includes(actorId), "invalid capture request");
  requireTraffic(!Object.entries(process.env).some(([key, value]) => /^(?:SKIP_|ARC_.*SKIP)/.test(key) && value && value !== "0"), "skip environment present");
  const before = sourceContext(root), executable = Bun.which(argv[0]);
  requireTraffic(executable, "command executable unavailable");
  const dir = `target/traffic/commands/${name}-${crypto.randomUUID()}`;
  mkdirSync(resolve(root, dir), { recursive: true });
  const started = new Date().toISOString(), start = performance.now(), out: Buffer[] = [], err: Buffer[] = [];
  const child = spawn(argv[0], argv.slice(1), { cwd: root, env: process.env, stdio: ["ignore", "pipe", "pipe"] });
  child.stdout.on("data", data => out.push(data)); child.stderr.on("data", data => err.push(data));
  const timer = setTimeout(() => child.kill("SIGTERM"), allowance);
  let exit: number;
  try { exit = await new Promise<number>((ok, bad) => { child.once("error", bad); child.once("close", code => ok(code ?? 1)); }); }
  finally { clearTimeout(timer); }
  const ended = new Date().toISOString(), after = sourceContext(root);
  const output = Object.fromEntries([["stdout", Buffer.concat(out)], ["stderr", Buffer.concat(err)]].map(([stream, bytes]) => {
    const path = `${dir}/${stream}.log`; writeFileSync(resolve(root, path), bytes as Buffer, { flag: "wx" });
    return [stream, { path, sha256: hash(bytes as Buffer) }];
  }));
  let nestedCoverage: ReturnType<typeof captureCoverage> | undefined, nestedCoverageError: string | undefined;
  if (equal(argv, ["bun", "run", "test:coverage"]) || equal(argv, ["./scripts/harness/validate.sh"])) {
    try { nestedCoverage = captureCoverage(root, dir, Buffer.concat(out).toString()); }
    catch (error) { nestedCoverageError = String(error); }
  }
  const receipt = { schemaVersion: 1, kind: "traffic-command", ...before, actorId, argv, cwd: ".", started, ended,
    elapsedMs: performance.now() - start, pid: child.pid, exit, status: exit === 0 ? "passed" : "failed", sourceHashAfter: after.sourceHash,
    timeoutAllowanceMs: allowance, executable: { path: realpathSync(executable), sha256: hash(readFileSync(executable)) }, skipFlags: false, output,
    nestedCoverage, nestedCoverageError };
  const path = `${dir}/receipt.json`, bytes = Buffer.from(JSON.stringify(receipt, null, 2) + "\n");
  writeFileSync(resolve(root, path), bytes, { flag: "wx" });
  requireTraffic(equal(before, after), "source, revision or host changed during command");
  return { path, sha256: hash(bytes), exit };
}
if (import.meta.main) {
  const args = process.argv.slice(2), actor = args[0] === "--actor" ? args.splice(0, 2)[1] : "verification-engineer";
  const [name, ...argv] = args;
  const result = await capture(resolve(import.meta.dir, "../.."), name, argv, actor);
  console.log(JSON.stringify(result));
  process.exitCode = result.exit;
}
