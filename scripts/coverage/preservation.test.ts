import { test, expect } from "bun:test";
import { resolve } from "node:path";
import { checkHistoricalPreservation } from "./preservation";
import { withHistoricalCheckout } from "./historical";
import { BASELINE } from "./config";
test("accepted baseline protected bytes, production tokens and existing assertions survive",()=>{
  const result=withHistoricalCheckout(resolve(import.meta.dir,"../.."), BASELINE, checkHistoricalPreservation);
  expect(result.baselineHead).toBe(BASELINE);expect(result.pass).toBe(true);
  expect(result.existingRustFiles).toBeGreaterThan(0);
});
