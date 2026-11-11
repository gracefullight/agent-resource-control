import { test, expect } from "bun:test";
import { readFileSync, existsSync } from "node:fs";
import { resolve, relative } from "node:path";
import { root, evidence, files, sourceIdentity, sha, writeJSON } from "../../harness/test-support";
import { auditTrackedEvidence, assertHygiene, prohibitedReasons } from "../coverage/hygiene";

test("all harness-speed evidence is bounded, compact and bound to current implementation", () => {
  const paths = files(evidence);
  // Include this test's receipt even on its first invocation.
  const finalCount = paths.length + (existsSync(resolve(evidence, "budget.json")) ? 0 : 1);
  expect(finalCount).toBeLessThan(150);
  const current = sourceIdentity();
  for (const path of paths) {
    const bytes = readFileSync(path);
    expect(bytes.length).toBeLessThan(1000000);
    expect(prohibitedReasons(relative(root, path), bytes)).toEqual([]);
  }
  const receipts = ["equivalence.json", "invocations.json", "timing.json", "coverage-verification.json"].map(name => {
    const path = resolve(evidence, name), bytes = readFileSync(path), receipt = JSON.parse(bytes.toString());
    expect(receipt.schemaVersion).toBe(1);
    expect(receipt.acceptedHead).toBe("f85ae0430038cc3d7a6125f650872dbfce6b5529");
    expect(receipt.candidateScriptSha256).toBe(sha(readFileSync(resolve(root, "scripts/harness/validate.sh"))));
    const specific = name === "timing.json" ? ["benchmark.ts", "performance.test.ts"] :
      name === "coverage-verification.json" ? ["coverage-verification.test.ts"] : ["fixtures.ts", "equivalence.test.ts"];
    const selected = current.entries.filter(e => e.path === "scripts/harness/validate.sh" || e.path === "scripts/harness/test-support.ts" ||
      specific.some(p => e.path === `scripts/harness/${p}`) || name === "coverage-verification.json" && e.path.startsWith("scripts/coverage/"));
    const bound = new Map(receipt.integratedSourceIdentity.entries.map((e: any) => [e.path, e]));
    for (const entry of selected) expect(bound.get(entry.path)).toEqual(entry);
    expect(["passed", "failed"]).toContain(receipt.status);
    expect(receipt.exit ?? (receipt.status === "passed" ? 0 : 1)).toBe(receipt.status === "passed" ? 0 : 1);
    expect(receipt.oracleSha256).toBe("93140173df8b8bd28b067aff68301134b0398b643240b0b1e6a13da4487ebe33");
    return { path: relative(root, path), sha256: sha(bytes) };
  });
  // Keep the earlier evidence-hygiene obligations live, including raw exports
  // renamed to arbitrary extensions and the independent old count budgets.
  const hygiene = auditTrackedEvidence(root); assertHygiene(hygiene);
  writeJSON(resolve(evidence, "budget.json"), { schemaVersion: 1, evidenceKind: "evidence-budget", status: "passed",
    checkedAtUtc: new Date().toISOString(), allFilesIncludingUntrackedAndNested: finalCount,
    trackedHarnessSpeedFiles: // The index count is separate from the recursive count.
      Bun.spawnSync(["git", "ls-files", "-z", "--", ".arc-verify/harness-speed"], { cwd: root }).stdout.toString().split("\0").filter(Boolean).length,
    sourceSha256: current.sha256, receipts, historicalHygiene: hygiene });
});
