import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { CRATES, PINS } from "../../scripts/coverage/config";
import { hash, inputIdentity } from "../../scripts/coverage/source";

const root = resolve(import.meta.dir, "../..");
const load = (path: string) => JSON.parse(readFileSync(resolve(root, path), "utf8"));
const results = load(".arc-verify/root-profile-hygiene/gate-results.json");
const baseline = load(".arc-verify/root-profile-hygiene/baseline.json");

function receipt(kind: "harness" | "coverage", argv: string[]) {
  const record = results[kind];
  if (record.status !== "passed") throw Error(`${kind} unavailable: ${record.reason}`);
  expect(record.argv).toEqual(argv);
  expect(record.exitCode).toBe(0);
  expect(record.skipCoverage).toBe(false);
  expect(Date.parse(record.startedAtUtc)).toBeGreaterThan(Date.parse(baseline.capturedAtUtc));
  expect(Date.parse(record.finishedAtUtc)).toBeGreaterThanOrEqual(Date.parse(record.startedAtUtc));
  const sourceHash = inputIdentity(root).sha256;
  expect(record.sourceBefore).toBe(sourceHash);
  expect(record.sourceAfter).toBe(sourceHash);
  expect(record.harnessSha256).toBe(hash(readFileSync(resolve(root, "scripts/harness/validate.sh"))));
  expect(record.coverageRunnerSha256).toBe(hash(readFileSync(resolve(root, "scripts/coverage/run.ts"))));
  const output = readFileSync(resolve(root, record.output.path), "utf8");
  expect(hash(output)).toBe(record.output.sha256);
  return { record, output, sourceHash };
}

test("fresh unchanged harness passed without skipping coverage", () => {
  const { output } = receipt("harness", ["./scripts/harness/validate.sh"]);
  expect(output).toContain("Coverage thresholds met");
  expect(output).toContain("All checks passed (0 violations).");
  expect(output).not.toContain("skipping coverage gate");
});

test("fresh six-crate coverage passed with a live source-bound receipt", () => {
  const { record, output, sourceHash } = receipt("coverage", ["bun", "run", "test:coverage"]);
  const latest = load(".arc-verify/rust-coverage/baseline/latest.json");
  const measured = load(`${latest.directory}/receipt.json`);
  expect(latest.pass).toBe(true);
  expect(latest.sourceHash).toBe(sourceHash);
  expect(latest.runId).toBe(record.runId);
  expect(measured.runId).toBe(latest.runId);
  expect(measured.pass).toBe(true);
  expect(measured.exit).toBe(0);
  expect(measured.sourceHash).toBe(sourceHash);
  const collection = measured.receipts.find((entry: any) => JSON.stringify(entry.argv) === JSON.stringify([
    "rustup", "run", PINS.rust, "cargo", "llvm-cov", "--workspace", "--all-targets", "--locked", "--offline",
    "--no-report", "--no-cfg-coverage", "--no-cfg-coverage-nightly",
  ]));
  expect(collection).toBeDefined();
  expect(collection.exit).toBe(0);
  expect(Date.parse(collection.started)).toBeGreaterThan(Date.parse(baseline.capturedAtUtc));
  expect(Date.parse(collection.ended)).toBeGreaterThanOrEqual(Date.parse(collection.started));
  expect(output).toContain(latest.directory);
  for (const crate of CRATES) expect(output).toContain(`PASS ${crate}:`);
});
