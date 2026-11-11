import { expect, test } from "bun:test";
import { lstatSync, readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import { account } from "./accounting";
import { CRATES } from "./config";
import { assertHygiene, auditTrackedEvidence, gitBytes, prohibitedReasons } from "./hygiene";
import { hash, inputIdentity, sourceInventory } from "./source";
import { parseCargoExecution } from "../archive/harness/managed-exec-test-output";

const root = resolve(import.meta.dir, "../..");
const directory = ".arc-verify/terminal-outcomes";
const load = (path: string) => JSON.parse(readFileSync(resolve(root, path), "utf8"));
const gates = () => load(`${directory}/gates.json`);

function bound(ref: { path: string; sha256: string; bytes?: number }, prefix = "target/") {
  expect(ref.path.startsWith(prefix)).toBe(true);
  expect(ref.path.split("/").some(part => part === ".." || part === ".")).toBe(false);
  const path = resolve(root, ref.path), stat = lstatSync(path);
  expect(stat.isFile() && !stat.isSymbolicLink()).toBe(true);
  const bytes = readFileSync(path);
  expect(hash(bytes)).toBe(ref.sha256);
  if (ref.bytes !== undefined) expect(bytes.length).toBe(ref.bytes);
  return bytes;
}

test("terminal-outcome gates bind current sources and successful native checks to raw output", () => {
  const report = gates();
  expect(report.host).toBe("Darwin");
  expect(report.acceptedHead).toBe("6b5218cf63b174b7976e52d81d56c7e3bf81112d");
  expect(gitBytes(root, ["rev-parse", "HEAD"]).toString().trim()).toBe(report.acceptedHead);
  const identity = inputIdentity(root);
  expect(report.sourceIdentity.sha256).toBe(identity.sha256);
  expect(JSON.parse(bound(report.sourceIdentity.manifest).toString())).toEqual(identity);
  expect(report.installation.acceptedSourceHashes).toHaveLength(18);
  for (const source of report.installation.acceptedSourceHashes)
    expect(hash(readFileSync(resolve(root, source.path)))).toBe(source.sha256);
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
    const stdout = bound(command.stdout).toString(), stderr = bound(command.stderr).toString();
    if (["workspace-tests", "terminal-outcomes", "status-compat"].includes(label)) {
      const execution = parseCargoExecution(stdout);
      expect(execution.ignored).toBe(0);
      if (label === "workspace-tests") expect(stdout).not.toMatch(/[1-9]\d* filtered out;/);
      if (label === "terminal-outcomes") for (const name of [
        "scheduled_exit_17_persists_failed_across_restart", "lifecycle_exit_17_persists_failed_across_restart",
      ]) expect(execution.tests.some(test => test.status === "ok" && test.name.endsWith(name))).toBe(true);
    }
    if (label === "harness") {
      expect(stdout).toContain("All checks passed (0 violations)");
      expect(stdout).toContain("Coverage thresholds met");
      expect(stderr).not.toContain("skipping coverage gate");
      for (const crate of CRATES) expect(stdout).toContain(`PASS ${crate}:`);
    }
  }
  expect(report.macTests.scheduledExit17).toBe("PASS");
  expect(report.macTests.lifecycleExit17).toBe("PASS");
  expect(report.harness.noSkips).toBe(true);
});

test("terminal-outcome coverage recomputes all seven production-line floors from fresh reports", () => {
  const report = gates(), coverage = report.coverage;
  expect(coverage.gatePass && coverage.verifiedFresh).toBe(true);
  expect(coverage.sourceHash).toBe(inputIdentity(root).sha256);
  const receipt = JSON.parse(bound(coverage.receipt, ".arc-verify/rust-coverage/").toString());
  const summary = JSON.parse(bound(coverage.summary, ".arc-verify/rust-coverage/").toString());
  expect(receipt.pass).toBe(true);
  expect(receipt.sourceHash).toBe(coverage.sourceHash);
  expect(receipt.runId).toBe(coverage.runId);
  expect(summary.sourceHash).toBe(coverage.sourceHash);
  const lcov = bound(coverage.rawReports.lcov, ".arc-verify/rust-coverage/").toString();
  const llvm = bound(coverage.rawReports.llvmJson, ".arc-verify/rust-coverage/").toString();
  expect(hash(lcov)).toBe(summary.rawSha256);
  expect(hash(llvm)).toBe(summary.llvmSha256);
  const recomputed = account(lcov, sourceInventory(root), root, undefined, JSON.parse(llvm));
  expect(recomputed.crates).toEqual(summary.crates);
  expect(recomputed.pass).toBe(true);
  expect(recomputed.crates.map(crate => crate.crate)).toEqual([...CRATES]);
  for (const crate of recomputed.crates) {
    expect(crate.covered * 100).toBeGreaterThanOrEqual(crate.total * 80);
    expect(coverage.crates[crate.crate]).toMatchObject(crate);
  }
  const native = receipt.receipts.find((command: any) => command.argv.includes("--no-report"));
  expect(native.exit).toBe(0);
  expect(native.argv).toContain("--workspace");
  expect(native.argv).toContain("--all-targets");
  expect(parseCargoExecution(bound(native.output.stdout).toString()).ignored).toBe(0);
  bound(native.output.stderr);
});

test("terminal-outcome migration checksums and accepted protocol-1 types remain unchanged", () => {
  for (const path of [
    ...readdirSync(resolve(root, "crates/arc-storage/migrations")).filter(name => name.endsWith(".sql"))
      .map(name => `crates/arc-storage/migrations/${name}`),
    "crates/arc-ipc/src/jobs/types.rs", "Cargo.toml", "Cargo.lock",
  ]) {
    const baseline = gitBytes(root, ["show", `${gates().acceptedHead}:${path}`]);
    expect(readFileSync(resolve(root, path))).toEqual(baseline);
  }
});

test("terminal-outcome evidence is compact and retired historical validators stay outside Bun discovery", () => {
  const paths = readdirSync(resolve(root, directory));
  expect(paths.length).toBeLessThan(30);
  let bytes = 0;
  for (const name of paths) {
    const path = `${directory}/${name}`, stat = lstatSync(resolve(root, path));
    expect(stat.isFile() && !stat.isSymbolicLink()).toBe(true);
    const content = readFileSync(resolve(root, path));
    bytes += content.length;
    expect(prohibitedReasons(path, content)).toEqual([]);
  }
  expect(bytes).toBeLessThan(5_000_000);
  assertHygiene(auditTrackedEvidence(root));
  const config = Bun.TOML.parse(readFileSync(resolve(root, "bunfig.toml"), "utf8"));
  expect(config.test.pathIgnorePatterns).toContain("scripts/archive/**");
  const archive = JSON.parse(bound(gates().activeSuite.archiveInventory).toString());
  expect(archive.files.length).toBeGreaterThan(0);
  for (const file of archive.files) {
    expect(file.to.startsWith("scripts/archive/")).toBe(true);
    expect(hash(readFileSync(resolve(root, file.to)))).toBe(file.archivedSha256);
    expect(hash(gitBytes(root, ["show", `${gates().acceptedHead}:${file.from}`]))).toBe(file.originalSha256);
  }
});
