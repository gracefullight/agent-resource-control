import { test, expect } from "bun:test";
import { readdirSync } from "node:fs";
import { resolve, join } from "node:path";
import { root, evidence, metadata, writeJSON, observe, timeout, sync } from "../../harness/test-support";

const separate = ["clean-checkout.test.ts", "coverage-cli.test.ts", "harness.test.ts", "evidence.test.ts"];
const nonCollecting = ["accounting.test.ts", "gate.test.ts", "hygiene-preservation.test.ts", "hygiene.test.ts", "preservation.test.ts"];
test("explicit serial coverage checks plus separate live checks cover the complete existing suite", async () => {
  const discovered = readdirSync(resolve(root, "scripts/coverage")).filter(p => p.endsWith(".test.ts")).sort();
  const accepted = sync(["git", "ls-tree", "-r", "--name-only", "f85ae0430038cc3d7a6125f650872dbfce6b5529", "--", "scripts/coverage"]).toString().trim().split("\n").filter(p => p.endsWith(".test.ts")).map(p => p.split("/").at(-1)!).sort();
  expect(discovered).toEqual([...nonCollecting, ...separate].sort());
  for (const path of accepted) expect(discovered).toContain(path);
  const receipt: any = { ...metadata("coverage-verification"), startedAtUtc: new Date().toISOString(), discovered, accepted,
    executed: nonCollecting, separatelyRequired: separate, inventoryComplete: true, commands: [], status: "running" };
  const save = () => writeJSON(resolve(evidence, "coverage-verification.json"), receipt);
  let failed = false;
  try {
    for (const file of nonCollecting) {
      const argv = ["bun", "test", `scripts/coverage/${file}`];
      const result = await observe(argv, root, process.env, resolve(root, "target/harness-speed/coverage-verification", receipt.runId, file));
      receipt.commands.push(result.receipt); save();
      failed ||= result.receipt.exit !== 0;
    }
    expect(failed).toBe(false);
    receipt.status = "passed"; receipt.exit = 0;
  } catch (error) { receipt.status = "failed"; receipt.exit = 1; receipt.error = String(error); throw error; }
  finally { receipt.endedAtUtc = new Date().toISOString(); save(); }
}, timeout * nonCollecting.length);
