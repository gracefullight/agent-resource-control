// Extract compact receipts from completed realAcceptance executions, never replay.
import { readFileSync, readdirSync, mkdirSync, writeFileSync, existsSync } from "node:fs";
import { resolve, relative } from "node:path";
import { managedInputs, nativeHost } from "../../../scripts/harness/managed-exec-evidence";
import { inputIdentity, hash } from "../../../scripts/coverage/source";
const root = resolve(import.meta.dir, "../../.."), area = resolve(import.meta.dir);
const [name, outerName, label] = process.argv.slice(2);
if (!["coverage-cli", "harness"].includes(name) || !outerName || !label) throw Error("name outer-receipt label required");
const outerPath = resolve(area, `${outerName}.json`);
const outer = JSON.parse(readFileSync(outerPath, "utf8"));
const inputs = managedInputs(root), native = inputIdentity(root);
if (outer.sourceHash !== inputs.sha256 || outer.exit !== 0) throw Error("outer source/outcome invalid");
const base = resolve(root, ".arc-verify/rust-coverage/tooling/acceptance/runs");
const candidates = readdirSync(base).filter(p => p.startsWith(`${name}-`)).flatMap(p => {
  const dir = resolve(base, p), bindingPath = resolve(dir, "binding.json");
  if (!existsSync(bindingPath)) return [];
  const binding = JSON.parse(readFileSync(bindingPath, "utf8"));
  return readdirSync(dir).filter(n => n.endsWith("-command.json")).map(n => ({
    path: resolve(dir, n), bindingPath, binding, command: JSON.parse(readFileSync(resolve(dir, n), "utf8")),
  }));
}).filter(({command: c}) => Date.parse(c.started) >= Date.parse(outer.started) && Date.parse(c.ended) <= Date.parse(outer.ended));
if (candidates.length !== 1) throw Error(`expected one real nested execution, got ${candidates.length}`);
const { path, bindingPath, binding, command: c } = candidates[0];
const expected = name === "coverage-cli" ? ["bun", "run", "test:coverage"] : ["./scripts/harness/validate.sh"];
if (c.cwd !== root || c.exit !== 0 || JSON.stringify(c.argv) !== JSON.stringify(expected) ||
    binding.sourceHash !== native.sha256 || binding.afterHash !== native.sha256 || !binding.unchanged || binding.exit !== 0)
  throw Error("nested command/source/cwd/outcome mismatch");
const out = resolve(area, "runs", `${label}-${crypto.randomUUID()}`); mkdirSync(out, { recursive: true });
for (const s of ["stdout", "stderr"] as const) writeFileSync(resolve(out, `${s}.txt`), c[s]);
const ref = (p: string) => ({path: relative(root, p), sha256: hash(readFileSync(p))});
const receipt = {schemaVersion: 1, actorId: "backend", sourceHash: inputs.sha256, host: nativeHost(),
  argv: c.argv, cwd: ".", started: c.started, ended: c.ended, exit: c.exit, status: "passed",
  timeoutAllowanceMs: 3600000, skipFlags: false, pid: c.pid, stdoutSha256: hash(c.stdout), stderrSha256: hash(c.stderr),
  output: {stdoutPath: relative(root, resolve(out, "stdout.txt")), stderrPath: relative(root, resolve(out, "stderr.txt"))},
  extractedFrom: {command: ref(path), binding: ref(bindingPath), outer: ref(outerPath), nativeInputHash: native.sha256}};
writeFileSync(resolve(area, `${label}.json`), JSON.stringify(receipt, null, 2) + "\n");
console.log(JSON.stringify({path: relative(root, resolve(area, `${label}.json`)), argv: c.argv, exit: c.exit, sourceHash: inputs.sha256}));
