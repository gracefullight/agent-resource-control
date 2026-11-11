import { existsSync, mkdirSync, writeFileSync, readFileSync, realpathSync } from "node:fs";
import { resolve } from "node:path";
import { spawn } from "node:child_process";
import { context, validateCommon, requireValue, digest, equal, canonicalBytes, boundJSON, reference, directory, root, hash } from "./exec-fail-open-inputs";
import { nativeCollection } from "./exec-fail-open-coverage";
import { parseCargoExecution, executableReviewTest } from "./managed-exec-test-output";
const captureTimeoutMs = 7_200_000;
export function commandOutput(root: string, receipt: any) {
  requireValue(receipt?.output && !receipt.stdout && !receipt.stderr && !receipt.environment, "raw or missing command output binding");
  const read = (stream: string) => {
    const path = receipt.output[`${stream}Path`];
    requireValue(typeof path === "string" && path.startsWith("target/exec-fail-open/"), "unsafe command output");
    const bytes = canonicalBytes(root, path);
    requireValue(hash(bytes) === receipt[`${stream}Sha256`], "command output digest mismatch"); return bytes.toString();
  };
  return { stdout: read("stdout"), stderr: read("stderr") };
}
export function validateCommand(receipt: any, ctx: ReturnType<typeof context>, root: string, actor = "backend") {
  validateCommon(receipt, ctx, "command", actor);
  requireValue(receipt.sourceHashAfter === ctx.sourceHash && equal(receipt.observedGitAfter, receipt.observedGit) && receipt.cwd === ".", "changed source/git or substituted cwd");
  requireValue(Array.isArray(receipt.argv) && receipt.argv.every((a: unknown) => typeof a === "string") && receipt.argv.length > 0, "missing actual argv");
  requireValue(!receipt.argv.some((a: string) => /^--(?:skip|ignored|no-run|list)(?:=|$)/.test(a)) && receipt.skipFlags === false, "skipped or nonexecuting command");
  requireValue(receipt.exit === 0 && receipt.status === "passed" && Number.isSafeInteger(receipt.pid) && receipt.pid > 0, "failed command or missing actual PID");
  requireValue(Number.isFinite(receipt.elapsedMs) && receipt.elapsedMs >= 0 && Date.parse(receipt.ended) >= Date.parse(receipt.started), "invalid elapsed measurements");
  requireValue(Number.isSafeInteger(receipt.timeoutAllowanceMs) && receipt.timeoutAllowanceMs >= 3600000, "per-check timeout too short");
  requireValue(digest(receipt.stdoutSha256) && digest(receipt.stderrSha256), "missing output digests");
  const executable = Bun.which(receipt.argv[0]);
  requireValue(executable && receipt.executable?.path === realpathSync(executable) && receipt.executable.sha256 === hash(readFileSync(executable)), "substituted executable or stale command binary");
  const output = commandOutput(root, receipt), combined = output.stdout + "\n" + output.stderr;
  if (receipt.argv[0] === "cargo" && receipt.argv[1] === "test") {
    requireValue(executableReviewTest(receipt.argv), "substitute Cargo command");
    const actual = parseCargoExecution(output.stdout);
    if (equal(receipt.argv, ["cargo", "test", "--workspace"])) requireValue(!/^test result:.*; [1-9]\d* filtered out;/m.test(output.stdout), "filtered workspace execution");
    requireValue(actual.ignored === 0 && actual.failed === 0 && equal(receipt.execution, actual), "test identity/count mismatch or ignored tests");
  } else if (receipt.argv[0] === "bun" && receipt.argv[1] === "test") {
    const passed = [...combined.matchAll(/^\s*(\d+) pass\s*$/gm)].map(m => Number(m[1]));
    requireValue(passed.length > 0 && passed.every(n => n > 0) && /^\s*0 fail\s*$/m.test(combined) && !/^\s*[1-9]\d* (?:fail|skip|todo)\s*$/m.test(combined), "Bun tests missing, failed or skipped");
  } else if (equal(receipt.argv, ["bun", "run", "test:coverage"]) || equal(receipt.argv, ["./scripts/harness/validate.sh"])) {
    // The maintained collector captures Cargo output in its own receipt rather
    // than echoing it. Read that real receipt and recompute its native profiles.
    const reports = [...output.stdout.matchAll(/^Coverage reports: (\.arc-verify\/rust-coverage\/baseline\/runs\/([A-Za-z0-9-]+))$/gm)];
    requireValue(reports.length === 1, "collector/harness did not report an actual native collection");
    const [, path, runId] = reports[0];
    nativeCollection(root, runId);
    const nested = JSON.parse(canonicalBytes(root, `${path}/receipt.json`).toString());
    const tests = nested.receipts.find((r: any) => r.argv?.[0] === "rustup" && r.argv[3] === "cargo" && r.argv[4] === "llvm-cov" && r.argv.includes("--workspace") && r.argv.includes("--all-targets") && r.argv.includes("--no-report"));
    requireValue(tests && tests.exit === 0 && tests.pid > 0 && tests.cwd === root && Date.parse(tests.started) >= Date.parse(receipt.started) && Date.parse(tests.ended) <= Date.parse(receipt.ended), "native Cargo execution missing or stale");
    const actual = parseCargoExecution(tests.stdout);
    requireValue(actual.ignored === 0 && actual.failed === 0, "native collector tests failed or ignored");
  }
  return receipt;
}
export function prerequisiteReceipts(root: string, gates: any, ctx: ReturnType<typeof context>) {
  const contract = JSON.parse(canonicalBytes(root, `${directory}/contract.json`).toString());
  const required: string[][] = contract.commandPhases.prerequisiteCommands;
  requireValue(Array.isArray(gates.commands) && gates.commands.length === required.length, "missing or extra prerequisite receipts");
  const receipts = gates.commands.map((ref: any) => validateCommand(boundJSON(root, ref), ctx, root));
  requireValue(new Set(receipts.map((r: any) => JSON.stringify(r.argv))).size === required.length && required.every(argv => receipts.some((r: any) => equal(r.argv, argv))), "exact prerequisite command partition differs");
  const live = receipts.find((r: any) => equal(r.argv, ["bun", "test", "scripts/coverage/evidence.test.ts"]));
  const collectors = receipts.filter((r: any) => equal(r.argv, ["bun", "run", "test:coverage"]) || r.argv[0] === "./scripts/harness/validate.sh" || ["clean-checkout", "coverage-cli", "harness"].some(n => equal(r.argv, ["bun", "test", `scripts/coverage/${n}.test.ts`])));
  requireValue(live && collectors.every((r: any) => Date.parse(r.ended) <= Date.parse(live.started)), "live coverage verification predates last collector");
  return receipts;
}
export async function capturePending(argv: string[], name: string, pending: string, actor = "backend", workspace = root, binding = () => context(workspace)) {
  requireValue(pending.startsWith("target/exec-fail-open/") && !pending.split("/").some(p => !p || p === "." || p === "..") && !/[\\\x00-\x1f\x7f]/.test(pending), "unsafe pending directory");
  return captureAt(argv, name, actor, workspace, binding, pending);
}
export async function capture(argv: string[], name: string, actor = "backend") {
  return captureAt(argv, name, actor, root, () => context(root));
}
async function captureAt(argv: string[], name: string, actor: string, root: string, binding: () => any, pending?: string) {
  const before = binding(), started = new Date().toISOString(), start = performance.now();
  requireValue(argv.length > 0 && /^[a-z0-9-]+$/.test(name), "invalid capture invocation");
  requireValue(!existsSync(resolve(root, pending ? `${pending}/${name}.json` : `${directory}/implementation/commands/${name}.json`)), "capture would overwrite an earlier attempt; use another name");
  const base = `target/exec-fail-open/commands/${name}-${crypto.randomUUID()}`;
  mkdirSync(resolve(root, base), { recursive: true });
  const output = { stdoutPath: `${base}/stdout.log`, stderrPath: `${base}/stderr.log` };
  const stdout: Buffer[] = [], stderr: Buffer[] = [];
  // No skip env is introduced or silently removed. Reject inherited bypasses.
  requireValue(!Object.entries(process.env).some(([k,v]) => /^(?:SKIP_|ARC_.*SKIP)/.test(k) && v && v !== "0"), "inherited skip environment");
  const child = spawn(argv[0], argv.slice(1), { cwd: root, env: process.env, stdio: ["ignore", "pipe", "pipe"] });
  child.stdout.on("data", data => stdout.push(data)); child.stderr.on("data", data => stderr.push(data));
  const timer = setTimeout(() => child.kill("SIGTERM"), captureTimeoutMs);
  const exit: number = await new Promise((ok, bad) => { child.once("error", bad); child.once("close", code => ok(code ?? 1)); });
  clearTimeout(timer);
  const out = Buffer.concat(stdout), err = Buffer.concat(stderr), after = binding();
  writeFileSync(resolve(root, output.stdoutPath), out); writeFileSync(resolve(root, output.stderrPath), err);
  const receipt = { ...before, kind: "command", actorId: actor, skill: actor === "reviewer" ? "oma-qa" : "oma-backend",
    argv, cwd: ".", started, ended: new Date().toISOString(), elapsedMs: performance.now()-start, pid: child.pid,
    exit, status: exit === 0 ? "passed" : "failed", sourceHashAfter: after.sourceHash, observedGitAfter: after.observedGit, timeoutAllowanceMs: captureTimeoutMs,
    executable: { path: realpathSync(Bun.which(argv[0])!), sha256: hash(readFileSync(Bun.which(argv[0])!)) },
    skipFlags: false, output, stdoutSha256: hash(out), stderrSha256: hash(err),
    ...(argv[0] === "cargo" && argv[1] === "test" && exit === 0 ? { execution: parseCargoExecution(out.toString()) } : {}) };
  const path = `${directory}/implementation/commands/${name}.json`;
  if (before.fixtureOnly) {
    const observation = `${base}/process.json`;
    writeFileSync(resolve(root, observation), JSON.stringify({ fixtureOnly: true, pid: child.pid, exit, stdoutSha256: hash(out), stderrSha256: hash(err) }));
    Object.assign(receipt, { processObservation: reference(root, observation) });
  }
  if (pending) {
    mkdirSync(resolve(root, pending), { recursive: true });
    requireValue(realpathSync(resolve(root, pending)) === resolve(realpathSync(root), pending), "unsafe pending parent");
    const bytes = Buffer.from(JSON.stringify(receipt, null, 2)+"\n");
    writeFileSync(resolve(root, pending, name+".json"), JSON.stringify({ path, sha256: hash(bytes), bytes: bytes.toString("base64") }), { flag: "wx" });
    requireValue(before.sourceHash === after.sourceHash && equal(before.observedGit, after.observedGit), "source/git changed while command ran");
    return { ref: { path, sha256: hash(bytes) }, receipt };
  }
  mkdirSync(resolve(root, directory, "implementation/commands"), { recursive: true });
  requireValue(!existsSync(resolve(root, path)), "capture would overwrite an earlier attempt; use another name");
  writeFileSync(resolve(root, path), JSON.stringify(receipt, null, 2)+"\n");
  requireValue(before.sourceHash === after.sourceHash && equal(before.observedGit, after.observedGit), "source/git changed while command ran");
  return { ref: reference(root, path), receipt };
}
if (import.meta.main) {
  const args = process.argv.slice(2);
  const pending = args[0] === "--pending" ? args.splice(0, 2)[1] : undefined;
  const [name, ...argv] = args;
  const result = pending ? await capturePending(argv, name, pending) : await capture(argv, name);
  console.log(JSON.stringify({ receipt: result.ref, exit: result.receipt.exit }));
  process.exitCode = result.receipt.exit;
}
