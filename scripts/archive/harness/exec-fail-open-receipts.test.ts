import { expect, spyOn, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { capturePending } from "./exec-fail-open-receipts";

test("new captures use and record the two-hour allowance", async () => {
  const workspace = mkdtempSync(resolve(tmpdir(), "arc-receipt-timeout-"));
  const timer = spyOn(globalThis, "setTimeout");
  const binding = () => ({ fixtureOnly: true, sourceHash: "fixture", observedGit: {} });
  try {
    const result = await capturePending(["bun", "-e", ""], "allowance", "target/exec-fail-open/pending", "backend", workspace, binding);
    expect(result.receipt.exit).toBe(0);
    expect(result.receipt.timeoutAllowanceMs).toBe(7_200_000);
    expect(timer.mock.calls.some(call => call[1] === 7_200_000)).toBe(true);
  } finally {
    timer.mockRestore();
    rmSync(workspace, { recursive: true, force: true });
  }
});
