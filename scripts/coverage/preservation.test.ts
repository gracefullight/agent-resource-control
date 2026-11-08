import { test, expect } from "bun:test";
import { resolve } from "node:path";
import { checkPreservation } from "./preservation";
import { BASELINE } from "./config";
test("accepted baseline protected bytes, production tokens and existing assertions survive",()=>{
  const result=checkPreservation(resolve(import.meta.dir,"../.."));
  expect(result.baselineHead).toBe(BASELINE);expect(result.pass).toBe(true);
  expect(result.existingRustFiles).toBeGreaterThan(0);
});
