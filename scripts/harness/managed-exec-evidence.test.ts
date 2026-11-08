import { test, expect } from "bun:test";
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve, join } from "node:path";
import { gitBytes } from "../coverage/hygiene";
import { hash } from "../coverage/source";
import { parseCargoExecution, validateCargoExecution } from "./managed-exec-test-output";
import { root } from "../coverage/acceptance";
import { managedInputs, nativeHost, validateCommandReceipt, validateFinalEvidence, requiredCommands, validateIndependentReview, validateTrackedEvidence, managedGit, validateSourceBinding } from "./managed-exec-evidence";
import "./managed-exec-coverage-identity.test";

// These are labeled validator unit fixtures, never command execution receipts.
// The final test below reads actual workspace/index, native artifacts and files.
const selectedTest = "nonempty_socket_override_and_empty_override_policy_are_preserved";
const selectedArgv = ["cargo", "test", "-p", "arc-core", "--test", "runtime", selectedTest, "--", "--exact"];
const unitOutput = `running 1 test\ntest ${selectedTest} ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.01s\n`;
function unitCommand(argv = ["cargo", "test", "--workspace"]) {
  return { schemaVersion: 1, actorId: "reviewer", sourceHash: "a".repeat(64), host: nativeHost(), argv, cwd: ".",
    started: "2026-11-08T09:00:00Z", ended: "2026-11-08T09:01:00Z", status: "passed", exit: 0,
    timeoutAllowanceMs: 3600000, skipFlags: false, stdoutSha256: hash(unitOutput), stderrSha256: hash(""),
    output: { stdout: unitOutput, stderr: "" }, execution: parseCargoExecution(unitOutput) };
}
test("receipt unit fixtures reject stale sources, foreign hosts, failures disguised as success and skips", () => {
  const source = "a".repeat(64), host = { ...nativeHost(), platform: "darwin" };
  const valid = { schemaVersion: 1, sourceHash: source, host, argv: ["cargo", "test", "--workspace"], cwd: ".",
    started: "2026-11-08T09:00:00Z", ended: "2026-11-08T09:01:00Z", exit: 0, status: "passed",
    stdoutSha256: "b".repeat(64), stderrSha256: "c".repeat(64), timeoutAllowanceMs: 3600000,
    skipFlags: false, actorId: "backend" };
  expect(() => validateCommandReceipt(valid, source, host)).not.toThrow();
  for (const patch of [
    { sourceHash: "d".repeat(64) }, { host: { ...host, platform: "linux" } }, { argv: [] }, { cwd: "/elsewhere" },
    { exit: 17 }, { status: "skipped" }, { ended: "invalid" }, { ended: "2026-11-08T08:00:00Z" },
    { stdoutSha256: "missing" }, { stderrSha256: null }, { timeoutAllowanceMs: 120000 }, { skipFlags: true },
    { environment: { SECRET: "sentinel" } }, { stdout: "raw output" }, { actorId: "reviewer" },
  ]) expect(() => validateCommandReceipt({ ...valid, ...patch }, source, host)).toThrow("MANAGED_EXEC_EVIDENCE");
});
test("source manifest covers actual Rust, migrations, tests, tooling and saved contract", () => {
  const inputs = managedInputs(root);
  expect(inputs.sha256).toMatch(/^[a-f0-9]{64}$/);
  for (const path of ["Cargo.toml", "Cargo.lock", "crates/arc-core/src/lib.rs", "crates/arc-cli/tests/binary.rs",
    "crates/arc-storage/migrations/0001_phase0.sql", "scripts/coverage/accounting.ts", "scripts/harness/validate.sh",
    "scripts/harness/managed-exec.test.ts", "scripts/harness/managed-exec-evidence.test.ts",
    ".arc-verify/managed-exec/contract.json", ".arc-verify/managed-exec/decision.md"]) {
    expect(inputs.files[path].sha256).toMatch(/^[a-f0-9]{64}$/);
    expect(inputs.files[path].mode).toBeGreaterThan(0);
  }
  expect(requiredCommands).toContainEqual(["./scripts/harness/validate.sh"]);
  expect(requiredCommands).toContainEqual(["bun", "test", "scripts/coverage/evidence.test.ts"]);
});
test("source binding unit fixtures reject edited input hashes, omitted inputs and stale Git identity", () => {
  const inputs = managedInputs(root), git = managedGit(root), summary = { inputs, git };
  expect(() => validateSourceBinding(summary, inputs, git)).not.toThrow();
  expect(() => validateSourceBinding({ ...summary, inputs: { ...inputs, sha256: "old" } }, inputs, git)).toThrow("current input/git");
  expect(() => validateSourceBinding({ ...summary, inputs: { ...inputs, files: {} } }, inputs, git)).toThrow("current input/git");
  expect(() => validateSourceBinding({ ...summary, git: { ...git, trackedDiffSha256: "old" } }, inputs, git)).toThrow("current input/git");
});
test("independent review unit fixtures reject self-review, stale binding and changed receipts/report", () => {
  const source = "a".repeat(64), host = nativeHost(), refs = [{ path: ".arc-verify/managed-exec/fmt.json", sha256: "b".repeat(64) }];
  const report = { path: ".arc-verify/managed-exec/qa.md", sha256: "c".repeat(64) };
  const qa = { schemaVersion: 1, actorId: "reviewer", skill: "oma-qa", status: "passed", sourceHash: source, host,
    reviewedReceipts: refs, report, commands: [unitCommand()] };
  expect(() => validateIndependentReview(qa, source, host, refs, report)).not.toThrow();
  for (const patch of [{ actorId: "backend" }, { status: "pending" }, { sourceHash: "old" }, { host: {} },
    { reviewedReceipts: [] }, { commands: [] }, { commands: [{ ...qa.commands[0], exit: 1 }] }, { report: { ...report, sha256: "changed" } }])
    expect(() => validateIndependentReview({ ...qa, ...patch }, source, host, refs, report)).toThrow("MANAGED_EXEC_EVIDENCE");
});
test("independent review unit fixtures require executable Cargo tests", () => {
  const source = "a".repeat(64), host = nativeHost(), refs: any[] = [], report = { path: ".arc-verify/managed-exec/qa.md", sha256: "b".repeat(64) };
  const command = unitCommand(selectedArgv);
  const qa = { schemaVersion: 1, actorId: "reviewer", skill: "oma-qa", status: "passed", sourceHash: source, host,
    reviewedReceipts: refs, report, commands: [unitCommand(), command] };
  expect(() => validateIndependentReview(qa, source, host, refs, report)).not.toThrow();
  for (const argv of [
    ["cargo", "test", "--help"], ["cargo", "test", "--no-run"], ["cargo", "test", "--", "--list"],
    ["cargo", "test", "-h"], ["cargo", "test", "--", "--help"], ["cargo", "test", "--", "-h"],
    ["cargo", "test", "--no-run=true"], ["cargo", "test", "--", "--list=true"],
    ["cargo", "test", "--workspace", "--no-run"], ["cargo", "test", "--test", "managed_exec", "--", "--list"],
    ["cargo", "test", null],
  ]) expect(() => validateIndependentReview({ ...qa, commands: [unitCommand(), { ...command, argv }] }, source, host, refs, report))
    .toThrow("MANAGED_EXEC_EVIDENCE");
});
test("tracked evidence unit fixture rejects 150 files, missing required artifacts and staged byte mismatch", () => {
  const dir = mkdtempSync(join(tmpdir(), "arc-managed-index-")), area = resolve(dir, ".arc-verify/managed-exec");
  try {
    gitBytes(dir, ["init", "--quiet"]); mkdirSync(area, { recursive: true });
    for (let i = 0; i < 149; i++) writeFileSync(join(area, `${i}.json`), "{}\n");
    gitBytes(dir, ["add", "--", ".arc-verify"]);
    expect(validateTrackedEvidence(dir, [".arc-verify/managed-exec/0.json"])).toBe(149);
    expect(() => validateTrackedEvidence(dir, [".arc-verify/managed-exec/missing.json"])).toThrow("not tracked");
    const file = join(area, "0.json"), bytes = readFileSync(file);
    writeFileSync(file, "changed");
    expect(() => validateTrackedEvidence(dir, [])).toThrow("unstaged"); writeFileSync(file, bytes);
    writeFileSync(join(area, "149.json"), "{}\n"); gitBytes(dir, ["add", "--", ".arc-verify"]);
    expect(() => validateTrackedEvidence(dir, [])).toThrow("between 1 and 149");
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
test("execution output unit fixture rejects skip-all and unmatched exact selectors", () => {
  const source = "a".repeat(64), host = nativeHost(), report = { path: ".arc-verify/managed-exec/qa.md", sha256: "b".repeat(64) };
  const command = unitCommand(["cargo", "test", "--workspace", "--", "--skip", ""]);
  const qa = { schemaVersion: 1, actorId: "reviewer", skill: "oma-qa", status: "passed", sourceHash: source, host,
    reviewedReceipts: [], report, commands: [command] };
  for (const argv of [command.argv, ["cargo", "test", "--workspace", "nonexistent_managed_exact_selector", "--", "--exact"]])
    expect(() => validateIndependentReview({ ...qa, commands: [unitCommand(), { ...command, argv }] }, source, host, [], report)).toThrow("MANAGED_EXEC_EVIDENCE");
});
test("execution output unit fixtures cross-check bytes, counts, selectors, host and cwd", () => {
  const command = unitCommand(selectedArgv);
  const zero = "running 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 0.00s\n";
  const ignored = "running 1 test\ntest fixture ... ignored\n\ntest result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s\n";
  expect(validateCargoExecution(command, root).passed).toBe(1);
  for (const patch of [
    { output: undefined }, { output: { stdout: unitOutput + "changed", stderr: "" } },
    { stdoutSha256: hash(zero), output: { stdout: zero, stderr: "" } },
    { stdoutSha256: hash(ignored), output: { stdout: ignored, stderr: "" } },
    { execution: { ...command.execution, passed: 99 } },
    { execution: { ...command.execution, tests: [] } },
    { output: { stdoutPath: "../escaped.txt", stderr: "" } },
    { argv: [...selectedArgv.slice(0, 6), "unmatched_exact_selector", "--", "--exact"] },
  ]) expect(() => validateCargoExecution({ ...command, ...patch }, root)).toThrow();
  for (const output of ["", "running 1 test\n", unitOutput.replace("1 passed", "2 passed"),
    unitOutput.replace(" ... ok", " ... FAILED"), "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n"])
    expect(() => parseCargoExecution(output)).toThrow("MANAGED_EXEC_EVIDENCE");
  const qa = { schemaVersion: 1, actorId: "reviewer", skill: "oma-qa", status: "passed", sourceHash: command.sourceHash,
    host: command.host, reviewedReceipts: [], report: {}, commands: [unitCommand(), command] };
  for (const patch of [{ cwd: "/wrong" }, { host: {} }, { actorId: "backend" }, { sourceHash: "old" }])
    expect(() => validateIndependentReview({ ...qa, commands: [unitCommand(), { ...command, ...patch }] },
      command.sourceHash, command.host, [], {})).toThrow("MANAGED_EXEC_EVIDENCE");
  expect(() => validateIndependentReview({ ...qa, commands: [command] }, command.sourceHash, command.host, [], {}))
    .toThrow("missing unfiltered independent cargo test --workspace");
  expect(parseCargoExecution(zero + unitOutput).passed).toBe(1);
});
test("real deterministic Cargo execution binds positive identity and actual output", async () => {
  const inputs = managedInputs(root), host = nativeHost(), started = new Date().toISOString();
  const child = Bun.spawn(selectedArgv, { cwd: root, stdout: "pipe", stderr: "pipe" });
  const timer = setTimeout(() => child.kill("SIGTERM"), 3600000);
  let result: [number, string, string];
  try { result = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()]); }
  finally { clearTimeout(timer); }
  const [exit, stdout, stderr] = result;
  expect(exit).toBe(0);
  const command = { schemaVersion: 1, actorId: "technical-director", sourceHash: inputs.sha256, host, argv: selectedArgv,
    cwd: ".", started, ended: new Date().toISOString(), status: "passed", exit, timeoutAllowanceMs: 3600000, skipFlags: false,
    stdoutSha256: hash(stdout), stderrSha256: hash(stderr), output: { stdout, stderr }, execution: parseCargoExecution(stdout) };
  validateCommandReceipt(command, inputs.sha256, host, "technical-director");
  expect(validateCargoExecution(command, root).tests).toContainEqual({ harness: 0, name: selectedTest, status: "ok" });
  expect(managedInputs(root)).toEqual(inputs);
  const local = resolve(root, "target/managed-exec");
  mkdirSync(local, { recursive: true });
  const run = mkdtempSync(join(local, "positive-execution-"));
  const receipt = resolve(run, "receipt.json");
  writeFileSync(receipt, JSON.stringify({
    scope: "task-5 validator integration; implementation evidence, not independent QA or lifecycle acceptance", workspace: root, inputs, command,
  }, null, 2) + "\n");
  // Retained evidence is promoted explicitly outside test execution, then
  // independently reviewed. Later runs must never invalidate that binding.
  console.log(`Validated Cargo execution receipt: ${receipt}`);
}, 3600000);
test("final native macOS evidence is current, independently reviewed and tracked below 150 files", () => {
  expect(validateFinalEvidence(root).pass).toBe(true);
});
