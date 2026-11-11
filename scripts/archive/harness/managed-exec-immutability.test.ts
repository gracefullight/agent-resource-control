import { expect, test } from "bun:test";
import { chmodSync, copyFileSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { tmpdir } from "node:os";
import { root } from "../coverage/acceptance";
import { gitBytes } from "../coverage/hygiene";
import { hash, walk } from "../../coverage/source";
import { directory, managedInputs, nativeHost, validateTrackedEvidence } from "./managed-exec-evidence";

// This file is deliberately separate from the invoked suite: no recursion or
// validator substitution. All staging happens in this test's owned checkout.
test("unfiltered evidence suite preserves retained bytes, membership, index and review bindings", async () => {
  const parent = resolve(root, "target/managed-exec");
  mkdirSync(parent, { recursive: true });
  const run = mkdtempSync(resolve(parent, "immutability-"));
  // Keep copied *.test.ts files outside the repository: Bun's path filters
  // otherwise discover previous fixture suites during later invocations.
  const fixture = resolve(mkdtempSync(resolve(tmpdir(), "arc-managed-immutability-")), "checkout");
  gitBytes(root, ["clone", "--quiet", "--shared", "--no-checkout", root, fixture]);
  const inputs = managedInputs(root);
  // Retained evidence includes indexed and pending nonignored artifacts. Local
  // diagnostics under ignored runs/ are not final evidence or staging inputs.
  const artifacts = [...new Set(gitBytes(root, ["ls-files", "-c", "-o", "--exclude-standard", "-z", "--", directory])
    .toString().split("\0").filter(Boolean))].sort();
  for (const path of [...Object.keys(inputs.files), ...artifacts]) {
    mkdirSync(dirname(resolve(fixture, path)), { recursive: true });
    copyFileSync(resolve(root, path), resolve(fixture, path));
    chmodSync(resolve(fixture, path), lstatSync(resolve(root, path)).mode & 0o777);
  }
  expect(managedInputs(fixture)).toEqual(inputs);
  gitBytes(fixture, ["read-tree", "HEAD"]);
  gitBytes(fixture, ["add", "--", directory]);
  const snapshot = () => ({
    artifacts: Object.fromEntries(walk(resolve(fixture, directory)).map(p => [relative(fixture, p), hash(readFileSync(p))])),
    index: gitBytes(fixture, ["ls-files", "--stage", "-z", "--", directory]).toString(),
    review: readFileSync(resolve(fixture, directory, "preservation-independent-review.json"), "utf8"),
  });
  const before = snapshot();
  expect(Object.keys(before.artifacts).sort()).toEqual(artifacts);
  for (const name of ["preservation-positive-execution.json", "preservation-independent-review.json", "preservation-independent-review.md"])
    expect(before.artifacts[`${directory}/${name}`]).toMatch(/^[a-f0-9]{64}$/);
  expect(validateTrackedEvidence(fixture, artifacts)).toBe(artifacts.length);
  const argv = ["bun", "test", "scripts/harness/managed-exec-evidence.test.ts"];
  const started = new Date().toISOString();
  const child = Bun.spawn(argv, { cwd: fixture, stdout: "pipe", stderr: "pipe" });
  const timer = setTimeout(() => child.kill("SIGTERM"), 3600000);
  let result: [number, string, string];
  try { result = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()]); }
  finally { clearTimeout(timer); }
  const [exit, stdout, stderr] = result, after = snapshot();
  writeFileSync(resolve(run, "stdout.txt"), stdout); writeFileSync(resolve(run, "stderr.txt"), stderr);
  writeFileSync(resolve(run, "receipt.json"), JSON.stringify({ argv, cwd: fixture, host: nativeHost(), inputs,
    started, ended: new Date().toISOString(), exit, timeoutAllowanceMs: 3600000,
    stdoutSha256: hash(stdout), stderrSha256: hash(stderr), before, after }, null, 2) + "\n");
  console.log(`Evidence immutability diagnostics: ${relative(root, run)}`);
  // Check immutability first, even when the genuine downstream gate is absent.
  expect(after).toEqual(before);
  expect(validateTrackedEvidence(fixture, artifacts)).toBe(artifacts.length);
  const output = stdout + stderr;
  expect(output).toContain("(pass) real deterministic Cargo execution binds positive identity and actual output");
  if (exit !== 0) {
    expect(output).toContain("(fail) final native macOS evidence is current, independently reviewed and tracked below 150 files");
    expect(output).toMatch(/\b1 fail\b/);
    expect(output).toMatch(/ENOENT|MANAGED_EXEC_EVIDENCE/);
  }
  // A genuine post-staging edit is still rejected by the real validator.
  const receipt = resolve(fixture, directory, "preservation-positive-execution.json");
  writeFileSync(receipt, readFileSync(receipt, "utf8") + "\n");
  expect(() => validateTrackedEvidence(fixture, artifacts)).toThrow("unstaged");
}, 3600000);
