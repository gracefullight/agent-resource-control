import { expect, test } from "bun:test";
import { readdirSync } from "node:fs";
import { resolve } from "node:path";
import { desktopEvidence } from "./desktop-evidence";

test("desktop evidence checks recorded gate outcomes and referenced raw log hashes", () => {
  const root=resolve(import.meta.dir,"../..");
  const result=desktopEvidence(root);
  expect(readdirSync(resolve(root,".arc-verify/desktop-foundation")).length).toBeLessThanOrEqual(7);
  expect(Object.keys(result.gateOutcomes)).toHaveLength(6);
  console.info(`Desktop evidence: ${JSON.stringify(result)}`);
});
