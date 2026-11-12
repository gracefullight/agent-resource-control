import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync, readdirSync, lstatSync, readlinkSync } from "node:fs";
import { dirname, resolve, join } from "node:path";
import { platform, release, arch, hostname } from "node:os";

export const root = resolve(import.meta.dir, "../..");
export const acceptedHead = "f85ae0430038cc3d7a6125f650872dbfce6b5529";
const outputRoot=process.env.ARC_HARNESS_OUTPUT_ROOT ?? "target/terminal-outcomes/tooling-fixtures";
if(!outputRoot.startsWith("target/") || outputRoot.includes("\\") || outputRoot.split("/").some(part=>!part || part==="." || part==="..")) throw Error("HARNESS_OUTPUT: expected a canonical scratch path below target/");
export const evidence = resolve(root, outputRoot);
export const timeout = 3600000;
export const sha = (data: string | Buffer) => createHash("sha256").update(data).digest("hex");
export function sync(argv: string[], cwd = root, stdin?: Buffer) {
  const p = Bun.spawnSync(argv, { cwd, stdin, stdout: "pipe", stderr: "pipe", maxBuffer: 1024 ** 3 });
  if (p.exitCode) throw Error(`${argv.join(" ")}: ${p.exitCode}\n${p.stderr}`);
  return p.stdout;
}
export const oracle = () => sync(["git", "show", `${acceptedHead}:scripts/harness/validate.sh`]);
export function implementation() {
  return ["scripts/harness/validate.sh"].map(path => ({ path, sha256: sha(readFileSync(resolve(root, path))) }));
}
export function writeJSON(path: string, value: unknown) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, JSON.stringify(value, null, 2) + "\n");
}
export function files(dir: string): string[] {
  return readdirSync(dir).sort().flatMap(name => {
    const p = join(dir, name);
    return lstatSync(p).isDirectory() ? files(p) : [p];
  });
}
export function sourceIdentity() {
  const paths = [...new Set([
    ...sync(["git", "ls-files", "-z", "--", "scripts/harness", "scripts/coverage"]).toString().split("\0").filter(Boolean),
    ...files(resolve(root, "scripts/harness")).map(p => p.slice(root.length + 1)),
  ])].sort();
  const entries = paths.map(path => {
    const p = resolve(root, path), s = lstatSync(p);
    return { path, mode: s.mode & 0o777, sha256: sha(s.isSymbolicLink() ? readlinkSync(p) : readFileSync(p)) };
  });
  return { head: sync(["git", "rev-parse", "HEAD"]).toString().trim(), sha256: sha(JSON.stringify(entries)), entries };
}
export function environment(env = process.env) {
  return Object.fromEntries(["PATH", "LANG", "LC_ALL", "LC_CTYPE", "SKIP_COVERAGE", "WORKSPACE_ROOT", "TMPDIR",
    "RUSTUP_TOOLCHAIN", "CARGO_HOME", "CARGO_TARGET_DIR", "CARGO_NET_OFFLINE", "RUSTFLAGS", "RUSTC_WRAPPER"]
    .map(k => [k, env[k] ?? null]));
}
export function metadata(kind: string) {
  return { schemaVersion: 1, evidenceKind: kind, runId: crypto.randomUUID(), acceptedHead,
    acceptedTree: sync(["git", "rev-parse", `${acceptedHead}^{tree}`]).toString().trim(),
    oracleSha256: sha(oracle()), candidateScriptPath: "scripts/harness/validate.sh",
    candidateScriptSha256: implementation()[0].sha256, integratedSourceIdentity: sourceIdentity(),
    sourceScope: "current working harness and coverage sources", sourceRevision: sync(["git", "rev-parse", "HEAD"]).toString().trim(),
    host: { platform: platform(), release: release(), arch: arch(), hostname: hostname() },
    tools: Object.fromEntries([["bash", ["/bin/bash", "--version"]], ["grep", ["grep", "--version"]], ["git", ["git", "--version"]],
      ["bun", ["bun", "--version"]], ["rustc", ["rustc", "-Vv"]], ["cargo", ["cargo", "--version"]],
      ["cargoLlvmCov", ["cargo", "llvm-cov", "--version"]]].map(([name, argv]) => {
      const args = argv as string[], p = Bun.spawnSync(args, { stdout: "pipe", stderr: "pipe" });
      return [name as string, { path: Bun.which(args[0]), version: p.stdout.toString(), stderr: p.stderr.toString(), exit: p.exitCode }];
    })), clock: "performance.now() monotonic milliseconds" };
}
export async function observe(argv: string[], cwd: string, env: Record<string, string | undefined>, dir: string, onMark?: (key: string) => void) {
  mkdirSync(dir, { recursive: true });
  const startedAtUtc = new Date().toISOString(), start = performance.now();
  const child = Bun.spawn(argv, { cwd, env, stdout: "pipe", stderr: "pipe" });
  const deadline = Number(env.HARNESS_VERIFICATION_DEADLINE_MS ?? Infinity);
  const allowance = Math.min(timeout, deadline - Date.now());
  if (!(allowance > 0)) { child.kill("SIGTERM"); throw Error("verification deadline exhausted"); }
  const timer = setTimeout(() => child.kill("SIGTERM"), allowance);
  let stdout = Buffer.alloc(0), stderr = Buffer.alloc(0);
  const marks: Record<string, { ms: number; byteOffset: number }> = {};
  const labels = { check1: "Check 1/5: Hardcoded secrets", check2: "Check 2/5: Dangerous shell commands", check3: "Check 3/5: Architecture layer violations", check4: "Check 4/5: File length cap", check5: "Check 5/5: Test coverage thresholds", coverageDone: "Coverage thresholds met" };
  const drain = async (stream: ReadableStream, out: boolean) => {
    for await (const chunk of stream) {
      if (out) {
        stdout = Buffer.concat([stdout, Buffer.from(chunk)]);
        for (const [key, text] of Object.entries(labels)) {
          const offset = stdout.indexOf(text);
          if (offset >= 0 && !marks[key]) {
            marks[key] = { ms: performance.now() - start, byteOffset: offset };
            onMark?.(key);
          }
        }
      } else stderr = Buffer.concat([stderr, Buffer.from(chunk)]);
    }
  };
  const exitObserved = child.exited.then(exit => ({ exit, exitMs: performance.now() - start }));
  let result: { exit: number; exitMs: number };
  try { [result] = await Promise.all([exitObserved, drain(child.stdout, true), drain(child.stderr, false)]); }
  finally { clearTimeout(timer); }
  const totalMs = performance.now() - start;
  writeFileSync(join(dir, "stdout.txt"), stdout); writeFileSync(join(dir, "stderr.txt"), stderr);
  const receipt = { command: { argv, cwd, environment: environment(env) }, startedAtUtc, endedAtUtc: new Date().toISOString(),
    pid: child.pid, timeoutAllowanceMs: allowance, ...result!, clock: "performance.now()", marks, totalMs,
    durationsSeconds: { startup: marks.check1 ? marks.check1.ms / 1000 : null,
      checks1to4: marks.check5 && marks.check1 ? (marks.check5.ms - marks.check1.ms) / 1000 : null,
      processStartToCheck5: marks.check5 ? marks.check5.ms / 1000 : null,
      check5: marks.coverageDone && marks.check5 ? (marks.coverageDone.ms - marks.check5.ms) / 1000 : null,
      check1: marks.check1 && marks.check2 ? (marks.check2.ms - marks.check1.ms) / 1000 : null,
      check2: marks.check2 && marks.check3 ? (marks.check3.ms - marks.check2.ms) / 1000 : null,
      check3: marks.check3 && marks.check4 ? (marks.check4.ms - marks.check3.ms) / 1000 : null,
      check4: marks.check4 && marks.check5 ? (marks.check5.ms - marks.check4.ms) / 1000 : null,
      finalization: marks.coverageDone ? (totalMs - marks.coverageDone.ms) / 1000 : null, total: totalMs / 1000 },
    stdoutSha256: sha(stdout), stderrSha256: sha(stderr), stdoutBytes: stdout.length, stderrBytes: stderr.length,
    diagnostics: { directory: dir, retention: "scratch retained locally; not durable evidence" } };
  writeJSON(join(dir, "receipt.json"), receipt);
  return { receipt, stdout, stderr };
}
