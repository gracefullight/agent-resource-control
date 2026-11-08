import { test, expect } from "bun:test";
import { resolve, join } from "node:path";
import { mkdirSync, writeFileSync } from "node:fs";
import { fixtures, fixtureRoot, executeFixture } from "./fixtures";
import { root, evidence, metadata, writeJSON, sha, timeout, environment } from "./test-support";

test("accepted oracle and candidate have identical stdout, stderr, exits and coverage calls", () => {
  const receipt = { ...metadata("equivalence"), startedAtUtc: new Date().toISOString(), cases: [] as any[], status: "running" };
  let failed = false;
  for (const fixture of fixtures()) {
    const { dir, env } = fixtureRoot(fixture);
    const original = executeFixture(fixture, dir, env, false), candidate = executeFixture(fixture, dir, env, true);
    const equal = { stdout: original.stdout.equals(candidate.stdout), stderr: original.stderr.equals(candidate.stderr), exit: original.exit === candidate.exit, coverageCalls: original.calls === candidate.calls };
    const pass = Object.values(equal).every(Boolean); failed ||= !pass;
    if (!pass) {
      for (const [name, result] of [["oracle", original], ["candidate", candidate]] as const) {
        writeFileSync(join(dir, `${name}.stdout.txt`), result.stdout); writeFileSync(join(dir, `${name}.stderr.txt`), result.stderr);
      }
      console.error(`Mismatch ${fixture.id}: ${dir} ${JSON.stringify(equal)}`);
    }
    receipt.cases.push({ id: fixture.id, fixtureDefinitionSha256: sha(JSON.stringify(fixture)), equal, pass,
      command: { argv: ["/bin/bash", "scripts/harness/validate.sh", ...(fixture.args ?? (fixture.staged || fixture.emptyStaged ? ["--staged"] : []))], cwd: dir, environment: environment(env) },
      original: { ...original.hashes, exit: original.exit, stdoutBytes: original.stdout.length, stderrBytes: original.stderr.length }, candidate: { ...candidate.hashes, exit: candidate.exit, stdoutBytes: candidate.stdout.length, stderrBytes: candidate.stderr.length }, diagnostics: dir });
  }
  Object.assign(receipt, { endedAtUtc: new Date().toISOString(), status: failed ? "failed" : "passed", exit: failed ? 1 : 0,
    command: { argv: ["bun", "test", "scripts/harness/equivalence.test.ts"], cwd: root } });
  writeJSON(resolve(evidence, "equivalence.json"), receipt);
  expect(failed).toBe(false);
}, timeout);

test("real grep invocation trace scales with patterns rather than eligible files", () => {
  const runs = [2, 80].map(count => {
    const fixture = { id: `scaling-${count}`, content: Object.fromEntries(Array.from({ length: count }, (_, i) => [`domain/a${i}.sh`, ["lin", "api", "A".repeat(24)].join("_") + "\n"])) };
    const { dir, env } = fixtureRoot(fixture);
    const original = executeFixture(fixture, dir, env, false), candidate = executeFixture(fixture, dir, env, true);
    expect(candidate.stdout).toEqual(original.stdout); expect(candidate.stderr).toEqual(original.stderr); expect(candidate.exit).toBe(original.exit);
    // Each wrapper appends one separator. Concurrent argv writes may leave
    // adjacent separators; counting only nonempty records loses invocations.
    const invocations = (b: Buffer) => b.toString().split("\n").length - 1;
    writeFileSync(join(dir, "oracle.trace"), original.trace); writeFileSync(join(dir, "candidate.trace"), candidate.trace);
    return { count, original: invocations(original.trace), candidate: invocations(candidate.trace), diagnostics: dir, traceSha256: sha(candidate.trace) };
  });
  expect(runs[1].original).toBeGreaterThan(runs[0].original * 10);
  expect(runs[1].candidate).toBeLessThanOrEqual(runs[0].candidate + 2);
  expect(runs[1].candidate).toBeLessThan(40);
  writeJSON(resolve(evidence, "invocations.json"), { ...metadata("invocation-scaling"), status: "passed", runs });
}, timeout);
