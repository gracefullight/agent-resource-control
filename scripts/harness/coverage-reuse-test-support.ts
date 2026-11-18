import { rmSync } from "node:fs";
import { reuseFixture } from "./coverage-reuse-fixture";

export function withFixture(run:(f:ReturnType<typeof reuseFixture>)=>void) {
  const f=reuseFixture();
  let passed=false;
  try {run(f);passed=true;} finally {if(passed) rmSync(f.root,{recursive:true,force:true});}
}
export async function withAsyncFixture(run:(f:ReturnType<typeof reuseFixture>)=>Promise<void>) {
  const f=reuseFixture();
  let passed=false;
  try {await run(f);passed=true;} finally {if(passed) rmSync(f.root,{recursive:true,force:true});}
}
export const HARNESS_WATCHDOG_MS=120_000;
export const HARNESS_TEST_WATCHDOG_MS=600_000;
