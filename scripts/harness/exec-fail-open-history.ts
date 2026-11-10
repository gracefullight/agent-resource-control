import { existsSync, mkdirSync, readFileSync, writeFileSync, lstatSync } from "node:fs";
import { resolve, dirname, relative } from "node:path";
import { spawnSync } from "node:child_process";
import { gitBytes } from "../coverage/hygiene";
import { hash, walk } from "../coverage/source";
import { parseCargoExecution } from "./managed-exec-test-output";
export const baseline = "00773c69a8e5e7fd417fd5cc78d03e222364f609";
export const baselineTree = "b98f996caa04fe13eeeeab1072c3c528f6d18c65";
function requireValue(ok: unknown, message: string): asserts ok { if (!ok) throw Error(`HISTORY: ${message}`); }
export function historicalBinary(root: string) {
  requireValue(gitBytes(root, ["rev-parse", `${baseline}^{tree}`]).toString().trim() === baselineTree, "baseline tree mismatch");
  const base = resolve(root, "target/exec-fail-open/history"), cwd = resolve(base, "source"), target = resolve(base, "target");
  mkdirSync(cwd, { recursive: true });
  const paths = gitBytes(root, ["ls-tree", "-r", "--name-only", baseline, "--", "crates", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "mise.toml", ".cargo"])
    .toString().trim().split("\n");
  const files: Record<string, string> = {};
  for (const path of paths) {
    const bytes = gitBytes(root, ["show", `${baseline}:${path}`]);
    const dest = resolve(cwd, path);
    requireValue(dest.startsWith(cwd + "/"), "unsafe fixture path");
    if (!existsSync(dest)) { mkdirSync(dirname(dest), { recursive: true }); writeFileSync(dest, bytes); }
    requireValue(lstatSync(dest).isFile() && !lstatSync(dest).isSymbolicLink() && hash(readFileSync(dest)) === hash(bytes), `fixture source changed: ${path}`);
    files[path] = hash(bytes);
  }
  requireValue(JSON.stringify(walk(cwd).map(p => relative(cwd, p)).sort()) === JSON.stringify([...paths].sort()), "extra or missing historical fixture input");
  const tools = ["cargo", "rustc"].map(tool => {
    const r = spawnSync(tool, ["--version"], { encoding: "utf8" });
    requireValue(r.status === 0, `tool unavailable: ${tool}`); return r.stdout.trim();
  });
  const identity = hash(JSON.stringify({ baseline, baselineTree, files, tools }));
  const binary = resolve(target, "debug/arcctl"), receiptPath = resolve(base, "preparation.json");
  if (existsSync(receiptPath)) {
    const receipt = JSON.parse(readFileSync(receiptPath, "utf8"));
    requireValue(receipt.identity === identity && receipt.exit === 0 && receipt.execution.passed > 0 && receipt.execution.ignored === 0, "stale preparation");
    requireValue(existsSync(binary) && hash(readFileSync(binary)) === receipt.binary.sha256, "historical binary mismatch");
    return { binary, receipt };
  }
  const env = { ...process.env };
  for (const key of ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "LLVM_PROFILE_FILE", "CARGO_TARGET_DIR", "CARGO_BUILD_TARGET", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_LLVM_COV", "CARGO_LLVM_COV_TARGET_DIR"])
    delete env[key];
  for (const key of Object.keys(env)) if (/^(?:CARGO_LLVM_COV|LLVM_PROFILE|CARGO_PROFILE_|CARGO_TARGET_.*RUSTFLAGS)/.test(key)) delete env[key];
  const argv = ["cargo", "test", "-p", "arc-cli", "--locked", "--offline", "--target-dir", target];
  const started = new Date().toISOString(), start = performance.now();
  const r = spawnSync(argv[0], argv.slice(1), { cwd, env, encoding: "utf8", timeout: 3600000, maxBuffer: 20 * 1024 * 1024 });
  writeFileSync(resolve(base, "stdout.log"), r.stdout ?? ""); writeFileSync(resolve(base, "stderr.log"), r.stderr ?? "");
  requireValue(r.status === 0, `historical cargo test failed: ${r.error ?? r.stderr}`);
  const execution = parseCargoExecution(r.stdout);
  requireValue(execution.ignored === 0, "ignored historical tests");
  const receipt = { schemaVersion: 1, kind: "historical-preparation", baselineHead: baseline, baselineTree,
    identity, files, tools, cwd: relative(root, cwd), argv, pid: r.pid, started, ended: new Date().toISOString(),
    elapsedMs: performance.now() - start, timeoutAllowanceMs: 3600000, exit: r.status, execution,
    stdoutSha256: hash(r.stdout), stderrSha256: hash(r.stderr), binary: { path: relative(root, binary), sha256: hash(readFileSync(binary)) } };
  writeFileSync(receiptPath, JSON.stringify(receipt, null, 2) + "\n");
  return { binary, receipt };
}
if (import.meta.main) {
  const { binary } = historicalBinary(resolve(import.meta.dir, "../.."));
  process.stdout.write(binary + "\n");
}
