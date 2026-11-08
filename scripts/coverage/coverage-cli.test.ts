import { test, expect } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { realAcceptance, root } from "./acceptance";
import { CRATES } from "./config";
test("root entry point executes the real six-crate coverage gate",async()=>{
  const pkg=JSON.parse(readFileSync(resolve(root,"package.json"),"utf8"));
  expect(pkg.private).toBe(true);expect(pkg.scripts["test:coverage"]).toBe("bun scripts/coverage/run.ts");
  const result=await realAcceptance("coverage-cli",["bun","run","test:coverage"]);
  for(const crate of CRATES) expect(result.stdout).toContain(`PASS ${crate}:`);
  expect(result.stdout).not.toContain("UNAVAILABLE");
},960000);
