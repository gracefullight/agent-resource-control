import { expect, test } from "bun:test";
import { lstatSync, readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import { TERMINAL_CRATES } from "./config";
import { assertHygiene, auditTrackedEvidence, gitBytes, prohibitedReasons } from "./hygiene";
import { hash } from "./source";
import { readEvidence, type EvidenceRef } from "./evidence";
import { terminalHistory, TERMINAL_DIRECTORY, TERMINAL_BASE, TERMINAL_DELIVERY } from "./historical";
import { parseCargoExecution } from "../archive/harness/managed-exec-test-output";

const root = resolve(import.meta.dir, "../..");
const history = terminalHistory(root), report = history.report;
function output(ref: EvidenceRef) {
  const bytes = readEvidence(root, ref, { optional: true, sizeRequired: false });
  if (bytes === undefined) {
    expect(history.unavailable).toContain(ref.path);
    expect(history.currentAcceptance).toBe(false);
  }
  return bytes?.toString();
}

test("terminal-outcome receipts bind their original source base and delivered overlay, independently of today's HEAD", () => {
  expect(report.host).toBe("Darwin");
  expect(history.sourceBase).toBe(TERMINAL_BASE);
  expect(history.deliveryCommit).toBe(TERMINAL_DELIVERY);
  expect(history.sourceOverlayVerified).toBe(24);
  expect(report.installation.acceptedSourceHashes).toHaveLength(18);
  expect(report.installation.repairOverlay).toHaveLength(6);
  const manifest = output(report.sourceIdentity.manifest);
  if (manifest !== undefined) expect(JSON.parse(manifest).sha256).toBe(report.sourceIdentity.sha256);
  const expected = {
    build: ["cargo", "build", "--workspace", "--bins"],
    fmt: ["cargo", "fmt", "--all", "--", "--check"],
    clippy: ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
    "workspace-tests": ["cargo", "test", "--workspace"],
    "terminal-outcomes": ["cargo", "test", "-p", "arc-daemon", "--test", "traffic_cli", "terminal_outcomes"],
    "status-compat": ["cargo", "test", "-p", "arc-daemon", "--test", "traffic_cli", "status_compat"],
    harness: ["./scripts/harness/validate.sh"],
  };
  for (const [label, argv] of Object.entries(expected)) {
    const command = report.commands.findLast((command: any) => command.label === label);
    expect(command.argv).toEqual(argv);
    expect(command.exitCode).toBe(0);
    expect(command.status).toBe("PASS");
    expect(command.timeoutMs).toBeGreaterThanOrEqual(7_200_000);
    const stdout = output(command.stdout), stderr = output(command.stderr);
    if (stdout !== undefined && ["workspace-tests", "terminal-outcomes", "status-compat"].includes(label)) {
      const execution = parseCargoExecution(stdout);
      expect(execution.ignored).toBe(0);
      if (label === "workspace-tests") expect(stdout).not.toMatch(/[1-9]\d* filtered out;/);
      if (label === "terminal-outcomes") for (const name of [
        "scheduled_exit_17_persists_failed_across_restart", "lifecycle_exit_17_persists_failed_across_restart",
      ]) expect(execution.tests.some(test => test.status === "ok" && test.name.endsWith(name))).toBe(true);
    }
    if (label === "harness") {
      if (stdout !== undefined) {
        expect(stdout).toContain("All checks passed (0 violations)");
        expect(stdout).toContain("Coverage thresholds met");
        for (const crate of TERMINAL_CRATES) expect(stdout).toContain(`PASS ${crate}:`);
      }
      if (stderr !== undefined) expect(stderr).not.toContain("skipping coverage gate");
    }
  }
  expect(report.macTests.scheduledExit17).toBe("PASS");
  expect(report.macTests.lifecycleExit17).toBe("PASS");
  expect(report.harness.noSkips).toBe(true);
});

test("terminal-outcome seven-crate coverage is preserved as historical claims with explicit raw availability", () => {
  const coverage = report.coverage;
  expect(coverage.gatePass && coverage.verifiedFresh).toBe(true);
  expect(coverage.sourceHash).toBe(report.sourceIdentity.sha256);
  expect(Object.keys(coverage.crates)).toEqual([...TERMINAL_CRATES]);
  for (const crate of TERMINAL_CRATES) {
    const measured = coverage.crates[crate];
    expect(measured.total).toBeGreaterThan(0);
    expect(measured.covered * 100).toBeGreaterThanOrEqual(measured.total * 80);
    expect(measured.percent).toBe(100 * measured.covered / measured.total);
    expect(measured.pass).toBe(true);
  }
  const receiptText = output(coverage.receipt), summaryText = output(coverage.summary);
  if (receiptText !== undefined) {
    const receipt = JSON.parse(receiptText);
    expect(receipt.pass).toBe(true);
    expect(receipt.sourceHash).toBe(coverage.sourceHash);
    expect(receipt.runId).toBe(coverage.runId);
    const native = receipt.receipts.find((command: any) => command.argv.includes("--no-report"));
    expect(native.exit).toBe(0);
    expect(native.argv).toContain("--workspace");
    expect(native.argv).toContain("--all-targets");
    const stdout = output(native.output.stdout);
    if (stdout !== undefined) expect(parseCargoExecution(stdout).ignored).toBe(0);
    output(native.output.stderr);
  }
  const lcov = output(coverage.rawReports.lcov), llvm = output(coverage.rawReports.llvmJson);
  if (summaryText !== undefined) {
    const summary = JSON.parse(summaryText);
    expect(summary.sourceHash).toBe(coverage.sourceHash);
    expect(summary.crates.map((crate: any) => crate.crate)).toEqual([...TERMINAL_CRATES]);
    if (lcov !== undefined) expect(hash(lcov)).toBe(summary.rawSha256);
    if (llvm !== undefined) expect(hash(llvm)).toBe(summary.llvmSha256);
  }
  // Current eight-crate recomputation lives in the desktop checker/accounting
  // gate. Missing predecessor output is never reconstructed or counted there.
  expect(history.currentAcceptance).toBe(false);
  if (history.unavailable.length) expect(history.gaps.length).toBeGreaterThan(0);
  console.info(`Historical terminal outcomes: ${JSON.stringify({ sourceBase: history.sourceBase, deliveryCommit: history.deliveryCommit,
    availableOutputs: history.available.length, unavailableOutputs: history.unavailable.length, gaps: history.gaps, currentAcceptance: false })}`);
});

test("current migration checksums and protocol-1 types stay equal to the accepted desktop baseline", () => {
  for (const path of [
    ...readdirSync(resolve(root, "crates/arc-storage/migrations")).filter(name => name.endsWith(".sql"))
      .map(name => `crates/arc-storage/migrations/${name}`),
    "crates/arc-ipc/src/jobs/types.rs",
  ]) expect(readFileSync(resolve(root, path)).equals(gitBytes(root, ["show", `${TERMINAL_DELIVERY}:${path}`]))).toBe(true);
});

test("terminal-outcome evidence and archived validators retain committed provenance and unchanged discovery", () => {
  const paths = readdirSync(resolve(root, TERMINAL_DIRECTORY));
  expect(paths.length).toBeLessThan(30);
  let bytes = 0;
  for (const name of paths) {
    const path = `${TERMINAL_DIRECTORY}/${name}`, stat = lstatSync(resolve(root, path));
    expect(stat.isFile() && !stat.isSymbolicLink()).toBe(true);
    const content = readFileSync(resolve(root, path));
    bytes += content.length;
    expect(prohibitedReasons(path, content)).toEqual([]);
  }
  expect(bytes).toBeLessThan(5_000_000);
  assertHygiene(auditTrackedEvidence(root));
  expect(readFileSync(resolve(root, "bunfig.toml")).equals(gitBytes(root, ["show", `${TERMINAL_DELIVERY}:bunfig.toml`]))).toBe(true);
  const archived = gitBytes(root, ["ls-tree", "-r", "--name-only", TERMINAL_DELIVERY, "--", "scripts/archive"]).toString().trim().split("\n");
  expect(archived).toHaveLength(report.activeSuite.archivedFiles);
  expect(archived.filter(path => path.endsWith(".test.ts"))).toHaveLength(report.activeSuite.archivedTestFiles);
  for (const path of archived) expect(readFileSync(resolve(root, path)).equals(gitBytes(root, ["show", `${TERMINAL_DELIVERY}:${path}`]))).toBe(true);
  const inventory = output(report.activeSuite.archiveInventory);
  if (inventory !== undefined) for (const file of JSON.parse(inventory).files) {
    expect(file.to.startsWith("scripts/archive/")).toBe(true);
    expect(hash(gitBytes(root, ["show", `${TERMINAL_DELIVERY}:${file.to}`]))).toBe(file.archivedSha256);
    expect(hash(gitBytes(root, ["show", `${TERMINAL_BASE}:${file.from}`]))).toBe(file.originalSha256);
  }
});
