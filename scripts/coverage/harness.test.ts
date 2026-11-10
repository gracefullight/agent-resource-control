import { test, expect } from "bun:test";
import { realAcceptance, root } from "./acceptance";
import { checkPreservation } from "./preservation";
import { CRATES } from "./config";
test("real unchanged harness passes with zero violations and no skip flags",async()=>{
  checkPreservation(root);
  const result=await realAcceptance("harness",["./scripts/harness/validate.sh"]);
  expect(result.stdout).toContain("All checks passed (0 violations)");
  expect(result.stdout).toContain("Coverage thresholds met");
  expect(result.stderr).not.toContain("skipping coverage gate");
  expect(result.stderr).not.toContain("[VIOLATION]");
  for(const crate of CRATES) expect(result.stdout).toContain(`PASS ${crate}:`);
},7200000);
