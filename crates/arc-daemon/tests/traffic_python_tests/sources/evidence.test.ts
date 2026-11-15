import { appendFileSync, existsSync } from "node:fs";
import { test, expect } from "bun:test";

test("held actual test body", async () => {
  appendFileSync(`${process.env.TBS_PROJECT}/bodies`, "body\n");
  const deadline = Date.now() + 120000;
  while (!existsSync(process.env.TBS_RELEASE!)) {
    if (Date.now() > deadline) throw new Error("explicit test release not received");
    await Bun.sleep(10);
  }
  expect(42).toBe(42);
  console.log("BUN_TEST_BODY_OK");
});
