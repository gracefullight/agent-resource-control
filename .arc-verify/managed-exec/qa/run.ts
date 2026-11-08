// Independent QA command capture. Product inputs are read, never written.
import { mkdirSync, writeFileSync, readFileSync, existsSync } from "node:fs";
import { resolve, relative } from "node:path";
import { managedInputs, nativeHost } from "../../../scripts/harness/managed-exec-evidence";
import { hash } from "../../../scripts/coverage/source";
import { parseCargoExecution } from "../../../scripts/harness/managed-exec-test-output";
const root = resolve(import.meta.dir, "../../.."), area = import.meta.dir;
const [label, ...argv] = process.argv.slice(2);
if (!/^[a-z0-9-]+$/.test(label ?? "") || !argv.length) throw Error("label/exact argv required");
const run = resolve(area, "runs", `${new Date().toISOString().replace(/[:.]/g, "-")}-${label}`);
mkdirSync(run, { recursive: true });
const inputs = managedInputs(root), host = nativeHost(), started = new Date().toISOString();
const child = Bun.spawn(argv, { cwd: root, stdout: "pipe", stderr: "pipe" });
console.log(`Independent ${label}: owned PID ${child.pid}; allowance 3600000ms`);
const timer = setTimeout(() => child.kill("SIGTERM"), 3600000);
const [exit, stdout, stderr] = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()]);
clearTimeout(timer);
for (const [stream, value] of [["stdout", stdout], ["stderr", stderr]]) writeFileSync(resolve(run, `${stream}.txt`), value);
const receipt = { schemaVersion: 1, actorId: "reviewer", sourceHash: inputs.sha256, host, argv, cwd: ".", started,
  ended: new Date().toISOString(), exit, status: exit === 0 ? "passed" : "failed", timeoutAllowanceMs: 3600000,
  skipFlags: false, pid: child.pid, stdoutSha256: hash(stdout), stderrSha256: hash(stderr),
  output: { stdoutPath: relative(root, resolve(run, "stdout.txt")), stderrPath: relative(root, resolve(run, "stderr.txt")) },
  ...(argv[0] === "cargo" && argv[1] === "test" ? { execution: parseCargoExecution(stdout) } : {}) };
writeFileSync(resolve(run, "receipt.json"), JSON.stringify(receipt, null, 2) + "\n");
const receiptsPath = resolve(area, "commands.json");
const receipts = existsSync(receiptsPath) ? JSON.parse(readFileSync(receiptsPath, "utf8")) : [];
receipts.push({ label, ...receipt });
writeFileSync(receiptsPath, JSON.stringify(receipts, null, 2) + "\n");
const unchanged = managedInputs(root).sha256 === inputs.sha256;
console.log(JSON.stringify({ label, exit, sourceUnchanged: unchanged, output: receipt.output,
  execution: receipt.execution ? { passed: receipt.execution.passed, ignored: receipt.execution.ignored, harnesses: receipt.execution.harnesses } : undefined }));
if (exit !== 0) console.log((stdout + stderr).slice(-5000));
process.exit(exit === 0 && unchanged ? 0 : 1);
